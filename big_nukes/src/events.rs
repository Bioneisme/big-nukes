//! The radar and the events (plan 13): the storms, the mutants, the satellites, their notices and the radar. The
//! rules (`Events`, `Saved::tick_events`, the mutants' and the satellite's) are rules.rs'.
//!
//! The API reads the ground (`surface_heights`) and throws falling blocks only in the level being ticked, so what
//! the server's tick decides for the ground (a satellite's target read, its fall and debris, a storm's packs) waits
//! in `PENDING` for that level's tick, a tick at most.

use crate::core::{self, lock};
use crate::core::Pl;
use crate::{bunker, rad, rules, silo};
use mod_api::{Block, EntityRef, Falling, GameMode, Heightmap, LevelRef, ServerHost, Text};
use std::sync::Mutex;

const OVERWORLD: &str = "minecraft:overworld";
/// The chunk ticket a falling satellite holds on its target, as a flight's (`Saved::force`).
const SATELLITE_TICKET: u64 = u64::MAX - 7;

// ---- the blocks ------------------------------------------------------------------------

/// The radar and the satellite's debris, `block(id, template, hardness)` as the core's.
pub(crate) fn blocks(blocks: &mut Vec<Block>, block: &dyn Fn(&str, &str, f32) -> Block) {
    // On the furnace, as the console: it faces its placer; its right click is the mod's, so no menu opens.
    blocks.push(block(rules::RADAR, "minecraft:furnace", 3.5));
    blocks.push(block(rules::SATELLITE_DEBRIS, "minecraft:iron_block", 4.0));
}

/// The events' blocks with a block entity: registered with the core's `Machine` part (no container).
pub(crate) fn has_entity(block: &str) -> bool {
    block == rules::RADAR
}

// ---- the radar -----------------------------------------------------------------------

/// The radars heard this session: dimension, place, whether powered, the tick last heard. A radar works while its
/// chunk is loaded and a generator burns near it.
static RADARS: Mutex<Vec<(String, [i32; 3], bool, u64)>> = Mutex::new(Vec::new());
/// A radar heard within this many ticks still counts.
const RADAR_HEARD: u64 = 2 * rules::SECOND;

/// A radar's server tick (it is heard, powered or not). `host.dimension()` is its level's. Once a second it says
/// where it is and whether it has power.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let Some((now, _)) = core::clock(host) else { return };
    if now % rules::SECOND != 0 {
        return;
    }
    let dimension = host.dimension();
    let powered = bunker::powered_now(dimension, pos);
    let mut radars = lock(&RADARS);
    radars.retain(|r| !(r.0 == dimension && r.1 == pos) && r.3 + RADAR_HEARD >= now);
    radars.push((dimension.to_owned(), pos, powered, now));
}

/// Whether a powered radar heard lately is within reach of `pos` in `dimension`.
fn radar_near(dimension: &str, pos: [f64; 3], now: u64) -> bool {
    lock(&RADARS).iter().any(|r| {
        r.0 == dimension
            && r.2
            && r.3 + RADAR_HEARD >= now
            && (f64::from(r.1[0]) + 0.5 - pos[0]).hypot(f64::from(r.1[2]) + 0.5 - pos[2]) <= rules::RADAR_REACH
    })
}

/// The overworld's powered radars heard lately, for the sirens of its events.
fn powered_radars(now: u64) -> Vec<(String, [i32; 3])> {
    lock(&RADARS).iter().filter(|r| r.0 == OVERWORLD && r.2 && r.3 + RADAR_HEARD >= now).map(|r| (r.0.clone(), r.1)).collect()
}

/// A right click on the radar: whether it was. Set `canceled` to keep the game's own use. Its report goes in chat.
pub(crate) fn right_click(host: &mut dyn ServerHost, who: &Pl, pos: [i32; 3], canceled: &mut bool) -> bool {
    let Some(level) = host.level(&who.dimension) else { return false };
    if core::block_at(host, level, pos) != rules::RADAR {
        return false;
    }
    *canceled = true;
    if !bunker::powered_now(&who.dimension, pos) {
        core::tell(host, who.uuid, core::tr("big_nukes.radar.unpowered", Vec::new()));
        return true;
    }
    let lines = {
        let Some(guard) = core::state(host) else { return true };
        let st = guard.as_ref().expect("opened");
        rules::radar_lines(&st.saved.events, st.saved.flights.len(), st.saved.tick)
    };
    for line in lines {
        let args: Vec<Text> = line.args.iter().map(core::lit).collect();
        core::tell(host, who.uuid, core::tr(&line.key, args));
    }
    true
}

