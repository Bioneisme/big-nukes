//! The bunker (plan 10): its walls (registered by chunk, cracked and broken by the waves), the hermetic door, the
//! fuel generator, the air filter's sealed rooms and the lamp, and each player's shelter.
//!
//! The walls copy stone, their cracked blocks too: vanilla's explosions read their resistance (TNT and creepers
//! don't break them), the wave reads the registry. The door is an iron door: two halves, and redstone opens it; a
//! hand opens it here. The generator and the filter are furnaces, which open a menu and face the player who placed
//! them. The lamp is a copper bulb, whose light follows its `lit`.

use crate::core::{self, Pl, argb, lock};
use crate::{decon, rules};
use mod_api::{
    Block, BlockProperties, BlockUpdateFlags, ClientHost, DrawContext, HostBlockStates, LevelRef, LightLayer, MenuQuickMoveEvent,
    ModBlockEntityView, ModContainer, Screen, ServerHost, SoundSource, Tag,
};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

// ---- declarations -----------------------------------------------------------------

pub(crate) fn blocks(blocks: &mut Vec<Block>) {
    let block = |id: &str, template: &str, hardness: f32, resistance: f32, sounds: Option<&str>| {
        let mut block = Block::new(id, template).with_item(true);
        block.properties = BlockProperties {
            hardness: Some(hardness),
            explosion_resistance: Some(resistance),
            requires_tool: Some(true),
            sounds: sounds.map(str::to_owned),
            ..Default::default()
        };
        block
    };
    for kind in rules::WallKind::ALL {
        let (hardness, sounds) = match kind {
            rules::WallKind::Reinforced => (6.0, None),
            rules::WallKind::Armored => (12.0, None),
            rules::WallKind::Lead => (5.0, Some("minecraft:iron_block")),
            rules::WallKind::Damper => (2.5, Some("minecraft:white_wool")),
            rules::WallKind::Glass => (8.0, Some("minecraft:glass")),
            // Not among them: the door is declared below, as a door.
            rules::WallKind::Door => continue,
        };
        // Radiation glass is tinted glass: seen through, but it keeps the sky's light out, so a roof of it shelters.
        let template = if kind == rules::WallKind::Glass { "minecraft:tinted_glass" } else { "minecraft:stone" };
        let resistance = kind.explosion_resistance();
        blocks.push(block(kind.id(), template, hardness, resistance, sounds));
        blocks.push(block(kind.cracked_id(), template, hardness / 2.0, resistance / 2.0, sounds));
    }
    let door = rules::WallKind::Door.explosion_resistance();
    blocks.push(block(rules::HERMETIC_DOOR, "minecraft:iron_door", 10.0, door, None));
    blocks.push(block(rules::FUEL_GENERATOR, "minecraft:furnace", 3.5, 6.0, None));
    blocks.push(block(rules::AIR_FILTER, "minecraft:furnace", 3.5, 6.0, None));
    blocks.push(block(rules::BUNKER_LAMP, "minecraft:waxed_copper_bulb", 3.0, 6.0, None));
}

/// The bunker's blocks with a block entity: registered with the core's `Machine` part.
pub(crate) fn has_entity(block: &str) -> bool {
    matches!(block, rules::FUEL_GENERATOR | rules::AIR_FILTER | rules::BUNKER_LAMP)
}

/// A block's container (slots, faces, menu), read once at registration: the generator's fuel slot, as the
/// decontaminator's, from every face; the filter's one slot hidden and shut to hoppers. The lamp has none.
///
/// A hopper puts only fuel into the generator's slot (`hoppers.rs`). What doesn't burn there anyway (put in by
/// another way) goes down into the container under it (`decon::spent_out`), as the decontaminator's does, so it
/// can't stop the generator.
pub(crate) fn container(block: &str) -> Option<ModContainer> {
    match block {
        rules::FUEL_GENERATOR => Some(ModContainer {
            slots: rules::GENERATOR_SLOTS,
            faces: std::array::from_fn(|_| vec![rules::GENERATOR_FUEL_SLOT]),
            inputs: vec![rules::GENERATOR_FUEL_SLOT],
            outputs: Vec::new(),
            menu: Some(rules::FUEL_GENERATOR.to_owned()),
        }),
        rules::AIR_FILTER => Some(ModContainer {
            slots: rules::FILTER_SLOTS,
            faces: std::array::from_fn(|_| Vec::new()),
            inputs: Vec::new(),
            outputs: Vec::new(),
            menu: Some(rules::AIR_FILTER.to_owned()),
        }),
        _ => None,
    }
}

