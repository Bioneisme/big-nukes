// Big Nukes in the game: the edges between the game's types and the rules
// (`game::big_nukes`, runtime.rs). Added to the game as `game::big_nukes_glue`;
// each small helper file calls it from a hook point.
//
// Everything here runs on the server. A wave's work is done at the end of
// each tick of its own dimension's level, within a time budget, as
// `native_edit_blocks` only edits the level being ticked.

use crate::game::{Game, NativeBlockEdit, NativeBlockRun, NativeFalling, NativeHeightmap, NativeShake, big_nukes as rules};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
    lang,
    mod_api::{NativeCommand, NativeScreen, NativeServerPlayer, ServerTickPhase, WorldPhase},
    text::{self, ClickEvent, Style, Text},
};
use horizon_core::geometry::Direction;
use horizon_mods::content::{Block, BlockProperties, Item, Rarity};
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

/// The time a level's tick may spend on waves.
const BUDGET: Duration = Duration::from_millis(6);
/// Ticks between saves while a wave runs.
const SAVE_EVERY: u64 = 200;

/// The open world's waves and where they are kept.
struct State {
    dir: PathBuf,
    saved: rules::Saved,
    /// Blasts whose refused edit is already in the log.
    warned: BTreeSet<u64>,
    /// Blasts whose opening (the fireball and the debris) has been shown.
    opened: BTreeSet<u64>,
    /// Something changed since the last save. A wave whose far chunks never load
    /// never ends, so saving only on changes keeps it from costing forever.
    dirty: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// The loaded chunks of each dimension, heard from the start of the world, before
/// its waves are read.
static LOADED: Mutex<Option<HashMap<String, BTreeSet<[i32; 2]>>>> = Mutex::new(None);

fn lock<T>(m: &'static Mutex<T>) -> MutexGuard<'static, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The open world's state, read from its folder the first time.
fn state(game: &Game<'_>) -> Option<MutexGuard<'static, Option<State>>> {
    let dir = game.native_world_dir()?.join(rules::DIR);
    let mut guard = lock(&STATE);
    if guard.as_ref().is_none_or(|s| s.dir != dir) {
        let saved = fs::read(dir.join("waves.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).map_err(|e| log::warn!("[big_nukes] waves.json: {e}")).ok())
            .unwrap_or_default();
        log::info!("[big_nukes] Big Nukes {} in {}", rules::VERSION, dir.display());
        *guard = Some(State { dir, saved, warned: BTreeSet::new(), opened: BTreeSet::new(), dirty: false });
    }
    Some(guard)
}

fn save(st: &mut State) {
    st.dirty = false;
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(&st.dir)?;
        let tmp = st.dir.join("waves.json.tmp");
        fs::write(&tmp, serde_json::to_vec(&st.saved)?)?;
        fs::rename(tmp, st.dir.join("waves.json"))
    };
    if let Err(e) = write() {
        log::warn!("[big_nukes] saving waves.json: {e}");
    }
}

// ---- declarations -------------------------------------------------------------------

/// The machines and the bombs. The centrifuge and the bombs are furnaces as a
/// template: they open a menu and face the player who placed them. The table is
/// a crafting table.
pub(in crate::game) fn blocks(blocks: &mut Vec<Block>) {
    let block = |id: &str, template: &str, hardness: f32| Block {
        id: id.into(),
        template: template.into(),
        properties: BlockProperties {
            hardness: Some(hardness),
            explosion_resistance: Some(6.0),
            requires_tool: Some(true),
            ..Default::default()
        },
        item: true,
        ..Default::default()
    };
    blocks.push(block(rules::CENTRIFUGE, "minecraft:furnace", 3.5));
    blocks.push(block(rules::ASSEMBLY_TABLE, "minecraft:crafting_table", 2.5));
    for kind in rules::BombKind::ALL {
        blocks.push(block(kind.id(), "minecraft:furnace", 2.0));
    }
}

pub(in crate::game) fn items(items: &mut Vec<Item>) {
    for (id, stack, rare) in rules::ITEMS {
        items.push(Item {
            id: id.into(),
            template: "minecraft:paper".into(),
            max_stack_size: Some(stack),
            rarity: Some(if rare { Rarity::Rare } else { Rarity::Common }),
            ..Default::default()
        });
    }
}

// ---- joining ---------------------------------------------------------------------

/// A player joins. The assets half, if it's there, reads the tag to know this
/// half is too (content/data/big_nukes/function/check.mcfunction). Without the
/// assets half, which brings the marker loot table, they hear what's missing and
/// where to get it, in plain text, as the language files are gone too.
pub(in crate::game) fn logged_in(game: &mut Game<'_>, player: u128, name: &str) {
    if !game.native_on(rules::ID) {
        return;
    }
    if world_items!(game).has_loot_table(rules::ASSETS_MARKER) {
        game.native_run_command(&format!("tag {name} add big_nukes.native"));
        // Someone joining mid-blast still hears what's on its way.
        let dimension = game.native_server_players().into_iter().find(|p| p.uuid == player).map(|p| p.dimension);
        let news = dimension
            .and_then(|d| state(game).and_then(|g| g.as_ref().map(|st| st.saved.waves.news(&d, st.saved.tick))))
            .unwrap_or_default();
        for n in &news {
            send(game, player, n);
        }
        return;
    }
    const ASSETS_PAGE: &str = "https://playhorizon.dev/mods/big_nukes";
    let link = Style {
        color: Some(text::AQUA),
        underlined: Some(true),
        click: Some(ClickEvent::OpenUrl(ASSETS_PAGE.to_owned())),
        ..Style::default()
    };
    for line in [
        "Big Nukes: the assets mod is missing. Install big_nukes too: ",
        "Big Nukes: не установлен мод с ассетами. Поставьте ещё big_nukes: ",
    ] {
        let line = Text::literal(line).append(Text::styled(ASSETS_PAGE, link.clone()));
        game.native_tell(player, line);
    }
}

// ---- the command -----------------------------------------------------------------

pub(in crate::game) fn commands(commands: &mut Vec<NativeCommand>) {
    // One text argument, last: the radius and the optional place are read from it.
    commands.push(NativeCommand::op("bignukes blast <args:text>", 2));
}

pub(in crate::game) fn perform(game: &mut Game<'_>, player: &NativeServerPlayer, line: &str, ran: &mut bool) {
    let words: Vec<&str> = line.split(' ').filter(|w| !w.is_empty()).collect();
    if *ran || words.first() != Some(&"bignukes") || !game.native_on(rules::ID) {
        return;
    }
    *ran = true;
    let reply = match words.get(1) {
        Some(&"blast") => match rules::parse_blast(&words[2..], player.position) {
            Some((radius, center)) => {
                let Some(mut guard) = state(game) else { return };
                let st = guard.as_mut().expect("opened");
                match st.saved.blast(&player.dimension, center, radius) {
                    Some(id) => {
                        save(st);
                        let news: Vec<rules::News> =
                            st.saved.waves.news(&player.dimension, st.saved.tick).into_iter().filter(|n| n.id == id).collect();
                        drop(guard);
                        tell_dimension(game, &player.dimension, &news);
                        Text::translate("big_nukes.blast.started", &[Text::literal(radius.to_string())])
                    }
                    None => Text::translate("big_nukes.blast.dimension", &[]),
                }
            }
            None => usage(),
        },
        _ => usage(),
    };
    game.native_feedback(reply);
}

fn usage() -> Text {
    let n = |v: u32| Text::literal(v.to_string());
    Text::translate("big_nukes.blast.usage", &[n(rules::MIN_RADIUS), n(rules::MAX_RADIUS)])
}

// ---- crafting ----------------------------------------------------------------------

/// A crafting grid's result for a server player, before it is shown. The bombs and
/// their parts come out only of a grid at `pos` whose block, in the player's level,
/// is the assembly table; the inventory grid (`pos` None) and other tables give
/// nothing for them.
pub(in crate::game) fn prepare_item_craft(game: &mut Game<'_>, player: u128, pos: Option<[i32; 3]>, result: &mut Option<ItemStack>) {
    if !result.as_ref().is_some_and(|s| rules::needs_assembly_table(&s.item)) || !game.native_on(rules::ID) {
        return;
    }
    let on_table = pos.is_some_and(|pos| {
        let dimension = game.native_server_players().into_iter().find(|p| p.uuid == player).map(|p| p.dimension);
        dimension
            .and_then(|d| game.native_level(&d))
            .and_then(|level| game.native_level_block(level, pos))
            .is_some_and(|block| block == rules::ASSEMBLY_TABLE)
    });
    if !on_table {
        *result = None;
    }
}

// ---- the blast's effects, on the server ---------------------------------------------

fn send(game: &mut Game<'_>, player: u128, news: &rules::News) {
    let Ok(bytes) = serde_json::to_vec(news) else { return };
    if let Err(e) = game.native_net_send_to_player(player, rules::NEWS_CHANNEL, &bytes) {
        log::warn!("[big_nukes] telling a player of blast {}: {e}", news.id);
    }
}

/// Every player in `dimension` hears of the blasts.
fn tell_dimension(game: &mut Game<'_>, dimension: &str, news: &[rules::News]) {
    for p in game.native_server_players().into_iter().filter(|p| p.dimension == dimension) {
        for n in news {
            send(game, p.uuid, n);
        }
    }
}

/// A command in the blast's dimension. Its answer isn't an error: false is also
/// a damage that found no one or smoke that no player is near enough to see.
fn run_in(game: &mut Game<'_>, blast: &rules::Blast, command: &str) {
    game.native_run_command(&format!("execute in {} run {command}", blast.dimension));
}

fn particle(id: &str, at: [f64; 3], spread: [f64; 3], speed: f64, count: u32) -> String {
    format!(
        "particle {id} {:.1} {:.1} {:.1} {:.1} {:.1} {:.1} {speed:.2} {count} force",
        at[0], at[1], at[2], spread[0], spread[1], spread[2]
    )
}

/// Smoke on the front and over the center, the opening's fireball and debris,
/// and the damage of the ground the front crossed this tick.
fn effects(game: &mut Game<'_>, st: &mut State, dimension: &str) {
    for (blast, from, to, age) in st.saved.waves.fronts(dimension, st.saved.tick) {
        let [cx, cy, cz] = blast.center.map(|v| v as f64 + 0.5);
        if age < 20 && st.opened.insert(blast.id) {
            run_in(game, &blast, &particle("minecraft:explosion_emitter", [cx, cy + 2.0, cz], [2.0, 2.0, 2.0], 0.0, 6));
            let registry = &game.pack.assets.registry;
            let thrown: Vec<_> = rules::debris(&blast)
                .into_iter()
                .filter_map(|(pos, v, block)| Some((pos, v, registry.default_state(block)?)))
                .collect();
            for (pos, v, state) in thrown {
                game.native_spawn_falling_block(&blast.dimension, state, pos, v, NativeFalling { drop_item: false, hurts: true });
            }
        }
        if age % 4 == 0 {
            for at in rules::front_ring(&blast, age) {
                run_in(game, &blast, &particle("minecraft:campfire_signal_smoke", at, [2.0, 1.0, 2.0], 0.02, 4));
                run_in(game, &blast, &particle("minecraft:large_smoke", at, [3.0, 0.5, 3.0], 0.05, 6));
            }
            if to < 512.0 {
                for h in [10.0, 20.0, 30.0, 40.0] {
                    run_in(game, &blast, &particle("minecraft:campfire_signal_smoke", [cx, cy + h, cz], [3.0, 2.0, 3.0], 0.05, 8));
                }
            }
        }
        for (a, b, amount) in rules::damage_band(blast.radius, from, to) {
            run_in(
                game,
                &blast,
                &format!(
                    "execute as @e[x={cx:.1},y={cy:.1},z={cz:.1},distance={a:.1}..{b:.1}] run damage @s {amount:.1} minecraft:explosion at {cx:.1} {cy:.1} {cz:.1}"
                ),
            );
        }
    }
}

// ---- the blast's effects, in the window ---------------------------------------------
//
// The window hears of a blast once and works out the rest from its own clock:
// the sounds are queued at once with their delays, the flash and the shake are
// drawn from the time since the blast and the camera's distance to it.

static HEARD: Mutex<Vec<rules::Heard>> = Mutex::new(Vec::new());

pub(in crate::game) fn heard(game: &mut Game<'_>, channel: &str, data: &[u8]) {
    if channel != rules::NEWS_CHANNEL || !game.native_on(rules::ID) {
        return;
    }
    let (Ok(news), Some(player), Some(clock)) =
        (serde_json::from_slice::<rules::News>(data), game.native_player(), game.native_clock())
    else {
        return;
    };
    let me = [player.x, player.y, player.z];
    let toward = news.center.map(|v| v as f64 + 0.5);
    let far = (0..3).map(|i| (toward[i] - me[i]).powi(2)).sum::<f64>().sqrt();
    log::info!("[big_nukes] blast {} heard at {far:.0} blocks", news.id);
    // From a little toward the blast, so it is heard from where it is.
    let step = if far > 6.0 { 6.0 / far } else { 1.0 };
    let at = [0, 1, 2].map(|i| me[i] + (toward[i] - me[i]) * step);
    for s in rules::sounds_for(far, news.radius, news.age) {
        game.native_play_local_sound(s.event, at, s.volume, 1.0, s.delay);
    }
    let mut heard = lock(&HEARD);
    heard.retain(|h| h.news.id != news.id);
    heard.push(rules::Heard { at: clock.game_time as f64 - news.age as f64, dimension: player.dimension.to_owned(), news });
}

/// Each heard blast still felt, with the ticks since it went off and the
/// player's distance to it. `partial` is the part of the tick gone, where the
/// hook knows it. The player's eyes stand for the camera, which mods can't read
/// while the GUI draws.
fn felt(game: &Game<'_>, partial: f64) -> Vec<(f64, f64, rules::News)> {
    let (Some(clock), Some(player)) = (game.native_clock(), game.native_player()) else {
        return vec![];
    };
    let eyes = [player.x, player.y + 1.62, player.z];
    let now = clock.game_time as f64 + partial;
    let mut heard = lock(&HEARD);
    heard.retain(|h| h.felt(player.dimension, eyes, now).is_some() || h.dimension != player.dimension);
    heard.iter().filter_map(|h| h.felt(player.dimension, eyes, now).map(|(t, d)| (t, d, h.news.clone()))).collect()
}

pub(in crate::game) fn flash(game: &Game<'_>, batch: &mut HudBatch) {
    let alpha: u32 = felt(game, 0.0).iter().map(|(t, d, n)| rules::flash_alpha(*t, *d, n.radius) as u32).sum::<u32>().min(255);
    if alpha > 0 {
        game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], alpha << 24 | 0x00FF_FFFF);
    }
}

/// Every drawn frame, whatever the screen (death, inventory, chat): the renderer's
/// list of blasts, newest first.
pub(in crate::game) fn frame(game: &mut Game<'_>) {
    let partial = game.native_camera().map_or(0.0, |c| c.partial_tick as f64);
    let mut felt = felt(game, partial);
    rules::newest_first(&mut felt);
    *horizon_client::gfx::native_render::big_nukes_sky::SKY.lock().unwrap_or_else(|e| e.into_inner()) = felt
        .iter()
        .map(|(t, _, n)| {
            let c = rules::cloud(*t, n.radius);
            horizon_client::gfx::native_render::big_nukes_sky::SkyBlast {
                center: [n.center[0] as f64 + 0.5, n.center[1] as f64, n.center[2] as f64 + 0.5],
                seed: n.seed as u32,
                age: *t as f32,
                cloud: [c.ball_h, c.ball_r, c.stem_r, c.cap_major, c.cap_minor, c.fire, c.fade, c.ring, c.ring_fade],
            }
        })
        .collect();
}

pub(in crate::game) fn shake(game: &Game<'_>, partial: f32, shake: &mut NativeShake) {
    let felt = felt(game, partial as f64);
    for (t, d, n) in felt {
        let [pitch, yaw, roll, x, y, z] = rules::shake(t, d, n.radius, n.seed);
        shake.pitch += pitch;
        shake.yaw += yaw;
        shake.roll += roll;
        shake.offset[0] += x;
        shake.offset[1] += y;
        shake.offset[2] += z;
    }
}

// ---- ticks, chunks and the world's phases ----------------------------------------

pub(in crate::game) fn server_tick(game: &mut Game<'_>, phase: ServerTickPhase) {
    if !matches!(phase, ServerTickPhase::ServerTickEnd { world_running: true }) || !game.native_on(rules::ID) {
        return;
    }
    let Some(mut guard) = state(game) else { return };
    let st = guard.as_mut().expect("opened");
    st.saved.tick += 1;
    if st.dirty && st.saved.tick % SAVE_EVERY == 0 {
        save(st);
    }
}

/// The end of a level's tick: its waves move on, and some of their work is done.
pub(in crate::game) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    if !game.native_on(rules::ID) {
        return;
    }
    let Some(mut guard) = state(game) else { return };
    let st = guard.as_mut().expect("opened");
    effects(game, st, dimension);
    if st.saved.waves.idle(dimension) {
        return;
    }
    let loaded = lock(&LOADED).as_ref().and_then(|m| m.get(dimension)).cloned().unwrap_or_default();
    st.dirty |= st.saved.waves.advance(dimension, st.saved.tick, &|c| loaded.contains(&c));
    let started = Instant::now();
    let warned = &mut st.warned;
    let done = st.saved.waves.work(dimension, BUDGET, &|| started.elapsed(), &mut |blast, chunk| {
        if !loaded.contains(&chunk) {
            return false;
        }
        for edit in rules::plan_chunk(blast, chunk) {
            if edit.runs.is_empty() {
                continue;
            }
            if let Err(e) = game.native_edit_blocks(&blast.dimension, &native(edit))
                && warned.insert(blast.id)
            {
                log::warn!("[big_nukes] blast {}: an edit was refused: {e}", blast.id);
            }
        }
        true
    });
    st.dirty |= done > 0;
    if st.saved.waves.idle(dimension) {
        log::info!("[big_nukes] {dimension}: the loaded part of every wave is done at tick {}", st.saved.tick);
    }
    if st.saved.waves.finished() {
        log::info!("[big_nukes] every wave is over");
        save(st);
    }
}

fn native(edit: rules::ChunkEdit) -> NativeBlockEdit {
    let surface = |s: rules::Surface| match s {
        rules::Surface::WorldSurface => NativeHeightmap::WorldSurface,
        rules::Surface::MotionBlockingNoLeaves => NativeHeightmap::MotionBlockingNoLeaves,
    };
    NativeBlockEdit {
        palette: edit.palette,
        filters: edit.filters,
        runs: edit
            .runs
            .into_iter()
            .map(|r| NativeBlockRun {
                x: r.x,
                z: r.z,
                y: r.y,
                height: r.height,
                surface: r.surface.map(surface),
                block: r.block,
                only: r.only,
            })
            .collect(),
    }
}

pub(in crate::game) fn chunk_load(game: &mut Game<'_>, dimension: &str, chunk: [i32; 2]) {
    lock(&LOADED).get_or_insert_default().entry(dimension.to_owned()).or_default().insert(chunk);
    if let Some(mut guard) = state(game) {
        let st = guard.as_mut().expect("opened");
        st.dirty |= st.saved.waves.chunk_loaded(dimension, chunk);
    }
}

pub(in crate::game) fn chunk_unload(dimension: &str, chunk: [i32; 2]) {
    if let Some(m) = lock(&LOADED).as_mut()
        && let Some(set) = m.get_mut(dimension)
    {
        set.remove(&chunk);
    }
}

/// The world closing: the waves are written and let go.
pub(in crate::game) fn lifecycle(phase: WorldPhase) {
    if !matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed) {
        return;
    }
    let mut state = lock(&STATE);
    if let Some(st) = state.as_mut() {
        save(st);
    }
    *state = None;
    *lock(&LOADED) = None;
    lock(&HEARD).clear();
}

