//! Iodine and the decontaminator (plan 9): the tablets and their effect, the decontaminator's block, fuel, circle
//! and its passes regrowing the grass.

use crate::core::{self, Pl, argb, lock};
use crate::{keys, rules};
use mod_api::{
    ActiveEffect, ApplyEffectTickEvent, Block, ClientHost, Compound, Direction, DrawContext, Heightmap, Host, HostTransaction,
    ItemResource, ItemStack, LevelRef, MenuQuickMoveEvent, ModBlockEntityView, ModContainer, Screen, ServerHost,
    ShouldApplyEffectTickThisTickEvent, Tag, ITEMS,
};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

/// The screens' labels, as the machines' have them.
const LABEL: u32 = 0x40_4040;

// ---- declarations -----------------------------------------------------------------
//
// A furnace as a template, as the centrifuge: it opens a menu and faces the player who placed it. One slot, the
// fuel's; hoppers bring things to it from every side.

/// The decontaminator, `block(id, template, hardness)` as the core's. (`rules::DECONTAMINATOR` has a block entity,
/// registered with the core's `Machine` part.)
pub(crate) fn blocks(blocks: &mut Vec<Block>, block: &dyn Fn(&str, &str, f32) -> Block) {
    blocks.push(block(rules::DECONTAMINATOR, "minecraft:furnace", 3.5));
}

/// The decontaminator's container (slots, faces, menu), read once at registration.
///
/// No hook filters what a hopper puts in (the old `container_accepts`, fuel only): a hopper may put anything into
/// the fuel slot. What doesn't burn goes on down into the container under the block (`spent_out`), as the bucket a
/// lava bucket leaves always did; with nothing under it, it waits in the slot for a hand to take it.
pub(crate) fn container() -> Option<ModContainer> {
    Some(ModContainer {
        slots: rules::DECON_SLOTS,
        faces: std::array::from_fn(|_| vec![rules::DECON_FUEL_SLOT]),
        inputs: vec![rules::DECON_FUEL_SLOT],
        outputs: Vec::new(),
        menu: Some(rules::DECONTAMINATOR.to_owned()),
    })
}

// ---- the decontaminator -------------------------------------------------------------
//
// Its state lives in its block entity, as the centrifuge's does: `nbt` holds the rules' `Decontaminator` as JSON,
// `items` the fuel, `data` what its screen shows. Its circle is the world's (`Saved.cleans`), so the background is
// reckoned from it everywhere, and it stays when the block is broken. The block ticks only in a loaded chunk:
// elsewhere the circle stands, and keeps cleaning what it reached.

/// The columns to turn back to grass, by dimension, queued by the blocks' ticks and edited at the end of their
/// level's tick, where the waves' edits are made.
static REGROW: Mutex<Vec<(String, Vec<[i32; 2]>)>> = Mutex::new(Vec::new());
/// A refused grass edit is logged once a world.
static WARNED: AtomicBool = AtomicBool::new(false);

const DECON_KEY: &str = "decontaminator";

fn load(nbt: &Compound) -> Option<rules::Decontaminator> {
    match nbt.get(&DECON_KEY.into()) {
        Some(Tag::ByteArray(bytes)) => serde_json::from_slice(bytes).ok(),
        _ => None,
    }
}

fn store(nbt: &mut Compound, decontaminator: &rules::Decontaminator) {
    if let Ok(bytes) = serde_json::to_vec(decontaminator) {
        nbt.insert(DECON_KEY.into(), Tag::ByteArray(bytes));
    }
}

/// One fuel out of `slot`; a lava bucket leaves its bucket (`rules::remainder`). The fuel generator's too.
pub(crate) fn take_fuel(host: &dyn Host, items: &mut [Option<ItemStack>], slot: usize) {
    let Some(stack) = items[slot].as_mut() else {
        return;
    };
    let left = stack.count().saturating_sub(1);
    if left == 0 {
        let id = stack.item().id(host);
        items[slot] = rules::remainder(&id).and_then(|left| core::stack_of(host, left));
    } else {
        let _ = stack.set_count(left);
    }
}

