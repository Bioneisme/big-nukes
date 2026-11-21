//! The silo and the bomber (plan 7): the launch pad, the console, the strike beacon, the siren, the missiles'
//! flights and the bomber's drops, the launch camera and the notices.
//!
//! This file holds the blocks, the console's server side (a launch, its checks, the shaft), the strike beacon's
//! click and the siren; `silo_flights.rs` the flights, the bomber's passes and the displays (the flying missile, the
//! bomber, its bomb and the missile standing on its pad); `silo_window.rs` the window's side (the console's screen
//! and fields, the pad's screen, the launch camera and the notices, drawn where the old glue added toasts).

#[path = "silo_flights.rs"]
mod flights;
#[path = "silo_window.rs"]
mod window;

pub(crate) use window::{
    camera_pose, char_typed, console_click, console_draw, console_screen, forget_camera, frame, gui_render_post,
    key_pressed, net_from_server, pad_draw, pad_screen,
};

use crate::core::{self, Pl, lit, lock, middle, tr};
use crate::rules;
use mod_api::{
    Block, BlockUpdateFlags, Direction, Hand, HostBlockStates, HostTransaction, Item, ItemStack, LevelRef, MenuQuickMoveEvent,
    ModBlockEntityView, ModContainer, PlayerRef, Rarity, ServerHost, SoundSource, Text,
};
use std::sync::Mutex;

// ---- declarations (called from `world::blocks`/`world::items` and at registration) ----
//
// The launch pad holds the missile and its warhead in a menu of two slots; the console and the beacon have one
// hidden slot each, as a bomb has: the console's for its menu, which needs one, the beacon's for the bomb it was
// charged with, so a broken beacon drops it. The hatch is a plain block.

/// The silo's blocks, `block(id, template, hardness)` as the core's.
pub(crate) fn blocks(blocks: &mut Vec<Block>, block: &dyn Fn(&str, &str, f32) -> Block) {
    blocks.push(block(rules::LAUNCH_PAD, "minecraft:furnace", 5.0));
    blocks.push(block(rules::SILO_HATCH, "minecraft:iron_block", 5.0));
    blocks.push(block(rules::LAUNCH_CONSOLE, "minecraft:furnace", 3.5));
    blocks.push(block(rules::STRIKE_BEACON, "minecraft:furnace", 3.5));
    // A copper bulb's states: lit while it wails.
    blocks.push(block(rules::SIREN, "minecraft:waxed_copper_bulb", 3.5));
}

/// The items that only show the flights (the missile's and the bomber's models): items no one is meant to hold.
pub(crate) fn model_items(items: &mut Vec<Item>) {
    for id in rules::MODEL_ITEMS {
        let mut item = Item::new(id, "minecraft:paper");
        item.max_stack_size = Some(1);
        item.rarity = Some(Rarity::Common);
        items.push(item);
    }
}

/// The silo's blocks with a block entity: registered with the core's `Machine` part.
pub(crate) fn has_entity(block: &str) -> bool {
    matches!(block, rules::LAUNCH_PAD | rules::LAUNCH_CONSOLE | rules::STRIKE_BEACON | rules::SIREN)
}

/// A block's container (slots, faces, menu), read once at registration.
pub(crate) fn container(block: &str) -> Option<ModContainer> {
    match block {
        // Hoppers bring the missile from above and the warhead from the sides.
        rules::LAUNCH_PAD => {
            let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| vec![rules::PAD_WARHEAD_SLOT]);
            faces[Direction::Up as usize] = vec![rules::PAD_MISSILE_SLOT];
            Some(ModContainer {
                slots: rules::PAD_SLOTS,
                faces,
                inputs: vec![rules::PAD_MISSILE_SLOT, rules::PAD_WARHEAD_SLOT],
                outputs: vec![],
                menu: Some(rules::LAUNCH_PAD.to_owned()),
            })
        }
        block if has_entity(block) => Some(ModContainer {
            slots: 1,
            faces: Default::default(),
            inputs: vec![],
            outputs: vec![],
            menu: (block == rules::LAUNCH_CONSOLE).then(|| rules::CONSOLE_MENU.to_owned()),
        }),
        _ => None,
    }
}

// ---- the server -------------------------------------------------------------------

