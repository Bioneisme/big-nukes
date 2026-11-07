// Big Nukes in the game: the edges between the game's types and the rules
// (`game::big_nukes`, runtime.rs). Added to the game as `game::big_nukes_glue`;
// each small helper file calls it from a hook point.
//
// Everything here runs on the server. A wave's work is done at the end of
// each tick of its own dimension's level, within a time budget, as
// `native_edit_blocks` only edits the level being ticked.

use crate::game::{
    Game, GameLevel, LevelDimension, NativeBlockEdit, NativeBlockRun, NativeFalling, NativeHeightmap, NativeShake,
    big_nukes as rules,
    // The silo and the bomber (plan 7): their glue is a module of its own, silo.rs.
    big_nukes_silo as silo,
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    fog::{FogData, FogType},
    hud::HudBatch,
    items::ItemStack,
    lang,
    mod_api::{EquipmentSlot, NativeCommand, NativeScreen, NativeScreenView, NativeServerPlayer, ServerTickPhase, WorldPhase},
    particles::{ParticleOptions, ParticleType},
    text::{self, ClickEvent, Style, Text},
};
use horizon_core::{geometry::Direction, position::BlockPos};
use horizon_mods::content::{Block, BlockProperties, Item, Rarity};
use horizon_world::{
    nbt::{Compound, JavaString, Tag},
    registry::BlockStateId,
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
pub(in crate::game) struct State {
    dir: PathBuf,
    pub(in crate::game) saved: rules::Saved,
    /// Blasts whose refused edit is already in the log.
    warned: BTreeSet<u64>,
    /// Blasts whose opening (the fireball and the debris) has been shown.
    opened: BTreeSet<u64>,
    /// Whether the game has the ash block, asked once a world, when a wave first works.
    ash: Option<bool>,
    /// Something changed since the last save. A wave whose far chunks never load
    /// never ends, so saving only on changes keeps it from costing forever.
    pub(in crate::game) dirty: bool,
    /// Dimensions a remote's blast went off in, to be opened at the end of the
    /// server's tick, where a command may run.
    wake: Vec<String>,
    /// Each dimension's next pass over its mobs in the zones: its tick, and where
    /// it starts in their list.
    mobs: HashMap<String, (u64, usize)>,
    /// Players whose window was last told of some radiation: they are told once
    /// more when there is none, so it lets go.
    told: BTreeSet<u128>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// The loaded chunks of each dimension, heard from the start of the world, before
/// its waves are read.
static LOADED: Mutex<Option<HashMap<String, BTreeSet<[i32; 2]>>>> = Mutex::new(None);
/// Bombs whose block changed into another, heard from the level, which may change
/// a block while the state is locked: they are forgotten the next time it opens.
static BROKEN: Mutex<Vec<(String, [i32; 3])>> = Mutex::new(Vec::new());
/// Bombs placed (`Some`) and taken away (`None`), heard from the level the same
/// way: the world's list of standing bombs takes them the next time it opens.
static PLACED: Mutex<Vec<(String, [i32; 3], Option<rules::BombKind>)>> = Mutex::new(Vec::new());
/// Players who asked for the guide with `/bignukes guide`, given it at the next server tick.
static GUIDES: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Players who died, heard while the state may be locked (a blast's damage or the
/// sickness' kills them from a command): their doses go the next time it opens.
static DIED: Mutex<Vec<u128>> = Mutex::new(Vec::new());
/// Doses the anti-radiation drug took down, the player and the rad, taken the
/// same way.
static ANTI_RAD: Mutex<Vec<(u128, f64)>> = Mutex::new(Vec::new());

pub(in crate::game) fn lock<T>(m: &'static Mutex<T>) -> MutexGuard<'static, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The open world's state, read from its folder the first time.
pub(in crate::game) fn state(game: &Game<'_>) -> Option<MutexGuard<'static, Option<State>>> {
    let dir = game.native_world_dir()?.join(rules::DIR);
    let mut guard = lock(&STATE);
    if guard.as_ref().is_none_or(|s| s.dir != dir) {
        let saved = fs::read(dir.join("waves.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).map_err(|e| log::warn!("[big_nukes] waves.json: {e}")).ok())
            .unwrap_or_default();
        log::info!("[big_nukes] Big Nukes {} in {}", rules::VERSION, dir.display());
        *guard = Some(State {
            dir,
            saved,
            warned: BTreeSet::new(),
            opened: BTreeSet::new(),
            ash: None,
            dirty: false,
            wake: Vec::new(),
            mobs: HashMap::new(),
            told: BTreeSet::new(),
        });
    }
    let broken = std::mem::take(&mut *lock(&BROKEN));
    let st = guard.as_mut().expect("opened");
    let mut forgot = false;
    for (dimension, pos) in broken {
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
    for uuid in std::mem::take(&mut *lock(&DIED)) {
        st.dirty |= st.saved.died(uuid);
    }
    for (uuid, amount) in std::mem::take(&mut *lock(&ANTI_RAD)) {
        st.dirty |= st.saved.take_dose(uuid, amount);
    }
    Some(guard)
}

pub(in crate::game) fn save(st: &mut State) {
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
/// template: they open a menu and face the player who placed them. The assembly
/// table keeps the crafting table's look and use; its block entity and menu are
/// the mod's own, as any template may have them.
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
    silo::blocks(blocks, block);
}

/// The items, in code like the blocks: the suit on iron armour, the rest on
/// paper, each with the rules' patch of its template's components.
pub(in crate::game) fn items(items: &mut Vec<Item>) {
    for (id, stack, rare) in rules::ITEMS {
        items.push(Item {
            id: id.into(),
            template: rules::template(id).into(),
            max_stack_size: Some(stack),
            rarity: Some(if rare { Rarity::Rare } else { Rarity::Common }),
            components: rules::components(id),
            ..Default::default()
        });
    }
    silo::model_items(items);
}

// ---- the anti-rad ------------------------------------------------------------------

/// A consume effect of the mod's own kind (API 74): a drink of anti-rad takes
/// its amount off the drinker's dose.
pub(in crate::game) fn consume_effect(game: &mut Game<'_>, kind: &str, data: &serde_json::Value) {
    if kind != rules::ANTI_RADIATION {
        return;
    }
    let Some(amount) = rules::anti_rad_amount(data) else {
        log::warn!("[big_nukes] {kind} without a positive amount: {data}");
        return;
    };
    let Some(who) = game.native_acting_player() else {
        log::warn!("[big_nukes] {kind}: no drinker");
        return;
    };
    anti_radiation(who.uuid, amount);
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
        // The guide, once a player: the tag says they had it.
        let tag = rules::GUIDE_TAG;
        game.native_run_command(&format!("execute as {name} unless entity @s[tag={tag}] run {}", rules::guide_give("@s")));
        game.native_run_command(&format!("tag {name} add {tag}"));
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
        "Big Nukes: its assets didn't load. If big_nukes is installed, restart the game; if not, install it: ",
        "Big Nukes: ассеты не загрузились. Если мод big_nukes установлен, перезапустите игру; если нет, поставьте его: ",
    ] {
        let line = Text::literal(line).append(Text::styled(ASSETS_PAGE, link.clone()));
        game.native_tell(player, line);
    }
}

// ---- the command -----------------------------------------------------------------

pub(in crate::game) fn commands(commands: &mut Vec<NativeCommand>) {
    // One text argument, last: the radius and the optional place are read from it.
    commands.push(NativeCommand::op("bignukes blast <args:text>", 2));
    commands.push(NativeCommand::op("bignukes maxpower [limit:integer]", 2));
    commands.push(NativeCommand::new("bignukes guide"));
}

pub(in crate::game) fn perform(game: &mut Game<'_>, player: &NativeServerPlayer, line: &str, ran: &mut bool) {
    let words: Vec<&str> = line.split(' ').filter(|w| !w.is_empty()).collect();
    if *ran || words.first() != Some(&"bignukes") || !game.native_on(rules::ID) {
        return;
    }
    *ran = true;
    let reply = match words.get(1) {
        Some(&"blast") => match rules::parse_blast(&words[2..], player.position) {
            Some((radius, center)) => match start_blast(game, &player.dimension, center, radius) {
                Ok(_) => Text::translate("big_nukes.blast.started", &[Text::literal(radius.to_string())]),
                Err(why) => refused(why),
            },
            None => usage(),
        },
        Some(&"maxpower") => maxpower(game, &words[2..]),
        // A command doesn't run inside a player's own: the next server tick gives the book.
        Some(&"guide") => {
            lock(&GUIDES).push(player.name.clone());
            Text::translate("big_nukes.guide.given", &[])
        }
        _ => usage(),
    };
    game.native_feedback(reply);
}

/// `/bignukes maxpower [limit]`: tells the operator's limit, or sets it to one
/// of the steps and saves it at once.
fn maxpower(game: &mut Game<'_>, args: &[&str]) -> Text {
    let n = |v: u32| Text::literal(v.to_string());
    let (Some(asked), Some(mut guard)) = (rules::parse_maxpower(args), state(game)) else {
        let steps: Vec<String> = rules::MAXPOWER_STEPS.iter().map(u32::to_string).collect();
        return Text::translate("big_nukes.maxpower.usage", &[Text::literal(steps.join("|"))]);
    };
    let st = guard.as_mut().expect("opened");
    match asked {
        Some(limit) => {
            st.saved.maxpower = limit;
            save(st);
            log::info!("[big_nukes] the operator's limit is now radius {limit}");
            Text::translate("big_nukes.maxpower.set", &[n(limit)])
        }
        None => Text::translate("big_nukes.maxpower.current", &[n(st.saved.maxpower)]),
    }
}

/// Starts a blast of `radius` at `center`, within the operator's limit: its
/// wave, saved at once, and the news to the players of its dimension. The
/// command and the bombs both go through here.
pub(in crate::game) fn start_blast(
    game: &mut Game<'_>,
    dimension: &str,
    center: [i32; 3],
    radius: u32,
) -> Result<u64, rules::Refused> {
    start(game, dimension, |saved| saved.fire(dimension, center, radius))
}

/// A blast in `dimension` that `fire` starts in the world's record: saved at
/// once, and its news to the players there.
fn start(
    game: &mut Game<'_>,
    dimension: &str,
    fire: impl FnOnce(&mut rules::Saved) -> Result<u64, rules::Refused>,
) -> Result<u64, rules::Refused> {
    let Some(mut guard) = state(game) else {
        return Err(rules::Refused::Dimension);
    };
    let st = guard.as_mut().expect("opened");
    let id = fire(&mut st.saved)?;
    save(st);
    let news: Vec<rules::News> = st.saved.waves.news(dimension, st.saved.tick).into_iter().filter(|n| n.id == id).collect();
    drop(guard);
    tell_dimension(game, dimension, &news);
    if let Some(n) = news.first() {
        log::info!("[big_nukes] blast {id}: radius {} at {:?} in {dimension}", n.radius, n.center);
    }
    Ok(id)
}

/// Why a blast didn't start, for the player who asked.
pub(in crate::game) fn refused(why: rules::Refused) -> Text {
    let n = |v: u32| Text::literal(v.to_string());
    match why {
        rules::Refused::Dimension => Text::translate("big_nukes.blast.dimension", &[]),
        rules::Refused::Maxpower { radius, maxpower } => Text::translate("big_nukes.maxpower.over", &[n(radius), n(maxpower)]),
        rules::Refused::Gone => Text::translate("big_nukes.remote.gone", &[]),
    }
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

// ---- radiation in the window --------------------------------------------------------
//
// The server tells the window its zones, the mod's clock and the dose once a
// second (`rad_news`); between the messages the window counts on with its own
// game time. The Geiger counter in either hand clicks and shows the reading
// over the hotbar; in a zone the fog turns green and closes in.

/// What the window was last told of the radiation.
static TOLD: Mutex<Option<rules::Told>> = Mutex::new(None);
/// The window's ticks, for the clicks' rolls, and the game time of the last one:
/// a clock that stands (single player paused) doesn't click.
static CLICKS: Mutex<(u64, i64)> = Mutex::new((0, i64::MIN));
/// The zone's fog as shown, and when: it eases toward the background's, so
/// crossing into a zone or out of one doesn't jump.
static FOG: Mutex<Option<(Instant, f64)>> = Mutex::new(None);

/// The radiation's message, in the window. One with nothing to tell lets go.
pub(in crate::game) fn told(game: &mut Game<'_>, channel: &str, data: &[u8]) {
    if channel != rules::RAD_CHANNEL || !game.native_on(rules::ID) {
        return;
    }
    let Some(clock) = game.native_clock() else { return };
    match serde_json::from_slice::<rules::Rad>(data) {
        Ok(rad) => *lock(&TOLD) = (!rad.is_empty()).then(|| rules::Told { rad, at: clock_ticks(clock.game_time) }),
        Err(e) => log::warn!("[big_nukes] the radiation's message: {e}"),
    }
}

/// The window's level goes (another dimension, or the world left): what it was
/// told goes with it. The next message comes within a second.
pub(in crate::game) fn forget_rad() {
    *lock(&TOLD) = None;
}

/// A game time as the rules count it: never below 0.
fn clock_ticks(game_time: i64) -> u64 {
    game_time.max(0) as u64
}

/// The background at `pos` in the window's dimension, by what it was told.
fn background_here(game: &Game<'_>, pos: [f64; 3]) -> f64 {
    let (Some(player), Some(clock)) = (game.native_player(), game.native_clock()) else {
        return 0.0;
    };
    lock(&TOLD).as_ref().map_or(0.0, |t| t.background(player.dimension, pos, clock_ticks(clock.game_time)))
}

/// The Geiger counter's reading when the player holds one in either hand: the
/// background at their eyes, their dose and where the eyes are.
fn reading(game: &Game<'_>) -> Option<(f64, f64, [f64; 3])> {
    let holds = [EquipmentSlot::MainHand, EquipmentSlot::OffHand]
        .into_iter()
        .any(|slot| game.native_equipment(slot).is_some_and(|s| &*s.item == rules::GEIGER_COUNTER));
    if !holds {
        return None;
    }
    let player = game.native_player()?;
    let eyes = [player.x, player.y + 1.62, player.z];
    let dose = lock(&TOLD).as_ref().map_or(0.0, |t| t.rad.dose);
    Some((background_here(game, eyes), dose, eyes))
}

/// The window's tick: the counter clicks by the background, at most once a tick.
pub(in crate::game) fn client_tick(game: &mut Game<'_>) {
    if !game.native_window() || !game.native_on(rules::ID) {
        return;
    }
    let Some(clock) = game.native_clock() else { return };
    let tick = {
        let mut clicks = lock(&CLICKS);
        let standing = clicks.1 == clock.game_time;
        clicks.0 += 1;
        clicks.1 = clock.game_time;
        if standing {
            return;
        }
        clicks.0
    };
    let Some((background, _, eyes)) = reading(game) else { return };
    if rules::clicks(background, tick) {
        game.native_play_local_sound(rules::GEIGER_CLICK, eyes, 0.6, rules::click_pitch(tick), 0);
    }
}

/// The counter's reading over the hotbar, coloured by the danger. With the HUD:
/// F1 hides it, and a screen covers it.
pub(in crate::game) fn hud(game: &Game<'_>, layer: &str, batch: &mut HudBatch) {
    if layer != "minecraft:hotbar" || !game.native_on(rules::ID) {
        return;
    }
    let Some((background, dose, _)) = reading(game) else { return };
    let (bg, dose_text) = rules::reading_numbers(background, dose);
    let line = lang::tr("big_nukes.geiger.reading", &[&bg, &dose_text]);
    let x = ((batch.width - game.native_text_width(&line)) / 2.0).round();
    game.native_text(batch, &line, [x, batch.height - READING_Y], rules::reading_colour(background), true);
}

/// The reading's line, this many GUI pixels over the bottom: above the hotbar,
/// the health and food rows, the item's name and the action bar.
const READING_Y: f32 = 84.0;

/// The world's fog, every frame: a zone's turns it green and brings it in, by
/// the background at the camera, eased over time. Only the fog of the open air:
/// water, lava and powder snow keep their own.
pub(in crate::game) fn fog(game: &Game<'_>, camera: [f64; 3], fog_type: FogType, fog: &mut FogData) {
    if !game.native_on(rules::ID) {
        return;
    }
    let target = rules::fog_strength(background_here(game, camera));
    let now = Instant::now();
    let shown = {
        let mut eased = lock(&FOG);
        let shown = match *eased {
            Some((then, shown)) => rules::ease(shown, target, now.duration_since(then).as_secs_f64()),
            None => target,
        };
        *eased = Some((now, shown));
        shown
    };
    if fog_type != FogType::Atmospheric {
        return;
    }
    if let Some((color, start, end)) = rules::tint_fog(shown, fog.color, fog.environmental_start, fog.environmental_end) {
        fog.color = color;
        fog.environmental_start = start;
        fog.environmental_end = end;
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
    // Without the assets half there is no damage type, effect, suit or anti-rad:
    // the radiation stays quiet rather than hurting with nothing to show or stop it.
    let (sick, rad) = if st.saved.tick % rules::SECOND == 0 && assets_here(game) {
        st.dirty |= st.saved.prune_zones(st.saved.tick);
        (sick(game, st), rad_news(game, st))
    } else {
        (Vec::new(), Vec::new())
    };
    if st.dirty && st.saved.tick % SAVE_EVERY == 0 {
        save(st);
    }
    let wake = std::mem::take(&mut st.wake);
    let second = st.saved.tick % 20 == 0;
    drop(guard);
    if second {
        known_damage_type(game);
    }
    // The commands run with the state let go: a death they cause is heard at once.
    for (name, stage, damage) in sick {
        sicken(game, &name, stage, damage);
    }
    for (player, bytes) in rad {
        if let Err(e) = game.native_net_send_to_player(player, rules::RAD_CHANNEL, &bytes) {
            log::warn!("[big_nukes] telling a player of the radiation: {e}");
        }
    }
    // A level nobody has entered since the world opened isn't open, and its
    // forced chunks don't tick: a command run in it opens it, so a blast there
    // works at once in the chunks an operator keeps loaded. The rest of its
    // wave waits for its chunks to load, as anywhere else.
    for dimension in wake {
        game.native_run_command(&format!("execute in {dimension} run forceload query"));
    }
    for name in std::mem::take(&mut *lock(&GUIDES)) {
        game.native_run_command(&rules::guide_give(&name));
    }
}

/// `/damage` checks a type against the command registry the server exported,
/// which holds only vanilla's damage types (Horizon 66f3981 adds a content mod's
/// items, blocks and effects there, not its damage types), so the radiation's
/// type is added once the world's registries are built. Only with the assets
/// half, which brings the type itself. This is the game's own table, not an API:
/// when it changes, the build breaks, not the game.
/// Whether the assets half is installed: it brings the marker loot table.
pub(in crate::game) fn assets_here(game: &Game<'_>) -> bool {
    world_items!(game).has_loot_table(rules::ASSETS_MARKER)
}

fn known_damage_type(game: &mut Game<'_>) {
    if !assets_here(game) {
        return;
    }
    if let Some(r) = game.ui.registries.by_registry.get_mut("minecraft:damage_type")
        && !r.ids.iter().any(|id| id == rules::RADIATION)
    {
        r.ids.push(rules::RADIATION.to_owned());
        r.ids.sort();
        log::info!("[big_nukes] {} is known to /damage", rules::RADIATION);
    }
}

/// The end of a level's tick: its waves move on, and some of their work is done.
pub(in crate::game) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    if !game.native_on(rules::ID) {
        return;
    }
    irradiate_mobs(game, dimension);
    silo::level_tick(game, dimension);
    let Some(mut guard) = state(game) else { return };
    let st = guard.as_mut().expect("opened");
    effects(game, st, dimension);
    if st.saved.waves.idle(dimension) {
        return;
    }
    let loaded = lock(&LOADED).as_ref().and_then(|m| m.get(dimension)).cloned().unwrap_or_default();
    st.dirty |= st.saved.waves.advance(dimension, st.saved.tick, &|c| loaded.contains(&c));
    let ash = *st.ash.get_or_insert_with(|| has_ash(game, dimension));
    let started = Instant::now();
    let warned = &mut st.warned;
    let mut worked = Vec::new();
    // The bombs a wave reaches go off as it gets to them: read before the edits take them.
    let placed = st.saved.placed.clone();
    let mut reached: Vec<[i32; 3]> = Vec::new();
    let done = st.saved.waves.work(dimension, BUDGET, &|| started.elapsed(), &mut |blast, chunk| {
        if !loaded.contains(&chunk) {
            return false;
        }
        worked.push(chunk);
        for (pos, kind) in rules::reached(&placed, blast, chunk) {
            if rules::BombKind::of_block(&game.native_block(pos)) == Some(kind) {
                reached.push(pos);
            }
        }
        for edit in rules::plan_chunk(blast, chunk) {
            if edit.runs.is_empty() {
                continue;
            }
            let edit = if ash { edit } else { edit.without_ash() };
            if let Err(e) = game.native_edit_blocks(&blast.dimension, &native(edit))
                && warned.insert(blast.id)
            {
                log::warn!("[big_nukes] blast {}: an edit was refused: {e}", blast.id);
            }
        }
        true
    });
    st.dirty |= done > 0;
    // A bulk edit tells no one of the blocks it changes: the bombs with a
    // receiver that a blast took are forgotten here. A bomb at a blast's center
    // that its crater missed (above the Nether's roof) goes now, so a bomb set
    // off from afar can't stay to be linked and set off again.
    let mut news = Vec::new();
    for pos in reached {
        if let Ok(id) = st.saved.set_off(dimension, pos) {
            log::info!("[big_nukes] blast {id}: set off by another at {pos:?} in {dimension}");
            take_bomb(game, dimension, pos);
            news.extend(st.saved.waves.news(dimension, st.saved.tick).into_iter().filter(|n| n.id == id));
        }
    }
    if !news.is_empty() {
        save(st);
    }
    for chunk in worked {
        for pos in st.saved.placed_in(dimension, chunk) {
            if rules::BombKind::of_block(&game.native_block(pos)).is_none() {
                st.dirty |= st.saved.unplace(dimension, pos);
            }
        }
        for center in st.saved.waves.centers_in(dimension, chunk) {
            if rules::BombKind::of_block(&game.native_block(center)).is_some() {
                take_bomb(game, dimension, center);
            }
        }
        for pos in st.saved.bombs_in(dimension, chunk) {
            if rules::BombKind::of_block(&game.native_block(pos)).is_none() {
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
    tell_dimension(game, dimension, &news);
}

/// A bomb gone off where it stands: its detonator with it, as its blast takes it.
fn take_bomb(game: &mut Game<'_>, dimension: &str, pos: [i32; 3]) {
    game.native_with_mod_block_entity(pos, |e| e.items.iter_mut().for_each(|s| *s = None));
    if let Some(air) = game.pack.assets.registry.default_state("minecraft:air") {
        game.native_set_block(dimension, pos, air, 3);
    }
}

/// Whether the ash block is in the game: it comes with the assets half. An edit with no runs reads its palette as
/// any other does, and changes nothing; an entry that doesn't read refuses it.
fn has_ash(game: &mut Game<'_>, dimension: &str) -> bool {
    let probe = NativeBlockEdit { palette: vec![rules::ASH.into()], ..Default::default() };
    match game.native_edit_blocks(dimension, &probe) {
        Ok(_) => true,
        Err(e) => {
            log::info!("[big_nukes] no ash ({e}): the scorch is coarse dirt");
            false
        }
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
    forget_rad();
    lock(&PLACED).clear();
    lock(&DIED).clear();
    lock(&ANTI_RAD).clear();
    silo::forget();
}

// ---- the machines ---------------------------------------------------------------
//
// The hooks the machines share hand each block to its own functions, by its id.

/// The blocks with a block entity: the machines and the bombs. Its id is its block's.
fn has_entity(block: &str) -> bool {
    matches!(block, rules::CENTRIFUGE | rules::ASSEMBLY_TABLE)
        || rules::BombKind::of_block(block).is_some()
        || silo::has_entity(block)
}

pub(in crate::game) fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if has_entity(block) && entity.is_none() {
        *entity = Some((block.to_owned(), Compound::default()));
    }
}

pub(in crate::game) fn block_entity_ticks(block: &str, ticks: &mut bool) {
    if has_entity(block) {
        *ticks = true;
    }
}

pub(in crate::game) fn container(block: &str, container: &mut Option<ModContainer>) {
    match block {
        rules::CENTRIFUGE => *container = Some(centrifuge_container()),
        rules::ASSEMBLY_TABLE => *container = Some(assembly_container()),
        block if silo::has_entity(block) => *container = Some(silo::container(block)),
        _ => {
            if let Some(kind) = rules::BombKind::of_block(block) {
                *container = Some(bomb_container(kind));
            }
        }
    }
}

pub(in crate::game) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if !game.native_on(rules::ID) {
        return;
    }
    match entity.id.as_str() {
        rules::CENTRIFUGE => centrifuge_tick(game, entity),
        rules::ASSEMBLY_TABLE => assembly_tick(entity),
        id if rules::BombKind::of_block(id).is_some() => bomb_tick(game, pos, entity),
        id if silo::has_entity(id) => silo::tick(game, pos, entity),
        _ => {}
    }
}

pub(in crate::game) fn menu_button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    if !game.native_on(rules::ID) {
        return;
    }
    match entity.id.as_str() {
        rules::ASSEMBLY_TABLE => assembly_button(game, entity, id),
        block if keypad_bomb(block) => keypad_button(game, pos, entity, id),
        _ => {}
    }
}

pub(in crate::game) fn screen(block: &str, screen: &mut Option<NativeScreen>) {
    match block {
        rules::CENTRIFUGE => *screen = Some(centrifuge_screen()),
        rules::ASSEMBLY_TABLE => *screen = Some(assembly_screen()),
        block if keypad_bomb(block) => *screen = Some(keypad_screen()),
        rules::LAUNCH_PAD => *screen = Some(silo::pad_screen()),
        rules::LAUNCH_CONSOLE => *screen = Some(silo::console_screen()),
        _ => {}
    }
}

pub(in crate::game) fn screen_draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    match block {
        rules::CENTRIFUGE => centrifuge_screen_draw(game, data, batch, origin),
        rules::ASSEMBLY_TABLE => assembly_screen_draw(game, data, batch, origin),
        block if keypad_bomb(block) => keypad_draw(game, block, data, batch, origin),
        rules::LAUNCH_PAD => silo::pad_draw(game, batch, origin),
        rules::LAUNCH_CONSOLE => silo::console_draw(game, data, batch, origin),
        _ => {}
    }
}

pub(in crate::game) fn screen_click(block: &str, at: [f32; 2], right: bool, button: &mut Option<i32>) {
    match block {
        rules::ASSEMBLY_TABLE => assembly_screen_click(at, right, button),
        block if keypad_bomb(block) => keypad_click(at, right, button),
        rules::LAUNCH_CONSOLE => silo::console_click(at, right),
        _ => {}
    }
}

/// The screens' labels.
const LABEL: u32 = 0x40_4040;

// ---- the centrifuge --------------------------------------------------------------
//
// Its state lives in its block entity, so it moves with the chunk's save: `nbt`
// holds the rules' `Centrifuge` as JSON, `items` the three slots, `data` what its
// screen shows.

/// A furnace's sides: hoppers put raw uranium in from above and fuel from the
/// sides, and take enriched uranium from below.
fn centrifuge_container() -> ModContainer {
    let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| vec![rules::FUEL_SLOT]);
    faces[Direction::Down as usize] = vec![rules::OUTPUT_SLOT];
    faces[Direction::Up as usize] = vec![rules::INPUT_SLOT];
    ModContainer {
        slots: rules::CENTRIFUGE_SLOTS,
        faces,
        inputs: vec![rules::INPUT_SLOT, rules::FUEL_SLOT],
        outputs: vec![rules::OUTPUT_SLOT],
        menu: Some(rules::CENTRIFUGE.to_owned()),
    }
}

/// What a hopper may put in: raw uranium in its slot, fuel in the other.
pub(in crate::game) fn accepts(game: &Game<'_>, entity: &ModBlockEntity, slot: usize, stack: &ItemStack, cancel: &mut bool) {
    if entity.id == rules::LAUNCH_PAD && !*cancel {
        *cancel = !rules::pad_takes(slot, &stack.item);
        return;
    }
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

fn centrifuge_tick(game: &mut Game<'_>, entity: &mut ModBlockEntity) {
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
    // Nothing moves into or out of a bomb's hidden slot, nor a console's or a beacon's.
    if rules::BombKind::of_block(&entity.id).is_some() || matches!(entity.id.as_str(), rules::LAUNCH_CONSOLE | rules::STRIKE_BEACON) {
        *handled = true;
        return;
    }
    if entity.id == rules::LAUNCH_PAD {
        silo::pad_quick_move(game, entity, container_slots, slot, stack, handled);
        return;
    }
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

fn centrifuge_screen() -> NativeScreen {
    NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some(INPUT_AT), Some(FUEL_AT), Some(OUTPUT_AT)],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: true,
    }
}

fn centrifuge_screen_draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
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
    let title = lang::get("big_nukes.screen.centrifuge");
    let width = game.native_text_width(&title);
    game.native_text(batch, &title, [x + (176.0 - width) / 2.0, y + 6.0], LABEL, false);
    game.native_text(batch, &lang::get("container.inventory"), [x + 8.0, y + 72.0], LABEL, false);
}

// ---- the assembly table ----------------------------------------------------------
//
// Nothing of its own but its slots: the grid's recipe is read again each tick, for
// the screen's data slot, and the button makes one of it.

/// Hoppers take the made items from below and put nothing in.
fn assembly_container() -> ModContainer {
    let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| Vec::new());
    faces[Direction::Down as usize] = vec![rules::ASSEMBLY_OUTPUT_SLOT];
    ModContainer {
        slots: rules::ASSEMBLY_SLOTS,
        faces,
        inputs: Vec::new(),
        outputs: vec![rules::ASSEMBLY_OUTPUT_SLOT],
        menu: Some(rules::ASSEMBLY_TABLE.to_owned()),
    }
}

/// The grid's items and counts, for the rules.
fn assembly_grid(entity: &ModBlockEntity) -> Vec<Option<(&str, u32)>> {
    (0..rules::GRID_SLOTS).map(|i| entity.items.get(i).and_then(Option::as_ref).map(|s| (&*s.item, s.count))).collect()
}

fn assembly_tick(entity: &mut ModBlockEntity) {
    let data = vec![rules::assembly_data(&assembly_grid(entity))];
    if entity.data != data {
        entity.data = data;
    }
}

/// Assemble: the recipe's parts out of the grid, one of its result into the output.
/// Nothing happens without a recipe in the grid or room in the output.
fn assembly_button(game: &Game<'_>, entity: &mut ModBlockEntity, id: i32) {
    if id != rules::ASSEMBLE_BUTTON {
        return;
    }
    let output = entity.items.get(rules::ASSEMBLY_OUTPUT_SLOT).and_then(Option::as_ref).map(|s| (&*s.item, s.count));
    let most = |item: &str| game.native_max_stack_size(item).unwrap_or(64);
    let Some(done) = rules::assemble(&assembly_grid(entity), output, &most) else {
        return;
    };
    if entity.items.len() < rules::ASSEMBLY_SLOTS {
        entity.items.resize(rules::ASSEMBLY_SLOTS, None);
    }
    for (slot, n) in done.take {
        if let Some(stack) = entity.items[slot].as_mut() {
            stack.count -= n;
            if stack.count == 0 {
                entity.items[slot] = None;
            }
        }
    }
    match entity.items[rules::ASSEMBLY_OUTPUT_SLOT].as_mut() {
        Some(out) => out.count += 1,
        None => entity.items[rules::ASSEMBLY_OUTPUT_SLOT] = Some(stack_of(done.result)),
    }
    assembly_tick(entity);
}

// ---- the assembly table's screen -------------------------------------------------
//
// The game draws the panel and the slots' frames; the mod draws the labels, the
// line of what the grid makes and the button.

const ASSEMBLY_WIDTH: f32 = 176.0;
const ASSEMBLY_HEIGHT: f32 = 180.0;
/// The grid's top left slot; the others are 18 pixels apart.
const GRID_AT: [f32; 2] = [30.0, 17.0];
const ASSEMBLY_OUTPUT_AT: [f32; 2] = [124.0, 21.0];
/// The button: x, y, width, height.
const ASSEMBLE_AT: [f32; 4] = [96.0, 47.0, 72.0, 18.0];
const ASSEMBLY_LINE_Y: f32 = 76.0;
const ASSEMBLY_INVENTORY_Y: f32 = 98.0;

fn assembly_screen() -> NativeScreen {
    let mut slots: Vec<Option<[f32; 2]>> = (0..rules::GRID_SLOTS)
        .map(|i| Some([GRID_AT[0] + 18.0 * (i % 3) as f32, GRID_AT[1] + 18.0 * (i / 3) as f32]))
        .collect();
    slots.push(Some(ASSEMBLY_OUTPUT_AT));
    NativeScreen {
        width: ASSEMBLY_WIDTH,
        height: ASSEMBLY_HEIGHT,
        slots,
        inventory_y: ASSEMBLY_INVENTORY_Y,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

fn assembly_screen_draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let [x, y] = origin;
    let title = lang::get("big_nukes.screen.assembly_table");
    let width = game.native_text_width(&title);
    game.native_text(batch, &title, [x + (ASSEMBLY_WIDTH - width) / 2.0, y + 6.0], LABEL, false);
    game.native_text(batch, &lang::get("container.inventory"), [x + 8.0, y + ASSEMBLY_INVENTORY_Y - 11.0], LABEL, false);
    let recipe = rules::assembly_of_data(data.first().copied().unwrap_or(0));
    let line = match recipe {
        Some(r) => lang::tr("big_nukes.screen.assembly_makes", &[&lang::get(&rules::name_key(r.result))]),
        None => lang::get("big_nukes.screen.assembly_hint"),
    };
    let width = game.native_text_width(&line);
    game.native_text(batch, &line, [x + (ASSEMBLY_WIDTH - width) / 2.0, y + ASSEMBLY_LINE_Y], LABEL, false);
    // A plain button, lit while the grid holds a recipe.
    let [bx, by, bw, bh] = ASSEMBLE_AT;
    let (body, text) = if recipe.is_some() { (0xFF8B_8B8B, 0xFF_FFFF) } else { (0xFF5A_5A5A, 0xA0_A0A0) };
    game.native_fill(batch, [x + bx, y + by, bw, bh], 0xFF00_0000);
    game.native_fill(batch, [x + bx + 1.0, y + by + 1.0, bw - 2.0, bh - 2.0], 0xFFC6_C6C6);
    game.native_fill(batch, [x + bx + 2.0, y + by + 2.0, bw - 3.0, bh - 3.0], body);
    let label = lang::get("big_nukes.screen.assemble");
    let width = game.native_text_width(&label);
    game.native_text(batch, &label, [x + bx + (bw - width) / 2.0, y + by + (bh - 8.0) / 2.0], text, true);
}

fn assembly_screen_click(at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let [bx, by, bw, bh] = ASSEMBLE_AT;
    if !right && (bx..bx + bw).contains(&at[0]) && (by..by + bh).contains(&at[1]) {
        *button = Some(rules::ASSEMBLE_BUTTON);
    }
}

// ---- the bombs -------------------------------------------------------------------
//
// A bomb's state lives in its block entity: `nbt` holds the rules' `Bomb` as
// JSON, its one hidden slot the detonator's item, and `data` what the keypad's
// screen shows. A right click puts a detonator in, lights a fuse or opens the
// keypad; the block entity's tick counts down, hisses, beeps and sets the blast
// off, taking the block with it. Its clock is `Saved.tick`.

/// Vanilla's `SoundSource.BLOCKS` and `SoundSource.PLAYERS`.
const BLOCKS: usize = 4;
const PLAYERS: usize = 7;
/// How far a bomb's refusal is told.
const TOLD_WITHIN: f64 = 64.0;

/// The player, the bomb and the tick of the last click handled on a bomb. The
/// game asks once per hand: the off hand's turn of the same click is let go,
/// so nothing goes in or is lit twice.
static LAST_CLICK: Mutex<Option<(u128, [i32; 3], u64)>> = Mutex::new(None);

/// A bomb's one slot holds its detonator, out of every hopper's reach and hidden on
/// its screen: a broken bomb drops it as a chest drops its items. The bombs that
/// take a keypad open its menu.
fn bomb_container(kind: rules::BombKind) -> ModContainer {
    ModContainer {
        slots: rules::BOMB_SLOTS,
        faces: Default::default(),
        inputs: vec![],
        outputs: vec![],
        menu: kind.takes(rules::Detonator::Keypad).then(|| rules::KEYPAD_MENU.to_owned()),
    }
}

fn bomb_key() -> JavaString {
    JavaString::from("bomb")
}

/// A bomb's state; its kind is its block's.
fn load_bomb(entity: &ModBlockEntity) -> Option<rules::Bomb> {
    let kind = rules::BombKind::of_block(&entity.id)?;
    let mut bomb = match entity.nbt.get(&bomb_key()) {
        Some(Tag::ByteArray(bytes)) => serde_json::from_slice(bytes).unwrap_or_else(|_| rules::Bomb::new(kind)),
        _ => rules::Bomb::new(kind),
    };
    bomb.kind = kind;
    Some(bomb)
}

fn store_bomb(entity: &mut ModBlockEntity, bomb: &rules::Bomb) {
    if let Ok(bytes) = serde_json::to_vec(bomb) {
        entity.nbt.insert(bomb_key(), Tag::ByteArray(bytes));
    }
}

/// The world's clock and the operator's limit.
pub(in crate::game) fn clock(game: &Game<'_>) -> Option<(u64, u32)> {
    let guard = state(game)?;
    let st = guard.as_ref()?;
    Some((st.saved.tick, st.saved.maxpower))
}

pub(in crate::game) fn middle(pos: [i32; 3]) -> [f64; 3] {
    pos.map(|v| f64::from(v) + 0.5)
}

fn sound(game: &mut Game<'_>, pos: [i32; 3], event: &str, volume: f32, pitch: f32) {
    game.native_level_play_sound(event, middle(pos), BLOCKS, volume, pitch);
}

/// A line in the chat of every player near the bomb.
pub(in crate::game) fn tell_near(game: &mut Game<'_>, dimension: &str, pos: [i32; 3], line: &Text) {
    let at = middle(pos);
    for p in game.native_server_players() {
        let far = (0..3).map(|i| (p.position[i] - at[i]).powi(2)).sum::<f64>().sqrt();
        if p.dimension == dimension && far <= TOLD_WITHIN {
            game.native_tell(p.uuid, line.clone());
        }
    }
}

fn item_name(item: &str) -> Text {
    Text::translate(&format!("item.{}", item.replace(':', ".")), &[])
}

/// Whether the keypad's menu type is there: the assets half brings it.
fn keypad_menu_known() -> bool {
    horizon_client::containers::mod_menu_type(rules::KEYPAD_MENU).is_some()
}

/// A right click on a bomb, with the main hand's item: a remote is linked, a
/// detonator goes in, flint and steel lights the fuse, and anything else opens the keypad, if
/// there is one. The furnace the bomb is built on never opens. Sneaking with
/// something else in hand lets it be used on the bomb (a block placed against it).
pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled || !game.native_on(rules::ID) || silo::right_click(game, pos, canceled) {
        return;
    }
    if rules::BombKind::of_block(&game.native_block(pos)).is_none() {
        return;
    }
    let (Some(who), Some((now, maxpower))) = (game.native_acting_player(), clock(game)) else {
        return;
    };
    *canceled = true;
    {
        let mut last = lock(&LAST_CLICK);
        if *last == Some((who.uuid, pos, now)) {
            return;
        }
        *last = Some((who.uuid, pos, now));
    }
    let Some(mut bomb) = game.native_with_mod_block_entity(pos, |e| load_bomb(e)).flatten() else {
        return;
    };
    let held = game.native_equipment(EquipmentSlot::MainHand).map(|s| s.item.to_string());
    let creative = who.game_mode == 1;
    let tell = |game: &mut Game<'_>, line: Text| {
        game.native_tell(who.uuid, line);
    };
    // A remote is linked to a bomb with a receiver, sneaking or not.
    if held.as_deref() == Some(rules::REMOTE) {
        match bomb.detonator {
            Some(rules::Detonator::Receiver) => link(game, &who, pos, bomb.kind),
            _ => tell(game, Text::translate("big_nukes.remote.no_receiver", &[])),
        }
        return;
    }
    if let Some(detonator) = held.as_deref().and_then(rules::Detonator::of_item) {
        match bomb.install(detonator) {
            Ok(()) => {
                if install(game, pos, &bomb, detonator, creative, who.uuid) && detonator == rules::Detonator::Receiver {
                    remember(game, &who.dimension, pos, bomb.kind);
                }
            }
            Err(rules::Refusal::HasOne) => tell(game, Text::translate("big_nukes.bomb.has_detonator", &[])),
            Err(rules::Refusal::FuseOnly) => tell(game, Text::translate("big_nukes.bomb.fuse_only", &[])),
        }
        return;
    }
    if held.as_deref() == Some(rules::FLINT_AND_STEEL) {
        match bomb.detonator {
            Some(rules::Detonator::Fuse) if bomb.fires_at.is_none() => {
                let radius = bomb.kind.radius();
                if radius > maxpower {
                    tell(game, refused(rules::Refused::Maxpower { radius, maxpower }));
                } else if bomb.light(now) {
                    game.native_with_mod_block_entity(pos, |e| store_bomb(e, &bomb));
                    sound(game, pos, "minecraft:item.flintandsteel.use", 1.0, 1.0);
                    sound(game, pos, "minecraft:entity.tnt.primed", 1.0, 1.0);
                    if !creative {
                        wear_flint(game, &who);
                    }
                }
            }
            // A lit fuse isn't lit again.
            Some(rules::Detonator::Fuse) => {}
            Some(_) => tell(game, Text::translate("big_nukes.bomb.not_fuse", &[])),
            None => tell(game, Text::translate("big_nukes.bomb.no_detonator", &[])),
        }
        return;
    }
    let keypad = bomb.detonator == Some(rules::Detonator::Keypad);
    if (keypad && keypad_menu_known()) || (held.is_some() && game.native_sneaking()) {
        // The game goes on: the block opens its keypad, or the item is used.
        *canceled = false;
        return;
    }
    let hint = match bomb.detonator {
        None => "big_nukes.bomb.no_detonator",
        Some(rules::Detonator::Fuse) => "big_nukes.bomb.light_fuse",
        Some(rules::Detonator::Receiver) => "big_nukes.bomb.receiver",
        Some(rules::Detonator::Keypad) => return,
    };
    tell(game, Text::translate(hint, &[]));
}

/// The detonator goes in: out of the player's hand, unless they're creative,
/// into the bomb's slot and its state. Whether it went in.
fn install(game: &mut Game<'_>, pos: [i32; 3], bomb: &rules::Bomb, detonator: rules::Detonator, creative: bool, player: u128) -> bool {
    if !creative && game.native_take_items(detonator.item(), 1) == 0 {
        return false;
    }
    let put = game.native_with_mod_block_entity(pos, |e| {
        if e.items.len() < rules::BOMB_SLOTS {
            e.items.resize(rules::BOMB_SLOTS, None);
        }
        e.items[rules::DETONATOR_SLOT] = Some(stack_of(detonator.item()));
        store_bomb(e, bomb);
    });
    if put.is_none() {
        if !creative {
            game.native_give_items(detonator.item(), 1);
        }
        return false;
    }
    sound(game, pos, "minecraft:block.iron_trapdoor.close", 1.0, 1.4);
    game.native_tell(player, Text::translate("big_nukes.bomb.installed", &[item_name(detonator.item())]));
    true
}

// ---- radiation -------------------------------------------------------------------
//
// The zones are rules (`rules::background`). A player's dose builds in their own
// tick, where their armour is in place; the sickness is given by commands at the
// end of the server's tick, where commands run; the mobs are hurt at the end of
// their level's tick, a few at a time.

/// A player's tick: once a second, the dose takes the background where they
/// stand, less what their hazmat suit keeps out, and each worn piece wears while
/// there is any to speak of. Creative players and spectators take none.
pub(in crate::game) fn player_tick(game: &mut Game<'_>) {
    if !game.native_on(rules::ID) {
        return;
    }
    let Some(who) = game.native_acting_player() else { return };
    if matches!(who.game_mode, 1 | 3) {
        return;
    }
    let worn = [EquipmentSlot::Head, EquipmentSlot::Chest, EquipmentSlot::Legs, EquipmentSlot::Feet]
        .map(|slot| game.native_equipment(slot));
    let suit: [bool; 4] = std::array::from_fn(|i| worn[i].as_ref().is_some_and(|s| &*s.item == rules::HAZMAT[i].0));
    let Some(mut guard) = state(game) else { return };
    let st = guard.as_mut().expect("opened");
    let had = st.saved.doses.contains_key(&rules::dose_key(who.uuid));
    let protection = rules::protection(suit[0], suit[1], suit[2], suit[3]);
    let Some(background) = st.saved.expose(who.uuid, &who.dimension, who.position, protection) else {
        return;
    };
    st.dirty |= had || background > 0.0;
    if background >= rules::WEARS_FROM {
        // Straight into the queue: `queue_held` would open the state again.
        let now = st.saved.tick;
        let mut held = lock(&HELD);
        for (i, _) in suit.iter().enumerate().filter(|(_, on)| **on) {
            held.push((who.uuid, Held::Armour { slot: 103 - i as i32 }, now));
        }
    }
}

/// A player died: their dose goes. Heard in a queue, as the state may be locked.
pub(in crate::game) fn player_died(game: &Game<'_>, player: u128) {
    if game.native_on(rules::ID) {
        lock(&DIED).push(player);
    }
}

/// The anti-radiation drug: `amount` rad off the player's dose, the next time the
/// state opens. The drug's consume hook calls it.
pub(in crate::game) fn anti_radiation(uuid: u128, amount: f64) {
    lock(&ANTI_RAD).push((uuid, amount));
}

/// The sick players on the server, once a second: their names, stages and the
/// damage due now. Creative players, spectators and the dead are left alone.
fn sick(game: &Game<'_>, st: &mut State) -> Vec<(String, u8, Option<u32>)> {
    if st.saved.doses.is_empty() {
        return Vec::new();
    }
    let mut sick = Vec::new();
    for p in game.native_server_players() {
        if matches!(p.game_mode, 1 | 3) || p.health <= 0.0 {
            continue;
        }
        if let Some((stage, damage)) = st.saved.sicken(p.uuid) {
            st.dirty |= damage.is_some();
            sick.push((p.name, stage, damage));
        }
    }
    sick
}

/// The radiation each player's window is told of, once a second: the zones of
/// their dimension and their dose, for the Geiger counter and the fog. Only to
/// the players with something to tell, and once more to those who had and now
/// have nothing, so their window lets go.
fn rad_news(game: &Game<'_>, st: &mut State) -> Vec<(u128, Vec<u8>)> {
    if st.saved.zones.is_empty() && st.saved.doses.is_empty() && st.told.is_empty() {
        return Vec::new();
    }
    let mut news = Vec::new();
    for p in game.native_server_players() {
        let rad = st.saved.rad_for(p.uuid, &p.dimension, p.position);
        if rad.is_empty() {
            if !st.told.remove(&p.uuid) {
                continue;
            }
        } else {
            st.told.insert(p.uuid);
        }
        match serde_json::to_vec(&rad) {
            Ok(bytes) => news.push((p.uuid, bytes)),
            Err(e) => log::warn!("[big_nukes] the radiation's message: {e}"),
        }
    }
    news
}

/// A sick player's effects, kept on for a few seconds at a time, and the damage due.
/// The mod's own effect only marks the stage: without the assets half its command fails, and nothing else.
fn sicken(game: &mut Game<'_>, name: &str, stage: u8, damage: Option<u32>) {
    game.native_run_command(&format!("effect give {name} minecraft:weakness 3 0 true"));
    game.native_run_command(&format!("effect give {name} minecraft:hunger 3 0 true"));
    if stage >= 2 {
        game.native_run_command(&format!("effect give {name} minecraft:nausea 5 0 true"));
    }
    game.native_run_command(&format!("effect give {name} {} 3 {} true", rules::RADIATION, stage - 1));
    if let Some(amount) = damage {
        hurt(game, "", name, amount);
    }
}

/// Radiation damage to `target` (a name or a UUID), `prefix` going before the
/// command (`execute in <dimension> run `). A `/damage` that fails did no damage,
/// so when the mod's damage type is refused (the game has no
/// `big_nukes:radiation` without the assets half, or its commands don't know it)
/// the same damage goes as the world border's; when the target couldn't be hurt,
/// that fails too.
fn hurt(game: &mut Game<'_>, prefix: &str, target: &str, amount: u32) {
    if !game.native_run_command(&format!("{prefix}damage {target} {amount} {}", rules::RADIATION)) {
        game.native_run_command(&format!("{prefix}damage {target} {amount} minecraft:outside_border"));
    }
}

/// The most mobs hurt in a dimension in a second; more take turns.
const MOBS_PER_PASS: usize = 64;

/// The mobs of `dimension` in its zones, once a second: each takes damage by the
/// background where it stands, up to `MOBS_PER_PASS` of them, the next ones on
/// the next pass.
fn irradiate_mobs(game: &mut Game<'_>, dimension: &str) {
    if !assets_here(game) {
        return;
    }
    let hurt_now: Vec<(u32, u32)> = {
        let Some(mut guard) = state(game) else { return };
        let st = guard.as_mut().expect("opened");
        if !st.saved.zones.iter().any(|z| z.dimension == dimension) {
            return;
        }
        let now = st.saved.tick;
        let (next, cursor) = st.mobs.entry(dimension.to_owned()).or_insert((0, 0));
        if now < *next {
            return;
        }
        *next = now + rules::SECOND;
        let zones = &st.saved.zones;
        let mut in_zone: Vec<(u32, u32)> = game
            .native_mobs(dimension)
            .iter()
            .filter_map(|m| rules::mob_harm(rules::background(zones, dimension, m.pos, now)).map(|n| (m.id, n)))
            .collect();
        if in_zone.is_empty() {
            return;
        }
        let from = *cursor % in_zone.len();
        in_zone.rotate_left(from);
        in_zone.truncate(MOBS_PER_PASS);
        *cursor = from + in_zone.len();
        in_zone
    };
    let prefix = format!("execute in {dimension} run ");
    for (id, amount) in hurt_now {
        let Some(uuid) = game.native_entity(id).and_then(|e| game.native_entity_uuid(e)) else {
            continue;
        };
        hurt(game, &prefix, &rules::uuid_text(uuid), amount);
    }
}

// ---- the held stack --------------------------------------------------------------
//
// A player's hook can't change the stack in hand: the host has no entity handle
// there (`native_acting_entity` is 0 for it). So a change is queued, and the next
// server tick of that player's selected slot (`native_inventory_tick`, where
// `native_carried_stack` changes a stack in place) makes it.

/// A change to the stack in a player's main hand, or in an armour slot.
#[derive(Clone, Debug)]
enum Held {
    /// The flint and steel loses a use, or breaks on its last.
    Wear,
    /// The remote is linked to the bomb of `kind` at `pos` in `dimension`.
    Link { dimension: String, pos: [i32; 3], kind: rules::BombKind },
    /// The hazmat piece in armour slot `slot` (100 feet to 103 head) loses a
    /// point of durability, or breaks on its last.
    Armour { slot: i32 },
}

impl Held {
    /// Whether the stack in `slot` takes the change: the hand's changes go to the
    /// selected slot, a piece's wear to its own slot.
    fn fits(&self, slot: i32, selected: bool) -> bool {
        match self {
            Held::Wear | Held::Link { .. } => selected,
            Held::Armour { slot: s } => *s == slot,
        }
    }
}

/// Queued changes: the player, the change and the tick it was asked at.
static HELD: Mutex<Vec<(u128, Held, u64)>> = Mutex::new(Vec::new());
/// A change the player's hand didn't take this long (the slot changed) is dropped.
const HELD_TICKS: u64 = 20;

fn queue_held(game: &Game<'_>, who: &NativeServerPlayer, change: Held) {
    let now = clock(game).map_or(0, |(now, _)| now);
    lock(&HELD).push((who.uuid, change, now));
}

/// One use off the flint and steel in the acting player's main hand.
fn wear_flint(game: &mut Game<'_>, who: &NativeServerPlayer) {
    queue_held(game, who, Held::Wear);
}

/// A server tick of a stack a player carries: the held one takes the player's
/// queued changes.
pub(in crate::game) fn inventory_tick(game: &mut Game<'_>, slot: i32, selected: bool) {
    if !(selected || rules::hazmat_in(slot).is_some()) || lock(&HELD).is_empty() {
        return;
    }
    let (Some(who), Some((now, _))) = (game.native_acting_player(), clock(game)) else {
        return;
    };
    let mine: Vec<Held> = {
        let mut held = lock(&HELD);
        held.retain(|(_, _, at)| now.saturating_sub(*at) <= HELD_TICKS);
        let (mine, rest): (Vec<_>, Vec<_>) =
            held.drain(..).partition(|(uuid, change, _)| *uuid == who.uuid && change.fits(slot, selected));
        *held = rest;
        mine.into_iter().map(|(_, change, _)| change).collect()
    };
    for change in mine {
        let Some(mut stack) = game.native_carried_stack(slot).cloned() else {
            continue;
        };
        match change {
            Held::Wear if &*stack.item == rules::FLINT_AND_STEEL => {
                match rules::worn(stack.damage) {
                    Some(damage) => stack.damage = damage,
                    None => {
                        stack.count = 0;
                        game.native_level_play_sound("minecraft:entity.item.break", who.position, PLAYERS, 0.8, 1.0);
                    }
                }
            }
            Held::Link { dimension, pos, kind } if &*stack.item == rules::REMOTE => {
                let value = Tag::compound([
                    ("dimension", Tag::String(JavaString::from(dimension.as_str()))),
                    ("pos", Tag::IntArray(pos.to_vec())),
                ]);
                if let Err(e) = game.native_set_component(&mut stack, rules::LINK, value) {
                    log::warn!("[big_nukes] linking a remote: {e}");
                    continue;
                }
                sound(game, pos, "minecraft:block.note_block.pling", 0.8, 2.0);
                game.native_tell(who.uuid, Text::translate("big_nukes.remote.linked", &[block_name(kind.id()), place(pos)]));
            }
            Held::Armour { slot: s } if rules::hazmat_in(s) == Some(&*stack.item) => {
                // The piece's own durability when the game knows it, as the assets half declares it.
                let max = world_items!(game).get(&stack.item).map_or(0, |i| i.max_damage);
                let max = if max > 0 { max } else { rules::HAZMAT_DURABILITY };
                match rules::wear(stack.damage, max) {
                    Some(damage) => stack.damage = damage,
                    None => {
                        stack.count = 0;
                        game.native_level_play_sound("minecraft:entity.item.break", who.position, PLAYERS, 0.8, 1.0);
                    }
                }
            }
            _ => continue,
        }
        if let Some(carried) = game.native_carried_stack(slot) {
            *carried = stack;
        }
    }
}

/// A bomb's tick: the hiss and smoke of a lit fuse, the beeps of a countdown,
/// and the blast at its end.
fn bomb_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let (Some(mut bomb), Some((now, maxpower))) = (load_bomb(entity), clock(game)) else {
        return;
    };
    let was = bomb.clone();
    let dimension = game.native_dimension();
    if let Some(left) = bomb.left(now) {
        counting(game, pos, &bomb, left);
    }
    if bomb.tick(now) {
        let radius = bomb.kind.radius();
        let start = if radius > maxpower {
            Err(rules::Refused::Maxpower { radius, maxpower })
        } else {
            start_blast(game, dimension, pos, radius)
        };
        match start {
            Ok(_) => {
                // The bomb goes with its blast, and its detonator with it.
                entity.items.iter_mut().for_each(|s| *s = None);
                if let Some(air) = game.pack.assets.registry.default_state("minecraft:air") {
                    game.native_set_block(dimension, pos, air, 3);
                }
                return;
            }
            Err(why) => tell_near(game, dimension, pos, &refused(why)),
        }
    }
    let data = bomb.data(now).to_vec();
    if entity.data != data {
        entity.data = data;
    }
    if bomb != was {
        store_bomb(entity, &bomb);
    }
}

/// What a counting bomb shows: a lit fuse smokes and sparks on top, a keypad
/// beeps each second, higher as it ends.
fn counting(game: &mut Game<'_>, pos: [i32; 3], bomb: &rules::Bomb, left: u64) {
    match bomb.detonator {
        Some(rules::Detonator::Fuse) => {
            let top = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.05, f64::from(pos[2]) + 0.5];
            let smoke = ParticleOptions::simple(ParticleType::Smoke);
            game.native_level_send_particles(&smoke, None, top, [0.1, 0.05, 0.1], 0.02, 2);
            if left % 2 == 0 {
                let flame = ParticleOptions::simple(ParticleType::Flame);
                game.native_level_send_particles(&flame, None, top, [0.05, 0.02, 0.05], 0.01, 1);
            }
            if left % 20 == 0 {
                sound(game, pos, "minecraft:block.fire.extinguish", 0.3, 2.0);
            }
        }
        Some(rules::Detonator::Keypad) if left % 20 == 0 => {
            let pitch = 1.0 + (rules::COUNTDOWN_TICKS.saturating_sub(left) as f32) / 200.0;
            sound(game, pos, "minecraft:block.note_block.pling", 1.0, pitch);
        }
        _ => {}
    }
}

/// A keypad button pressed on the server.
fn keypad_button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let (Some(mut bomb), Some(key), Some((now, maxpower))) = (load_bomb(entity), rules::Key::of_button(id), clock(game)) else {
        return;
    };
    match bomb.press(key, now) {
        rules::Press::Accepted if bomb.kind.radius() > maxpower => {
            // The code was right, but the bomb is over the operator's limit.
            bomb.fires_at = None;
            let why = rules::Refused::Maxpower { radius: bomb.kind.radius(), maxpower };
            let dimension = game.native_dimension();
            tell_near(game, dimension, pos, &refused(why));
        }
        rules::Press::CodeSet => sound(game, pos, "minecraft:block.note_block.chime", 1.0, 1.0),
        rules::Press::Wrong | rules::Press::TooShort => sound(game, pos, "minecraft:block.note_block.bass", 1.0, 0.8),
        rules::Press::Locked => sound(game, pos, "minecraft:block.note_block.bass", 1.0, 0.5),
        rules::Press::Accepted | rules::Press::Typed | rules::Press::Cleared | rules::Press::Ignored => {}
    }
    entity.data = bomb.data(now).to_vec();
    store_bomb(entity, &bomb);
}

// ---- the remote ------------------------------------------------------------------
//
// A right click on a bomb with a receiver links the remote in the main hand: its
// `big_nukes:link` component (API 65) keeps the bomb's dimension and place. The
// world keeps each bomb with a receiver in `Saved.bombs`, so a right click with a
// linked remote sets the bomb off from anywhere, its chunk loaded or not.

/// The player and the tick of the last remote pressed. The game asks once per
/// hand, so a click sets off one bomb at most.
static LAST_USE: Mutex<Option<(u128, u64)>> = Mutex::new(None);

/// The remote's component, registered before the content loads.
pub(in crate::game) fn component_types(types: &mut Vec<String>) {
    if !types.iter().any(|t| t == rules::LINK) {
        types.push(rules::LINK.to_owned());
    }
}

fn place(pos: [i32; 3]) -> Text {
    Text::literal(format!("{} {} {}", pos[0], pos[1], pos[2]))
}

/// A bomb with a receiver goes in the world's record, saved at once. False when
/// it went off and only stands until its wave reaches it.
fn remember(game: &Game<'_>, dimension: &str, pos: [i32; 3], kind: rules::BombKind) -> bool {
    let Some(mut guard) = state(game) else {
        return false;
    };
    let st = guard.as_mut().expect("opened");
    let recorded = st.saved.record(dimension, pos, kind);
    if recorded {
        save(st);
    }
    recorded
}

/// The bomb the remote is linked to, if it is linked.
fn link_of(game: &Game<'_>, stack: &ItemStack) -> Option<rules::Link> {
    let tag = game.native_component(stack, rules::LINK).ok()??;
    let Some(Tag::String(dimension)) = tag.get("dimension") else {
        return None;
    };
    let Some(Tag::IntArray(pos)) = tag.get("pos") else {
        return None;
    };
    rules::Link::of(&dimension.to_string_lossy(), <[i32; 3]>::try_from(pos.as_slice()).ok()?)
}

/// Links the remote in the acting player's main hand to the bomb at `pos`,
/// recording the bomb again if its record was lost. A link to another bomb is
/// replaced.
fn link(game: &mut Game<'_>, who: &NativeServerPlayer, pos: [i32; 3], kind: rules::BombKind) {
    if !remember(game, &who.dimension, pos, kind) {
        game.native_tell(who.uuid, Text::translate("big_nukes.remote.spent", &[]));
        return;
    }
    if game.native_equipment(EquipmentSlot::MainHand).is_some_and(|s| &*s.item == rules::REMOTE) {
        queue_held(game, who, Held::Link { dimension: who.dimension.clone(), pos, kind });
    }
}

fn block_name(block: &str) -> Text {
    Text::translate(&format!("block.{}", block.replace(':', ".")), &[])
}

/// Whether the chunk of `pos` is loaded in `dimension`.
fn loaded(dimension: &str, pos: [i32; 3]) -> bool {
    let chunk = rules::chunk_of(pos[0], pos[2]);
    lock(&LOADED).as_ref().and_then(|m| m.get(dimension)).is_some_and(|set| set.contains(&chunk))
}

/// A right click with an item, in the air or on a block that let it through:
/// a linked remote sets its bomb off. A bomb in a loaded chunk of the player's
/// level is checked and taken away at once; any other goes with its wave's crater.
/// A right click on a block with a remote: the game asks the block's item use
/// first, and only when nothing claims it the use in the air. Claiming it here
/// keeps a click on the ground from setting a bomb off; the remote fires only
/// when aimed at nothing.
pub(in crate::game) fn use_item_on(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed || &*stack.item != rules::REMOTE || !game.native_on(rules::ID) {
        return;
    }
    *claimed = true;
    let (Some(who), Some((now, _))) = (game.native_acting_player(), clock(game)) else {
        return;
    };
    {
        let mut last = lock(&LAST_USE);
        if *last == Some((who.uuid, now)) {
            return;
        }
        *last = Some((who.uuid, now));
    }
    game.native_tell(who.uuid, Text::translate("big_nukes.remote.aim", &[]));
}

pub(in crate::game) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if !*claimed && &*stack.item == rules::GUIDE && game.native_on(rules::ID) {
        read_guide(game, claimed);
        return;
    }
    if *claimed || &*stack.item != rules::REMOTE || !game.native_on(rules::ID) {
        return;
    }
    let (Some(who), Some((now, _))) = (game.native_acting_player(), clock(game)) else {
        return;
    };
    *claimed = true;
    {
        let mut last = lock(&LAST_USE);
        if *last == Some((who.uuid, now)) {
            return;
        }
        *last = Some((who.uuid, now));
    }
    let Some(link) = link_of(game, stack) else {
        game.native_tell(who.uuid, Text::translate("big_nukes.remote.not_linked", &[]));
        return;
    };
    let (dimension, pos) = (link.dimension.as_str(), link.pos);
    let here = dimension == who.dimension && loaded(dimension, pos);
    if here {
        // Something the world doesn't report (a bulk edit, another mod) may have
        // taken the bomb or its receiver.
        let receiver = game
            .native_with_mod_block_entity(pos, |e| load_bomb(e))
            .flatten()
            .is_some_and(|b| b.detonator == Some(rules::Detonator::Receiver));
        if !receiver
            && let Some(mut guard) = state(game)
        {
            let st = guard.as_mut().expect("opened");
            if st.saved.forget(dimension, pos) {
                save(st);
            }
        }
    }
    let line = match start(game, dimension, |saved| saved.fire_bomb(dimension, pos)) {
        Ok(_) => {
            if here {
                // The bomb goes with its blast, and its detonator with it.
                game.native_with_mod_block_entity(pos, |e| e.items.iter_mut().for_each(|s| *s = None));
                if let Some(air) = game.pack.assets.registry.default_state("minecraft:air") {
                    game.native_set_block(dimension, pos, air, 3);
                }
            } else if dimension != who.dimension
                && let Some(mut guard) = state(game)
            {
                let st = guard.as_mut().expect("opened");
                if !st.wake.iter().any(|d| d == dimension) {
                    st.wake.push(dimension.to_owned());
                }
            }
            game.native_level_play_sound("minecraft:block.stone_button.click_on", who.position, PLAYERS, 0.6, 1.4);
            Text::translate("big_nukes.remote.fired", &[place(pos)])
        }
        Err(why) => refused(why),
    };
    game.native_tell(who.uuid, line);
}

