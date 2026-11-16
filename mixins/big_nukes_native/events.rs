// Big Nukes' events in the game (plan 13): the radar, the radioactive storm, the mutants and the falling satellite.
// Added to the game as `game::big_nukes_events`, beside the rest of the glue (`game::big_nukes_glue`, glue.rs), whose
// hooks call it. The rules (`Events`, `Saved::tick_events`, the mutants' and the satellite's) are runtime.rs'.

use crate::game::{
    Game, NativeFalling, NativeHeightmap, big_nukes as rules, big_nukes_bunker as bunker,
    big_nukes_glue::{self as glue, lock},
    big_nukes_silo as silo,
};
use horizon_client::text::Text;
use horizon_mods::content::Block;
use std::sync::Mutex;

const OVERWORLD: &str = "minecraft:overworld";
/// The chunk ticket a falling satellite holds on its target, as a flight's (`Saved::force`).
const SATELLITE_TICKET: u64 = u64::MAX - 7;

// ---- the blocks ------------------------------------------------------------------------

pub(in crate::game) fn blocks(blocks: &mut Vec<Block>, block: impl Fn(&str, &str, f32) -> Block) {
    // On the furnace, as the console: it faces its placer; its right click is the mod's, so no menu opens.
    blocks.push(block(rules::RADAR, "minecraft:furnace", 3.5));
    blocks.push(block(rules::SATELLITE_DEBRIS, "minecraft:iron_block", 4.0));
}

pub(in crate::game) fn has_entity(block: &str) -> bool {
    block == rules::RADAR
}

/// The radars heard this session: dimension, place, whether powered, the tick last heard. A radar works while its
/// chunk is loaded and a generator burns near it.
static RADARS: Mutex<Vec<(String, [i32; 3], bool, u64)>> = Mutex::new(Vec::new());
/// A radar heard within this many ticks still counts.
const RADAR_HEARD: u64 = 2 * rules::SECOND;

/// A radar's tick: once a second it says where it is and whether it has power.
pub(in crate::game) fn tick(game: &mut Game<'_>, pos: [i32; 3]) {
    let Some((now, _)) = glue::clock(game) else { return };
    if now % rules::SECOND != 0 {
        return;
    }
    let dimension = game.native_dimension().to_owned();
    let powered = bunker::powered_now(&dimension, pos);
    let mut radars = lock(&RADARS);
    radars.retain(|r| !(r.0 == dimension && r.1 == pos) && r.3 + RADAR_HEARD >= now);
    radars.push((dimension, pos, powered, now));
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

/// A right click on the radar: its report in chat. Whether the block was the radar.
pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) -> bool {
    if game.native_block(pos) != rules::RADAR {
        return false;
    }
    *canceled = true;
    let Some(who) = game.native_acting_player() else { return true };
    if !bunker::powered_now(&who.dimension, pos) {
        game.native_tell(who.uuid, Text::translate("big_nukes.radar.unpowered", &[]));
        return true;
    }
    let lines = {
        let Some(guard) = glue::state(game) else { return true };
        let st = guard.as_ref().expect("opened");
        rules::radar_lines(&st.saved.events, st.saved.flights.len(), st.saved.tick)
    };
    for line in lines {
        let args: Vec<Text> = line.args.iter().map(|a| Text::literal(a.clone())).collect();
        game.native_tell(who.uuid, Text::translate(&line.key, &args));
    }
    true
}

// ---- the command -----------------------------------------------------------------------

