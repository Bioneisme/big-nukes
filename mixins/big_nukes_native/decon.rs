// Big Nukes' iodine and decontaminator in the game (plan 9): the decontaminator's
// block, its fuel, its circle and its passes over the grass, and the iodine's
// effect on the dose. Added to the game as `game::big_nukes_decon`, next to the
// rest of the glue (`game::big_nukes_glue`, glue.rs), whose shared hooks hand the
// block here by its id.

use crate::game::{
    Game, NativeHeightmap, big_nukes as rules,
    big_nukes_glue::{self as glue, lock},
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
    lang,
    mod_api::NativeScreen,
};
use horizon_core::geometry::Direction;
use horizon_mods::content::Block;
use horizon_world::nbt::{JavaString, Tag};
use std::{
    collections::BTreeSet,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

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

// ---- its work --------------------------------------------------------------------
//
// Its state lives in its block entity, as the centrifuge's does: `nbt` holds the rules'
// `Decontaminator` as JSON, `items` the fuel, `data` what its screen shows. Its circle is
// the world's (`Saved.cleans`), so the background is reckoned from it everywhere, and it
// stays when the block is broken. The block ticks only in a loaded chunk: elsewhere the
// circle stands, and keeps cleaning what it reached.

/// The columns to turn back to grass, by dimension, queued by the blocks' ticks and
/// edited at the end of their level's tick, where the waves' edits are made.
static REGROW: Mutex<Vec<(String, Vec<[i32; 2]>)>> = Mutex::new(Vec::new());
/// A refused grass edit is logged once a world.
static WARNED: AtomicBool = AtomicBool::new(false);

fn key_name() -> JavaString {
    JavaString::from("decontaminator")
}

fn load(entity: &ModBlockEntity) -> Option<rules::Decontaminator> {
    match entity.nbt.get(&key_name()) {
        Some(Tag::ByteArray(bytes)) => serde_json::from_slice(bytes).ok(),
        _ => None,
    }
}

fn store(entity: &mut ModBlockEntity, decontaminator: &rules::Decontaminator) {
    if let Ok(bytes) = serde_json::to_vec(decontaminator) {
        entity.nbt.insert(key_name(), Tag::ByteArray(bytes));
    }
}

/// One fuel out of its slot; a lava bucket leaves its bucket.
fn take_fuel(entity: &mut ModBlockEntity) {
    let Some(stack) = entity.items[rules::DECON_FUEL_SLOT].as_mut() else {
        return;
    };
    stack.count -= 1;
    if stack.count == 0 {
        let left = rules::remainder(&stack.item);
        entity.items[rules::DECON_FUEL_SLOT] = left.map(|item| ItemStack { item: item.into(), count: 1, damage: 0, extra: None });
    }
}

/// What lies in the fuel slot and doesn't burn (the bucket a lava bucket leaves) goes down into the
/// container under the block, a hopper say, as a furnace's bucket goes: the slot is free for the next fuel.
fn spent_out(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(stack) = entity.items[rules::DECON_FUEL_SLOT].clone() else {
        return;
    };
    if glue::burn_time(game, &stack) > 0 {
        return;
    }
    let left = game.native_insert_item([pos[0], pos[1] - 1, pos[2]], Direction::Up, stack.clone());
    if left.as_ref() != Some(&stack) {
        entity.items[rules::DECON_FUEL_SLOT] = left;
    }
}

/// A tick: the fuel burns and the circle grows (`Decontaminator::step`), the world's
/// circle is kept (`Saved::cleaned`: a blast over it starts it again), once a second a
/// pass of its columns is queued for the grass, and the screen shows the background here.
pub(in crate::game) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.items.len() < rules::DECON_SLOTS {
        entity.items.resize(rules::DECON_SLOTS, None);
    }
    let was = load(entity);
    let mut decon = was.unwrap_or_default();
    let fuel = entity.items[rules::DECON_FUEL_SLOT]
        .as_ref()
        .map(|s| glue::burn_time(game, s))
        .filter(|t| *t > 0)
        .map(i32::unsigned_abs);
    let step = decon.step(fuel);
    if step.took_fuel {
        take_fuel(entity);
    }
    if fuel.is_none() {
        spent_out(game, pos, entity);
    }
    let dimension = game.native_dimension().to_owned();
    let (mut pass, mut background) = (Vec::new(), 0.0);
    if let Some(mut guard) = glue::state(game) {
        let st = guard.as_mut().expect("opened");
        let radius = st.saved.cleaned(&dimension, pos, decon.radius);
        st.dirty |= step.made || radius != decon.radius;
        decon.keep_circle(radius);
        let now = st.saved.tick;
        // A pass waits while any of its columns is in a chunk not loaded: the edit would skip it for good.
        if now % rules::SECOND == 0 {
            let from = decon.next_column;
            pass = decon.regrow_pass().into_iter().map(|[dx, dz]| [pos[0] + dx, pos[2] + dz]).collect();
            if !pass.iter().all(|&[x, z]| glue::loaded(&dimension, [x, pos[1], z])) {
                decon.next_column = from;
                pass.clear();
            }
        }
        // What it is fighting: the background here before any cleaning.
        background = rules::background(&st.saved.zones, &[], &dimension, glue::middle(pos), now);
    }
    if !pass.is_empty() {
        lock(&REGROW).push((dimension, pass));
    }
    let data = decon.data(background).to_vec();
    if entity.data != data {
        entity.data = data;
    }
    if was != Some(decon) {
        store(entity, &decon);
    }
}

