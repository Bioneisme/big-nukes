//! The bombs, their keypad, the Sunbreaker and the remote.
//!
//! A bomb's state lives in its block entity: `nbt` holds the rules' `Bomb` as JSON, its one hidden slot the
//! detonator's item, and `data` what the keypad's screen shows. A right click puts a detonator in, lights a fuse or
//! opens the keypad; the block entity's tick counts down, hisses, beeps and sets the blast off, taking the block
//! with it. Its clock is `Saved.tick`.

use crate::core::{self, Pl, argb, block_at, lit, lock, middle, tell, tr};
use crate::machines::LABEL;
use crate::world::{self, read_from, structure_now};
use crate::{keys, rules};
use mod_api::{
    ClientHost, Component, Compound, DrawContext, ENTITY_ITEMS, Hand, HostTransaction, ItemResource, ItemStack, LevelRef,
    ModBlockEntityView, ModContainer, ParticleOptions, Screen, ServerHost, SoundSource, Tag, Text,
};
use std::{collections::HashMap, sync::Mutex};

/// How far a Sunbreaker's refusal is told when its fuse burns out: whoever lit it has run.
const SUNBREAKER_TOLD_WITHIN: f64 = 256.0;

/// The player, the bomb and the tick of the last click handled on a bomb. The game asks once per hand: the off
/// hand's turn of the same click is let go, so nothing goes in or is lit twice, even when the main hand's turn used
/// up the item it held.
static SAME_TICK: Mutex<Option<(u128, [i32; 3], u64)>> = Mutex::new(None);
/// The clicks heard on the bombs: the player, the bomb and the item in hand. A click held down, which the game
/// repeats, is answered once.
static LAST_CLICK: Mutex<rules::Clicks<(u128, [i32; 3], Option<String>)>> = Mutex::new(rules::Clicks::new(rules::HELD_CLICK_TICKS));
/// The player and the tick of the last remote pressed. The game asks once per hand, so a click sets off one bomb at
/// most; a click on a block asks the block's use first, then the use in the air, in the same tick.
static LAST_USE: Mutex<Option<(u128, u64)>> = Mutex::new(None);

pub(crate) fn forget() {
    lock(&MIRRORS).clear();
    *lock(&SAME_TICK) = None;
    *lock(&LAST_USE) = None;
}

/// A bomb's one slot holds its detonator, out of every hopper's reach and hidden on its screen: a broken bomb drops
/// it as a chest drops its items. The bombs that take a keypad open its menu.
pub(crate) fn container(kind: rules::BombKind) -> ModContainer {
    ModContainer {
        slots: rules::BOMB_SLOTS,
        faces: Default::default(),
        inputs: vec![],
        outputs: vec![],
        menu: kind.takes(rules::Detonator::Keypad).then(|| rules::KEYPAD_MENU.to_owned()),
    }
}

const BOMB_KEY: &str = "bomb";

/// A bomb's state from its saved compound; its kind is its block's.
fn load_bomb(kind: rules::BombKind, nbt: &Compound) -> rules::Bomb {
    let mut bomb = match nbt.get(&BOMB_KEY.into()) {
        Some(Tag::ByteArray(bytes)) => serde_json::from_slice(bytes).unwrap_or_else(|_| rules::Bomb::new(kind)),
        _ => rules::Bomb::new(kind),
    };
    bomb.kind = kind;
    bomb
}

fn store_bomb(nbt: &mut Compound, bomb: &rules::Bomb) {
    if let Ok(bytes) = serde_json::to_vec(bomb) {
        nbt.insert(BOMB_KEY.into(), Tag::ByteArray(bytes));
    }
}

/// The bomb standing at `pos`, read from its block entity.
fn bomb_at(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> Option<rules::Bomb> {
    let kind = rules::BombKind::of_block(&block_at(host, level, pos))?;
    let data = host.block_entity(level, pos)?.data(host)?;
    Some(load_bomb(kind, &data))
}

/// Writes the bomb at `pos`'s state back into its block entity.
fn store_at(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], bomb: &rules::Bomb) -> bool {
    let Some(be) = host.block_entity(level, pos) else {
        return false;
    };
    let mut data = Compound::default();
    store_bomb(&mut data, bomb);
    be.merge_data(host, data).unwrap_or(false)
}

/// Item slot `slot` of the block entity at `pos`.
fn set_slot(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], slot: usize, stack: Option<ItemStack>) -> bool {
    let Some(be) = host.block_entity(level, pos) else {
        return false;
    };
    host.transaction(|host, tx| be.set_item_slot(host, tx, slot as u32, stack.clone()).map_err(drop)).unwrap_or(false)
}