/// A block changed, by any means: a bomb that is no longer there is forgotten.
/// Runs for every change in the world, so it asks the block's name first, and
/// leaves the forgetting to the state's next opening, as the level may change a
/// block while the state is locked.
pub(in crate::game) fn block_changed(
    level: &GameLevel<'_>,
    dimension: LevelDimension,
    pos: BlockPos,
    old: BlockStateId,
    new: BlockStateId,
) {
    let was = &*level.facts.get(old).name;
    let now = &*level.facts.get(new).name;
    // A strike beacon or a console taken away.
    if now != was && matches!(was, rules::STRIKE_BEACON | rules::LAUNCH_CONSOLE) {
        silo::taken(silo::dimension_id(dimension), [pos.x, pos.y, pos.z]);
    }
    let (taken, placed) = (rules::BombKind::of_block(was), rules::BombKind::of_block(now));
    if now == was || (taken.is_none() && placed.is_none()) {
        return;
    }
    let dimension = match dimension {
        LevelDimension::Overworld => "minecraft:overworld",
        LevelDimension::Nether => "minecraft:the_nether",
        LevelDimension::End => "minecraft:the_end",
    };
    let at = [pos.x, pos.y, pos.z];
    if taken.is_some() {
        lock(&BROKEN).push((dimension.to_owned(), at));
        lock(&PLACED).push((dimension.to_owned(), at, None));
    }
    if placed.is_some() {
        lock(&PLACED).push((dimension.to_owned(), at, placed));
    }
}