// ---- the command -----------------------------------------------------------------------

/// `/bignukes events ...`, its words after `events`: the reply.
pub(crate) fn command(host: &mut dyn ServerHost, args: &[&str]) -> Text {
    let Some(mut guard) = core::state(host) else {
        return core::tr("big_nukes.events.usage", Vec::new());
    };
    let st = guard.as_mut().expect("opened");
    let line = match args {
        ["trigger", name] => match rules::EventKind::of_name(name) {
            Some(kind) => st.saved.trigger(kind),
            None => "big_nukes.events.usage",
        },
        _ => st.saved.events.command(args).unwrap_or("big_nukes.events.usage"),
    };
    core::save(st);
    core::tr(line, Vec::new())
}

// ---- the server's tick -------------------------------------------------------------------

/// What a storm or a satellite asks of the game this tick, done with the state let go.
#[derive(Default)]
struct Work {
    notices: Vec<(u128, rules::Notice)>,
    commands: Vec<String>,
    force: Vec<[i32; 2]>,
    packs: Vec<(u128, [f64; 3])>,
    fell: Vec<rules::Satellite>,
}

/// What waits for a level's tick: the storms' packs (the overworld's) and the satellites down.
struct Pending {
    packs: Vec<(u128, [f64; 3])>,
    fell: Vec<rules::Satellite>,
}

static PENDING: Mutex<Pending> = Mutex::new(Pending { packs: Vec::new(), fell: Vec::new() });

fn playing(p: &Pl) -> bool {
    p.game_mode != GameMode::Spectator
}