/// A bomb gone off: its detonator with it.
pub(crate) fn clear_items(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) {
    for slot in 0..rules::BOMB_SLOTS {
        set_slot(host, level, pos, slot, None);
    }
}

/// The data slots of the block entity at `pos`: false when there is none.
fn set_data_at(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], data: &[i32]) -> bool {
    let Some(be) = host.block_entity(level, pos) else {
        return false;
    };
    if be.data_slots(host).as_deref() == Some(data) {
        return true;
    }
    be.set_data_slots(host, data.to_vec()).is_ok()
}

fn sound(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3], event: &str, volume: f32, pitch: f32) {
    core::play(host, dimension, event, middle(pos), SoundSource::Blocks, volume, pitch);
}

fn item_name(item: &str) -> Text {
    tr(&format!("item.{}", item.replace(':', ".")), vec![])
}

fn block_name(block: &str) -> Text {
    tr(&format!("block.{}", block.replace(':', ".")), vec![])
}

fn place(pos: [i32; 3]) -> Text {
    lit(format!("{} {} {}", pos[0], pos[1], pos[2]))
}

pub(crate) fn keypad_bomb(block: &str) -> bool {
    rules::BombKind::of_block(block).is_some_and(|k| k.takes(rules::Detonator::Keypad))
}

/// Whether `block` is one of a Sunbreaker's structure.
pub(crate) fn is_part(block: &str) -> bool {
    rules::SUNBREAKER_PARTS.contains(&block)
}

// ---- the right click -------------------------------------------------------------

/// A right click on a bomb, with the main hand's item: a remote is linked, a detonator goes in, flint and steel
/// lights the fuse, and anything else opens the keypad, if there is one. The furnace the bomb is built on never
/// opens. Sneaking with something else in hand lets it be used on the bomb (a block placed against it).
///
/// A block of a Sunbreaker's structure stands for its core, hidden inside: the click acts on the core, and the
/// core's keypad opens on the block clicked (`MIRRORS`). One with no core near opens nothing, and takes a block
/// placed against it only from a sneaking player, as the core does. Whether the block was a bomb's.
pub(crate) fn right_click(host: &mut dyn ServerHost, who: &Pl, pos: [i32; 3], canceled: &mut bool) -> bool {
    let Some(level) = who.player.level(host) else { return false };
    let block = block_at(host, level, pos);
    let core = if rules::BombKind::of_block(&block).is_some() {
        pos
    } else if is_part(&block) {
        match rules::core_near(pos, &|p| block_at(host, level, p)) {
            Some(core) => core,
            None => {
                *canceled = !(core::held(host, who).is_some() && core::sneaking(host, who));
                return true;
            }
        }
    } else {
        return false;
    };
    let Some((now, maxpower)) = core::clock(host) else {
        return true;
    };
    *canceled = true;
    {
        let mut last = lock(&SAME_TICK);
        if *last == Some((who.uuid, core, now)) {
            return true;
        }
        *last = Some((who.uuid, core, now));
    }
    let held = core::held(host, who);
    let held_id = held.as_ref().map(|s| s.item().id(host));
    // A click held down: what it does goes on (a block placed, the keypad), what it says is said once.
    let held_down = lock(&LAST_CLICK).heard((who.uuid, core, held_id.clone()), now).is_some();
    let clicked = pos;
    let pos = core;
    let Some(mut bomb) = bomb_at(host, level, pos) else {
        return true;
    };
    let dimension = who.dimension.as_str();
    // A remote is linked to a bomb with a receiver, sneaking or not.
    if held_id.as_deref() == Some(rules::REMOTE) {
        if held_down {
            return true;
        }
        match bomb.detonator {
            Some(rules::Detonator::Receiver) => link(host, who, pos, bomb.kind),
            _ => tell(host, who.uuid, tr("big_nukes.remote.no_receiver", vec![])),
        }
        return true;
    }
    if let Some(detonator) = held_id.as_deref().and_then(rules::Detonator::of_item) {
        if held_down {
            return true;
        }
        match bomb.install(detonator) {
            Ok(()) => {
                if install(host, level, who, pos, &bomb, detonator) && detonator == rules::Detonator::Receiver {
                    remember(host, dimension, pos, bomb.kind);
                }
            }
            Err(rules::Refusal::HasOne) => tell(host, who.uuid, tr("big_nukes.bomb.has_detonator", vec![])),
            Err(rules::Refusal::FuseOnly) => tell(host, who.uuid, tr("big_nukes.bomb.fuse_only", vec![])),
        }
        return true;
    }
    if held_id.as_deref() == Some(rules::FLINT_AND_STEEL) {
        if held_down {
            return true;
        }
        match bomb.detonator {
            Some(rules::Detonator::Fuse) if bomb.fires_at.is_none() => {
                if let Err(why) = may_go_off(host, level, pos, bomb.kind, maxpower) {
                    tell(host, who.uuid, world::refused(why));
                } else if bomb.light(now) {
                    store_at(host, level, pos, &bomb);
                    sound(host, dimension, pos, "minecraft:item.flintandsteel.use", 1.0, 1.0);
                    sound(host, dimension, pos, "minecraft:entity.tnt.primed", 1.0, 1.0);
                    if !who.creative() {
                        wear_flint(host, who);
                    }
                }
            }
            // A lit fuse isn't lit again.
            Some(rules::Detonator::Fuse) => {}
            Some(_) => tell(host, who.uuid, tr("big_nukes.bomb.not_fuse", vec![])),
            None => tell(host, who.uuid, tr("big_nukes.bomb.no_detonator", vec![])),
        }
        return true;
    }
    let keypad = bomb.detonator == Some(rules::Detonator::Keypad);
    if held.is_some() && core::sneaking(host, who) {
        // The game goes on: the item is used.
        *canceled = false;
        return true;
    }
    if keypad && (clicked == pos || mirror(host, level, dimension, pos, clicked, &bomb, now)) {
        // The game goes on: the block opens its keypad, the core's own or the one it shows on the block clicked.
        *canceled = false;
        return true;
    }
    if held_down {
        return true;
    }
    if bomb.kind == rules::BombKind::Sunbreaker {
        let line = structure_line(host, level, pos);
        tell(host, who.uuid, line);
    }
    let hint = match bomb.detonator {
        None => "big_nukes.bomb.no_detonator",
        Some(rules::Detonator::Fuse) => "big_nukes.bomb.light_fuse",
        Some(rules::Detonator::Receiver) => "big_nukes.bomb.receiver",
        Some(rules::Detonator::Keypad) => return true,
    };
    tell(host, who.uuid, tr(hint, vec![]));
    true
}

