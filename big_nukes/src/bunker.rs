//! The bunker (plan 10): its walls (registered by chunk, cracked and broken by the waves), the hermetic door, the
//! fuel generator, the air filter's sealed rooms and the lamp, and each player's shelter.
//!
//! Stage 2 ports this module from `mixins/big_nukes_native/bunker.rs`. `lib.rs` and the core already call every
//! function below; only their bodies change. The core's surface is `crate::core` (see `CORE_API.md`).
// TODO(stage 2): port from mixins/big_nukes_native/bunker.rs, then drop these allows.
#![allow(dead_code, unused_variables)]

use crate::core::Pl;
use crate::rules;
use mod_api::{
    Block, ClientHost, DrawContext, LevelRef, MenuQuickMoveEvent, ModBlockEntityView, ModContainer, Screen,
    ServerHost,
};
use std::collections::BTreeMap;

// ---- declarations -----------------------------------------------------------------

pub(crate) fn blocks(blocks: &mut Vec<Block>) {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::blocks
}

/// The bunker's blocks with a block entity: registered with the core's `Machine` part.
pub(crate) fn has_entity(block: &str) -> bool {
    matches!(block, rules::FUEL_GENERATOR | rules::AIR_FILTER | rules::BUNKER_LAMP)
}

/// A block's container (slots, faces, menu), read once at registration.
pub(crate) fn container(block: &str) -> Option<ModContainer> {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::container
    None
}

// ---- the walls (the core's waves call these) -------------------------------------------

/// A block at `pos` changed from `was` to `now` (the mod's block ids, "" for any other block): a wall placed,
/// replaced or taken away is queued. Heard from `block_changed`, maybe while the state is locked: never lock it here.
pub(crate) fn block_changed(dimension: &str, pos: [i32; 3], was: &str, now: &str) {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::block_changed
}

/// The walls heard since the state last opened go into the registry (called from `core::state`). Whether it changed.
pub(crate) fn take_heard(saved: &mut rules::Saved) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::take_heard
    false
}

/// The walls a chunk's plan is given (`rules::walls_near`), corrected from the world as it stands; the corrections
/// go into `fixes`. Called inside a wave's work, with the state locked: read blocks only.
pub(crate) fn walls_for(
    host: &dyn ServerHost,
    level: LevelRef,
    walls: &BTreeMap<String, Vec<rules::Wall>>,
    dimension: &str,
    chunk: [i32; 2],
    fixes: &mut Vec<([i32; 3], Option<(rules::WallKind, bool)>)>,
) -> Vec<rules::Wall> {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::walls_for (the world's corrections)
    rules::walls_near(walls, dimension, chunk)
}

/// The corrections `walls_for` found, into the registry. Whether it changed.
pub(crate) fn fix_walls(saved: &mut rules::Saved, dimension: &str, fixes: Vec<([i32; 3], Option<(rules::WallKind, bool)>)>) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::fix_walls
    false
}

// ---- the machines -----------------------------------------------------------------

/// The start of the core's server tick (the old `server_tick()`, the machines' clock).
pub(crate) fn server_tick(host: &mut dyn ServerHost) {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::server_tick
}

/// A bunker block entity's server tick. `host.dimension()` is its level's.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::tick
}

/// A shift-click in a bunker block's menu (the generator's goes as the decontaminator's: `decon::quick_move`).
/// Whether it was handled.
pub(crate) fn quick_move(host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::quick_move
    false
}

/// Whether a generator burns near `pos`, heard lately: events.rs's radar asks it.
pub(crate) fn powered_now(dimension: &str, pos: [i32; 3]) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::powered_now
    false
}

// ---- the shelter ------------------------------------------------------------------

/// The share of the background where `who` stands (`rules::shelter`), kept for their window: 1 is none.
pub(crate) fn shelter(host: &dyn ServerHost, who: &Pl) -> f64 {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::shelter
    1.0
}

/// The shelter `player` had at their last second: none (1) for a player not reckoned.
pub(crate) fn shelter_of(player: u128) -> f64 {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::shelter_of
    1.0
}

/// A right click on the hermetic door: whether it was the door. Set `canceled` to keep the game's own use.
pub(crate) fn right_click(host: &mut dyn ServerHost, who: &Pl, pos: [i32; 3], canceled: &mut bool) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::right_click
    false
}

// ---- the screens ------------------------------------------------------------------

pub(crate) fn screen(block: &str) -> Option<Screen> {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::screen
    None
}

pub(crate) fn screen_draw(host: &dyn ClientHost, block: &str, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::screen_draw
}

/// The world closing: what waits for a tick goes.
pub(crate) fn forget() {
    // TODO(stage 2): port from mixins/big_nukes_native/bunker.rs::forget
}
