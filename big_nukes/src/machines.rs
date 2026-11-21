//! The mod's block entities: one part (`Machine`) for all of them, which hands each block to its own functions by
//! its id (the old glue's dispatch); the menus' slot rules (`Menus`); the centrifuge and the assembly table.
//!
//! A block entity type's id is its block's, as the old glue's were, so saved worlds keep them.

use crate::core::{self, argb};
use crate::{bombs, bunker, decon, events, rules, silo};
use mod_api::{
    Block, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart, ClientHost, Direction,
    DrawContext, Flow, Host, ItemStack, MenuButtonEvent, MenuContainer, MenuPart, MenuQuickMoveEvent,
    MenuSlotRulesEvent, ModBlockEntityView, ModContainer, Registrar, Screen, ScreenClickEvent, ScreenDrawEvent,
    ScreenEvent, ServerHost, Tag,
};

/// The screens' labels.
pub(crate) const LABEL: u32 = 0x40_4040;

/// The blocks with a block entity: the machines and the bombs, a Sunbreaker's structure, and the stage-2 modules'.
pub(crate) fn has_entity(block: &str) -> bool {
    matches!(block, rules::CENTRIFUGE | rules::ASSEMBLY_TABLE | rules::DECONTAMINATOR)
        || rules::BombKind::of_block(block).is_some()
        || bombs::is_part(block)
        || silo::has_entity(block)
        || bunker::has_entity(block)
        || events::has_entity(block)
}

/// A block's container: its slots, what hoppers reach, its menu.
fn container(block: &str) -> Option<ModContainer> {
    match block {
        rules::CENTRIFUGE => Some(centrifuge_container()),
        rules::ASSEMBLY_TABLE => Some(assembly_container()),
        rules::DECONTAMINATOR => decon::container(),
        block if silo::has_entity(block) => silo::container(block),
        block if bunker::has_entity(block) => bunker::container(block),
        block if bombs::is_part(block) => Some(bombs::container(rules::BombKind::Sunbreaker)),
        block => rules::BombKind::of_block(block).map(bombs::container),
    }
}

/// The menus whose slots take only some items (the old `slot_rules.rs`).
const MENUS: [&str; 5] = [rules::KEYPAD_MENU, rules::CONSOLE_MENU, rules::AIR_FILTER, rules::LAUNCH_PAD, rules::CENTRIFUGE];

/// Every block with a block entity gets its type (named as the block), the part and its container; the menus get
/// their slot rules.
pub(crate) fn register(r: &mut dyn Registrar, blocks: &[Block]) {
    for block in blocks.iter().filter(|b| has_entity(&b.id)) {
        let id = block.id.as_str();
        let kind = r.block_entity_type(id).blocks(&[id]).build();
        if let Err(e) = r.block_entity_part(&kind, Machine) {
            log::error!("[big_nukes] {id}'s block entity: {e:?}");
        }
        if let Some(c) = container(id) {
            let key = r.block_key(id);
            r.block_container(&key, c);
        }
    }
    for menu in MENUS {
        r.menu_part(menu, Menus);
    }
}

/// The part every block entity of the mod has.
pub(crate) struct Machine;

#[mod_api::horizon_part]
impl BlockPart for Machine {}

#[mod_api::horizon_part]
impl BlockEntityPart for Machine {
    /// Every block entity of the mod ticks but those of a Sunbreaker's structure, which only show its core's keypad.
    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if !bombs::is_part(event.block) {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        tick(host, event);
    }

    fn menu_button(&self, host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
        menu_button(host, event);
    }

