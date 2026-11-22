//! The world's side of Big Nukes: its state and files, the command, the server's and the levels' ticks, the
//! blasts and their waves, the chunks, the world's phases and the declarations.
//!
//! Everything here runs on the server. A wave's work is done at the end of each tick of its own dimension's
//! level, within a time budget, as `edit_blocks` only edits the level being ticked.
//!
//! The state's lock is held across a wave's edits and its damage commands, as the old glue did: the hooks those
//! can fire (`block_changed`, `player_died`, `chunk_load`) never take it, they queue what they heard for the next
//! time it opens (`state`).

use crate::core::{self, Pl, block_at, lit, lock, run, send, tell, tell_everyone, tr};
use crate::{bunker, decon, events, keys, rad, rules, silo};
use mod_api::{
    Block, BlockChangedEvent, BlockProperties, Command, CommandArgumentValue, CommandSource, Falling, Flow, Item,
    LevelRef, Rarity, RunModCommandEvent, ServerHost, Text, ValidateTicketsEvent,
};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

/// The time a level's tick may spend on waves.
const BUDGET: Duration = Duration::from_millis(6);
/// Ticks between saves while a wave runs.
const SAVE_EVERY: u64 = 200;
/// How often the winter keeps the overworld raining, in ticks.
const WINTER_RAIN_EVERY: u64 = 200;
/// The tag a sheltered player wears while a wave's damage passes over them.
pub(crate) const SHELTERED: &str = "big_nukes_sheltered";
/// How many server ticks a chunk to hold waits for its level to open before it is given up for this session.
const WAKE_TRIES: u32 = 100;