/// The end of a level's tick (`level_tick_post`, before the waves): the launches asked of its consoles, its
/// missiles' flights, its bombers' passes, the missiles its pads hold and its sirens. Only here do the displays,
/// the hatch and the commands reach the level.
pub(crate) fn level_tick(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    launches(host, level, dimension);
    flights::flights(host, level, dimension);
    take_away(host, dimension);
    flights::strikes(host, level, dimension);
    flights::pads(host, level, dimension);
    sirens(host, level, dimension);
}

/// A silo block entity's server tick (pad, console, beacon, siren). `host.dimension()` is its level's.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    let id = entity.id().to_owned();
    match id.as_str() {
        rules::LAUNCH_CONSOLE => console_tick(host, pos, entity),
        rules::LAUNCH_PAD => flights::pad_tick(host, pos, entity),
        rules::SIREN => siren_tick(host, pos),
        _ => {}
    }
}

/// The sirens heard in their ticks, set and sounded at the end of their level's tick: the dimension, the place,
/// whether it is to be lit, and whether a wail starts.
static SIRENS: Mutex<Vec<(String, [i32; 3], bool, bool)>> = Mutex::new(Vec::new());

/// A siren: lit, and wailing every `WAIL_EVERY`, while an alarm covers it (`Saved::alarm_at`). It looks twice a
/// second; its block is changed at the end of the level's tick (`sirens`), not while its own block entity is out
/// of the world for its tick.
fn siren_tick(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let dimension = host.dimension();
    let (now, wails) = {
        let Some(guard) = core::state(host) else { return };
        let st = guard.as_ref().expect("opened");
        (st.saved.tick, st.saved.alarm_at(dimension, pos))
    };
    if now % 10 != 0 {
        return;
    }
    let mut sirens = lock(&SIRENS);
    if sirens.len() < 4096 {
        sirens.push((dimension.to_owned(), pos, wails, wails && now % rules::WAIL_EVERY == 0));
    }
}