/// Whether the bomb at `pos` may go off now: its radius, or why not (a Sunbreaker's structure as it stands, the
/// operator's limit).
fn may_go_off(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3], kind: rules::BombKind, maxpower: u32) -> Result<u32, rules::Refused> {
    let radius = kind.power(pos, &|p| block_at(host, level, p)).map_err(rules::Refused::Structure)?;
    let level_of = kind.level(radius);
    if level_of > maxpower {
        return Err(rules::Refused::Maxpower { radius: level_of, maxpower });
    }
    Ok(radius)
}

/// What a Sunbreaker's structure is now: its layers and its radius.
fn structure_line(host: &dyn ServerHost, level: LevelRef, core: [i32; 3]) -> Text {
    let s = rules::read_structure(core, &|p| block_at(host, level, p));
    let n = |v: u32| lit(v);
    let layers = vec![n(s.shell), n(rules::SHELL_BLOCKS), n(s.reflector), n(rules::REFLECTOR_BLOCKS)];
    match s.radius() {
        Ok(radius) => {
            let mut args = layers;
            args.push(n(radius));
            tr("big_nukes.sunbreaker.state", args)
        }
        Err(_) => tr("big_nukes.sunbreaker.state.open", layers),
    }
}

/// One of `item` out of the player's inventory: whether there was one.
fn take_one(host: &mut dyn ServerHost, who: &Pl, item: &str) -> bool {
    let Some(stack) = core::stack_of(host, item) else { return false };
    let Some(items) = who.player.entity().capability(host, &ENTITY_ITEMS, ()) else {
        return false;
    };
    let resource = ItemResource::of(&stack);
    host.transaction(|host, tx| Ok::<u32, ()>(items.extract_any(host, tx, &resource, 1))).unwrap_or(0) == 1
}