// ---- the centrifuge --------------------------------------------------------------
//
// Its state lives in its block entity, so it moves with the chunk's save: `nbt`
// holds the rules' `Centrifuge` as JSON, `items` the three slots, `data` what its
// screen shows.

pub(in crate::game) fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == rules::CENTRIFUGE && entity.is_none() {
        *entity = Some((rules::CENTRIFUGE.to_owned(), Compound::default()));
    }
}

pub(in crate::game) fn block_entity_ticks(block: &str, ticks: &mut bool) {
    if block == rules::CENTRIFUGE {
        *ticks = true;
    }
}

/// A furnace's sides: hoppers put raw uranium in from above and fuel from the
/// sides, and take enriched uranium from below.
pub(in crate::game) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == rules::CENTRIFUGE {
        let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| vec![rules::FUEL_SLOT]);
        faces[Direction::Down as usize] = vec![rules::OUTPUT_SLOT];
        faces[Direction::Up as usize] = vec![rules::INPUT_SLOT];
        *container = Some(ModContainer {
            slots: rules::CENTRIFUGE_SLOTS,
            faces,
            inputs: vec![rules::INPUT_SLOT, rules::FUEL_SLOT],
            outputs: vec![rules::OUTPUT_SLOT],
            menu: Some(rules::CENTRIFUGE.to_owned()),
        });
    }
}