/// The end of the server's tick (after the core's): the events' clock, notices, storm, satellite. Their notices
/// once a second, the storm's ash, the satellite's trail; its ground, fall and the packs go to the level's tick.
pub(crate) fn server_tick(host: &mut dyn ServerHost) {
    let players = core::players(host);
    let mut work = Work::default();
    {
        let Some(mut guard) = core::state(host) else { return };
        let st = guard.as_mut().expect("opened");
        if !st.saved.events.started {
            return;
        }
        let now = st.saved.tick;
        let overworld: Vec<[f64; 2]> =
            players.iter().filter(|p| p.dimension == OVERWORLD && playing(p)).map(|p| [p.position[0], p.position[2]]).collect();
        // The places a satellite keeps clear of, read only when one is about to be set.
        let ev = &st.saved.events;
        let built: Vec<[i32; 2]> = if ev.satellite.is_none() && now + rules::SATELLITE_WARNING >= ev.next_satellite {
            st.saved.walls.values().flatten().map(|w| [w.pos[0], w.pos[2]]).chain(st.saved.placed.iter().map(|b| [b.pos[0], b.pos[2]])).collect()
        } else {
            Vec::new()
        };
        let spawn = [0.0, 0.0];
        let happened = st.saved.tick_events(&overworld, spawn, &built);
        for h in &happened {
            match h {
                rules::Happening::StormSet(storm) => {
                    log::info!("[big_nukes] a radioactive storm from tick {} to {}", storm.from, storm.until);
                    // The sirens by the powered radars wail until it comes.
                    for (dimension, pos) in powered_radars(now) {
                        st.saved.alarms.push(rules::Alarm { dimension, at: pos, reach: rules::SIREN_REACH, until: storm.from });
                    }
                }
                rules::Happening::StormBegan => {
                    for p in players.iter().filter(|p| p.dimension == OVERWORLD && playing(p)) {
                        let near = st.saved.zones.iter().any(|z| {
                            z.dimension == OVERWORLD
                                && (f64::from(z.center[0]) - p.position[0]).hypot(f64::from(z.center[2]) - p.position[2]) <= rules::PACK_NEAR
                        });
                        if near && st.saved.events.on(rules::EventKind::Mutants) {
                            work.packs.push((p.uuid, p.position));
                        }
                    }
                }
                rules::Happening::StormEnded => log::info!("[big_nukes] the radioactive storm is over"),
                rules::Happening::SatelliteSet(sat) => {
                    log::info!("[big_nukes] a satellite falls at {:?} at tick {}", sat.to, sat.at);
                    let chunk = rules::chunk_of(sat.to[0], sat.to[1]);
                    if st.saved.force(SATELLITE_TICKET, &sat.dimension, chunk) {
                        work.force.push(chunk);
                    }
                    for (dimension, pos) in powered_radars(now) {
                        st.saved.alarms.push(rules::Alarm { dimension, at: pos, reach: rules::SIREN_REACH, until: sat.at });
                    }
                }
                rules::Happening::SatelliteFell(sat) => work.fell.push(sat.clone()),
            }
        }
        if !happened.is_empty() {
            st.dirty = true;
        }
        // The satellite's trail down the sky (its ground is read in its level's tick, `satellite_ground`).
        if let Some(sat) = st.saved.events.satellite.as_ref() {
            let fall_from = sat.at.saturating_sub(rules::SATELLITE_FALL);
            if now >= fall_from && now < sat.at {
                let ground = sat.surface.unwrap_or(rules::SURFACE_GUESS);
                let [x, y, z] = rules::satellite_fall(sat.to, ground, now - fall_from, sat.seed);
                work.commands.push(format!("particle minecraft:campfire_signal_smoke {x:.1} {y:.1} {z:.1} 1 1 1 0.02 6 force"));
                work.commands.push(format!("particle minecraft:flame {x:.1} {y:.1} {z:.1} 0.6 0.6 0.6 0.05 12 force"));
                if now == fall_from {
                    work.commands.push(format!("playsound minecraft:item.trident.thunder ambient @a {x:.1} {:.1} {z:.1} 40 0.6", ground + 40));
                }
            }
        }
        // Once a second: the notices, and the storm's ash on those under the open sky.
        if now % rules::SECOND == 0 {
            let storm = st.saved.events.storm;
            let sat = st.saved.events.satellite.clone();
            let blowing = st.saved.events.storm_now(now);
            for p in players.iter().filter(|p| p.dimension == OVERWORLD) {
                let radar = radar_near(OVERWORLD, p.position, now);
                if let Some(n) = storm.as_ref().and_then(|s| rules::storm_notice(s, now, radar)) {
                    work.notices.push((p.uuid, n));
                }
                if let Some(n) = sat.as_ref().and_then(|s| rules::satellite_notice(s, now, radar)) {
                    work.notices.push((p.uuid, n));
                }
                if blowing && bunker::shelter_of(p.uuid) >= 1.0 {
                    let [x, y, z] = p.position;
                    work.commands.push(format!(
                        "particle minecraft:ash {x:.1} {:.1} {z:.1} 10 4 10 0 300 normal {}",
                        y + 5.0,
                        rules::uuid_text(p.uuid)
                    ));
                }
            }
        }
    }
    for chunk in work.force {
        if !core::force_chunk(host, OVERWORLD, chunk, true) {
            log::warn!("[big_nukes] couldn't hold the satellite's chunk {chunk:?}");
        }
    }
    for command in work.commands {
        core::run(host, &format!("execute in {OVERWORLD} run {command}"));
    }
    for (player, notice) in work.notices {
        silo::notify(host, player, &notice);
    }
    if !work.packs.is_empty() || !work.fell.is_empty() {
        let mut pending = lock(&PENDING);
        pending.packs.extend(work.packs);
        pending.fell.extend(work.fell);
    }
}

// ---- the level's tick: the ground ----------------------------------------------------------

/// The end of a level's tick (before the waves): the satellite's ground, its fall and the storm's packs waiting
/// for this level, then the mutants.
pub(crate) fn level_tick(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    satellite_ground(host, dimension);
    let (packs, fell) = {
        let mut pending = lock(&PENDING);
        let packs = if dimension == OVERWORLD { std::mem::take(&mut pending.packs) } else { Vec::new() };
        let (fell, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut pending.fell).into_iter().partition(|s| s.dimension == dimension);
        pending.fell = rest;
        (packs, fell)
    };
    for sat in fell {
        fall(host, &sat);
    }
    for (player, at) in packs {
        pack(host, player, at);
    }
    mutants(host, level, dimension);
}

/// The satellite's ground, read once its chunk is in (it is held from when the satellite is set).
fn satellite_ground(host: &dyn ServerHost, dimension: &str) {
    let Some(mut guard) = core::state(host) else { return };
    let Some(st) = guard.as_mut() else { return };
    let now = st.saved.tick;
    let Some(sat) = st.saved.events.satellite.as_mut() else { return };
    if sat.dimension != dimension || sat.surface.is_some() || now + rules::SATELLITE_WARNING < sat.at {
        return;
    }
    sat.surface = host
        .surface_heights(dimension, &[(sat.to[0], sat.to[1])], Heightmap::WorldSurface)
        .ok()
        .and_then(|h| h.first().copied().flatten());
    if sat.surface.is_some() {
        st.dirty = true;
    }
}

