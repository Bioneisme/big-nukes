// Big Nukes' bunker in the game (plan 10): its walls, the hermetic door, the fuel
// generator, the air filter and the lamps. Added to the game as `game::big_nukes_bunker`,
// next to the rest of the glue (`game::big_nukes_glue`, glue.rs), whose shared hooks hand
// the blocks here by their ids.

use crate::game::{
    Game, big_nukes as rules,
    big_nukes_decon as decon,
    big_nukes_glue::{self as glue, lock},
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
    lang,
    mod_api::{EquipmentSlot, NativeScreen, NativeServerPlayer},
};
use horizon_mods::content::{Block, BlockProperties};
use horizon_world::nbt::{JavaString, Tag};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

// ---- the blocks ------------------------------------------------------------------
//
// The walls copy stone, their cracked blocks too: vanilla's explosions read their
// resistance (TNT and creepers don't break them), the wave reads the registry. The door
// is an iron door: two halves, and redstone opens it; a hand opens it here. The generator
// and the filter are furnaces, which open a menu and face the player who placed them. The
// lamp is a copper bulb, whose light follows its `lit`.

pub(in crate::game) fn blocks(blocks: &mut Vec<Block>) {
    let block = |id: &str, template: &str, hardness: f32, resistance: f32, sounds: Option<&str>| Block {
        id: id.into(),
        template: template.into(),
        properties: BlockProperties {
            hardness: Some(hardness),
            explosion_resistance: Some(resistance),
            requires_tool: Some(true),
            sounds: sounds.map(str::to_owned),
            ..Default::default()
        },
        item: true,
        ..Default::default()
    };
    for kind in rules::WallKind::ALL {
        let (hardness, sounds) = match kind {
            rules::WallKind::Reinforced => (6.0, None),
            rules::WallKind::Armored => (12.0, None),
            rules::WallKind::Lead => (5.0, Some("minecraft:iron_block")),
            rules::WallKind::Damper => (2.5, Some("minecraft:white_wool")),
            // Not among them: the door is declared below, as a door.
            rules::WallKind::Door => continue,
        };
        let resistance = kind.explosion_resistance();
        blocks.push(block(kind.id(), "minecraft:stone", hardness, resistance, sounds));
        blocks.push(block(kind.cracked_id(), "minecraft:stone", hardness / 2.0, resistance / 2.0, sounds));
    }
    let door = rules::WallKind::Door.explosion_resistance();
    blocks.push(block(rules::HERMETIC_DOOR, "minecraft:iron_door", 10.0, door, None));
    blocks.push(block(rules::FUEL_GENERATOR, "minecraft:furnace", 3.5, 6.0, None));
    blocks.push(block(rules::AIR_FILTER, "minecraft:furnace", 3.5, 6.0, None));
    blocks.push(block(rules::BUNKER_LAMP, "minecraft:waxed_copper_bulb", 3.0, 6.0, None));
}

// ---- the walls' registry -------------------------------------------------------------
//
// The world keeps its walls by chunk (`Saved.walls`), the hermetic door's two halves among them
// (`WallKind::Door`, holding as armored concrete). The level tells of every block set
// through `Level.setBlock`, maybe while the state is locked: the walls placed and taken
// away wait here until it opens (`take_heard`, from `glue::state`). The wave's own edits tell
// no one: its plan answers what it cracked and broke, and the glue gives that back.

/// Walls placed (`Some`: the wall and whether it is cracked) and taken away (`None`), heard from the level.
static HEARD: Mutex<Vec<(String, [i32; 3], Option<(rules::WallKind, bool)>)>> = Mutex::new(Vec::new());

/// A block at `pos` changed from `was` to `now` (block ids): a wall placed, replaced or taken away is queued.
pub(in crate::game) fn block_changed(dimension: &str, pos: [i32; 3], was: &str, now: &str) {
    let wall = rules::WallKind::of_block(now);
    if was == now || (wall.is_none() && rules::WallKind::of_block(was).is_none()) {
        return;
    }
    lock(&HEARD).push((dimension.to_owned(), pos, wall));
}

