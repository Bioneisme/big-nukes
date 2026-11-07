// Big Nukes' silo and bomber in the game: the launch pad, the hatch, the launch
// console, the missile's flight, the strike beacon and the bomber's pass. Added
// to the game as `game::big_nukes_silo`, next to the rest of the glue
// (`game::big_nukes_glue`, glue.rs), whose shared hooks hand these blocks here
// by their ids.

use crate::game::{
    Game, LevelDimension, NativeHeightmap,
    big_nukes as rules,
    big_nukes_glue::{self as glue, lock},
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
    lang,
    mod_api::{EquipmentSlot, NativeScreen, NativeScreenView, NativeServerPlayer},
    text::Text,
};
use horizon_core::geometry::Direction;
use horizon_mods::content::{Block, Item, Rarity};
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// The screens' labels, as the machines' have them.
const LABEL: u32 = 0x40_4040;

// ---- blocks ----------------------------------------------------------------------
//
// The launch pad holds the missile and its warhead in a menu of two slots; the
// console and the beacon have one hidden slot each, as a bomb has: the console's
// for its menu, which needs one, the beacon's for the bomb it was charged with,
// so a broken beacon drops it. The hatch is a plain block.

pub(in crate::game) fn blocks(blocks: &mut Vec<Block>, block: impl Fn(&str, &str, f32) -> Block) {
    blocks.push(block(rules::LAUNCH_PAD, "minecraft:furnace", 5.0));
    blocks.push(block(rules::SILO_HATCH, "minecraft:iron_block", 5.0));
    blocks.push(block(rules::LAUNCH_CONSOLE, "minecraft:furnace", 3.5));
    blocks.push(block(rules::STRIKE_BEACON, "minecraft:furnace", 3.5));
}

/// What the flying displays show: items no one is meant to hold.
pub(in crate::game) fn model_items(items: &mut Vec<Item>) {
    for id in rules::MODEL_ITEMS {
        items.push(Item {
            id: id.into(),
            template: "minecraft:paper".into(),
            max_stack_size: Some(1),
            rarity: Some(Rarity::Common),
            ..Default::default()
        });
    }
}

/// The silo's blocks with a block entity.
pub(in crate::game) fn has_entity(block: &str) -> bool {
    matches!(block, rules::LAUNCH_PAD | rules::LAUNCH_CONSOLE | rules::STRIKE_BEACON)
}

pub(in crate::game) fn container(block: &str) -> ModContainer {
    match block {
        // Hoppers bring the missile from above and the warhead from the sides.
        rules::LAUNCH_PAD => {
            let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| vec![rules::PAD_WARHEAD_SLOT]);
            faces[Direction::Up as usize] = vec![rules::PAD_MISSILE_SLOT];
            ModContainer {
                slots: rules::PAD_SLOTS,
                faces,
                inputs: vec![rules::PAD_MISSILE_SLOT, rules::PAD_WARHEAD_SLOT],
                outputs: vec![],
                menu: Some(rules::LAUNCH_PAD.to_owned()),
            }
        }
        _ => ModContainer {
            slots: 1,
            faces: Default::default(),
            inputs: vec![],
            outputs: vec![],
            menu: (block == rules::LAUNCH_CONSOLE).then(|| rules::CONSOLE_MENU.to_owned()),
        },
    }
}

/// A shift-click from the inventory onto the pad: the missile and the warhead to their slots.
pub(in crate::game) fn pad_quick_move(
    game: &Game<'_>,
    entity: &mut ModBlockEntity,
    container_slots: usize,
    slot: usize,
    stack: &mut ItemStack,
    handled: &mut bool,
) {
    if slot < container_slots {
        return;
    }
    *handled = true;
    if entity.items.len() < rules::PAD_SLOTS {
        entity.items.resize(rules::PAD_SLOTS, None);
    }
    let Some(to) = [rules::PAD_MISSILE_SLOT, rules::PAD_WARHEAD_SLOT].into_iter().find(|s| rules::pad_takes(*s, &stack.item)) else {
        return;
    };
    let most = game.native_max_stack_size(&stack.item).unwrap_or(64);
    match entity.items[to].as_mut() {
        None => {
            entity.items[to] = Some(stack.clone());
            stack.count = 0;
        }
        Some(have) if have.item == stack.item => {
            let moved = most.saturating_sub(have.count).min(stack.count);
            have.count += moved;
            stack.count -= moved;
        }
        Some(_) => {}
    }
}

// ---- the pad's screen ------------------------------------------------------------

/// A furnace's size: the two slots side by side with their names under them.
const PAD_MISSILE_AT: [f32; 2] = [52.0, 30.0];
const PAD_WARHEAD_AT: [f32; 2] = [108.0, 30.0];

pub(in crate::game) fn pad_screen() -> NativeScreen {
    NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some(PAD_MISSILE_AT), Some(PAD_WARHEAD_AT)],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

pub(in crate::game) fn pad_draw(game: &Game<'_>, batch: &mut HudBatch, origin: [f32; 2]) {
    let [x, y] = origin;
    let centered = |batch: &mut HudBatch, text: &str, middle: f32, top: f32| {
        let width = game.native_text_width(text);
        game.native_text(batch, text, [x + middle - width / 2.0, y + top], LABEL, false);
    };
    centered(batch, &lang::get("big_nukes.screen.launch_pad"), 88.0, 6.0);
    centered(batch, &lang::get("big_nukes.screen.launch_pad.missile"), PAD_MISSILE_AT[0] + 8.0, PAD_MISSILE_AT[1] + 22.0);
    centered(batch, &lang::get("big_nukes.screen.launch_pad.warhead"), PAD_WARHEAD_AT[0] + 8.0, PAD_WARHEAD_AT[1] + 22.0);
    game.native_text(batch, &lang::get("container.inventory"), [x + 8.0, y + 72.0], LABEL, false);
}

// ---- the shared hooks --------------------------------------------------------------

/// The end of a level's tick: the launches asked of its consoles, its missiles' flights and its bombers' passes.
/// Only here do the helpers reach the level, the displays and the commands.
pub(in crate::game) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    launches(game, dimension);
    flights(game, dimension);
    strikes(game, dimension);
}

/// The silo's block entities' ticks.
pub(in crate::game) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.id == rules::LAUNCH_CONSOLE {
        console_tick(game, pos, entity);
    }
}

