//! Radiation on the server. The zones are rules (`rules::background`). A player's dose builds in their own tick,
//! where their armour is in place; the sickness is given at the end of the server's tick; the mobs are hurt at
//! the end of their level's tick, a few at a time.

use crate::core::{self, Pl, lock, run};
use crate::world::State;
use crate::{bunker, decon, events, keys, rules};
use mod_api::{
    ConsumeEffectEvent, EntityRef, EquipmentSlot, Host, LevelRef, PlayerRef, ServerHost, SoundSource,
};
use std::sync::Mutex;

/// Players who died, heard while the state may be locked (a blast's damage or the sickness kills them): their
/// doses go the next time it opens.
static DIED: Mutex<Vec<u128>> = Mutex::new(Vec::new());
/// Doses the anti-radiation drug took down, the player and the rad, taken the same way.
static ANTI_RAD: Mutex<Vec<(u128, f64)>> = Mutex::new(Vec::new());

/// The deaths and the drinks heard since the state last opened. Whether it changed.
pub(crate) fn take_heard(saved: &mut rules::Saved) -> bool {
    let mut changed = false;
    for uuid in std::mem::take(&mut *lock(&DIED)) {
        changed |= saved.died(uuid);
    }
    for (uuid, amount) in std::mem::take(&mut *lock(&ANTI_RAD)) {
        changed |= saved.take_dose(uuid, amount);
    }
    changed
}

pub(crate) fn forget() {
    lock(&DIED).clear();
    lock(&ANTI_RAD).clear();
}

/// A player died: their dose goes. Heard in a queue, as the state may be locked.
pub(crate) fn player_died(player: u128) {
    lock(&DIED).push(player);
}

/// The anti-radiation drug: `amount` rad off the player's dose, the next time the state opens.
pub(crate) fn anti_radiation(uuid: u128, amount: f64) {
    lock(&ANTI_RAD).push((uuid, amount));
}

/// A consume effect of the mod's own kind: a drink of anti-rad takes its amount off the drinker's dose; an iodine
/// tablet's is the decontaminator's.
pub(crate) fn consume_effect(host: &mut dyn ServerHost, event: &ConsumeEffectEvent<'_>) {
    if decon::consume_effect(host, event.kind) {
        return;
    }
    if event.kind != rules::ANTI_RADIATION {
        return;
    }
    let Some(amount) = rules::anti_rad_amount(event.data) else {
        log::warn!("[big_nukes] {} without a positive amount: {:?}", event.kind, event.data);
        return;
    };
    let Some(who) = host.players().acting_player() else {
        log::warn!("[big_nukes] {}: no drinker", event.kind);
        return;
    };
    anti_radiation(who.uuid(), amount);
}

/// The armour slots, head first, as `rules::HAZMAT` lists the suit, and their inventory slots (39 head ... 36 feet).
const WORN: [(EquipmentSlot, u32); 4] =
    [(EquipmentSlot::Head, 39), (EquipmentSlot::Chest, 38), (EquipmentSlot::Legs, 37), (EquipmentSlot::Feet, 36)];

/// What the player carries gives off its own radiation (uranium, a Sunbreaker's core, trinitite): their inventory,
/// read whole. The old glue summed it as the game ticked each stack; the API reads the inventory at once.
fn carried(host: &dyn ServerHost, player: PlayerRef) -> f64 {
    let Some(slots) = player.inventory_slots(host) else { return 0.0 };
    let stacks: Vec<(&str, u32)> = slots
        .iter()
        .filter_map(|(_, s)| keys().carried.iter().find(|c| c.0 == s.item()).map(|c| (c.1, s.count())))
        .collect();
    rules::carried_rad(&stacks)
}