/// What lies in `slot` and doesn't burn (the bucket a lava bucket leaves, or whatever a hopper put in) goes down
/// into the container under the block, a hopper say, as a furnace's bucket goes: the slot is free for the next fuel.
/// The fuel generator's too.
pub(crate) fn spent_out(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], items: &mut [Option<ItemStack>], slot: usize) {
    let Some(stack) = items[slot].clone() else {
        return;
    };
    if core::burn_time(host, &stack) > 0 {
        return;
    }
    let Some(below) = level.capability(host, &ITEMS, [pos[0], pos[1] - 1, pos[2]], Some(Direction::Up)) else {
        return;
    };
    let resource = ItemResource::of(&stack);
    let count = stack.count();
    let moved = host.transaction(|host, tx| Ok::<u32, ()>(below.insert_any(host, tx, &resource, count))).unwrap_or(0);
    if moved > 0 {
        items[slot] = resource.to_stack(count.saturating_sub(moved));
    }
}

/// Its server tick. `host.dimension()` is its level's.
///
/// The fuel burns and the circle grows (`Decontaminator::step`), the world's circle is kept (`Saved::cleaned`: a
/// blast over it starts it again), once a second a pass of its columns is queued for the grass, and the screen shows
/// the background here.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    if entity.items.len() < rules::DECON_SLOTS {
        entity.items.resize(rules::DECON_SLOTS, None);
    }
    let was = load(&entity.nbt);
    let mut decon = was.unwrap_or_default();
    let fuel = entity.items[rules::DECON_FUEL_SLOT]
        .as_ref()
        .map(|s| core::burn_time(host, s))
        .filter(|t| *t > 0)
        .map(i32::unsigned_abs);
    let step = decon.step(fuel);
    if step.took_fuel {
        take_fuel(host, &mut entity.items, rules::DECON_FUEL_SLOT);
    }
    let dimension = host.dimension();
    if fuel.is_none()
        && let Some(level) = host.level(dimension)
    {
        spent_out(host, level, pos, &mut entity.items, rules::DECON_FUEL_SLOT);
    }
    let (mut pass, mut background) = (Vec::new(), 0.0);
    if let Some(mut guard) = core::state(host) {
        let st = guard.as_mut().expect("opened");
        let radius = st.saved.cleaned(dimension, pos, decon.radius);
        st.dirty |= step.made || radius != decon.radius;
        decon.keep_circle(radius);
        let now = st.saved.tick;
        // A pass waits while any of its columns is in a chunk not loaded: the edit would skip it for good.
        if now % rules::SECOND == 0 {
            let from = decon.next_column;
            pass = decon.regrow_pass().into_iter().map(|[dx, dz]| [pos[0] + dx, pos[2] + dz]).collect();
            if !pass.iter().all(|&[x, z]| core::loaded(dimension, [x, pos[1], z])) {
                decon.next_column = from;
                pass.clear();
            }
        }
        // What it is fighting: the background here before any cleaning.
        background = rules::background(&st.saved.zones, &[], dimension, core::middle(pos), now);
    }
    if !pass.is_empty() {
        lock(&REGROW).push((dimension.to_owned(), pass));
    }
    let data = decon.data(background).to_vec();
    if entity.data != data {
        entity.data = data;
    }
    if was != Some(decon) {
        store(&mut entity.nbt, &decon);
    }
}

