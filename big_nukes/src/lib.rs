//! Big Nukes in the game: the edges between the game (`mod_api`) and the rules (`rules.rs`).
//!
//! Every hook the mod listens to is here, each handed to its module: the core (`world` the state, the blasts and
//! the waves; `rad` the radiation; `client` the window's effects; `machines` the block entities; `bombs` the bombs
//! and the remote; `guide`; `sky` what the renderer draws) and the four stage-2 modules (`silo`, `bunker`, `decon`,
//! `events`), which take what they share from `core` (`CORE_API.md`).
//!
//! The content (textures, models, sounds, language files, loot tables, recipes, menus, damage types, mob effects)
//! ships in this package's `assets/` and `data/`.

mod bombs;
mod bunker;
mod client;
mod core;
mod decon;
mod events;
mod guide;
mod machines;
mod rad;
// TODO(stage 2): many rules are only used by the stage-2 modules, still stubs; drop this allow once they are ported.
#[allow(dead_code)]
mod rules;
mod silo;
pub mod sky;
mod world;

use mod_api::{
    BlockChangedEvent, BlockKey, BlocksEvent, CameraPoseEvent, CameraShakeEvent, ChunkLoadEvent,
    ChunkUnloadEvent, ClientHost, ClientLevelUnloadEvent, ClientTickPostEvent, ComponentTypesEvent, ConsumeEffectEvent,
    EffectKey, Flow, GuiLayerPostEvent, GuiRenderPostEvent, Hooks, HostSide, IPayloadContext, IPayloadHandler,
    InteractionResult, ItemKey, ItemPart, ItemTooltipEvent, ItemsEvent, LevelHost, LevelTickPostEvent, ParticleKey,
    PayloadBytes, PayloadHandleError, PayloadId, PlayerDiedEvent, PlayerLoggedInEvent, PlayerTickPostEvent,
    RegisterCommandsEvent, Registrar, RegistrationContext, RenderFogEvent, RenderFramePreEvent, RightClickBlockEvent,
    RunModCommandEvent, ScreenCharTypedPreEvent, ScreenKeyPressedPreEvent, ScreenMouseButtonPressedPreEvent,
    ScreenMouseScrolledPreEvent, ScreenRenderPostEvent, ServerHost, ServerStoppingEvent, ServerTickEvent,
    TicketController, UseItemEvent, UseOnEvent, ValidateTicketsEvent,
    WorldLifecycleEvent,
};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

/// The registry keys the mod asks about, looked up once at registration.
pub(crate) struct Keys {
    pub air: BlockKey,
    pub ash: BlockKey,
    /// The mod's blocks by key, for the hooks that can't ask a host for a block's id (`block_changed`).
    pub names: HashMap<BlockKey, String>,
    pub geiger: ItemKey,
    pub remote: ItemKey,
    pub flint_and_steel: ItemKey,
    pub bucket: ItemKey,
    pub lava_bucket: ItemKey,
    /// The suit's pieces, head first (`rules::HAZMAT`).
    pub hazmat: [ItemKey; 4],
    /// What gives off radiation when carried, and its id (`rules::CARRIED`).
    pub carried: Vec<(ItemKey, &'static str)>,
    #[allow(dead_code)] // TODO(stage 2): decon::iodine_on reads it.
    pub iodine: EffectKey,
    pub smoke: ParticleKey,
    pub flame: ParticleKey,
    /// The flights' and the satellites' chunks (`core::force_chunk`).
    pub tickets: TicketController,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

pub(crate) fn keys() -> &'static Keys {
    KEYS.get().expect("the mod's keys are looked up at registration")
}

/// The blocks the mod declares in `data/big_nukes/horizon/block/`, not in code.
const DATA_BLOCKS: [&str; 4] = ["big_nukes:uranium_ore", "big_nukes:deepslate_uranium_ore", rules::ASH, rules::TRINITITE];

fn flow(taken: bool) -> Flow {
    if taken { Flow::Cancel } else { Flow::Continue }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        // Full ids everywhere: a bare name would take the mod's id (big_nukes_native) as its namespace, and saved
        // worlds keep `big_nukes:...`.
        let blocks = world::blocks();
        let names = blocks
            .iter()
            .map(|b| b.id.clone())
            .chain(DATA_BLOCKS.map(str::to_owned))
            .map(|id| (r.block_key(&id), id))
            .collect();
        machines::register(r, &blocks);
        let remote = r.item_key(rules::REMOTE);
        r.item_part(remote, Remote);
        let guide = r.item_key(rules::GUIDE);
        r.item_part(guide, Guide);
        let tickets = r.register_ticket_controller(rules::TICKET_OWNER, true);
        let _ = KEYS.set(Keys {
            air: r.block_key("minecraft:air"),
            ash: r.block_key(rules::ASH),
            names,
            geiger: r.item_key(rules::GEIGER_COUNTER),
            remote,
            flint_and_steel: r.item_key(rules::FLINT_AND_STEEL),
            bucket: r.item_key("minecraft:bucket"),
            lava_bucket: r.item_key("minecraft:lava_bucket"),
            hazmat: rules::HAZMAT.map(|(id, _, _)| r.item_key(id)),
            carried: rules::CARRIED.iter().map(|(id, _)| (r.item_key(id), *id)).collect(),
            iodine: r.effect_key(rules::IODINE_EFFECT),
            smoke: r.particle_key("minecraft:smoke"),
            flame: r.particle_key("minecraft:flame"),
            tickets,
        });
        // The mod's messages: the news of a blast, the radiation, the guide to open, the console's news and the
        // notices to the window; a launch typed on a console to the server.
        let mut payloads = r.payload_handlers();
        let mut view = payloads.registrar("1");
        for channel in [rules::NEWS_CHANNEL, rules::RAD_CHANNEL, rules::GUIDE_CHANNEL, rules::CONSOLE_CHANNEL, rules::NOTICE_CHANNEL] {
            view.play_to_client(PayloadId(channel.into()), Some(Arc::new(ToClient(channel))));
        }
        view.play_to_server(PayloadId(rules::LAUNCH_CHANNEL.into()), Arc::new(ToServer(rules::LAUNCH_CHANNEL)));
    }