/// The open world's waves and where they are kept.
pub(crate) struct State {
    dir: PathBuf,
    pub saved: rules::Saved,
    /// Blasts whose refused edit is already in the log.
    warned: BTreeSet<u64>,
    /// Blasts whose opening (the fireball and the debris) has been shown.
    opened: BTreeSet<u64>,
    /// Whether the game has the ash block, asked once a world.
    ash: Option<bool>,
    /// Something changed since the last save. A wave whose far chunks never load never ends, so saving only on
    /// changes keeps it from costing forever.
    pub dirty: bool,
    /// Dimensions to be opened at the end of the server's tick, where a command may run: those a remote's blast
    /// went off in, and when the world opens, those its flights and strikes are on their way in.
    pub(crate) wake: Vec<String>,
    /// Each dimension's next pass over its mobs in the zones: its tick, and where it starts in their list.
    pub(crate) mobs: HashMap<String, (u64, usize)>,
    /// Players whose window was last told of some radiation: they are told once more when there is none.
    pub(crate) told: BTreeSet<u128>,
    /// The last save's error while saving fails: the state stays dirty and is tried again.
    save_error: Option<String>,
    /// The flights' chunks to hold again by ticket at the end of the server's tick, with the ticks they waited
    /// for their level to open.
    regrab: Vec<(String, [i32; 2])>,
    regrab_tries: u32,
    /// The chunks a world from before 0.2.0 had `/forceload`ed for its flights, given back once.
    unforceload: Vec<(String, [i32; 2])>,
    /// Whether the players were told of the failing save (once until a save succeeds).
    save_told: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// The loaded chunks of each dimension, heard from the start of the world, before its waves are read.
static LOADED: Mutex<Option<HashMap<String, BTreeSet<[i32; 2]>>>> = Mutex::new(None);
/// Chunks loaded, for the waves waiting on them: taken the next time the state opens (a chunk may load while it
/// is locked).
static CHUNKS: Mutex<Vec<(String, [i32; 2])>> = Mutex::new(Vec::new());
/// Bombs whose block changed into another, heard from the level: forgotten the next time the state opens.
static BROKEN: Mutex<Vec<(String, [i32; 3])>> = Mutex::new(Vec::new());
/// Bombs placed (`Some`) and taken away (`None`), heard the same way.
static PLACED: Mutex<Vec<(String, [i32; 3], Option<rules::BombKind>)>> = Mutex::new(Vec::new());
/// Players who asked for the guide with `/bignukes guide`, given it at the next server tick.
static GUIDES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The open world's state, read from its folder the first time.
pub(crate) fn state(host: &dyn ServerHost) -> Option<std::sync::MutexGuard<'static, Option<State>>> {
    let dir = host.world_dir()?.join(rules::DIR);
    let mut guard = lock(&STATE);
    if guard.as_ref().is_none_or(|s| s.dir != dir) {
        let mut saved: rules::Saved = fs::read(dir.join("waves.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).map_err(|e| log::warn!("[big_nukes] waves.json: {e}")).ok())
            .unwrap_or_default();
        log::info!("[big_nukes] Big Nukes {} in {}", rules::VERSION, dir.display());
        let wake = saved.to_wake();
        let old = !saved.tickets;
        let unforceload = saved.to_tickets();
        let regrab = saved.held();
        *guard = Some(State {
            dir,
            saved,
            warned: BTreeSet::new(),
            opened: BTreeSet::new(),
            ash: None,
            dirty: old,
            wake,
            mobs: HashMap::new(),
            told: BTreeSet::new(),
            save_error: None,
            save_told: false,
            regrab,
            regrab_tries: 0,
            unforceload,
        });
    }
    let st = guard.as_mut().expect("opened");
    let mut forgot = false;
    for (dimension, pos) in std::mem::take(&mut *lock(&BROKEN)) {
        forgot |= st.saved.forget(&dimension, pos);
    }
    for (dimension, pos, kind) in std::mem::take(&mut *lock(&PLACED)) {
        forgot |= match kind {
            Some(kind) => st.saved.place(&dimension, pos, kind),
            None => st.saved.unplace(&dimension, pos),
        };
    }
    if forgot {
        save(st);
    }
    for (dimension, chunk) in std::mem::take(&mut *lock(&CHUNKS)) {
        st.dirty |= st.saved.waves.chunk_loaded(&dimension, chunk);
    }
    // The bunker's walls placed and taken away: a wall is placed a block at a time, so they keep till the next save.
    st.dirty |= bunker::take_heard(&mut st.saved);
    st.dirty |= rad::take_heard(&mut st.saved);
    Some(guard)
}

/// Writes the state. A failed write leaves it dirty, so the periodic save tries again; the error is kept for the
/// players to be told (`save_trouble`).
pub(crate) fn save(st: &mut State) -> bool {
    match rules::save_to(&st.saved, &mut st.dirty, &mut rules::Disk(&st.dir)) {
        Ok(()) => {
            if st.save_error.take().is_some() {
                log::info!("[big_nukes] {} saved again", rules::SAVE_FILE);
            }
            st.save_told = false;
            true
        }
        Err(e) => {
            log::warn!("[big_nukes] saving {}: {e}; it is tried again", rules::SAVE_FILE);
            st.save_error = Some(e.to_string());
            false
        }
    }
}

/// The line for the players when saving fails, once until a save succeeds.
fn save_trouble(st: &mut State) -> Option<Text> {
    let error = st.save_error.as_ref().filter(|_| !st.save_told)?;
    st.save_told = true;
    Some(tr("big_nukes.save.failed", vec![lit(error)]))
}

/// The server is stopping, its players still there: the state is saved now, and if that fails they are told, as
/// the world's close can only write it in the log.
pub(crate) fn server_stopping(host: &mut dyn ServerHost) {
    let Some(mut guard) = state(host) else { return };
    let st = guard.as_mut().expect("opened");
    if save(st) {
        return;
    }
    let error = st.save_error.clone().unwrap_or_default();
    drop(guard);
    tell_everyone(host, &tr("big_nukes.save.lost", vec![lit(error)]));
}

/// The game saved the world (its autosave, `/save-all`, a pause): the state goes with it, so what the world holds
/// and what the mod holds are of one moment. Not while the state is held: the periodic save takes it then.
pub(crate) fn world_saved() {
    let Ok(mut guard) = STATE.try_lock() else { return };
    if let Some(st) = guard.as_mut()
        && st.dirty
    {
        save(st);
    }
}

/// The world's clock and the operator's limit.
pub(crate) fn clock(host: &dyn ServerHost) -> Option<(u64, u32)> {
    let guard = state(host)?;
    let st = guard.as_ref()?;
    Some((st.saved.tick, st.saved.maxpower))
}

// ---- declarations -------------------------------------------------------------------

/// A block of the mod: a template's look and use, its hardness, a pickaxe's to break.
pub(crate) fn block(id: &str, template: &str, hardness: f32) -> Block {
    let mut block = Block::new(id, template).with_item(true);
    block.properties = BlockProperties {
        hardness: Some(hardness),
        explosion_resistance: Some(6.0),
        requires_tool: Some(true),
        ..Default::default()
    };
    block
}

/// The machines and the bombs. The centrifuge and the bombs are furnaces as a template: they open a menu and face
/// the player who placed them. The assembly table keeps the crafting table's look and use; its block entity and
/// menu are the mod's own.
pub(crate) fn blocks() -> Vec<Block> {
    let mut blocks = Vec::new();
    blocks.push(block(rules::CENTRIFUGE, "minecraft:furnace", 3.5));
    blocks.push(block(rules::ASSEMBLY_TABLE, "minecraft:crafting_table", 2.5));
    for kind in rules::BombKind::ALL {
        blocks.push(block(kind.id(), "minecraft:furnace", 2.0));
    }
    // The Sunbreaker's structure: plain metal blocks, the shell softer than the reflector. On the furnace too: the
    // game opens a block's menu on a right click only for a block that has one, and a part shows its core's keypad.
    blocks.push(block(rules::IMPLOSION_SHELL, "minecraft:furnace", 3.0));
    blocks.push(block(rules::NEUTRON_REFLECTOR, "minecraft:furnace", 5.0));
    decon::blocks(&mut blocks, &block);
    silo::blocks(&mut blocks, &block);
    events::blocks(&mut blocks, &block);
    bunker::blocks(&mut blocks);
    blocks
}

/// The items, in code like the blocks: the suit on iron armour, the rest on paper, each with the rules' patch of
/// its template's components (JSON in the rules, NBT for the game; a `!` name removes the template's).
pub(crate) fn items(items: &mut Vec<Item>) {
    for (id, stack, rare) in rules::ITEMS {
        let mut item = Item::new(id, rules::template(id));
        item.max_stack_size = Some(stack);
        item.rarity = Some(if rare { Rarity::Rare } else { Rarity::Common });
        item.components = rules::components(id)
            .iter()
            .map(|(name, value)| match name.strip_prefix('!') {
                Some(removed) => (removed.to_owned(), None),
                None => (name.clone(), Some(rules::json_tag(value))),
            })
            .collect();
        items.push(item);
    }
    silo::model_items(items);
}

// ---- joining ---------------------------------------------------------------------

/// A player joins: a shelter tag left on by a game stopped mid-blow goes, the guide is given once, and someone
/// joining mid-blast hears what's on its way.
pub(crate) fn logged_in(host: &mut dyn ServerHost, player: u128, name: &str) {
    run(host, &format!("tag {name} remove {SHELTERED}"));
    untag_sheltered(host);
    // The guide, once a player: the tag says they had it.
    let tag = rules::GUIDE_TAG;
    run(host, &format!("execute as {name} unless entity @s[tag={tag}] run {}", rules::guide_give("@s")));
    run(host, &format!("tag {name} add {tag}"));
    let dimension = core::by_uuid(host, player).map(|p| p.dimension);
    let news = dimension
        .and_then(|d| state(host).and_then(|g| g.as_ref().map(|st| st.saved.waves.news(&d, st.saved.tick))))
        .unwrap_or_default();
    for n in &news {
        send_news(host, player, n);
    }
}

// ---- the command -----------------------------------------------------------------

pub(crate) fn commands(commands: &mut Vec<Command>) {
    // One text argument, last: the radius and the optional place are read from it.
    commands.push(Command::op("bignukes blast <args:text>", 2));
    commands.push(Command::op("bignukes maxpower", 2));
    commands.push(Command::op("bignukes maxpower <limit:integer>", 2));
    commands.push(Command::op("bignukes events", 2));
    commands.push(Command::op("bignukes events <args:text>", 2));
    commands.push(Command::new("bignukes guide"));
}

/// The command's words as the old glue split its line: the literals, then the arguments' text.
fn words(event: &RunModCommandEvent<'_>) -> Vec<String> {
    let mut words = event.command.literals.clone();
    for name in ["args", "limit"] {
        match event.command.argument(name) {
            Some(CommandArgumentValue::String(s)) => words.extend(s.split(' ').filter(|w| !w.is_empty()).map(str::to_owned)),
            Some(CommandArgumentValue::Integer(n)) => words.push(n.to_string()),
            _ => {}
        }
    }
    words
}

pub(crate) fn command(host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
    if event.command.literals.first().map(String::as_str) != Some("bignukes") {
        return Flow::Continue;
    }
    let words = words(event);
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    // Where it runs: the player's place, or the command's own (a command block, `execute`).
    let who: Option<Pl> = match event.source {
        CommandSource::Player { player, .. } => core::pl(host, *player),
        _ => None,
    };
    let (dimension, position) = match &who {
        Some(p) => (p.dimension.clone(), p.position),
        None => (event.command.dimension.id(host).unwrap_or_default(), event.command.position),
    };
    let reply = match words.get(1) {
        Some(&"blast") => match rules::parse_blast(&words[2..], position) {
            Some((radius, center)) => match start_blast(host, &dimension, center, radius) {
                Ok(_) => tr("big_nukes.blast.started", vec![lit(radius)]),
                Err(why) => refused(why),
            },
            None => usage(),
        },
        Some(&"maxpower") => maxpower(host, &words[2..]),
        Some(&"events") => events::command(host, &words[2..]),
        // A command doesn't run inside a player's own: the next server tick gives the book.
        Some(&"guide") => match &who {
            Some(p) => {
                lock(&GUIDES).push(p.name.clone());
                tr("big_nukes.guide.given", vec![])
            }
            None => usage(),
        },
        _ => usage(),
    };
    let _ = host.commands().feedback(reply);
    Flow::Cancel
}

/// `/bignukes maxpower [limit]`: tells the operator's limit, or sets it to one of the steps and saves it at once.
fn maxpower(host: &mut dyn ServerHost, args: &[&str]) -> Text {
    let n = |v: u32| lit(v);
    let (Some(asked), Some(mut guard)) = (rules::parse_maxpower(args), state(host)) else {
        let steps: Vec<String> = rules::MAXPOWER_STEPS.iter().map(u32::to_string).collect();
        return tr("big_nukes.maxpower.usage", vec![lit(steps.join("|"))]);
    };
    let st = guard.as_mut().expect("opened");
    match asked {
        Some(limit) => {
            st.saved.maxpower = limit;
            save(st);
            log::info!("[big_nukes] the operator's limit is now radius {limit}");
            tr("big_nukes.maxpower.set", vec![n(limit)])
        }
        None => tr("big_nukes.maxpower.current", vec![n(st.saved.maxpower)]),
    }
}

fn usage() -> Text {
    tr("big_nukes.blast.usage", vec![lit(rules::MIN_RADIUS), lit(rules::MAX_RADIUS)])
}

// ---- starting a blast --------------------------------------------------------------

/// Starts a blast of `radius` at `center`, within the operator's limit: its wave, saved at once, and the news to
/// the players of its dimension. The command and the satellite go through here.
pub(crate) fn start_blast(host: &mut dyn ServerHost, dimension: &str, center: [i32; 3], radius: u32) -> Result<u64, rules::Refused> {
    start(host, dimension, |saved| saved.fire(dimension, center, radius))
}

/// Starts the blast of a bomb of `kind` reaching `radius` at `center`: as `start_blast`, with the bomb's effect,
/// within the operator's limit by its level.
pub(crate) fn start_bomb(
    host: &mut dyn ServerHost,
    dimension: &str,
    center: [i32; 3],
    kind: rules::BombKind,
    radius: u32,
) -> Result<u64, rules::Refused> {
    start(host, dimension, |saved| saved.fire_kind(dimension, center, kind, radius))
}

/// A blast in `dimension` that `fire` starts in the world's record: saved at once, and its news to the players
/// there.
pub(crate) fn start(
    host: &mut dyn ServerHost,
    dimension: &str,
    fire: impl FnOnce(&mut rules::Saved) -> Result<u64, rules::Refused>,
) -> Result<u64, rules::Refused> {
    let Some(mut guard) = state(host) else {
        return Err(rules::Refused::Dimension);
    };
    let st = guard.as_mut().expect("opened");
    let id = fire(&mut st.saved)?;
    save(st);
    let news: Vec<rules::News> = st.saved.waves.news(dimension, st.saved.tick).into_iter().filter(|n| n.id == id).collect();
    drop(guard);
    tell_dimension(host, dimension, &news);
    if let Some(n) = news.first() {
        log::info!("[big_nukes] blast {id}: radius {} at {:?} in {dimension}", n.radius, n.center);
    }
    Ok(id)
}

/// A dimension to open at the end of the server's tick (a remote's blast in a level nobody is in).
pub(crate) fn wake(dimension: &str) {
    if let Some(st) = lock(&STATE).as_mut()
        && !st.wake.iter().any(|d| d == dimension)
    {
        st.wake.push(dimension.to_owned());
    }
}

/// Why a blast didn't start, for the player who asked.
pub(crate) fn refused(why: rules::Refused) -> Text {
    let n = |v: u32| lit(v);
    match why {
        rules::Refused::Dimension => tr("big_nukes.blast.dimension", vec![]),
        rules::Refused::Maxpower { radius, maxpower } => tr("big_nukes.maxpower.over", vec![n(radius), n(maxpower)]),
        rules::Refused::Gone => tr("big_nukes.remote.gone", vec![]),
        rules::Refused::Unread => tr("big_nukes.sunbreaker.unread", vec![]),
        rules::Refused::Structure(rules::StructureProblem::Shell(shell)) => {
            tr("big_nukes.sunbreaker.shell", vec![n(shell), n(rules::SHELL_BLOCKS)])
        }
    }
}

/// How far a bomb's refusal is told.
const TOLD_WITHIN: f64 = 64.0;

/// A line in the chat of every player near the bomb.
pub(crate) fn tell_near(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3], line: &Text) {
    tell_within(host, dimension, pos, line, TOLD_WITHIN);
}

/// A line in the chat of every player within `within` blocks of `pos`.
pub(crate) fn tell_within(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3], line: &Text, within: f64) {
    let at = core::middle(pos);
    for p in core::players(host) {
        if p.dimension == dimension && core::distance(p.position, at) <= within {
            tell(host, p.uuid, line.clone());
        }
    }
}

