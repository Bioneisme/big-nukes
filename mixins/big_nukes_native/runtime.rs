// Big Nukes' rules: no game type here, so they build and test on their own.
// The bundle adds this file to the game as `game::big_nukes`; the glue
// (`game::big_nukes_glue`, glue.rs) connects it to the game.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, VecDeque},
    time::Duration,
};

pub const ID: &str = "big_nukes_native";
pub const VERSION: &str = "0.1.0";
/// The mod's folder in a world's folder.
pub const DIR: &str = "horizon-big-nukes";
/// A loot table only the assets half brings: the sign that it's installed.
pub const ASSETS_MARKER: &str = "big_nukes:marker";

// ---- blast geometry ----------------------------------------------------------
//
// A blast leaves zones by how far a column is from the center, measured against
// an uneven border: the core is a crater, emptied up to the sky; the middle
// loses its plants, its glass and its water and is scorched; the outer ring
// loses glass and, less and less outward, its leaves and plants.

/// A dimension's height: its lowest block and how many there are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub min_y: i32,
    pub height: i32,
    /// Where a blast stops going up: the world's top, or under the Nether's bedrock roof.
    pub roof: i32,
}

impl World {
    pub const OVERWORLD: World = World { min_y: -64, height: 384, roof: 320 };
    pub const NETHER: World = World { min_y: 0, height: 256, roof: 123 };
    pub const END: World = World { min_y: 0, height: 256, roof: 256 };

    /// The first height above the world.
    pub fn top(&self) -> i32 {
        self.min_y + self.height
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blast {
    pub id: u64,
    pub dimension: String,
    pub center: [i32; 3],
    pub radius: u32,
    pub seed: u64,
    pub world: World,
}

pub fn chunk_of(x: i32, z: i32) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

/// How much the border swings in and out of the radius.
const BORDER: f64 = 0.12;
/// The zones' outer edges, as a share of the border's distance.
const CORE: f64 = 0.35;
const MIDDLE: f64 = 0.7;
/// The crater's depth at the center, as a share of the radius.
const CRATER_DEPTH: f64 = 0.25;
/// Bedrock's layers stay: the crater stops this far above the world's floor.
const ABOVE_FLOOR: i32 = 5;
/// How far below the center the middle zone looks for plants.
const PLANTS_BELOW: i32 = 48;
/// The share of scorched columns that catch fire.
const FIRE: f64 = 0.03;

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// A number in [0, 1) for a column, the same every time.
fn chance(seed: u64, x: i32, z: i32, salt: u64) -> f64 {
    let h = mix(seed ^ mix((x as u32 as u64) << 32 | z as u32 as u64) ^ salt);
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// A smooth swing in [-1, 1] around the circle: four harmonics with phases
/// drawn from the seed.
fn swing(seed: u64, angle: f64) -> f64 {
    [(2.0, 0.4), (3.0, 0.3), (5.0, 0.2), (7.0, 0.1)]
        .iter()
        .enumerate()
        .map(|(i, (k, a))| {
            let phase = (mix(seed ^ (i as u64 + 1)) >> 11) as f64 / (1u64 << 53) as f64 * std::f64::consts::TAU;
            a * (k * angle + phase).sin()
        })
        .sum()
}

/// How far a column is from the center: 0 there, 1 on the border.
pub fn edge(blast: &Blast, x: i32, z: i32) -> f64 {
    let (dx, dz) = ((x - blast.center[0]) as f64, (z - blast.center[2]) as f64);
    let dist = dx.hypot(dz);
    if dist == 0.0 {
        return 0.0;
    }
    let border = blast.radius as f64 * (1.0 + BORDER * swing(blast.seed, dz.atan2(dx)));
    dist / border
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Core,
    Middle,
    Outer,
    Untouched,
}

pub fn zone(d: f64) -> Zone {
    if d < CORE {
        Zone::Core
    } else if d < MIDDLE {
        Zone::Middle
    } else if d < 1.0 {
        Zone::Outer
    } else {
        Zone::Untouched
    }
}

/// The crater's floor under a column at distance `d`: a bowl, deepest at the
/// center, never into bedrock.
pub fn crater_floor(blast: &Blast, d: f64) -> i32 {
    let depth = if d < CORE { CRATER_DEPTH * blast.radius as f64 * (1.0 - (d / CORE).powi(2)) } else { 0.0 };
    (blast.center[1] - depth as i32).max(blast.world.min_y + ABOVE_FLOOR)
}

/// A heightmap a run's `y` counts from, as the game's `NativeHeightmap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    WorldSurface,
    MotionBlockingNoLeaves,
}

/// `height` blocks up column (`x`, `z`) from `y`, set to palette entry `block`
/// where filter `only` passes: the game's `NativeBlockRun`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub x: i32,
    pub z: i32,
    pub y: i32,
    pub height: u32,
    pub surface: Option<Surface>,
    pub block: u32,
    pub only: Option<u32>,
}

/// One `native_edit_blocks` call.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChunkEdit {
    pub palette: Vec<String>,
    pub filters: Vec<String>,
    pub runs: Vec<Run>,
}

const PALETTE: [&str; 3] = ["minecraft:air", "minecraft:coarse_dirt", "minecraft:fire"];
const AIR: u32 = 0;
const SCORCHED: u32 = 1;
const FLAME: u32 = 2;
const FILTERS: [&str; 14] = [
    "#minecraft:leaves",
    "#minecraft:logs",
    "#minecraft:impermeable",
    "#minecraft:substrate_overworld",
    "minecraft:water",
    "minecraft:air",
    "#minecraft:flowers",
    "#minecraft:saplings",
    "minecraft:short_grass",
    "minecraft:tall_grass",
    "minecraft:fern",
    "minecraft:large_fern",
    "minecraft:dead_bush",
    "minecraft:vine",
];
const LEAVES: u32 = 0;
const LOGS: u32 = 1;
const GLASS: u32 = 2;
const GROUND: u32 = 3;
const WATER: u32 = 4;
const EMPTY: u32 = 5;
/// Plants, named one by one: `#minecraft:replaceable` would take water and lava too.
const PLANTS: std::ops::Range<u32> = 6..14;

fn edit() -> ChunkEdit {
    ChunkEdit {
        palette: PALETTE.iter().map(|s| s.to_string()).collect(),
        filters: FILTERS.iter().map(|s| s.to_string()).collect(),
        runs: vec![],
    }
}

/// What a blast does to one chunk: two edits, made in order. The first takes the
/// plants and the crater; the second scorches what is then the surface, as a
/// run's heightmap is read once, before its edit changes anything.
pub fn plan_chunk(blast: &Blast, chunk: [i32; 2]) -> [ChunkEdit; 2] {
    let (mut plants, mut surface) = (edit(), edit());
    let top = blast.world.roof.min(blast.world.top());
    let from = |y: i32| y.clamp(blast.world.min_y, top);
    for x in chunk[0] * 16..chunk[0] * 16 + 16 {
        for z in chunk[1] * 16..chunk[1] * 16 + 16 {
            let d = edge(blast, x, z);
            let absolute = |lo: i32, block: u32, only: Option<u32>| {
                let y = from(lo);
                (y < top).then(|| Run { x, z, y, height: (top - y) as u32, surface: None, block, only })
            };
            let below = blast.center[1] - PLANTS_BELOW;
            match zone(d) {
                Zone::Core => plants.runs.extend(absolute(crater_floor(blast, d), AIR, None)),
                Zone::Middle => {
                    for f in [LEAVES, LOGS, GLASS].into_iter().chain(PLANTS) {
                        plants.runs.extend(absolute(below, AIR, Some(f)));
                    }
                    let near = |y, height, block, only| Run { x, z, y, height, surface: None, block, only: Some(only) };
                    let mut on = |s, y, height, block, only| {
                        let mut r = near(y, height, block, only);
                        r.surface = Some(s);
                        surface.runs.push(r);
                    };
                    on(Surface::MotionBlockingNoLeaves, -1, 1, SCORCHED, GROUND);
                    on(Surface::WorldSurface, -3, 3, AIR, WATER);
                    if chance(blast.seed, x, z, 1) < FIRE {
                        on(Surface::MotionBlockingNoLeaves, 0, 1, FLAME, EMPTY);
                    }
                }
                Zone::Outer => {
                    plants.runs.extend(absolute(below, AIR, Some(GLASS)));
                    if chance(blast.seed, x, z, 2) < (1.0 - d) / (1.0 - MIDDLE) {
                        plants.runs.extend(absolute(below, AIR, Some(LEAVES)));
                        for f in PLANTS {
                            plants.runs.extend(absolute(below, AIR, Some(f)));
                        }
                    }
                }
                Zone::Untouched => {}
            }
        }
    }
    [plants, surface]
}

