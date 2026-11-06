// Big Nukes' rules: no game type here, so they build and test on their own.
// The bundle adds this file to the game as `game::big_nukes`; the glue
// (`game::big_nukes_glue`, glue.rs) connects it to the game.

use serde::{Deserialize, Serialize};

pub const ID: &str = "big_nukes_native";
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
    MotionBlocking,
    MotionBlockingNoLeaves,
    OceanFloor,
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
    "#minecraft:dirt",
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
const DIRT: u32 = 3;
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
                    on(Surface::MotionBlockingNoLeaves, -1, 1, SCORCHED, DIRT);
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
}