/// The end of a level's tick: the passes regrowing the grass (bulk edits through `core::native`). Its queued
/// passes turn the ash on their columns' surface back to grass, in one edit.
pub(crate) fn level_tick(host: &mut dyn ServerHost, _level: LevelRef, dimension: &str) {
    let columns: Vec<[i32; 2]> = {
        let mut queue = lock(&REGROW);
        let (here, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *queue).into_iter().partition(|(d, _)| d == dimension);
        *queue = rest;
        here.into_iter().flat_map(|(_, c)| c).collect()
    };
    if columns.is_empty() {
        return;
    }
    let ash = core::ash_here(host, dimension);
    // Each column read down from its highest block: the ash under what the wave left over it.
    let target = rules::regrow_target(ash);
    let cols: Vec<(i32, i32)> = columns.iter().map(|&[x, z]| (x, z)).collect();
    let depth = rules::REGROW_DEPTH as usize;
    let read = match host.read_columns(dimension, &cols, Heightmap::WorldSurface, (rules::REGROW_DEPTH, 0)) {
        Ok(read) => read,
        Err(e) => {
            if !WARNED.swap(true, Ordering::Relaxed) {
                log::warn!("[big_nukes] the decontaminator couldn't read the ground: {e}");
            }
            return;
        }
    };
    let names: Vec<String> = read.palette.iter().map(|s| s.block().id(host)).collect();
    let found: Vec<[i32; 3]> = columns
        .iter()
        .enumerate()
        .filter_map(|(i, &[x, z])| {
            let height = (*read.bases.get(i)?)?;
            let column = read.blocks.get(i * depth..(i + 1) * depth)?;
            let top_down = column.iter().rev().map(|&b| names.get(usize::from(b)).map_or("minecraft:air", String::as_str));
            let k = rules::ash_under(top_down, target)?;
            Some([x, height - 1 - k as i32, z])
        })
        .collect();
    if found.is_empty() {
        return;
    }
    let edit = rules::regrow_edit(&found, ash);
    let result = core::native(host, edit).and_then(|edit| host.edit_blocks(dimension, &edit));
    if let Err(e) = result
        && !WARNED.swap(true, Ordering::Relaxed)
    {
        log::warn!("[big_nukes] the decontaminator's grass was refused: {e}");
    }
}