// ---- the wave ------------------------------------------------------------------
//
// The front runs out from the center at a steady speed. A chunk joins its
// wave when the front reaches the chunk's nearest block: loaded, it waits in
// `ready` for its turn; not loaded, in `deferred` until it is. The work is
// done a little each tick, within a time budget, nearest first.

/// Blocks the front runs in a tick: 50 blocks in a second, 1000 in twenty.
pub const FRONT_SPEED: f64 = 2.5;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Wave {
    blast: Blast,
    start: u64,
    /// How many of the wave's chunks, in `order`, the front has reached.
    reached: usize,
    ready: VecDeque<[i32; 2]>,
    deferred: BTreeSet<[i32; 2]>,
    /// The front is past the reach: its smoke and damage are over too.
    #[serde(default)]
    front_out: bool,
    /// The chunks the border can reach, nearest first, with how near they are:
    /// worked out again from the blast after loading.
    #[serde(skip)]
    order: Vec<(f64, [i32; 2])>,
}

impl Wave {
    fn order(&mut self) -> &[(f64, [i32; 2])] {
        if self.order.is_empty() {
            self.order = order(&self.blast);
        }
        &self.order
    }

    fn done(&mut self) -> bool {
        self.front_out && self.ready.is_empty() && self.deferred.is_empty() && self.reached == self.order().len()
    }
}

/// How far a chunk's nearest block is from the center.
fn nearest(center: [i32; 3], chunk: [i32; 2]) -> f64 {
    let x = center[0].clamp(chunk[0] * 16, chunk[0] * 16 + 15) - center[0];
    let z = center[2].clamp(chunk[1] * 16, chunk[1] * 16 + 15) - center[2];
    (x as f64).hypot(z as f64)
}

fn order(blast: &Blast) -> Vec<(f64, [i32; 2])> {
    let reach = blast.radius as f64 * (1.0 + BORDER);
    let c = chunk_of(blast.center[0], blast.center[2]);
    let r = (reach / 16.0).ceil() as i32 + 1;
    let mut out: Vec<(f64, [i32; 2])> = (c[0] - r..=c[0] + r)
        .flat_map(|x| (c[1] - r..=c[1] + r).map(move |z| [x, z]))
        .map(|chunk| (nearest(blast.center, chunk), chunk))
        .filter(|(d, _)| *d <= reach)
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    out
}

/// Every blast whose wave isn't over, in every dimension: what a world keeps.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Waves {
    waves: Vec<Wave>,
}

impl Waves {
    pub fn start(&mut self, blast: Blast, now_tick: u64) {
        self.waves.push(Wave { blast, start: now_tick, reached: 0, ready: VecDeque::new(), deferred: BTreeSet::new(), front_out: false, order: vec![] });
    }

    /// Moves the front of each wave in `dimension` on to `now_tick`.
    /// Answers whether the front reached any chunk.
    pub fn advance(&mut self, dimension: &str, now_tick: u64, loaded: &dyn Fn([i32; 2]) -> bool) -> bool {
        let mut moved = false;
        for w in self.waves.iter_mut().filter(|w| w.blast.dimension == dimension) {
            let front = now_tick.saturating_sub(w.start) as f64 * FRONT_SPEED;
            w.order();
            while let Some(&(d, chunk)) = w.order.get(w.reached) {
                if d > front {
                    break;
                }
                if loaded(chunk) {
                    w.ready.push_back(chunk);
                } else {
                    w.deferred.insert(chunk);
                }
                w.reached += 1;
                moved = true;
            }
            if !w.front_out && front > w.blast.radius as f64 * (1.0 + BORDER) {
                w.front_out = true;
                moved = true;
            }
        }
        self.waves.retain_mut(|w| !w.done());
        moved
    }

    /// A chunk came in: its waiting work is next. Answers whether any waited.
    pub fn chunk_loaded(&mut self, dimension: &str, chunk: [i32; 2]) -> bool {
        let mut moved = false;
        for w in self.waves.iter_mut().filter(|w| w.blast.dimension == dimension) {
            if w.deferred.remove(&chunk) {
                w.ready.push_front(chunk);
                moved = true;
            }
        }
        moved
    }

    /// The next chunk to change, taken off its wave's list.
    pub fn next_job(&mut self, dimension: &str) -> Option<(Blast, [i32; 2])> {
        let w = self.waves.iter_mut().find(|w| w.blast.dimension == dimension && !w.ready.is_empty())?;
        Some((w.blast.clone(), w.ready.pop_front()?))
    }

    /// Changes chunks until the budget is spent. `apply` answers false for a
    /// chunk that is no longer loaded, which then waits for its load.
    pub fn work(
        &mut self,
        dimension: &str,
        budget: Duration,
        elapsed: &dyn Fn() -> Duration,
        apply: &mut dyn FnMut(&Blast, [i32; 2]) -> bool,
    ) -> usize {
        let mut done = 0;
        while elapsed() < budget {
            let Some((blast, chunk)) = self.next_job(dimension) else { break };
            if apply(&blast, chunk) {
                done += 1;
            } else if let Some(w) = self.waves.iter_mut().find(|w| w.blast.id == blast.id) {
                w.deferred.insert(chunk);
            }
        }
        self.waves.retain_mut(|w| !w.done());
        done
    }

    pub fn finished(&self) -> bool {
        self.waves.is_empty()
    }

    /// Nothing more to do in `dimension` until a chunk loads: every front there
    /// is out and every ready chunk changed.
    pub fn idle(&mut self, dimension: &str) -> bool {
        self.waves
            .iter_mut()
            .filter(|w| w.blast.dimension == dimension)
            .all(|w| w.front_out && w.ready.is_empty() && w.reached == w.order().len())
    }
}

// ---- the command and what a world keeps -----------------------------------------

/// The smallest and largest radius `/bignukes blast` takes.
pub const MIN_RADIUS: u32 = 8;
pub const MAX_RADIUS: u32 = 1024;

/// A dimension's height, for the ones the game has.
pub fn world_of(dimension: &str) -> Option<World> {
    match dimension {
        "minecraft:overworld" => Some(World::OVERWORLD),
        "minecraft:the_nether" => Some(World::NETHER),
        "minecraft:the_end" => Some(World::END),
        _ => None,
    }
}

/// `blast <radius> [x y z]`, after the command's name: the radius and the
/// center, the player's block when no place is given.
pub fn parse_blast(args: &[&str], at: [f64; 3]) -> Option<(u32, [i32; 3])> {
    let (radius, place) = args.split_first()?;
    let radius: u32 = radius.parse().ok().filter(|r| (MIN_RADIUS..=MAX_RADIUS).contains(r))?;
    let center = match place {
        [] => at.map(|v| v.floor() as i32),
        [x, y, z] => [x.parse().ok()?, y.parse().ok()?, z.parse().ok()?],
        _ => return None,
    };
    Some((radius, center))
}