fn sirens(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    let mine: Vec<([i32; 3], bool, bool)> = {
        let mut sirens = lock(&SIRENS);
        if sirens.is_empty() {
            return;
        }
        let (mine, rest): (Vec<_>, Vec<_>) = sirens.drain(..).partition(|(d, ..)| d == dimension);
        *sirens = rest;
        mine.into_iter().map(|(_, pos, lit, wail)| (pos, lit, wail)).collect()
    };
    for (pos, lit, wail) in mine {
        if core::block_at(host, level, pos) != rules::SIREN {
            continue;
        }
        let lit = if lit { "true" } else { "false" };
        if level.block_property(host, pos, "lit").as_deref() != Some(lit)
            && let Some(state) = level.block_state(host, pos)
            && let Ok(state) = state.with(&host.block_states(), "lit", lit)
        {
            let _ = level.set_block(host, pos, state, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
        }
        if wail {
            play(host, dimension, rules::SIREN_WAIL, middle(pos), 6.0, 1.0);
        }
    }
}

/// A sound for whoever is near: as `/playsound … block @a`, heard 16 blocks for each of volume.
fn play(host: &mut dyn ServerHost, dimension: &str, sound: &str, at: [f64; 3], volume: f32, pitch: f32) {
    core::play(host, dimension, sound, at, SoundSource::Blocks, volume, pitch);
}

/// A notice for a player's window (`NOTICE_CHANNEL`): events.rs calls it too.
pub(crate) fn notify(host: &mut dyn ServerHost, player: u128, notice: &rules::Notice) {
    let Ok(bytes) = serde_json::to_vec(notice) else { return };
    if let Err(e) = core::send(host, player, rules::NOTICE_CHANNEL, bytes) {
        log::warn!("[big_nukes] a notice to a player: {e}");
    }
}

/// The flights launched now from the pad under `from`: the sirens are sounded for each, the one who launched is
/// told when it lands, and the players round each target that it is coming.
fn tell_launch(host: &mut dyn ServerHost, dimension: &str, from: [i32; 3], aim: [i32; 2], who: u128) {
    let (now, launched) = {
        let Some(mut guard) = core::state(host) else { return };
        let st = guard.as_mut().expect("opened");
        let now = st.saved.tick;
        let launched: Vec<rules::Flight> =
            st.saved.flights.iter().filter(|f| f.dimension == dimension && f.from == from && f.start == now).cloned().collect();
        st.saved.prune_alarms();
        for f in &launched {
            st.saved.alarm_for(f);
        }
        core::save(st);
        (now, launched)
    };
    let Some(last) = launched.iter().max_by_key(|f| f.lands_at()) else { return };
    notify(host, who, &rules::Notice::launched(last, aim, now));
    let reach = f64::from(rules::TARGET_ALARM_REACH);
    for p in core::players(host).into_iter().filter(|p| p.dimension == dimension && p.uuid != who) {
        let near = launched.iter().find(|f| (p.position[0] - f64::from(f.to[0])).hypot(p.position[2] - f64::from(f.to[1])) <= reach);
        if let Some(f) = near {
            notify(host, p.uuid, &rules::Notice::inbound(f, now));
        }
    }
}

/// A right click on a block (server): whether it was the silo's. The console tells the window where it is, so a
/// launch can name it, and opens; the pad opens; the strike beacon takes a bomb and never opens. `canceled` stays
/// false for the console and the pad, so the game opens their menus.
pub(crate) fn right_click(host: &mut dyn ServerHost, who: &Pl, pos: [i32; 3], canceled: &mut bool) -> bool {
    let Some(level) = who.player.level(host) else { return false };
    match core::block_at(host, level, pos).as_str() {
        rules::LAUNCH_CONSOLE => {
            news(host, who.uuid, &rules::ConsoleNews { at: Some(pos), ..Default::default() });
            true
        }
        rules::LAUNCH_PAD => true,
        rules::STRIKE_BEACON => {
            beacon_click(host, who, level, pos, canceled);
            true
        }
        _ => false,
    }
}

/// A shift-click in the launch pad's menu: whether it was handled (the game's routing is then skipped).
///
/// From the inventory, the missile and the warhead go to their slots. A mod-api stack can't be empty, so a move of
/// the whole stack is left to the game, which routes it under the menu's slot rules (`rules::pad_takes`); only a
/// top-up that leaves some behind is made here. What the pad takes nowhere does nothing, as before.
pub(crate) fn pad_quick_move(host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> bool {
    if event.slot < event.container_slots {
        return false;
    }
    let item = event.stack.item().id(host);
    let Some(to) = [rules::PAD_MISSILE_SLOT, rules::PAD_WARHEAD_SLOT].into_iter().find(|s| rules::pad_takes(*s, &item)) else {
        return true;
    };
    if event.entity.items.len() < rules::PAD_SLOTS {
        event.entity.items.resize(rules::PAD_SLOTS, None);
    }
    let most = host.max_stack_size(event.stack.item()).unwrap_or(64);
    let count = event.stack.count();
    let moved = match event.entity.items[to].as_ref() {
        // An empty slot takes the whole stack: the game's to move.
        None => return false,
        Some(have) if core::stacks_with(have, event.stack) => most.saturating_sub(have.count()).min(count),
        Some(_) => 0,
    };
    if moved == count {
        return false;
    }
    if moved > 0 {
        let Some(have) = event.entity.items[to].as_mut() else { return false };
        let total = have.count() + moved;
        if have.set_count(total).is_err() || event.stack.set_count(count - moved).is_err() {
            return false;
        }
    }
    true
}

/// A launch's or a beacon's answer as a line of chat.
fn text(line: &rules::Reply) -> Text {
    tr(&line.key, line.args.iter().map(lit).collect())
}

fn news(host: &mut dyn ServerHost, player: u128, news: &rules::ConsoleNews) {
    let Ok(bytes) = serde_json::to_vec(news) else { return };
    if let Err(e) = core::send(host, player, rules::CONSOLE_CHANNEL, bytes) {
        log::warn!("[big_nukes] telling a player of their console: {e}");
    }
}

/// Item slot `slot` of the block entity at `pos` set: None when there is none or it refused.
fn set_slot(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], slot: usize, stack: Option<ItemStack>) -> Option<bool> {
    let be = host.block_entity(level, pos)?;
    host.transaction(|host, tx| be.set_item_slot(host, tx, slot as u32, stack.clone()).map_err(drop)).ok()
}

/// A beacon, a console or a pad taken away (any way: `block_changed` hears it, maybe while the state is locked):
/// its strike, its code or its missile's display goes at the end of its level's tick.
pub(crate) fn taken(dimension: &str, pos: [i32; 3]) {
    lock(&TAKEN).push((dimension.to_owned(), pos));
}

/// The world closing: what waits for a tick goes.
pub(crate) fn forget() {
    lock(&LAUNCHES).clear();
    lock(&SIRENS).clear();
    lock(&TAKEN).clear();
    *lock(&LAST_CHARGE) = rules::Clicks::new(rules::HELD_CLICK_TICKS);
    *lock(&SAME_TICK) = None;
    flights::forget();
}

// ---- the console, on the server ---------------------------------------------------
//
// A launch comes from the player's window as a mod message; it is worked at the end of the tick of the player's
// level, where the console, its pad and its shaft are in place. The console's code is the world's
// (`Saved.consoles`), the flight too (`Saved.flights`).

/// Launches asked for: the player and the request.
static LAUNCHES: Mutex<Vec<(u128, rules::LaunchMessage)>> = Mutex::new(Vec::new());
/// The most launches waiting at once, so a client can't fill the queue.
const LAUNCHES_WAITING: usize = 64;
/// How far from a console its player may stand.
const STAND_BY: f64 = rules::CONSOLE_REACH as f64 + 4.0;

/// A message from a player's window (`LAUNCH_CHANNEL`): a launch typed on a console.
pub(crate) fn net_from_client(_host: &mut dyn ServerHost, player: PlayerRef, channel: &str, data: &[u8]) {
    if channel != rules::LAUNCH_CHANNEL {
        return;
    }
    match rules::read_launch(data) {
        Some(m) => {
            let mut waiting = lock(&LAUNCHES);
            if waiting.len() < LAUNCHES_WAITING {
                waiting.push((player.uuid(), m));
            }
        }
        None => log::warn!("[big_nukes] an unreadable launch from a client"),
    }
}

/// The launches of the players in `dimension`: each is answered in the chat and on the console's screen.
fn launches(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    if lock(&LAUNCHES).is_empty() {
        return;
    }
    let players = core::players(host);
    let mine: Vec<(Pl, rules::LaunchMessage)> = {
        let mut waiting = lock(&LAUNCHES);
        // A player who left is gone with their launch.
        waiting.retain(|(uuid, _)| players.iter().any(|p| p.uuid == *uuid));
        let (mine, rest): (Vec<_>, Vec<_>) =
            waiting.drain(..).partition(|(uuid, _)| players.iter().any(|p| p.uuid == *uuid && p.dimension == dimension));
        *waiting = rest;
        mine.into_iter().filter_map(|(uuid, m)| Some((players.iter().find(|p| p.uuid == uuid)?.clone(), m))).collect()
    };
    for (who, m) in mine {
        let line = launch(host, level, dimension, &who, &m);
        core::tell(host, who.uuid, text(&line));
        news(host, who.uuid, &rules::ConsoleNews { reply: Some(line), ..Default::default() });
    }
}

/// A launch from the console at `m.console`, by `who`: the answer, the launch's or why not. The code is checked
/// first, so a wrong one counts whatever the shaft's state.
fn launch(host: &mut dyn ServerHost, level: LevelRef, dimension: &str, who: &Pl, m: &rules::LaunchMessage) -> rules::Reply {
    let console = m.console;
    if core::distance(who.position, middle(console)) > STAND_BY {
        return rules::Reply::new("big_nukes.console.stand_by", &[]);
    }
    if core::block_at(host, level, console) != rules::LAUNCH_CONSOLE {
        return rules::Reply::new("big_nukes.console.gone", &[]);
    }
    let Some(pad) = find_pad(host, level, console) else {
        return rules::ShaftProblem::NoPad.line();
    };
    let (to, code) = match rules::launch_request([pad[0], pad[2]], &m.x, &m.z, &m.code) {
        Ok(asked) => asked,
        Err(why) => return why.line(),
    };
    let from = [pad[0], pad[1] + 1, pad[2]];
    {
        let Some(mut guard) = core::state(host) else {
            return rules::Reply::new("big_nukes.console.gone", &[]);
        };
        let st = guard.as_mut().expect("opened");
        let now = st.saved.tick;
        let entry = st.saved.consoles.entry(rules::console_key(dimension, console)).or_default();
        let was = entry.clone();
        let checked = entry.check(&code, now);
        if *entry != was {
            core::save(st);
        }
        if let Err(why) = checked {
            return why.line();
        }
        if st.saved.pad_busy(dimension, from, now) {
            return rules::Reply::new("big_nukes.console.busy", &[]);
        }
    }
    let hatch = match read_shaft(host, level, dimension, pad) {
        Ok(h) => pad[1] + h,
        Err(why) => return why.line(),
    };
    let (missile, warhead) = pad_items(host, level, pad);
    let kind = match rules::pad_load(missile.as_deref(), warhead.as_deref()) {
        Ok(kind) => kind,
        Err(why) => return why.line(),
    };
    let split = rules::splits(missile.as_deref());
    let spread = match split.then(|| rules::spread_of(&m.spread)).transpose() {
        Ok(spread) => spread,
        Err(why) => return why.line(),
    };
    let flight = {
        let Some(mut guard) = core::state(host) else {
            return rules::Reply::new("big_nukes.console.gone", &[]);
        };
        let st = guard.as_mut().expect("opened");
        let launched = match spread {
            Some(spread) => st.saved.launch_split(dimension, from, to, spread, hatch),
            None => st.saved.launch(dimension, from, to, kind, hatch),
        };
        match launched {
            Ok(f) => {
                st.saved.consoles.entry(rules::console_key(dimension, console)).or_default().keep(&code);
                core::save(st);
                f
            }
            Err(why) => return why.line(),
        }
    };
    // The missile and its warhead leave the pad; a splitting missile's own warheads are in it, and the slot's stays.
    let slots: &[usize] = if split { &[rules::PAD_MISSILE_SLOT] } else { &[rules::PAD_MISSILE_SLOT, rules::PAD_WARHEAD_SLOT] };
    for &slot in slots {
        let Some(be) = host.block_entity(level, pad) else { break };
        let Ok(Some(mut stack)) = be.item_slot(host, slot as u32) else { continue };
        let left = stack.count().saturating_sub(1);
        let left = if left == 0 || stack.set_count(left).is_err() { None } else { Some(stack) };
        if set_slot(host, level, pad, slot, left).is_none() {
            log::warn!("[big_nukes] the pad at {pad:?} in {dimension} kept slot {slot} of missile {}", flight.id);
        }
    }
    // The flight's display takes its place at the end of this tick (`flights`).
    flights::hide_pad(host, dimension, pad);
    tell_launch(host, dimension, from, to, who.uuid);
    log::info!("[big_nukes] missile {}: {:?} from {pad:?} to {to:?} in {dimension}, by {}", flight.id, kind, who.name);
    if m.camera {
        news(host, who.uuid, &rules::ConsoleNews { now: flight.start, missile: Some(flight.clone()), ..Default::default() });
    }
    let seconds = rules::LAUNCH_COUNTDOWN / rules::SECOND;
    rules::Reply::new("big_nukes.console.launched", &[&to[0], &to[1], &seconds])
}

/// The launch pad nearest the console, within `CONSOLE_REACH`.
fn find_pad(host: &dyn ServerHost, level: LevelRef, console: [i32; 3]) -> Option<[i32; 3]> {
    let r = rules::CONSOLE_REACH;
    let pad = host.block_key(rules::LAUNCH_PAD)?;
    let mut best: Option<(i32, [i32; 3])> = None;
    for dx in -r..=r {
        for dy in -r..=r {
            for dz in -r..=r {
                let d = dx * dx + dy * dy + dz * dz;
                if d > r * r || best.is_some_and(|(b, _)| b <= d) {
                    continue;
                }
                let at = [console[0] + dx, console[1] + dy, console[2] + dz];
                if level.block(host, at) == Some(pad) {
                    best = Some((d, at));
                }
            }
        }
    }
    best.map(|(_, at)| at)
}

/// The shaft over the pad, up to the sky or the Nether's roof: the hatch's height over the pad.
fn read_shaft(host: &dyn ServerHost, level: LevelRef, dimension: &str, pad: [i32; 3]) -> Result<i32, rules::ShaftProblem> {
    let top = rules::world_of(dimension).map_or(320, |w| w.roof.min(w.top()));
    let column: Vec<String> = (pad[1] + 1..top).map(|y| core::block_at(host, level, [pad[0], y, pad[2]])).collect();
    let names: Vec<&str> = column.iter().map(String::as_str).collect();
    rules::shaft(&names)
}

/// What the pad holds in its two slots.
fn pad_items(host: &dyn ServerHost, level: LevelRef, pad: [i32; 3]) -> (Option<String>, Option<String>) {
    let Some(be) = host.block_entity(level, pad) else { return (None, None) };
    let item = |slot: usize| be.item_slot(host, slot as u32).ok().flatten().map(|s| s.item().id(host));
    (item(rules::PAD_MISSILE_SLOT), item(rules::PAD_WARHEAD_SLOT))
}

/// Once a second, the console's screen learns the state of its shaft, its countdown or its lock.
fn console_tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    let Some((now, _)) = core::clock(host) else { return };
    if entity.data.len() == rules::CONSOLE_DATA && now % rules::SECOND != 0 {
        return;
    }
    let data = console_state(host, pos, now).data().to_vec();
    if entity.data != data {
        entity.data = data;
    }
}

