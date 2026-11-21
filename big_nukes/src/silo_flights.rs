//! The missiles' flights, the bomber's passes and the displays that show them (`silo.rs`'s `flights` module).
//!
//! The rules say where a missile is at each tick of the world's clock; here, at the end of its level's tick, its
//! display is made or moved, its trail drawn and its sounds played, the hatch opened and shut, its target's chunk
//! held and read, and its blast set off. The displays live in memory only: one missing (a chunk that went, a world
//! opened again) is made anew when the flight next shows.

use super::{play, set_slot, text};
use crate::core::{self, lock, middle};
use crate::rules;
use mod_api::{BlockUpdateFlags, Compound, Heightmap, LevelRef, ModBlockEntityView, ServerHost, Tag, Text};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/// Each flight's displays this session: (flight, part) to the entity.
static DISPLAYS: Mutex<BTreeMap<(u64, u8), u32>> = Mutex::new(BTreeMap::new());
/// Each dimension's next sweep for displays no flight owns.
static SWEEPS: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());
/// This session's mark on its displays: a world opened again finds the last session's and takes them away.
static SESSION: Mutex<Option<u64>> = Mutex::new(None);
const MISSILE_PART: u8 = 0;
const PLANE_PART: u8 = 1;
const BOMB_PART: u8 = 2;
/// Ticks between sweeps.
const SWEEP_EVERY: u64 = 200;
/// The ticks a display glides between two places: moved every tick, it never stops. The window's camera looks this
/// far behind (`silo_window.rs`).
pub(super) const GLIDE: i32 = 2;
/// A sound is heard 16 blocks for each of volume: the launch's roar about 300.
const ROAR_VOLUME: f32 = 19.0;
/// The drone of the bomber is heard about 190 blocks off.
const DRONE_VOLUME: f32 = 12.0;
/// How far a beacon's news is told.
const BEACON_TOLD: f64 = 64.0;

pub(super) fn forget() {
    lock(&DISPLAYS).clear();
    lock(&PADS).clear();
    lock(&SWEEPS).clear();
    *lock(&SESSION) = None;
}

/// A command in `dimension`, from a level's tick, where commands run. Its answer isn't an error: false is also
/// smoke no player is near enough to see.
fn run_in(host: &mut dyn ServerHost, dimension: &str, command: &str) {
    core::run(host, &format!("execute in {dimension} run {command}"));
}

/// Particles seen from afar (`force`): the API's own reach 32 blocks.
fn particle(id: &str, at: [f64; 3], spread: f64, speed: f64, count: u32) -> String {
    format!("particle {id} {:.2} {:.2} {:.2} {spread:.2} {spread:.2} {spread:.2} {speed:.2} {count} force", at[0], at[1], at[2])
}

fn session() -> u64 {
    *lock(&SESSION).get_or_insert_with(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_millis() as u64 % 1_000_000_000)
    })
}

fn session_tag() -> String {
    format!("{}_s{}", rules::FLIGHT_TAG, session())
}

fn flight_tag(id: u64) -> String {
    format!("{}_{id}", rules::FLIGHT_TAG)
}

fn flight_tags(flight: u64) -> [String; 3] {
    [rules::FLIGHT_TAG.to_owned(), flight_tag(flight), session_tag()]
}