// ---- the blast's effects, on the server ---------------------------------------------

fn send_news(host: &mut dyn ServerHost, player: u128, news: &rules::News) {
    let Ok(bytes) = serde_json::to_vec(news) else { return };
    if let Err(e) = send(host, player, rules::NEWS_CHANNEL, bytes) {
        log::warn!("[big_nukes] telling a player of blast {}: {e}", news.id);
    }
}

/// Every player in `dimension` hears of the blasts.
pub(crate) fn tell_dimension(host: &mut dyn ServerHost, dimension: &str, news: &[rules::News]) {
    if news.is_empty() {
        return;
    }
    for p in core::players(host).into_iter().filter(|p| p.dimension == dimension) {
        for n in news {
            send_news(host, p.uuid, n);
        }
    }
}

/// A command in the blast's dimension.
fn run_in(host: &mut dyn ServerHost, blast: &rules::Blast, command: &str) {
    run(host, &format!("execute in {} run {command}", blast.dimension));
}

/// A particle command seen from any distance (`force`: the API's particles reach 32 blocks).
fn particle(id: &str, at: [f64; 3], spread: [f64; 3], speed: f64, count: u32) -> String {
    format!(
        "particle {id} {:.1} {:.1} {:.1} {:.1} {:.1} {:.1} {speed:.2} {count} force",
        at[0], at[1], at[2], spread[0], spread[1], spread[2]
    )
}