/// The detonator goes in: out of the player's hand, unless they're creative, into the bomb's slot and its state.
/// Whether it went in.
fn install(host: &mut dyn ServerHost, level: LevelRef, who: &Pl, pos: [i32; 3], bomb: &rules::Bomb, detonator: rules::Detonator) -> bool {
    let creative = who.creative();
    if !creative && !take_one(host, who, detonator.item()) {
        return false;
    }
    let stack = core::stack_of(host, detonator.item());
    let put = set_slot(host, level, pos, rules::DETONATOR_SLOT, stack) && store_at(host, level, pos, bomb);
    if !put {
        if !creative && let Some(stack) = core::stack_of(host, detonator.item()) {
            let _ = who.player.give_stack(host, stack);
        }
        return false;
    }
    sound(host, &who.dimension, pos, "minecraft:block.iron_trapdoor.close", 1.0, 1.4);
    tell(host, who.uuid, tr("big_nukes.bomb.installed", vec![item_name(detonator.item())]));
    true
}

/// One use off the flint and steel in the player's main hand, or it breaks on its last. Written at once: the old
/// glue queued it for the hand's next inventory tick.
fn wear_flint(host: &mut dyn ServerHost, who: &Pl) {
    let Some(living) = host.living(who.player.entity()) else { return };
    let Some(mut stack) = living.hand_item(host, Hand::Main) else { return };
    if stack.item() != keys().flint_and_steel {
        return;
    }
    let left = match rules::worn(stack.damage()) {
        Some(damage) => {
            stack.set_damage(damage);
            Some(stack)
        }
        None => {
            core::play(host, &who.dimension, "minecraft:entity.item.break", who.position, SoundSource::Players, 0.8, 1.0);
            None
        }
    };
    if let Err(e) = living.set_hand_item(host, Hand::Main, left) {
        log::warn!("[big_nukes] wearing the flint and steel: {e:?}");
    }
}

// ---- the sunbreaker ---------------------------------------------------------------
//
// Its core is a bomb as the others are. The blocks of its structure have a block entity each, which never ticks:
// its one hidden slot and its keypad menu are there so the core's keypad opens on the block clicked. While a player
// may have it open, the core's tick copies its keypad's data slots to that block (`MIRRORS`), and the keys pressed
// there go to the core.

/// The blocks showing a core's keypad: the dimension, the core and the block, the newest last.
static MIRRORS: Mutex<Vec<(String, [i32; 3], [i32; 3])>> = Mutex::new(Vec::new());
/// How many are kept: a block whose screen is closed costs a copy a tick until it is pushed out.
const MIRRORS_KEPT: usize = 8;

/// Shows the core's keypad on the block `part`, before the game opens its menu: false when the block has no block
/// entity to show it.
fn mirror(host: &mut dyn ServerHost, level: LevelRef, dimension: &str, core: [i32; 3], part: [i32; 3], bomb: &rules::Bomb, now: u64) -> bool {
    let data = bomb.data(now).to_vec();
    if !set_data_at(host, level, part, &data) {
        return false;
    }
    let mut mirrors = lock(&MIRRORS);
    mirrors.retain(|m| m.0 != dimension || m.2 != part);
    mirrors.push((dimension.to_owned(), core, part));
    let over = mirrors.len().saturating_sub(MIRRORS_KEPT);
    mirrors.drain(..over);
    true
}

/// A core's tick: its keypad's data on the blocks that show it. One gone is let go.
fn mirror_data(host: &mut dyn ServerHost, level: LevelRef, dimension: &str, core: [i32; 3], data: &[i32]) {
    let parts: Vec<[i32; 3]> = lock(&MIRRORS).iter().filter(|m| m.0 == dimension && m.1 == core).map(|m| m.2).collect();
    for part in parts {
        let kept = is_part(&block_at(host, level, part)) && set_data_at(host, level, part, data);
        if !kept {
            lock(&MIRRORS).retain(|m| m.0 != dimension || m.2 != part);
        }
    }
}

/// A key pressed on a block's keypad: the core's, as `core_near` finds it.
pub(crate) fn part_button(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>, id: i32) {
    let Some(level) = host.level(host.dimension()) else { return };
    let Some(core) = rules::core_near(pos, &|p| block_at(host, level, p)) else {
        return;
    };
    let Some(mut bomb) = bomb_at(host, level, core) else {
        return;
    };
    let Some(data) = press_key(host, level, core, &mut bomb, id) else {
        return;
    };
    store_at(host, level, core, &bomb);
    set_data_at(host, level, core, &data);
    entity.data = data;
}

// ---- the bomb's tick ---------------------------------------------------------------