/// The walls heard since the state last opened go into the registry. Whether it changed.
pub(in crate::game) fn take_heard(saved: &mut rules::Saved) -> bool {
    let mut changed = false;
    for (dimension, pos, wall) in std::mem::take(&mut *lock(&HEARD)) {
        changed |= match wall {
            Some((kind, cracked)) => saved.place_wall(&dimension, pos, kind, cracked),
            None => saved.remove_wall(&dimension, pos),
        };
    }
    changed
}

/// The walls a chunk's plan is given (`rules::walls_near`). The chunk is loaded, so its own are
/// read from the world as they stand: one the registry has wrong (gone without the level telling,
/// or cracked) is planned as it is, and its correction goes into `fixes` for the registry.
pub(in crate::game) fn walls_for(
    game: &Game<'_>,
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
        let block = game.native_block(w.pos);
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
pub(in crate::game) fn fix_walls(saved: &mut rules::Saved, dimension: &str, fixes: Vec<([i32; 3], Option<(rules::WallKind, bool)>)>) -> bool {
    let mut changed = false;
    for (pos, wall) in fixes {
        changed |= match wall {
            Some((kind, cracked)) => saved.place_wall(dimension, pos, kind, cracked),
            None => saved.remove_wall(dimension, pos),
        };
    }
    changed
}

// ---- the machines ------------------------------------------------------------------
//
// The generator, the filter and the lamp have block entities that tick. A burning generator
// is heard each tick it burns, by place (`GENERATORS`); a filter and a lamp look once a second
// for one heard lately within `POWER_RADIUS`, so a generator gone out, broken or unloaded
// stops powering them within two seconds. The rooms the filters found sealed are kept by
// filter (`ROOMS`), their cells in a set: a player's shelter is a look-up.

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
/// The sound category of blocks.
const BLOCKS: usize = 4;

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

/// The end of the server's tick: the machines' clock moves on, and what they heard long ago goes.
pub(in crate::game) fn server_tick() {
    let now = TICK.fetch_add(1, Ordering::Relaxed) + 1;
    if now % (10 * rules::SECOND) == 0 {
        lock(&GENERATORS).retain(|g| g.2 + POWER_LASTS >= now);
        lock(&ROOMS).retain(|r| r.powered + POWER_LASTS >= now);
    }
}

pub(in crate::game) fn has_entity(block: &str) -> bool {
    matches!(block, rules::FUEL_GENERATOR | rules::AIR_FILTER | rules::BUNKER_LAMP)
}

/// The generator's fuel slot, as the decontaminator's; the filter's one slot hidden and shut to hoppers. The
/// lamp has none.
pub(in crate::game) fn container(block: &str) -> Option<ModContainer> {
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

/// What a hopper may put in: fuel into the generator. Whether the block is the bunker's.
pub(in crate::game) fn accepts(game: &Game<'_>, block: &str, slot: usize, stack: &ItemStack, cancel: &mut bool) -> bool {
    match block {
        rules::FUEL_GENERATOR => *cancel = slot != rules::GENERATOR_FUEL_SLOT || glue::burn_time(game, stack) <= 0,
        rules::AIR_FILTER => *cancel = true,
        _ => return false,
    }
    true
}

/// A shift-click: fuel from the inventory into the generator, as into the decontaminator (one fuel slot,
/// the first); nothing into the filter's hidden slot. Whether the block is the bunker's.
pub(in crate::game) fn quick_move(game: &Game<'_>, entity: &mut ModBlockEntity, container_slots: usize, slot: usize, stack: &mut ItemStack, handled: &mut bool) -> bool {
    match entity.id.as_str() {
        rules::FUEL_GENERATOR => decon::quick_move(game, entity, container_slots, slot, stack, handled),
        rules::AIR_FILTER => *handled = true,
        _ => return false,
    }
    true
}

pub(in crate::game) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    match entity.id.as_str() {
        rules::FUEL_GENERATOR => generator_tick(game, pos, entity),
        rules::AIR_FILTER => filter_tick(game, pos, entity),
        rules::BUNKER_LAMP => lamp_tick(game, pos),
        _ => {}
    }
}

/// Whether a generator heard burning lately powers `pos` in `dimension`.
fn powered_at(dimension: &str, pos: [i32; 3], now: u64) -> bool {
    let burning: Vec<[i32; 3]> = lock(&GENERATORS).iter().filter(|g| g.0 == dimension && g.2 + POWER_LASTS >= now).map(|g| g.1).collect();
    rules::powered(&burning, pos)
}

/// The block's `lit` set to `lit`, when it isn't already.
fn light(game: &mut Game<'_>, pos: [i32; 3], lit: bool) {
    let value = if lit { "true" } else { "false" };
    if game.native_block_property(pos, "lit").as_deref() != Some(value) {
        game.native_set_block_property(pos, "lit", value);
    }
}

fn generator_key() -> JavaString {
    JavaString::from("generator")
}

/// A tick: the fuel burns (`Generator::step`), the block's front is lit while it does, and the
/// generator is heard burning.
fn generator_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.items.len() < rules::GENERATOR_SLOTS {
        entity.items.resize(rules::GENERATOR_SLOTS, None);
    }
    let was = match entity.nbt.get(&generator_key()) {
        Some(Tag::ByteArray(bytes)) => serde_json::from_slice::<rules::Generator>(bytes).ok(),
        _ => None,
    };
    let mut generator = was.unwrap_or_default();
    let fuel = entity.items[rules::GENERATOR_FUEL_SLOT]
        .as_ref()
        .map(|s| glue::burn_time(game, s))
        .filter(|t| *t > 0)
        .map(i32::unsigned_abs);
    let step = generator.step(fuel);
    if step.took_fuel
        && let Some(stack) = entity.items[rules::GENERATOR_FUEL_SLOT].as_mut()
    {
        stack.count -= 1;
        if stack.count == 0 {
            let left = rules::remainder(&stack.item);
            entity.items[rules::GENERATOR_FUEL_SLOT] = left.map(|item| ItemStack { item: item.into(), count: 1, damage: 0, extra: None });
        }
    }
    if step.made {
        let (dimension, now) = (game.native_dimension().to_owned(), now());
        let mut heard = lock(&GENERATORS);
        match heard.iter_mut().find(|g| g.0 == dimension && g.1 == pos) {
            Some(g) => g.2 = now,
            None => heard.push((dimension, pos, now)),
        }
    }
    light(game, pos, step.made);
    let data = generator.data().to_vec();
    if entity.data != data {
        entity.data = data;
    }
    if was != Some(generator)
        && let Ok(bytes) = serde_json::to_vec(&generator)
    {
        entity.nbt.insert(generator_key(), Tag::ByteArray(bytes));
    }
}

