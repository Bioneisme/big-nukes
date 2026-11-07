// Big Nukes' bunker in the game (plan 10): its walls, the hermetic door, the fuel
// generator, the air filter and the lamps. Added to the game as `game::big_nukes_bunker`,
// next to the rest of the glue (`game::big_nukes_glue`, glue.rs), whose shared hooks hand
// the blocks here by their ids.

use crate::game::big_nukes as rules;
use horizon_mods::content::{Block, BlockProperties};

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