/// What a world keeps in `<world>/horizon-big-nukes/waves.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Saved {
    /// Server ticks counted since the mod first saw the world.
    pub tick: u64,
    pub next_id: u64,
    pub waves: Waves,
}

impl Saved {
    /// A new blast at `center`, its wave started now.
    pub fn blast(&mut self, dimension: &str, center: [i32; 3], radius: u32) -> Option<u64> {
        let world = world_of(dimension)?;
        self.next_id += 1;
        let id = self.next_id;
        let seed = mix(self.tick ^ mix(id));
        self.waves.start(Blast { id, dimension: dimension.to_owned(), center, radius, seed, world }, self.tick);
        Some(id)
    }
}

// ---- the blast's effects ------------------------------------------------------
//
// What a player sees, hears and feels, worked out from how far they are and how
// long ago the blast was: the server sends `News` once, and the window does the rest.

/// A blast as a window hears of it: `age` is how many ticks ago it went off.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct News {
    pub id: u64,
    pub center: [i32; 3],
    pub radius: u32,
    pub seed: u64,
    pub age: u64,
}

/// The mod message that carries `News`.
pub const NEWS_CHANNEL: &str = "big_nukes:blast";
/// After this long a blast is past telling: its sound has come and gone.
const NEWS_FOR: u64 = 2400;

pub fn news_of(blast: &Blast, now_tick: u64, start: u64) -> News {
    News { id: blast.id, center: blast.center, radius: blast.radius, seed: blast.seed, age: now_tick.saturating_sub(start) }
}

impl Waves {
    /// The running fronts of `dimension`: each blast with the ground its front
    /// crossed since the last tick (from, to) and its age in ticks.
    pub fn fronts(&self, dimension: &str, now_tick: u64) -> Vec<(Blast, f64, f64, u64)> {
        self.waves
            .iter()
            .filter(|w| w.blast.dimension == dimension)
            .filter_map(|w| {
                let age = now_tick.saturating_sub(w.start);
                let from = age.saturating_sub(1) as f64 * FRONT_SPEED;
                (from <= w.blast.radius as f64 * (1.0 + BORDER)).then(|| (w.blast.clone(), from, age as f64 * FRONT_SPEED, age))
            })
            .collect()
    }

    /// The blasts of `dimension` still worth telling a player about.
    pub fn news(&self, dimension: &str, now_tick: u64) -> Vec<News> {
        self.waves
            .iter()
            .filter(|w| w.blast.dimension == dimension && now_tick.saturating_sub(w.start) < NEWS_FOR)
            .map(|w| news_of(&w.blast, now_tick, w.start))
            .collect()
    }
}

/// A blast as the window keeps it: its news, the dimension the player heard it
/// in, and the game time it went off.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub news: News,
    pub dimension: String,
    pub at: f64,
}

impl Heard {
    /// The ticks since the blast and the distance to it, for a player in
    /// `dimension` with eyes at `eyes`, at game time `now` (with the part of the
    /// tick gone). None once it is past, in another dimension, or from another
    /// world whose clock is behind.
    pub fn felt(&self, dimension: &str, eyes: [f64; 3], now: f64) -> Option<(f64, f64)> {
        let c = self.news.center;
        let d = [eyes[0] - c[0] as f64 - 0.5, eyes[1] - c[1] as f64, eyes[2] - c[2] as f64 - 0.5];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let t = now - self.at;
        // Kept while its front, its sound or its cloud (five minutes) is still to see.
        let lasts = (dist / FRONT_SPEED + 200.0).max(CLOUD_TICKS);
        (dimension == self.dimension && t > -20.0 && t < lasts).then_some((t, dist))
    }
}

/// The felt blasts in the order they are drawn: the newest first, so a fresh
/// fireball is never left out for older clouds when there are too many.
pub fn newest_first(felt: &mut [(f64, f64, News)]) {
    felt.sort_by(|a, b| a.0.total_cmp(&b.0));
}

/// How long a cloud stands: it is gone at five minutes.
pub const CLOUD_TICKS: f64 = 6000.0;

/// The white of the flash, 0 to 255: full within two radii, gone at twelve,
/// and fading over a second or so.
pub fn flash_alpha(t_ticks: f64, dist: f64, radius: u32) -> u8 {
    if !(0.0..80.0).contains(&t_ticks) {
        return 0;
    }
    let r = radius as f64;
    let peak = ((12.0 * r - dist) / (10.0 * r)).clamp(0.0, 1.0);
    (255.0 * peak * (-t_ticks / 20.0).exp()).round() as u8
}

/// A sound to play, `delay` ticks from now.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub event: &'static str,
    pub delay: i32,
    pub volume: f32,
}

/// What a player `dist` blocks from the center hears, `age` ticks after the
/// blast: the boom, near or far, and the roar of the wave if it reaches them,
/// all arriving with the front. What is already past is left out.
pub fn sounds_for(dist: f64, radius: u32, age: u64) -> Vec<Sound> {
    let r = radius as f64;
    if dist > 3000.0 {
        return vec![];
    }
    let delay = (dist / FRONT_SPEED).min(1200.0).round() as i64 - age as i64;
    if delay < 0 {
        return vec![];
    }
    let volume = (1.5 - dist / (10.0 * r)).clamp(0.2, 1.0) as f32;
    let boom = if dist < 1.5 * r { "big_nukes:blast.near" } else { "big_nukes:blast.far" };
    let mut out = vec![Sound { event: boom, delay: delay as i32, volume }];
    if dist <= r * (1.0 + BORDER) {
        out.push(Sound { event: "big_nukes:wave.roar", delay: delay as i32, volume });
    }
    out
}

/// The camera's shake: pitch, yaw and roll in degrees, then an offset in blocks.
/// It starts when the front reaches the player and dies out in a few seconds.
pub fn shake(t_ticks: f64, dist: f64, radius: u32, seed: u64) -> [f32; 6] {
    let since = t_ticks - dist / FRONT_SPEED;
    let strength = (1.3 - dist / (4.0 * radius as f64)).clamp(0.0, 1.0);
    if !(0.0..=100.0).contains(&since) || strength == 0.0 {
        return [0.0; 6];
    }
    let e = strength * (-since / 20.0).exp();
    let wobble = |k: u64, rate: f64| {
        let phase = (mix(seed ^ k) >> 11) as f64 / (1u64 << 53) as f64 * std::f64::consts::TAU;
        (t_ticks * rate + phase).sin()
    };
    [
        (6.0 * e * wobble(1, 1.9)) as f32,
        (6.0 * e * wobble(2, 2.3)) as f32,
        (3.0 * e * wobble(3, 1.3)) as f32,
        (0.3 * e * wobble(4, 2.9)) as f32,
        (0.3 * e * wobble(5, 3.1)) as f32,
        (0.3 * e * wobble(6, 2.7)) as f32,
    ]
}

/// Sixteen points on the front, two blocks over the center's height, while it runs.
pub fn front_ring(blast: &Blast, t: u64) -> Vec<[f64; 3]> {
    let r = t as f64 * FRONT_SPEED;
    if r < 1.0 || r > blast.radius as f64 * (1.0 + BORDER) {
        return vec![];
    }
    let turn = (mix(blast.seed ^ 7) >> 11) as f64 / (1u64 << 53) as f64;
    let [cx, cy, cz] = blast.center.map(|v| v as f64);
    (0..16)
        .map(|k| {
            let a = (k as f64 + turn) / 16.0 * std::f64::consts::TAU;
            [cx + r * a.cos(), cy + 2.0, cz + r * a.sin()]
        })
        .collect()
}