fn console_state(host: &dyn ServerHost, pos: [i32; 3], now: u64) -> rules::ConsoleState {
    let dimension = host.dimension();
    let Some(level) = host.level(dimension) else {
        return rules::ConsoleState::Shaft(rules::ShaftProblem::NoPad);
    };
    let pad = find_pad(host, level, pos);
    let (locked, flight) = {
        let Some(guard) = core::state(host) else { return rules::ConsoleState::Shaft(rules::ShaftProblem::NoPad) };
        let st = guard.as_ref().expect("opened");
        let locked = st.saved.consoles.get(&rules::console_key(dimension, pos)).map_or(0, |c| c.locked_until.min(now + rules::LOCK_TICKS));
        let flight = pad.and_then(|p| st.saved.launching(dimension, [p[0], p[1] + 1, p[2]], now).cloned());
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
    if let Err(why) = read_shaft(host, level, dimension, pad) {
        return rules::ConsoleState::Shaft(why);
    }
    let (missile, warhead) = pad_items(host, level, pad);
    match rules::pad_load(missile.as_deref(), warhead.as_deref()) {
        Ok(_) => rules::ConsoleState::Ready,
        Err(why) => rules::ConsoleState::Shaft(why),
    }
}

// ---- the strike beacon --------------------------------------------------------------
//
// A right click with a tactical warhead or an atomic bomb charges a beacon: the bomb goes from the hand into the
// beacon's hidden slot, so a beacon broken before the drop gives it back, and the world records the strike.
// Thirty seconds of red smoke later the bomber flies over (`silo_flights.rs`).

/// The player, the beacon and the tick of the last click handled on a beacon: the game asks once per hand, and the
/// off hand's turn is let go even when the main hand's took the bomb it held.
static SAME_TICK: Mutex<Option<(u128, [i32; 3], u64)>> = Mutex::new(None);
/// The clicks heard on the beacons: the player, the beacon and the item in hand. A click held down, which the
/// game repeats, is answered once.
static LAST_CHARGE: Mutex<rules::Clicks<(u128, [i32; 3], Option<String>)>> = Mutex::new(rules::Clicks::new(rules::HELD_CLICK_TICKS));
/// Beacons, consoles and pads taken away, heard from the level, which may change a block while the state is locked.
static TAKEN: Mutex<Vec<(String, [i32; 3])>> = Mutex::new(Vec::new());

/// The beacons, consoles and pads of `dimension` taken away since its last tick: a beacon's strike is called off
/// before its drop (its bomb dropped with the beacon), a console's code is forgotten, so a new one starts afresh,
/// and a pad's missile no longer shows.
fn take_away(host: &mut dyn ServerHost, dimension: &str) {
    let gone: Vec<[i32; 3]> = {
        let mut taken = lock(&TAKEN);
        if taken.is_empty() {
            return;
        }
        let (mine, rest): (Vec<_>, Vec<_>) = taken.drain(..).partition(|(d, _)| d == dimension);
        *taken = rest;
        mine.into_iter().map(|(_, pos)| pos).collect()
    };
    if gone.is_empty() {
        return;
    }
    for &pos in &gone {
        flights::pad_taken(host, dimension, pos);
    }
    let Some(mut guard) = core::state(host) else { return };
    let st = guard.as_mut().expect("opened");
    for pos in gone {
        if let Some(kind) = st.saved.uncharge(dimension, pos) {
            log::info!("[big_nukes] the beacon at {pos:?} in {dimension} is gone: its {kind:?} isn't dropped");
        }
        st.saved.consoles.remove(&rules::console_key(dimension, pos));
    }
    core::save(st);
}

/// A right click on a strike beacon: a bomb that flies charges it, anything else hears what it takes. The furnace
/// it is built on never opens. Sneaking with something else in hand lets it be used (a block placed against it).
fn beacon_click(host: &mut dyn ServerHost, who: &Pl, level: LevelRef, pos: [i32; 3], canceled: &mut bool) {
    let hand = core::held(host, who);
    let held = hand.as_ref().map(|s| s.item().id(host));
    let flies = held.as_deref().and_then(rules::BombKind::of_block).is_some_and(rules::BombKind::flies);
    if held.is_some() && !flies && core::sneaking(host, who) {
        return;
    }
    *canceled = true;
    let Some((now, _)) = core::clock(host) else { return };
    {
        let mut last = lock(&SAME_TICK);
        if *last == Some((who.uuid, pos, now)) {
            return;
        }
        *last = Some((who.uuid, pos, now));
    }
    if lock(&LAST_CHARGE).heard((who.uuid, pos, held.clone()), now).is_some() {
        return;
    }
    let dimension = who.dimension.clone();
    let charged = {
        let Some(mut guard) = core::state(host) else { return };
        let st = guard.as_mut().expect("opened");
        let charge = st.saved.charge(&dimension, pos, held.as_deref());
        let left = st.saved.beacon_at(&dimension, pos).map(|f| f.pass().arrives().saturating_sub(now));
        (charge, left)
    };
    let seconds = |ticks: Option<u64>| ticks.unwrap_or(0).div_ceil(rules::SECOND);
    match charged {
        (rules::Charge::Taken(kind), left) => {
            // One of the very stack in hand, a renamed bomb too, into the beacon's slot (a broken beacon gives it
            // back as it was), then out of the hand unless creative.
            let hand = hand.filter(|_| held.as_deref() == Some(kind.id()));
            let one = hand.clone().and_then(|mut s| s.set_count(1).ok().map(|_| s));
            let creative = who.creative();
            let mut g: &mut dyn ServerHost = host;
            let charged = rules::charge_from_hand(
                &mut g,
                one,
                |g, one| set_slot(&mut **g, level, pos, 0, Some(one)).is_some(),
                |g| creative || hand.as_ref().is_some_and(|seen| take_from_hand(&mut **g, who, seen)),
                |g| {
                    set_slot(&mut **g, level, pos, 0, None);
                },
            );
            let host = g;
            if !charged {
                if let Some(mut guard) = core::state(host) {
                    let st = guard.as_mut().expect("opened");
                    st.saved.beacons.retain(|f| f.dimension != dimension || f.from != pos);
                }
                core::tell(host, who.uuid, text(&rules::Reply::new("big_nukes.beacon.not_taken", &[])));
                return;
            }
            if let Some(mut guard) = core::state(host) {
                core::save(guard.as_mut().expect("opened"));
            }
            log::info!("[big_nukes] a bomber is called to {pos:?} in {dimension} with a {kind:?}, by {}", who.name);
            play(host, &dimension, "minecraft:block.beacon.activate", middle(pos), 1.0, 1.4);
            core::tell(host, who.uuid, text(&rules::Reply::new("big_nukes.beacon.charged", &[&seconds(left)])));
        }
        (rules::Charge::Busy, left) => core::tell(host, who.uuid, text(&rules::Reply::new("big_nukes.beacon.busy", &[&seconds(left)]))),
        (rules::Charge::Not, _) => core::tell(host, who.uuid, text(&rules::Reply::new("big_nukes.beacon.hint", &[]))),
        (rules::Charge::Maxpower { radius, maxpower }, _) => {
            core::tell(host, who.uuid, core::refused(rules::Refused::Maxpower { radius, maxpower }));
        }
    }
}

/// One of `seen`, the stack seen in the player's main hand, taken from that hand, a renamed one too: the hand is
/// read again and written back one less, if it still holds that very stack. Whether one was.
fn take_from_hand(host: &mut dyn ServerHost, who: &Pl, seen: &ItemStack) -> bool {
    let Some(living) = host.living(who.player.entity()) else { return false };
    let Some(mut stack) = living.hand_item(host, Hand::Main) else { return false };
    if !core::stacks_with(&stack, seen) {
        return false;
    }
    let left = stack.count().saturating_sub(1);
    let left = if left == 0 || stack.set_count(left).is_err() { None } else { Some(stack) };
    matches!(living.set_hand_item(host, Hand::Main, left), Ok(true))
}
