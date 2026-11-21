//! What the core shares: the helpers every module uses (players, text, blocks, sounds, chunk tickets, the bulk
//! edits) and, re-exported, the world's state and the blasts. The stage-2 modules (`silo`, `bunker`, `decon`,
//! `events`) take what they need from here: see `CORE_API.md`.

use crate::{keys, rules};
use mod_api::{
    BlockEdit, BlockKey, BlockRun, BlockUpdateFlags, ClientHost, GameMode, Heightmap, Host, HostBlockStates, Identifier, ItemKey,
    ItemStack, LevelRef, ParticleOptions, PayloadBytes, PayloadId, PlayerRef, ServerHost, SoundSource,
    StateConstraint, StatePredicate, Text, TicketOwner, text::TextStyle,
};
use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

// The stage-2 modules' share of the machines and the world (CORE_API.md).
#[allow(unused_imports)] // TODO(stage 2): used by silo, bunker, decon and events once ported.
pub(crate) use crate::machines::{burn_time, stacks_with};
#[allow(unused_imports)] // TODO(stage 2): used by silo, bunker, decon and events once ported.
pub(crate) use crate::world::{
    State, ash_here, clock, loaded, refused, save, start_blast, start_bomb, state, tell_near, tell_within,
};

/// A static's lock, taken back from a panic that poisoned it.
pub(crate) fn lock<T>(m: &'static Mutex<T>) -> MutexGuard<'static, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A block's middle.
pub(crate) fn middle(pos: [i32; 3]) -> [f64; 3] {
    pos.map(|v| f64::from(v) + 0.5)
}

pub(crate) fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

// ---- players -------------------------------------------------------------------

/// A player on the server, read once: what the old glue's `NativeServerPlayer` held.
#[derive(Clone, Debug)]
pub(crate) struct Pl {
    pub player: PlayerRef,
    pub uuid: u128,
    pub name: String,
    pub dimension: String,
    pub position: [f64; 3],
    pub game_mode: GameMode,
    pub health: f32,
}

impl Pl {
    /// Creative and spectating players take no dose and no blow.
    pub fn exempt(&self) -> bool {
        matches!(self.game_mode, GameMode::Creative | GameMode::Spectator)
    }

    pub fn creative(&self) -> bool {
        self.game_mode == GameMode::Creative
    }
}

/// `namespace:path`.
pub(crate) fn identifier(id: &Identifier) -> String {
    format!("{}:{}", id.namespace(), id.path())
}

pub(crate) fn pl(host: &dyn ServerHost, player: PlayerRef) -> Option<Pl> {
    let entity = player.entity();
    Some(Pl {
        player,
        uuid: player.uuid(),
        name: player.name(host)?,
        dimension: identifier(&player.dimension(host)?),
        position: entity.position(host)?,
        game_mode: player.game_mode(host).unwrap_or(GameMode::Survival),
        health: host.living(entity).and_then(|l| l.health(host)).unwrap_or(0.0),
    })
}

/// Every player on the server.
pub(crate) fn players(host: &dyn ServerHost) -> Vec<Pl> {
    host.players().server_players().into_iter().filter_map(|p| pl(host, p)).collect()
}

/// The player whose action the game is running (a click, a command, their own tick).
#[allow(dead_code)] // TODO(stage 2): the old modules' `native_acting_player`.
pub(crate) fn acting(host: &dyn ServerHost) -> Option<Pl> {
    pl(host, host.players().acting_player()?)
}

pub(crate) fn by_uuid(host: &dyn ServerHost, uuid: u128) -> Option<Pl> {
    pl(host, host.player(uuid)?)
}

/// What the player holds in their main hand.
pub(crate) fn held(host: &dyn ServerHost, who: &Pl) -> Option<ItemStack> {
    host.living(who.player.entity())?.hand_item(host, mod_api::Hand::Main)
}

/// Whether the player sneaks.
pub(crate) fn sneaking(host: &dyn ServerHost, who: &Pl) -> bool {
    who.player.entity().is_shift_key_down(host).unwrap_or(false)
}

// ---- text ----------------------------------------------------------------------

/// A language line with its arguments, read in each player's own language.
pub(crate) fn tr(key: &str, args: Vec<Text>) -> Text {
    Text::translate(key, args)
}

pub(crate) fn lit(s: impl ToString) -> Text {
    Text::literal(s.to_string())
}

/// `text` in a named colour (`gray`, `red`...).
pub(crate) fn colored(text: Text, color: &str) -> Text {
    let mut style = TextStyle::default();
    style.color = Some(color.to_owned());
    text.with_style(style)
}

/// A line in a player's chat.
pub(crate) fn tell(host: &mut dyn ServerHost, uuid: u128, line: Text) {
    if let Some(player) = host.player(uuid) {
        let _ = player.tell(host, line);
    }
}

pub(crate) fn tell_everyone(host: &mut dyn ServerHost, line: &Text) {
    for p in host.players().server_players() {
        let _ = p.tell(host, line.clone());
    }
}

/// A mod message to a player's window.
pub(crate) fn send(host: &mut dyn ServerHost, uuid: u128, channel: &str, bytes: Vec<u8>) -> Result<(), String> {
    let player = host.player(uuid).ok_or_else(|| "no such player".to_owned())?;
    player
        .send_payload(host, &PayloadId(channel.into()), PayloadBytes(bytes))
        .map(drop)
        .map_err(|e| format!("{e:?}"))
}

/// A console command. Its answer isn't an error: false is also a damage that found no one.
pub(crate) fn run(host: &mut dyn ServerHost, command: &str) -> bool {
    host.commands().run_command(command)
}

/// A language line in the window's language: the key itself when it has none.
pub(crate) fn lang(host: &dyn ClientHost, key: &str) -> String {
    host.translate(key).unwrap_or_else(|| key.to_owned())
}

pub(crate) fn lang_has(host: &dyn ClientHost, key: &str) -> bool {
    host.translate(key).is_some()
}

/// A language line with its `%s` filled in, in the window's language.
pub(crate) fn lang_tr(host: &dyn ClientHost, key: &str, args: &[&str]) -> String {
    let text = Text::translate(key, args.iter().map(|a| Text::literal((*a).to_owned())).collect());
    host.text_string(&text).unwrap_or_else(|_| key.to_owned())
}

/// An opaque colour for the draw calls, which take ARGB, from the old glue's RGB.
pub(crate) fn argb(rgb: u32) -> u32 {
    0xFF00_0000 | rgb
}

// ---- blocks and items ----------------------------------------------------------

/// A block's id; `minecraft:air` where nothing is read (an unloaded block).
pub(crate) fn block_at(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> String {
    level.block(host, pos).map_or_else(|| "minecraft:air".to_owned(), |b| b.id(host))
}

/// The mod's own block of `key`, by id, as the hooks without a host to ask read it.
pub(crate) fn mod_block(key: BlockKey) -> Option<&'static str> {
    keys().names.get(&key).map(String::as_str)
}

pub(crate) fn item_key(host: &dyn Host, id: &str) -> Option<ItemKey> {
    host.item_key(&Identifier::parse(id).ok()?)
}

/// One of `id`, or none when the game has no such item.
pub(crate) fn stack_of(host: &dyn Host, id: &str) -> Option<ItemStack> {
    item_key(host, id).map(ItemStack::of)
}

/// `count` of `id`.
pub(crate) fn stack_of_n(host: &dyn Host, id: &str, count: u32) -> Option<ItemStack> {
    let mut stack = stack_of(host, id)?;
    let _ = stack.set_count(count);
    Some(stack)
}

/// Sets the block at `pos` to air, telling its neighbours and the clients (the old flags 3).
pub(crate) fn set_air(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) {
    if let Some(air) = host.block_state_default(keys().air) {
        let _ = level.set_block(host, pos, air, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
    }
}

/// A sound at `at` in `dimension`, for whoever is near.
pub(crate) fn play(host: &mut dyn ServerHost, dimension: &str, event: &str, at: [f64; 3], source: SoundSource, volume: f32, pitch: f32) {
    if let Some(level) = host.level(dimension) {
        let _ = level.play_sound(host, event, at, source, volume, pitch);
    }
}

/// Particles at `at`, for the players near enough to see them (32 blocks: the long-range ones are commands).
pub(crate) fn particles(host: &mut dyn ServerHost, level: LevelRef, particle: &ParticleOptions, at: [f64; 3], spread: [f64; 3], speed: f64, count: u32) {
    let _ = level.send_particles(host, particle, None, at, spread, speed, i32::try_from(count).unwrap_or(i32::MAX));
}

// ---- chunk tickets -------------------------------------------------------------

/// Holds (or lets go of) chunk `chunk` of `dimension` under the mod's ticket controller, owned by its own corner
/// so each chunk lets go apart: the old `native_force_chunk_in(rules::TICKET_OWNER, ...)`. False where the level
/// isn't open (the Nether and the End open when someone goes there: `world::server_tick` opens them with a
/// command and tries again).
pub(crate) fn force_chunk(host: &mut dyn ServerHost, dimension: &str, chunk: [i32; 2], hold: bool) -> bool {
    let Some(level) = host.level(dimension) else {
        return false;
    };
    let Some(chunk_ref) = level.chunk_ref(host, chunk) else {
        return false;
    };
    chunk_ref.set_mod_forced(host, keys().tickets, ticket_owner(chunk), hold, false)
}

/// The owner of a chunk's ticket: its corner at y 0.
pub(crate) fn ticket_owner(chunk: [i32; 2]) -> TicketOwner {
    TicketOwner::Block([chunk[0] * 16, 0, chunk[1] * 16])
}

// ---- the bulk edits ------------------------------------------------------------

/// A palette entry or a filter read once: the rules name them as strings, the game wants its own states.
static STATES: Mutex<Option<HashMap<String, mod_api::BlockState>>> = Mutex::new(None);
static FILTERS: Mutex<Option<HashMap<String, StatePredicate>>> = Mutex::new(None);

/// The caches go with the world (a datapack reload may change the tags).
pub(crate) fn forget_edits() {
    *lock(&STATES) = None;
    *lock(&FILTERS) = None;
}

/// `ns:id[a=b,c=d]` as the id and its properties.
fn split_state(s: &str) -> (&str, Vec<(&str, &str)>) {
    match s.split_once('[') {
        Some((id, rest)) => {
            let props = rest
                .trim_end_matches(']')
                .split(',')
                .filter_map(|p| p.split_once('='))
                .map(|(k, v)| (k.trim(), v.trim()))
                .collect();
            (id, props)
        }
        None => (s, Vec::new()),
    }
}

fn palette_state(host: &dyn ServerHost, s: &str) -> Result<mod_api::BlockState, String> {
    if let Some(found) = lock(&STATES).as_ref().and_then(|m| m.get(s)) {
        return Ok(*found);
    }
    let (id, props) = split_state(s);
    let key = host.block_key(id).ok_or_else(|| format!("no block {id}"))?;
    let mut state = host.block_state_default(key).ok_or_else(|| format!("no state of {id}"))?;
    let states = host.block_states();
    for (k, v) in props {
        state = state.with(&states, k, v).map_err(|e| format!("{s}: {e:?}"))?;
    }
    lock(&STATES).get_or_insert_default().insert(s.to_owned(), state);
    Ok(state)
}

fn filter(host: &dyn ServerHost, s: &str) -> Result<StatePredicate, String> {
    if let Some(found) = lock(&FILTERS).as_ref().and_then(|m| m.get(s)) {
        return Ok(found.clone());
    }
    let states = host.block_states();
    let predicate = match s.strip_prefix('#') {
        Some(tag) => {
            let id = Identifier::parse(tag).map_err(|e| format!("{s}: {e:?}"))?;
            let key = host.tags().key(&id).ok_or_else(|| format!("no tag {s}"))?;
            StatePredicate::tag(&states, host.tags(), key, &[]).map_err(|e| format!("{s}: {e:?}"))?
        }
        None => {
            let (id, props) = split_state(s);
            let block = host.block_key(id).ok_or_else(|| format!("no block {id}"))?;
            let constraints: Vec<StateConstraint> = props.iter().map(|(k, v)| StateConstraint::new(*k, *v)).collect();
            StatePredicate::block(&states, block, &constraints).map_err(|e| format!("{s}: {e:?}"))?
        }
    };
    lock(&FILTERS).get_or_insert_default().insert(s.to_owned(), predicate.clone());
    Ok(predicate)
}

/// The rules' edit as the game's (the old `native(ChunkEdit) -> NativeBlockEdit`): its palette and filters read
/// as the game's states and predicates. An entry the game doesn't know refuses the whole edit.
pub(crate) fn native(host: &dyn ServerHost, edit: rules::ChunkEdit) -> Result<BlockEdit, String> {
    let surface = |s: rules::Surface| match s {
        rules::Surface::WorldSurface => Heightmap::WorldSurface,
        rules::Surface::MotionBlockingNoLeaves => Heightmap::MotionBlockingNoLeaves,
    };
    let palette = edit.palette.iter().map(|s| palette_state(host, s)).collect::<Result<Vec<_>, _>>()?;
    let filters = edit.filters.iter().map(|s| filter(host, s)).collect::<Result<Vec<_>, _>>()?;
    let runs = edit
        .runs
        .into_iter()
        .map(|r| BlockRun { x: r.x, z: r.z, y: r.y, height: r.height, surface: r.surface.map(surface), block: r.block, only: r.only })
        .collect();
    Ok(BlockEdit { palette, filters, runs })
}