/// The damage of the ground the front crossed, from `from` to `to` blocks out:
/// `(from, to, amount)` bands, cut where the zones change. Death in the core,
/// a little at the edge, nothing past the reach.
pub fn damage_band(radius: u32, from: f64, to: f64) -> Vec<(f64, f64, f32)> {
    let r = radius as f64;
    let to = to.min(r * (1.0 + BORDER));
    let cuts = [CORE * r, MIDDLE * r];
    let mut edges = vec![from];
    edges.extend(cuts.iter().copied().filter(|c| *c > from && *c < to));
    edges.push(to);
    edges
        .windows(2)
        .filter(|e| e[1] > e[0])
        .map(|e| {
            let d = (e[0] + e[1]) / 2.0 / r;
            let amount = if d < CORE {
                1000.0
            } else if d < MIDDLE {
                20.0 - (d - CORE) / (MIDDLE - CORE) * 14.0
            } else {
                4.0 - (d - MIDDLE) / (1.0 + BORDER - MIDDLE) * 3.0
            };
            (e[0], e[1], amount as f32)
        })
        .collect()
}

/// Blocks thrown out of the crater at the start: where, how fast and what.
pub fn debris(blast: &Blast) -> Vec<([f64; 3], [f64; 3], &'static str)> {
    const BLOCKS: [&str; 3] = ["minecraft:stone", "minecraft:cobblestone", "minecraft:dirt"];
    let n = (blast.radius / 2).min(48) as u64;
    let [cx, cy, cz] = blast.center.map(|v| v as f64 + 0.5);
    (0..n)
        .map(|k| {
            let roll = |salt: u64| (mix(blast.seed ^ (k << 8) ^ salt) >> 11) as f64 / (1u64 << 53) as f64;
            let a = roll(1) * std::f64::consts::TAU;
            let out = 0.8 + 0.8 * roll(2);
            let up = 0.9 + 0.7 * roll(3);
            ([cx, cy - 0.5 + 3.0, cz], [out * a.cos(), up, out * a.sin()], BLOCKS[(k % 3) as usize])
        })
        .collect()
}

// ---- the mushroom cloud ---------------------------------------------------------
//
// Its shape over time, in blocks: the renderer gets these numbers and only draws.

/// A cloud at one moment: the fireball's height and size, the stem, the cap (a
/// torus around the fireball), how much fire still glows, how visible it all is,
/// and the dust ring on the ground with its own fade.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cloud {
    pub ball_h: f32,
    pub ball_r: f32,
    pub stem_r: f32,
    pub cap_major: f32,
    pub cap_minor: f32,
    pub fire: f32,
    pub fade: f32,
    pub ring: f32,
    pub ring_fade: f32,
}