    fn menu_quick_move(&self, host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> Flow {
        quick_move(host, event)
    }

    fn screen(&self, _: &dyn Host, event: &mut ScreenEvent<'_>) -> Option<Screen> {
        screen(event.block)
    }

    fn screen_draw(&self, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        screen_draw(host, event);
    }

    fn screen_click(&self, host: &dyn ClientHost, event: &mut ScreenClickEvent<'_>) -> Option<i32> {
        screen_click(host, event)
    }
}

/// A block entity's server tick: the machines' work, the bombs' countdowns.
fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let pos = event.pos;
    let id = event.entity.id().to_owned();
    let entity = &mut event.entity;
    match id.as_str() {
        rules::CENTRIFUGE => centrifuge_tick(host, entity),
        rules::ASSEMBLY_TABLE => assembly_refresh(host, entity),
        rules::DECONTAMINATOR => decon::tick(host, pos, entity),
        id if rules::BombKind::of_block(id).is_some() => bombs::tick(host, pos, entity),
        id if silo::has_entity(id) => silo::tick(host, pos, entity),
        id if bunker::has_entity(id) => bunker::tick(host, pos, entity),
        id if events::has_entity(id) => events::tick(host, pos),
        _ => {}
    }
}

/// A menu button pressed on the server: the assembly table's Assemble or a key of a bomb's keypad.
fn menu_button(host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
    let (pos, button) = (event.context.pos, event.context.id);
    let id = event.entity.id().to_owned();
    match id.as_str() {
        rules::ASSEMBLY_TABLE => assembly_button(host, &mut event.entity, button),
        block if bombs::keypad_bomb(block) => bombs::keypad_button(host, pos, &mut event.entity, button),
        block if bombs::is_part(block) => bombs::part_button(host, pos, &mut event.entity, button),
        _ => {}
    }
}

fn handled(done: bool) -> Flow {
    if done { Flow::Cancel } else { Flow::Continue }
}