/// Smoke on the front and over the center, the opening's fireball and debris, and the damage of the ground the
/// front crossed this tick.
fn effects(host: &mut dyn ServerHost, st: &mut State, dimension: &str) {
    for (blast, from, to, age) in st.saved.waves.fronts(dimension, st.saved.tick) {
        let [cx, cy, cz] = blast.center.map(|v| v as f64 + 0.5);
        if blast.effect == rules::Effect::Neutron {
            neutron(host, st, &blast, from, to, age);
            continue;
        }
        if age < 20 && st.opened.insert(blast.id) {
            run_in(host, &blast, &particle("minecraft:explosion_emitter", [cx, cy + 2.0, cz], [2.0, 2.0, 2.0], 0.0, 6));
            let mut falling = Falling::default();
            falling.drop_item = false;
            falling.hurts = true;
            for (pos, v, block) in rules::debris(&blast) {
                if let Some(state) = host.block_key(block).and_then(|k| host.block_state_default(k)) {
                    host.spawn_falling_block(&blast.dimension, state, pos, v, falling.clone());
                }
            }
        }
        if age % 4 == 0 {
            for at in rules::front_ring(&blast, age) {
                run_in(host, &blast, &particle("minecraft:campfire_signal_smoke", at, [2.0, 1.0, 2.0], 0.02, 4));
                run_in(host, &blast, &particle("minecraft:large_smoke", at, [3.0, 0.5, 3.0], 0.05, 6));
            }
            if to < 512.0 {
                for h in [10.0, 20.0, 30.0, 40.0] {
                    run_in(host, &blast, &particle("minecraft:campfire_signal_smoke", [cx, cy + h, cz], [3.0, 2.0, 3.0], 0.05, 8));
                }
            }
        }
        let bands = rules::damage_band(blast.radius, from, to);
        if bands.is_empty() {
            continue;
        }
        // A player the blow reaches this tick under a roof of the bunker the wave leaves standing, or deep under
        // the crater's floor, is spared it: tagged, by their UUID, for the damage's selector to pass over. The
        // damage stays a command: it has to come from the blast's center for its knockback (the API's `hurt` has
        // no source position).
        let sheltered: Vec<u128> = core::players(host)
            .into_iter()
            .filter(|p| p.dimension == blast.dimension && rules::in_band(&bands, [cx, cy, cz], p.position))
            .filter(|p| {
                let feet = p.position.map(|v| v.floor() as i32);
                let head = [feet[0], feet[1] + 1, feet[2]];
                let chunk = rules::chunk_of(feet[0], feet[2]);
                let walls = rules::walls_near(&st.saved.walls, &blast.dimension, chunk);
                rules::below_crater(&blast, head) || rules::roofed(&blast, &walls, head, st.saved.waves.worked(blast.id, chunk))
            })
            .map(|p| p.uuid)
            .collect();
        for uuid in &sheltered {
            run(host, &format!("tag {} add {SHELTERED}", rules::uuid_text(*uuid)));
        }
        for (a, b, amount) in bands {
            run_in(
                host,
                &blast,
                &format!(
                    "execute as @e[x={cx:.1},y={cy:.1},z={cz:.1},distance={a:.1}..{b:.1},tag=!{SHELTERED}] run damage @s {amount:.1} minecraft:explosion at {cx:.1} {cy:.1} {cz:.1}"
                ),
            );
        }
        if !sheltered.is_empty() {
            untag_sheltered(host);
        }
    }
}