/// The remote's tooltip: the bomb it is linked to.
pub(in crate::game) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    if !game.native_on(rules::ID) {
        return;
    }
    // How to use it, under its name.
    for key in rules::tips(&stack.item) {
        lines.push((Text::translate(key, &[]).with_color(text::GRAY), u32::MAX));
    }
    if &*stack.item != rules::REMOTE {
        return;
    }
    let line = match link_of(game, stack) {
        Some(link) => Text::translate("big_nukes.remote.tooltip.linked", &[place(link.pos), Text::literal(link.dimension)]),
        None => Text::translate("big_nukes.remote.tooltip.unlinked", &[]),
    };
    lines.push((line.with_color(text::GRAY), u32::MAX));
}

// ---- the keypad's screen ---------------------------------------------------------
//
// Drawn over the game's plain panel, as a chest's with six rows is laid out: the
// bomb's name, the digits typed as stars, a line of what the keypad waits for,
// and twelve buttons, then the inventory.

const PAD_HEIGHT: f32 = 222.0;
const PAD_INVENTORY_Y: f32 = 140.0;
const KEY_SIZE: [f32; 2] = [32.0, 16.0];
const KEY_GAP: f32 = 4.0;
/// The top left of the first key.
const KEYS_AT: [f32; 2] = [36.0, 48.0];
/// The menu buttons in their places, row by row: 1 to 9, then clear, 0, enter.
const KEY_ROWS: [[i32; 3]; 4] = [[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 0, 11]];