/// A shift-click: a module's own machines decide, the rest goes the game's way.
fn quick_move(host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> Flow {
    let id = event.entity.id().to_owned();
    if bunker::has_entity(&id) {
        return handled(bunker::quick_move(host, event));
    }
    // Nothing moves into or out of a bomb's hidden slot, nor a console's, a beacon's or a Sunbreaker's block's.
    if rules::BombKind::of_block(&id).is_some() || bombs::is_part(&id) || matches!(id.as_str(), rules::LAUNCH_CONSOLE | rules::STRIKE_BEACON) {
        return Flow::Cancel;
    }
    if id == rules::LAUNCH_PAD {
        return handled(silo::pad_quick_move(host, event));
    }
    if id == rules::DECONTAMINATOR {
        return handled(decon::quick_move(host, event));
    }
    // The centrifuge: the game's own shift-click, under the menu's slot rules (what it works to its slot, fuel to its
    // slot, nothing else). A mod-api stack can't be empty, so a move of a whole stack must be the game's.
    Flow::Continue
}

/// A menu opens for a block: its screen's layout.
fn screen(block: &str) -> Option<Screen> {
    match block {
        rules::CENTRIFUGE => Some(centrifuge_screen()),
        rules::ASSEMBLY_TABLE => Some(assembly_screen()),
        rules::DECONTAMINATOR => decon::screen(),
        block if bombs::keypad_bomb(block) || bombs::is_part(block) => Some(bombs::keypad_screen()),
        rules::LAUNCH_PAD => silo::pad_screen(),
        rules::LAUNCH_CONSOLE => silo::console_screen(),
        block if bunker::has_entity(block) => bunker::screen(block),
        _ => None,
    }
}

fn screen_draw(host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let (block, data, origin) = (event.block, event.data, event.origin);
    let draw = &mut *event.draw;
    match block {
        rules::CENTRIFUGE => centrifuge_screen_draw(host, data, draw, origin),
        rules::ASSEMBLY_TABLE => assembly_screen_draw(host, data, draw, origin),
        rules::DECONTAMINATOR => decon::screen_draw(host, data, draw, origin),
        block if bombs::keypad_bomb(block) => bombs::keypad_draw(host, block, data, draw, origin),
        // A block of a Sunbreaker's structure shows its core's keypad, under the core's name.
        block if bombs::is_part(block) => bombs::keypad_draw(host, rules::SUNBREAKER_CORE, data, draw, origin),
        rules::LAUNCH_PAD => silo::pad_draw(host, draw, origin),
        rules::LAUNCH_CONSOLE => silo::console_draw(host, data, draw, origin),
        block if bunker::has_entity(block) => bunker::screen_draw(host, block, data, draw, origin),
        _ => {}
    }
}

/// A press on a machine's screen over no slot: the menu button it is on, if any.
fn screen_click(_: &dyn ClientHost, event: &mut ScreenClickEvent<'_>) -> Option<i32> {
    match event.block {
        rules::ASSEMBLY_TABLE => assembly_screen_click(event.at, event.right),
        block if bombs::keypad_bomb(block) || bombs::is_part(block) => bombs::keypad_click(event.at, event.right),
        rules::LAUNCH_CONSOLE => {
            silo::console_click(event.at, event.right);
            None
        }
        _ => None,
    }
}

// ---- the slot rules --------------------------------------------------------------

/// The menus' slots, for a player's clicks: the centrifuge takes raw uranium only in its input and anything but raw
/// uranium as fuel (what isn't fuel just doesn't burn), nothing into its output; the launch pad what it launches; a
/// bomb's keypad, a launch console and an air filter have one hidden slot that takes nothing.
pub(crate) struct Menus;

#[mod_api::horizon_part]
impl MenuPart for Menus {
    fn menu_slot_rules(&self, host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.container != MenuContainer::Main {
            return;
        }
        event.may_place = match event.menu {
            rules::KEYPAD_MENU | rules::CONSOLE_MENU | rules::AIR_FILTER => false,
            rules::LAUNCH_PAD => rules::pad_takes(event.slot, &event.stack.item().id(host)),
            rules::CENTRIFUGE => {
                let works = rules::enrichment(&event.stack.item().id(host)).is_some();
                match event.slot {
                    rules::INPUT_SLOT => works,
                    rules::FUEL_SLOT => !works && burn_time(host, event.stack) > 0,
                    _ => false,
                }
            }
            _ => event.may_place,
        };
    }
}

// ---- shared ----------------------------------------------------------------------

/// The game's burn time of `stack`, 0 for what doesn't burn.
pub(crate) fn burn_time(host: &dyn Host, stack: &ItemStack) -> i32 {
    host.item_burn_time(stack.item()).unwrap_or(0)
}

/// Whether `stack` goes onto `have`: the same item with the same components (its name, its damage...), as the game
/// stacks them. Only the count may differ.
pub(crate) fn stacks_with(have: &ItemStack, stack: &ItemStack) -> bool {
    let mut same = stack.clone();
    let _ = same.set_count(have.count());
    *have == same
}

/// A slot's item id and count, for the rules.
fn slot_of(host: &dyn Host, items: &[Option<ItemStack>], i: usize) -> Option<(String, u32)> {
    items.get(i).and_then(Option::as_ref).map(|s| (s.item().id(host), s.count()))
}

/// The most of `item` a stack holds.
fn most_of(host: &dyn Host, item: &str) -> u32 {
    core::item_key(host, item).and_then(|k| host.max_stack_size(k)).unwrap_or(64)
}

// ---- the centrifuge --------------------------------------------------------------
//
// Its state lives in its block entity, so it moves with the chunk's save: `nbt` holds the rules' `Centrifuge` as
// JSON, `items` the three slots, `data` what its screen shows.

/// A furnace's sides: hoppers put raw uranium in from above and fuel from the sides, and take enriched uranium from
/// below.
// No hook filters what a hopper puts in (the old `container_accepts`): a slot only takes from its faces, and the
// tick leaves what it can't work where it is.
fn centrifuge_container() -> ModContainer {
    let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| vec![rules::FUEL_SLOT]);
    faces[Direction::Down as usize] = vec![rules::OUTPUT_SLOT];
    faces[Direction::Up as usize] = vec![rules::INPUT_SLOT];
    ModContainer {
        slots: rules::CENTRIFUGE_SLOTS,
        faces,
        inputs: vec![rules::INPUT_SLOT, rules::FUEL_SLOT],
        outputs: vec![rules::OUTPUT_SLOT],
        menu: Some(rules::CENTRIFUGE.to_owned()),
    }
}

const CENTRIFUGE_KEY: &str = "centrifuge";

fn load(nbt: &mod_api::Compound) -> Option<rules::Centrifuge> {
    match nbt.get(&CENTRIFUGE_KEY.into()) {
        Some(Tag::ByteArray(bytes)) => serde_json::from_slice(bytes).ok(),
        _ => None,
    }
}

