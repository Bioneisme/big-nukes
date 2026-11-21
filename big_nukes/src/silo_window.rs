//! The silo in the window (`silo.rs`'s `window` module): the pad's and the console's screens, the console's fields
//! and keys, the launch camera, and the notices the old glue showed as toasts.

use super::flights::GLIDE;
use crate::core::{self, argb, lock};
use crate::machines::LABEL;
use crate::rules;
use mod_api::{
    CameraPoseEvent, ClientHost, DrawContext, GuiRenderPostEvent, PayloadBytes, PayloadId, Screen, ScreenView,
};
use std::{
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

// ---- the pad's screen ------------------------------------------------------------

/// A furnace's size: the two slots side by side with their names under them.
const PAD_MISSILE_AT: [f32; 2] = [52.0, 30.0];
const PAD_WARHEAD_AT: [f32; 2] = [108.0, 30.0];

pub(crate) fn pad_screen() -> Option<Screen> {
    Some(Screen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some(PAD_MISSILE_AT), Some(PAD_WARHEAD_AT)],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

pub(crate) fn pad_draw(host: &dyn ClientHost, draw: &mut dyn DrawContext, origin: [f32; 2]) {
    let [x, y] = origin;
    let mut centered = |text: &str, middle: f32, top: f32| {
        let width = draw.text_width(text);
        draw.text(text, [x + middle - width / 2.0, y + top], argb(LABEL), false);
    };
    centered(&core::lang(host, "big_nukes.screen.launch_pad"), 88.0, 6.0);
    centered(&core::lang(host, "big_nukes.screen.launch_pad.missile"), PAD_MISSILE_AT[0] + 8.0, PAD_MISSILE_AT[1] + 22.0);
    centered(&core::lang(host, "big_nukes.screen.launch_pad.warhead"), PAD_WARHEAD_AT[0] + 8.0, PAD_WARHEAD_AT[1] + 22.0);
    draw.text(&core::lang(host, "container.inventory"), [x + 8.0, y + 72.0], argb(LABEL), false);
}

// ---- the console, in the window ---------------------------------------------------
//
// The fields live in the window: digits come from the keyboard (the screen's key and character hooks) and from the
// screen's keys. A launch is queued and sent to the server from the next frame (the screen's hooks have the window
// read-only).

#[derive(Default)]
struct ConsoleUi {
    /// The console the window last opened, as the server told it.
    at: Option<[i32; 3]>,
    input: rules::ConsoleInput,
    /// The server's last answer, shown a while over the shaft's state.
    reply: Option<(Instant, rules::Reply)>,
    /// Launches to send.
    outbox: Vec<Vec<u8>>,
}

static CONSOLE_UI: Mutex<Option<ConsoleUi>> = Mutex::new(None);
/// The launch camera, on until the player turns it off; the window keeps it until the game closes.
static CAMERA_ON: AtomicBool = AtomicBool::new(true);
/// How long an answer stays on the screen.
const REPLY_FOR: Duration = Duration::from_secs(6);
/// When the window started drawing, for the caret's blink.
static START: OnceLock<Instant> = OnceLock::new();

fn with_ui<R>(f: impl FnOnce(&mut ConsoleUi) -> R) -> R {
    f(lock(&CONSOLE_UI).get_or_insert_with(ConsoleUi::default))
}

/// A mod message in the window: `CONSOLE_CHANNEL` (the console's news) and `NOTICE_CHANNEL` (a notice).
pub(crate) fn net_from_server(host: &mut dyn ClientHost, channel: &str, data: &[u8]) {
    if channel == rules::NOTICE_CHANNEL {
        notice(data);
        return;
    }
    if channel != rules::CONSOLE_CHANNEL {
        return;
    }
    let news = match serde_json::from_slice::<rules::ConsoleNews>(data) {
        Ok(news) => news,
        Err(e) => {
            log::warn!("[big_nukes] the console's news: {e}");
            return;
        }
    };
    if let Some(flight) = news.missile.clone() {
        follow(host, flight, news.now);
    }
    with_ui(|ui| {
        if let Some(at) = news.at {
            // Another console starts empty; the same one keeps what was typed.
            if ui.at != Some(at) {
                ui.input = rules::ConsoleInput::default();
            }
            ui.at = Some(at);
            ui.reply = None;
        }
        if let Some(reply) = news.reply {
            ui.reply = Some((Instant::now(), reply));
        }
    });
}

fn ours(screen: &ScreenView<'_>) -> bool {
    screen.menu == Some(rules::CONSOLE_MENU)
}

/// A key over a screen: on the console's, every key but Escape and the function keys is the console's, so none
/// closes the screen or moves an item. Whether it was taken.
pub(crate) fn key_pressed(_host: &dyn ClientHost, screen: &ScreenView<'_>, key: &str) -> bool {
    if !ours(screen) || !rules::console_takes(key) {
        return false;
    }
    with_ui(|ui| {
        if ui.input.key(key) == rules::Keyed::Launch {
            queue_launch(ui);
        }
    });
    true
}

/// A character typed at a screen: the console's fields take it. Whether it was taken.
pub(crate) fn char_typed(_host: &dyn ClientHost, screen: &ScreenView<'_>, c: char) -> bool {
    if !ours(screen) {
        return false;
    }
    with_ui(|ui| ui.input.typed(c));
    true
}

fn queue_launch(ui: &mut ConsoleUi) {
    match ui.at {
        Some(at) => ui.outbox.push(ui.input.request(at, CAMERA_ON.load(Ordering::Relaxed)).bytes()),
        None => ui.reply = Some((Instant::now(), rules::Reply::new("big_nukes.console.gone", &[]))),
    }
}

/// Every frame (`render_frame_pre`): the launches the console's screen queued go to the server.
pub(crate) fn frame(host: &mut dyn ClientHost) {
    let outbox = with_ui(|ui| std::mem::take(&mut ui.outbox));
    for bytes in outbox {
        if let Err(e) = host.send_payload_to_server(&PayloadId(rules::LAUNCH_CHANNEL.into()), PayloadBytes(bytes)) {
            let e = format!("{e:?}");
            with_ui(|ui| ui.reply = Some((Instant::now(), rules::Reply::new("big_nukes.console.send_failed", &[&e]))));
        }
    }
}

// The screen, a keypad's size: the three fields, a line of the shaft's state or the last answer, twelve keys, and
// Erase, Launch and Camera on the right; then the inventory, as the game lays it.

const CONSOLE_HEIGHT: f32 = 240.0;
const CONSOLE_INVENTORY_Y: f32 = 158.0;
/// The fields: x, y, width, height. X, Z and the code in a row under their labels; the spread on a row of its own
/// under the buttons, its label left of it.
const FIELDS: [[f32; 4]; 4] =
    [[8.0, 28.0, 52.0, 14.0], [64.0, 28.0, 52.0, 14.0], [120.0, 28.0, 48.0, 14.0], [120.0, 128.0, 48.0, 14.0]];
const STATUS_Y: f32 = 45.0;
const KEYS_AT: [f32; 2] = [8.0, 66.0];
const KEY: [f32; 2] = [22.0, 13.0];
const KEY_GAP: f32 = 2.0;
/// The keys row by row: what each types.
const KEY_ROWS: [[char; 3]; 4] = [['1', '2', '3'], ['4', '5', '6'], ['7', '8', '9'], ['-', '0', '<']];
const ERASE_AT: [f32; 4] = [86.0, 66.0, 82.0, 13.0];
const LAUNCH_AT: [f32; 4] = [86.0, 81.0, 82.0, 28.0];
const CAMERA_AT: [f32; 4] = [86.0, 111.0, 82.0, 13.0];

fn console_keys() -> impl Iterator<Item = (char, [f32; 4])> {
    KEY_ROWS.into_iter().enumerate().flat_map(|(row, keys)| {
        keys.into_iter().enumerate().map(move |(col, c)| {
            let x = KEYS_AT[0] + col as f32 * (KEY[0] + KEY_GAP);
            let y = KEYS_AT[1] + row as f32 * (KEY[1] + KEY_GAP);
            (c, [x, y, KEY[0], KEY[1]])
        })
    })
}

fn inside(at: [f32; 2], [x, y, w, h]: [f32; 4]) -> bool {
    at[0] >= x && at[0] < x + w && at[1] >= y && at[1] < y + h
}

pub(crate) fn console_screen() -> Option<Screen> {
    Some(Screen {
        width: 176.0,
        height: CONSOLE_HEIGHT,
        // The menu's one slot is hidden.
        slots: vec![None],
        inventory_y: CONSOLE_INVENTORY_Y,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

/// A press on the console's screen over no slot (its keys and buttons are the window's own, no menu button): a
/// field takes the focus, a key types, a button acts. Nothing goes to the server as a menu button.
pub(crate) fn console_click(at: [f32; 2], right: bool) {
    if right {
        return;
    }
    with_ui(|ui| {
        if let Some(field) = FIELDS.iter().position(|f| inside(at, *f)) {
            ui.input.set_focus(field);
        } else if let Some((c, _)) = console_keys().find(|(_, r)| inside(at, *r)) {
            if c == '<' {
                ui.input.key("key.keyboard.backspace");
            } else {
                ui.input.typed(c);
            }
        } else if inside(at, ERASE_AT) {
            ui.input.clear();
        } else if inside(at, LAUNCH_AT) {
            queue_launch(ui);
        } else if inside(at, CAMERA_AT) {
            CAMERA_ON.fetch_xor(true, Ordering::Relaxed);
        }
    });
}

/// `line` broken at its spaces into lines no wider than `width`.
fn wrap(draw: &dyn DrawContext, line: &str, width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in line.split(' ') {
        match lines.last_mut() {
            Some(last) if draw.text_width(&format!("{last} {word}")) <= width => {
                last.push(' ');
                last.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

fn button(draw: &mut dyn DrawContext, origin: [f32; 2], [bx, by, bw, bh]: [f32; 4], label: &str, face: u32, lit: bool) {
    let [x, y] = origin;
    draw.fill([x + bx, y + by, bw, bh], 0xFF2A_2A2A);
    let face = if lit { face.saturating_add(0x0018_1818) } else { face };
    draw.fill([x + bx + 1.0, y + by + 1.0, bw - 2.0, bh - 2.0], face);
    let width = draw.text_width(label);
    draw.text(label, [x + bx + (bw - width) / 2.0, y + by + (bh - 8.0) / 2.0], argb(0xFF_FFFF), true);
}

pub(crate) fn console_draw(host: &dyn ClientHost, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    let [x, y] = origin;
    let (input, reply) = with_ui(|ui| (ui.input.clone(), ui.reply.clone().filter(|(at, _)| at.elapsed() < REPLY_FOR)));
    let centered = |draw: &mut dyn DrawContext, text: &str, top: f32, rgb: u32| {
        let width = draw.text_width(text);
        draw.text(text, [x + (176.0 - width) / 2.0, y + top], argb(rgb), false);
    };
    centered(draw, &core::lang(host, "big_nukes.screen.console"), 6.0, LABEL);
    // The fields, the code as stars, a caret in the focused one.
    let caret = START.get_or_init(Instant::now).elapsed().as_millis() / 500 % 2 == 0;
    for (i, [fx, fy, fw, fh]) in FIELDS.into_iter().enumerate() {
        let label = match i {
            rules::FIELD_X => "X".to_owned(),
            rules::FIELD_Z => "Z".to_owned(),
            rules::FIELD_SPREAD => core::lang(host, "big_nukes.screen.console.spread"),
            _ => core::lang(host, "big_nukes.screen.console.code"),
        };
        if i == rules::FIELD_SPREAD {
            let width = draw.text_width(&label);
            draw.text(&label, [x + fx - width - 4.0, y + fy + 3.0], argb(LABEL), false);
        } else {
            draw.text(&label, [x + fx, y + fy - 10.0], argb(LABEL), false);
        }
        let edge = if input.focus == i { 0xFFFF_FFFF } else { 0xFF37_3737 };
        draw.fill([x + fx, y + fy, fw, fh], edge);
        draw.fill([x + fx + 1.0, y + fy + 1.0, fw - 2.0, fh - 2.0], 0xFF10_1810);
        let shown = if i == rules::FIELD_CODE { "*".repeat(input.fields[i].len()) } else { input.fields[i].clone() };
        let shown = if input.focus == i && caret { format!("{shown}_") } else { shown };
        draw.text(&shown, [x + fx + 3.0, y + fy + 3.0], argb(0x55_FF55), false);
    }
    // The answer for a while, else the shaft's state.
    let state = rules::ConsoleState::of_data(data);
    let (line, rgb) = match (&reply, state) {
        (Some((_, r)), _) => (r.clone(), if r.key == "big_nukes.console.launched" { 0x00_8800 } else { 0xAA_0000 }),
        (None, Some(s)) => (s.line(), if s == rules::ConsoleState::Ready { 0x00_8800 } else { 0xAA_0000 }),
        (None, None) => (rules::Reply::default(), LABEL),
    };
    if !line.key.is_empty() {
        let args: Vec<&str> = line.args.iter().map(String::as_str).collect();
        let said = core::lang_tr(host, &line.key, &args);
        for (k, l) in wrap(draw, &said, 168.0).iter().take(2).enumerate() {
            centered(draw, l, STATUS_Y + k as f32 * 10.0, rgb);
        }
    }
    let cursor = host.cursor();
    let at = [cursor[0] - x, cursor[1] - y];
    for (c, r) in console_keys() {
        button(draw, origin, r, &c.to_string(), 0xFF8C_8C8C, inside(at, r));
    }
    button(draw, origin, ERASE_AT, &core::lang(host, "big_nukes.screen.console.erase"), 0xFF6E_6E6E, inside(at, ERASE_AT));
    button(draw, origin, LAUNCH_AT, &core::lang(host, "big_nukes.screen.console.launch"), 0xFFA8_2020, inside(at, LAUNCH_AT));
    let camera = if CAMERA_ON.load(Ordering::Relaxed) {
        core::lang(host, "big_nukes.screen.console.camera_on")
    } else {
        core::lang(host, "big_nukes.screen.console.camera_off")
    };
    button(draw, origin, CAMERA_AT, &camera, 0xFF4A_5A6E, inside(at, CAMERA_AT));
    draw.text(&core::lang(host, "container.inventory"), [x + 8.0, y + CONSOLE_INVENTORY_Y - 11.0], argb(LABEL), false);
}

// ---- the launch camera ------------------------------------------------------------
//
// The server tells the window of the missile it launched with the camera on; the window works the camera out from
// its own clock each frame, and lets go once the missile is high or the player turns it off.

/// The missile followed: the flight, the window's game time less the world's tick when it was told, and the
/// player's eyes then, whose side the camera keeps.
static CAMERA: Mutex<Option<(rules::Flight, f64, [f64; 3])>> = Mutex::new(None);

fn follow(host: &dyn ClientHost, flight: rules::Flight, now: u64) {
    let (Some(clock), Some(player)) = (host.clock(), host.local_player()) else { return };
    let Some(eye) = player.entity().eye_position(host) else { return };
    let offset = clock.game_time as f64 - now as f64;
    *lock(&CAMERA) = Some((flight, offset, eye));
}

/// The launch camera, for the one who launched.
pub(crate) fn camera_pose(host: &dyn ClientHost, event: &mut CameraPoseEvent<'_>) {
    let Some((flight, offset, eye)) = lock(&CAMERA).clone() else { return };
    if !CAMERA_ON.load(Ordering::Relaxed) {
        return;
    }
    let (Some(clock), Some(player)) = (host.clock(), host.local_player()) else { return };
    if player.dimension(host).is_none_or(|d| core::identifier(&d) != flight.dimension) {
        return;
    }
    // The display glides its last ticks behind its place: the camera looks where it is seen.
    let now = clock.game_time as f64 + f64::from(event.partial) - offset - f64::from(GLIDE);
    match rules::launch_camera(&flight, now, eye) {
        Some((at, yaw, pitch)) => {
            *event.position = at;
            event.yaw = yaw;
            event.pitch = pitch;
        }
        None if now > flight.liftoff() as f64 => *lock(&CAMERA) = None,
        None => {}
    }
}

/// The window drops a level: the launch camera goes.
pub(crate) fn forget_camera() {
    *lock(&CAMERA) = None;
}

// ---- the notices ------------------------------------------------------------------
//
// The old glue added a launch's, a storm's or a satellite's notice to the game's toasts. The API has no toast of
// a mod's own (`Hooks::toast_add` only filters the game's), so the window draws them itself, as toasts look: down
// from the top right, each a few seconds, sliding in and out.

/// The notices heard, with when each was.
static NOTICES: Mutex<Vec<(Instant, rules::Toast)>> = Mutex::new(Vec::new());
/// How long a notice shows, and how long it takes to slide in or out.
const NOTICE_FOR: Duration = Duration::from_millis(5000);
const NOTICE_SLIDE: f32 = 0.25;
/// The most notices shown at once; the rest wait their turn.
const NOTICES_SHOWN: usize = 4;
const NOTICE_WIDTH: f32 = 160.0;

fn notice(data: &[u8]) {
    let notice = match serde_json::from_slice::<rules::Notice>(data) {
        Ok(n) => n,
        Err(e) => {
            log::warn!("[big_nukes] a notice: {e}");
            return;
        }
    };
    let mut notices = lock(&NOTICES);
    if notices.len() < 32 {
        // A later notice waits for a free place: its time starts when it shows.
        let start = notices.iter().rev().nth(NOTICES_SHOWN - 1).map_or_else(Instant::now, |(at, _)| (*at + NOTICE_FOR).max(Instant::now()));
        notices.push((start, rules::toast_of(&notice)));
    }
}

/// Over everything on the screen (`gui_render_post`): the notices the old glue showed as toasts.
pub(crate) fn gui_render_post(host: &dyn ClientHost, event: &mut GuiRenderPostEvent<'_>) {
    let shown: Vec<(f32, rules::Toast)> = {
        let mut notices = lock(&NOTICES);
        if notices.is_empty() {
            return;
        }
        let now = Instant::now();
        notices.retain(|(at, _)| now < *at + NOTICE_FOR);
        notices
            .iter()
            .filter(|(at, _)| *at <= now)
            .take(NOTICES_SHOWN)
            .map(|(at, t)| {
                let age = now.duration_since(*at).as_secs_f32();
                let left = NOTICE_FOR.as_secs_f32() - age;
                // How far in it has slid: 0 off the screen, 1 all in.
                (age.min(left).min(NOTICE_SLIDE) / NOTICE_SLIDE, t.clone())
            })
            .collect()
    };
    let draw = &mut *event.batch;
    let [width, _] = draw.size();
    let mut top = 0.0;
    for (slid, toast) in shown {
        let title = core::lang(host, toast.title);
        let args: Vec<&str> = toast.args.iter().map(String::as_str).collect();
        let lines = wrap(draw, &core::lang_tr(host, toast.description, &args), NOTICE_WIDTH - 36.0);
        let height = (20.0 + 10.0 * lines.len() as f32).max(32.0);
        let x = width - NOTICE_WIDTH * slid;
        draw.fill([x, top, NOTICE_WIDTH, height], 0xFF00_0000);
        draw.fill([x + 1.0, top + 1.0, NOTICE_WIDTH - 2.0, height - 2.0], 0xFF55_5555);
        draw.fill([x + 2.0, top + 2.0, NOTICE_WIDTH - 4.0, height - 4.0], 0xF020_2020);
        if let Some(icon) = core::item_key(host, toast.icon) {
            draw.item(icon, [x + 8.0, top + (height - 16.0) / 2.0], 16.0);
        }
        // A challenge's title in purple, the rest's in yellow, as the game's advancement toasts.
        let color = if toast.challenge { 0xFFFF_88FF } else { 0xFFFF_FF00 };
        draw.text(&title, [x + 30.0, top + 7.0], color, false);
        for (i, line) in lines.iter().enumerate() {
            draw.text(line, [x + 30.0, top + 18.0 + 10.0 * i as f32], 0xFFFF_FFFF, false);
        }
        top += height;
    }
}