/// What a hopper may put in: raw uranium in its slot, fuel in the other.
pub(in crate::game) fn accepts(game: &Game<'_>, entity: &ModBlockEntity, slot: usize, stack: &ItemStack, cancel: &mut bool) {
    if entity.id != rules::CENTRIFUGE || *cancel {
        return;
    }
    let raw = &*stack.item == rules::RAW_URANIUM;
    *cancel = match slot {
        rules::INPUT_SLOT => !raw,
        rules::FUEL_SLOT => raw || burn_time(game, stack) <= 0,
        _ => true,
    };
}

fn key_name() -> JavaString {
    JavaString::from("centrifuge")
}

fn load(entity: &ModBlockEntity) -> Option<rules::Centrifuge> {
    match entity.nbt.get(&key_name()) {
        Some(Tag::ByteArray(bytes)) => serde_json::from_slice(bytes).ok(),
        _ => None,
    }
}

fn store(entity: &mut ModBlockEntity, centrifuge: &rules::Centrifuge) {
    if let Ok(bytes) = serde_json::to_vec(centrifuge) {
        entity.nbt.insert(key_name(), Tag::ByteArray(bytes));
    }
}

// --- internals ---
// The game's own table, which no hook point offers: it may change with a new
// Horizon revision, so it stays here. The same burn table as the quarry's.