// ---- the walls (the core's waves call these) -------------------------------------------
//
// The world keeps its walls by chunk (`Saved.walls`), the hermetic door's two halves among them (`WallKind::Door`,
// holding as armored concrete). The level tells of every block changed, maybe while the state is locked: the walls
// placed and taken away wait here until it opens (`take_heard`, from `core::state`). The wave's own edits tell no
// one: its plan answers what it cracked and broke, and the core gives that back.

/// Walls placed (`Some`: the wall and whether it is cracked) and taken away (`None`), heard from the level.
static HEARD: Mutex<Vec<(String, [i32; 3], Option<(rules::WallKind, bool)>)>> = Mutex::new(Vec::new());

/// A block at `pos` changed from `was` to `now` (the mod's block ids, "" for any other block): a wall placed,
/// replaced or taken away is queued. Heard from `block_changed`, maybe while the state is locked: never lock it here.
pub(crate) fn block_changed(dimension: &str, pos: [i32; 3], was: &str, now: &str) {
    let wall = rules::WallKind::of_block(now);
    if was == now || (wall.is_none() && rules::WallKind::of_block(was).is_none()) {
        return;
    }
    lock(&HEARD).push((dimension.to_owned(), pos, wall));
}

/// The walls heard since the state last opened go into the registry (called from `core::state`). Whether it changed.
pub(crate) fn take_heard(saved: &mut rules::Saved) -> bool {
    let mut changed = false;
    for (dimension, pos, wall) in std::mem::take(&mut *lock(&HEARD)) {
        changed |= match wall {
            Some((kind, cracked)) => saved.place_wall(&dimension, pos, kind, cracked),
            None => saved.remove_wall(&dimension, pos),
        };
    }
    changed
}

/// The walls a chunk's plan is given (`rules::walls_near`), corrected from the world as it stands; the corrections
/// go into `fixes`. Called inside a wave's work, with the state locked: read blocks only.
///
/// The chunk is loaded, so its own walls are read from the world: one the registry has wrong (gone without the
/// level telling, or cracked) is planned as it is.
pub(crate) fn walls_for(
    host: &dyn ServerHost,
    level: LevelRef,
    walls: &BTreeMap<String, Vec<rules::Wall>>,
    dimension: &str,
    chunk: [i32; 2],
    fixes: &mut Vec<([i32; 3], Option<(rules::WallKind, bool)>)>,
) -> Vec<rules::Wall> {
    let mut near = rules::walls_near(walls, dimension, chunk);
    near.retain_mut(|w| {
        if rules::chunk_of(w.pos[0], w.pos[2]) != chunk {
            return true;
        }
        let block = core::block_at(host, level, w.pos);
        if block == w.block() {
            return true;
        }
        let now = rules::WallKind::of_block(&block);
        fixes.push((w.pos, now));
        match now {
            Some((kind, cracked)) => {
                (w.kind, w.cracked) = (kind, cracked);
                true
            }
            None => false,
        }
    });
    near
}

/// The corrections `walls_for` found, into the registry. Whether it changed.
pub(crate) fn fix_walls(saved: &mut rules::Saved, dimension: &str, fixes: Vec<([i32; 3], Option<(rules::WallKind, bool)>)>) -> bool {
    let mut changed = false;
    for (pos, wall) in fixes {
        changed |= match wall {
            Some((kind, cracked)) => saved.place_wall(dimension, pos, kind, cracked),
            None => saved.remove_wall(dimension, pos),
        };
    }
    changed
}

// ---- the machines -----------------------------------------------------------------
//
// The generator, the filter and the lamp have block entities that tick. A burning generator is heard each tick it
// burns, by place (`GENERATORS`); a filter and a lamp look once a second for one heard lately within
// `POWER_RADIUS`, so a generator gone out, broken or unloaded stops powering them within two seconds. The rooms the
// filters found sealed are kept by filter (`ROOMS`), their cells in a set: a player's shelter is a look-up.

