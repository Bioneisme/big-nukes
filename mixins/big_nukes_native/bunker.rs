// Big Nukes' bunker in the game (plan 10): its walls, the hermetic door, the fuel
// generator, the air filter and the lamps. Added to the game as `game::big_nukes_bunker`,
// next to the rest of the glue (`game::big_nukes_glue`, glue.rs), whose shared hooks hand
// the blocks here by their ids.

use crate::game::{Game, big_nukes as rules, big_nukes_glue::lock};
use horizon_mods::content::{Block, BlockProperties};
use std::{collections::BTreeMap, sync::Mutex};

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
        };
        let resistance = kind.explosion_resistance();
        blocks.push(block(kind.id(), "minecraft:stone", hardness, resistance, sounds));
        blocks.push(block(kind.cracked_id(), "minecraft:stone", hardness / 2.0, resistance / 2.0, sounds));
    }
    let armored = rules::WallKind::Armored.explosion_resistance();
    blocks.push(block(rules::HERMETIC_DOOR, "minecraft:iron_door", 10.0, armored, None));
    blocks.push(block(rules::FUEL_GENERATOR, "minecraft:furnace", 3.5, 6.0, None));
    blocks.push(block(rules::AIR_FILTER, "minecraft:furnace", 3.5, 6.0, None));
    blocks.push(block(rules::BUNKER_LAMP, "minecraft:waxed_copper_bulb", 3.0, 6.0, None));
}

// ---- the walls' registry -------------------------------------------------------------
//
// The world keeps its walls by chunk (`Saved.walls`). The level tells of every block set
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

/// The world closing: what is queued goes.
pub(in crate::game) fn forget() {
    lock(&HEARD).clear();
}