fn burn_time(game: &Game<'_>, stack: &ItemStack) -> i32 {
    game.res.providers().burn_time(world_items!(game), stack, false)
}
// --- end of internals ---

fn stack_of(item: &str) -> ItemStack {
    ItemStack { item: item.into(), count: 1, damage: 0, extra: None }
}

/// One fuel out of its slot; a lava bucket leaves its bucket.
fn take_fuel(entity: &mut ModBlockEntity) {
    let Some(stack) = entity.items[rules::FUEL_SLOT].as_mut() else {
        return;
    };
    let lava = &*stack.item == "minecraft:lava_bucket";
    stack.count -= 1;
    if stack.count == 0 {
        entity.items[rules::FUEL_SLOT] = lava.then(|| stack_of("minecraft:bucket"));
    }
}

/// One raw uranium out, one enriched uranium in.
fn make(entity: &mut ModBlockEntity) {
    if let Some(input) = entity.items[rules::INPUT_SLOT].as_mut() {
        input.count -= 1;
        if input.count == 0 {
            entity.items[rules::INPUT_SLOT] = None;
        }
    }
    match entity.items[rules::OUTPUT_SLOT].as_mut() {
        Some(out) => out.count += 1,
        None => entity.items[rules::OUTPUT_SLOT] = Some(stack_of(rules::ENRICHED_URANIUM)),
    }
}