/// An item display of `item` with `tags`, lit as by day, seen from afar and gliding between its places. Its model
/// lies along z: `scale` makes it its size and `lift` moves it forward along its look.
fn display_nbt(item: &str, scale: f32, lift: f32, yaw: f32, pitch: f32, tags: &[String]) -> Compound {
    let float = |v: f32| Tag::Float(v.to_bits());
    let floats = |v: [f32; 4]| Tag::List(v.into_iter().map(float).collect());
    let three = |v: [f32; 3]| Tag::List(v.into_iter().map(float).collect());
    [
        ("item", Tag::compound([("id", Tag::String(item.into())), ("count", Tag::Int(1))])),
        ("item_display", Tag::String("none".into())),
        ("view_range", float(12.0)),
        ("teleport_duration", Tag::Int(GLIDE)),
        ("brightness", Tag::compound([("sky", Tag::Int(15)), ("block", Tag::Int(15))])),
        ("Tags", Tag::List(tags.iter().map(|t| Tag::String(t.as_str().into())).collect())),
        ("Rotation", Tag::List(vec![float(yaw), float(pitch)])),
        (
            "transformation",
            Tag::compound([
                ("left_rotation", floats([0.0, 0.0, 0.0, 1.0])),
                ("right_rotation", floats([0.0, 0.0, 0.0, 1.0])),
                ("translation", three([0.0, 0.0, lift])),
                ("scale", three([scale; 3])),
            ]),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect()
}

/// The display `id` of the level in place moved to `pos`, turned: false when it is gone.
fn move_display(host: &mut dyn ServerHost, id: u32, pos: [f64; 3], yaw: f32, pitch: f32) -> bool {
    match host.entity(id) {
        Some(e) => e.teleport_to(host, pos, Some([yaw, pitch])),
        None => false,
    }
}

fn remove_display(host: &mut dyn ServerHost, id: u32) {
    if let Some(e) = host.entity(id) {
        let _ = e.discard(host);
    }
}

/// A new display in the level in place: its id, none in a chunk that isn't loaded.
fn summon_display(host: &mut dyn ServerHost, dimension: &str, pos: [f64; 3], yaw: f32, pitch: f32, nbt: Compound) -> Option<u32> {
    let id = host.summon(dimension, "minecraft:item_display", pos, nbt)?;
    move_display(host, id, pos, yaw, pitch);
    Some(id)
}

/// A flight's display at `pos`, made when it is missing.
#[allow(clippy::too_many_arguments)]
fn show(host: &mut dyn ServerHost, dimension: &str, key: (u64, u8), item: &str, scale: f32, lift: f32, pos: [f64; 3], yaw: f32, pitch: f32) {
    let known = lock(&DISPLAYS).get(&key).copied();
    if let Some(id) = known {
        if move_display(host, id, pos, yaw, pitch) {
            return;
        }
        lock(&DISPLAYS).remove(&key);
    }
    // None in a chunk that isn't loaded: it is tried again the next tick.
    let nbt = display_nbt(item, scale, lift, yaw, pitch, &flight_tags(key.0));
    if let Some(id) = summon_display(host, dimension, pos, yaw, pitch, nbt) {
        lock(&DISPLAYS).insert(key, id);
    }
}

fn hide(host: &mut dyn ServerHost, key: (u64, u8)) {
    let id = lock(&DISPLAYS).remove(&key);
    if let Some(id) = id {
        remove_display(host, id);
    }
}

/// The displays no flight owns: on a dimension's first tick this session every tagged one (a closed world's),
/// then now and again those of another session or of a flight that is over. The pads' too, a closed world's or
/// another session's; this session's are each kept alone at its pad (`alone`).
fn sweep(host: &mut dyn ServerHost, dimension: &str, now: u64, live: &[u64]) {
    let first = {
        let mut sweeps = lock(&SWEEPS);
        match sweeps.get(dimension) {
            Some(next) if now < *next => return,
            seen => {
                let first = seen.is_none();
                sweeps.insert(dimension.to_owned(), now + SWEEP_EVERY);
                first
            }
        }
    };
    let (tag, pads) = (rules::FLIGHT_TAG, rules::PAD_TAG);
    if first {
        run_in(host, dimension, &format!("kill @e[type=minecraft:item_display,tag={tag}]"));
        run_in(host, dimension, &format!("kill @e[type=minecraft:item_display,tag={pads}]"));
        return;
    }
    run_in(host, dimension, &format!("kill @e[type=minecraft:item_display,tag={tag},tag=!{}]", session_tag()));
    run_in(host, dimension, &format!("kill @e[type=minecraft:item_display,tag={pads},tag=!{}]", pad_session_tag()));
    let keep: String = live.iter().map(|id| format!(",tag=!{}", flight_tag(*id))).collect();
    run_in(host, dimension, &format!("kill @e[type=minecraft:item_display,tag={tag}{keep}]"));
}

/// What a tick of the flights changes in the world's record, made with the state held once.
enum Change {
    Surface(u64, i32),
    HatchOpened(u64),
    HatchShut(u64),
    Force(u64, [i32; 2]),
    Emptied(u64),
    Impact(u64, [i32; 3], rules::BombKind),
}

/// The missiles of `dimension`.
pub(super) fn flights(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    let (now, mine, live) = {
        let Some(guard) = core::state(host) else { return };
        let st = guard.as_ref().expect("opened");
        let mine: Vec<rules::Flight> = st.saved.flights.iter().filter(|f| f.dimension == dimension).cloned().collect();
        let live: Vec<u64> = st.saved.flights.iter().chain(&st.saved.beacons).map(|f| f.id).collect();
        (st.saved.tick, mine, live)
    };
    sweep(host, dimension, now, &live);
    let mut changes = Vec::new();
    for f in &mine {
        missile(host, level, dimension, f, now, &mut changes);
    }
    settle(host, dimension, changes);
}

/// The world's record takes a tick's changes; the blasts go off, and the chunks no flight keeps are let go.
fn settle(host: &mut dyn ServerHost, dimension: &str, changes: Vec<Change>) {
    let mut fired = Vec::new();
    for c in &changes {
        if let Change::Impact(id, at, kind) = *c {
            // The blast's own record and news, with the state let go.
            let center = match core::start_bomb(host, dimension, at, kind, kind.radius()) {
                Ok(blast) => {
                    log::info!("[big_nukes] flight {id} went off at {at:?} in {dimension}: blast {blast}");
                    Some(at)
                }
                Err(why) => {
                    log::info!("[big_nukes] flight {id} at {at:?} in {dimension} didn't go off: {why:?}");
                    core::tell_near(host, dimension, at, &core::refused(why));
                    None
                }
            };
            fired.push((id, center));
        }
    }
    let (force, free) = {
        let Some(mut guard) = core::state(host) else { return };
        let st = guard.as_mut().expect("opened");
        let (mut force, mut released) = (Vec::new(), Vec::new());
        // Saved only when something changed: most ticks only ask again for a chunk already held.
        let forced = st.saved.forced.len();
        let mut changed = !fired.is_empty();
        for c in changes {
            match c {
                Change::Surface(id, y) => {
                    if let Some(f) = st.saved.flights.iter_mut().chain(&mut st.saved.beacons).find(|f| f.id == id) {
                        changed |= f.surface != Some(y);
                        f.surface = Some(y);
                    }
                }
                Change::HatchOpened(id) => {
                    if let Some(f) = st.saved.flights.iter_mut().find(|f| f.id == id) {
                        changed |= f.hatch_moved != Some(true);
                        f.hatch_moved = Some(true);
                    }
                }
                Change::HatchShut(id) => {
                    if let Some(f) = st.saved.flights.iter_mut().find(|f| f.id == id) {
                        changed |= f.hatch.is_some();
                        f.hatch = None;
                        // The pad's chunk was kept for the hatch only.
                        let pad = rules::chunk_of(f.from[0], f.from[2]);
                        if st.saved.release(id, dimension, pad) {
                            released.push((dimension.to_owned(), pad));
                        }
                    }
                }
                Change::Force(id, chunk) => {
                    if st.saved.force(id, dimension, chunk) {
                        force.push(chunk);
                    }
                }
                Change::Emptied(id) => changed |= st.saved.emptied(id),
                Change::Impact(..) => {}
            }
        }
        for (id, center) in &fired {
            st.saved.flights.retain(|f| f.id != *id);
            st.saved.beacons.retain(|f| f.id != *id);
            st.saved.landed(*id, *center);
        }
        let mut free = st.saved.unforce();
        free.extend(released);
        if changed || st.saved.forced.len() != forced {
            core::save(st);
        }
        (force, free)
    };
    // The mod's own chunk tickets hold them: a player's `/forceload` of the same chunk is a flag of its own, which
    // the mod neither asks about nor lets go.
    for chunk in force {
        if core::force_chunk(host, dimension, chunk, true) {
            log::info!("[big_nukes] holding chunk {chunk:?} of {dimension} by ticket");
        } else {
            log::warn!("[big_nukes] couldn't hold chunk {chunk:?} of {dimension}");
        }
    }
    for (d, chunk) in free {
        if core::force_chunk(host, &d, chunk, false) {
            log::info!("[big_nukes] let go of chunk {chunk:?} of {d}");
        } else {
            log::warn!("[big_nukes] couldn't let go of chunk {chunk:?} of {d}: its level isn't open");
        }
    }
    for (id, _) in fired {
        for part in 0..3 {
            hide(host, (id, part));
        }
        run_in(host, dimension, &format!("kill @e[type=minecraft:item_display,tag={}]", flight_tag(id)));
    }
}

/// The ground's height at a column of a loaded chunk of the ticking level: one over its highest block.
fn ground_at(host: &dyn ServerHost, dimension: &str, x: i32, z: i32) -> Option<i32> {
    host.surface_heights(dimension, &[(x, z)], Heightmap::WorldSurface).ok()?.first().copied().flatten()
}

/// A missile's tick, by its leg.
fn missile(host: &mut dyn ServerHost, level: LevelRef, dimension: &str, f: &rules::Flight, now: u64, changes: &mut Vec<Change>) {
    let pad = [f64::from(f.from[0]) + 0.5, f64::from(f.from[1]), f64::from(f.from[2]) + 0.5];
    hatch(host, level, dimension, f, now, changes);
    let key = (f.id, MISSILE_PART);
    let leg = f.leg(now);
    if now >= f.liftoff() {
        // The target is held from the liftoff, so its chunk is in by the time it is wanted.
        changes.push(Change::Force(f.id, f.chunk_to()));
    }
    // A splitting missile's riders are in the one seen until they part over their targets.
    let leg = match leg {
        rules::Leg::Countdown { .. } | rules::Leg::Ascent { .. } if f.rider => rules::Leg::Cruise,
        leg => leg,
    };
    match leg {
        rules::Leg::Countdown { left } => {
            // It stands in the shaft where the pad showed it, turning to face its target.
            let (pos, yaw, pitch) = f.standing(now);
            show(host, dimension, key, rules::MISSILE_MODEL, rules::MISSILE_SCALE, rules::MISSILE_LIFT, pos, yaw, pitch);
            if left % rules::SECOND == 0 {
                let pitch = 1.0 + (rules::LAUNCH_COUNTDOWN.saturating_sub(left) as f32) / 200.0;
                play(host, dimension, "minecraft:block.note_block.pling", pad, 3.0, pitch);
            }
            if left <= rules::SECOND && left % 2 == 0 {
                run_in(host, dimension, &particle("minecraft:large_smoke", [pad[0], pad[1] + 0.5, pad[2]], 0.6, 0.05, 8));
            }
        }
        rules::Leg::Ascent { pos, yaw, pitch } => {
            show(host, dimension, key, rules::MISSILE_MODEL, rules::MISSILE_SCALE, rules::MISSILE_LIFT, pos, yaw, pitch);
            let t = now - f.liftoff();
            if t == 0 {
                play(host, dimension, rules::MISSILE_LAUNCH, pad, ROAR_VOLUME, 1.0);
            }
            if t < rules::SECOND {
                // Smoke boils out of the shaft as it goes.
                run_in(host, dimension, &particle("minecraft:campfire_signal_smoke", [pad[0], pad[1] + 1.0, pad[2]], 0.8, 0.08, 6));
            }
            if t % 2 == 0 {
                trail(host, dimension, pos, true);
            }
        }
        rules::Leg::Cruise => {
            hide(host, key);
            if f.surface.is_none() {
                if let Some(y) = ground_at(host, dimension, f.to[0], f.to[1]) {
                    changes.push(Change::Surface(f.id, y));
                } else if f.surface_overdue(now) {
                    log::info!("[big_nukes] flight {}: the target's ground never loaded; it lands at {}", f.id, rules::SURFACE_GUESS);
                    changes.push(Change::Surface(f.id, rules::SURFACE_GUESS));
                }
            }
        }
        rules::Leg::Descent { pos, yaw, pitch } => {
            show(host, dimension, key, rules::MISSILE_MODEL, rules::MISSILE_SCALE, rules::MISSILE_LIFT, pos, yaw, pitch);
            if now % 2 == 0 {
                trail(host, dimension, pos, false);
            }
            if now + 40 == f.lands_at() {
                // Its fall's whistle, from where it will land.
                let target = [f64::from(f.to[0]) + 0.5, pos[1].min(f64::from(f.surface.unwrap_or(64)) + 20.0), f64::from(f.to[1]) + 0.5];
                play(host, dimension, "minecraft:entity.phantom.swoop", target, 12.0, 0.5);
            }
        }
        rules::Leg::Impact { at } => {
            hide(host, key);
            changes.push(Change::Impact(f.id, at, f.kind));
        }
    }
}

/// The smoke and fire a missile leaves behind its tail.
fn trail(host: &mut dyn ServerHost, dimension: &str, tail: [f64; 3], climbing: bool) {
    let (smoke, fire) = if climbing { (5, 6) } else { (3, 3) };
    run_in(host, dimension, &particle("minecraft:campfire_signal_smoke", tail, 0.35, 0.02, smoke));
    run_in(host, dimension, &particle("minecraft:flame", tail, 0.2, 0.04, fire));
}

/// The hatch: taken away while it should stand open, and the one taken put back after (`Flight::hatch_act`);
/// once back, the flight forgets it. The pad's chunk is kept loaded meanwhile, and the hatch waits for it.
fn hatch(host: &mut dyn ServerHost, level: LevelRef, dimension: &str, f: &rules::Flight, now: u64, changes: &mut Vec<Change>) {
    let Some(y) = f.hatch else { return };
    let at = [f.from[0], y, f.from[2]];
    changes.push(Change::Force(f.id, rules::chunk_of(at[0], at[2])));
    let shut = host.block_key(rules::SILO_HATCH).and_then(|k| host.block_state_default(k));
    let (Some(air), Some(shut)) = (host.block_state_default(crate::keys().air), shut) else {
        return;
    };
    let here = core::loaded(dimension, at).then(|| core::block_at(host, level, at));
    let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
    match f.hatch_act(now, here.as_deref()) {
        Some(rules::HatchAct::Open) => {
            if matches!(level.set_block(host, at, air, flags), Ok(true)) {
                play(host, dimension, "minecraft:block.iron_trapdoor.open", middle(at), 2.0, 0.6);
                changes.push(Change::HatchOpened(f.id));
            }
        }
        // Tried again the next tick if the level didn't take it.
        Some(rules::HatchAct::Shut) => {
            if matches!(level.set_block(host, at, shut, flags), Ok(true)) {
                play(host, dimension, "minecraft:block.iron_trapdoor.close", middle(at), 2.0, 0.6);
                changes.push(Change::HatchShut(f.id));
            }
        }
        // Over the pad, where it falls into the shaft; tried again the next tick if it couldn't be made.
        Some(rules::HatchAct::Drop) => {
            let item: Compound =
                [("Item".into(), Tag::compound([("id", Tag::String(rules::SILO_HATCH.into())), ("count", Tag::Int(1))]))]
                    .into_iter()
                    .collect();
            if host.summon(dimension, "minecraft:item", middle(f.from), item).is_some() {
                changes.push(Change::HatchShut(f.id));
            }
        }
        Some(rules::HatchAct::Forget) => changes.push(Change::HatchShut(f.id)),
        None => {}
    }
}

// ---- the bomber -----------------------------------------------------------------------
//
// Thirty seconds of red smoke after a beacon is charged, the bomber flies over at the end of the beacon's level's
// tick, as a display, and the bomb leaves it as another; where the bomb meets the ground it goes off.

/// The strikes of `dimension`: the beacon's smoke and news, the bomber's pass, the bomb's fall and its blast.
pub(super) fn strikes(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    let (now, mine) = {
        let Some(guard) = core::state(host) else { return };
        let st = guard.as_ref().expect("opened");
        (st.saved.tick, st.saved.beacons.iter().filter(|f| f.dimension == dimension).cloned().collect::<Vec<_>>())
    };
    if mine.is_empty() {
        return;
    }
    let mut changes = Vec::new();
    for f in &mine {
        strike(host, level, dimension, f, now, &mut changes);
    }
    settle(host, dimension, changes);
}

fn strike(host: &mut dyn ServerHost, level: LevelRef, dimension: &str, f: &rules::Flight, now: u64, changes: &mut Vec<Change>) {
    let beacon = middle(f.from);
    let pass = f.pass();
    // The beacon's chunk stays loaded until the blast has worked it.
    changes.push(Change::Force(f.id, rules::chunk_of(f.from[0], f.from[2])));
    let comes = pass.arrives();
    if now < comes {
        if now % 10 == 0 {
            let smoke = [beacon[0], beacon[1] + 1.5, beacon[2]];
            run_in(host, dimension, &particle("minecraft:dust{color:[1.0,0.0,0.0],scale:2.0}", smoke, 0.3, 0.02, 12));
        }
        if comes - now == 10 * rules::SECOND {
            let line = text(&rules::Reply::new("big_nukes.beacon.soon", &[&10]));
            tell_around(host, dimension, beacon, &line);
        }
        return;
    }
    if now == comes {
        play(host, dimension, rules::BOMBER_FLYBY, [beacon[0], beacon[1] + 30.0, beacon[2]], DRONE_VOLUME, 1.0);
    }
    match pass.plane(now) {
        Some((at, yaw)) => show(host, dimension, (f.id, PLANE_PART), rules::BOMBER_MODEL, rules::BOMBER_SCALE, 0.0, at, yaw, 0.0),
        None => hide(host, (f.id, PLANE_PART)),
    }
    if now < pass.release {
        return;
    }
    // The bomb leaves the beacon's slot as it leaves the plane, tried again each tick until it is done: a beacon
    // broken before then drops it and calls the strike off, one broken after drops nothing.
    if f.to_empty(now) && set_slot(host, level, f.from, 0, None).is_some() {
        changes.push(Change::Emptied(f.id));
    }
    let surface = match f.surface {
        Some(y) => Some(y),
        None => {
            let read = ground_at(host, dimension, f.from[0], f.from[2]);
            if let Some(y) = read {
                changes.push(Change::Surface(f.id, y));
            }
            read
        }
    };
    let ground = rules::Flight { surface, ..f.clone() }.bomb_ground();
    let lands = pass.impact(ground);
    match (pass.bomb(now), lands) {
        (Some(_), Some(t)) if now >= t => {
            let at = pass.bomb(t).unwrap_or(beacon);
            hide(host, (f.id, BOMB_PART));
            changes.push(Change::Impact(f.id, [at[0].floor() as i32, ground, at[2].floor() as i32], f.kind));
        }
        (Some(at), _) => {
            let (_, yaw) = pass.plane(pass.release).unwrap_or((at, 0.0));
            // It noses down as it falls.
            let pitch = ((now - pass.release) as f32 * 1.5).min(80.0);
            show(host, dimension, (f.id, BOMB_PART), f.kind.id(), 1.0, 0.0, at, yaw, pitch);
        }
        // Past the end of its worked-out fall: it goes off at the beacon.
        (None, _) => {
            hide(host, (f.id, BOMB_PART));
            changes.push(Change::Impact(f.id, [f.from[0], ground, f.from[2]], f.kind));
        }
    }
}

/// A line to every player of `dimension` near `at`.
fn tell_around(host: &mut dyn ServerHost, dimension: &str, at: [f64; 3], line: &Text) {
    for p in core::players(host) {
        if p.dimension == dimension && core::distance(p.position, at) <= BEACON_TOLD {
            core::tell(host, p.uuid, line.clone());
        }
    }
}

// ---- the missile on its pad ----------------------------------------------------------
//
// A pad with a missile in its slot shows it standing in the shaft, as a display in the very pose its flight's
// starts the climb in (`rules::pad_pose`). The pad's tick tells what its slot holds; the display is made, taken
// away and checked at the end of its level's tick. A launch takes it away as the flight's own takes its place,
// and none shows while a flight stands in the shaft. Each display is tagged with its pad, this session and a
// serial of its own, so a copy that comes back with its chunk, or one a closed world left, goes.

/// A pad as its tick last saw it.
#[derive(Default)]
struct Pad {
    /// What its missile slot holds.
    missile: Option<String>,
    /// The tick its block entity last ticked.
    seen: u64,
    /// Its display: the entity and its serial.
    display: Option<(u32, u64)>,
}

/// The pads seen this session, by dimension and place.
static PADS: Mutex<BTreeMap<String, BTreeMap<[i32; 3], Pad>>> = Mutex::new(BTreeMap::new());
/// The serial the next pad's display is tagged with.
static PAD_SERIAL: AtomicU64 = AtomicU64::new(0);

fn pad_session_tag() -> String {
    format!("{}_s{}", rules::PAD_TAG, session())
}

fn pad_serial_tag(serial: u64) -> String {
    format!("{}_{serial}", pad_session_tag())
}

/// The pad's tick: what its missile slot holds, written down when it changes.
pub(super) fn pad_tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &ModBlockEntityView<'_>) {
    let Some((now, _)) = core::clock(host) else { return };
    let slot = entity.items.get(rules::PAD_MISSILE_SLOT).and_then(Option::as_ref).map(|s| s.item().id(host));
    let dimension = host.dimension();
    let mut pads = lock(&PADS);
    let pad = pads.entry(dimension.to_owned()).or_default().entry(pos).or_default();
    if pad.missile != slot {
        pad.missile = slot;
    }
    pad.seen = now;
}

/// The pads of `dimension` at the end of its tick: each one's display made, taken away or checked.
pub(super) fn pads(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    let mine: Vec<([i32; 3], Option<String>, u64, bool)> = match lock(&PADS).get(dimension) {
        Some(pads) if !pads.is_empty() => pads.iter().map(|(pos, p)| (*pos, p.missile.clone(), p.seen, p.display.is_some())).collect(),
        _ => return,
    };
    let (now, busy) = {
        let Some(guard) = core::state(host) else { return };
        let st = guard.as_ref().expect("opened");
        let now = st.saved.tick;
        let busy: Vec<bool> = mine.iter().map(|(p, ..)| st.saved.pad_busy(dimension, [p[0], p[1] + 1, p[2]], now)).collect();
        (now, busy)
    };
    for ((pos, missile, seen, shown), busy) in mine.into_iter().zip(busy) {
        if seen + rules::SECOND < now {
            // Its block entity stopped ticking. With its chunk gone, so did the display, and a pad that is no
            // more takes it along; one only left without its ticks keeps it.
            if !core::loaded(dimension, pos) {
                forget_pad(dimension, pos);
            } else if core::block_at(host, level, pos) != rules::LAUNCH_PAD {
                hide_pad(host, dimension, pos);
                forget_pad(dimension, pos);
            }
            continue;
        }
        match rules::pad_act(shown, rules::pad_shows(missile.as_deref(), busy), now) {
            Some(rules::PadAct::Show) => show_pad(host, dimension, pos),
            Some(rules::PadAct::Hide) => hide_pad(host, dimension, pos),
            Some(rules::PadAct::Check) => check_pad(host, dimension, pos),
            None => {}
        }
    }
}

/// A pad taken away: its display goes with it.
pub(super) fn pad_taken(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3]) {
    if with_pad(dimension, pos, |_| ()).is_some() {
        hide_pad(host, dimension, pos);
        forget_pad(dimension, pos);
    }
}