fn store(nbt: &mut mod_api::Compound, centrifuge: &rules::Centrifuge) {
    if let Ok(bytes) = serde_json::to_vec(centrifuge) {
        nbt.insert(CENTRIFUGE_KEY.into(), Tag::ByteArray(bytes));
    }
}

/// One fuel out of its slot; a lava bucket leaves its bucket.
fn take_fuel(items: &mut [Option<ItemStack>]) {
    let Some(stack) = items[rules::FUEL_SLOT].as_mut() else {
        return;
    };
    let lava = stack.item() == crate::keys().lava_bucket;
    let left = stack.count().saturating_sub(1);
    if left == 0 {
        items[rules::FUEL_SLOT] = lava.then(|| ItemStack::of(crate::keys().bucket));
    } else {
        let _ = stack.set_count(left);
    }
}

/// The step's uranium out of the input, one of what it makes into the output.
fn make(host: &dyn Host, items: &mut [Option<ItemStack>], step: rules::Enrichment) {
    if let Some(input) = items[rules::INPUT_SLOT].as_mut() {
        let left = input.count().saturating_sub(step.takes);
        if left == 0 {
            items[rules::INPUT_SLOT] = None;
        } else {
            let _ = input.set_count(left);
        }
    }
    match items[rules::OUTPUT_SLOT].as_mut() {
        Some(out) => {
            let _ = out.set_count(out.count() + 1);
        }
        None => items[rules::OUTPUT_SLOT] = core::stack_of(host, step.output),
    }
}

fn centrifuge_tick(host: &mut dyn ServerHost, entity: &mut ModBlockEntityView<'_>) {
    let host: &dyn ServerHost = host;
    if entity.items.len() < rules::CENTRIFUGE_SLOTS {
        entity.items.resize(rules::CENTRIFUGE_SLOTS, None);
    }
    let was = load(&entity.nbt);
    let mut centrifuge = was.unwrap_or_default();
    let fuel = entity.items[rules::FUEL_SLOT]
        .as_ref()
        .filter(|s| rules::enrichment(&s.item().id(host)).is_none())
        .map(|s| burn_time(host, s))
        .filter(|t| *t > 0)
        .map(i32::unsigned_abs);
    let most = |item: &str| most_of(host, item);
    let (input, output) = (slot_of(host, &entity.items, rules::INPUT_SLOT), slot_of(host, &entity.items, rules::OUTPUT_SLOT));
    let input = input.as_ref().map(|(id, n)| (id.as_str(), *n));
    let output = output.as_ref().map(|(id, n)| (id.as_str(), *n));
    let step = centrifuge.step(input, fuel, output, &most);
    if step.took_fuel {
        take_fuel(&mut entity.items);
    }
    if step.made {
        make(host, &mut entity.items, centrifuge.enrichment());
    }
    let data = centrifuge.data().to_vec();
    if entity.data != data {
        entity.data = data;
    }
    if was != Some(centrifuge) {
        store(&mut entity.nbt, &centrifuge);
    }
}

// ---- the centrifuge's screen -----------------------------------------------------

/// Where the screen's parts are, from its top left: tools/make_textures.py draws them in the same places. Slots are
/// the items' corners.
const INPUT_AT: [f32; 2] = [56.0, 17.0];
const FUEL_AT: [f32; 2] = [56.0, 53.0];
const OUTPUT_AT: [f32; 2] = [116.0, 35.0];
const FLAME_AT: [f32; 2] = [57.0, 37.0];
const ARROW_AT: [f32; 2] = [79.0, 34.0];
/// The lit flame and the full arrow, right of the panel in the texture.
const LIT_FLAME: [f32; 2] = [176.0, 0.0];
const FULL_ARROW: [f32; 2] = [176.0, 14.0];
const ARROW_HEIGHT: f32 = 17.0;
const GUI: &str = "big_nukes:gui/centrifuge";

fn centrifuge_screen() -> Screen {
    Screen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some(INPUT_AT), Some(FUEL_AT), Some(OUTPUT_AT)],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: true,
        recipe_book: None,
        recipe_book_toggle: None,
    }
}