/// A right click on a block: the console tells the window where it is, so a launch can name it, and opens;
/// the pad opens; the strike beacon takes a bomb. Without the assets half, whose menus they open, the player
/// hears why not. Whether the block was one of the silo's.
pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) -> bool {
    let block = game.native_block(pos);
    let menu = match &*block {
        rules::LAUNCH_CONSOLE => rules::CONSOLE_MENU,
        rules::LAUNCH_PAD => rules::LAUNCH_PAD,
        rules::STRIKE_BEACON => {
            beacon_click(game, pos, canceled);
            return true;
        }
        _ => return false,
    };
    let Some(who) = game.native_acting_player() else { return true };
    if horizon_client::containers::mod_menu_type(menu).is_none() {
        *canceled = true;
        game.native_tell(who.uuid, text(&rules::Reply::new("big_nukes.console.no_assets", &[])));
        return true;
    }
    if menu == rules::CONSOLE_MENU {
        news(game, who.uuid, &rules::ConsoleNews { at: Some(pos), ..Default::default() });
    }
    true
}

/// The world closing: what waits for a tick goes.
pub(in crate::game) fn forget() {
    lock(&LAUNCHES).clear();
    lock(&DISPLAYS).clear();
    lock(&SWEEPS).clear();
    *lock(&SESSION) = None;
    lock(&TAKEN).clear();
    *lock(&LAST_CHARGE) = rules::Clicks::new();
}

fn text(line: &rules::Reply) -> Text {
    let args: Vec<Text> = line.args.iter().map(|a| Text::literal(a.clone())).collect();
    Text::translate(&line.key, &args)
}

fn news(game: &mut Game<'_>, player: u128, news: &rules::ConsoleNews) {
    let Ok(bytes) = serde_json::to_vec(news) else { return };
    if let Err(e) = game.native_net_send_to_player(player, rules::CONSOLE_CHANNEL, &bytes) {
        log::warn!("[big_nukes] telling a player of their console: {e}");
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

// ---- the console, on the server ---------------------------------------------------
//
// A launch comes from the player's window as a mod message, heard in the tick after
// it arrives; it is worked at the end of the tick of the player's level, where the
// console, its pad and its shaft are in place. The console's code is the world's
// (`Saved.consoles`), the flight too (`Saved.flights`).

/// Launches asked for: the player and the request.
static LAUNCHES: Mutex<Vec<(u128, rules::LaunchMessage)>> = Mutex::new(Vec::new());
/// The most launches waiting at once, so a client can't fill the queue.
const LAUNCHES_WAITING: usize = 64;
/// How far from a console its player may stand.
const STAND_BY: f64 = rules::CONSOLE_REACH as f64 + 4.0;

pub(in crate::game) fn net_from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != rules::LAUNCH_CHANNEL || !game.native_on(rules::ID) {
        return;
    }
    match rules::read_launch(data) {
        Some(m) => {
            let mut waiting = lock(&LAUNCHES);
            if waiting.len() < LAUNCHES_WAITING {
                waiting.push((player, m));
            }
        }
        None => log::warn!("[big_nukes] an unreadable launch from a client"),
    }
}

/// The launches of the players in `dimension`: each is answered in the chat and on the console's screen.
fn launches(game: &mut Game<'_>, dimension: &str) {
    if lock(&LAUNCHES).is_empty() {
        return;
    }
    let players = game.native_server_players();
    let mine: Vec<(NativeServerPlayer, rules::LaunchMessage)> = {
        let mut waiting = lock(&LAUNCHES);
        // A player who left is gone with their launch.
        waiting.retain(|(uuid, _)| players.iter().any(|p| p.uuid == *uuid));
        let (mine, rest): (Vec<_>, Vec<_>) =
            waiting.drain(..).partition(|(uuid, _)| players.iter().any(|p| p.uuid == *uuid && p.dimension == dimension));
        *waiting = rest;
        mine.into_iter().filter_map(|(uuid, m)| Some((players.iter().find(|p| p.uuid == uuid)?.clone(), m))).collect()
    };
    for (who, m) in mine {
        let line = launch(game, dimension, &who, &m);
        game.native_tell(who.uuid, text(&line));
        news(game, who.uuid, &rules::ConsoleNews { reply: Some(line), ..Default::default() });
    }
}

/// A launch from the console at `m.console`, by `who`: the answer, the launch's or why not. The code is checked
/// first, so a wrong one counts whatever the shaft's state.
fn launch(game: &mut Game<'_>, dimension: &str, who: &NativeServerPlayer, m: &rules::LaunchMessage) -> rules::Reply {
    let console = m.console;
    if distance(who.position, glue::middle(console)) > STAND_BY {
        return rules::Reply::new("big_nukes.console.stand_by", &[]);
    }
    if &*game.native_block(console) != rules::LAUNCH_CONSOLE {
        return rules::Reply::new("big_nukes.console.gone", &[]);
    }
    if !glue::assets_here(game) {
        return rules::Reply::new("big_nukes.console.no_assets", &[]);
    }
    let Some(pad) = find_pad(game, console) else {
        return rules::ShaftProblem::NoPad.line();
    };
    let (to, code) = match rules::launch_request([pad[0], pad[2]], &m.x, &m.z, &m.code) {
        Ok(asked) => asked,
        Err(why) => return why.line(),
    };
    let from = [pad[0], pad[1] + 1, pad[2]];
    {
        let Some(mut guard) = glue::state(game) else {
            return rules::Reply::new("big_nukes.console.gone", &[]);
        };
        let st = guard.as_mut().expect("opened");
        let now = st.saved.tick;
        let entry = st.saved.consoles.entry(rules::console_key(dimension, console)).or_default();
        let was = entry.clone();
        let checked = entry.check(&code, now);
        if *entry != was {
            glue::save(st);
        }
        if let Err(why) = checked {
            return why.line();
        }
        if st.saved.pad_busy(dimension, from, now) {
            return rules::Reply::new("big_nukes.console.busy", &[]);
        }
    }
    let hatch = match read_shaft(game, dimension, pad) {
        Ok(h) => pad[1] + h,
        Err(why) => return why.line(),
    };
    let (missile, warhead) = pad_items(game, pad);
    let kind = match rules::pad_load(missile.as_deref(), warhead.as_deref()) {
        Ok(kind) => kind,
        Err(why) => return why.line(),
    };
    let flight = {
        let Some(mut guard) = glue::state(game) else {
            return rules::Reply::new("big_nukes.console.gone", &[]);
        };
        let st = guard.as_mut().expect("opened");
        match st.saved.launch(dimension, from, to, kind, hatch) {
            Ok(f) => {
                st.saved.consoles.entry(rules::console_key(dimension, console)).or_default().keep(&code);
                glue::save(st);
                f
            }
            Err(why) => return why.line(),
        }
    };
    // The missile and its warhead leave the pad.
    game.native_with_mod_block_entity(pad, |e| {
        for slot in [rules::PAD_MISSILE_SLOT, rules::PAD_WARHEAD_SLOT] {
            if let Some(Some(stack)) = e.items.get_mut(slot) {
                stack.count -= 1;
                if stack.count == 0 {
                    e.items[slot] = None;
                }
            }
        }
    });
    log::info!("[big_nukes] missile {}: {:?} from {pad:?} to {to:?} in {dimension}, by {}", flight.id, kind, who.name);
    if m.camera {
        news(game, who.uuid, &rules::ConsoleNews { now: flight.start, missile: Some(flight.clone()), ..Default::default() });
    }
    let seconds = rules::LAUNCH_COUNTDOWN / rules::SECOND;
    rules::Reply::new("big_nukes.console.launched", &[&to[0], &to[1], &seconds])
}

/// The launch pad nearest the console, within `CONSOLE_REACH`.
fn find_pad(game: &Game<'_>, console: [i32; 3]) -> Option<[i32; 3]> {
    let r = rules::CONSOLE_REACH;
    let mut best: Option<(i32, [i32; 3])> = None;
    for dx in -r..=r {
        for dy in -r..=r {
            for dz in -r..=r {
                let d = dx * dx + dy * dy + dz * dz;
                if d > r * r || best.is_some_and(|(b, _)| b <= d) {
                    continue;
                }
                let at = [console[0] + dx, console[1] + dy, console[2] + dz];
                if &*game.native_block(at) == rules::LAUNCH_PAD {
                    best = Some((d, at));
                }
            }
        }
    }
    best.map(|(_, at)| at)
}

/// The shaft over the pad, up to the sky or the Nether's roof: the hatch's height over the pad.
fn read_shaft(game: &Game<'_>, dimension: &str, pad: [i32; 3]) -> Result<i32, rules::ShaftProblem> {
    let top = rules::world_of(dimension).map_or(320, |w| w.roof.min(w.top()));
    let column: Vec<String> = (pad[1] + 1..top).map(|y| game.native_block([pad[0], y, pad[2]]).to_string()).collect();
    let names: Vec<&str> = column.iter().map(String::as_str).collect();
    rules::shaft(&names)
}

/// What the pad holds in its two slots.
fn pad_items(game: &mut Game<'_>, pad: [i32; 3]) -> (Option<String>, Option<String>) {
    let item = |e: &ModBlockEntity, slot: usize| e.items.get(slot).and_then(Option::as_ref).map(|s| s.item.to_string());
    game.native_with_mod_block_entity(pad, |e| (item(e, rules::PAD_MISSILE_SLOT), item(e, rules::PAD_WARHEAD_SLOT)))
        .unwrap_or((None, None))
}

/// Once a second, the console's screen learns the state of its shaft, its countdown or its lock.
fn console_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some((now, _)) = glue::clock(game) else { return };
    if entity.data.len() == rules::CONSOLE_DATA && now % rules::SECOND != 0 {
        return;
    }
    let data = console_state(game, pos, now).data().to_vec();
    if entity.data != data {
        entity.data = data;
    }
}