pub fn cloud(age_ticks: f64, radius: u32) -> Cloud {
    if age_ticks < 0.0 {
        return Cloud::default();
    }
    let r = radius as f64;
    let t = age_ticks / 20.0;
    let top = 0.9 * r + 60.0;
    let ball_r = 0.12 * r + 8.0;
    let reach = r * (1.0 + BORDER);
    let out_at = reach / FRONT_SPEED;
    Cloud {
        ball_h: (top * (1.0 - (-t / 25.0).exp())) as f32,
        ball_r: ball_r as f32,
        stem_r: (0.35 * ball_r) as f32,
        cap_major: (0.5 * ball_r * (1.0 + t / 20.0)).min(1.4 * ball_r) as f32,
        cap_minor: (0.8 * ball_r) as f32,
        fire: (-t / 20.0).exp() as f32,
        fade: (1.0 - (t - 180.0) / (CLOUD_TICKS / 20.0 - 180.0)).clamp(0.0, 1.0) as f32,
        ring: (age_ticks * FRONT_SPEED).min(reach) as f32,
        ring_fade: (1.0 - (age_ticks - out_at).max(0.0) / 200.0).clamp(0.0, 1.0) as f32,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    fn file(path: &str) -> String {
        std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn language(code: &str) -> BTreeMap<String, String> {
        let path = format!("content/assets/big_nukes/lang/{code}.json");
        serde_json::from_str(&file(&path)).unwrap()
    }

    fn keys_in(source: &str) -> Vec<String> {
        source
            .split('"')
            .filter(|t| t.starts_with("big_nukes.") && !t.ends_with(".json"))
            .filter(|t| !t.contains(' ') && !t.ends_with('.'))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn every_line_is_in_both_languages_with_the_same_arguments() {
        let (en, ru) = (language("en_us"), language("ru_ru"));
        let used: BTreeSet<String> = [
            "mixins/big_nukes_native/runtime.rs",
            "mixins/big_nukes_native/glue.rs",
            "content/data/big_nukes/function/check.mcfunction",
        ]
        .iter()
        .flat_map(|s| keys_in(file(s).split("#[cfg(test)]").next().unwrap()))
        .collect();
        assert!(!used.is_empty());
        for key in &used {
            assert!(en.contains_key(key), "{key} isn't in en_us");
        }
        for (key, line) in &en {
            let theirs = ru.get(key).unwrap_or_else(|| panic!("{key} isn't in ru_ru"));
            assert_eq!(line.matches("%s").count(), theirs.matches("%s").count(), "{key}");
            assert!(
                key.starts_with("block.")
                    || key.starts_with("item.")
                    || key.starts_with("death.attack.big_nukes.")
                    || key.starts_with("subtitles.big_nukes.")
                    || key == "big_nukes.mod.description"
                    || used.contains(key),
                "{key} is used nowhere"
            );
        }
        assert_eq!(en.len(), ru.len());
    }

    #[test]
    fn the_assets_bring_the_marker() {
        let (namespace, name) = super::ASSETS_MARKER.split_once(':').unwrap();
        let table: serde_json::Value = serde_json::from_str(&file(&format!("content/data/{namespace}/loot_table/{name}.json"))).unwrap();
        assert_eq!(table["pools"], serde_json::json!([]));
    }

    // ---- blast geometry (Task 2) -------------------------------------------

    use super::*;

    fn blast(center: [i32; 3], radius: u32, seed: u64, world: World) -> Blast {
        Blast { id: 1, dimension: "minecraft:overworld".into(), center, radius, seed, world }
    }

    fn columns_in(b: &Blast, wanted: Zone) -> BTreeSet<[i32; 2]> {
        let r = b.radius as i32 * 2;
        let mut out = BTreeSet::new();
        for x in b.center[0] - r..=b.center[0] + r {
            for z in b.center[2] - r..=b.center[2] + r {
                if zone(edge(b, x, z)) == wanted {
                    out.insert([x, z]);
                }
            }
        }
        out
    }

    #[test]
    fn chunk_of_rounds_down_for_negative_coordinates() {
        assert_eq!(chunk_of(-1, -1), [-1, -1]);
        assert_eq!(chunk_of(-16, 15), [-1, 0]);
        assert_eq!(chunk_of(-17, 16), [-2, 1]);
        assert_eq!(chunk_of(0, 31), [0, 1]);
    }

    #[test]
    fn edge_is_zero_at_the_center_and_stays_within_twelve_percent_of_the_radius() {
        let b = blast([0, 64, 0], 200, 7, World::OVERWORLD);
        assert_eq!(edge(&b, 0, 0), 0.0);
        for deg in 0..360 {
            let a = (deg as f64).to_radians();
            let at = |r: f64| edge(&b, (r * a.cos()).round() as i32, (r * a.sin()).round() as i32);
            assert!(at(200.0 * 0.87) <= 1.0, "{deg}");
            assert!(at(200.0 * 1.13) >= 1.0, "{deg}");
        }
    }

    #[test]
    fn the_border_is_uneven_but_repeatable() {
        let a = blast([0, 64, 0], 200, 7, World::OVERWORLD);
        let again = blast([0, 64, 0], 200, 7, World::OVERWORLD);
        let other = blast([0, 64, 0], 200, 8, World::OVERWORLD);
        let ring = |b: &Blast| (0..360).map(|d| edge(b, (200.0 * (d as f64).to_radians().cos()) as i32, (200.0 * (d as f64).to_radians().sin()) as i32)).collect::<Vec<_>>();
        assert_eq!(ring(&a), ring(&again));
        assert_ne!(ring(&a), ring(&other));
        let r = ring(&a);
        let (lo, hi) = r.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(hi - lo > 0.05, "the border is a circle: {lo}..{hi}");
    }

    #[test]
    fn zones_split_at_035_07_and_1() {
        assert_eq!(zone(0.0), Zone::Core);
        assert_eq!(zone(0.3499), Zone::Core);
        assert_eq!(zone(0.35), Zone::Middle);
        assert_eq!(zone(0.6999), Zone::Middle);
        assert_eq!(zone(0.7), Zone::Outer);
        assert_eq!(zone(0.9999), Zone::Outer);
        assert_eq!(zone(1.0), Zone::Untouched);
    }

    #[test]
    fn the_crater_is_deepest_at_the_center_and_never_reaches_bedrock() {
        let b = blast([0, 70, 0], 250, 1, World::OVERWORLD);
        assert_eq!(crater_floor(&b, 0.0), 70 - 62);
        assert!(crater_floor(&b, 0.2) > crater_floor(&b, 0.0));
        assert_eq!(crater_floor(&b, 0.35), 70);
        let deep = blast([0, -50, 0], 250, 1, World::OVERWORLD);
        assert_eq!(crater_floor(&deep, 0.0), -64 + 5);
    }

    #[test]
    fn a_blast_at_minus_one_is_symmetric() {
        let a = blast([-1, 64, -1], 24, 3, World::OVERWORLD);
        let b = blast([0, 64, 0], 24, 3, World::OVERWORLD);
        let shifted: BTreeSet<[i32; 2]> = columns_in(&b, Zone::Core).iter().map(|[x, z]| [x - 1, z - 1]).collect();
        assert_eq!(columns_in(&a, Zone::Core), shifted);
        assert!(columns_in(&a, Zone::Core).contains(&[4, -1]) && columns_in(&a, Zone::Core).contains(&[-6, -1]));
    }

    fn all_runs(b: &Blast) -> Vec<(usize, Run)> {
        let r = b.radius as i32 * 2 / 16 + 1;
        let c = chunk_of(b.center[0], b.center[2]);
        let mut out = vec![];
        for cx in c[0] - r..=c[0] + r {
            for cz in c[1] - r..=c[1] + r {
                for (i, e) in plan_chunk(b, [cx, cz]).into_iter().enumerate() {
                    out.extend(e.runs.into_iter().map(|run| (i, run)));
                }
            }
        }
        out
    }

    #[test]
    fn runs_stay_inside_the_world() {
        for (center, world) in [([5, 300, 5], World::OVERWORLD), ([5, 250, 5], World::NETHER), ([5, 2, 5], World::NETHER)] {
            let b = blast(center, 60, 9, world);
            let runs = all_runs(&b);
            assert!(!runs.is_empty());
            for (_, r) in runs.iter().filter(|(_, r)| r.surface.is_none()) {
                assert!(r.y >= world.min_y && r.y + r.height as i32 <= world.top(), "{r:?} in {world:?}");
                assert!(r.height > 0);
            }
        }
    }

    #[test]
    fn a_chunk_beyond_the_edge_plans_nothing() {
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        for e in plan_chunk(&b, chunk_of(230, 0)) {
            assert!(e.runs.is_empty());
        }
    }

    #[test]
    fn the_core_clears_whole_columns_and_the_rim_only_filters() {
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        let edits = plan_chunk(&b, [0, 0]);
        let air = edits[0].palette.iter().position(|p| p == "minecraft:air").unwrap() as u32;
        let core = edits[0].runs.iter().find(|r| r.x == 1 && r.z == 1 && r.only.is_none()).expect("a core column is cleared");
        assert_eq!(core.block, air);
        assert_eq!(core.y + core.height as i32, World::OVERWORLD.top());
        let outer = plan_chunk(&b, chunk_of(85, 0));
        let at_85: Vec<&Run> = outer.iter().flat_map(|e| &e.runs).filter(|r| r.x == 85 && r.z == 0).collect();
        assert!(!at_85.is_empty() && at_85.iter().all(|r| r.only.is_some()), "{at_85:?}");
    }

    #[test]
    fn the_middle_scorches_the_surface_after_the_plants_are_gone() {
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        let [plants, surface] = plan_chunk(&b, chunk_of(50, 0));
        let dirt = surface.palette.iter().position(|p| p == "minecraft:coarse_dirt").unwrap() as u32;
        assert!(plants.runs.iter().any(|r| r.x == 50 && r.z == 0));
        assert!(surface.runs.iter().any(|r| r.x == 50 && r.z == 0 && r.block == dirt && r.surface.is_some()));
    }

    #[test]
    fn water_and_lava_are_never_taken_with_the_plants() {
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        for chunk in [[0, 0], chunk_of(50, 0), chunk_of(85, 0)] {
            let [plants, surface] = plan_chunk(&b, chunk);
            for r in plants.runs.iter().filter(|r| r.only.is_some()) {
                let f = &plants.filters[r.only.unwrap() as usize];
                assert!(!f.contains("replaceable") && !f.contains("water") && !f.contains("lava"), "{f}");
            }
            for r in surface.runs.iter().filter(|r| surface.filters[r.only.unwrap() as usize].contains("water")) {
                assert_eq!((r.y, r.height, r.surface), (-3, 3, Some(Surface::WorldSurface)));
            }
        }
    }

    // ---- the wave (Task 3) -------------------------------------------------

    use std::time::Duration;

    fn waves_with(blasts: &[Blast]) -> Waves {
        let mut w = Waves::default();
        for b in blasts {
            w.start(b.clone(), 0);
        }
        w
    }

    fn drain(w: &mut Waves, dim: &str) -> Vec<(u64, [i32; 2])> {
        std::iter::from_fn(|| w.next_job(dim)).map(|(b, c)| (b.id, c)).collect()
    }

    const OW: &str = "minecraft:overworld";

    #[test]
    fn the_front_reaches_250_blocks_in_100_ticks() {
        let mut w = waves_with(&[blast([7, 64, 0], 300, 1, World::OVERWORLD)]);
        let target = [16, 0]; // its nearest block, x = 256, is 249 blocks out
        w.advance(OW, 99, &|_| true);
        assert!(!drain(&mut w, OW).iter().any(|(_, c)| *c == target));
        w.advance(OW, 100, &|_| true);
        assert!(drain(&mut w, OW).iter().any(|(_, c)| *c == target));
    }

    #[test]
    fn chunks_come_out_nearest_first() {
        let mut w = waves_with(&[blast([0, 64, 0], 100, 1, World::OVERWORLD)]);
        w.advance(OW, 1000, &|_| true);
        let near = |c: [i32; 2]| {
            let x = 0f64.clamp(c[0] as f64 * 16.0, c[0] as f64 * 16.0 + 15.0);
            let z = 0f64.clamp(c[1] as f64 * 16.0, c[1] as f64 * 16.0 + 15.0);
            x.hypot(z)
        };
        let out: Vec<f64> = drain(&mut w, OW).into_iter().map(|(_, c)| near(c)).collect();
        assert!(out.len() > 100);
        assert!(out.windows(2).all(|p| p[0] <= p[1]), "{out:?}");
    }

    #[test]
    fn an_unloaded_chunk_waits_and_comes_back_once_when_loaded() {
        let mut w = waves_with(&[blast([0, 64, 0], 60, 1, World::OVERWORLD)]);
        let far = [3, 0];
        w.advance(OW, 1000, &|c| c != far);
        assert!(!drain(&mut w, OW).iter().any(|(_, c)| *c == far));
        assert!(!w.finished());
        w.chunk_loaded(OW, far);
        w.chunk_loaded(OW, far);
        assert_eq!(drain(&mut w, OW), vec![(1, far)]);
        w.advance(OW, 1001, &|_| true);
        assert!(w.finished());
    }

    #[test]
    fn two_overlapping_blasts_each_process_a_shared_chunk_once() {
        let a = blast([0, 64, 0], 60, 1, World::OVERWORLD);
        let mut b = blast([40, 64, 0], 60, 2, World::OVERWORLD);
        b.id = 2;
        let mut w = waves_with(&[a, b]);
        w.advance(OW, 1000, &|_| true);
        let jobs = drain(&mut w, OW);
        let shared = [1, 0];
        assert_eq!(jobs.iter().filter(|j| **j == (1, shared)).count(), 1);
        assert_eq!(jobs.iter().filter(|j| **j == (2, shared)).count(), 1);
        let unique: BTreeSet<_> = jobs.iter().collect();
        assert_eq!(unique.len(), jobs.len());
    }

    #[test]
    fn work_stops_when_the_budget_is_spent() {
        let mut w = waves_with(&[blast([0, 64, 0], 200, 1, World::OVERWORLD)]);
        w.advance(OW, 1000, &|_| true);
        let done = std::cell::Cell::new(0u32);
        let clock = || Duration::from_millis(3 * done.get() as u64);
        let n = w.work(OW, Duration::from_millis(10), &clock, &mut |_, _| {
            done.set(done.get() + 1);
            true
        });
        assert_eq!(n, 4);
    }

    #[test]
    fn a_chunk_that_unloads_before_its_turn_waits_for_its_load() {
        let mut w = waves_with(&[blast([0, 64, 0], 40, 1, World::OVERWORLD)]);
        w.advance(OW, 1000, &|_| true);
        let first = std::cell::Cell::new(None);
        w.work(OW, Duration::from_secs(1), &|| Duration::ZERO, &mut |_, c| {
            if first.get().is_none() {
                first.set(Some(c));
                return false;
            }
            true
        });
        let c = first.get().unwrap();
        assert!(!w.finished());
        w.chunk_loaded(OW, c);
        assert_eq!(drain(&mut w, OW), vec![(1, c)]);
    }

    #[test]
    fn a_wave_is_finished_only_when_nothing_is_left() {
        let mut w = waves_with(&[blast([0, 64, 0], 40, 1, World::OVERWORLD)]);
        assert!(!w.finished());
        w.advance(OW, 5, &|_| true);
        assert!(!w.finished(), "the front isn't out yet");
        w.advance(OW, 1000, &|_| true);
        assert!(!w.finished(), "jobs are waiting");
        drain(&mut w, OW);
        w.advance(OW, 1001, &|_| true);
        assert!(w.finished());
    }

    #[test]
    fn other_dimensions_are_left_alone() {
        let mut w = waves_with(&[blast([0, 64, 0], 40, 1, World::OVERWORLD)]);
        w.advance("minecraft:the_nether", 1000, &|_| true);
        assert!(drain(&mut w, "minecraft:the_nether").is_empty());
        w.advance(OW, 1000, &|_| true);
        assert!(drain(&mut w, "minecraft:the_nether").is_empty());
        assert!(!drain(&mut w, OW).is_empty());
    }

    #[test]
    fn waves_survive_a_round_trip_through_json() {
        let mut w = waves_with(&[blast([0, 64, 0], 200, 1, World::OVERWORLD)]);
        w.advance(OW, 40, &|c| c != [2, 2]);
        let done: Vec<_> = (0..5).filter_map(|_| w.next_job(OW)).map(|(_, c)| c).collect();
        let json = serde_json::to_string(&w).unwrap();
        let mut back: Waves = serde_json::from_str(&json).unwrap();
        let rest = drain(&mut w, OW);
        let rest_back = drain(&mut back, OW);
        assert_eq!(rest, rest_back);
        assert!(!rest_back.iter().any(|(_, c)| done.contains(c)));
        back.advance(OW, 1000, &|_| true);
        w.advance(OW, 1000, &|_| true);
        assert_eq!(drain(&mut w, OW), drain(&mut back, OW));
        back.chunk_loaded(OW, [2, 2]);
        assert_eq!(drain(&mut back, OW), vec![(1, [2, 2])]);
    }

    // ---- the command (Task 4) ----------------------------------------------

    #[test]
    fn the_blast_command_reads_a_radius_and_an_optional_place() {
        let at = [-0.5, 70.9, 10.2];
        assert_eq!(parse_blast(&["50"], at), Some((50, [-1, 70, 10])));
        assert_eq!(parse_blast(&["250", "-7", "64", "3"], at), Some((250, [-7, 64, 3])));
        assert_eq!(parse_blast(&["7"], at), None);
        assert_eq!(parse_blast(&["1025"], at), None);
        assert_eq!(parse_blast(&["50", "1", "2"], at), None);
        assert_eq!(parse_blast(&["fifty"], at), None);
        assert_eq!(parse_blast(&[], at), None);
    }

    #[test]
    fn blasts_know_their_dimension_and_get_their_own_ids() {
        let mut saved = Saved::default();
        assert_eq!(saved.blast("minecraft:overworld", [0, 64, 0], 50), Some(1));
        assert_eq!(saved.blast("minecraft:the_nether", [0, 64, 0], 50), Some(2));
        assert_eq!(saved.blast("mymod:moon", [0, 64, 0], 50), None);
        let json = serde_json::to_string(&saved).unwrap();
        let mut back: Saved = serde_json::from_str(&json).unwrap();
        assert_eq!(back.blast("minecraft:overworld", [0, 64, 0], 50), Some(3));
        back.waves.advance("minecraft:the_nether", 1000, &|_| true);
        let (b, _) = back.waves.next_job("minecraft:the_nether").unwrap();
        assert_eq!((b.id, b.world), (2, World::NETHER));
    }

    #[test]
    fn a_dimension_is_idle_once_its_fronts_are_out_and_its_loaded_work_done() {
        let mut w = waves_with(&[blast([0, 64, 0], 60, 1, World::OVERWORLD)]);
        let far = [3, 0];
        assert!(!w.idle(OW));
        w.advance(OW, 1000, &|c| c != far);
        assert!(!w.idle(OW), "jobs are ready");
        drain(&mut w, OW);
        assert!(w.idle(OW), "only a deferred chunk is left");
        assert!(!w.finished());
        assert!(w.idle("minecraft:the_nether"));
    }

    // ---- review fixes ---------------------------------------------------------

    #[test]
    fn advance_and_loads_tell_whether_anything_moved() {
        let mut w = waves_with(&[blast([0, 64, 0], 60, 1, World::OVERWORLD)]);
        let far = [3, 0];
        assert!(w.advance(OW, 1000, &|c| c != far));
        drain(&mut w, OW);
        assert!(!w.advance(OW, 1001, &|_| true), "the front is out and nothing new is reached");
        assert!(!w.chunk_loaded(OW, [40, 40]), "no work waits there");
        assert!(w.chunk_loaded(OW, far));
        assert!(!w.chunk_loaded(OW, far));
    }

    #[test]
    fn nether_blasts_keep_the_bedrock_roof() {
        let b = Blast { dimension: "minecraft:the_nether".into(), ..blast([0, 70, 0], 200, 4, World::NETHER) };
        for (_, r) in all_runs(&b).iter().filter(|(_, r)| r.surface.is_none()) {
            assert!(r.y + r.height as i32 <= 123, "{r:?} reaches the roof");
        }
        assert_eq!(world_of("minecraft:the_end").map(|w| w.roof), Some(256));
        assert_eq!(world_of("minecraft:overworld").map(|w| w.roof), Some(320));
    }

    #[test]
    fn a_wave_ends_with_its_last_job_without_another_advance() {
        let mut w = waves_with(&[blast([0, 64, 0], 40, 1, World::OVERWORLD)]);
        w.advance(OW, 1000, &|_| true);
        w.work(OW, Duration::from_secs(1), &|| Duration::ZERO, &mut |_, _| true);
        assert!(w.finished());
    }

    // ---- the blast's effects (plan 2, Task 1) -------------------------------------

    #[test]
    fn the_flash_is_white_near_fades_with_distance_and_time() {
        assert_eq!(flash_alpha(0.0, 10.0, 100), 255);
        assert_eq!(flash_alpha(0.0, 1200.0, 100), 0);
        assert_eq!(flash_alpha(80.0, 10.0, 100), 0);
        assert!(flash_alpha(20.0, 10.0, 100) >= 90, "a second on, the flash still glares");
        assert_eq!(flash_alpha(-1.0, 10.0, 100), 0);
        let by_time: Vec<u8> = (0..80).map(|t| flash_alpha(t as f64, 10.0, 100)).collect();
        assert!(by_time.windows(2).all(|p| p[0] >= p[1]));
        let by_dist: Vec<u8> = (0..130).map(|d| flash_alpha(2.0, d as f64 * 10.0, 100)).collect();
        assert!(by_dist.windows(2).all(|p| p[0] >= p[1]));
        assert!(flash_alpha(0.0, 600.0, 100) > 0 && flash_alpha(0.0, 600.0, 100) < 255);
    }

    #[test]
    fn sounds_come_with_the_front_and_skip_what_is_past() {
        let far = sounds_for(250.0, 100, 0);
        assert_eq!(far.iter().map(|s| (s.event, s.delay)).collect::<Vec<_>>(), vec![("big_nukes:blast.far", 100)]);
        let near = sounds_for(50.0, 100, 0);
        let events: Vec<_> = near.iter().map(|s| (s.event, s.delay)).collect();
        assert_eq!(events, vec![("big_nukes:blast.near", 20), ("big_nukes:wave.roar", 20)]);
        assert!(sounds_for(50.0, 100, 30).is_empty());
        assert_eq!(sounds_for(50.0, 100, 5)[0].delay, 15);
        assert!(sounds_for(4000.0, 100, 0).is_empty());
        assert_eq!(sounds_for(2900.0, 1000, 0)[0].delay, 1160);
        for d in [0.0, 50.0, 500.0, 2999.0] {
            for s in sounds_for(d, 100, 0) {
                assert!((0.2..=1.0).contains(&s.volume), "{d} {}", s.volume);
                assert!((0..=1200).contains(&s.delay));
            }
        }
    }

    #[test]
    fn the_shake_starts_with_the_front_and_dies_out() {
        let arrive = 100.0 / FRONT_SPEED;
        assert_eq!(shake(arrive - 1.0, 100.0, 200, 3), [0.0; 6]);
        assert_eq!(shake(arrive + 101.0, 100.0, 200, 3), [0.0; 6]);
        assert!(shake(arrive + 1.0, 100.0, 200, 3).iter().any(|v| *v != 0.0));
        assert_eq!(shake(10_000.0, 1100.0, 200, 3), [0.0; 6]);
        assert_eq!(shake(1100.0 / FRONT_SPEED + 1.0, 1100.0, 200, 3), [0.0; 6], "beyond 5.2R nothing shakes");
        for t in 0..400 {
            let v = shake(t as f64, 10.0, 200, 9);
            assert!(v[..3].iter().all(|a| a.abs() <= 6.0) && v[3..].iter().all(|o| o.abs() <= 0.3), "{v:?}");
        }
    }

    #[test]
    fn the_ring_follows_the_front() {
        let b = blast([10, 64, -20], 100, 5, World::OVERWORLD);
        let ring = front_ring(&b, 20);
        assert_eq!(ring.len(), 16);
        for p in &ring {
            let d = (p[0] - 10.0).hypot(p[2] + 20.0);
            assert!((d - 20.0 * FRONT_SPEED).abs() < 0.01, "{d}");
            assert_eq!(p[1], 66.0);
        }
        assert!(front_ring(&b, 44).len() == 16 && front_ring(&b, 45).is_empty(), "the reach is 112 blocks");
        assert!(front_ring(&b, 0).is_empty());
    }

    #[test]
    fn damage_bands_split_at_the_zones_and_stop_at_the_reach() {
        let bands = damage_band(100, 30.0, 40.0);
        assert_eq!(bands.len(), 2);
        assert_eq!((bands[0].0, bands[0].1, bands[0].2), (30.0, 35.0, 1000.0));
        assert_eq!((bands[1].0, bands[1].1), (35.0, 40.0));
        assert!(bands[1].2 <= 20.0 && bands[1].2 >= 6.0);
        assert!(damage_band(100, 113.0, 120.0).is_empty());
        let edge = damage_band(100, 105.0, 120.0);
        assert_eq!(edge.len(), 1);
        assert!((edge[0].1 - 112.0).abs() < 1e-9);
        assert!(edge[0].2 <= 4.0 && edge[0].2 >= 1.0);
        assert!(damage_band(100, 40.0, 40.0).is_empty());
    }

    #[test]
    fn news_round_trips_and_lists_only_running_blasts_of_the_dimension() {
        let mut w = Waves::default();
        w.start(blast([0, 64, 0], 100, 1, World::OVERWORLD), 10);
        let mut other = blast([0, 64, 0], 100, 2, World::NETHER);
        other.id = 2;
        other.dimension = "minecraft:the_nether".into();
        w.start(other, 10);
        let news = w.news(OW, 40);
        assert_eq!(news.len(), 1);
        assert_eq!((news[0].id, news[0].age, news[0].radius), (1, 30, 100));
        let back: News = serde_json::from_slice(&serde_json::to_vec(&news[0]).unwrap()).unwrap();
        assert_eq!(back, news[0]);
        assert!(w.news(OW, 10 + 2400).is_empty(), "long past");
    }

    #[test]
    fn debris_flies_out_and_up_and_is_capped() {
        let small = debris(&blast([0, 64, 0], 30, 1, World::OVERWORLD));
        assert_eq!(small.len(), 15);
        let big = debris(&blast([0, 64, 0], 1000, 1, World::OVERWORLD));
        assert_eq!(big.len(), 48);
        for (pos, v, block) in &big {
            assert_eq!(pos[1], 67.0);
            assert!(v[1] >= 0.9 && v[1] <= 1.6);
            let out = v[0].hypot(v[2]);
            assert!((0.8..=1.6).contains(&out), "{out}");
            assert!(block.starts_with("minecraft:"));
        }
    }

    // ---- assets of the effects (plan 2, Task 2) -----------------------------------

    #[test]
    fn every_sound_event_has_its_files() {
        let sounds: serde_json::Value = serde_json::from_str(&file("content/assets/big_nukes/sounds.json")).unwrap();
        for event in ["blast.near", "blast.far", "wave.roar"] {
            let entry = &sounds[event];
            assert!(entry.is_object(), "{event} isn't declared");
            for s in entry["sounds"].as_array().unwrap() {
                let name = s["name"].as_str().unwrap().strip_prefix("big_nukes:").unwrap();
                assert!(std::path::Path::new(&format!("{}/content/assets/big_nukes/sounds/{name}.ogg", env!("CARGO_MANIFEST_DIR"))).exists(), "{name}");
            }
        }
        for d in [10.0, 200.0, 2000.0] {
            for s in sounds_for(d, 100, 0) {
                assert!(sounds[s.event.strip_prefix("big_nukes:").unwrap()].is_object(), "{}", s.event);
            }
        }
    }


    #[test]
    fn fronts_report_the_band_passed_this_tick() {
        let mut w = Waves::default();
        w.start(blast([0, 64, 0], 100, 1, World::OVERWORLD), 10);
        let f = w.fronts(OW, 14);
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].0.id, f[0].1, f[0].2, f[0].3), (1, 7.5, 10.0, 4));
        assert_eq!(w.fronts(OW, 10)[0].1, 0.0, "the first tick starts at the center");
        assert!(w.fronts("minecraft:the_nether", 14).is_empty());
        assert!(w.fronts(OW, 10 + 46).is_empty(), "past the reach of 112 blocks");
    }

    // ---- plan 2 review fixes ---------------------------------------------------------

    fn heard_news(radius: u32) -> News {
        News { id: 1, center: [0, 64, 0], radius, seed: 3, age: 0 }
    }

    #[test]
    fn a_heard_blast_is_felt_only_in_its_own_dimension() {
        let h = Heard { news: heard_news(500), dimension: OW.into(), at: 100.0 };
        assert!(h.felt(OW, [125.0, 64.0, 0.0], 150.0).is_some());
        assert!(h.felt("minecraft:the_nether", [125.0, 64.0, 0.0], 150.0).is_none());
    }

    #[test]
    fn a_heard_blast_uses_the_fraction_of_the_tick() {
        let h = Heard { news: heard_news(100), dimension: OW.into(), at: 100.0 };
        let (t, _) = h.felt(OW, [0.0, 64.0, 0.0], 120.4).unwrap();
        assert!((t - 20.4).abs() < 1e-9);
    }

    #[test]
    fn a_heard_blast_is_let_go_when_past_or_from_another_world() {
        let h = Heard { news: heard_news(100), dimension: OW.into(), at: 1000.0 };
        assert!(h.felt(OW, [50.0, 64.0, 0.0], 1000.0 + 50.0 / FRONT_SPEED + 201.0).is_some(), "the cloud still stands");
        assert!(h.felt(OW, [50.0, 64.0, 0.0], 1000.0 + 5999.0).is_some());
        assert!(h.felt(OW, [50.0, 64.0, 0.0], 1000.0 + 6001.0).is_none(), "the cloud is gone at five minutes");
        let far = Heard { news: heard_news(100), dimension: OW.into(), at: 0.0 };
        assert!(far.felt(OW, [16000.0, 64.0, 0.0], 6300.0).is_some(), "the sound is still on its way");
        assert!(h.felt(OW, [50.0, 64.0, 0.0], 500.0).is_none(), "a clock far behind: another world");
    }

    #[test]
    fn a_small_wave_lives_until_its_front_is_past_the_reach() {
        let mut w = waves_with(&[blast([0, 64, 0], 8, 1, World::OVERWORLD)]);
        w.advance(OW, 2, &|_| true);
        w.work(OW, Duration::from_secs(1), &|| Duration::ZERO, &mut |_, _| true);
        assert!(!w.finished(), "the front is at 5 blocks of 8.96");
        assert!(!w.idle(OW));
        assert_eq!(w.fronts(OW, 3).len(), 1);
        w.advance(OW, 5, &|_| true);
        assert!(w.finished(), "the front is out at 12.5");
    }

    // ---- the mushroom cloud (plan 3, Task 2) ------------------------------------------

    #[test]
    fn the_fireball_rises_and_slows() {
        let h = |s: f64| cloud(s * 20.0, 200).ball_h;
        assert!(h(0.0) < 1.0);
        assert!(h(10.0) > 50.0 && h(10.0) < h(30.0) && h(30.0) < h(120.0));
        assert!(h(120.0) <= 0.9 * 200.0 + 60.0 + 0.01);
        assert!(h(20.0) - h(10.0) > h(60.0) - h(50.0), "it slows");
    }

    #[test]
    fn the_cap_grows_to_a_limit() {
        let c = |s: f64| cloud(s * 20.0, 200);
        assert!(c(10.0).cap_major < c(20.0).cap_major);
        assert!((c(600.0).cap_major - 1.4 * c(600.0).ball_r).abs() < 0.01);
        assert!((c(30.0).cap_minor - 0.8 * c(30.0).ball_r).abs() < 0.01);
        assert!((c(30.0).stem_r - 0.35 * c(30.0).ball_r).abs() < 0.01);
    }

    #[test]
    fn the_fire_dies_in_a_minute() {
        assert!((cloud(0.0, 100).fire - 1.0).abs() < 1e-6);
        assert!(cloud(60.0 * 20.0, 100).fire < 0.06);
    }

    #[test]
    fn the_cloud_fades_after_three_minutes() {
        assert_eq!(cloud(170.0 * 20.0, 100).fade, 1.0);
        let mid = cloud(240.0 * 20.0, 100).fade;
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(cloud(300.0 * 20.0, 100).fade, 0.0);
        assert_eq!(cloud(400.0 * 20.0, 100).fade, 0.0);
    }

    #[test]
    fn the_ring_runs_to_the_reach_then_fades() {
        assert!((cloud(20.0, 100).ring - 50.0).abs() < 1e-3);
        assert_eq!(cloud(20.0, 100).ring_fade, 1.0);
        let reach = 100.0 * 1.12;
        assert!((cloud(1000.0, 100).ring - reach).abs() < 1e-3);
        let out_at = reach / FRONT_SPEED as f32;
        assert!(cloud(out_at as f64 + 100.0, 100).ring_fade > 0.0);
        assert_eq!(cloud(out_at as f64 + 201.0, 100).ring_fade, 0.0);
    }

    #[test]
    fn a_young_cloud_is_finite_at_age_zero() {
        let c = cloud(0.0, 8);
        for v in [c.ball_h, c.ball_r, c.stem_r, c.cap_major, c.cap_minor, c.fire, c.fade, c.ring, c.ring_fade] {
            assert!(v.is_finite() && v >= 0.0);
        }
        assert!(cloud(-5.0, 100).fade == 0.0, "not yet");
    }

    // ---- the shader (plan 3, Task 3) ------------------------------------------------

    #[test]
    fn the_sky_shader_validates() {
        let source = file("mixins/big_nukes_native/sky.rs");
        let start = source.find("const SHADER: &str = r#\"").expect("sky.rs holds the shader") + "const SHADER: &str = r#\"".len();
        let end = start + source[start..].find("\"#;").expect("the shader ends");
        let module = naga::front::wgsl::parse_str(&source[start..end]).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source[start..end])));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{e:?}"));
        for entry in ["vs", "fs"] {
            assert!(module.entry_points.iter().any(|e| e.name == entry), "{entry}");
        }
    }

    #[test]
    fn the_newest_blasts_are_drawn_first() {
        let n = |id| News { id, center: [0, 64, 0], radius: 100, seed: 1, age: 0 };
        let mut felt = vec![(5000.0, 10.0, n(1)), (30.0, 10.0, n(5)), (4000.0, 10.0, n(2)), (900.0, 10.0, n(4)), (3000.0, 10.0, n(3))];
        newest_first(&mut felt);
        let ids: Vec<u64> = felt.iter().take(4).map(|f| f.2.id).collect();
        assert_eq!(ids, vec![5, 4, 3, 2]);
    }
}
