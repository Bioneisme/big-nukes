// Big Nukes' iodine and decontaminator in the game (plan 9): the decontaminator's
// block, its fuel, its circle and its passes over the grass, and the iodine's
// effect on the dose. Added to the game as `game::big_nukes_decon`, next to the
// rest of the glue (`game::big_nukes_glue`, glue.rs), whose shared hooks hand the
// block here by its id.

use crate::game::{Game, big_nukes as rules, big_nukes_glue as glue};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
    lang,
    mod_api::NativeScreen,
};
use horizon_mods::content::Block;

/// The screens' labels, as the machines' have them.
const LABEL: u32 = 0x40_4040;

// ---- the block -------------------------------------------------------------------
//
// A furnace as a template, as the centrifuge: it opens a menu and faces the player
// who placed it. One slot, the fuel's; hoppers bring fuel from every side.

pub(in crate::game) fn blocks(blocks: &mut Vec<Block>, block: impl Fn(&str, &str, f32) -> Block) {
    blocks.push(block(rules::DECONTAMINATOR, "minecraft:furnace", 3.5));
}

pub(in crate::game) fn container() -> ModContainer {
    ModContainer {
        slots: rules::DECON_SLOTS,
        faces: std::array::from_fn(|_| vec![rules::DECON_FUEL_SLOT]),
        inputs: vec![rules::DECON_FUEL_SLOT],
        outputs: Vec::new(),
        menu: Some(rules::DECONTAMINATOR.to_owned()),
    }
}

/// What a hopper may put in: fuel.
pub(in crate::game) fn accepts(game: &Game<'_>, slot: usize, stack: &ItemStack) -> bool {
    slot == rules::DECON_FUEL_SLOT && glue::burn_time(game, stack) > 0
}

/// A shift-click from the inventory puts fuel in its slot, and nothing else. One from
/// the decontaminator goes the game's way, to the inventory.
pub(in crate::game) fn quick_move(game: &Game<'_>, entity: &mut ModBlockEntity, container_slots: usize, slot: usize, stack: &mut ItemStack, handled: &mut bool) {
    if slot < container_slots {
        return;
    }
    *handled = true;
    if glue::burn_time(game, stack) <= 0 {
        return;
    }
    if entity.items.len() < rules::DECON_SLOTS {
        entity.items.resize(rules::DECON_SLOTS, None);
    }
    let most = game.native_max_stack_size(&stack.item).unwrap_or(64);
    match entity.items[rules::DECON_FUEL_SLOT].as_mut() {
        None => {
            entity.items[rules::DECON_FUEL_SLOT] = Some(stack.clone());
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

// ---- its screen --------------------------------------------------------------------

/// Where the screen's parts are, from its top left: tools/make_textures.py draws them in
/// the same places. The slot is its item's corner.
const FUEL_AT: [f32; 2] = [80.0, 53.0];
const FLAME_AT: [f32; 2] = [62.0, 54.0];
const BAR_AT: [f32; 2] = [34.0, 18.0];
const BAR_HEIGHT: f32 = 8.0;
const RADIUS_Y: f32 = 30.0;
const BACKGROUND_Y: f32 = 41.0;
/// The lit flame right of the panel in the texture, the full bar under it.
const LIT_FLAME: [f32; 2] = [176.0, 0.0];
const FULL_BAR: [f32; 2] = [0.0, 166.0];
const GUI: &str = "big_nukes:gui/decontaminator";

pub(in crate::game) fn screen() -> NativeScreen {
    NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some(FUEL_AT)],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: true,
    }
}

/// The flame of its fuel, the bar of its radius with "R / 48" under it, and the
/// background at the block (`Decontaminator::data`).
pub(in crate::game) fn screen_draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let [x, y] = origin;
    game.native_sprite(batch, GUI, [0.0, 0.0, 176.0, 166.0], [x, y, 176.0, 166.0]);
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    let flame = rules::flame_height(get(0), get(1)) as f32;
    if flame > 0.0 {
        let skip = rules::FLAME_HEIGHT as f32 - flame;
        game.native_sprite(
            batch,
            GUI,
            [LIT_FLAME[0], LIT_FLAME[1] + skip, rules::FLAME_HEIGHT as f32, flame],
            [x + FLAME_AT[0], y + FLAME_AT[1] + skip, rules::FLAME_HEIGHT as f32, flame],
        );
    }
    let bar = rules::radius_bar(get(2)) as f32;
    if bar > 0.0 {
        game.native_sprite(batch, GUI, [FULL_BAR[0], FULL_BAR[1], bar, BAR_HEIGHT], [x + BAR_AT[0], y + BAR_AT[1], bar, BAR_HEIGHT]);
    }
    let centred = |line: &str, row: f32, batch: &mut HudBatch| {
        let width = game.native_text_width(line);
        game.native_text(batch, line, [x + ((176.0 - width) / 2.0).round(), y + row], LABEL, false);
    };
    centred(&lang::get("big_nukes.screen.decontaminator"), 6.0, batch);
    let (radius, most) = (get(2).max(0).to_string(), rules::CLEAN_RADIUS.to_string());
    centred(&lang::tr("big_nukes.screen.decontaminator.radius", &[&radius, &most]), RADIUS_Y, batch);
    let background = format!("{:.1}", f64::from(get(3).max(0)) / 10.0);
    centred(&lang::tr("big_nukes.screen.decontaminator.background", &[&background]), BACKGROUND_Y, batch);
    game.native_text(batch, &lang::get("container.inventory"), [x + 8.0, y + 72.0], LABEL, false);
}