fn console_state(game: &mut Game<'_>, pos: [i32; 3], now: u64) -> rules::ConsoleState {
    let dimension = game.native_dimension().to_owned();
    let pad = find_pad(game, pos);
    let (locked, flight) = {
        let Some(guard) = glue::state(game) else { return rules::ConsoleState::Shaft(rules::ShaftProblem::NoPad) };
        let st = guard.as_ref().expect("opened");
        let locked = st.saved.consoles.get(&rules::console_key(&dimension, pos)).map_or(0, |c| c.locked_until.min(now + rules::LOCK_TICKS));
        let flight = pad.and_then(|p| st.saved.launching(&dimension, [p[0], p[1] + 1, p[2]], now).cloned());
        (locked, flight)
    };
    if now < locked {
        return rules::ConsoleState::locked(locked - now);
    }
    let Some(pad) = pad else {
        return rules::ConsoleState::Shaft(rules::ShaftProblem::NoPad);
    };
    if let Some(f) = flight {
        return if now < f.liftoff() { rules::ConsoleState::counting(f.liftoff() - now) } else { rules::ConsoleState::Flying };
    }
    if let Err(why) = read_shaft(game, &dimension, pad) {
        return rules::ConsoleState::Shaft(why);
    }
    let (missile, warhead) = pad_items(game, pad);
    match rules::pad_load(missile.as_deref(), warhead.as_deref()) {
        Ok(_) => rules::ConsoleState::Ready,
        Err(why) => rules::ConsoleState::Shaft(why),
    }
}

// ---- the console, in the window ---------------------------------------------------
//
// The fields live in the window: digits come from the keyboard (the screen's key and
// character hooks, API 104) and from the screen's keys. A launch is queued and sent
// to the server from the next frame (`native_net_send_to_server` needs the game
// mutably, which the screen's hooks don't have).

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

fn with_ui<R>(f: impl FnOnce(&mut ConsoleUi) -> R) -> R {
    f(lock(&CONSOLE_UI).get_or_insert_with(ConsoleUi::default))
}

/// The console's news in the window: where it is, the answer to a launch.
pub(in crate::game) fn net_from_server(game: &mut Game<'_>, channel: &str, data: &[u8]) {
    if channel != rules::CONSOLE_CHANNEL || !game.native_on(rules::ID) {
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
        follow(game, flight, news.now);
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

fn ours(screen: &NativeScreenView<'_>) -> bool {
    screen.menu == Some(rules::CONSOLE_MENU)
}

/// A key on the console's screen: every key but Escape and the function keys is the console's, so none
/// closes the screen or moves an item.
pub(in crate::game) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    if !ours(screen) || !rules::console_takes(key) || !game.native_on(rules::ID) {
        return;
    }
    *canceled = true;
    with_ui(|ui| {
        if ui.input.key(key) == rules::Keyed::Launch {
            queue_launch(ui);
        }
    });
}

pub(in crate::game) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char, canceled: &mut bool) {
    if !ours(screen) || !game.native_on(rules::ID) {
        return;
    }
    *canceled = true;
    with_ui(|ui| ui.input.typed(c));
}

fn queue_launch(ui: &mut ConsoleUi) {
    match ui.at {
        Some(at) => ui.outbox.push(ui.input.request(at, CAMERA_ON.load(Ordering::Relaxed)).bytes()),
        None => ui.reply = Some((Instant::now(), rules::Reply::new("big_nukes.console.gone", &[]))),
    }
}

/// Every frame: the queued launches go to the server.
pub(in crate::game) fn frame(game: &mut Game<'_>) {
    let outbox = with_ui(|ui| std::mem::take(&mut ui.outbox));
    for bytes in outbox {
        if let Err(e) = game.native_net_send_to_server(rules::LAUNCH_CHANNEL, &bytes) {
            with_ui(|ui| ui.reply = Some((Instant::now(), rules::Reply::new("big_nukes.console.send_failed", &[&e]))));
        }
    }
}