/// The server's ticks, counted here: the machines' clock.
static TICK: AtomicU64 = AtomicU64::new(0);
/// The burning generators, by dimension and place: the tick each was last heard burning.
static GENERATORS: Mutex<Vec<(String, [i32; 3], u64)>> = Mutex::new(Vec::new());
/// Each filter's room, by dimension and the filter's place.
static ROOMS: Mutex<Vec<Room>> = Mutex::new(Vec::new());
/// The share of the background each player's shelter let in at their last second, for their window.
static SHELTER: Mutex<Vec<(u128, f64)>> = Mutex::new(Vec::new());
/// Each player's last click on a door (player, its lower half, tick), so one press turns it once.
static DOOR_PRESSES: Mutex<Vec<(u128, [i32; 3], u64)>> = Mutex::new(Vec::new());

/// How long a generator heard burning powers what is round it.
const POWER_LASTS: u64 = 2 * rules::SECOND;

struct Room {
    dimension: String,
    filter: [i32; 3],
    /// Its cells when sealed, none when not.
    cells: Option<HashSet<[i32; 3]>>,
    /// When the filter last had power.
    powered: u64,
}

fn now() -> u64 {
    TICK.load(Ordering::Relaxed)
}

/// The start of the core's server tick (the old `server_tick()`, the machines' clock): the clock moves on, and what
/// they heard long ago goes.
pub(crate) fn server_tick(_host: &mut dyn ServerHost) {
    let now = TICK.fetch_add(1, Ordering::Relaxed) + 1;
    if now % (10 * rules::SECOND) == 0 {
        lock(&GENERATORS).retain(|g| g.2 + POWER_LASTS >= now);
        lock(&ROOMS).retain(|r| r.powered + POWER_LASTS >= now);
    }
}

/// A bunker block entity's server tick. `host.dimension()` is its level's.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    let id = entity.id().to_owned();
    match id.as_str() {
        rules::FUEL_GENERATOR => generator_tick(host, pos, entity),
        rules::AIR_FILTER => filter_tick(host, pos, entity),
        rules::BUNKER_LAMP => lamp_tick(host, pos),
        _ => {}
    }
}

/// A shift-click in a bunker block's menu (the generator's goes as the decontaminator's: `decon::quick_move`).
/// Whether it was handled. Nothing goes into or out of the filter's hidden slot; the lamp has no menu.
pub(crate) fn quick_move(host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> bool {
    let id = event.entity.id().to_owned();
    match id.as_str() {
        rules::FUEL_GENERATOR => decon::quick_move(host, event),
        rules::AIR_FILTER => true,
        _ => false,
    }
}

/// Whether a generator burns near `pos`, heard lately: events.rs's radar asks it.
pub(crate) fn powered_now(dimension: &str, pos: [i32; 3]) -> bool {
    powered_at(dimension, pos, now())
}

/// Whether a generator heard burning lately powers `pos` in `dimension`.
fn powered_at(dimension: &str, pos: [i32; 3], now: u64) -> bool {
    let burning: Vec<[i32; 3]> = lock(&GENERATORS).iter().filter(|g| g.0 == dimension && g.2 + POWER_LASTS >= now).map(|g| g.1).collect();
    rules::powered(&burning, pos)
}

/// The block's `lit` set to `lit`, when it isn't already.
fn light(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], lit: bool) {
    set_property(host, level, pos, "lit", if lit { "true" } else { "false" });
}