/// A neutron blast's front: a flash of light at its opening, no debris, no dust; the creatures it crossed this
/// tick die and the players it crossed take its dose at once, but those under a roof of the bunker's walls or in a
/// sealed room.
fn neutron(host: &mut dyn ServerHost, st: &mut State, blast: &rules::Blast, from: f64, to: f64, age: u64) {
    let [cx, cy, cz] = blast.center.map(|v| v as f64 + 0.5);
    let reach = blast.radius as f64;
    if age < 20 && st.opened.insert(blast.id) {
        run_in(host, blast, &particle("minecraft:flash", [cx, cy + 2.0, cz], [0.0, 0.0, 0.0], 0.0, 1));
        run_in(host, blast, &particle("minecraft:end_rod", [cx, cy + 2.0, cz], [4.0, 4.0, 4.0], 0.2, 80));
    }
    if from >= reach {
        return;
    }
    let to = to.min(reach);
    let dist = |pos: [f64; 3]| core::distance(pos, [cx, cy, cz]);
    // Creative and spectating players take no dose, as they take none in a zone.
    for p in core::players(host).into_iter().filter(|p| p.dimension == blast.dimension && !p.exempt()) {
        let d = dist(p.position);
        if d < from || d >= to {
            continue;
        }
        let feet = p.position.map(|v| v.floor() as i32);
        let head = [feet[0], feet[1] + 1, feet[2]];
        let walls = rules::walls_near(&st.saved.walls, &blast.dimension, rules::chunk_of(feet[0], feet[2]));
        let shielded = rules::roofed(blast, &walls, head, false) || bunker::shelter_of(p.uuid) <= rules::SEALED_SHELTER;
        st.saved.irradiate(p.uuid, rules::neutron_dose(d, blast.radius, shielded));
        st.dirty = true;
    }
    // The mobs it crossed die; items, frames, carts and the like are not alive and stay.
    let Some(level) = host.level(&blast.dimension) else { return };
    for (entity, pos) in rad::mobs(host, level) {
        if (from..to).contains(&dist(pos)) {
            rad::hurt(host, entity, rules::NEUTRON_MOB_DAMAGE as f32);
        }
    }
}