/// A bomb's tick: the hiss and smoke of a lit fuse, the beeps of a countdown, and the blast at its end.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    let Some(kind) = rules::BombKind::of_block(entity.id()) else { return };
    let Some((now, maxpower)) = core::clock(host) else { return };
    let dimension = host.dimension();
    let Some(level) = host.level(dimension) else { return };
    let mut bomb = load_bomb(kind, &entity.nbt);
    let was = bomb.clone();
    // A Sunbreaker's core reads its structure once a second, for a remote that fires it from afar.
    if bomb.kind == rules::BombKind::Sunbreaker && now % rules::SECOND == 0 {
        let power = bomb.kind.power(pos, &|p| block_at(host, level, p));
        if let Some(mut guard) = core::state(host) {
            let st = guard.as_mut().expect("opened");
            st.dirty |= st.saved.read_power(dimension, pos, power);
        }
    }
    if let Some(left) = bomb.left(now) {
        counting(host, level, dimension, pos, &bomb, left);
    }
    if bomb.tick(now) {
        // A Sunbreaker's radius is its structure's now.
        let start = match may_go_off(host, level, pos, bomb.kind, maxpower) {
            Ok(radius) => world::start_bomb(host, dimension, pos, bomb.kind, radius),
            Err(why) => Err(why),
        };
        match start {
            Ok(_) => {
                // The bomb goes with its blast, and its detonator with it.
                entity.items.iter_mut().for_each(|s| *s = None);
                core::set_air(host, level, pos);
                return;
            }
            Err(why) => {
                let within = if bomb.kind == rules::BombKind::Sunbreaker { SUNBREAKER_TOLD_WITHIN } else { 64.0 };
                world::tell_within(host, dimension, pos, &world::refused(why), within);
            }
        }
    }
    let data = bomb.data(now).to_vec();
    if bomb.kind == rules::BombKind::Sunbreaker {
        mirror_data(host, level, dimension, pos, &data);
    }
    if entity.data != data {
        entity.data = data;
    }
    if bomb != was {
        store_bomb(&mut entity.nbt, &bomb);
    }
}

/// What a counting bomb shows: a lit fuse smokes and sparks on top, a keypad beeps each second, higher as it ends.
fn counting(host: &mut dyn ServerHost, level: LevelRef, dimension: &str, pos: [i32; 3], bomb: &rules::Bomb, left: u64) {
    let sunbreaker = bomb.kind == rules::BombKind::Sunbreaker;
    match bomb.detonator {
        Some(rules::Detonator::Fuse) => {
            // A Sunbreaker's core smokes over its structure, two blocks up.
            let above = if sunbreaker { 3.05 } else { 1.05 };
            let top = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + above, f64::from(pos[2]) + 0.5];
            core::particles(host, level, &ParticleOptions::simple(keys().smoke), top, [0.1, 0.05, 0.1], 0.02, 2);
            if left % 2 == 0 {
                core::particles(host, level, &ParticleOptions::simple(keys().flame), top, [0.05, 0.02, 0.05], 0.01, 1);
            }
            if left % 20 == 0 {
                sound(host, dimension, pos, "minecraft:block.fire.extinguish", 0.3, 2.0);
            }
            // The Sunbreaker's last seconds: an alarm, two notes, heard far round it.
            if sunbreaker && left <= rules::ALARM_TICKS && left % 5 == 0 {
                let pitch = if left % 10 == 0 { 1.5 } else { 1.0 };
                sound(host, dimension, pos, "minecraft:block.note_block.bit", 4.0, pitch);
            }
        }
        Some(rules::Detonator::Keypad) if left % 20 == 0 => {
            let pitch = 1.0 + (rules::COUNTDOWN_TICKS.saturating_sub(left) as f32) / 200.0;
            sound(host, dimension, pos, "minecraft:block.note_block.pling", 1.0, pitch);
        }
        _ => {}
    }
}

/// A keypad button pressed on the server.
pub(crate) fn keypad_button(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>, id: i32) {
    let Some(kind) = rules::BombKind::of_block(entity.id()) else { return };
    let Some(level) = host.level(host.dimension()) else { return };
    let mut bomb = load_bomb(kind, &entity.nbt);
    if let Some(data) = press_key(host, level, pos, &mut bomb, id) {
        entity.data = data;
        store_bomb(&mut entity.nbt, &bomb);
    }
}

