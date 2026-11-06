// Big Nukes in the game: the edges between the game's types and the rules
// (`game::big_nukes`, runtime.rs). Added to the game as `game::big_nukes_glue`;
// each small helper file calls it from a hook point.
//
// Everything here runs on the server. A wave's work is done at the end of
// each tick of its own dimension's level, within a time budget, as
// `native_edit_blocks` only edits the level being ticked.

use crate::game::{Game, NativeBlockEdit, NativeBlockRun, NativeFalling, NativeHeightmap, NativeShake, big_nukes as rules};
use horizon_client::{
    hud::HudBatch,
    mod_api::{NativeCommand, NativeServerPlayer, ServerTickPhase, WorldPhase},
    text::{self, ClickEvent, Style, Text},
};
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
    commands.push(NativeCommand::op("bignukes blast <radius:text>", 2));
    commands.push(NativeCommand::op("bignukes blast <radius:text> <x:text> <y:text> <z:text>", 2));
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

pub(in crate::game) fn shake(game: &Game<'_>, partial: f32, shake: &mut NativeShake) {
    for (t, d, n) in felt(game, partial as f64) {
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