/// Every player's shelter tag taken off: after the damage, and as a player joins, so none is left on for good.
fn untag_sheltered(host: &mut dyn ServerHost) {
    run(host, &format!("tag @a[tag={SHELTERED}] remove {SHELTERED}"));
}

// ---- ticks, chunks and the world's phases ----------------------------------------

/// The end of each server tick.
pub(crate) fn server_tick(host: &mut dyn ServerHost) {
    bunker::server_tick(host);
    let Some(mut guard) = state(host) else { return };
    let st = guard.as_mut().expect("opened");
    st.saved.tick += 1;
    let (sick, rad) = if st.saved.tick % rules::SECOND == 0 {
        st.dirty |= st.saved.prune_zones(st.saved.tick);
        (rad::sick(host, st), rad::rad_news(host, st))
    } else {
        (Vec::new(), Vec::new())
    };
    if st.dirty && st.saved.tick % SAVE_EVERY == 0 {
        save(st);
    }
    let trouble = save_trouble(st);
    // The winter keeps the overworld raining (snowing where it is cold)...
    let rain = st.saved.winter() && st.saved.tick % WINTER_RAIN_EVERY == 0;
    // ...and clears once, as it ends: with the weather cycle off, the rain would stay for good.
    let thaw = st.saved.winter_until > 0 && st.saved.tick == st.saved.winter_until;
    let wake = std::mem::take(&mut st.wake);
    let regrab = std::mem::take(&mut st.regrab);
    let unforceload = std::mem::take(&mut st.unforceload);
    drop(guard);
    if let Some(line) = trouble {
        tell_everyone(host, &line);
    }
    // The duration in seconds ("s"): a bare number is ticks.
    if rain {
        run(host, &format!("execute in minecraft:overworld run weather rain {}s", 2 * WINTER_RAIN_EVERY / rules::SECOND));
    }
    if thaw {
        run(host, "execute in minecraft:overworld run weather clear");
    }
    // The effects and the damage go with the state let go: a death they cause is heard at once.
    for (who, stage, damage) in sick {
        rad::sicken(host, &who, stage, damage);
    }
    for (player, bytes) in rad {
        if let Err(e) = send(host, player, rules::RAD_CHANNEL, bytes) {
            log::warn!("[big_nukes] telling a player of the radiation: {e}");
        }
    }
    // A level nobody has entered since the world opened isn't open, and its forced chunks don't tick: a command
    // run in it opens it, so a blast there works at once in the chunks an operator keeps loaded. The rest of its
    // wave waits for its chunks to load, as anywhere else.
    for dimension in wake {
        run(host, &format!("execute in {dimension} run forceload query"));
    }
    regrab_chunks(host, regrab);
    for (dimension, [x, z]) in unforceload {
        run(host, &format!("execute in {dimension} run forceload remove {} {}", x * 16, z * 16));
    }
    decon::server_tick(host);
    events::server_tick(host);
    let asked = std::mem::take(&mut *lock(&GUIDES));
    for name in asked {
        run(host, &rules::guide_give(&name));
    }
}

/// The flights' chunks held again as the world opens (a world from 0.4 and older kept no tickets). A level not
/// open yet is opened by a command and its chunks wait for the next ticks, a while.
fn regrab_chunks(host: &mut dyn ServerHost, regrab: Vec<(String, [i32; 2])>) {
    let mut waiting = Vec::new();
    let mut opening: Vec<String> = Vec::new();
    for (dimension, chunk) in regrab {
        if host.level(&dimension).is_none() {
            if !opening.contains(&dimension) {
                run(host, &format!("execute in {dimension} run forceload query"));
                opening.push(dimension.clone());
            }
            waiting.push((dimension, chunk));
            continue;
        }
        if !core::force_chunk(host, &dimension, chunk, true) {
            log::warn!("[big_nukes] couldn't hold chunk {chunk:?} of {dimension}");
        }
    }
    if waiting.is_empty() {
        return;
    }
    if let Some(st) = lock(&STATE).as_mut() {
        st.regrab_tries += 1;
        if st.regrab_tries < WAKE_TRIES {
            st.regrab.extend(waiting);
        } else {
            log::warn!("[big_nukes] {} didn't open: their flights' chunks wait for someone to go there", opening.join(", "));
        }
    }
}

/// The world's saved tickets, as a level loads: a chunk's ticket stays only while a flight keeps it.
pub(crate) fn validate_tickets(host: &mut dyn ServerHost, event: &mut ValidateTicketsEvent<'_>) {
    let Some(guard) = state(host) else { return };
    let st = guard.as_ref().expect("opened");
    let held: BTreeSet<[i32; 3]> = st
        .saved
        .held()
        .into_iter()
        .filter(|(d, _)| d == event.dimension)
        .map(|(_, c)| [c[0] * 16, 0, c[1] * 16])
        .collect();
    drop(guard);
    for (pos, _) in event.validation.block_tickets() {
        if !held.contains(&pos) {
            event.validation.remove_block_tickets(pos);
        }
    }
}