fn centrifuge_screen_draw(host: &dyn ClientHost, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    let [x, y] = origin;
    draw.sprite(GUI, [0.0, 0.0, 176.0, 166.0], [x, y, 176.0, 166.0], None);
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    // The flame burns down from its top, the arrow fills from its left.
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
    let arrow = rules::arrow_width(get(2), get(3)) as f32;
    if arrow > 0.0 {
        draw.sprite(
            GUI,
            [FULL_ARROW[0], FULL_ARROW[1], arrow, ARROW_HEIGHT],
            [x + ARROW_AT[0], y + ARROW_AT[1], arrow, ARROW_HEIGHT],
            None,
        );
    }
    let title = core::lang(host, "big_nukes.screen.centrifuge");
    let width = draw.text_width(&title);
    draw.text(&title, [x + (176.0 - width) / 2.0, y + 6.0], argb(LABEL), false);
    draw.text(&core::lang(host, "container.inventory"), [x + 8.0, y + 72.0], argb(LABEL), false);
}

// ---- the assembly table ----------------------------------------------------------
//
// Nothing of its own but its slots: the grid's recipe is read again each tick, for the screen's data slot, and the
// button makes one of it.

/// Hoppers take the made items from below and put nothing in.
fn assembly_container() -> ModContainer {
    let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| Vec::new());
    faces[Direction::Down as usize] = vec![rules::ASSEMBLY_OUTPUT_SLOT];
    ModContainer {
        slots: rules::ASSEMBLY_SLOTS,
        faces,
        inputs: Vec::new(),
        outputs: vec![rules::ASSEMBLY_OUTPUT_SLOT],
        menu: Some(rules::ASSEMBLY_TABLE.to_owned()),
    }
}

/// The grid's items and counts, for the rules.
fn assembly_grid(host: &dyn Host, items: &[Option<ItemStack>]) -> Vec<Option<(String, u32)>> {
    (0..rules::GRID_SLOTS).map(|i| slot_of(host, items, i)).collect()
}

fn grid_ref(grid: &[Option<(String, u32)>]) -> Vec<Option<(&str, u32)>> {
    grid.iter().map(|s| s.as_ref().map(|(id, n)| (id.as_str(), *n))).collect()
}

/// The grid's recipe, as the screen's data slot: read again each tick.
fn assembly_refresh(host: &dyn Host, entity: &mut ModBlockEntityView<'_>) {
    let grid = assembly_grid(host, &entity.items);
    let data = vec![rules::assembly_data(&grid_ref(&grid))];
    if entity.data != data {
        entity.data = data;
    }
}

/// Assemble: the recipe's parts out of the grid, its result into the output. Nothing happens without a recipe in
/// the grid or room in the output.
fn assembly_button(host: &mut dyn ServerHost, entity: &mut ModBlockEntityView<'_>, id: i32) {
    let host: &dyn ServerHost = host;
    if id != rules::ASSEMBLE_BUTTON {
        return;
    }
    let grid = assembly_grid(host, &entity.items);
    let output = slot_of(host, &entity.items, rules::ASSEMBLY_OUTPUT_SLOT);
    let most = |item: &str| most_of(host, item);
    let Some(done) = rules::assemble(&grid_ref(&grid), output.as_ref().map(|(id, n)| (id.as_str(), *n)), &most) else {
        return;
    };
    if entity.items.len() < rules::ASSEMBLY_SLOTS {
        entity.items.resize(rules::ASSEMBLY_SLOTS, None);
    }
    for (slot, n) in done.take {
        let Some(stack) = entity.items[slot].as_mut() else { continue };
        let left = stack.count().saturating_sub(n);
        if left == 0 {
            // A water bucket leaves its bucket, as on a crafting table.
            let id = stack.item().id(host);
            entity.items[slot] = rules::remainder(&id).and_then(|left| core::stack_of_n(host, left, n));
        } else {
            let _ = stack.set_count(left);
        }
    }
    match entity.items[rules::ASSEMBLY_OUTPUT_SLOT].as_mut() {
        Some(out) => {
            let _ = out.set_count(out.count() + done.count);
        }
        None => entity.items[rules::ASSEMBLY_OUTPUT_SLOT] = core::stack_of_n(host, done.result, done.count),
    }
    assembly_refresh(host, entity);
}