/// A player's tick: once a second, the dose takes the background where they stand, less what their hazmat suit
/// and the iodine's effect keep out, and each worn piece wears while there is any to speak of. Creative players and
/// spectators take none.
pub(crate) fn player_tick(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(who) = core::pl(host, player) else { return };
    // Read each tick, so a creative player's iodine doesn't linger.
    let iodine = decon::iodine_on(host, &who);
    if who.exempt() {
        // No dose for them, but the fallout's ash falls round them too.
        let ash = state_ash(host, &who);
        if ash {
            fall_ash(host, &who);
        }
        return;
    }
    let living = host.living(player.entity());
    let worn: [Option<mod_api::ItemStack>; 4] = std::array::from_fn(|i| living.and_then(|l| l.equipment(host, WORN[i].0)));
    let suit: [bool; 4] = std::array::from_fn(|i| worn[i].as_ref().is_some_and(|s| s.item() == keys().hazmat[i]));
    // A roof halves the background, a powered filter's sealed room keeps out most of it.
    let shelter = bunker::shelter(host, &who);
    let carried = carried(host, player);
    let Some(mut guard) = core::state(host) else { return };
    let st = guard.as_mut().expect("opened");
    let had = st.saved.doses.contains_key(&rules::dose_key(who.uuid));
    let protection = rules::shielded(rules::protection(suit[0], suit[1], suit[2], suit[3]), iodine);
    // A storm blowing over the overworld, and the mutants beside the player, add their own.
    let storm = if who.dimension == "minecraft:overworld" && st.saved.events.storm_now(st.saved.tick) {
        rules::storm_background(shelter)
    } else {
        0.0
    };
    let carried = carried + storm + rules::mutants_radiation(events::mutants_near(who.uuid, st.saved.tick));
    let Some(background) = st.saved.expose_carrying(who.uuid, &who.dimension, who.position, protection, shelter, carried) else {
        return;
    };
    st.dirty |= had || background > 0.0;
    // In a fresh plume the fallout's ash comes down round the player, once a second.
    let ash = st.saved.fallout_at(&who.dimension, who.position);
    drop(guard);
    if background >= rules::WEARS_FROM {
        for (i, on) in suit.into_iter().enumerate() {
            if on && let Some(stack) = worn[i].clone() {
                wear_piece(host, &who, i, stack);
            }
        }
    }
    if ash {
        fall_ash(host, &who);
    }
}

/// Whether ash falls round `who` this second (no state, no ash).
fn state_ash(host: &dyn ServerHost, who: &Pl) -> bool {
    core::state(host).is_some_and(|g| {
        g.as_ref().is_some_and(|st| st.saved.tick % rules::SECOND == 0 && st.saved.fallout_at(&who.dimension, who.position))
    })
}

/// A worn piece of the suit loses a point of its durability, or breaks on its last. Written straight back: the old
/// glue queued it for the piece's own inventory tick.
fn wear_piece(host: &mut dyn ServerHost, who: &Pl, piece: usize, mut stack: mod_api::ItemStack) {
    // The piece's own durability when the game knows it.
    let max = host.max_damage(stack.item()).filter(|m| *m > 0).unwrap_or(rules::HAZMAT_DURABILITY);
    let slot = WORN[piece].1;
    match rules::wear(stack.damage(), max) {
        Some(damage) => {
            stack.set_damage(damage);
            let _ = who.player.set_inventory_slot(host, slot, Some(stack));
        }
        None => {
            let _ = who.player.set_inventory_slot(host, slot, None);
            core::play(host, &who.dimension, "minecraft:entity.item.break", who.position, SoundSource::Players, 0.8, 1.0);
        }
    }
}

/// The fallout's ash round `who`, for them to see: a command, as only they see it (`normal <player>`).
fn fall_ash(host: &mut dyn ServerHost, who: &Pl) {
    let [x, y, z] = who.position;
    run(
        host,
        &format!(
            "execute in {} run particle minecraft:ash {x:.1} {:.1} {z:.1} 8 3 8 0 160 normal {}",
            who.dimension,
            y + 5.0,
            rules::uuid_text(who.uuid)
        ),
    );
}

