//! The radar and the events (plan 13): the storms, the mutants, the satellites, their notices and the radar.
//!
//! Stage 2 ports this module from `mixins/big_nukes_native/events.rs`. `lib.rs` and the core already call every
//! function below; only their bodies change. The core's surface is `crate::core` (see `CORE_API.md`).
// TODO(stage 2): port from mixins/big_nukes_native/events.rs, then drop these allows.
#![allow(dead_code, unused_variables)]

use crate::core::{self, Pl};
use crate::rules;
use mod_api::{Block, LevelRef, ServerHost, Text};

/// The radar and the satellite's debris, `block(id, template, hardness)` as the core's.
pub(crate) fn blocks(blocks: &mut Vec<Block>, block: &dyn Fn(&str, &str, f32) -> Block) {
    // TODO(stage 2): port from mixins/big_nukes_native/events.rs::blocks
}

/// The events' blocks with a block entity: registered with the core's `Machine` part (no container).
pub(crate) fn has_entity(block: &str) -> bool {
    block == rules::RADAR
}

/// A radar's server tick (it is heard, powered or not). `host.dimension()` is its level's.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: [i32; 3]) {
    // TODO(stage 2): port from mixins/big_nukes_native/events.rs::tick
}

/// A right click on the radar: whether it was. Set `canceled` to keep the game's own use.
pub(crate) fn right_click(host: &mut dyn ServerHost, who: &Pl, pos: [i32; 3], canceled: &mut bool) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/events.rs::right_click
    false
}

/// `/bignukes events ...`, its words after `events`: the reply.
pub(crate) fn command(host: &mut dyn ServerHost, args: &[&str]) -> Text {
    // TODO(stage 2): port from mixins/big_nukes_native/events.rs::command
    core::lit("")
}

/// The end of the server's tick (after the core's): the events' clock, notices, storm, satellite.
pub(crate) fn server_tick(host: &mut dyn ServerHost) {
    // TODO(stage 2): port from mixins/big_nukes_native/events.rs::server_tick
}

/// The end of a level's tick (before the waves): the mutants.
pub(crate) fn level_tick(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    // TODO(stage 2): port from mixins/big_nukes_native/events.rs::level_tick
}

/// How many mutants were within reach of `player` at their last second, at most a second before `now`. Asked with
/// the state locked: never lock it here.
pub(crate) fn mutants_near(player: u128, now: u64) -> usize {
    // TODO(stage 2): port from mixins/big_nukes_native/events.rs::mutants_near
    0
}