    // ---- declarations -------------------------------------------------------------

    fn blocks(&self, _: &RegistrationContext, event: &mut BlocksEvent<'_>) {
        event.blocks.extend(world::blocks());
    }

    fn items(&self, _: &RegistrationContext, event: &mut ItemsEvent<'_>) {
        world::items(event.items);
    }

    /// The remote's link, registered before the content loads.
    fn component_types(&self, _: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        if !event.types.iter().any(|t| t == rules::LINK) {
            event.types.push(rules::LINK.to_owned());
        }
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        world::commands(event.commands);
    }

    fn run_mod_command(&self, host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
        world::command(host, event)
    }

    // ---- the server -------------------------------------------------------------------

    /// A right click on a block: the silo's, the bunker's door, the radar, or a bomb (a detonator goes in, a fuse
    /// is lit, the keypad opens).
    fn right_click_block(&self, host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        let Some(who) = core::by_uuid(host, event.click.player) else {
            return Flow::Continue;
        };
        let pos = event.click.pos;
        let mut canceled = false;
        let _ = silo::right_click(host, &who, pos, &mut canceled)
            || bunker::right_click(host, &who, pos, &mut canceled)
            || events::right_click(host, &who, pos, &mut canceled)
            || bombs::right_click(host, &who, pos, &mut canceled);
        if canceled {
            *event.cancellation_result = InteractionResult::Success;
        }
        flow(canceled)
    }

    /// Any block changed: a bomb, a wall or a silo block gone is queued for the state (which may be locked).
    fn block_changed(&self, _: &dyn LevelHost, event: &mut BlockChangedEvent) {
        world::block_changed(event);
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        world::logged_in(host, event.player, event.name);
    }

