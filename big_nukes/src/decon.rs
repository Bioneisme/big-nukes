//! Iodine and the decontaminator (plan 9): the tablets and their effect, the decontaminator's block, fuel, circle
//! and its passes regrowing the grass.
//!
//! Stage 2 ports this module from `mixins/big_nukes_native/decon.rs`. `lib.rs` and the core already call every
//! function below; only their bodies change. The core's surface is `crate::core` (see `CORE_API.md`).
// TODO(stage 2): port from mixins/big_nukes_native/decon.rs, then drop these allows.
#![allow(dead_code, unused_variables)]

use crate::core::Pl;
use mod_api::{
    ApplyEffectTickEvent, Block, ClientHost, DrawContext, Host, LevelRef, MenuQuickMoveEvent, ModBlockEntityView,
    ModContainer, Screen, ServerHost, ShouldApplyEffectTickThisTickEvent,
};

// ---- declarations -----------------------------------------------------------------

/// The decontaminator, `block(id, template, hardness)` as the core's. (`rules::DECONTAMINATOR` has a block entity,
/// registered with the core's `Machine` part.)
pub(crate) fn blocks(blocks: &mut Vec<Block>, block: &dyn Fn(&str, &str, f32) -> Block) {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::blocks
}

/// The decontaminator's container (slots, faces, menu), read once at registration.
pub(crate) fn container() -> Option<ModContainer> {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::container
    None
}

// ---- the decontaminator -------------------------------------------------------------

/// Its server tick. `host.dimension()` is its level's.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::tick
}

/// The end of a level's tick: the passes regrowing the grass (bulk edits through `core::native`).
pub(crate) fn level_tick(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::level_tick
}

/// A shift-click in the decontaminator's menu, and the fuel generator's (bunker.rs). Whether it was handled.
pub(crate) fn quick_move(host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::quick_move
    false
}

pub(crate) fn screen() -> Option<Screen> {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::screen
    None
}

pub(crate) fn screen_draw(host: &dyn ClientHost, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::screen_draw
}

// ---- iodine -----------------------------------------------------------------------

/// A consume effect of the mod's own kind: a tablet's (`rules::IODINE_EFFECT`). Whether it was; the drinker is
/// `host.players().acting_player()`.
pub(crate) fn consume_effect(host: &mut dyn ServerHost, kind: &str) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::consume_effect
    false
}

/// The end of the server's tick, where commands run: the tablets eaten give their effect.
pub(crate) fn server_tick(host: &mut dyn ServerHost) {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::server_tick
}

/// Whether the iodine's effect is on `who` now, read in their tick (`rad::player_tick`) to shield their dose. The
/// API reads it straight (`crate::rad::has_effect(host, who, keys().iodine)`): the two effect hooks below can go.
pub(crate) fn iodine_on(host: &dyn ServerHost, who: &Pl) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::iodine_on
    false
}

/// Whether a content mod's effect acts this tick (`should_apply_effect_tick_this_tick`): the iodine's every tick.
pub(crate) fn effect_applies(host: &dyn Host, event: &mut ShouldApplyEffectTickThisTickEvent) {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::effect_applies (or leave empty: see iodine_on)
}

/// A content mod's effect acts (`apply_effect_tick`, a read-only host).
pub(crate) fn effect_tick(host: &dyn Host, event: &mut ApplyEffectTickEvent<'_>) {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::effect_tick (or leave empty: see iodine_on)
}

/// The world closing: what is queued goes.
pub(crate) fn forget() {
    // TODO(stage 2): port from mixins/big_nukes_native/decon.rs::forget
}