pub(in crate::game) fn tick(game: &mut Game<'_>, entity: &mut ModBlockEntity) {
    if entity.id != rules::CENTRIFUGE || !game.native_on(rules::ID) {
        return;
    }
    if entity.items.len() < rules::CENTRIFUGE_SLOTS {
        entity.items.resize(rules::CENTRIFUGE_SLOTS, None);
    }
    let was = load(entity);
    let mut centrifuge = was.unwrap_or_default();
    let input = entity.items[rules::INPUT_SLOT].as_ref().filter(|s| &*s.item == rules::RAW_URANIUM).map_or(0, |s| s.count);
    let fuel = entity.items[rules::FUEL_SLOT]
        .as_ref()
        .filter(|s| &*s.item != rules::RAW_URANIUM)
        .map(|s| burn_time(game, s))
        .filter(|t| *t > 0)
        .map(i32::unsigned_abs);
    let most = game.native_max_stack_size(rules::ENRICHED_URANIUM).unwrap_or(64);
    let room = match &entity.items[rules::OUTPUT_SLOT] {
        None => true,
        Some(s) => &*s.item == rules::ENRICHED_URANIUM && s.count < most,
    };
    let step = centrifuge.step(input, fuel, room);
    if step.took_fuel {
        take_fuel(entity);
    }
    if step.made {
        make(entity);
    }
    let data = centrifuge.data().to_vec();
    if entity.data != data {
        entity.data = data;
    }
    if was != Some(centrifuge) {
        store(entity, &centrifuge);
    }
}