/// The end of a level's tick: its mobs in the zones are hurt, its waves move on, and some of their work is done.
pub(crate) fn level_tick(host: &mut dyn ServerHost, level: LevelRef) {
    let Some(dimension) = level.dimension().id(host) else { return };
    let dimension = dimension.as_str();
    rad::irradiate_mobs(host, level, dimension);
    events::level_tick(host, level, dimension);
    silo::level_tick(host, level, dimension);
    decon::level_tick(host, level, dimension);
    let Some(mut guard) = state(host) else { return };
    let st = guard.as_mut().expect("opened");
    effects(host, st, dimension);
    if st.saved.waves.idle(dimension) {
        return;
    }
    let loaded = lock(&LOADED).as_ref().and_then(|m| m.get(dimension)).cloned().unwrap_or_default();
    st.dirty |= st.saved.waves.advance(dimension, st.saved.tick, &|c| loaded.contains(&c));
    let ash = *st.ash.get_or_insert_with(|| has_ash(host));
    let started = Instant::now();
    let warned = &mut st.warned;
    let mut worked = Vec::new();
    // The bombs a wave reaches go off as it gets to them: read before the edits take them.
    let placed = st.saved.placed.clone();
    let mut reached: Vec<([i32; 3], HashMap<[i32; 3], String>)> = Vec::new();
    // The bunker's walls, read when each chunk is worked; what the work does to them goes back after.
    let walls = &st.saved.walls;
    let (mut wall_fixes, mut wall_changes) = (Vec::new(), Vec::new());
    let done = st.saved.waves.work(dimension, BUDGET, &|| started.elapsed(), &mut |blast, chunk| {
        if !loaded.contains(&chunk) {
            return false;
        }
        worked.push(chunk);
        for (pos, kind) in rules::reached(&placed, blast, chunk) {
            // A Sunbreaker is reached from each chunk of its structure: read with the first.
            if reached.iter().any(|(p, _)| *p == pos) {
                continue;
            }
            if rules::BombKind::of_block(&block_at(host, level, pos)) == Some(kind) {
                // A Sunbreaker goes off with what is left of its structure as the wave gets there.
                let blocks = if kind == rules::BombKind::Sunbreaker { structure_now(host, level, pos) } else { HashMap::new() };
                reached.push((pos, blocks));
            }
        }
        let near = bunker::walls_for(host, level, walls, &blast.dimension, chunk, &mut wall_fixes);
        let (edits, changes) = rules::plan_chunk(blast, chunk, &near);
        wall_changes.extend(changes);
        for edit in edits {
            if edit.runs.is_empty() {
                continue;
            }
            let edit = if ash { edit } else { edit.without_ash() };
            let result = core::native(host, edit).and_then(|edit| host.edit_blocks(&blast.dimension, &edit));
            if let Err(e) = result
                && warned.insert(blast.id)
            {
                log::warn!("[big_nukes] blast {}: an edit was refused: {e}", blast.id);
            }
        }
        true
    });
    st.dirty |= done > 0;
    st.dirty |= bunker::fix_walls(&mut st.saved, dimension, wall_fixes);
    st.dirty |= st.saved.change_walls(dimension, &wall_changes);
    // A bulk edit tells no one of the blocks it changes: the bombs with a receiver that a blast took are forgotten
    // here. A bomb at a blast's center that its crater missed (above the Nether's roof) goes now, so a bomb set off
    // from afar can't stay to be linked and set off again.
    let mut news = Vec::new();
    for (pos, blocks) in reached {
        match st.saved.set_off(dimension, pos, &read_from(&blocks)) {
            Ok(id) => {
                log::info!("[big_nukes] blast {id}: set off by another at {pos:?} in {dimension}");
                take_bomb(host, level, pos);
                news.extend(st.saved.waves.news(dimension, st.saved.tick).into_iter().filter(|n| n.id == id));
            }
            // A Sunbreaker without a whole shell doesn't go off and stays: a crater that reaches its core takes it.
            Err(rules::Refused::Structure(problem)) => {
                log::info!("[big_nukes] the sunbreaker at {pos:?} in {dimension} didn't go off: {problem:?}");
            }
            Err(_) => {}
        }
    }
    if !news.is_empty() {
        save(st);
    }
    for chunk in worked {
        for pos in st.saved.placed_in(dimension, chunk) {
            if rules::BombKind::of_block(&block_at(host, level, pos)).is_none() {
                st.dirty |= st.saved.unplace(dimension, pos);
            }
        }
        for center in st.saved.waves.centers_in(dimension, chunk) {
            if rules::BombKind::of_block(&block_at(host, level, center)).is_some() {
                take_bomb(host, level, center);
            }
        }
        for pos in st.saved.bombs_in(dimension, chunk) {
            if rules::BombKind::of_block(&block_at(host, level, pos)).is_none() {
                st.dirty |= st.saved.forget(dimension, pos);
            }
        }
    }
    if st.saved.waves.idle(dimension) {
        log::info!("[big_nukes] {dimension}: the loaded part of every wave is done at tick {}", st.saved.tick);
    }
    if st.saved.waves.finished() {
        log::info!("[big_nukes] every wave is over");
        save(st);
    }
    drop(guard);
    tell_dimension(host, dimension, &news);
}

