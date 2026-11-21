//! The silo and the bomber (plan 7): the launch pad, the console, the strike beacon, the siren, the missiles'
//! flights and the bomber's drops, the launch camera and the notices.
//!
//! Stage 2 ports this module from `mixins/big_nukes_native/silo.rs`. `lib.rs` and the core already call every
//! function below; only their bodies change. The core's surface is `crate::core` (see `CORE_API.md`).
// TODO(stage 2): port from mixins/big_nukes_native/silo.rs, then drop these allows.
#![allow(dead_code, unused_variables)]

use crate::core::Pl;
use crate::rules;
use mod_api::{
    Block, CameraPoseEvent, ClientHost, DrawContext, GuiRenderPostEvent, Item, LevelRef, MenuQuickMoveEvent,
    ModBlockEntityView, ModContainer, PlayerRef, Screen, ScreenView, ServerHost,
};

// ---- declarations (called from `world::blocks`/`world::items` and at registration) ----

/// The silo's blocks, `block(id, template, hardness)` as the core's.
pub(crate) fn blocks(blocks: &mut Vec<Block>, block: &dyn Fn(&str, &str, f32) -> Block) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::blocks
}

/// The items that only show the flights (the missile's and the bomber's models).
pub(crate) fn model_items(items: &mut Vec<Item>) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::model_items
}

/// The silo's blocks with a block entity: registered with the core's `Machine` part.
pub(crate) fn has_entity(block: &str) -> bool {
    matches!(block, rules::LAUNCH_PAD | rules::LAUNCH_CONSOLE | rules::STRIKE_BEACON | rules::SIREN)
}

/// A block's container (slots, faces, menu), read once at registration.
pub(crate) fn container(block: &str) -> Option<ModContainer> {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::container
    None
}

// ---- the server -------------------------------------------------------------------

/// The end of a level's tick (`level_tick_post`, before the waves): flights, drops, displays.
pub(crate) fn level_tick(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::level_tick
}

/// A silo block entity's server tick (pad, console, beacon, siren). `host.dimension()` is its level's.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::tick
}

/// A notice for a player's window (`NOTICE_CHANNEL`): events.rs calls it too.
pub(crate) fn notify(host: &mut dyn ServerHost, player: u128, notice: &rules::Notice) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::notify
}

/// A right click on a block (server): whether it was the silo's. Set `canceled` to keep the game's own use (a menu)
/// from happening; leave it false to let the game go on (the console's and the pad's menus open).
pub(crate) fn right_click(host: &mut dyn ServerHost, who: &Pl, pos: [i32; 3], canceled: &mut bool) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::right_click (and beacon_click)
    false
}

/// A shift-click in the launch pad's menu: whether it was handled (the game's routing is then skipped).
pub(crate) fn pad_quick_move(host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::pad_quick_move
    false
}

/// A message from a player's window (`LAUNCH_CHANNEL`): a launch typed on a console.
pub(crate) fn net_from_client(host: &mut dyn ServerHost, player: PlayerRef, channel: &str, data: &[u8]) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::net_from_client
}

/// A beacon, a console or a pad taken away (any way: `block_changed` hears it, maybe while the state is locked).
pub(crate) fn taken(dimension: &str, pos: [i32; 3]) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::taken
}

/// The world closing: what waits for a tick goes.
pub(crate) fn forget() {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::forget
}

// ---- the window -------------------------------------------------------------------

/// A mod message in the window: `CONSOLE_CHANNEL` (the console's news) and `NOTICE_CHANNEL` (a notice).
pub(crate) fn net_from_server(host: &mut dyn ClientHost, channel: &str, data: &[u8]) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::net_from_server
}

/// Every frame (`render_frame_pre`): the launches the console's screen queued go to the server.
pub(crate) fn frame(host: &mut dyn ClientHost) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::frame
}

/// Over everything on the screen (`gui_render_post`): the notices the old glue showed as toasts. The API adds no
/// toast (`Hooks::toast_add` only filters them), so they are drawn here.
pub(crate) fn gui_render_post(host: &dyn ClientHost, event: &mut GuiRenderPostEvent<'_>) {
    // TODO(stage 2): draw the notices of mixins/big_nukes_native/silo.rs::net_from_server (native_add_toast)
}

/// A key over a screen: the console's Tab, Backspace and Enter, and no other key closes it. Whether it was taken.
pub(crate) fn key_pressed(host: &dyn ClientHost, screen: &ScreenView<'_>, key: &str) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::key_pressed
    false
}

/// A character typed at a screen: the console's fields take it. Whether it was taken.
pub(crate) fn char_typed(host: &dyn ClientHost, screen: &ScreenView<'_>, c: char) -> bool {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::char_typed
    false
}

/// The launch camera, for the one who launched.
pub(crate) fn camera_pose(host: &dyn ClientHost, event: &mut CameraPoseEvent<'_>) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::camera_pose
}

/// The window drops a level: the launch camera goes.
pub(crate) fn forget_camera() {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::forget_camera
}

pub(crate) fn pad_screen() -> Option<Screen> {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::pad_screen
    None
}

pub(crate) fn pad_draw(host: &dyn ClientHost, draw: &mut dyn DrawContext, origin: [f32; 2]) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::pad_draw
}

pub(crate) fn console_screen() -> Option<Screen> {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::console_screen
    None
}

/// A press on the console's screen over no slot (its keys and buttons are the window's own, no menu button).
pub(crate) fn console_click(at: [f32; 2], right: bool) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::console_click
}

pub(crate) fn console_draw(host: &dyn ClientHost, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    // TODO(stage 2): port from mixins/big_nukes_native/silo.rs::console_draw
}