/// Once a second: lit while a generator powers it.
fn lamp_tick(game: &mut Game<'_>, pos: [i32; 3]) {
    let now = now();
    if now % rules::SECOND != 0 {
        return;
    }
    let lit = powered_at(game.native_dimension(), pos, now);
    light(game, pos, lit);
}

/// A place as a filter's room sees it: air goes through, and so does a block that doesn't fill its place
/// (`rules::lets_air`: a pane, a fence, a torch...) and a door, a trapdoor or a gate standing open; such a place
/// under the open sky (`open_to_sky`) is the sky; anything else is a wall.
fn cell(game: &Game<'_>, pos: [i32; 3]) -> rules::Cell {
    let block = game.native_block(pos);
    let through = if rules::lets_air(&block) {
        rules::Cell::Open
    } else {
        match game.native_block_property(pos, "open") {
            Some(open) => rules::door_cell(Some(&open)),
            None => rules::Cell::Wall,
        }
    };
    if through == rules::Cell::Open && rules::open_to_sky(game.native_light(pos).1) { rules::Cell::Sky } else { through }
}

/// Once a second: the filter has power or not; with power, every `FILTER_EVERY` at its own second
/// (`rules::filter_due`) it goes round its room from each face, and the sealed rooms it reaches are kept as
/// one. A face whose air an earlier face's flood went through is the same room, and isn't flooded again. Its
/// screen shows what it found.
fn filter_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let now = now();
    if now % rules::SECOND != 0 {
        return;
    }
    let dimension = game.native_dimension().to_owned();
    let powered = powered_at(&dimension, pos, now);
    let due = {
        let mut rooms = lock(&ROOMS);
        let i = match rooms.iter().position(|r| r.dimension == dimension && r.filter == pos) {
            Some(i) => i,
            None => {
                rooms.push(Room { dimension: dimension.clone(), filter: pos, cells: None, powered: 0 });
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
    if due {
        let mut sealed: Option<HashSet<[i32; 3]>> = None;
        let mut open = HashSet::new();
        let [x, y, z] = pos;
        for start in [[x, y + 1, z], [x + 1, y, z], [x - 1, y, z], [x, y, z + 1], [x, y, z - 1], [x, y - 1, z]] {
            if open.contains(&start) || sealed.as_ref().is_some_and(|s| s.contains(&start)) {
                continue;
            }
            match rules::sealed_room(start, &|p| cell(game, p), rules::ROOM_LIMIT) {
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

// ---- the shelter -------------------------------------------------------------------

/// The share of the background where `who` stands (`rules::shelter`): the sky's light at their eyes, and
/// their feet or head in a powered filter's sealed room. Kept for their window's next message.
pub(in crate::game) fn shelter(game: &Game<'_>, who: &NativeServerPlayer) -> f64 {
    let [x, y, z] = who.position;
    let feet = [x.floor() as i32, y.floor() as i32, z.floor() as i32];
    let head = [feet[0], (y + 1.0).floor() as i32, feet[2]];
    let eyes = [feet[0], (y + EYES).floor() as i32, feet[2]];
    let sky = rules::open_to_sky(game.native_light(eyes).1);
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

/// A player's eyes over their feet, standing.
const EYES: f64 = 1.62;

/// The shelter `who` had at their last second, for their window: none (1) for a player not reckoned.
pub(in crate::game) fn shelter_of(player: u128) -> f64 {
    lock(&SHELTER).iter().find(|s| s.0 == player).map_or(1.0, |s| s.1)
}

// ---- the hermetic door -------------------------------------------------------------

/// A right click on the hermetic door: a hand opens and shuts it, both halves, as a wooden door's
/// would (the iron door it copies opens only by redstone). Sneaking with something in hand places
/// against it as usual. Whether the block was the door.
pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) -> bool {
    if game.native_block(pos) != rules::HERMETIC_DOOR {
        return false;
    }
    if game.native_equipment(EquipmentSlot::MainHand).is_some() && game.native_sneaking() {
        return true;
    }
    *canceled = true;
    let Some(who) = game.native_acting_player() else { return true };
    let other = match game.native_block_property(pos, "half").as_deref() {
        Some("upper") => [pos[0], pos[1] - 1, pos[2]],
        _ => [pos[0], pos[1] + 1, pos[2]],
    };
    // Both halves are one door: its lower half names it.
    let door = if other[1] < pos[1] { other } else { pos };
    if !rules::new_press(&mut lock(&DOOR_PRESSES), who.uuid, door, now()) {
        return true;
    }
    let open = game.native_block_property(pos, "open").as_deref() != Some("true");
    let value = if open { "true" } else { "false" };
    // The game's door brings its other half along; set it too, in case it doesn't.
    game.native_set_block_property(pos, "open", value);
    if game.native_block(other) == rules::HERMETIC_DOOR {
        game.native_set_block_property(other, "open", value);
    }
    let event = if open { "minecraft:block.iron_door.open" } else { "minecraft:block.iron_door.close" };
    game.native_play_sound_in_place(event, glue::middle(pos), BLOCKS, 1.0, 0.9);
    true
}

// ---- the screens -------------------------------------------------------------------
//
// On the game's plain panel, as the launch console's: the generator's fuel slot and its
// flame (the decontaminator's sprites, in its screen's texture), the filter's power and room.

const FUEL_AT: [f32; 2] = [80.0, 53.0];
const FLAME_AT: [f32; 2] = [62.0, 54.0];
/// The decontaminator's screen's texture: its unlit flame at `FLAME_AT`, its lit one right of the panel.
const DECON_GUI: &str = "big_nukes:gui/decontaminator";
const LIT_FLAME: [f32; 2] = [176.0, 0.0];
const LABEL: u32 = 0x40_4040;
const GOOD: u32 = 0x2A_7A2A;
const BAD: u32 = 0x9A_2A1A;
const WIDTH: f32 = 176.0;

pub(in crate::game) fn screen(block: &str) -> Option<NativeScreen> {
    let slots = match block {
        rules::FUEL_GENERATOR => vec![Some(FUEL_AT)],
        rules::AIR_FILTER => vec![None],
        _ => return None,
    };
    Some(NativeScreen { width: WIDTH, height: 166.0, slots, inventory_y: 84.0, inventory_x: 8.0, own_labels: true, own_background: false })
}

/// Draws a bunker block's screen.
pub(in crate::game) fn screen_draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let [x, y] = origin;
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    let centred = |line: &str, row: f32, colour: u32, batch: &mut HudBatch| {
        let line = rules::fit(line, WIDTH - 16.0, &|t| game.native_text_width(t));
        let width = game.native_text_width(&line);
        game.native_text(batch, &line, [x + ((WIDTH - width) / 2.0).round(), y + row], colour, false);
    };
    let radius = rules::POWER_RADIUS.to_string();
    match block {
        rules::FUEL_GENERATOR => {
            centred(&lang::get("big_nukes.screen.fuel_generator"), 6.0, LABEL, batch);
            let size = rules::FLAME_HEIGHT as f32;
            game.native_sprite(batch, DECON_GUI, [FLAME_AT[0], FLAME_AT[1], size, size], [x + FLAME_AT[0], y + FLAME_AT[1], size, size]);
            let flame = rules::flame_height(get(0), get(1)) as f32;
            if flame > 0.0 {
                let skip = size - flame;
                game.native_sprite(batch, DECON_GUI, [LIT_FLAME[0], LIT_FLAME[1] + skip, size, flame], [x + FLAME_AT[0], y + FLAME_AT[1] + skip, size, flame]);
            }
            let (line, colour) = if get(0) > 0 {
                (lang::tr("big_nukes.screen.fuel_generator.burning", &[&radius]), GOOD)
            } else {
                (lang::get("big_nukes.screen.fuel_generator.idle"), BAD)
            };
            centred(&line, 24.0, colour, batch);
        }
        rules::AIR_FILTER => {
            centred(&lang::get("big_nukes.screen.air_filter"), 6.0, LABEL, batch);
            let (power, colour) = if get(0) > 0 {
                (lang::get("big_nukes.screen.air_filter.powered"), GOOD)
            } else {
                (lang::tr("big_nukes.screen.air_filter.unpowered", &[&radius]), BAD)
            };
            centred(&power, 26.0, colour, batch);
            if get(0) > 0 {
                let (room, colour) = if get(1) > 0 {
                    (lang::tr("big_nukes.screen.air_filter.sealed", &[&get(2).to_string()]), GOOD)
                } else {
                    (lang::tr("big_nukes.screen.air_filter.open", &[&rules::ROOM_LIMIT.to_string()]), BAD)
                };
                centred(&room, 42.0, colour, batch);
            }
        }
        _ => return,
    }
    game.native_text(batch, &lang::get("container.inventory"), [x + 8.0, y + 72.0], LABEL, false);
}

/// The world closing: what is queued and heard goes.
pub(in crate::game) fn forget() {
    lock(&HEARD).clear();
    lock(&GENERATORS).clear();
    lock(&ROOMS).clear();
    lock(&SHELTER).clear();
    lock(&DOOR_PRESSES).clear();
    TICK.store(0, Ordering::Relaxed);
}