/// Button `id` of the keypad of the bomb at `pos`: the keypad's data slots after it.
fn press_key(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], bomb: &mut rules::Bomb, id: i32) -> Option<Vec<i32>> {
    let (Some(key), Some((now, maxpower))) = (rules::Key::of_button(id), core::clock(host)) else {
        return None;
    };
    let dimension = host.dimension();
    match bomb.press(key, now) {
        rules::Press::Accepted => {
            // The code was right, but the bomb may not go off: over the operator's limit, or a Sunbreaker without
            // its shell.
            if let Err(why) = may_go_off(host, level, pos, bomb.kind, maxpower) {
                bomb.fires_at = None;
                world::tell_near(host, dimension, pos, &world::refused(why));
            }
        }
        rules::Press::CodeSet => sound(host, dimension, pos, "minecraft:block.note_block.chime", 1.0, 1.0),
        rules::Press::Wrong | rules::Press::TooShort => sound(host, dimension, pos, "minecraft:block.note_block.bass", 1.0, 0.8),
        rules::Press::Locked => sound(host, dimension, pos, "minecraft:block.note_block.bass", 1.0, 0.5),
        rules::Press::Typed | rules::Press::Cleared | rules::Press::Ignored => {}
    }
    Some(bomb.data(now).to_vec())
}

// ---- the remote ------------------------------------------------------------------
//
// A right click on a bomb with a receiver links the remote in the main hand: its `big_nukes:link` component keeps
// the bomb's dimension and place. The world keeps each bomb with a receiver in `Saved.bombs`, so a right click with
// a linked remote sets the bomb off from anywhere, its chunk loaded or not.

/// The remote's link: the bomb's dimension and place.
pub(crate) struct Link;

impl Component for Link {
    type Value = Compound;
    fn id(&self) -> &str {
        rules::LINK
    }
    fn decode(&self, value: &Tag) -> Option<Compound> {
        match value {
            Tag::Compound(c) => Some(c.clone()),
            _ => None,
        }
    }
    fn encode(&self, value: Compound) -> Tag {
        Tag::Compound(value)
    }
}

/// A bomb with a receiver goes in the world's record, saved at once. False when it went off and only stands until
/// its wave reaches it.
fn remember(host: &dyn ServerHost, dimension: &str, pos: [i32; 3], kind: rules::BombKind) -> bool {
    let Some(mut guard) = core::state(host) else {
        return false;
    };
    let st = guard.as_mut().expect("opened");
    let recorded = st.saved.record(dimension, pos, kind);
    if recorded {
        core::save(st);
    }
    recorded
}

/// The bomb the remote is linked to, if it is linked.
pub(crate) fn link_of(stack: &ItemStack) -> Option<rules::Link> {
    let tag = stack.get(&Link)?;
    let Some(Tag::String(dimension)) = tag.get(&"dimension".into()) else {
        return None;
    };
    let Some(Tag::IntArray(pos)) = tag.get(&"pos".into()) else {
        return None;
    };
    rules::Link::of(&dimension.to_string_lossy(), <[i32; 3]>::try_from(pos.as_slice()).ok()?)
}

/// Links the remote in the player's main hand to the bomb at `pos`, recording the bomb again if its record was
/// lost. A link to another bomb is replaced. Written into the hand at once (the old glue queued it).
fn link(host: &mut dyn ServerHost, who: &Pl, pos: [i32; 3], kind: rules::BombKind) {
    if !remember(host, &who.dimension, pos, kind) {
        tell(host, who.uuid, tr("big_nukes.remote.spent", vec![]));
        return;
    }
    let Some(living) = host.living(who.player.entity()) else { return };
    let Some(mut stack) = living.hand_item(host, Hand::Main) else { return };
    if stack.item() != keys().remote {
        return;
    }
    let mut value = Compound::default();
    value.insert("dimension".into(), Tag::String(who.dimension.as_str().into()));
    value.insert("pos".into(), Tag::IntArray(pos.to_vec()));
    if let Err(e) = stack.set(&Link, value) {
        log::warn!("[big_nukes] linking a remote: {e:?}");
        return;
    }
    if let Err(e) = living.set_hand_item(host, Hand::Main, Some(stack)) {
        log::warn!("[big_nukes] linking a remote: {e:?}");
        return;
    }
    sound(host, &who.dimension, pos, "minecraft:block.note_block.pling", 0.8, 2.0);
    tell(host, who.uuid, tr("big_nukes.remote.linked", vec![block_name(kind.id()), place(pos)]));
}

/// Whether this click is the second of the same tick (a click on a block, then in the air): it is let go.
fn same_use(who: u128, now: u64) -> bool {
    let mut last = lock(&LAST_USE);
    if *last == Some((who, now)) {
        return true;
    }
    *last = Some((who, now));
    false
}