/// A shift-click from the inventory: raw uranium to its slot, fuel to its slot,
/// nothing else. One from the centrifuge goes the game's way, to the inventory.
pub(in crate::game) fn quick_move(game: &Game<'_>, entity: &mut ModBlockEntity, container_slots: usize, slot: usize, stack: &mut ItemStack, handled: &mut bool) {
    if entity.id != rules::CENTRIFUGE || slot < container_slots {
        return;
    }
    *handled = true;
    if entity.items.len() < rules::CENTRIFUGE_SLOTS {
        entity.items.resize(rules::CENTRIFUGE_SLOTS, None);
    }
    let to = if &*stack.item == rules::RAW_URANIUM {
        rules::INPUT_SLOT
    } else if burn_time(game, stack) > 0 {
        rules::FUEL_SLOT
    } else {
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

// ---- the centrifuge's screen -----------------------------------------------------

/// Where the screen's parts are, from its top left: tools/make_textures.py draws
/// them in the same places. Slots are the items' corners.
const INPUT_AT: [f32; 2] = [56.0, 17.0];
const FUEL_AT: [f32; 2] = [56.0, 53.0];
const OUTPUT_AT: [f32; 2] = [116.0, 35.0];
const FLAME_AT: [f32; 2] = [57.0, 37.0];
const ARROW_AT: [f32; 2] = [79.0, 34.0];
/// The lit flame and the full arrow, right of the panel in the texture.
const LIT_FLAME: [f32; 2] = [176.0, 0.0];
const FULL_ARROW: [f32; 2] = [176.0, 14.0];
const ARROW_HEIGHT: f32 = 17.0;
const GUI: &str = "big_nukes:gui/centrifuge";

pub(in crate::game) fn screen(block: &str, screen: &mut Option<NativeScreen>) {
    if block == rules::CENTRIFUGE {
        *screen = Some(NativeScreen {
            width: 176.0,
            height: 166.0,
            slots: vec![Some(INPUT_AT), Some(FUEL_AT), Some(OUTPUT_AT)],
            inventory_y: 84.0,
            inventory_x: 8.0,
            own_labels: true,
            own_background: true,
        });
    }
}

pub(in crate::game) fn screen_draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    if block != rules::CENTRIFUGE {
        return;
    }
    let [x, y] = origin;
    game.native_sprite(batch, GUI, [0.0, 0.0, 176.0, 166.0], [x, y, 176.0, 166.0]);
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    // The flame burns down from its top, the arrow fills from its left.
    let flame = rules::flame_height(get(0), get(1)) as f32;
    if flame > 0.0 {
        let skip = rules::FLAME_HEIGHT as f32 - flame;
        game.native_sprite(
            batch,
            GUI,
            [LIT_FLAME[0], LIT_FLAME[1] + skip, rules::FLAME_HEIGHT as f32, flame],
            [x + FLAME_AT[0], y + FLAME_AT[1] + skip, rules::FLAME_HEIGHT as f32, flame],
        );
    }
    let arrow = rules::arrow_width(get(2), get(3)) as f32;
    if arrow > 0.0 {
        game.native_sprite(
            batch,
            GUI,
            [FULL_ARROW[0], FULL_ARROW[1], arrow, ARROW_HEIGHT],
            [x + ARROW_AT[0], y + ARROW_AT[1], arrow, ARROW_HEIGHT],
        );
    }
    const LABEL: u32 = 0x40_4040;
    let title = lang::get("big_nukes.screen.centrifuge");
    let width = game.native_text_width(&title);
    game.native_text(batch, &title, [x + (176.0 - width) / 2.0, y + 6.0], LABEL, false);
    game.native_text(batch, &lang::get("container.inventory"), [x + 8.0, y + 72.0], LABEL, false);
}