/// The blocks round the bomb at `pos` that a Sunbreaker's structure takes, as they stand now: read for a rule that
/// looks at them after the state is locked or the wave has changed them.
pub(crate) fn structure_now(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> HashMap<[i32; 3], String> {
    rules::structure_cells(pos).map(|(p, _)| (p, block_at(host, level, p))).collect()
}

/// Reads `blocks` as the world: what isn't there is air.
pub(crate) fn read_from(blocks: &HashMap<[i32; 3], String>) -> impl Fn([i32; 3]) -> String + '_ {
    move |p| blocks.get(&p).cloned().unwrap_or_else(|| "minecraft:air".to_owned())
}

/// A bomb gone off where it stands: its detonator with it, as its blast takes it.
pub(crate) fn take_bomb(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) {
    crate::bombs::clear_items(host, level, pos);
    core::set_air(host, level, pos);
}

/// Whether the world's game has the ash block, asked once a world. The ash ships with the mod, so this is the
/// game's word that it loaded.
#[allow(dead_code)] // TODO(stage 2): decon's regrowth asks it.
pub(crate) fn ash_here(host: &dyn ServerHost, _dimension: &str) -> bool {
    let Some(mut guard) = state(host) else { return false };
    let st = guard.as_mut().expect("opened");
    *st.ash.get_or_insert_with(|| has_ash(host))
}

fn has_ash(host: &dyn ServerHost) -> bool {
    let here = host.block_state_default(keys().ash).is_some();
    if !here {
        log::info!("[big_nukes] no ash: the scorch is coarse dirt");
    }
    here
}

/// Whether the chunk of `pos` is loaded in `dimension`.
pub(crate) fn loaded(dimension: &str, pos: [i32; 3]) -> bool {
    let chunk = rules::chunk_of(pos[0], pos[2]);
    lock(&LOADED).as_ref().and_then(|m| m.get(dimension)).is_some_and(|set| set.contains(&chunk))
}

pub(crate) fn chunk_load(dimension: &str, chunk: [i32; 2]) {
    lock(&LOADED).get_or_insert_default().entry(dimension.to_owned()).or_default().insert(chunk);
    lock(&CHUNKS).push((dimension.to_owned(), chunk));
}

pub(crate) fn chunk_unload(dimension: &str, chunk: [i32; 2]) {
    if let Some(m) = lock(&LOADED).as_mut()
        && let Some(set) = m.get_mut(dimension)
    {
        set.remove(&chunk);
    }
}

/// The world closing: the waves are written and let go.
pub(crate) fn close_world() {
    {
        let mut state = lock(&STATE);
        if let Some(st) = state.as_mut()
            && !save(st)
        {
            log::error!(
                "[big_nukes] the world closed without its Big Nukes state saved ({}): the blasts, radiation and bunkers since the last good save in {} are lost",
                st.save_error.as_deref().unwrap_or("?"),
                st.dir.display()
            );
        }
        *state = None;
    }
    *lock(&LOADED) = None;
    lock(&CHUNKS).clear();
    lock(&BROKEN).clear();
    lock(&PLACED).clear();
    lock(&GUIDES).clear();
    core::forget_edits();
    rad::forget();
    crate::bombs::forget();
    crate::client::forget();
    silo::forget();
    decon::forget();
    bunker::forget();
    events::forget();
}

/// A block changed, by any means: a bomb that is no longer there is forgotten. Runs for every change in the world,
/// so it asks the mod's own block names only, and leaves the forgetting to the state's next opening, as the level
/// may change a block while the state is locked.
pub(crate) fn block_changed(event: &BlockChangedEvent) {
    let (was, now) = (event.old.block(), event.new.block());
    if was == now {
        return;
    }
    let (was, now) = (core::mod_block(was), core::mod_block(now));
    if was.is_none() && now.is_none() {
        return;
    }
    let dimension = event.dimension.key().to_string();
    let at = [event.pos.x, event.pos.y, event.pos.z];
    // A wall of the bunker placed or taken away.
    bunker::block_changed(&dimension, at, was.unwrap_or(""), now.unwrap_or(""));
    // A strike beacon, a console or a launch pad taken away.
    if matches!(was, Some(rules::STRIKE_BEACON | rules::LAUNCH_CONSOLE | rules::LAUNCH_PAD)) {
        silo::taken(&dimension, at);
    }
    let (taken, placed) = (was.and_then(rules::BombKind::of_block), now.and_then(rules::BombKind::of_block));
    if taken.is_some() {
        lock(&BROKEN).push((dimension.clone(), at));
        lock(&PLACED).push((dimension.clone(), at, None));
    }
    if placed.is_some() {
        lock(&PLACED).push((dimension, at, placed));
    }
}