/// A right click on a block with a remote: the game asks the block's item use first, and only when nothing claims
/// it the use in the air. Claiming it keeps a click on the ground from setting a bomb off; the remote fires only
/// when aimed at nothing.
pub(crate) fn use_on(host: &mut dyn ServerHost, player: u128) {
    let (Some(who), Some((now, _))) = (core::by_uuid(host, player), core::clock(host)) else {
        return;
    };
    if same_use(who.uuid, now) {
        return;
    }
    tell(host, who.uuid, tr("big_nukes.remote.aim", vec![]));
}

/// A right click with a remote in the air: a linked remote sets its bomb off. A bomb in a loaded chunk of the
/// player's level is checked and taken away at once; any other goes with its wave's crater.
pub(crate) fn use_remote(host: &mut dyn ServerHost, player: u128, stack: &ItemStack) {
    let (Some(who), Some((now, _))) = (core::by_uuid(host, player), core::clock(host)) else {
        return;
    };
    if same_use(who.uuid, now) {
        return;
    }
    let Some(link) = link_of(stack) else {
        tell(host, who.uuid, tr("big_nukes.remote.not_linked", vec![]));
        return;
    };
    let (dimension, pos) = (link.dimension.as_str(), link.pos);
    let level = host.level(dimension);
    let here = dimension == who.dimension && world::loaded(dimension, pos);
    if here && let Some(level) = level {
        // Something the world doesn't report (a bulk edit, another mod) may have taken the bomb or its receiver.
        let receiver = bomb_at(host, level, pos).is_some_and(|b| b.detonator == Some(rules::Detonator::Receiver));
        if !receiver
            && let Some(mut guard) = core::state(host)
        {
            let st = guard.as_mut().expect("opened");
            if st.saved.forget(dimension, pos) {
                core::save(st);
            }
        }
    }
    // From afar a Sunbreaker goes off with the radius its core last read.
    let blocks = match (here, level) {
        (true, Some(level)) => structure_now(host, level, pos),
        _ => HashMap::new(),
    };
    let reader = read_from(&blocks);
    let started = world::start(host, dimension, |saved| {
        saved.fire_bomb(dimension, pos, here.then_some(&reader as &dyn Fn([i32; 3]) -> String))
    });
    let line = match started {
        Ok(_) => {
            if here && let Some(level) = level {
                // The bomb goes with its blast, and its detonator with it.
                world::take_bomb(host, level, pos);
            } else if dimension != who.dimension {
                world::wake(dimension);
            }
            core::play(host, &who.dimension, "minecraft:block.stone_button.click_on", who.position, SoundSource::Players, 0.6, 1.4);
            tr("big_nukes.remote.fired", vec![place(pos)])
        }
        Err(why) => world::refused(why),
    };
    tell(host, who.uuid, line);
}

/// The tooltip's lines: how to use the item, under its name, and the bomb a remote is linked to.
pub(crate) fn tooltip(host: &dyn ClientHost, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let id = stack.item().id(host);
    for key in rules::tips(&id) {
        lines.push((core::colored(tr(key, vec![]), "gray"), u32::MAX));
    }
    if stack.item() != keys().remote {
        return;
    }
    let line = match link_of(stack) {
        Some(link) => tr("big_nukes.remote.tooltip.linked", vec![place(link.pos), lit(link.dimension)]),
        None => tr("big_nukes.remote.tooltip.unlinked", vec![]),
    };
    lines.push((core::colored(line, "gray"), u32::MAX));
}

// ---- the keypad's screen ---------------------------------------------------------
//
// Drawn over the game's plain panel, as a chest's with six rows is laid out: the bomb's name, the digits typed as
// stars, a line of what the keypad waits for, and twelve buttons, then the inventory.

const PAD_HEIGHT: f32 = 222.0;
const PAD_INVENTORY_Y: f32 = 140.0;
const KEY_SIZE: [f32; 2] = [32.0, 16.0];
const KEY_GAP: f32 = 4.0;
/// The top left of the first key.
const KEYS_AT: [f32; 2] = [36.0, 48.0];
/// The menu buttons in their places, row by row: 1 to 9, then clear, 0, enter.
const KEY_ROWS: [[i32; 3]; 4] = [[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 0, 11]];