/// A block's property set to `value` (its neighbours and the clients told), when it isn't already.
fn set_property(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], key: &str, value: &str) {
    if level.block_property(host, pos, key).as_deref() == Some(value) {
        return;
    }
    let Some(state) = level.block_state(host, pos) else { return };
    let Ok(changed) = state.with(&host.block_states(), key, value) else { return };
    let _ = level.set_block(host, pos, changed, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
}

const GENERATOR_KEY: &str = "generator";

/// A tick: the fuel burns (`Generator::step`), the block's front is lit while it does, and the generator is heard
/// burning.
fn generator_tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    if entity.items.len() < rules::GENERATOR_SLOTS {
        entity.items.resize(rules::GENERATOR_SLOTS, None);
    }
    let was = match entity.nbt.get(&GENERATOR_KEY.into()) {
        Some(Tag::ByteArray(bytes)) => serde_json::from_slice::<rules::Generator>(bytes).ok(),
        _ => None,
    };
    let mut generator = was.unwrap_or_default();
    let fuel = entity.items[rules::GENERATOR_FUEL_SLOT]
        .as_ref()
        .map(|s| core::burn_time(host, s))
        .filter(|t| *t > 0)
        .map(i32::unsigned_abs);
    let step = generator.step(fuel);
    let dimension = host.dimension();
    let level = host.level(dimension);
    if step.took_fuel {
        decon::take_fuel(host, &mut entity.items, rules::GENERATOR_FUEL_SLOT);
    }
    if fuel.is_none()
        && let Some(level) = level
    {
        decon::spent_out(host, level, pos, &mut entity.items, rules::GENERATOR_FUEL_SLOT);
    }
    if step.made {
        let now = now();
        let mut heard = lock(&GENERATORS);
        match heard.iter_mut().find(|g| g.0 == dimension && g.1 == pos) {
            Some(g) => g.2 = now,
            None => heard.push((dimension.to_owned(), pos, now)),
        }
    }
    if let Some(level) = level {
        light(host, level, pos, step.made);
    }
    let data = generator.data().to_vec();
    if entity.data != data {
        entity.data = data;
    }
    if was != Some(generator)
        && let Ok(bytes) = serde_json::to_vec(&generator)
    {
        entity.nbt.insert(GENERATOR_KEY.into(), Tag::ByteArray(bytes));
    }
}

/// Once a second: lit while a generator powers it.
fn lamp_tick(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let now = now();
    if now % rules::SECOND != 0 {
        return;
    }
    let dimension = host.dimension();
    let lit = powered_at(dimension, pos, now);
    if let Some(level) = host.level(dimension) {
        light(host, level, pos, lit);
    }
}

