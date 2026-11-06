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
}

impl World {
    pub const OVERWORLD: World = World { min_y: -64, height: 384 };
    pub const NETHER: World = World { min_y: 0, height: 256 };

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
    let top = blast.world.top();
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
        self.ready.is_empty() && self.deferred.is_empty() && self.reached == self.order().len()
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
        self.waves.push(Wave { blast, start: now_tick, reached: 0, ready: VecDeque::new(), deferred: BTreeSet::new(), order: vec![] });
    }

    /// Moves the front of each wave in `dimension` on to `now_tick`.
    pub fn advance(&mut self, dimension: &str, now_tick: u64, loaded: &dyn Fn([i32; 2]) -> bool) {
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
            }
        }
        self.waves.retain_mut(|w| !w.done());
    }

    /// A chunk came in: its waiting work is next.
    pub fn chunk_loaded(&mut self, dimension: &str, chunk: [i32; 2]) {
        for w in self.waves.iter_mut().filter(|w| w.blast.dimension == dimension) {
            if w.deferred.remove(&chunk) {
                w.ready.push_front(chunk);
            }
        }
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
            .all(|w| w.ready.is_empty() && w.reached == w.order().len())
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
        "minecraft:the_nether" | "minecraft:the_end" => Some(World::NETHER),
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
        .flat_map(|s| keys_in(&file(s)))
        .collect();
        assert!(!used.is_empty());
        for key in &used {
            assert!(en.contains_key(key), "{key} isn't in en_us");
        }
        for (key, line) in &en {
            let theirs = ru.get(key).unwrap_or_else(|| panic!("{key} isn't in ru_ru"));
            assert_eq!(line.matches("%s").count(), theirs.matches("%s").count(), "{key}");
            assert!(
                key.starts_with("block.") || key.starts_with("item.") || key == "big_nukes.mod.description" || used.contains(key),
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
}