// The screen, a keypad's size: the three fields, a line of the shaft's state or the
// last answer, twelve keys, and Erase, Launch and Camera on the right; then the
// inventory, as the game lays it.

const CONSOLE_HEIGHT: f32 = 222.0;
const CONSOLE_INVENTORY_Y: f32 = 140.0;
/// The fields: x, y, width, height.
const FIELDS: [[f32; 4]; 3] = [[8.0, 28.0, 52.0, 14.0], [64.0, 28.0, 52.0, 14.0], [120.0, 28.0, 48.0, 14.0]];
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

pub(in crate::game) fn console_screen() -> NativeScreen {
    NativeScreen {
        width: 176.0,
        height: CONSOLE_HEIGHT,
        // The menu's one slot is hidden.
        slots: vec![None],
        inventory_y: CONSOLE_INVENTORY_Y,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

/// A press on the console's screen, over no slot: a field takes the focus, a key types, a button acts. Nothing
/// goes to the server as a menu button.
pub(in crate::game) fn console_click(at: [f32; 2], right: bool) {
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
fn wrap(game: &Game<'_>, line: &str, width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in line.split(' ') {
        match lines.last_mut() {
            Some(last) if game.native_text_width(&format!("{last} {word}")) <= width => {
                last.push(' ');
                last.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

fn button(game: &Game<'_>, batch: &mut HudBatch, origin: [f32; 2], [bx, by, bw, bh]: [f32; 4], label: &str, face: u32, lit: bool) {
    let [x, y] = origin;
    game.native_fill(batch, [x + bx, y + by, bw, bh], 0xFF2A_2A2A);
    let face = if lit { face.saturating_add(0x0018_1818) } else { face };
    game.native_fill(batch, [x + bx + 1.0, y + by + 1.0, bw - 2.0, bh - 2.0], face);
    let width = game.native_text_width(label);
    game.native_text(batch, label, [x + bx + (bw - width) / 2.0, y + by + (bh - 8.0) / 2.0], 0xFF_FFFF, true);
}

pub(in crate::game) fn console_draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let [x, y] = origin;
    let (input, reply) = with_ui(|ui| (ui.input.clone(), ui.reply.clone().filter(|(at, _)| at.elapsed() < REPLY_FOR)));
    let centered = |batch: &mut HudBatch, text: &str, top: f32, rgb: u32| {
        let width = game.native_text_width(text);
        game.native_text(batch, text, [x + (176.0 - width) / 2.0, y + top], rgb, false);
    };
    centered(batch, &lang::get("big_nukes.screen.console"), 6.0, LABEL);
    // The fields, the code as stars, a caret in the focused one.
    let caret = Instant::now().duration_since(*START.get_or_init(Instant::now)).as_millis() / 500 % 2 == 0;
    for (i, [fx, fy, fw, fh]) in FIELDS.into_iter().enumerate() {
        let label = match i {
            rules::FIELD_X => "X".to_owned(),
            rules::FIELD_Z => "Z".to_owned(),
            _ => lang::get("big_nukes.screen.console.code"),
        };
        game.native_text(batch, &label, [x + fx, y + fy - 10.0], LABEL, false);
        let edge = if input.focus == i { 0xFFFF_FFFF } else { 0xFF37_3737 };
        game.native_fill(batch, [x + fx, y + fy, fw, fh], edge);
        game.native_fill(batch, [x + fx + 1.0, y + fy + 1.0, fw - 2.0, fh - 2.0], 0xFF10_1810);
        let shown = if i == rules::FIELD_CODE { "*".repeat(input.fields[i].len()) } else { input.fields[i].clone() };
        let shown = if input.focus == i && caret { format!("{shown}_") } else { shown };
        game.native_text(batch, &shown, [x + fx + 3.0, y + fy + 3.0], 0x55_FF55, false);
    }
    // The answer for a while, else the shaft's state.
    let state = rules::ConsoleState::of_data(data);
    let (line, rgb) = match (&reply, state) {
        (Some((_, r)), _) => (r.clone(), if r.key == "big_nukes.console.launched" { 0x00_8800 } else { 0xAA_0000 }),
        (None, Some(s)) => (s.line(), if s == rules::ConsoleState::Ready { 0x00_8800 } else { 0xAA_0000 }),
        (None, None) => (rules::Reply::default(), LABEL),
    };
    if !line.key.is_empty() {
        let args: Vec<&dyn std::fmt::Display> = line.args.iter().map(|a| a as &dyn std::fmt::Display).collect();
        for (k, l) in wrap(game, &lang::tr(&line.key, &args), 168.0).iter().take(2).enumerate() {
            centered(batch, l, STATUS_Y + k as f32 * 10.0, rgb);
        }
    }
    let cursor = game.native_cursor();
    let at = [cursor[0] - x, cursor[1] - y];
    for (c, r) in console_keys() {
        button(game, batch, origin, r, &c.to_string(), 0xFF8C_8C8C, inside(at, r));
    }
    button(game, batch, origin, ERASE_AT, &lang::get("big_nukes.screen.console.erase"), 0xFF6E_6E6E, inside(at, ERASE_AT));
    button(game, batch, origin, LAUNCH_AT, &lang::get("big_nukes.screen.console.launch"), 0xFFA8_2020, inside(at, LAUNCH_AT));
    let camera = if CAMERA_ON.load(Ordering::Relaxed) {
        lang::get("big_nukes.screen.console.camera_on")
    } else {
        lang::get("big_nukes.screen.console.camera_off")
    };
    button(game, batch, origin, CAMERA_AT, &camera, 0xFF4A_5A6E, inside(at, CAMERA_AT));
    game.native_text(batch, &lang::get("container.inventory"), [x + 8.0, y + CONSOLE_INVENTORY_Y - 11.0], LABEL, false);
}

/// When the window started drawing, for the caret's blink.
static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

// ---- the missile's flight -----------------------------------------------------------
//
// The rules say where a missile is at each tick of the world's clock; here, at the
// end of its level's tick, its display is made or moved, its trail drawn and its
// sounds played, the hatch opened and shut, its target's chunk forced and read, and
// its blast set off. The displays live in memory only: one missing (a chunk that
// went, a world opened again) is made anew when the flight next shows.

/// Each flight's displays this session: (flight, part) to the entity.
static DISPLAYS: Mutex<BTreeMap<(u64, u8), u32>> = Mutex::new(BTreeMap::new());
/// Each dimension's next sweep for displays no flight owns.
static SWEEPS: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());
/// This session's mark on its displays: a world opened again finds the last session's and takes them away.
static SESSION: Mutex<Option<u64>> = Mutex::new(None);
const MISSILE_PART: u8 = 0;
/// Ticks between sweeps.
const SWEEP_EVERY: u64 = 200;
/// The ticks a display glides between two places: moved every tick, it never stops.
const GLIDE: i32 = 2;
/// Vanilla's `/playsound` reaches 16 blocks for each of volume: the launch's roar about 300.
const ROAR_VOLUME: f32 = 19.0;

/// A command in `dimension`, from a level's tick, where commands run. Its answer isn't an error: false is also
/// smoke no player is near enough to see.
fn run_in(game: &mut Game<'_>, dimension: &str, command: &str) {
    game.native_run_command(&format!("execute in {dimension} run {command}"));
}

fn particle(id: &str, at: [f64; 3], spread: f64, speed: f64, count: u32) -> String {
    format!("particle {id} {:.2} {:.2} {:.2} {spread:.2} {spread:.2} {spread:.2} {speed:.2} {count} force", at[0], at[1], at[2])
}

fn play(game: &mut Game<'_>, dimension: &str, sound: &str, at: [f64; 3], volume: f32, pitch: f32) {
    run_in(game, dimension, &format!("playsound {sound} block @a {:.1} {:.1} {:.1} {volume:.1} {pitch:.2}", at[0], at[1], at[2]));
}

fn session_tag() -> String {
    let session = *lock(&SESSION).get_or_insert_with(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_millis() as u64 % 1_000_000_000)
    });
    format!("{}_s{session}", rules::FLIGHT_TAG)
}

fn flight_tag(id: u64) -> String {
    format!("{}_{id}", rules::FLIGHT_TAG)
}

/// An item display of `item`, tagged as the flight's, lit as by day, seen from afar and gliding between its
/// places. Its model lies along z: `scale` makes it its size and `lift` moves it forward along its look.
fn display_nbt(item: &str, scale: f32, lift: f32, yaw: f32, pitch: f32, flight: u64) -> Compound {
    let float = |v: f32| Tag::Float(v.to_bits());
    let floats = |v: [f32; 4]| Tag::List(v.into_iter().map(float).collect());
    let three = |v: [f32; 3]| Tag::List(v.into_iter().map(float).collect());
    let tags = [rules::FLIGHT_TAG.to_owned(), flight_tag(flight), session_tag()];
    [
        ("item", Tag::compound([("id", Tag::String(JavaString::from(item))), ("count", Tag::Int(1))])),
        ("item_display", Tag::String(JavaString::from("none"))),
        ("view_range", float(12.0)),
        ("teleport_duration", Tag::Int(GLIDE)),
        ("brightness", Tag::compound([("sky", Tag::Int(15)), ("block", Tag::Int(15))])),
        ("Tags", Tag::List(tags.iter().map(|t| Tag::String(JavaString::from(t.as_str()))).collect())),
        ("Rotation", Tag::List(vec![float(yaw), float(pitch)])),
        (
            "transformation",
            Tag::compound([
                ("left_rotation", floats([0.0, 0.0, 0.0, 1.0])),
                ("right_rotation", floats([0.0, 0.0, 0.0, 1.0])),
                ("translation", three([0.0, 0.0, lift])),
                ("scale", three([scale; 3])),
            ]),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (JavaString::from(k), v))
    .collect()
}

/// A flight's display at `pos`, made when it is missing.
fn show(game: &mut Game<'_>, dimension: &str, key: (u64, u8), item: &str, scale: f32, lift: f32, pos: [f64; 3], yaw: f32, pitch: f32) {
    let known = lock(&DISPLAYS).get(&key).copied();
    if let Some(id) = known {
        if game.native_move_entity(dimension, id, pos, yaw, pitch) {
            return;
        }
        lock(&DISPLAYS).remove(&key);
    }
    // None in a chunk that isn't loaded: it is tried again the next tick.
    if let Some(id) = game.native_summon(dimension, "minecraft:item_display", pos, display_nbt(item, scale, lift, yaw, pitch, key.0)) {
        game.native_move_entity(dimension, id, pos, yaw, pitch);
        lock(&DISPLAYS).insert(key, id);
    }
}

fn hide(game: &mut Game<'_>, dimension: &str, key: (u64, u8)) {
    let id = lock(&DISPLAYS).remove(&key);
    if let Some(id) = id {
        game.native_remove_entity(dimension, id);
    }
}

/// The displays no flight owns: on a dimension's first tick this session every tagged one (a closed world's),
/// then now and again those of another session or of a flight that is over.
fn sweep(game: &mut Game<'_>, dimension: &str, now: u64, live: &[u64]) {
    let first = {
        let mut sweeps = lock(&SWEEPS);
        match sweeps.get(dimension) {
            Some(next) if now < *next => return,
            seen => {
                let first = seen.is_none();
                sweeps.insert(dimension.to_owned(), now + SWEEP_EVERY);
                first
            }
        }
    };
    let tag = rules::FLIGHT_TAG;
    if first {
        run_in(game, dimension, &format!("kill @e[type=minecraft:item_display,tag={tag}]"));
        return;
    }
    run_in(game, dimension, &format!("kill @e[type=minecraft:item_display,tag={tag},tag=!{}]", session_tag()));
    let keep: String = live.iter().map(|id| format!(",tag=!{}", flight_tag(*id))).collect();
    run_in(game, dimension, &format!("kill @e[type=minecraft:item_display,tag={tag}{keep}]"));
}

/// What a tick of the flights changes in the world's record, made with the state held once.
enum Change {
    Surface(u64, i32),
    HatchShut(u64),
    Force(u64, [i32; 2]),
    Emptied(u64),
    Impact(u64, [i32; 3], rules::BombKind),
}

fn flights(game: &mut Game<'_>, dimension: &str) {
    let (now, mine, live) = {
        let Some(guard) = glue::state(game) else { return };
        let st = guard.as_ref().expect("opened");
        let mine: Vec<rules::Flight> = st.saved.flights.iter().filter(|f| f.dimension == dimension).cloned().collect();
        let live: Vec<u64> = st.saved.flights.iter().chain(&st.saved.beacons).map(|f| f.id).collect();
        (st.saved.tick, mine, live)
    };
    sweep(game, dimension, now, &live);
    let mut changes = Vec::new();
    for f in &mine {
        missile(game, dimension, f, now, &mut changes);
    }
    settle(game, dimension, changes);
}

/// The world's record takes a tick's changes; the blasts go off, and the chunks no flight keeps are let go.
fn settle(game: &mut Game<'_>, dimension: &str, changes: Vec<Change>) {
    let mut fired = Vec::new();
    for c in &changes {
        if let Change::Impact(id, at, kind) = *c {
            // The blast's own record and news, with the state let go.
            let center = match glue::start_blast(game, dimension, at, kind.radius()) {
                Ok(blast) => {
                    log::info!("[big_nukes] flight {id} went off at {at:?} in {dimension}: blast {blast}");
                    Some(at)
                }
                Err(why) => {
                    log::info!("[big_nukes] flight {id} at {at:?} in {dimension} didn't go off: {why:?}");
                    glue::tell_near(game, dimension, at, &glue::refused(why));
                    None
                }
            };
            fired.push((id, center));
        }
    }
    let (force, free) = {
        let Some(mut guard) = glue::state(game) else { return };
        let st = guard.as_mut().expect("opened");
        let mut force = Vec::new();
        // Saved only when something changed: most ticks only ask again for a chunk already forced.
        let forced = st.saved.forced.len();
        let mut changed = !fired.is_empty();
        for c in changes {
            match c {
                Change::Surface(id, y) => {
                    if let Some(f) = st.saved.flights.iter_mut().chain(&mut st.saved.beacons).find(|f| f.id == id) {
                        changed |= f.surface != Some(y);
                        f.surface = Some(y);
                    }
                }
                Change::HatchShut(id) => {
                    if let Some(f) = st.saved.flights.iter_mut().find(|f| f.id == id) {
                        changed |= f.hatch.is_some();
                        f.hatch = None;
                    }
                }
                Change::Force(id, chunk) => {
                    if st.saved.force(id, dimension, chunk) {
                        force.push(chunk);
                    }
                }
                Change::Emptied(id) => changed |= st.saved.emptied(id),
                Change::Impact(..) => {}
            }
        }
        for (id, center) in &fired {
            st.saved.flights.retain(|f| f.id != *id);
            st.saved.beacons.retain(|f| f.id != *id);
            st.saved.landed(*id, *center);
        }
        let free = st.saved.unforce();
        if changed || st.saved.forced.len() != forced {
            glue::save(st);
        }
        (force, free)
    };
    for chunk in force {
        run_in(game, dimension, &format!("forceload add {} {}", chunk[0] * 16, chunk[1] * 16));
    }
    for (d, chunk) in free {
        run_in(game, &d, &format!("forceload remove {} {}", chunk[0] * 16, chunk[1] * 16));
    }
    for (id, _) in fired {
        for part in 0..3 {
            hide(game, dimension, (id, part));
        }
        run_in(game, dimension, &format!("kill @e[type=minecraft:item_display,tag={}]", flight_tag(id)));
    }
}

/// The ground's height at a column of a loaded chunk of the ticking level: one over its highest block.
fn ground_at(game: &Game<'_>, dimension: &str, x: i32, z: i32) -> Option<i32> {
    game.native_surface_heights(dimension, &[(x, z)], NativeHeightmap::WorldSurface).ok()?.first().copied().flatten()
}

/// A missile's tick, by its leg.
fn missile(game: &mut Game<'_>, dimension: &str, f: &rules::Flight, now: u64, changes: &mut Vec<Change>) {
    let pad = [f64::from(f.from[0]) + 0.5, f64::from(f.from[1]), f64::from(f.from[2]) + 0.5];
    hatch(game, dimension, f, now, changes);
    let key = (f.id, MISSILE_PART);
    let leg = f.leg(now);
    if now >= f.liftoff() {
        // The target is forced from the liftoff, so its chunk is in by the time it is wanted.
        changes.push(Change::Force(f.id, f.chunk_to()));
    }
    match leg {
        rules::Leg::Countdown { left } => {
            if left % rules::SECOND == 0 {
                let pitch = 1.0 + (rules::LAUNCH_COUNTDOWN.saturating_sub(left) as f32) / 200.0;
                play(game, dimension, "minecraft:block.note_block.pling", pad, 3.0, pitch);
            }
            if left <= rules::SECOND && left % 2 == 0 {
                run_in(game, dimension, &particle("minecraft:large_smoke", [pad[0], pad[1] + 0.5, pad[2]], 0.6, 0.05, 8));
            }
        }
        rules::Leg::Ascent { pos, yaw, pitch } => {
            show(game, dimension, key, rules::MISSILE_MODEL, rules::MISSILE_SCALE, rules::MISSILE_LIFT, pos, yaw, pitch);
            let t = now - f.liftoff();
            if t == 0 {
                play(game, dimension, rules::MISSILE_LAUNCH, pad, ROAR_VOLUME, 1.0);
            }
            if t < rules::SECOND {
                // Smoke boils out of the shaft as it goes.
                run_in(game, dimension, &particle("minecraft:campfire_signal_smoke", [pad[0], pad[1] + 1.0, pad[2]], 0.8, 0.08, 6));
            }
            if t % 2 == 0 {
                trail(game, dimension, pos, true);
            }
        }
        rules::Leg::Cruise => {
            hide(game, dimension, key);
            if f.surface.is_none() {
                if let Some(y) = ground_at(game, dimension, f.to[0], f.to[1]) {
                    changes.push(Change::Surface(f.id, y));
                } else if f.surface_overdue(now) {
                    log::info!("[big_nukes] flight {}: the target's ground never loaded; it lands at {}", f.id, rules::SURFACE_GUESS);
                    changes.push(Change::Surface(f.id, rules::SURFACE_GUESS));
                }
            }
        }
        rules::Leg::Descent { pos, yaw, pitch } => {
            show(game, dimension, key, rules::MISSILE_MODEL, rules::MISSILE_SCALE, rules::MISSILE_LIFT, pos, yaw, pitch);
            if now % 2 == 0 {
                trail(game, dimension, pos, false);
            }
            if now + 40 == f.lands_at() {
                // Its fall's whistle, from where it will land.
                let target = [f64::from(f.to[0]) + 0.5, pos[1].min(f64::from(f.surface.unwrap_or(64)) + 20.0), f64::from(f.to[1]) + 0.5];
                play(game, dimension, "minecraft:entity.phantom.swoop", target, 12.0, 0.5);
            }
        }
        rules::Leg::Impact { at } => {
            hide(game, dimension, key);
            changes.push(Change::Impact(f.id, at, f.kind));
        }
    }
}

/// The smoke and fire a missile leaves behind its tail.
fn trail(game: &mut Game<'_>, dimension: &str, tail: [f64; 3], climbing: bool) {
    let (smoke, fire) = if climbing { (5, 6) } else { (3, 3) };
    run_in(game, dimension, &particle("minecraft:campfire_signal_smoke", tail, 0.35, 0.02, smoke));
    run_in(game, dimension, &particle("minecraft:flame", tail, 0.2, 0.04, fire));
}

/// The hatch: taken away while it should stand open, put back after; once back, the flight forgets it.
fn hatch(game: &mut Game<'_>, dimension: &str, f: &rules::Flight, now: u64, changes: &mut Vec<Change>) {
    let Some(y) = f.hatch else { return };
    let at = [f.from[0], y, f.from[2]];
    let registry = &game.pack.assets.registry;
    let (Some(air), Some(shut)) = (registry.default_state("minecraft:air"), registry.default_state(rules::SILO_HATCH)) else {
        return;
    };
    let here = game.native_block(at);
    if f.hatch_open(now) {
        if here == rules::SILO_HATCH {
            game.native_set_block(dimension, at, air, 3);
            play(game, dimension, "minecraft:block.iron_trapdoor.open", glue::middle(at), 2.0, 0.6);
        }
    } else if now > f.liftoff() {
        if here == "minecraft:air" {
            game.native_set_block(dimension, at, shut, 3);
            play(game, dimension, "minecraft:block.iron_trapdoor.close", glue::middle(at), 2.0, 0.6);
        }
        changes.push(Change::HatchShut(f.id));
    }
}

// ---- the launch camera, in the window -------------------------------------------------
//
// The server tells the window of the missile it launched with the camera on; the
// window works the camera out from its own clock each frame (`native_camera_pose`,
// API 402), and lets go once the missile is high or the player turns it off.

/// The missile followed: the flight, the window's game time less the world's tick when it was told, and the
/// player's eyes then, whose side the camera keeps.
static CAMERA: Mutex<Option<(rules::Flight, f64, [f64; 3])>> = Mutex::new(None);

fn follow(game: &Game<'_>, flight: rules::Flight, now: u64) {
    let (Some(clock), Some(player)) = (game.native_clock(), game.native_player()) else { return };
    let offset = clock.game_time as f64 - now as f64;
    *lock(&CAMERA) = Some((flight, offset, [player.x, player.y + 1.62, player.z]));
}

pub(in crate::game) fn camera_pose(game: &Game<'_>, partial: f32, position: &mut [f64; 3], yaw: &mut f32, pitch: &mut f32) {
    let Some((flight, offset, eye)) = lock(&CAMERA).clone() else { return };
    if !CAMERA_ON.load(Ordering::Relaxed) || !game.native_on(rules::ID) {
        return;
    }
    let (Some(clock), Some(player)) = (game.native_clock(), game.native_player()) else { return };
    if flight.dimension != player.dimension {
        return;
    }
    // The display glides its last ticks behind its place: the camera looks where it is seen.
    let now = clock.game_time as f64 + f64::from(partial) - offset - f64::from(GLIDE);
    match rules::launch_camera(&flight, now, eye) {
        Some((at, y, p)) => {
            *position = at;
            *yaw = y;
            *pitch = p;
        }
        None if now > flight.liftoff() as f64 => *lock(&CAMERA) = None,
        None => {}
    }
}

/// The window's level goes: so does the camera.
pub(in crate::game) fn forget_camera() {
    *lock(&CAMERA) = None;
}

// ---- the strike beacon --------------------------------------------------------------
//
// A right click with a tactical warhead or an atomic bomb charges a beacon: the bomb
// goes from the hand into the beacon's hidden slot, so a beacon broken before the
// drop gives it back, and the world records the strike. Thirty seconds of red smoke
// later the bomber flies over at the end of the beacon's level's tick, as a display,
// and the bomb leaves it as another; where the bomb meets the ground it goes off.

const PLANE_PART: u8 = 1;
const BOMB_PART: u8 = 2;
/// The drone of the bomber is heard about 190 blocks off.
const DRONE_VOLUME: f32 = 12.0;
/// How far a beacon's news is told.
const BEACON_TOLD: f64 = 64.0;

/// The clicks heard on the beacons: the player, the beacon and the item in hand. The game asks once per hand,
/// and repeats a click held down: it is answered once.
static LAST_CHARGE: Mutex<rules::Clicks<(u128, [i32; 3], Option<String>)>> = Mutex::new(rules::Clicks::new());
/// Beacons and consoles taken away, heard from the level, which may change a block while the state is locked.
static TAKEN: Mutex<Vec<(String, [i32; 3])>> = Mutex::new(Vec::new());

/// The game's name of a level's dimension.
pub(in crate::game) fn dimension_id(dimension: LevelDimension) -> &'static str {
    match dimension {
        LevelDimension::Overworld => "minecraft:overworld",
        LevelDimension::Nether => "minecraft:the_nether",
        LevelDimension::End => "minecraft:the_end",
    }
}

/// A beacon or a console taken away (broken, blown up, replaced): its strike or its code goes the next time the
/// state opens at the end of its level's tick.
pub(in crate::game) fn taken(dimension: &str, pos: [i32; 3]) {
    lock(&TAKEN).push((dimension.to_owned(), pos));
}

/// The beacons and consoles of `dimension` taken away since its last tick: a beacon's strike is called off before
/// its drop (its bomb dropped with the beacon), a console's code is forgotten, so a new one starts afresh.
fn take_away(game: &mut Game<'_>, dimension: &str) {
    let gone: Vec<[i32; 3]> = {
        let mut taken = lock(&TAKEN);
        let (mine, rest): (Vec<_>, Vec<_>) = taken.drain(..).partition(|(d, _)| d == dimension);
        *taken = rest;
        mine.into_iter().map(|(_, pos)| pos).collect()
    };
    if gone.is_empty() {
        return;
    }
    let Some(mut guard) = glue::state(game) else { return };
    let st = guard.as_mut().expect("opened");
    for pos in gone {
        if let Some(kind) = st.saved.uncharge(dimension, pos) {
            log::info!("[big_nukes] the beacon at {pos:?} in {dimension} is gone: its {kind:?} isn't dropped");
        }
        st.saved.consoles.remove(&rules::console_key(dimension, pos));
    }
    glue::save(st);
}

/// A right click on a strike beacon: a bomb that flies charges it, anything else hears what it takes. The furnace
/// it is built on never opens. Sneaking with something else in hand lets it be used (a block placed against it).
fn beacon_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let held = game.native_equipment(EquipmentSlot::MainHand).map(|s| s.item.to_string());
    let flies = held.as_deref().and_then(rules::BombKind::of_block).is_some_and(rules::BombKind::flies);
    if held.is_some() && !flies && game.native_sneaking() {
        return;
    }
    *canceled = true;
    let (Some(who), Some((now, _))) = (game.native_acting_player(), glue::clock(game)) else { return };
    if lock(&LAST_CHARGE).heard((who.uuid, pos, held.clone()), now).is_some() {
        return;
    }
    let tell = |game: &mut Game<'_>, line: rules::Reply| {
        game.native_tell(who.uuid, text(&line));
    };
    if !glue::assets_here(game) {
        tell(game, rules::Reply::new("big_nukes.beacon.no_assets", &[]));
        return;
    }
    let dimension = who.dimension.clone();
    let charged = {
        let Some(mut guard) = glue::state(game) else { return };
        let st = guard.as_mut().expect("opened");
        let charge = st.saved.charge(&dimension, pos, held.as_deref());
        let left = st.saved.beacon_at(&dimension, pos).map(|f| f.pass().arrives().saturating_sub(now));
        (charge, left)
    };
    let seconds = |ticks: Option<u64>| ticks.unwrap_or(0).div_ceil(rules::SECOND);
    match charged {
        (rules::Charge::Taken(kind), left) => {
            // Out of the hand, unless creative, into the beacon's slot.
            let taken = who.game_mode == 1 || game.native_take_items(kind.id(), 1) == 1;
            let put = taken
                && game
                    .native_with_mod_block_entity(pos, |e| {
                        if e.items.is_empty() {
                            e.items.resize(1, None);
                        }
                        e.items[0] = Some(ItemStack { item: kind.id().into(), count: 1, damage: 0, extra: None });
                    })
                    .is_some();
            if !put {
                if taken && who.game_mode != 1 {
                    game.native_give_items(kind.id(), 1);
                }
                if let Some(mut guard) = glue::state(game) {
                    let st = guard.as_mut().expect("opened");
                    st.saved.beacons.retain(|f| f.dimension != dimension || f.from != pos);
                }
                return;
            }
            if let Some(mut guard) = glue::state(game) {
                glue::save(guard.as_mut().expect("opened"));
            }
            log::info!("[big_nukes] a bomber is called to {pos:?} in {dimension} with a {kind:?}, by {}", who.name);
            game.native_level_play_sound("minecraft:block.beacon.activate", glue::middle(pos), 4, 1.0, 1.4);
            tell(game, rules::Reply::new("big_nukes.beacon.charged", &[&seconds(left)]));
        }
        (rules::Charge::Busy, left) => tell(game, rules::Reply::new("big_nukes.beacon.busy", &[&seconds(left)])),
        (rules::Charge::Not, _) => tell(game, rules::Reply::new("big_nukes.beacon.hint", &[])),
        (rules::Charge::Maxpower { radius, maxpower }, _) => {
            game.native_tell(who.uuid, glue::refused(rules::Refused::Maxpower { radius, maxpower }));
        }
    }
}