/// A satellite down on its ground: a small blast, a weak zone for a day, and its debris falling into the crater.
fn fall(host: &mut dyn ServerHost, sat: &rules::Satellite) {
    let ground = sat.surface.unwrap_or(rules::SURFACE_GUESS);
    let center = [sat.to[0], ground, sat.to[1]];
    match core::start_blast(host, &sat.dimension, center, rules::SATELLITE_RADIUS) {
        Ok(id) => log::info!("[big_nukes] the satellite fell at {center:?}: blast {id}"),
        Err(why) => log::info!("[big_nukes] the satellite fell at {center:?} without a blast: {why:?}"),
    }
    if let Some(mut guard) = core::state(host) {
        let st = guard.as_mut().expect("opened");
        let now = st.saved.tick;
        st.saved.zones.push(rules::satellite_zone(&sat.dimension, center, now));
        st.saved.landed(SATELLITE_TICKET, Some(center));
        core::save(st);
    }
    let Some(state) = host.block_key(rules::SATELLITE_DEBRIS).and_then(|key| host.block_state_default(key)) else { return };
    for k in 0..rules::debris_count(sat.seed) {
        let a = f64::from(k) * 2.1 + (sat.seed % 7) as f64;
        let pos = [f64::from(center[0]) + 0.5 + a.cos() * 2.5, f64::from(ground) + 24.0 + f64::from(k) * 2.0, f64::from(center[2]) + 0.5 + a.sin() * 2.5];
        let mut falling = Falling::default();
        falling.drop_item = true;
        falling.hurts = true;
        if !host.spawn_falling_block(&sat.dimension, state, pos, [0.0, -0.5, 0.0], falling) {
            log::warn!("[big_nukes] the satellite's debris {k} didn't fall");
        }
    }
}

/// The tag a pack's zombie wears while it is made a mutant.
const PACK_TAG: &str = "big_nukes_pack";

/// A storm's pack round `at`: mutant zombies coming out of the dust, 16 to 24 blocks off.
fn pack(host: &mut dyn ServerHost, player: u128, at: [f64; 3]) {
    let seed = (player as u64) ^ (player >> 64) as u64;
    let n = rules::pack_size(seed);
    for k in 0..n {
        let a = f64::from(k) * std::f64::consts::TAU / f64::from(n) + (seed % 360) as f64;
        let d = 16.0 + f64::from(k % 3) * 4.0;
        let (x, z) = ((at[0] + a.cos() * d).floor() as i32, (at[2] + a.sin() * d).floor() as i32);
        let Some(y) = host
            .surface_heights(OVERWORLD, &[(x, z)], Heightmap::MotionBlockingNoLeaves)
            .ok()
            .and_then(|h| h.first().copied().flatten())
        else {
            continue;
        };
        let prefix = format!("execute in {OVERWORLD} positioned {x} {y} {z} run ");
        core::run(host, &format!("{prefix}summon minecraft:zombie ~0.5 ~ ~0.5 {{Tags:[\"{PACK_TAG}\"]}}"));
        // A helmet, so the daylight doesn't burn the pack.
        core::run(host, &format!("{prefix}item replace entity @e[tag={PACK_TAG},limit=1,sort=nearest] armor.head with minecraft:leather_helmet"));
        for command in rules::mutate_commands("minecraft:zombie", &format!("@e[tag={PACK_TAG},limit=1,sort=nearest]")) {
            core::run(host, &format!("{prefix}{command}"));
        }
        core::run(host, &format!("{prefix}tag @e[tag={PACK_TAG}] remove {PACK_TAG}"));
    }
}

// ---- the mutants ---------------------------------------------------------------------------

/// At most this many mobs mutate in a dimension at a look.
const MUTATE_AT_ONCE: usize = 8;

/// Each mutant near a player, by the player: how many, for their dose (`mutants_near`).
static NEAR: Mutex<Vec<(u128, usize, u64)>> = Mutex::new(Vec::new());

/// A mob of the level as the mutants' rules read it: its kind and its maximum health (a mutant's is doubled, and a
/// quarter over: `rules::is_mutant`).
struct Mob {
    entity: EntityRef,
    pos: [f64; 3],
    kind: String,
    max_health: f64,
}