/// The sick players on the server, once a second: who, their stage and the damage due now. Creative players,
/// spectators and the dead are left alone.
pub(crate) fn sick(host: &dyn ServerHost, st: &mut State) -> Vec<(Pl, u8, Option<u32>)> {
    if st.saved.doses.is_empty() {
        return Vec::new();
    }
    let mut sick = Vec::new();
    for p in core::players(host) {
        if p.exempt() || p.health <= 0.0 {
            continue;
        }
        if let Some((stage, damage)) = st.saved.sicken(p.uuid) {
            st.dirty |= damage.is_some();
            sick.push((p, stage, damage));
        }
    }
    sick
}

/// The radiation each player's window is told of, once a second: the zones of their dimension and their dose, for
/// the Geiger counter and the fog. Only to the players with something to tell, and once more to those who had and
/// now have nothing, so their window lets go.
pub(crate) fn rad_news(host: &dyn ServerHost, st: &mut State) -> Vec<(u128, Vec<u8>)> {
    if st.saved.zones.is_empty() && st.saved.doses.is_empty() && st.told.is_empty() && !st.saved.winter() && st.saved.events.storm.is_none() {
        return Vec::new();
    }
    let mut news = Vec::new();
    for p in core::players(host) {
        let mut rad = st.saved.rad_for(p.uuid, &p.dimension, p.position);
        rad.shelter = bunker::shelter_of(p.uuid);
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

/// A sick player's effects, kept on for a few seconds at a time, and the damage due. The effects stay commands
/// (`effect give`, hidden particles), as before.
pub(crate) fn sicken(host: &mut dyn ServerHost, who: &Pl, stage: u8, damage: Option<u32>) {
    let name = &who.name;
    run(host, &format!("effect give {name} minecraft:weakness 3 0 true"));
    run(host, &format!("effect give {name} minecraft:hunger 3 0 true"));
    if stage >= 2 {
        run(host, &format!("effect give {name} minecraft:nausea 5 0 true"));
    }
    run(host, &format!("effect give {name} {} 3 {} true", rules::RADIATION, stage - 1));
    if let Some(amount) = damage {
        hurt(host, who.player.entity(), amount as f32);
    }
}

/// Radiation damage to `entity`: the mod's own damage type, or the world border's when the game refuses it (the
/// old `/damage` fallback).
pub(crate) fn hurt(host: &mut dyn ServerHost, entity: EntityRef, amount: f32) {
    if entity.hurt(host, rules::RADIATION, amount, None, None) != Some(true) {
        let _ = entity.hurt(host, "minecraft:outside_border", amount, None, None);
    }
}

/// The mobs of a level and where they stand: every living entity but the players, as the old `native_mobs`.
pub(crate) fn mobs(host: &dyn ServerHost, level: LevelRef) -> Vec<(EntityRef, [f64; 3])> {
    level
        .all_entities(host)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| host.mob(*e).is_some())
        .filter_map(|e| Some((e, e.position(host)?)))
        .collect()
}

/// The most mobs hurt in a dimension in a second; more take turns.
const MOBS_PER_PASS: usize = 64;

/// The mobs of `dimension` in its zones, once a second: each takes damage by the background where it stands, up to
/// `MOBS_PER_PASS` of them, the next ones on the next pass.
pub(crate) fn irradiate_mobs(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    let hurt_now: Vec<(EntityRef, u32)> = {
        let Some(mut guard) = core::state(host) else { return };
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
        let (zones, cleans) = (&st.saved.zones, &st.saved.cleans);
        let mut in_zone: Vec<(EntityRef, u32)> = mobs(host, level)
            .into_iter()
            .filter_map(|(e, pos)| rules::mob_harm(rules::background(zones, cleans, dimension, pos, now)).map(|n| (e, n)))
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
    for (entity, amount) in hurt_now {
        hurt(host, entity, amount as f32);
    }
}

/// A host's read of whether `effect` is on `who` (the iodine), for the decontaminator's module.
#[allow(dead_code)] // TODO(stage 2): decon::iodine_on may use it.
pub(crate) fn has_effect(host: &dyn Host, who: &Pl, effect: mod_api::EffectKey) -> bool {
    host.living(who.player.entity()).and_then(|l| l.effect(host, effect)).is_some()
}