/// The strikes of `dimension`: the beacon's smoke and news, the bomber's pass, the bomb's fall and its blast.
fn strikes(game: &mut Game<'_>, dimension: &str) {
    take_away(game, dimension);
    let (now, mine) = {
        let Some(guard) = glue::state(game) else { return };
        let st = guard.as_ref().expect("opened");
        (st.saved.tick, st.saved.beacons.iter().filter(|f| f.dimension == dimension).cloned().collect::<Vec<_>>())
    };
    if mine.is_empty() {
        return;
    }
    let mut changes = Vec::new();
    for f in &mine {
        strike(game, dimension, f, now, &mut changes);
    }
    settle(game, dimension, changes);
}

fn strike(game: &mut Game<'_>, dimension: &str, f: &rules::Flight, now: u64, changes: &mut Vec<Change>) {
    let beacon = glue::middle(f.from);
    let pass = f.pass();
    // The beacon's chunk stays loaded until the blast has worked it.
    changes.push(Change::Force(f.id, rules::chunk_of(f.from[0], f.from[2])));
    let comes = pass.arrives();
    if now < comes {
        if now % 10 == 0 {
            let smoke = [beacon[0], beacon[1] + 1.5, beacon[2]];
            run_in(game, dimension, &particle("minecraft:dust{color:[1.0,0.0,0.0],scale:2.0}", smoke, 0.3, 0.02, 12));
        }
        if comes - now == 10 * rules::SECOND {
            let line = text(&rules::Reply::new("big_nukes.beacon.soon", &[&10]));
            tell_around(game, dimension, beacon, &line);
        }
        return;
    }
    if now == comes {
        play(game, dimension, rules::BOMBER_FLYBY, [beacon[0], beacon[1] + 30.0, beacon[2]], DRONE_VOLUME, 1.0);
    }
    match pass.plane(now) {
        Some((at, yaw)) => show(game, dimension, (f.id, PLANE_PART), rules::BOMBER_MODEL, rules::BOMBER_SCALE, 0.0, at, yaw, 0.0),
        None => hide(game, dimension, (f.id, PLANE_PART)),
    }
    if now < pass.release {
        return;
    }
    // The bomb leaves the beacon's slot as it leaves the plane, tried again each tick until it is done: a beacon
    // broken before then drops it and calls the strike off, one broken after drops nothing.
    if f.to_empty(now) && game.native_with_mod_block_entity(f.from, |e| e.items.iter_mut().for_each(|s| *s = None)).is_some() {
        changes.push(Change::Emptied(f.id));
    }
    let surface = match f.surface {
        Some(y) => Some(y),
        None => {
            let read = ground_at(game, dimension, f.from[0], f.from[2]);
            if let Some(y) = read {
                changes.push(Change::Surface(f.id, y));
            }
            read
        }
    };
    let ground = rules::Flight { surface, ..f.clone() }.bomb_ground();
    let lands = pass.impact(ground);
    match (pass.bomb(now), lands) {
        (Some(_), Some(t)) if now >= t => {
            let at = pass.bomb(t).unwrap_or(beacon);
            hide(game, dimension, (f.id, BOMB_PART));
            changes.push(Change::Impact(f.id, [at[0].floor() as i32, ground, at[2].floor() as i32], f.kind));
        }
        (Some(at), _) => {
            let (_, yaw) = pass.plane(pass.release).unwrap_or((at, 0.0));
            // It noses down as it falls.
            let pitch = ((now - pass.release) as f32 * 1.5).min(80.0);
            show(game, dimension, (f.id, BOMB_PART), f.kind.id(), 1.0, 0.0, at, yaw, pitch);
        }
        // Past the end of its worked-out fall: it goes off at the beacon.
        (None, _) => {
            hide(game, dimension, (f.id, BOMB_PART));
            changes.push(Change::Impact(f.id, [f.from[0], ground, f.from[2]], f.kind));
        }
    }
}

/// A line to every player of `dimension` near `at`.
fn tell_around(game: &mut Game<'_>, dimension: &str, at: [f64; 3], line: &Text) {
    for p in game.native_server_players() {
        if p.dimension == dimension && distance(p.position, at) <= BEACON_TOLD {
            game.native_tell(p.uuid, line.clone());
        }
    }
}
