// Big Nukes' silo and bomber in the game: the launch pad, the hatch, the launch
// console, the missile's flight, the strike beacon and the bomber's pass. Added
// to the game as `game::big_nukes_silo`, next to the rest of the glue
// (`game::big_nukes_glue`, glue.rs), whose shared hooks hand these blocks here
// by their ids.

use crate::game::{Game, big_nukes as rules};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
    lang,
    mod_api::NativeScreen,
};
use horizon_core::geometry::Direction;
use horizon_mods::content::{Block, Item, Rarity};

/// The screens' labels, as the machines' have them.
const LABEL: u32 = 0x40_4040;

// ---- blocks ----------------------------------------------------------------------
//
// The launch pad holds the missile and its warhead in a menu of two slots; the
// console and the beacon have one hidden slot each, as a bomb has: the console's
// for its menu, which needs one, the beacon's for the bomb it was charged with,
// so a broken beacon drops it. The hatch is a plain block.

pub(in crate::game) fn blocks(blocks: &mut Vec<Block>, block: impl Fn(&str, &str, f32) -> Block) {
    blocks.push(block(rules::LAUNCH_PAD, "minecraft:furnace", 5.0));
    blocks.push(block(rules::SILO_HATCH, "minecraft:iron_block", 5.0));
    blocks.push(block(rules::LAUNCH_CONSOLE, "minecraft:furnace", 3.5));
    blocks.push(block(rules::STRIKE_BEACON, "minecraft:furnace", 3.5));
}

/// What the flying displays show: items no one is meant to hold.
pub(in crate::game) fn model_items(items: &mut Vec<Item>) {
    for id in rules::MODEL_ITEMS {
        items.push(Item {
            id: id.into(),
            template: "minecraft:paper".into(),
            max_stack_size: Some(1),
            rarity: Some(Rarity::Common),
            ..Default::default()
        });
    }
}

/// The silo's blocks with a block entity.
pub(in crate::game) fn has_entity(block: &str) -> bool {
    matches!(block, rules::LAUNCH_PAD | rules::LAUNCH_CONSOLE | rules::STRIKE_BEACON)
}

pub(in crate::game) fn container(block: &str) -> ModContainer {
    match block {
        // Hoppers bring the missile from above and the warhead from the sides.
        rules::LAUNCH_PAD => {
            let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| vec![rules::PAD_WARHEAD_SLOT]);
            faces[Direction::Up as usize] = vec![rules::PAD_MISSILE_SLOT];
            ModContainer {
                slots: rules::PAD_SLOTS,
                faces,
                inputs: vec![rules::PAD_MISSILE_SLOT, rules::PAD_WARHEAD_SLOT],
                outputs: vec![],
                menu: Some(rules::LAUNCH_PAD.to_owned()),
            }
        }
        _ => ModContainer {
            slots: 1,
            faces: Default::default(),
            inputs: vec![],
            outputs: vec![],
            menu: (block == rules::LAUNCH_CONSOLE).then(|| rules::CONSOLE_MENU.to_owned()),
        },
    }
}

/// A shift-click from the inventory onto the pad: the missile and the warhead to their slots.
pub(in crate::game) fn pad_quick_move(
    game: &Game<'_>,
    entity: &mut ModBlockEntity,
    container_slots: usize,
    slot: usize,
    stack: &mut ItemStack,
    handled: &mut bool,
) {
    if slot < container_slots {
        return;
    }
    *handled = true;
    if entity.items.len() < rules::PAD_SLOTS {
        entity.items.resize(rules::PAD_SLOTS, None);
    }
    let Some(to) = [rules::PAD_MISSILE_SLOT, rules::PAD_WARHEAD_SLOT].into_iter().find(|s| rules::pad_takes(*s, &stack.item)) else {
        return;
    };
    let most = game.native_max_stack_size(&stack.item).unwrap_or(64);
    match entity.items[to].as_mut() {
        None => {
            entity.items[to] = Some(stack.clone());
            stack.count = 0;
        }
        Some(have) if have.item == stack.item => {
            let moved = most.saturating_sub(have.count).min(stack.count);
            have.count += moved;
            stack.count -= moved;
        }
        Some(_) => {}
    }
}

// ---- the pad's screen ------------------------------------------------------------

/// A furnace's size: the two slots side by side with their names under them.
const PAD_MISSILE_AT: [f32; 2] = [52.0, 30.0];
const PAD_WARHEAD_AT: [f32; 2] = [108.0, 30.0];

pub(in crate::game) fn pad_screen() -> NativeScreen {
    NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some(PAD_MISSILE_AT), Some(PAD_WARHEAD_AT)],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

pub(in crate::game) fn pad_draw(game: &Game<'_>, batch: &mut HudBatch, origin: [f32; 2]) {
    let [x, y] = origin;
    let centered = |batch: &mut HudBatch, text: &str, middle: f32, top: f32| {
        let width = game.native_text_width(text);
        game.native_text(batch, text, [x + middle - width / 2.0, y + top], LABEL, false);
    };
    centered(batch, &lang::get("big_nukes.screen.launch_pad"), 88.0, 6.0);
    centered(batch, &lang::get("big_nukes.screen.launch_pad.missile"), PAD_MISSILE_AT[0] + 8.0, PAD_MISSILE_AT[1] + 22.0);
    centered(batch, &lang::get("big_nukes.screen.launch_pad.warhead"), PAD_WARHEAD_AT[0] + 8.0, PAD_WARHEAD_AT[1] + 22.0);
    game.native_text(batch, &lang::get("container.inventory"), [x + 8.0, y + 72.0], LABEL, false);
}