/// The sky's light at `pos`, the old glue's `native_light(pos).1`. None read (not loaded) counts as the open sky.
fn sky_light(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> u8 {
    level.brightness(host, LightLayer::Sky, pos).map_or(15, |l| l.clamp(0, 15) as u8)
}

/// A place as a filter's room sees it: air goes through, and so does a block that doesn't fill its place
/// (`rules::lets_air`: a pane, a fence, a torch...) and a door, a trapdoor or a gate standing open; such a place
/// under the open sky (`open_to_sky`) is the sky; anything else is a wall.
fn cell(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> rules::Cell {
    let block = core::block_at(host, level, pos);
    let through = if rules::lets_air(&block) {
        rules::Cell::Open
    } else {
        match level.block_property(host, pos, "open") {
            Some(open) => rules::door_cell(Some(&open)),
            None => rules::Cell::Wall,
        }
    };
    if through == rules::Cell::Open && rules::open_to_sky(sky_light(host, level, pos)) { rules::Cell::Sky } else { through }
}

/// Once a second: the filter has power or not; with power, every `FILTER_EVERY` at its own second
/// (`rules::filter_due`) it goes round its room from each face, and the sealed rooms it reaches are kept as one. A
/// face whose air an earlier face's flood went through is the same room, and isn't flooded again. Its screen shows
/// what it found.
fn filter_tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    let now = now();
    if now % rules::SECOND != 0 {
        return;
    }
    let dimension = host.dimension();
    let powered = powered_at(dimension, pos, now);
    let due = {
        let mut rooms = lock(&ROOMS);
        let i = match rooms.iter().position(|r| r.dimension == dimension && r.filter == pos) {
            Some(i) => i,
            None => {
                rooms.push(Room { dimension: dimension.to_owned(), filter: pos, cells: None, powered: 0 });
                rooms.len() - 1
            }
        };
        let room = &mut rooms[i];
        if powered {
            room.powered = now;
        } else {
            room.cells = None;
        }
        powered && rules::filter_due(pos, now)
    };
    if due && let Some(level) = host.level(dimension) {
        let host: &dyn ServerHost = host;
        let mut sealed: Option<HashSet<[i32; 3]>> = None;
        let mut open = HashSet::new();
        let [x, y, z] = pos;
        for start in [[x, y + 1, z], [x + 1, y, z], [x - 1, y, z], [x, y, z + 1], [x, y, z - 1], [x, y - 1, z]] {
            if open.contains(&start) || sealed.as_ref().is_some_and(|s| s.contains(&start)) {
                continue;
            }
            match rules::sealed_room(start, &|p| cell(host, level, p), rules::ROOM_LIMIT) {
                rules::Flood::Sealed(room) => sealed.get_or_insert_with(HashSet::new).extend(room),
                rules::Flood::Open(explored) => open.extend(explored),
            }
        }
        let mut rooms = lock(&ROOMS);
        if let Some(room) = rooms.iter_mut().find(|r| r.dimension == dimension && r.filter == pos) {
            room.cells = sealed;
        }
    }
    let size = lock(&ROOMS).iter().find(|r| r.dimension == dimension && r.filter == pos).and_then(|r| r.cells.as_ref().map(HashSet::len));
    let data = rules::filter_data(powered, size).to_vec();
    if entity.data != data {
        entity.data = data;
    }
}

// ---- the shelter ------------------------------------------------------------------

/// A player's eyes over their feet, standing.
const EYES: f64 = 1.62;

/// The share of the background where `who` stands (`rules::shelter`), kept for their window: 1 is none. The sky's
/// light at their eyes, and their feet or head in a powered filter's sealed room.
pub(crate) fn shelter(host: &dyn ServerHost, who: &Pl) -> f64 {
    let [x, y, z] = who.position;
    let feet = [x.floor() as i32, y.floor() as i32, z.floor() as i32];
    let head = [feet[0], (y + 1.0).floor() as i32, feet[2]];
    let eyes = [feet[0], (y + EYES).floor() as i32, feet[2]];
    let sky = host.level(&who.dimension).is_none_or(|level| rules::open_to_sky(sky_light(host, level, eyes)));
    let now = now();
    let sealed = lock(&ROOMS).iter().any(|r| {
        r.dimension == who.dimension
            && r.powered + POWER_LASTS >= now
            && r.cells.as_ref().is_some_and(|c| c.contains(&feet) || c.contains(&head))
    });
    let shelter = rules::shelter(sky, sealed);
    let mut kept = lock(&SHELTER);
    match kept.iter_mut().find(|s| s.0 == who.uuid) {
        Some(s) => s.1 = shelter,
        None => kept.push((who.uuid, shelter)),
    }
    shelter
}

/// The shelter `player` had at their last second: none (1) for a player not reckoned.
pub(crate) fn shelter_of(player: u128) -> f64 {
    lock(&SHELTER).iter().find(|s| s.0 == player).map_or(1.0, |s| s.1)
}

// ---- the hermetic door -------------------------------------------------------------

/// A right click on the hermetic door: whether it was the door. Set `canceled` to keep the game's own use.
///
/// A hand opens and shuts it, both halves, as a wooden door's would (the iron door it copies opens only by
/// redstone). Sneaking with something in hand places against it as usual.
pub(crate) fn right_click(host: &mut dyn ServerHost, who: &Pl, pos: [i32; 3], canceled: &mut bool) -> bool {
    let Some(level) = host.level(&who.dimension) else { return false };
    if core::block_at(host, level, pos) != rules::HERMETIC_DOOR {
        return false;
    }
    if core::held(host, who).is_some() && core::sneaking(host, who) {
        return true;
    }
    *canceled = true;
    let other = match level.block_property(host, pos, "half").as_deref() {
        Some("upper") => [pos[0], pos[1] - 1, pos[2]],
        _ => [pos[0], pos[1] + 1, pos[2]],
    };
    // Both halves are one door: its lower half names it.
    let door = if other[1] < pos[1] { other } else { pos };
    let pressed = rules::new_press(&mut lock(&DOOR_PRESSES), who.uuid, door, now());
    if !pressed {
        return true;
    }
    let open = level.block_property(host, pos, "open").as_deref() != Some("true");
    let value = if open { "true" } else { "false" };
    // The game's door brings its other half along; set it too, in case it doesn't.
    set_property(host, level, pos, "open", value);
    if core::block_at(host, level, other) == rules::HERMETIC_DOOR {
        set_property(host, level, other, "open", value);
    }
    let event = if open { "minecraft:block.iron_door.open" } else { "minecraft:block.iron_door.close" };
    core::play(host, &who.dimension, event, core::middle(pos), SoundSource::Blocks, 1.0, 0.9);
    true
}

// ---- the screens ------------------------------------------------------------------
//
// On the game's plain panel, as the launch console's: the generator's fuel slot and its flame (the
// decontaminator's sprites, in its screen's texture), the filter's power and room.

const FUEL_AT: [f32; 2] = [80.0, 53.0];
const FLAME_AT: [f32; 2] = [62.0, 54.0];
/// The decontaminator's screen's texture: its unlit flame at `FLAME_AT`, its lit one right of the panel.
const DECON_GUI: &str = "big_nukes:gui/decontaminator";
const LIT_FLAME: [f32; 2] = [176.0, 0.0];
const LABEL: u32 = 0x40_4040;
const GOOD: u32 = 0x2A_7A2A;
const BAD: u32 = 0x9A_2A1A;
const WIDTH: f32 = 176.0;

pub(crate) fn screen(block: &str) -> Option<Screen> {
    let slots = match block {
        rules::FUEL_GENERATOR => vec![Some(FUEL_AT)],
        rules::AIR_FILTER => vec![None],
        _ => return None,
    };
    Some(Screen {
        width: WIDTH,
        height: 166.0,
        slots,
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

/// A line centred on the panel, cut to fit it.
fn centred(draw: &mut dyn DrawContext, line: &str, origin: [f32; 2], row: f32, colour: u32) {
    let line = rules::fit(line, WIDTH - 16.0, &|t| draw.text_width(t));
    let width = draw.text_width(&line);
    draw.text(&line, [origin[0] + ((WIDTH - width) / 2.0).round(), origin[1] + row], argb(colour), false);
}

pub(crate) fn screen_draw(host: &dyn ClientHost, block: &str, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    let [x, y] = origin;
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    let radius = rules::POWER_RADIUS.to_string();
    match block {
        rules::FUEL_GENERATOR => {
            centred(draw, &core::lang(host, "big_nukes.screen.fuel_generator"), origin, 6.0, LABEL);
            let size = rules::FLAME_HEIGHT as f32;
            draw.sprite(DECON_GUI, [FLAME_AT[0], FLAME_AT[1], size, size], [x + FLAME_AT[0], y + FLAME_AT[1], size, size], None);
            let flame = rules::flame_height(get(0), get(1)) as f32;
            if flame > 0.0 {
                let skip = size - flame;
                draw.sprite(
                    DECON_GUI,
                    [LIT_FLAME[0], LIT_FLAME[1] + skip, size, flame],
                    [x + FLAME_AT[0], y + FLAME_AT[1] + skip, size, flame],
                    None,
                );
            }
            let (line, colour) = if get(0) > 0 {
                (core::lang_tr(host, "big_nukes.screen.fuel_generator.burning", &[&radius]), GOOD)
            } else {
                (core::lang(host, "big_nukes.screen.fuel_generator.idle"), BAD)
            };
            centred(draw, &line, origin, 24.0, colour);
        }
        rules::AIR_FILTER => {
            centred(draw, &core::lang(host, "big_nukes.screen.air_filter"), origin, 6.0, LABEL);
            let (power, colour) = if get(0) > 0 {
                (core::lang(host, "big_nukes.screen.air_filter.powered"), GOOD)
            } else {
                (core::lang_tr(host, "big_nukes.screen.air_filter.unpowered", &[&radius]), BAD)
            };
            centred(draw, &power, origin, 26.0, colour);
            if get(0) > 0 {
                let (room, colour) = if get(1) > 0 {
                    (core::lang_tr(host, "big_nukes.screen.air_filter.sealed", &[&get(2).to_string()]), GOOD)
                } else {
                    (core::lang_tr(host, "big_nukes.screen.air_filter.open", &[&rules::ROOM_LIMIT.to_string()]), BAD)
                };
                centred(draw, &room, origin, 42.0, colour);
            }
        }
        _ => return,
    }
    draw.text(&core::lang(host, "container.inventory"), [x + 8.0, y + 72.0], argb(LABEL), false);
}

/// The world closing: what waits for a tick goes, the events' too (`world::close_world` asks only this module and
/// the decontaminator's, and events.rs has no `forget` of its own there).
pub(crate) fn forget() {
    lock(&HEARD).clear();
    lock(&GENERATORS).clear();
    lock(&ROOMS).clear();
    lock(&SHELTER).clear();
    lock(&DOOR_PRESSES).clear();
    TICK.store(0, Ordering::Relaxed);
}