fn keypad_bomb(block: &str) -> bool {
    rules::BombKind::of_block(block).is_some_and(|k| k.takes(rules::Detonator::Keypad))
}

/// Each key's button and its box from the screen's top left.
fn keys() -> impl Iterator<Item = (i32, [f32; 4])> {
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
fn pad_status(data: &[i32]) -> String {
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    let with = |key: &str, n: i32| lang::get(key).replacen("%s", &n.to_string(), 1);
    match get(0) {
        rules::PAD_COUNTING => with("big_nukes.screen.keypad.countdown", get(2)),
        rules::PAD_LOCKED => with("big_nukes.screen.keypad.locked", get(2)),
        rules::PAD_ENTER if get(3) < i32::from(rules::TRIES) => with("big_nukes.screen.keypad.wrong", get(3)),
        rules::PAD_ENTER => lang::get("big_nukes.screen.keypad.enter"),
        rules::PAD_SET => lang::get("big_nukes.screen.keypad.set"),
        _ => String::new(),
    }
}

fn keypad_screen() -> NativeScreen {
    NativeScreen {
        width: 176.0,
        height: PAD_HEIGHT,
        // The detonator's slot is hidden.
        slots: vec![None],
        inventory_y: PAD_INVENTORY_Y,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

fn keypad_draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    const LABEL: u32 = 0x40_4040;
    let [x, y] = origin;
    let centered = |batch: &mut HudBatch, text: &str, top: f32, rgb: u32, shadow: bool| {
        let width = game.native_text_width(text);
        game.native_text(batch, text, [x + (176.0 - width) / 2.0, y + top], rgb, shadow);
    };
    centered(batch, &lang::get(&format!("block.{}", block.replace(':', "."))), 6.0, LABEL, false);
    // The display: a star for each digit typed.
    game.native_fill(batch, [x + 37.0, y + 17.0, 102.0, 14.0], 0xFF37_3737);
    game.native_fill(batch, [x + 38.0, y + 18.0, 100.0, 12.0], 0xFF10_1810);
    let typed = data.get(1).copied().unwrap_or(0).clamp(0, rules::CODE_DIGITS as i32) as usize;
    centered(batch, &"*".repeat(typed), 21.0, 0x55_FF55, false);
    let state = data.get(0).copied();
    let status_color = match state {
        Some(rules::PAD_COUNTING | rules::PAD_LOCKED) => 0xAA_0000,
        _ => LABEL,
    };
    centered(batch, &pad_status(data), 35.0, status_color, false);
    // The keys, dimmed while the keypad takes nothing.
    let open = matches!(state, Some(rules::PAD_SET | rules::PAD_ENTER));
    let cursor = game.native_cursor();
    let at = [cursor[0] - x, cursor[1] - y];
    for (id, [kx, ky, w, h]) in keys() {
        let lit = open && inside(at, [kx, ky, w, h]);
        let face = match (open, lit) {
            (false, _) => 0xFF6B_6B6B,
            (true, true) => 0xFFB8_B8D8,
            (true, false) => 0xFF9C_9C9C,
        };
        game.native_fill(batch, [x + kx, y + ky, w, h], 0xFF2A_2A2A);
        game.native_fill(batch, [x + kx + 1.0, y + ky + 1.0, w - 2.0, h - 2.0], face);
        let label = match id {
            10 => lang::get("big_nukes.screen.keypad.clear"),
            11 => lang::get("big_nukes.screen.keypad.ok"),
            digit => digit.to_string(),
        };
        let width = game.native_text_width(&label);
        let rgb = if open { 0xFF_FFFF } else { 0xA0_A0A0 };
        game.native_text(batch, &label, [x + kx + (w - width) / 2.0, y + ky + 4.0], rgb, true);
    }
    game.native_text(batch, &lang::get("container.inventory"), [x + 8.0, y + PAD_INVENTORY_Y - 11.0], LABEL, false);
}

/// A left press on a key of the keypad, sent to the server as its menu button.
fn keypad_click(at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if right || button.is_some() {
        return;
    }
    *button = keys().find(|(_, r)| inside(at, *r)).map(|(id, _)| id);
}

// ---- the guide -------------------------------------------------------------------
//
// A right click with the guide, on the server, asks the player's window to open it.
// The window opens the mod's own screen (API 111), whose hooks only read the game: the
// open book lives in a static, and a page turned is heard at the next frame. The book
// stays after the screen closes, so it opens again where it was left.

/// A right click with the guide: the player's window opens it. The use is claimed
/// even when the message can't go, so the guide never acts as a plain book.
fn read_guide(game: &mut Game<'_>, claimed: &mut bool) {
    *claimed = true;
    let Some(who) = game.native_acting_player() else {
        return;
    };
    if let Err(e) = game.native_net_send_to_player(who.uuid, rules::GUIDE_CHANNEL, &[]) {
        log::warn!("[big_nukes] opening the guide: {e}");
    }
}

static GUIDE_BOOK: Mutex<Option<rules::Guide>> = Mutex::new(None);
static PAGE_TURNED: Mutex<bool> = Mutex::new(false);

/// The server's message: the book is laid out in the window's language and opened, at the
/// spread it was left at or, the first time, at its contents.
pub(in crate::game) fn guide_asked(game: &mut Game<'_>, channel: &str) {
    if channel != rules::GUIDE_CHANNEL || !game.native_on(rules::ID) {
        return;
    }
    let mut guide = rules::Guide::new(&lang::get, &|text: &str| game.native_text_width(text));
    if let Some(old) = lock(&GUIDE_BOOK).take() {
        guide.keep(&old);
    }
    *lock(&GUIDE_BOOK) = Some(guide);
    if !game.native_mod_screen_open(rules::ID) {
        log::info!("[big_nukes] the guide can't open over the screen that is open");
    }
}

fn guide_shown(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "mod" && game.native_mod_screen() == Some(rules::ID)
}

/// Does `act` to the open book; a page it turns is heard at the next frame.
fn turn<R>(guide: &mut rules::Guide, act: impl FnOnce(&mut rules::Guide) -> R) -> R {
    let before = guide.spread;
    let done = act(guide);
    if guide.spread != before {
        *lock(&PAGE_TURNED) = true;
    }
    done
}

/// An item's name in the window's language: a block's item is named as its block.
fn item_title(id: &str) -> String {
    let path = id.replace(':', ".");
    let item = format!("item.{path}");
    if lang::has(&item) { lang::get(&item) } else { lang::get(&format!("block.{path}")) }
}

pub(in crate::game) fn guide_render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !guide_shown(game, screen) {
        return;
    }
    let draws = {
        let mut book = lock(&GUIDE_BOOK);
        let Some(guide) = book.as_mut() else {
            return;
        };
        guide.window = [batch.width, batch.height];
        guide.draw(screen.mouse, &|text: &str| game.native_text_width(text), &item_title)
    };
    // Without the assets half there is no texture: plain colours stand in for it.
    let textured = game.native_sprite_size(rules::GUIDE_GUI) == Some(rules::GUIDE_TEXTURE);
    game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], rules::GUIDE_DIM);
    for draw in draws {
        match draw {
            rules::GuideDraw::Sprite(src, dst) if textured => {
                game.native_sprite(batch, rules::GUIDE_GUI, src, dst);
            }
            rules::GuideDraw::Sprite(src, dst) if src == [0.0, 0.0, rules::BOOK_SIZE[0], rules::BOOK_SIZE[1]] => {
                game.native_fill(batch, dst, 0xFF4A_2C1A);
                for [x, y, w, h] in rules::PAGES {
                    game.native_fill(batch, [dst[0] + x, dst[1] + y, w, h], 0xFFE9_DDB8);
                }
            }
            rules::GuideDraw::Sprite(_, dst) => game.native_fill(batch, dst, 0xFF8A_6A40),
            rules::GuideDraw::Fill(rect, argb) => game.native_fill(batch, rect, argb),
            rules::GuideDraw::Text(text, at, rgb) => game.native_text(batch, &text, at, rgb, false),
            rules::GuideDraw::Item(id, at) => {
                game.native_item(batch, id, at, 16.0);
            }
        }
    }
}