/// `/bignukes events ...` after its name.
pub(in crate::game) fn command(game: &mut Game<'_>, args: &[&str]) -> Text {
    let Some(mut guard) = glue::state(game) else {
        return Text::translate("big_nukes.events.usage", &[]);
    };
    let st = guard.as_mut().expect("opened");
    let line = match args {
        ["trigger", name] => match rules::EventKind::of_name(name) {
            Some(kind) => st.saved.trigger(kind),
            None => "big_nukes.events.usage",
        },
        _ => st.saved.events.command(args).unwrap_or("big_nukes.events.usage"),
    };
    glue::save(st);
    Text::translate(line, &[])
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

/// Each server tick: the events' clock, their notices once a second, the storm's ash, the satellite's trail and fall.
pub(in crate::game) fn server_tick(game: &mut Game<'_>) {
    let players = game.native_server_players();
    let mut work = Work::default();
    {
        let Some(mut guard) = glue::state(game) else { return };
        let st = guard.as_mut().expect("opened");
        if !st.saved.events.started {
            return;
        }
        let now = st.saved.tick;
        let overworld: Vec<[f64; 2]> =
            players.iter().filter(|p| p.dimension == OVERWORLD && p.game_mode != 3).map(|p| [p.position[0], p.position[2]]).collect();
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
                    for p in players.iter().filter(|p| p.dimension == OVERWORLD && p.game_mode != 3) {
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
        // The satellite's ground, read once its chunk is in; its trail down the sky.
        if let Some(sat) = st.saved.events.satellite.as_mut() {
            if sat.surface.is_none() && now + rules::SATELLITE_WARNING >= sat.at {
                sat.surface = game
                    .native_surface_heights(&sat.dimension, &[(sat.to[0], sat.to[1])], NativeHeightmap::WorldSurface)
                    .ok()
                    .and_then(|h| h.first().copied().flatten());
            }
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
        if !game.native_force_chunk_in(rules::TICKET_OWNER, OVERWORLD, chunk, true) {
            log::warn!("[big_nukes] couldn't hold the satellite's chunk {chunk:?}");
        }
    }
    for command in work.commands {
        game.native_run_command(&format!("execute in {OVERWORLD} run {command}"));
    }
    for (player, notice) in work.notices {
        silo::notify(game, player, &notice);
    }
    for (player, at) in work.packs {
        pack(game, player, at);
    }
    for sat in work.fell {
        fall(game, &sat);
    }
}

/// A satellite down on its ground: a small blast, a weak zone for a day, and its debris falling into the crater.
fn fall(game: &mut Game<'_>, sat: &rules::Satellite) {
    let ground = sat.surface.unwrap_or(rules::SURFACE_GUESS);
    let center = [sat.to[0], ground, sat.to[1]];
    match glue::start_blast(game, &sat.dimension, center, rules::SATELLITE_RADIUS) {
        Ok(id) => log::info!("[big_nukes] the satellite fell at {center:?}: blast {id}"),
        Err(why) => log::info!("[big_nukes] the satellite fell at {center:?} without a blast: {why:?}"),
    }
    if let Some(mut guard) = glue::state(game) {
        let st = guard.as_mut().expect("opened");
        let now = st.saved.tick;
        st.saved.zones.push(rules::satellite_zone(&sat.dimension, center, now));
        st.saved.landed(SATELLITE_TICKET, Some(center));
        glue::save(st);
    }
    let Some(state) = game.pack.assets.registry.default_state(rules::SATELLITE_DEBRIS) else { return };
    for k in 0..rules::debris_count(sat.seed) {
        let a = f64::from(k) * 2.1 + (sat.seed % 7) as f64;
        let pos = [f64::from(center[0]) + 0.5 + a.cos() * 2.5, f64::from(ground) + 24.0 + f64::from(k) * 2.0, f64::from(center[2]) + 0.5 + a.sin() * 2.5];
        game.native_spawn_falling_block(&sat.dimension, state, pos, [0.0, -0.5, 0.0], NativeFalling { drop_item: true, hurts: true });
    }
}

/// A storm's pack round `at`: mutant zombies coming out of the dust, 16 to 24 blocks off.
fn pack(game: &mut Game<'_>, player: u128, at: [f64; 3]) {
    let seed = (player as u64) ^ (player >> 64) as u64;
    let n = rules::pack_size(seed);
    for k in 0..n {
        let a = f64::from(k) * std::f64::consts::TAU / f64::from(n) + (seed % 360) as f64;
        let d = 16.0 + f64::from(k % 3) * 4.0;
        let (x, z) = ((at[0] + a.cos() * d).floor() as i32, (at[2] + a.sin() * d).floor() as i32);
        let Some(y) = game
            .native_surface_heights(OVERWORLD, &[(x, z)], NativeHeightmap::MotionBlockingNoLeaves)
            .ok()
            .and_then(|h| h.first().copied().flatten())
        else {
            continue;
        };
        let prefix = format!("execute in {OVERWORLD} positioned {x} {y} {z} run ");
        game.native_run_command(&format!("{prefix}summon minecraft:zombie ~0.5 ~ ~0.5 {{Tags:[\"{PACK_TAG}\"]}}"));
        // A helmet, so the daylight doesn't burn the pack.
        game.native_run_command(&format!("{prefix}item replace entity @e[tag={PACK_TAG},limit=1,sort=nearest] armor.head with minecraft:leather_helmet"));
        for command in rules::mutate_commands("minecraft:zombie", &format!("@e[tag={PACK_TAG},limit=1,sort=nearest]")) {
            game.native_run_command(&format!("{prefix}{command}"));
        }
        game.native_run_command(&format!("{prefix}tag @e[tag={PACK_TAG}] remove {PACK_TAG}"));
    }
}

/// The tag a pack's zombie wears while it is made a mutant.
const PACK_TAG: &str = "big_nukes_pack";

// ---- the mutants ---------------------------------------------------------------------------

/// At most this many mobs mutate in a dimension at a look.
const MUTATE_AT_ONCE: usize = 8;

/// Each mutant near a player, by the player: how many, for their dose (`mutants_near`).
static NEAR: Mutex<Vec<(u128, usize, u64)>> = Mutex::new(Vec::new());

/// A level's tick: every `MUTATE_EVERY` the mobs near its players in a zone may mutate; once a second the mutants
/// near players glow and are counted for their radiation.
pub(in crate::game) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    let (now, zones, cleans, on) = {
        let Some(guard) = glue::state(game) else { return };
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
    let players: Vec<_> = game.native_server_players().into_iter().filter(|p| p.dimension == dimension && p.game_mode != 3).collect();
    if players.is_empty() {
        return;
    }
    let near = |pos: [f64; 3], reach: f64| players.iter().filter(|p| (0..3).map(|i| (p.position[i] - pos[i]).powi(2)).sum::<f64>() <= reach * reach).count();
    let mobs = game.native_mobs(dimension);
    // The mutants: they glow, and are counted round each player.
    let mutants: Vec<[f64; 3]> = mobs.iter().filter(|m| rules::is_mutant(&m.kind, f64::from(m.max_health))).map(|m| m.pos).collect();
    {
        let mut counted = lock(&NEAR);
        for p in &players {
            let n = mutants.iter().filter(|m| (0..3).map(|i| (p.position[i] - m[i]).powi(2)).sum::<f64>() <= rules::MUTANT_REACH.powi(2)).count();
            counted.retain(|c| c.0 != p.uuid);
            counted.push((p.uuid, n, now));
        }
    }
    for m in mutants.iter().filter(|m| near(**m, 32.0) > 0) {
        game.native_run_command(&format!(
            "execute in {dimension} run particle minecraft:happy_villager {:.1} {:.1} {:.1} 0.3 0.6 0.3 0 3 normal",
            m[0],
            m[1] + 1.0,
            m[2]
        ));
    }
    if now % rules::MUTATE_EVERY != 0 {
        return;
    }
    let turning: Vec<(u32, String)> = mobs
        .iter()
        .filter(|m| rules::mutable(&m.kind, f64::from(m.max_health)) && near(m.pos, rules::MUTATE_NEAR) > 0)
        .filter(|m| rules::mutates(m.id, now, rules::background(&zones, &cleans, dimension, m.pos, now)))
        .take(MUTATE_AT_ONCE)
        .map(|m| (m.id, m.kind.to_string()))
        .collect();
    for (id, kind) in turning {
        let Some(uuid) = game.native_entity(id).and_then(|e| game.native_entity_uuid(e)) else { continue };
        for command in rules::mutate_commands(&kind, &rules::uuid_text(uuid)) {
            game.native_run_command(&format!("execute in {dimension} run {command}"));
        }
    }
}

/// How many mutants were within reach of `player` at their last second, as counted at most a second before `now`.
pub(in crate::game) fn mutants_near(player: u128, now: u64) -> usize {
    lock(&NEAR).iter().find(|c| c.0 == player && c.2 + rules::SECOND >= now).map_or(0, |c| c.1)
}