/// The end of a level's tick: its queued passes turn the ash on their columns' surface
/// back to grass, in one edit.
pub(in crate::game) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    let columns: Vec<[i32; 2]> = {
        let mut queue = lock(&REGROW);
        let (here, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *queue).into_iter().partition(|(d, _)| d == dimension);
        *queue = rest;
        here.into_iter().flat_map(|(_, c)| c).collect()
    };
    if columns.is_empty() {
        return;
    }
    let ash = glue::ash_here(game, dimension);
    // Each column read down from its highest block: the ash under what the wave left over it.
    let target = rules::regrow_target(ash);
    let cols: Vec<(i32, i32)> = columns.iter().map(|&[x, z]| (x, z)).collect();
    let depth = rules::REGROW_DEPTH as usize;
    let read = match game.native_read_columns(dimension, &cols, NativeHeightmap::WorldSurface, (rules::REGROW_DEPTH, 0)) {
        Ok(read) => read,
        Err(e) => {
            if !WARNED.swap(true, Ordering::Relaxed) {
                log::warn!("[big_nukes] the decontaminator couldn't read the ground: {e}");
            }
            return;
        }
    };
    let found: Vec<[i32; 3]> = columns
        .iter()
        .enumerate()
        .filter_map(|(i, &[x, z])| {
            let height = (*read.bases.get(i)?)?;
            let column = read.blocks.get(i * depth..(i + 1) * depth)?;
            let top_down = column.iter().rev().map(|&b| read.palette.get(usize::from(b)).map_or("minecraft:air", String::as_str));
            let k = rules::ash_under(top_down, target)?;
            Some([x, height - 1 - k as i32, z])
        })
        .collect();
    if found.is_empty() {
        return;
    }
    let edit = rules::regrow_edit(&found, ash);
    if let Err(e) = game.native_edit_blocks(dimension, &glue::native(edit))
        && !WARNED.swap(true, Ordering::Relaxed)
    {
        log::warn!("[big_nukes] the decontaminator's grass was refused: {e}");
    }
}

// ---- iodine ----------------------------------------------------------------------
//
// A tablet's consume effect gives the player the mod's iodine effect by a command, as the
// radiation's effect is given; the effect, while it ticks on a player, tells the dose to
// shield them. Without the assets half the effect isn't in the game: the command fails,
// nothing ticks, and the tablet does nothing.

/// The players who ate a tablet, given the effect at the end of the server's tick, where
/// commands run (a player's hook answers false).
static TABLETS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The players the iodine's effect ticked on since their last player tick.
static IODINE_ON: Mutex<BTreeSet<u128>> = Mutex::new(BTreeSet::new());

/// A consume effect of the mod's own kind: a tablet's. Whether it was.
pub(in crate::game) fn consume_effect(game: &Game<'_>, kind: &str) -> bool {
    if kind != rules::IODINE_EFFECT {
        return false;
    }
    match game.native_acting_player() {
        Some(who) => lock(&TABLETS).push(who.name),
        None => log::warn!("[big_nukes] {kind}: no eater"),
    }
    true
}

/// The end of the server's tick, where commands run: the tablets eaten give their effect.
pub(in crate::game) fn server_tick(game: &mut Game<'_>) {
    let eaten = std::mem::take(&mut *lock(&TABLETS));
    for name in eaten {
        game.native_run_command(&rules::iodine_command(&name));
    }
}

/// The iodine's effect acts every tick it is on (API 56): `effect_tick` hears it.
pub(in crate::game) fn effect_applies(game: &Game<'_>, effect: &str, apply: &mut bool) {
    if effect == rules::IODINE_EFFECT && game.native_on(rules::ID) {
        *apply = true;
    }
}

/// The iodine's effect ticks on a player (`entity` None, the player swapped in): their
/// next player tick shields their dose.
pub(in crate::game) fn effect_tick(game: &Game<'_>, effect: &str, entity: Option<i32>) {
    if effect != rules::IODINE_EFFECT || entity.is_some() {
        return;
    }
    if let Some(who) = game.native_acting_player() {
        lock(&IODINE_ON).insert(who.uuid);
    }
}

/// Whether the iodine's effect ticked on `player` since this was last asked: asked once
/// a player tick, after the effects' tick.
pub(in crate::game) fn iodine_on(player: u128) -> bool {
    lock(&IODINE_ON).remove(&player)
}

/// The world closing: what is queued goes.
pub(in crate::game) fn forget() {
    lock(&REGROW).clear();
    lock(&TABLETS).clear();
    lock(&IODINE_ON).clear();
    WARNED.store(false, Ordering::Relaxed);
}