pub(in crate::game) fn guide_mouse(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if guide_shown(game, screen)
        && let Some(guide) = lock(&GUIDE_BOOK).as_mut()
    {
        if button == 0 {
            turn(guide, |g| g.click(screen.mouse));
        }
        *canceled = true;
    }
}

pub(in crate::game) fn guide_wheel(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if guide_shown(game, screen)
        && let Some(guide) = lock(&GUIDE_BOOK).as_mut()
    {
        turn(guide, |g| g.wheel(scroll[1]));
        *canceled = true;
    }
}

/// The arrow keys turn the pages and Home goes to the contents; Escape and the rest are the game's.
pub(in crate::game) fn guide_key(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    if guide_shown(game, screen)
        && let Some(guide) = lock(&GUIDE_BOOK).as_mut()
        && turn(guide, |g| g.key(key))
    {
        *canceled = true;
    }
}

/// A page turned since the last frame: the book's own sound, where the player stands.
pub(in crate::game) fn guide_frame(game: &mut Game<'_>) {
    if !std::mem::take(&mut *lock(&PAGE_TURNED)) {
        return;
    }
    if let Some(p) = game.native_player() {
        game.native_play_local_sound("minecraft:item.book.page_turn", [p.x, p.y + 1.62, p.z], 1.0, 1.0, 0);
    }
}