fn level_mobs(host: &dyn ServerHost, level: LevelRef) -> Vec<Mob> {
    rad::mobs(host, level)
        .into_iter()
        .filter_map(|(entity, pos)| {
            let kind = entity.kind(host)?.id(host);
            let max_health = f64::from(host.living(entity)?.max_health(host)?);
            Some(Mob { entity, pos, kind, max_health })
        })
        .collect()
}

/// Every `MUTATE_EVERY` the mobs near the level's players in a zone may mutate; once a second the mutants near
/// players glow and are counted for their radiation.
fn mutants(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    let (now, zones, cleans, on) = {
        let Some(guard) = core::state(host) else { return };
        let Some(st) = guard.as_ref() else { return };
        let now = st.saved.tick;
        if now % rules::SECOND != 0 {
            return;
        }
        // Counts more than a second old go, whatever this level's tick finds.
        lock(&NEAR).retain(|c| c.2 + rules::SECOND >= now);
        if !st.saved.zones.iter().any(|z| z.dimension == dimension) {
            return;
        }
        let zones: Vec<rules::Zone> = st.saved.zones.iter().filter(|z| z.dimension == dimension).cloned().collect();
        (now, zones, st.saved.cleans.clone(), st.saved.events.started && st.saved.events.on(rules::EventKind::Mutants))
    };
    if !on {
        return;
    }
    let players: Vec<Pl> = core::players(host).into_iter().filter(|p| p.dimension == dimension && playing(p)).collect();
    if players.is_empty() {
        return;
    }
    let near = |pos: [f64; 3], reach: f64| players.iter().filter(|p| (0..3).map(|i| (p.position[i] - pos[i]).powi(2)).sum::<f64>() <= reach * reach).count();
    let mobs = level_mobs(host, level);
    // The mutants: they glow, and are counted round each player.
    let mutants: Vec<[f64; 3]> = mobs.iter().filter(|m| rules::is_mutant(&m.kind, m.max_health)).map(|m| m.pos).collect();
    {
        let mut counted = lock(&NEAR);
        for p in &players {
            let n = mutants.iter().filter(|m| (0..3).map(|i| (p.position[i] - m[i]).powi(2)).sum::<f64>() <= rules::MUTANT_REACH.powi(2)).count();
            counted.retain(|c| c.0 != p.uuid);
            counted.push((p.uuid, n, now));
        }
    }
    for m in mutants.iter().filter(|m| near(**m, 32.0) > 0) {
        core::run(
            host,
            &format!(
                "execute in {dimension} run particle minecraft:happy_villager {:.1} {:.1} {:.1} 0.3 0.6 0.3 0 3 normal",
                m[0],
                m[1] + 1.0,
                m[2]
            ),
        );
    }
    if now % rules::MUTATE_EVERY != 0 {
        return;
    }
    let candidates: Vec<&Mob> = mobs.iter().filter(|m| rules::mutable(&m.kind, m.max_health) && near(m.pos, rules::MUTATE_NEAR) > 0).collect();
    let turning: Vec<(EntityRef, String)> = candidates
        .iter()
        .filter(|m| rules::mutates(m.entity.id(), now, rules::background(&zones, &cleans, dimension, m.pos, now)))
        .take(MUTATE_AT_ONCE)
        .map(|m| (m.entity, m.kind.clone()))
        .collect();
    if !turning.is_empty() {
        log::info!("[big_nukes] {} of {} mobs near players in a zone of {dimension} mutate", turning.len(), candidates.len());
    }
    // Its attributes, full health and loot by commands, as before: the mark (`rules::is_mutant`) is its maximum
    // health, which the command sets as the attribute's base.
    for (entity, kind) in turning {
        let Some(uuid) = entity.uuid(host) else { continue };
        for command in rules::mutate_commands(&kind, &rules::uuid_text(uuid)) {
            core::run(host, &format!("execute in {dimension} run {command}"));
        }
    }
}

/// How many mutants were within reach of `player` at their last second, at most a second before `now`. Asked with
/// the state locked: never lock it here.
pub(crate) fn mutants_near(player: u128, now: u64) -> usize {
    lock(&NEAR).iter().find(|c| c.0 == player && c.2 + rules::SECOND >= now).map_or(0, |c| c.1)
}

/// The world closing: the radars heard, the mutants counted and what waits for a level go (called from
/// `bunker::forget`, as `world::close_world` has no call of its own for this module).
pub(crate) fn forget() {
    lock(&RADARS).clear();
    lock(&NEAR).clear();
    let mut pending = lock(&PENDING);
    pending.packs.clear();
    pending.fell.clear();
}