/// A shift-click in the decontaminator's menu, and the fuel generator's (bunker.rs). Whether it was handled.
///
/// One from the block goes the game's way, to the inventory. One from the inventory: fuel goes the game's way too,
/// into the one slot there is (a mod-api stack can't be emptied here, so a whole stack's move must be the game's);
/// anything else doesn't move.
pub(crate) fn quick_move(host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> bool {
    if event.slot < event.container_slots {
        return false;
    }
    core::burn_time(host, event.stack) <= 0
}

// ---- its screen --------------------------------------------------------------------

/// Where the screen's parts are, from its top left: tools/make_textures.py draws them in the same places. The slot
/// is its item's corner.
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
const WIDTH: f32 = 176.0;

pub(crate) fn screen() -> Option<Screen> {
    Some(Screen {
        width: WIDTH,
        height: 166.0,
        slots: vec![Some(FUEL_AT)],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: true,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

/// The flame of its fuel, the bar of its radius with "R / 48" under it, and the background at the block
/// (`Decontaminator::data`).
pub(crate) fn screen_draw(host: &dyn ClientHost, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    let [x, y] = origin;
    draw.sprite(GUI, [0.0, 0.0, WIDTH, 166.0], [x, y, WIDTH, 166.0], None);
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    let flame = rules::flame_height(get(0), get(1)) as f32;
    if flame > 0.0 {
        let skip = rules::FLAME_HEIGHT as f32 - flame;
        draw.sprite(
            GUI,
            [LIT_FLAME[0], LIT_FLAME[1] + skip, rules::FLAME_HEIGHT as f32, flame],
            [x + FLAME_AT[0], y + FLAME_AT[1] + skip, rules::FLAME_HEIGHT as f32, flame],
            None,
        );
    }
    let bar = rules::radius_bar(get(2)) as f32;
    if bar > 0.0 {
        draw.sprite(GUI, [FULL_BAR[0], FULL_BAR[1], bar, BAR_HEIGHT], [x + BAR_AT[0], y + BAR_AT[1], bar, BAR_HEIGHT], None);
    }
    let centred = |draw: &mut dyn DrawContext, line: &str, row: f32| {
        let width = draw.text_width(line);
        draw.text(line, [x + ((WIDTH - width) / 2.0).round(), y + row], argb(LABEL), false);
    };
    centred(draw, &core::lang(host, "big_nukes.screen.decontaminator"), 6.0);
    let (radius, most) = (get(2).max(0).to_string(), rules::CLEAN_RADIUS.to_string());
    centred(draw, &core::lang_tr(host, "big_nukes.screen.decontaminator.radius", &[&radius, &most]), RADIUS_Y);
    let background = format!("{:.1}", f64::from(get(3).max(0)) / 10.0);
    centred(draw, &core::lang_tr(host, "big_nukes.screen.decontaminator.background", &[&background]), BACKGROUND_Y);
    draw.text(&core::lang(host, "container.inventory"), [x + 8.0, y + 72.0], argb(LABEL), false);
}

// ---- iodine -----------------------------------------------------------------------
//
// A tablet's consume effect gives the player the mod's iodine effect at the end of the server's tick; while it is
// on, their tick reads it (`iodine_on`) and their dose is shielded.

/// The players who ate a tablet, given the effect at the end of the server's tick.
static TABLETS: Mutex<Vec<u128>> = Mutex::new(Vec::new());

/// A consume effect of the mod's own kind: a tablet's (`rules::IODINE_EFFECT`). Whether it was; the drinker is
/// `host.players().acting_player()`.
pub(crate) fn consume_effect(host: &mut dyn ServerHost, kind: &str) -> bool {
    if kind != rules::IODINE_EFFECT {
        return false;
    }
    match host.players().acting_player() {
        Some(who) => lock(&TABLETS).push(who.uuid()),
        None => log::warn!("[big_nukes] {kind}: no eater"),
    }
    true
}

/// The end of the server's tick, where commands run: the tablets eaten give their effect, for `IODINE_TICKS`,
/// without its particles (another tablet makes it that long again, never longer). The effect command is the
/// fallback, as before.
pub(crate) fn server_tick(host: &mut dyn ServerHost) {
    let eaten = std::mem::take(&mut *lock(&TABLETS));
    let ticks = i32::try_from(rules::IODINE_TICKS).unwrap_or(i32::MAX);
    for uuid in eaten {
        let Some(who) = core::by_uuid(host, uuid) else { continue };
        let given = host
            .living(who.player.entity())
            .is_some_and(|living| living.add_effect(host, &ActiveEffect::new(keys().iodine, ticks, 0, false, false, true)));
        if !given {
            core::run(host, &rules::iodine_command(&who.name));
        }
    }
}

/// Whether the iodine's effect is on `who` now, read in their tick (`rad::player_tick`) to shield their dose. The
/// API reads it straight, so the two effect hooks below have nothing to do.
pub(crate) fn iodine_on(host: &dyn ServerHost, who: &Pl) -> bool {
    crate::rad::has_effect(host, who, keys().iodine)
}

/// Whether a content mod's effect acts this tick (`should_apply_effect_tick_this_tick`). Nothing: `iodine_on` reads
/// the effect itself, and the old glue's tick (which only told the player's tick that it was on) has no work left.
pub(crate) fn effect_applies(_host: &dyn Host, _event: &mut ShouldApplyEffectTickThisTickEvent) {}

/// A content mod's effect acts (`apply_effect_tick`, a read-only host). Nothing: see `effect_applies`.
pub(crate) fn effect_tick(_host: &dyn Host, _event: &mut ApplyEffectTickEvent<'_>) {}

/// The world closing: what is queued goes.
pub(crate) fn forget() {
    lock(&REGROW).clear();
    lock(&TABLETS).clear();
    WARNED.store(false, Ordering::Relaxed);
}