    fn server_stopping(&self, host: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        world::server_stopping(host);
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, mod_api::ServerTickPhase::ServerTickEnd { world_running: true }) {
            world::server_tick(host);
        }
    }

    /// The world closing: the waves are written and let go.
    fn world_lifecycle(&self, _: &mut dyn ServerHost, event: &mut WorldLifecycleEvent<'_>) {
        if matches!(event.phase, mod_api::WorldPhase::WorldCloseFinal | mod_api::WorldPhase::WorldCloseFailed) {
            world::close_world();
        }
    }

    /// The end of a living player's server tick: their dose takes the background where they stand.
    fn player_tick_post(&self, host: HostSide<'_>, event: &mut PlayerTickPostEvent) {
        if let HostSide::Server(host) = host {
            rad::player_tick(host, event.player);
        }
    }

    fn player_died(&self, _: &mut dyn ServerHost, event: &mut PlayerDiedEvent<'_>) {
        rad::player_died(event.player);
    }

    /// The end of a level's tick: its mobs in the zones are hurt, and the waves of its dimension move on.
    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        if let HostSide::Server(host) = host {
            world::level_tick(host, event.level);
        }
    }

    fn chunk_load(&self, _: &mut dyn ServerHost, event: &mut ChunkLoadEvent<'_>) {
        world::chunk_load(event.dimension, event.chunk);
    }

    fn chunk_unload(&self, _: &mut dyn ServerHost, event: &mut ChunkUnloadEvent<'_>) {
        world::chunk_unload(event.dimension, event.chunk);
    }

    /// The world's saved chunk tickets, as a level loads: the flights' and the satellites' stay while they hold.
    fn validate_tickets(&self, host: &mut dyn ServerHost, event: &mut ValidateTicketsEvent<'_>) {
        if event.controller == keys().tickets {
            world::validate_tickets(host, event);
        }
    }

    /// A consumable's effect of the mod's own kind: the anti-rad's and the iodine tablets'.
    fn consume_effect(&self, host: &mut dyn ServerHost, event: &mut ConsumeEffectEvent<'_>) {
        rad::consume_effect(host, event);
    }

    // ---- the window -------------------------------------------------------------------

    /// An item's tooltip: how to use it, and the bomb a remote is linked to.
    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        bombs::tooltip(host, event.stack, event.lines);
    }

    /// The window's tick: the Geiger counter's clicks.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        client::client_tick(host);
    }

    /// The window drops a level (another dimension, or the world left): what it was told of the radiation goes, and
    /// the launch camera.
    fn client_level_unload(&self, _: &mut dyn ClientHost, _: &mut ClientLevelUnloadEvent<'_>) {
        client::forget_rad();
        silo::forget_camera();
    }

    /// Every frame, on any screen: the renderer's list of blasts and the ears' ringing, the guide's page turns heard
    /// and the console's launches sent.
    fn render_frame_pre(&self, host: &mut dyn ClientHost, _: &mut RenderFramePreEvent) {
        client::frame(host);
        guide::frame(host);
        silo::frame(host);
    }

    /// After each of the HUD's layers: the Geiger counter's reading over the hotbar.
    fn gui_layer_post(&self, host: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
        client::hud(host, event);
    }

    /// Over everything on the screen, even under F1: the flash, and the silo's notices (the old toasts).
    fn gui_render_post(&self, host: &dyn ClientHost, event: &mut GuiRenderPostEvent<'_>) {
        client::flash(host, event);
        silo::gui_render_post(host, event);
    }

    /// A zone's fog.
    fn render_fog(&self, host: &dyn ClientHost, event: &mut RenderFogEvent<'_>) {
        client::fog(host, event);
    }

    /// The ground shaking as the front passes.
    fn camera_shake(&self, host: &dyn ClientHost, event: &mut CameraShakeEvent<'_>) {
        client::shake(host, event);
    }

    /// The launch camera, for the one who launched.
    fn camera_pose(&self, host: &dyn ClientHost, event: &mut CameraPoseEvent<'_>) {
        silo::camera_pose(host, event);
    }

    /// The guide's screen, over the world.
    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        guide::render(host, event);
    }

    /// A mouse button over the guide: a bookmark or an arrow.
    fn screen_mouse_button_pressed_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseButtonPressedPreEvent<'_>) -> Flow {
        flow(guide::mouse(host, event.screen, event.button))
    }

    /// The wheel over the guide turns its pages.
    fn screen_mouse_scrolled_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseScrolledPreEvent<'_>) -> Flow {
        flow(guide::wheel(host, event.screen, event.scroll))
    }

    /// A key over a screen: the guide's page turns; the launch console's Tab, Backspace and Enter.
    fn screen_key_pressed_pre(&self, host: &dyn ClientHost, event: &mut ScreenKeyPressedPreEvent<'_>) -> Flow {
        let guide = guide::key(host, event.screen, event.key);
        let console = silo::key_pressed(host, event.screen, event.key);
        flow(guide || console)
    }

    /// A character typed at a screen: the launch console's fields take it.
    fn screen_char_typed_pre(&self, host: &dyn ClientHost, event: &mut ScreenCharTypedPreEvent<'_>) -> Flow {
        flow(silo::char_typed(host, event.screen, event.c))
    }
}

/// The remote: a right click in the air sets its bomb off; one on a block (not a bomb, which takes it first) only
/// says to aim at nothing.
struct Remote;

#[mod_api::horizon_part]
impl ItemPart for Remote {
    fn use_item(&self, host: HostSide<'_>, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        bombs::use_remote(host, event.player, event.stack);
        Some(InteractionResult::Success)
    }

    /// Claimed on both sides, so the window doesn't go on to the use in the air.
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        if let HostSide::Server(host) = host {
            bombs::use_on(host, event.player);
        }
        Some(InteractionResult::Fail)
    }
}

/// The guide: a right click asks the player's window to open it. Claimed even when the message can't go, so the
/// guide never acts as a plain book.
struct Guide;

#[mod_api::horizon_part]
impl ItemPart for Guide {
    fn use_item(&self, host: HostSide<'_>, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        if let HostSide::Server(host) = host {
            guide::read_guide(host, event.player);
        }
        Some(InteractionResult::Success)
    }
}

/// A mod message in the window.
struct ToClient(&'static str);

impl IPayloadHandler for ToClient {
    fn handle(&self, payload: PayloadBytes, context: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        let host = context.client_host().map_err(|e| PayloadHandleError::Failed(format!("{e:?}")))?;
        match self.0 {
            rules::NEWS_CHANNEL => client::heard(host, &payload.0),
            rules::RAD_CHANNEL => client::told(host, &payload.0),
            rules::GUIDE_CHANNEL => guide::asked(host),
            channel => silo::net_from_server(host, channel, &payload.0),
        }
        Ok(())
    }
}

/// A mod message from a player's window, checked there as any player's input.
struct ToServer(&'static str);

impl IPayloadHandler for ToServer {
    fn handle(&self, payload: PayloadBytes, context: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        let player = context.player().map_err(|e| PayloadHandleError::Failed(format!("{e:?}")))?;
        let host = context.server_host().map_err(|e| PayloadHandleError::Failed(format!("{e:?}")))?;
        silo::net_from_client(host, player, self.0, &payload.0);
        Ok(())
    }
}
