//! The guide. A right click with it, on the server, asks the player's window to open it. The window opens the mod's
//! own screen, whose hooks only read the game: the open book lives in a static, and a page turned is heard at the
//! next frame. The book stays after the screen closes, so it opens again where it was left.

use crate::core::{self, argb, lock};
use crate::rules;
use mod_api::{ClientHost, Identifier, ScreenRenderPostEvent, ScreenView, ServerHost, SoundPlay, SoundSource, Text};
use std::{
    sync::{Mutex, OnceLock},
    time::Instant,
};

/// The players who right-clicked with the guide lately, by the wall clock: a click held down asks once, so a window
/// that sees the book late (a laggy server) doesn't get it again after Escape.
static GUIDE_CLICKS: Mutex<rules::Clicks<u128>> = Mutex::new(rules::Clicks::new(rules::GUIDE_CLICK_MS));
/// The wall clock the guide's clicks are counted on: a laggy or frozen server's ticks would swallow real clicks.
static GUIDE_EPOCH: OnceLock<Instant> = OnceLock::new();

/// A right click with the guide: the player's window opens it. The use is claimed even when the message can't go,
/// so the guide never acts as a plain book.
pub(crate) fn read_guide(host: &mut dyn ServerHost, player: u128) {
    let now = GUIDE_EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64;
    if lock(&GUIDE_CLICKS).heard(player, now).is_some() {
        return;
    }
    if let Err(e) = core::send(host, player, rules::GUIDE_CHANNEL, Vec::new()) {
        log::warn!("[big_nukes] opening the guide: {e}");
    }
}

static GUIDE_BOOK: Mutex<Option<rules::Guide>> = Mutex::new(None);
static PAGE_TURNED: Mutex<bool> = Mutex::new(false);

/// The server's message: the book is laid out in the window's language and opened, at the spread it was left at or,
/// the first time, at its contents.
pub(crate) fn asked(host: &mut dyn ClientHost) {
    // A message late for a book already open is let go.
    if host.mod_screen() == Some(rules::ID) {
        return;
    }
    let guide = {
        let host: &dyn ClientHost = &*host;
        let mut guide = rules::Guide::new(&|key: &str| core::lang(host, key), &|text: &str| {
            host.text_width(&Text::literal(text.to_owned())).unwrap_or(0) as f32
        });
        if let Some(old) = lock(&GUIDE_BOOK).take() {
            guide.keep(&old);
        }
        guide
    };
    *lock(&GUIDE_BOOK) = Some(guide);
    if !host.mod_screen_open(rules::ID) {
        log::info!("[big_nukes] the guide can't open over the screen that is open");
    }
}

/// Every mod's own screen is `mod`: this one is the guide when the open mod screen is this mod's.
fn shown(host: &dyn ClientHost, screen: &ScreenView<'_>) -> bool {
    screen.id == "mod" && host.mod_screen() == Some(rules::ID)
}

/// Does `act` to the open book; a page it turns is heard at the next frame.
fn turn<R>(guide: &mut rules::Guide, act: impl FnOnce(&mut rules::Guide) -> R) -> R {
    let before = guide.spread;
    let done = act(guide);
    if guide.spread != before {
        *lock(&PAGE_TURNED) = true;
    }
    done
}

/// An item's name in the window's language: a block's item is named as its block.
fn item_title(host: &dyn ClientHost, id: &str) -> String {
    let path = id.replace(':', ".");
    let item = format!("item.{path}");
    if core::lang_has(host, &item) { core::lang(host, &item) } else { core::lang(host, &format!("block.{path}")) }
}

/// The guide's screen, over the world. Its texture ships with the mod, so the old plain-colour stand-in is gone.
pub(crate) fn render(host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
    if !shown(host, event.screen) {
        return;
    }
    let draws = {
        let mut book = lock(&GUIDE_BOOK);
        let Some(guide) = book.as_mut() else {
            return;
        };
        let draw: &dyn mod_api::DrawContext = &*event.draw;
        guide.window = draw.size();
        guide.draw(event.screen.mouse, &|text: &str| draw.text_width(text), &|id: &str| item_title(host, id))
    };
    let draw = &mut *event.draw;
    let [w, h] = draw.size();
    draw.fill([0.0, 0.0, w, h], rules::GUIDE_DIM);
    for d in draws {
        match d {
            rules::GuideDraw::Sprite(src, dst) => {
                draw.sprite(rules::GUIDE_GUI, src, dst, None);
            }
            rules::GuideDraw::Fill(rect, color) => draw.fill(rect, color),
            rules::GuideDraw::Text(text, at, rgb) => draw.text(&text, at, argb(rgb), false),
            rules::GuideDraw::Item(id, at) => {
                if let Some(key) = Identifier::parse(id).ok().and_then(|id| host.item_key(&id)) {
                    draw.item(key, at, 16.0);
                }
            }
        }
    }
}

/// A mouse button over the guide: a bookmark or an arrow. Whether it was the guide's.
pub(crate) fn mouse(host: &dyn ClientHost, screen: &ScreenView<'_>, button: i32) -> bool {
    if !shown(host, screen) {
        return false;
    }
    if let Some(guide) = lock(&GUIDE_BOOK).as_mut() {
        if button == 0 {
            turn(guide, |g| g.click(screen.mouse));
        }
        return true;
    }
    false
}

/// The wheel over the guide turns its pages.
pub(crate) fn wheel(host: &dyn ClientHost, screen: &ScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !shown(host, screen) {
        return false;
    }
    if let Some(guide) = lock(&GUIDE_BOOK).as_mut() {
        turn(guide, |g| g.wheel(scroll[1]));
        return true;
    }
    false
}

/// The arrow keys turn the pages and Home goes to the contents; Escape and the rest are the game's.
pub(crate) fn key(host: &dyn ClientHost, screen: &ScreenView<'_>, key: &str) -> bool {
    shown(host, screen) && lock(&GUIDE_BOOK).as_mut().is_some_and(|guide| turn(guide, |g| g.key(key)))
}

/// A page turned since the last frame: the book's own sound, where the player stands.
pub(crate) fn frame(host: &mut dyn ClientHost) {
    if !std::mem::take(&mut *lock(&PAGE_TURNED)) {
        return;
    }
    if let Some((_, feet)) = crate::client::me(host) {
        let sound = SoundPlay::at("minecraft:item.book.page_turn", SoundSource::Players, [feet[0], feet[1] + 1.62, feet[2]]);
        let _ = host.play_local_sound(&sound, 0);
    }
}