fn with_pad<R>(dimension: &str, pos: [i32; 3], f: impl FnOnce(&mut Pad) -> R) -> Option<R> {
    lock(&PADS).get_mut(dimension).and_then(|pads| pads.get_mut(&pos)).map(f)
}

fn forget_pad(dimension: &str, pos: [i32; 3]) {
    if let Some(pads) = lock(&PADS).get_mut(dimension) {
        pads.remove(&pos);
    }
}

/// The pad's display made, and any other at the pad taken away; tried again the next tick if it couldn't be made.
fn show_pad(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3]) {
    let serial = PAD_SERIAL.fetch_add(1, Ordering::Relaxed);
    let (at, yaw, pitch) = rules::pad_pose(pos);
    let tags = [rules::PAD_TAG.to_owned(), rules::pad_tag(pos), pad_session_tag(), pad_serial_tag(serial)];
    let nbt = display_nbt(rules::MISSILE_MODEL, rules::MISSILE_SCALE, rules::MISSILE_LIFT, yaw, pitch, &tags);
    let Some(id) = summon_display(host, dimension, at, yaw, pitch, nbt) else { return };
    with_pad(dimension, pos, |p| p.display = Some((id, serial)));
    alone(host, dimension, pos, serial);
}

/// Every display at the pad but the one of `serial` taken away: one that came back with its chunk, say.
fn alone(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3], serial: u64) {
    run_in(host, dimension, &format!("kill @e[type=minecraft:item_display,tag={},tag=!{}]", rules::pad_tag(pos), pad_serial_tag(serial)));
}

/// Once a second: the pad's display still there, else made again, and alone.
fn check_pad(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3]) {
    let Some(Some((id, serial))) = with_pad(dimension, pos, |p| p.display) else { return };
    let (at, yaw, pitch) = rules::pad_pose(pos);
    if move_display(host, id, at, yaw, pitch) {
        alone(host, dimension, pos, serial);
    } else {
        with_pad(dimension, pos, |p| p.display = None);
        show_pad(host, dimension, pos);
    }
}

/// The pad's display taken away, and any other at the pad.
pub(super) fn hide_pad(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3]) {
    if let Some(Some((id, _))) = with_pad(dimension, pos, |p| p.display.take()) {
        remove_display(host, id);
    }
    run_in(host, dimension, &format!("kill @e[type=minecraft:item_display,tag={}]", rules::pad_tag(pos)));
}