/// Each key's button and its box from the screen's top left.
fn key_boxes() -> impl Iterator<Item = (i32, [f32; 4])> {
    KEY_ROWS.into_iter().enumerate().flat_map(|(row, ids)| {
        ids.into_iter().enumerate().map(move |(col, id)| {
            let x = KEYS_AT[0] + col as f32 * (KEY_SIZE[0] + KEY_GAP);
            let y = KEYS_AT[1] + row as f32 * (KEY_SIZE[1] + KEY_GAP);
            (id, [x, y, KEY_SIZE[0], KEY_SIZE[1]])
        })
    })
}

fn inside(at: [f32; 2], [x, y, w, h]: [f32; 4]) -> bool {
    at[0] >= x && at[0] < x + w && at[1] >= y && at[1] < y + h
}

/// The keypad's line for its state, from its data slots.
fn pad_status(host: &dyn ClientHost, data: &[i32]) -> String {
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    let with = |key: &str, n: i32| core::lang(host, key).replacen("%s", &n.to_string(), 1);
    match get(0) {
        rules::PAD_COUNTING => with("big_nukes.screen.keypad.countdown", get(2)),
        rules::PAD_LOCKED => with("big_nukes.screen.keypad.locked", get(2)),
        rules::PAD_ENTER if get(3) < i32::from(rules::TRIES) => with("big_nukes.screen.keypad.wrong", get(3)),
        rules::PAD_ENTER => core::lang(host, "big_nukes.screen.keypad.enter"),
        rules::PAD_SET => core::lang(host, "big_nukes.screen.keypad.set"),
        _ => String::new(),
    }
}

pub(crate) fn keypad_screen() -> Screen {
    Screen {
        width: 176.0,
        height: PAD_HEIGHT,
        // The detonator's slot is hidden.
        slots: vec![None],
        inventory_y: PAD_INVENTORY_Y,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    }
}

pub(crate) fn keypad_draw(host: &dyn ClientHost, block: &str, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    let [x, y] = origin;
    let centered = |draw: &mut dyn DrawContext, text: &str, top: f32, rgb: u32, shadow: bool| {
        let width = draw.text_width(text);
        draw.text(text, [x + (176.0 - width) / 2.0, y + top], argb(rgb), shadow);
    };
    centered(draw, &core::lang(host, &format!("block.{}", block.replace(':', "."))), 6.0, LABEL, false);
    // The display: a star for each digit typed.
    draw.fill([x + 37.0, y + 17.0, 102.0, 14.0], 0xFF37_3737);
    draw.fill([x + 38.0, y + 18.0, 100.0, 12.0], 0xFF10_1810);
    let typed = data.get(1).copied().unwrap_or(0).clamp(0, rules::CODE_DIGITS as i32) as usize;
    centered(draw, &"*".repeat(typed), 21.0, 0x55_FF55, false);
    let state = data.first().copied();
    let status_color = match state {
        Some(rules::PAD_COUNTING | rules::PAD_LOCKED) => 0xAA_0000,
        _ => LABEL,
    };
    centered(draw, &pad_status(host, data), 35.0, status_color, false);
    // The keys, dimmed while the keypad takes nothing.
    let open = matches!(state, Some(rules::PAD_SET | rules::PAD_ENTER));
    let cursor = host.cursor();
    let at = [cursor[0] - x, cursor[1] - y];
    for (id, [kx, ky, w, h]) in key_boxes() {
        let lit = open && inside(at, [kx, ky, w, h]);
        let face = match (open, lit) {
            (false, _) => 0xFF6B_6B6B,
            (true, true) => 0xFFB8_B8D8,
            (true, false) => 0xFF9C_9C9C,
        };
        draw.fill([x + kx, y + ky, w, h], 0xFF2A_2A2A);
        draw.fill([x + kx + 1.0, y + ky + 1.0, w - 2.0, h - 2.0], face);
        let label = match id {
            10 => core::lang(host, "big_nukes.screen.keypad.clear"),
            11 => core::lang(host, "big_nukes.screen.keypad.ok"),
            digit => digit.to_string(),
        };
        let width = draw.text_width(&label);
        let rgb = if open { 0xFF_FFFF } else { 0xA0_A0A0 };
        draw.text(&label, [x + kx + (w - width) / 2.0, y + ky + 4.0], argb(rgb), true);
    }
    draw.text(&core::lang(host, "container.inventory"), [x + 8.0, y + PAD_INVENTORY_Y - 11.0], argb(LABEL), false);
}

/// A left press on a key of the keypad, sent to the server as its menu button.
pub(crate) fn keypad_click(at: [f32; 2], right: bool) -> Option<i32> {
    if right {
        return None;
    }
    key_boxes().find(|(_, r)| inside(at, *r)).map(|(id, _)| id)
}