// ---- the assembly table's screen -------------------------------------------------
//
// The game draws the panel and the slots' frames; the mod draws the labels, the line of what the grid makes and
// the button.

const ASSEMBLY_WIDTH: f32 = 176.0;
const ASSEMBLY_HEIGHT: f32 = 180.0;
/// The grid's top left slot; the others are 18 pixels apart.
const GRID_AT: [f32; 2] = [30.0, 17.0];
const ASSEMBLY_OUTPUT_AT: [f32; 2] = [124.0, 21.0];
/// The button: x, y, width, height.
const ASSEMBLE_AT: [f32; 4] = [96.0, 47.0, 72.0, 18.0];
const ASSEMBLY_LINE_Y: f32 = 76.0;
const ASSEMBLY_INVENTORY_Y: f32 = 98.0;

fn assembly_screen() -> Screen {
    let mut slots: Vec<Option<[f32; 2]>> = (0..rules::GRID_SLOTS)
        .map(|i| Some([GRID_AT[0] + 18.0 * (i % 3) as f32, GRID_AT[1] + 18.0 * (i / 3) as f32]))
        .collect();
    slots.push(Some(ASSEMBLY_OUTPUT_AT));
    Screen {
        width: ASSEMBLY_WIDTH,
        height: ASSEMBLY_HEIGHT,
        slots,
        inventory_y: ASSEMBLY_INVENTORY_Y,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    }
}

fn assembly_screen_draw(host: &dyn ClientHost, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    let [x, y] = origin;
    let title = core::lang(host, "big_nukes.screen.assembly_table");
    let width = draw.text_width(&title);
    draw.text(&title, [x + (ASSEMBLY_WIDTH - width) / 2.0, y + 6.0], argb(LABEL), false);
    draw.text(&core::lang(host, "container.inventory"), [x + 8.0, y + ASSEMBLY_INVENTORY_Y - 11.0], argb(LABEL), false);
    let recipe = rules::assembly_of_data(data.first().copied().unwrap_or(0));
    let line = match recipe {
        Some(r) if r.count > 1 => {
            let made = format!("{} x{}", core::lang(host, &rules::name_key(r.result)), r.count);
            core::lang_tr(host, "big_nukes.screen.assembly_makes", &[&made])
        }
        Some(r) => core::lang_tr(host, "big_nukes.screen.assembly_makes", &[&core::lang(host, &rules::name_key(r.result))]),
        None => core::lang(host, "big_nukes.screen.assembly_hint"),
    };
    let width = draw.text_width(&line);
    draw.text(&line, [x + (ASSEMBLY_WIDTH - width) / 2.0, y + ASSEMBLY_LINE_Y], argb(LABEL), false);
    // A plain button, lit while the grid holds a recipe.
    let [bx, by, bw, bh] = ASSEMBLE_AT;
    let (body, text) = if recipe.is_some() { (0xFF8B_8B8B, 0xFF_FFFF) } else { (0xFF5A_5A5A, 0xA0_A0A0) };
    draw.fill([x + bx, y + by, bw, bh], 0xFF00_0000);
    draw.fill([x + bx + 1.0, y + by + 1.0, bw - 2.0, bh - 2.0], 0xFFC6_C6C6);
    draw.fill([x + bx + 2.0, y + by + 2.0, bw - 3.0, bh - 3.0], body);
    let label = core::lang(host, "big_nukes.screen.assemble");
    let width = draw.text_width(&label);
    draw.text(&label, [x + bx + (bw - width) / 2.0, y + by + (bh - 8.0) / 2.0], argb(text), true);
}

fn assembly_screen_click(at: [f32; 2], right: bool) -> Option<i32> {
    let [bx, by, bw, bh] = ASSEMBLE_AT;
    (!right && (bx..bx + bw).contains(&at[0]) && (by..by + bh).contains(&at[1])).then_some(rules::ASSEMBLE_BUTTON)
}
