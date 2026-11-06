// Big Nukes' rules: no game type here, so they build and test on their own.
// The bundle adds this file to the game as `game::big_nukes`; the glue
// (`game::big_nukes_glue`, glue.rs) connects it to the game.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::Duration,
};

pub const ID: &str = "big_nukes_native";
pub const VERSION: &str = "0.1.0";
/// The mod's folder in a world's folder.
pub const DIR: &str = "horizon-big-nukes";
/// A loot table only the assets half brings: the sign that it's installed.
/// The damage radiation does and the effect that marks the sickness' stage: the assets half's
/// `data/big_nukes/damage_type/radiation.json` and `horizon/mob_effect/radiation.json`.
pub const RADIATION: &str = "big_nukes:radiation";
pub const ASSETS_MARKER: &str = "big_nukes:blocks/atomic_bomb";

// ---- what the mod adds ----------------------------------------------------------

pub const CENTRIFUGE: &str = "big_nukes:centrifuge";
pub const ASSEMBLY_TABLE: &str = "big_nukes:assembly_table";

/// The bombs, smallest first. A bomb's saved state names its kind; one saved
/// without it is read as the smallest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BombKind {
    #[default]
    Demolition,
    Tactical,
    Atomic,
}

impl BombKind {
    pub const ALL: [BombKind; 3] = [BombKind::Demolition, BombKind::Tactical, BombKind::Atomic];

    pub fn id(self) -> &'static str {
        match self {
            BombKind::Demolition => "big_nukes:demolition_charge",
            BombKind::Tactical => "big_nukes:tactical_warhead",
            BombKind::Atomic => "big_nukes:atomic_bomb",
        }
    }

    /// How far the blast reaches, before the operator's limit.
    pub fn radius(self) -> u32 {
        match self {
            BombKind::Demolition => 16,
            BombKind::Tactical => 50,
            BombKind::Atomic => 250,
        }
    }

    /// The bomb a block is, if it is one.
    pub fn of_block(id: &str) -> Option<BombKind> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// The demolition charge takes only a fuse; the others take any detonator.
    pub fn takes(self, detonator: Detonator) -> bool {
        self != BombKind::Demolition || detonator == Detonator::Fuse
    }
}

/// The items: id, how many stack, and whether they are rare. Each copies its
/// `template` with the `components` patch.
pub const ITEMS: [(&str, u8, bool); 14] = [
    ("big_nukes:raw_uranium", 64, false),
    ("big_nukes:enriched_uranium", 64, true),
    ("big_nukes:nuclear_charge", 16, true),
    ("big_nukes:bomb_casing", 16, false),
    ("big_nukes:fuse_detonator", 16, false),
    ("big_nukes:keypad_detonator", 16, false),
    ("big_nukes:receiver_detonator", 16, false),
    ("big_nukes:remote", 1, true),
    (HAZMAT[0].0, 1, false),
    (HAZMAT[1].0, 1, false),
    (HAZMAT[2].0, 1, false),
    (HAZMAT[3].0, 1, false),
    (ANTI_RAD, 16, false),
    (GEIGER_COUNTER, 1, false),
];

/// The hazmat suit: each piece's id, the slot it is worn in, and the vanilla
/// armour it copies.
pub const HAZMAT: [(&str, &str, &str); 4] = [
    ("big_nukes:hazmat_helmet", "head", "minecraft:iron_helmet"),
    ("big_nukes:hazmat_chestplate", "chest", "minecraft:iron_chestplate"),
    ("big_nukes:hazmat_leggings", "legs", "minecraft:iron_leggings"),
    ("big_nukes:hazmat_boots", "feet", "minecraft:iron_boots"),
];
/// The suit's look on a player: `assets/big_nukes/equipment/hazmat.json`.
pub const HAZMAT_ASSET: &str = "big_nukes:hazmat";
pub const ANTI_RAD: &str = "big_nukes:anti_rad";
/// The anti-rad's consume effect, which `native_consume_effect` hears.
pub const ANTI_RADIATION: &str = "big_nukes:anti_radiation";
pub const GEIGER_COUNTER: &str = "big_nukes:geiger_counter";

/// How many rad a drink of anti-rad takes off.
const ANTI_RAD_DOSE: f64 = 300.0;
/// How much wear a piece of the suit takes, and what the dose's wear counts when the game doesn't tell it.
pub const HAZMAT_DURABILITY: u32 = 600;

/// The vanilla item an item copies: a suit piece its iron armour, the rest paper.
pub fn template(id: &str) -> &'static str {
    HAZMAT.iter().find(|p| p.0 == id).map_or("minecraft:paper", |p| p.2)
}

/// An item's patch over its template's components; a patched component
/// replaces the template's whole one.
pub fn components(id: &str) -> BTreeMap<String, serde_json::Value> {
    use serde_json::json;
    let mut c = BTreeMap::new();
    if let Some(&(id, slot, _)) = HAZMAT.iter().find(|p| p.0 == id) {
        // `helmet`, `chestplate`...: vanilla names the armour's modifier after its piece.
        let piece = id.rsplit('_').next().unwrap_or(id);
        c.insert(
            "minecraft:equippable".into(),
            json!({"slot": slot, "asset_id": HAZMAT_ASSET, "equip_sound": "minecraft:item.armor.equip_leather"}),
        );
        c.insert("minecraft:max_damage".into(), json!(HAZMAT_DURABILITY));
        // Iron mends iron armour; the suit is only made anew.
        c.insert("!minecraft:repairable".into(), json!({}));
        c.insert(
            "minecraft:attribute_modifiers".into(),
            json!([{"type": "minecraft:armor", "amount": 1.0, "operation": "add_value",
                "id": format!("minecraft:armor.{piece}"), "slot": slot}]),
        );
    } else if id == ANTI_RAD {
        c.insert(
            "minecraft:consumable".into(),
            json!({"consume_seconds": 1.6, "animation": "drink", "sound": "minecraft:entity.generic.drink",
                "has_consume_particles": false,
                "on_consume_effects": [{"type": ANTI_RADIATION, "amount": ANTI_RAD_DOSE}]}),
        );
        c.insert("minecraft:use_remainder".into(), json!({"id": "minecraft:glass_bottle", "count": 1}));
    }
    c
}

/// How many rad an anti-rad effect takes off, from the JSON its item declares:
/// none unless it is a positive number.
pub fn anti_rad_amount(effect: &serde_json::Value) -> Option<f64> {
    effect["amount"].as_f64().filter(|a| a.is_finite() && *a > 0.0)
}

// ---- the centrifuge -------------------------------------------------------------

pub const RAW_URANIUM: &str = "big_nukes:raw_uranium";
pub const ENRICHED_URANIUM: &str = "big_nukes:enriched_uranium";
/// Its slots: raw uranium in, fuel, enriched uranium out. Its menu type is
/// content/data/big_nukes/horizon/menu/centrifuge.json, named as the block.
pub const INPUT_SLOT: usize = 0;
pub const FUEL_SLOT: usize = 1;
pub const OUTPUT_SLOT: usize = 2;
pub const CENTRIFUGE_SLOTS: usize = 3;
/// Ticks of work that turn one raw uranium into one enriched.
pub const SPIN_TICKS: u32 = 400;
/// The screen's flame is this tall and its arrow this wide, in GUI pixels.
pub const FLAME_HEIGHT: i32 = 14;
pub const ARROW_WIDTH: i32 = 24;

/// A centrifuge's state, kept in its block entity: the ticks left of the fuel
/// it burns and that fuel's whole burn, and the ticks of work on the current input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Centrifuge {
    pub burn: u32,
    pub burn_max: u32,
    pub spin: u32,
}

/// What a tick asks of the slots: one fuel taken, one input made into one output.
/// Both can happen in one tick, so they are two flags, not one variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Step {
    pub took_fuel: bool,
    pub made: bool,
}

impl Step {
    pub const IDLE: Step = Step { took_fuel: false, made: false };
}

impl Centrifuge {
    /// One tick. `input` is the raw uranium in its slot, `fuel` the burn ticks of
    /// the fuel in its slot (none, or 0, when nothing there burns), `output_room`
    /// whether the output slot takes one more enriched uranium.
    ///
    /// The fire burns only while it spins: an empty input or a full output keeps
    /// the rest of the burning fuel, and takes no new fuel. Taking the input out
    /// loses the progress, as a furnace does.
    pub fn step(&mut self, input: u32, fuel: Option<u32>, output_room: bool) -> Step {
        if input == 0 {
            self.spin = 0;
            return Step::IDLE;
        }
        if !output_room {
            return Step::IDLE;
        }
        let mut step = Step::IDLE;
        if self.burn == 0 {
            match fuel {
                Some(ticks) if ticks > 0 => {
                    self.burn = ticks;
                    self.burn_max = ticks;
                    step.took_fuel = true;
                }
                _ => return Step::IDLE,
            }
        }
        self.burn -= 1;
        self.spin += 1;
        if self.spin >= SPIN_TICKS {
            self.spin = 0;
            step.made = true;
        }
        step
    }

    /// The menu's data slots: `[burn, burn_max, spin, SPIN_TICKS]`, kept within
    /// the 16 bits a data slot has on the wire.
    pub fn data(&self) -> [i32; 4] {
        let short = |v: u32| v.min(i16::MAX as u32) as i32;
        [short(self.burn), short(self.burn_max), short(self.spin), short(SPIN_TICKS)]
    }
}

/// The lit part of the screen's flame, from the bottom: at least 1 while anything burns.
pub fn flame_height(burn: i32, burn_max: i32) -> i32 {
    if burn <= 0 || burn_max <= 0 {
        return 0;
    }
    (burn.min(burn_max) * FLAME_HEIGHT / burn_max).max(1)
}

/// The filled part of the screen's arrow, from the left.
pub fn arrow_width(spin: i32, spin_max: i32) -> i32 {
    if spin <= 0 || spin_max <= 0 {
        return 0;
    }
    spin.min(spin_max) * ARROW_WIDTH / spin_max
}

// ---- the assembly table -----------------------------------------------------------
//
// The bombs and their parts are made here, not on a crafting table: their parts
// go anywhere in a 3×3 grid, and the Assemble button makes one of the recipe
// whose items the grid holds. Its menu type is
// content/data/big_nukes/horizon/menu/assembly_table.json, named as the block.

pub const GRID_SLOTS: usize = 9;
pub const ASSEMBLY_OUTPUT_SLOT: usize = 9;
pub const ASSEMBLY_SLOTS: usize = 10;
/// The menu button id of Assemble.
pub const ASSEMBLE_BUTTON: i32 = 0;

/// A recipe of the assembly table: its result, and how many of each item it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assembly {
    pub result: &'static str,
    pub parts: &'static [(&'static str, u32)],
}

pub const ASSEMBLIES: [Assembly; 5] = [
    Assembly { result: "big_nukes:nuclear_charge", parts: &[(ENRICHED_URANIUM, 1), ("minecraft:tnt", 4)] },
    Assembly { result: "big_nukes:bomb_casing", parts: &[("minecraft:iron_block", 4), ("minecraft:redstone", 4)] },
    Assembly {
        result: "big_nukes:demolition_charge",
        parts: &[("minecraft:tnt", 4), ("minecraft:iron_ingot", 4), ("minecraft:redstone", 1)],
    },
    Assembly {
        result: "big_nukes:tactical_warhead",
        parts: &[
            ("big_nukes:nuclear_charge", 1),
            ("big_nukes:bomb_casing", 1),
            ("minecraft:iron_ingot", 6),
            ("minecraft:redstone", 1),
        ],
    },
    Assembly {
        result: "big_nukes:atomic_bomb",
        parts: &[("big_nukes:nuclear_charge", 4), ("big_nukes:bomb_casing", 4), ("minecraft:redstone_block", 1)],
    },
];

/// What Assemble does: the result to add to the output, and how many to take
/// from which grid slots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assembled {
    pub result: &'static str,
    pub take: Vec<(usize, u32)>,
}

/// The recipe whose items the grid holds, enough of each and nothing else: its
/// index in `ASSEMBLIES`. The grid is read as a total per item, wherever it lies.
pub fn assembly_in(grid: &[Option<(&str, u32)>]) -> Option<usize> {
    let mut totals: BTreeMap<&str, u32> = BTreeMap::new();
    for &(item, count) in grid.iter().flatten().filter(|s| s.1 > 0) {
        *totals.entry(item).or_default() += count;
    }
    ASSEMBLIES.iter().position(|a| {
        a.parts.len() == totals.len() && a.parts.iter().all(|(id, need)| totals.get(id).is_some_and(|have| have >= need))
    })
}

/// Assemble pressed: what to take and make, or None when the grid holds no
/// recipe or the output has no room for one more of its result.
pub fn assemble(grid: &[Option<(&str, u32)>], output: Option<(&str, u32)>, max_stack: &dyn Fn(&str) -> u32) -> Option<Assembled> {
    let recipe = &ASSEMBLIES[assembly_in(grid)?];
    let room = match output {
        None => true,
        Some((item, count)) => item == recipe.result && count < max_stack(recipe.result),
    };
    if !room {
        return None;
    }
    let mut take = Vec::new();
    for &(id, need) in recipe.parts {
        let mut left = need;
        for (slot, stack) in grid.iter().enumerate() {
            if left == 0 {
                break;
            }
            if let Some((item, count)) = *stack
                && item == id
                && count > 0
            {
                let n = count.min(left);
                take.push((slot, n));
                left -= n;
            }
        }
    }
    Some(Assembled { result: recipe.result, take })
}

/// The screen's data slot: 0 for no recipe, else the recipe's index plus one.
pub fn assembly_data(grid: &[Option<(&str, u32)>]) -> i32 {
    assembly_in(grid).map_or(0, |i| i as i32 + 1)
}

/// The recipe a data slot names, as `assembly_data` wrote it.
pub fn assembly_of_data(data: i32) -> Option<&'static Assembly> {
    usize::try_from(data).ok()?.checked_sub(1).and_then(|i| ASSEMBLIES.get(i))
}

/// The language key of a block's or an item's name: the bombs are blocks.
pub fn name_key(id: &str) -> String {
    let (namespace, name) = id.split_once(':').unwrap_or(("minecraft", id));
    let kind = if BombKind::ALL.iter().any(|k| k.id() == id) { "block" } else { "item" };
    format!("{kind}.{namespace}.{name}")
}

// ---- the bombs -------------------------------------------------------------------
//
// A bomb's state lives in its block entity, as the centrifuge's does. It goes
// off only through its detonator: a fuse lit with flint and steel, a keypad's
// code, or a receiver's remote. The fuse and the keypad count to a tick of
// `Saved.tick`, and the block entity's tick sets the blast off; a remote sets
// it off at once, from the world's record of the bomb (`Saved.bombs`).

pub const FLINT_AND_STEEL: &str = "minecraft:flint_and_steel";
/// The uses in a flint and steel, vanilla's `max_damage`.
pub const FLINT_AND_STEEL_USES: u32 = 64;
/// A lit fuse hisses this long before the blast.
pub const FUSE_TICKS: u64 = 80;
/// The right code starts a countdown this long.
pub const COUNTDOWN_TICKS: u64 = 100;
/// Wrong codes in a row that lock the keypad, and for how long.
pub const TRIES: u8 = 3;
pub const LOCK_TICKS: u64 = 600;
/// The longest code.
pub const CODE_DIGITS: usize = 8;
/// The shortest code a keypad takes, so it can't be guessed in a few tries.
pub const CODE_MIN_DIGITS: usize = 3;
/// The keypad's menu type, content/data/big_nukes/horizon/menu/keypad.json.
/// Its one slot holds the detonator, hidden, so a broken bomb drops it as a
/// container drops its items.
pub const KEYPAD_MENU: &str = "big_nukes:keypad";
pub const BOMB_SLOTS: usize = 1;
pub const DETONATOR_SLOT: usize = 0;
/// The keypad screen's data slots: what it shows, the digits typed, the seconds
/// of a lock or a countdown, and the tries left.
pub const PAD_DATA: usize = 4;
pub const PAD_SET: i32 = 0;
pub const PAD_ENTER: i32 = 1;
pub const PAD_LOCKED: i32 = 2;
pub const PAD_COUNTING: i32 = 3;

/// What sets a bomb off, put in as an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Detonator {
    Fuse,
    Keypad,
    Receiver,
}

impl Detonator {
    pub const ALL: [Detonator; 3] = [Detonator::Fuse, Detonator::Keypad, Detonator::Receiver];

    pub fn item(self) -> &'static str {
        match self {
            Detonator::Fuse => "big_nukes:fuse_detonator",
            Detonator::Keypad => "big_nukes:keypad_detonator",
            Detonator::Receiver => "big_nukes:receiver_detonator",
        }
    }

    pub fn of_item(item: &str) -> Option<Detonator> {
        Self::ALL.into_iter().find(|d| d.item() == item)
    }
}

/// A flint and steel's damage after one more use; none when that use breaks it.
pub fn worn(damage: u32) -> Option<u32> {
    let damage = damage + 1;
    (damage < FLINT_AND_STEEL_USES).then_some(damage)
}

/// A placed bomb: its detonator, and the keypad's code and what was typed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bomb {
    pub kind: BombKind,
    pub detonator: Option<Detonator>,
    /// The keypad's code, its digits; empty until the first entry sets it.
    pub code: String,
    /// The digits typed since the last enter or clear.
    pub entry: String,
    /// Wrong codes in a row.
    pub tries: u8,
    /// The keypad takes nothing before this tick.
    pub locked_until: u64,
    /// The tick it goes off, once its fuse is lit or its code entered.
    pub fires_at: Option<u64>,
}

/// Why a detonator doesn't go in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    HasOne,
    FuseOnly,
}

/// A key of the keypad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Digit(u8),
    Clear,
    Enter,
}

impl Key {
    /// The key a menu button stands for: the digits 0 to 9, then clear and enter.
    pub fn of_button(id: i32) -> Option<Key> {
        match id {
            0..=9 => Some(Key::Digit(id as u8)),
            10 => Some(Key::Clear),
            11 => Some(Key::Enter),
            _ => None,
        }
    }
}

/// What a key press did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Ignored,
    Typed,
    Cleared,
    CodeSet,
    /// A new code shorter than `CODE_MIN_DIGITS`: its digits stay, to go on typing.
    TooShort,
    Accepted,
    Wrong,
    Locked,
}

impl Bomb {
    pub fn new(kind: BombKind) -> Bomb {
        Bomb { kind, ..Bomb::default() }
    }

    /// Puts a detonator in, the only one it gets.
    pub fn install(&mut self, detonator: Detonator) -> Result<(), Refusal> {
        if self.detonator.is_some() {
            return Err(Refusal::HasOne);
        }
        if !self.kind.takes(detonator) {
            return Err(Refusal::FuseOnly);
        }
        self.detonator = Some(detonator);
        Ok(())
    }

    /// Starts the count to the blast, `ticks` from `now`; false when it already
    /// counts. The fuse and the keypad come here.
    pub fn arm(&mut self, now: u64, ticks: u64) -> bool {
        if self.fires_at.is_some() {
            return false;
        }
        self.fires_at = Some(now + ticks);
        true
    }

    /// Lights the fuse: false without one, or when it's already lit.
    pub fn light(&mut self, now: u64) -> bool {
        self.detonator == Some(Detonator::Fuse) && self.arm(now, FUSE_TICKS)
    }

    /// A key of the keypad. The first code entered becomes the code; then the
    /// right one starts the countdown, and `TRIES` wrong ones in a row lock the
    /// keypad. Nothing is taken while it's locked or counting.
    pub fn press(&mut self, key: Key, now: u64) -> Press {
        if self.detonator != Some(Detonator::Keypad) || self.fires_at.is_some() || now < self.locked_until {
            return Press::Ignored;
        }
        match key {
            Key::Digit(d) if d <= 9 && self.entry.len() < CODE_DIGITS => {
                self.entry.push(char::from(b'0' + d));
                Press::Typed
            }
            Key::Digit(_) => Press::Ignored,
            Key::Clear => {
                self.entry.clear();
                Press::Cleared
            }
            Key::Enter if self.entry.is_empty() => Press::Ignored,
            Key::Enter => {
                let entry = std::mem::take(&mut self.entry);
                if self.code.is_empty() && entry.len() < CODE_MIN_DIGITS {
                    self.entry = entry;
                    Press::TooShort
                } else if self.code.is_empty() {
                    self.code = entry;
                    Press::CodeSet
                } else if entry == self.code {
                    self.tries = 0;
                    self.arm(now, COUNTDOWN_TICKS);
                    Press::Accepted
                } else {
                    self.tries += 1;
                    if self.tries < TRIES {
                        return Press::Wrong;
                    }
                    self.tries = 0;
                    self.locked_until = now + LOCK_TICKS;
                    Press::Locked
                }
            }
        }
    }

    /// The ticks left before it goes off, while it counts.
    pub fn left(&self, now: u64) -> Option<u64> {
        self.fires_at.map(|at| at.saturating_sub(now))
    }

    /// A tick: true once, when it goes off. A lock or a count read from a
    /// world whose clock went back is cut to its own length.
    pub fn tick(&mut self, now: u64) -> bool {
        self.locked_until = self.locked_until.min(now + LOCK_TICKS);
        self.fires_at = self.fires_at.map(|at| at.min(now + FUSE_TICKS.max(COUNTDOWN_TICKS)));
        match self.fires_at {
            Some(at) if at <= now => {
                self.fires_at = None;
                true
            }
            _ => false,
        }
    }

    /// The keypad screen's data slots (`PAD_DATA` of them).
    pub fn data(&self, now: u64) -> [i32; PAD_DATA] {
        let seconds = |ticks: u64| ticks.div_ceil(20).min(i16::MAX as u64) as i32;
        let tries_left = i32::from(TRIES.saturating_sub(self.tries));
        if let Some(left) = self.left(now) {
            [PAD_COUNTING, 0, seconds(left), tries_left]
        } else if now < self.locked_until {
            [PAD_LOCKED, 0, seconds(self.locked_until - now), tries_left]
        } else if self.code.is_empty() {
            [PAD_SET, self.entry.len() as i32, 0, tries_left]
        } else {
            [PAD_ENTER, self.entry.len() as i32, 0, tries_left]
        }
    }
}

// ---- blast geometry ----------------------------------------------------------
//
// A blast leaves zones by how far a column is from the center, measured against
// an uneven border: the core is a crater, emptied up to the sky; the middle
// loses its plants, its glass and its water and is scorched; the outer ring
// loses glass and, less and less outward, its leaves and plants.

/// A dimension's height: its lowest block and how many there are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub min_y: i32,
    pub height: i32,
    /// Where a blast stops going up: the world's top, or under the Nether's bedrock roof.
    pub roof: i32,
}

impl World {
    pub const OVERWORLD: World = World { min_y: -64, height: 384, roof: 320 };
    pub const NETHER: World = World { min_y: 0, height: 256, roof: 123 };
    pub const END: World = World { min_y: 0, height: 256, roof: 256 };

    /// The first height above the world.
    pub fn top(&self) -> i32 {
        self.min_y + self.height
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blast {
    pub id: u64,
    pub dimension: String,
    pub center: [i32; 3],
    pub radius: u32,
    pub seed: u64,
    pub world: World,
}

pub fn chunk_of(x: i32, z: i32) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

/// How much the border swings in and out of the radius.
const BORDER: f64 = 0.12;
/// The zones' outer edges, as a share of the border's distance.
const CORE: f64 = 0.35;
const MIDDLE: f64 = 0.7;
/// The crater's depth at the center, as a share of the radius.
const CRATER_DEPTH: f64 = 0.25;
/// Bedrock's layers stay: the crater stops this far above the world's floor.
const ABOVE_FLOOR: i32 = 5;
/// How far below the center the middle zone looks for plants.
const PLANTS_BELOW: i32 = 48;
/// The share of scorched columns that catch fire.
const FIRE: f64 = 0.03;

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// A number in [0, 1) for a column, the same every time.
fn chance(seed: u64, x: i32, z: i32, salt: u64) -> f64 {
    let h = mix(seed ^ mix((x as u32 as u64) << 32 | z as u32 as u64) ^ salt);
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// A smooth swing in [-1, 1] around the circle: four harmonics with phases
/// drawn from the seed.
fn swing(seed: u64, angle: f64) -> f64 {
    [(2.0, 0.4), (3.0, 0.3), (5.0, 0.2), (7.0, 0.1)]
        .iter()
        .enumerate()
        .map(|(i, (k, a))| {
            let phase = (mix(seed ^ (i as u64 + 1)) >> 11) as f64 / (1u64 << 53) as f64 * std::f64::consts::TAU;
            a * (k * angle + phase).sin()
        })
        .sum()
}

/// How far a column is from the center: 0 there, 1 on the border.
pub fn edge(blast: &Blast, x: i32, z: i32) -> f64 {
    let (dx, dz) = ((x - blast.center[0]) as f64, (z - blast.center[2]) as f64);
    let dist = dx.hypot(dz);
    if dist == 0.0 {
        return 0.0;
    }
    let border = blast.radius as f64 * (1.0 + BORDER * swing(blast.seed, dz.atan2(dx)));
    dist / border
}

/// The rings of a blast's damage, by a column's share of the border's distance (`edge`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ring {
    Core,
    Middle,
    Outer,
    Untouched,
}

pub fn ring(d: f64) -> Ring {
    if d < CORE {
        Ring::Core
    } else if d < MIDDLE {
        Ring::Middle
    } else if d < 1.0 {
        Ring::Outer
    } else {
        Ring::Untouched
    }
}

/// The crater's floor under a column at distance `d`: a bowl, deepest at the
/// center, never into bedrock.
pub fn crater_floor(blast: &Blast, d: f64) -> i32 {
    let depth = if d < CORE { CRATER_DEPTH * blast.radius as f64 * (1.0 - (d / CORE).powi(2)) } else { 0.0 };
    (blast.center[1] - depth as i32).max(blast.world.min_y + ABOVE_FLOOR)
}

/// A heightmap a run's `y` counts from, as the game's `NativeHeightmap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    WorldSurface,
    MotionBlockingNoLeaves,
}

/// `height` blocks up column (`x`, `z`) from `y`, set to palette entry `block`
/// where filter `only` passes: the game's `NativeBlockRun`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub x: i32,
    pub z: i32,
    pub y: i32,
    pub height: u32,
    pub surface: Option<Surface>,
    pub block: u32,
    pub only: Option<u32>,
}

/// One `native_edit_blocks` call.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChunkEdit {
    pub palette: Vec<String>,
    pub filters: Vec<String>,
    pub runs: Vec<Run>,
}

/// The ash the wave leaves on scorched ground: a block of the assets half.
pub const ASH: &str = "big_nukes:ash";
/// The ground scorched without the assets half, which the ash is a copy of.
const ASH_STAND_IN: &str = "minecraft:coarse_dirt";

impl ChunkEdit {
    /// The edit for a game without the ash block: coarse dirt in its place.
    pub fn without_ash(mut self) -> Self {
        for p in self.palette.iter_mut().filter(|p| *p == ASH) {
            *p = ASH_STAND_IN.to_string();
        }
        self
    }
}

const PALETTE: [&str; 3] = ["minecraft:air", ASH, "minecraft:fire"];
const AIR: u32 = 0;
const SCORCHED: u32 = 1;
const FLAME: u32 = 2;
const FILTERS: [&str; 14] = [
    "#minecraft:leaves",
    "#minecraft:logs",
    "#minecraft:impermeable",
    "#minecraft:substrate_overworld",
    "minecraft:water",
    "minecraft:air",
    "#minecraft:flowers",
    "#minecraft:saplings",
    "minecraft:short_grass",
    "minecraft:tall_grass",
    "minecraft:fern",
    "minecraft:large_fern",
    "minecraft:dead_bush",
    "minecraft:vine",
];
const LEAVES: u32 = 0;
const LOGS: u32 = 1;
const GLASS: u32 = 2;
const GROUND: u32 = 3;
const WATER: u32 = 4;
const EMPTY: u32 = 5;
/// Plants, named one by one: `#minecraft:replaceable` would take water and lava too.
const PLANTS: std::ops::Range<u32> = 6..14;

fn edit() -> ChunkEdit {
    ChunkEdit {
        palette: PALETTE.iter().map(|s| s.to_string()).collect(),
        filters: FILTERS.iter().map(|s| s.to_string()).collect(),
        runs: vec![],
    }
}

/// What a blast does to one chunk: two edits, made in order. The first takes the
/// plants and the crater; the second scorches what is then the surface, as a
/// run's heightmap is read once, before its edit changes anything.
pub fn plan_chunk(blast: &Blast, chunk: [i32; 2]) -> [ChunkEdit; 2] {
    let (mut plants, mut surface) = (edit(), edit());
    let top = blast.world.roof.min(blast.world.top());
    let from = |y: i32| y.clamp(blast.world.min_y, top);
    for x in chunk[0] * 16..chunk[0] * 16 + 16 {
        for z in chunk[1] * 16..chunk[1] * 16 + 16 {
            let d = edge(blast, x, z);
            let absolute = |lo: i32, block: u32, only: Option<u32>| {
                let y = from(lo);
                (y < top).then(|| Run { x, z, y, height: (top - y) as u32, surface: None, block, only })
            };
            let below = blast.center[1] - PLANTS_BELOW;
            match ring(d) {
                Ring::Core => plants.runs.extend(absolute(crater_floor(blast, d), AIR, None)),
                Ring::Middle => {
                    for f in [LEAVES, LOGS, GLASS].into_iter().chain(PLANTS) {
                        plants.runs.extend(absolute(below, AIR, Some(f)));
                    }
                    let near = |y, height, block, only| Run { x, z, y, height, surface: None, block, only: Some(only) };
                    let mut on = |s, y, height, block, only| {
                        let mut r = near(y, height, block, only);
                        r.surface = Some(s);
                        surface.runs.push(r);
                    };
                    on(Surface::MotionBlockingNoLeaves, -1, 1, SCORCHED, GROUND);
                    on(Surface::WorldSurface, -3, 3, AIR, WATER);
                    if chance(blast.seed, x, z, 1) < FIRE {
                        on(Surface::MotionBlockingNoLeaves, 0, 1, FLAME, EMPTY);
                    }
                }
                Ring::Outer => {
                    plants.runs.extend(absolute(below, AIR, Some(GLASS)));
                    if chance(blast.seed, x, z, 2) < (1.0 - d) / (1.0 - MIDDLE) {
                        plants.runs.extend(absolute(below, AIR, Some(LEAVES)));
                        for f in PLANTS {
                            plants.runs.extend(absolute(below, AIR, Some(f)));
                        }
                    }
                }
                Ring::Untouched => {}
            }
        }
    }
    [plants, surface]
}

// ---- the wave ------------------------------------------------------------------
//
// The front runs out from the center at a steady speed. A chunk joins its
// wave when the front reaches the chunk's nearest block: loaded, it waits in
// `ready` for its turn; not loaded, in `deferred` until it is. The work is
// done a little each tick, within a time budget, nearest first.

/// Blocks the front runs in a tick: 50 blocks in a second, 1000 in twenty.
pub const FRONT_SPEED: f64 = 2.5;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Wave {
    blast: Blast,
    start: u64,
    /// How many of the wave's chunks, in `order`, the front has reached.
    reached: usize,
    ready: VecDeque<[i32; 2]>,
    deferred: BTreeSet<[i32; 2]>,
    /// The front is past the reach: its smoke and damage are over too.
    #[serde(default)]
    front_out: bool,
    /// The chunks the border can reach, nearest first, with how near they are:
    /// worked out again from the blast after loading.
    #[serde(skip)]
    order: Vec<(f64, [i32; 2])>,
}

impl Wave {
    fn order(&mut self) -> &[(f64, [i32; 2])] {
        if self.order.is_empty() {
            self.order = order(&self.blast);
        }
        &self.order
    }

    fn done(&mut self) -> bool {
        self.front_out && self.ready.is_empty() && self.deferred.is_empty() && self.reached == self.order().len()
    }
}

/// How far a chunk's nearest block is from the center.
fn nearest(center: [i32; 3], chunk: [i32; 2]) -> f64 {
    let x = center[0].clamp(chunk[0] * 16, chunk[0] * 16 + 15) - center[0];
    let z = center[2].clamp(chunk[1] * 16, chunk[1] * 16 + 15) - center[2];
    (x as f64).hypot(z as f64)
}

fn order(blast: &Blast) -> Vec<(f64, [i32; 2])> {
    let reach = blast.radius as f64 * (1.0 + BORDER);
    let c = chunk_of(blast.center[0], blast.center[2]);
    let r = (reach / 16.0).ceil() as i32 + 1;
    let mut out: Vec<(f64, [i32; 2])> = (c[0] - r..=c[0] + r)
        .flat_map(|x| (c[1] - r..=c[1] + r).map(move |z| [x, z]))
        .map(|chunk| (nearest(blast.center, chunk), chunk))
        .filter(|(d, _)| *d <= reach)
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    out
}

/// Every blast whose wave isn't over, in every dimension: what a world keeps.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Waves {
    waves: Vec<Wave>,
}

impl Waves {
    pub fn start(&mut self, blast: Blast, now_tick: u64) {
        self.waves.push(Wave { blast, start: now_tick, reached: 0, ready: VecDeque::new(), deferred: BTreeSet::new(), front_out: false, order: vec![] });
    }

    /// Moves the front of each wave in `dimension` on to `now_tick`.
    /// Answers whether the front reached any chunk.
    pub fn advance(&mut self, dimension: &str, now_tick: u64, loaded: &dyn Fn([i32; 2]) -> bool) -> bool {
        let mut moved = false;
        for w in self.waves.iter_mut().filter(|w| w.blast.dimension == dimension) {
            let front = now_tick.saturating_sub(w.start) as f64 * FRONT_SPEED;
            w.order();
            while let Some(&(d, chunk)) = w.order.get(w.reached) {
                if d > front {
                    break;
                }
                if loaded(chunk) {
                    w.ready.push_back(chunk);
                } else {
                    w.deferred.insert(chunk);
                }
                w.reached += 1;
                moved = true;
            }
            if !w.front_out && front > w.blast.radius as f64 * (1.0 + BORDER) {
                w.front_out = true;
                moved = true;
            }
        }
        self.waves.retain_mut(|w| !w.done());
        moved
    }

    /// A chunk came in: its waiting work is next. Answers whether any waited.
    pub fn chunk_loaded(&mut self, dimension: &str, chunk: [i32; 2]) -> bool {
        let mut moved = false;
        for w in self.waves.iter_mut().filter(|w| w.blast.dimension == dimension) {
            if w.deferred.remove(&chunk) {
                w.ready.push_front(chunk);
                moved = true;
            }
        }
        moved
    }

    /// The next chunk to change, taken off its wave's list.
    pub fn next_job(&mut self, dimension: &str) -> Option<(Blast, [i32; 2])> {
        let w = self.waves.iter_mut().find(|w| w.blast.dimension == dimension && !w.ready.is_empty())?;
        Some((w.blast.clone(), w.ready.pop_front()?))
    }

    /// Changes chunks until the budget is spent. `apply` answers false for a
    /// chunk that is no longer loaded, which then waits for its load.
    pub fn work(
        &mut self,
        dimension: &str,
        budget: Duration,
        elapsed: &dyn Fn() -> Duration,
        apply: &mut dyn FnMut(&Blast, [i32; 2]) -> bool,
    ) -> usize {
        let mut done = 0;
        while elapsed() < budget {
            let Some((blast, chunk)) = self.next_job(dimension) else { break };
            if apply(&blast, chunk) {
                done += 1;
            } else if let Some(w) = self.waves.iter_mut().find(|w| w.blast.id == blast.id) {
                w.deferred.insert(chunk);
            }
        }
        self.waves.retain_mut(|w| !w.done());
        done
    }

    pub fn finished(&self) -> bool {
        self.waves.is_empty()
    }

    /// Whether a running blast in `dimension` went off at `center`.
    /// The centers of the blasts in `dimension` that stand in `chunk`: a bomb set
    /// off there is taken when its wave works that chunk, even where its crater
    /// doesn't reach (above the Nether's roof).
    pub fn centers_in(&self, dimension: &str, chunk: [i32; 2]) -> Vec<[i32; 3]> {
        self.waves
            .iter()
            .filter(|w| w.blast.dimension == dimension && chunk_of(w.blast.center[0], w.blast.center[2]) == chunk)
            .map(|w| w.blast.center)
            .collect()
    }

    pub fn centered_at(&self, dimension: &str, center: [i32; 3]) -> bool {
        self.waves.iter().any(|w| w.blast.dimension == dimension && w.blast.center == center)
    }

    /// Nothing more to do in `dimension` until a chunk loads: every front there
    /// is out and every ready chunk changed.
    pub fn idle(&mut self, dimension: &str) -> bool {
        self.waves
            .iter_mut()
            .filter(|w| w.blast.dimension == dimension)
            .all(|w| w.front_out && w.ready.is_empty() && w.reached == w.order().len())
    }
}

// ---- the command and what a world keeps -----------------------------------------

/// The smallest and largest radius `/bignukes blast` takes.
pub const MIN_RADIUS: u32 = 8;
pub const MAX_RADIUS: u32 = 1024;

/// A dimension's height, for the ones the game has.
pub fn world_of(dimension: &str) -> Option<World> {
    match dimension {
        "minecraft:overworld" => Some(World::OVERWORLD),
        "minecraft:the_nether" => Some(World::NETHER),
        "minecraft:the_end" => Some(World::END),
        _ => None,
    }
}

/// `blast <radius> [x y z]`, after the command's name: the radius and the
/// center, the player's block when no place is given.
pub fn parse_blast(args: &[&str], at: [f64; 3]) -> Option<(u32, [i32; 3])> {
    let (radius, place) = args.split_first()?;
    let radius: u32 = radius.parse().ok().filter(|r| (MIN_RADIUS..=MAX_RADIUS).contains(r))?;
    let center = match place {
        [] => at.map(|v| v.floor() as i32),
        [x, y, z] => [x.parse().ok()?, y.parse().ok()?, z.parse().ok()?],
        _ => return None,
    };
    Some((radius, center))
}

/// What a world keeps in `<world>/horizon-big-nukes/waves.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Saved {
    /// Server ticks counted since the mod first saw the world.
    pub tick: u64,
    pub next_id: u64,
    pub waves: Waves,
    /// The operator's limit on a blast's radius: bombs and the command keep within it.
    #[serde(default = "largest_radius")]
    pub maxpower: u32,
    /// The bombs with a receiver: a remote fires one from its record, its chunk loaded or not.
    #[serde(default)]
    pub bombs: Vec<Receiver>,
    /// Every bomb standing in the world, as placing and taking it away tell: a
    /// blast's wave sets off the ones it reaches, loaded when it gets there or later.
    #[serde(default)]
    pub placed: Vec<Receiver>,
    /// The contaminated zones the larger blasts left.
    #[serde(default)]
    pub zones: Vec<Zone>,
    /// Each player's dose, by their UUID in hex (`dose_key`).
    #[serde(default)]
    pub doses: BTreeMap<String, Dose>,
}

fn largest_radius() -> u32 {
    MAX_RADIUS
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            tick: 0,
            next_id: 0,
            waves: Waves::default(),
            maxpower: largest_radius(),
            bombs: Vec::new(),
            placed: Vec::new(),
            zones: Vec::new(),
            doses: BTreeMap::new(),
        }
    }
}

/// Why a blast doesn't start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Dimension,
    Maxpower { radius: u32, maxpower: u32 },
    /// A remote's bomb that isn't there any more: broken, or already gone off.
    Gone,
}

impl Saved {
    /// A new blast at `center`, its wave started now.
    pub fn blast(&mut self, dimension: &str, center: [i32; 3], radius: u32) -> Option<u64> {
        let world = world_of(dimension)?;
        self.next_id += 1;
        let id = self.next_id;
        let seed = mix(self.tick ^ mix(id));
        let blast = Blast { id, dimension: dimension.to_owned(), center, radius, seed, world };
        self.zones.extend(zone_of(&blast, self.tick));
        self.waves.start(blast, self.tick);
        Some(id)
    }

    /// Whether the operator's limit lets a blast of `radius` go off.
    pub fn allows(&self, radius: u32) -> bool {
        radius <= self.maxpower
    }

    /// A new blast, as `blast`, within the operator's limit.
    pub fn fire(&mut self, dimension: &str, center: [i32; 3], radius: u32) -> Result<u64, Refused> {
        if !self.allows(radius) {
            return Err(Refused::Maxpower { radius, maxpower: self.maxpower });
        }
        self.blast(dimension, center, radius).ok_or(Refused::Dimension)
    }
}

/// The limits `/bignukes maxpower` takes: each bomb's radius, then the largest blast.
pub const MAXPOWER_STEPS: [u32; 4] = [16, 50, 250, 1024];

/// `maxpower [limit]`, after the command's name: Some(None) asks for the limit,
/// Some(Some(limit)) sets one of the steps, None is anything else.
pub fn parse_maxpower(args: &[&str]) -> Option<Option<u32>> {
    match args {
        [] => Some(None),
        [limit] => limit.parse().ok().filter(|l| MAXPOWER_STEPS.contains(l)).map(Some),
        _ => None,
    }
}

// ---- the remote --------------------------------------------------------------
//
// A remote is linked to a bomb with a receiver by a right click on it, and keeps
// the bomb's dimension and place in its `big_nukes:link` component. The world
// keeps each bomb with a receiver in `Saved.bombs`, so a remote fires one from
// anywhere, its chunk loaded or not: the wave writes the chunk when it loads.

pub const REMOTE: &str = "big_nukes:remote";
/// The remote's component: the linked bomb's dimension and place.
pub const LINK: &str = "big_nukes:link";

/// The bombs of `placed` in `chunk` that `blast` sets off: within its radius of
/// its center, in its dimension, its own bomb apart.
pub fn reached(placed: &[Receiver], blast: &Blast, chunk: [i32; 2]) -> Vec<([i32; 3], BombKind)> {
    let reach = f64::from(blast.radius).powi(2);
    placed
        .iter()
        .filter(|b| b.dimension == blast.dimension && b.pos != blast.center && chunk_of(b.pos[0], b.pos[2]) == chunk)
        .filter(|b| (0..3).map(|i| f64::from(b.pos[i] - blast.center[i]).powi(2)).sum::<f64>() <= reach)
        .map(|b| (b.pos, b.kind))
        .collect()
}

/// A bomb with a receiver, as the world keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receiver {
    pub dimension: String,
    pub pos: [i32; 3],
    pub kind: BombKind,
}

/// What a remote is linked to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub dimension: String,
    pub pos: [i32; 3],
}

impl Link {
    /// A link read from a remote; none without a dimension.
    pub fn of(dimension: &str, pos: [i32; 3]) -> Option<Link> {
        (!dimension.is_empty()).then(|| Link { dimension: dimension.to_owned(), pos })
    }
}

impl Saved {
    /// Records a bomb with a receiver, or what it is now when it was recorded
    /// before. False for a bomb whose blast is still running: it went off, and
    /// only stands until its wave reaches its chunk.
    pub fn record(&mut self, dimension: &str, pos: [i32; 3], kind: BombKind) -> bool {
        if self.waves.centered_at(dimension, pos) {
            return false;
        }
        match self.bombs.iter_mut().find(|b| b.dimension == dimension && b.pos == pos) {
            Some(b) => b.kind = kind,
            None => self.bombs.push(Receiver { dimension: dimension.to_owned(), pos, kind }),
        }
        true
    }

    /// The bomb recorded at `pos`, if there is one.
    pub fn recorded(&self, dimension: &str, pos: [i32; 3]) -> Option<BombKind> {
        self.bombs.iter().find(|b| b.dimension == dimension && b.pos == pos).map(|b| b.kind)
    }

    /// Forgets the bomb at `pos`: it was broken or went off. Whether there was one.
    pub fn forget(&mut self, dimension: &str, pos: [i32; 3]) -> bool {
        let before = self.bombs.len();
        self.bombs.retain(|b| b.dimension != dimension || b.pos != pos);
        self.bombs.len() != before
    }

    /// A bomb placed at `pos`: false when it was known already.
    pub fn place(&mut self, dimension: &str, pos: [i32; 3], kind: BombKind) -> bool {
        if self.placed.iter().any(|b| b.dimension == dimension && b.pos == pos && b.kind == kind) {
            return false;
        }
        self.unplace(dimension, pos);
        self.placed.push(Receiver { dimension: dimension.to_owned(), pos, kind });
        true
    }

    /// The bomb at `pos` taken away: false when none was known there.
    pub fn unplace(&mut self, dimension: &str, pos: [i32; 3]) -> bool {
        let before = self.placed.len();
        self.placed.retain(|b| b.dimension != dimension || b.pos != pos);
        self.placed.len() != before
    }

    /// The placed bombs in `chunk`.
    pub fn placed_in(&self, dimension: &str, chunk: [i32; 2]) -> Vec<[i32; 3]> {
        self.placed
            .iter()
            .filter(|b| b.dimension == dimension && chunk_of(b.pos[0], b.pos[2]) == chunk)
            .map(|b| b.pos)
            .collect()
    }

    /// The bomb at `pos`, reached by another blast, goes off with its own radius
    /// and is forgotten, its remote's record too. One over the operator's limit stays.
    pub fn set_off(&mut self, dimension: &str, pos: [i32; 3]) -> Result<u64, Refused> {
        let kind = self.placed.iter().find(|b| b.dimension == dimension && b.pos == pos).map(|b| b.kind).ok_or(Refused::Gone)?;
        let id = self.fire(dimension, pos, kind.radius())?;
        self.unplace(dimension, pos);
        self.forget(dimension, pos);
        Ok(id)
    }

    /// The recorded bombs in `chunk`.
    pub fn bombs_in(&self, dimension: &str, chunk: [i32; 2]) -> Vec<[i32; 3]> {
        self.bombs
            .iter()
            .filter(|b| b.dimension == dimension && chunk_of(b.pos[0], b.pos[2]) == chunk)
            .map(|b| b.pos)
            .collect()
    }

    /// A remote's press: the recorded bomb at `pos` goes off, within the
    /// operator's limit, and is forgotten. One over the limit stays recorded.
    pub fn fire_bomb(&mut self, dimension: &str, pos: [i32; 3]) -> Result<u64, Refused> {
        let kind = self.recorded(dimension, pos).ok_or(Refused::Gone)?;
        let id = self.fire(dimension, pos, kind.radius())?;
        self.forget(dimension, pos);
        self.unplace(dimension, pos);
        Ok(id)
    }
}

// ---- radiation ---------------------------------------------------------------
//
// A blast of radius 40 or more leaves a zone: a rule, not a map of chunks. The
// background in a place is worked out from the zones, falling from the center to
// the edge and with time. A player's dose builds in it, less in a hazmat suit,
// and heals slowly outside; its stage brings the sickness.

/// The smallest blast that leaves a zone: the demolition charge leaves none.
pub const ZONE_MIN_RADIUS: u32 = 40;
/// A zone reaches this much past the blast's radius.
const ZONE_REACH: f64 = 1.2;
/// The background at a zone's center, in rad a second, for each block of radius.
const PEAK_PER_BLOCK: f64 = 1.0 / 12.0;
/// From this radius a zone fades slower and never quite goes.
const BIG_ZONE: u32 = 150;
const HALF_LIFE: u64 = 12_000;
const BIG_HALF_LIFE: u64 = 48_000;
const BIG_FLOOR: f64 = 0.05;
/// A zone with no floor goes once it is this faint, or this many half-lives old.
const ZONE_GONE: f64 = 0.001;
const ZONE_HALF_LIVES: u64 = 10;

/// A contaminated zone: `peak` rad a second at `center`, nothing past `radius`
/// blocks, halved every `half_life` ticks since `start`, but never below `floor`
/// of the peak.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub dimension: String,
    pub center: [i32; 3],
    pub radius: f64,
    pub peak: f64,
    pub start: u64,
    pub half_life: u64,
    pub floor: f64,
}

impl Zone {
    fn age(&self, now: u64) -> u64 {
        now.saturating_sub(self.start)
    }

    /// The share of the peak left at `now`.
    pub fn fade(&self, now: u64) -> f64 {
        0.5f64.powf(self.age(now) as f64 / self.half_life.max(1) as f64).max(self.floor)
    }

    /// The background at `pos` (a point in the zone's dimension) at `now`: the
    /// distance counts up and down as well as across, from the middle of the
    /// center's block.
    pub fn at(&self, pos: [f64; 3], now: u64) -> f64 {
        let d2: f64 = (0..3).map(|i| (pos[i] - (self.center[i] as f64 + 0.5)).powi(2)).sum();
        let r2 = self.radius * self.radius;
        if d2 >= r2 {
            return 0.0;
        }
        self.peak * (1.0 - d2 / r2) * self.fade(now)
    }

    /// Whether the zone has faded out for good: one with a floor never does.
    fn gone(&self, now: u64) -> bool {
        self.floor <= 0.0 && (self.age(now) > ZONE_HALF_LIVES * self.half_life || self.peak * self.fade(now) < ZONE_GONE)
    }
}

/// The zone a blast leaves at `now`, if it is large enough to leave one.
pub fn zone_of(blast: &Blast, now: u64) -> Option<Zone> {
    if blast.radius < ZONE_MIN_RADIUS {
        return None;
    }
    let r = blast.radius as f64;
    let big = blast.radius >= BIG_ZONE;
    Some(Zone {
        dimension: blast.dimension.clone(),
        center: blast.center,
        radius: ZONE_REACH * r,
        peak: r * PEAK_PER_BLOCK,
        start: now,
        half_life: if big { BIG_HALF_LIFE } else { HALF_LIFE },
        floor: if big { BIG_FLOOR } else { 0.0 },
    })
}

/// The background at `pos` in `dimension`: every zone's share added up.
pub fn background(zones: &[Zone], dimension: &str, pos: [f64; 3], now: u64) -> f64 {
    zones.iter().filter(|z| z.dimension == dimension).map(|z| z.at(pos, now)).sum()
}

/// Ticks in a second: a dose is reckoned once a second.
pub const SECOND: u64 = 20;
/// The dose heals by this much a second.
const HEAL: f64 = 0.5;
/// The doses where each stage starts, in rad.
pub const STAGES: [f64; 4] = [100.0, 300.0, 600.0, 1000.0];

/// A player's dose and its clock.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dose {
    /// The dose, in rad.
    pub rad: f64,
    /// The tick the dose was last reckoned at.
    pub stepped: u64,
    /// The tick the sickness last did damage at.
    pub hurt: Option<u64>,
}

/// The dose after one second in `background`, with `protection` kept out.
pub fn dose_step(dose: f64, background: f64, protection: f64) -> f64 {
    (dose + background * (1.0 - protection.clamp(0.0, 1.0)) - HEAL).max(0.0)
}

/// The sickness' stage: 0 well, 1 weak and hungry, 2 sick, 3 very sick, 4 lethal.
pub fn stage(dose: f64) -> u8 {
    STAGES.iter().filter(|&&s| dose >= s).count() as u8
}

/// The damage a stage does and how often, in ticks.
pub fn harm(stage: u8) -> Option<(u32, u64)> {
    match stage {
        0 | 1 => None,
        2 => Some((1, 5 * SECOND)),
        3 => Some((2, 2 * SECOND)),
        _ => Some((1000, SECOND)),
    }
}

/// The damage a mob takes each second in `background`.
pub fn mob_harm(background: f64) -> Option<u32> {
    if background >= 5.0 {
        Some(2)
    } else if background >= 1.0 {
        Some(1)
    } else {
        None
    }
}

/// What each piece keeps out, head to feet.
const SUIT: [f64; 4] = [0.25, 0.30, 0.25, 0.15];
/// A worn piece loses a point of durability each second the background is at least this.
pub const WEARS_FROM: f64 = 0.1;

/// The share of the background the worn pieces keep out.
pub fn protection(head: bool, chest: bool, legs: bool, feet: bool) -> f64 {
    [head, chest, legs, feet].iter().zip(SUIT).filter(|(worn, _)| **worn).map(|(_, kept)| kept).sum()
}

/// The hazmat piece that belongs in an armour slot, by the inventory's numbers
/// (100 feet to 103 head).
pub fn hazmat_in(slot: i32) -> Option<&'static str> {
    (100..=103).contains(&slot).then(|| HAZMAT[(103 - slot) as usize].0)
}

/// A piece's damage after one more point of wear; none when that breaks it.
pub fn wear(damage: u32, max_damage: u32) -> Option<u32> {
    let damage = damage + 1;
    (damage < max_damage).then_some(damage)
}

/// The key of a player's dose: their UUID in hex.
pub fn dose_key(uuid: u128) -> String {
    format!("{uuid:032x}")
}


/// A UUID as commands take it: `00000000-0000-0000-0000-000000000000`.
pub fn uuid_text(uuid: u128) -> String {
    let h = format!("{uuid:032x}");
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

impl Saved {
    /// The zones that faded out for good are dropped. Whether any was.
    pub fn prune_zones(&mut self, now: u64) -> bool {
        let before = self.zones.len();
        self.zones.retain(|z| !z.gone(now));
        self.zones.len() != before
    }

    /// A second of the player's exposure at `pos`, with `protection` kept out:
    /// the background there, or None when a second hasn't passed since the last.
    pub fn expose(&mut self, uuid: u128, dimension: &str, pos: [f64; 3], protection: f64) -> Option<f64> {
        let (now, key) = (self.tick, dose_key(uuid));
        // A player with no dose is looked at on the second's ticks; one with a
        // dose a second after the last time, which holds while the clock stands.
        match self.doses.get(&key) {
            Some(d) if now < d.stepped + SECOND => return None,
            None if now % SECOND != 0 => return None,
            _ => {}
        }
        let bg = background(&self.zones, dimension, pos, now);
        let dose = self.doses.entry(key.clone()).or_default();
        dose.rad = dose_step(dose.rad, bg, protection);
        dose.stepped = now;
        if dose.rad <= 0.0 {
            self.doses.remove(&key);
        }
        Some(bg)
    }

    /// The player's stage and the damage due now, once a second; the damage's
    /// clock starts again when it is done.
    pub fn sicken(&mut self, uuid: u128) -> Option<(u8, Option<u32>)> {
        let now = self.tick;
        let dose = self.doses.get_mut(&dose_key(uuid))?;
        let stage = stage(dose.rad);
        if stage == 0 {
            return None;
        }
        let due = harm(stage).filter(|(_, every)| dose.hurt.is_none_or(|h| now >= h + every));
        let damage = due.map(|(n, _)| {
            dose.hurt = Some(now);
            n
        });
        Some((stage, damage))
    }

    /// The anti-radiation drug takes `amount` off the player's dose. Whether there was any.
    pub fn take_dose(&mut self, uuid: u128, amount: f64) -> bool {
        let key = dose_key(uuid);
        let Some(dose) = self.doses.get_mut(&key) else {
            return false;
        };
        dose.rad = (dose.rad - amount.max(0.0)).max(0.0);
        if dose.rad <= 0.0 {
            self.doses.remove(&key);
        }
        true
    }

    /// The player died: their dose is gone. Whether they had one.
    pub fn died(&mut self, uuid: u128) -> bool {
        self.doses.remove(&dose_key(uuid)).is_some()
    }

    /// What the window of a player at `pos` in `dimension` is told: the zones
    /// of that dimension nearest to them, at most `RAD_ZONES`, the mod's clock
    /// and their dose.
    pub fn rad_for(&self, uuid: u128, dimension: &str, pos: [f64; 3]) -> Rad {
        let now = self.tick;
        let outside = |z: &Zone| {
            let d2: f64 = (0..3).map(|i| (pos[i] - (z.center[i] as f64 + 0.5)).powi(2)).sum();
            (d2.sqrt() - z.radius).max(0.0)
        };
        let mut zones: Vec<&Zone> = self.zones.iter().filter(|z| z.dimension == dimension).collect();
        zones.sort_by(|a, b| outside(a).total_cmp(&outside(b)).then(b.at(pos, now).total_cmp(&a.at(pos, now))));
        zones.truncate(RAD_ZONES);
        Rad {
            zones: zones.into_iter().cloned().collect(),
            now,
            dose: self.doses.get(&dose_key(uuid)).map_or(0.0, |d| d.rad),
        }
    }
}

// ---- radiation in the window ----------------------------------------------------
//
// The server tells each player's window its zones, its clock and their dose once
// a second; the window works out the background where the player is from them, on
// its own clock between the messages, for the Geiger counter and the fog.

/// The mod message that carries `Rad`.
pub const RAD_CHANNEL: &str = "big_nukes:rad";
/// The most zones a message carries: the nearest.
pub const RAD_ZONES: usize = 32;
/// The counter's click, a sound event of the assets half.
pub const GEIGER_CLICK: &str = "big_nukes:geiger.click";

/// What a window knows of the radiation: its dimension's nearest zones, the
/// mod's clock they are reckoned on, and the player's dose.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rad {
    pub zones: Vec<Zone>,
    pub now: u64,
    pub dose: f64,
}

impl Rad {
    /// Nothing to tell: no zone and no dose. Sent once, so the window lets go.
    pub fn is_empty(&self) -> bool {
        self.zones.is_empty() && self.dose <= 0.0
    }
}

/// A `Rad` as the window keeps it, with its own game time when it came.
#[derive(Clone, Debug, PartialEq)]
pub struct Told {
    pub rad: Rad,
    pub at: u64,
}

impl Told {
    /// The background at `pos` in `dimension` when the window's game time is
    /// `clock`: the mod's clock counts on from the message. A clock behind the
    /// message's counts as no time passed.
    pub fn background(&self, dimension: &str, pos: [f64; 3], clock: u64) -> f64 {
        background(&self.rad.zones, dimension, pos, self.rad.now + clock.saturating_sub(self.at))
    }
}

/// The counter's chance of a click in a tick: a quarter for each rad a second, at most 0.9.
pub fn click_chance(background: f64) -> f64 {
    if background > 0.0 { (background * 0.25).min(0.9) } else { 0.0 }
}

/// A number from 0 to 1 made from `tick` and `salt`, the same each time.
fn roll(tick: u64, salt: u64) -> f64 {
    (mix(tick ^ salt) >> 11) as f64 / (1u64 << 53) as f64
}

/// Whether the counter clicks on the window's tick `tick` in `background`.
pub fn clicks(background: f64, tick: u64) -> bool {
    roll(tick, 0x6E16_E500) < click_chance(background)
}

/// A click's pitch, 0.9 to 1.1, so they don't sound alike.
pub fn click_pitch(tick: u64) -> f32 {
    (0.9 + 0.2 * roll(tick, 0xC11C_0000)) as f32
}

pub const GREEN: u32 = 0x55_FF55;
pub const YELLOW: u32 = 0xFF_FF55;
pub const RED: u32 = 0xFF_5555;

/// The reading's colour: green below 1 rad a second, yellow below 5, red from 5.
pub fn reading_colour(background: f64) -> u32 {
    if background >= 5.0 {
        RED
    } else if background >= 1.0 {
        YELLOW
    } else {
        GREEN
    }
}

/// The reading's numbers: the background to a tenth, the dose to a whole rad.
pub fn reading_numbers(background: f64, dose: f64) -> (String, String) {
    (format!("{:.1}", background.max(0.0)), format!("{:.0}", dose.max(0.0)))
}

/// The fog of a zone shows from this background, and is full at `FOG_FULL`.
const FOG_FROM: f64 = 0.2;
const FOG_FULL: f64 = 5.0;
/// The colour a full zone's fog turns to, and how near it ends.
const FOG_TINT: [f32; 3] = [0.55, 0.62, 0.45];
const FOG_END: f32 = 64.0;
/// A fog starting this far is Horizon's Fog option turned off: its distances stay.
const FOG_OFF: f32 = 1e8;
/// The seconds the fog takes to come most of the way (63%) to a new strength.
const FOG_EASE: f64 = 0.4;

/// How strong the zone's fog is in `background`, 0 to 1.
pub fn fog_strength(background: f64) -> f64 {
    if background >= FOG_FROM { (background / FOG_FULL).min(1.0) } else { 0.0 }
}

/// The fog shown moves from `shown` toward `target`, `seconds` later: by the
/// time gone, so the frame rate doesn't change it.
pub fn ease(shown: f64, target: f64, seconds: f64) -> f64 {
    shown + (target - shown) * (1.0 - (-seconds.max(0.0) / FOG_EASE).exp())
}

/// The world's fog (its colour, the start and the end of the fog of the
/// surroundings) with a zone's of `strength` over it: the colour toward the
/// zone's, the end in toward 64 blocks and the start toward the eyes. None for
/// no fog, and when the result isn't one the game takes.
pub fn tint_fog(strength: f64, color: [f32; 4], start: f32, end: f32) -> Option<([f32; 4], f32, f32)> {
    let s = strength.min(1.0) as f32;
    if !(s > 0.0) || !start.is_finite() || !end.is_finite() {
        return None;
    }
    let mut tinted = color;
    for i in 0..3 {
        tinted[i] = (color[i] + (FOG_TINT[i] - color[i]) * s).clamp(0.0, 1.0);
    }
    if start >= FOG_OFF {
        return Some((tinted, start, end));
    }
    let start = start + (start.min(0.0) - start) * s;
    let end = end + (end.min(FOG_END) - end) * s;
    (end > start).then_some((tinted, start, end))
}

// ---- the blast's effects ------------------------------------------------------
//
// What a player sees, hears and feels, worked out from how far they are and how
// long ago the blast was: the server sends `News` once, and the window does the rest.

/// A blast as a window hears of it: `age` is how many ticks ago it went off.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct News {
    pub id: u64,
    pub center: [i32; 3],
    pub radius: u32,
    pub seed: u64,
    pub age: u64,
}

/// The mod message that carries `News`.
pub const NEWS_CHANNEL: &str = "big_nukes:blast";
/// After this long a blast is past telling: its sound has come and gone.
const NEWS_FOR: u64 = 2400;

pub fn news_of(blast: &Blast, now_tick: u64, start: u64) -> News {
    News { id: blast.id, center: blast.center, radius: blast.radius, seed: blast.seed, age: now_tick.saturating_sub(start) }
}

impl Waves {
    /// The running fronts of `dimension`: each blast with the ground its front
    /// crossed since the last tick (from, to) and its age in ticks.
    pub fn fronts(&self, dimension: &str, now_tick: u64) -> Vec<(Blast, f64, f64, u64)> {
        self.waves
            .iter()
            .filter(|w| w.blast.dimension == dimension)
            .filter_map(|w| {
                let age = now_tick.saturating_sub(w.start);
                let from = age.saturating_sub(1) as f64 * FRONT_SPEED;
                (from <= w.blast.radius as f64 * (1.0 + BORDER)).then(|| (w.blast.clone(), from, age as f64 * FRONT_SPEED, age))
            })
            .collect()
    }

    /// The blasts of `dimension` still worth telling a player about.
    pub fn news(&self, dimension: &str, now_tick: u64) -> Vec<News> {
        self.waves
            .iter()
            .filter(|w| w.blast.dimension == dimension && now_tick.saturating_sub(w.start) < NEWS_FOR)
            .map(|w| news_of(&w.blast, now_tick, w.start))
            .collect()
    }
}

/// A blast as the window keeps it: its news, the dimension the player heard it
/// in, and the game time it went off.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub news: News,
    pub dimension: String,
    pub at: f64,
}

impl Heard {
    /// The ticks since the blast and the distance to it, for a player in
    /// `dimension` with eyes at `eyes`, at game time `now` (with the part of the
    /// tick gone). None once it is past, in another dimension, or from another
    /// world whose clock is behind.
    pub fn felt(&self, dimension: &str, eyes: [f64; 3], now: f64) -> Option<(f64, f64)> {
        let c = self.news.center;
        let d = [eyes[0] - c[0] as f64 - 0.5, eyes[1] - c[1] as f64, eyes[2] - c[2] as f64 - 0.5];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let t = now - self.at;
        // Kept while its front, its sound or its cloud (five minutes) is still to see.
        let lasts = (dist / FRONT_SPEED + 200.0).max(CLOUD_TICKS);
        (dimension == self.dimension && t > -20.0 && t < lasts).then_some((t, dist))
    }
}

/// The felt blasts in the order they are drawn: the newest first, so a fresh
/// fireball is never left out for older clouds when there are too many.
pub fn newest_first(felt: &mut [(f64, f64, News)]) {
    felt.sort_by(|a, b| a.0.total_cmp(&b.0));
}

/// How long a cloud stands: it is gone at five minutes.
pub const CLOUD_TICKS: f64 = 6000.0;

/// The white of the flash, 0 to 255: full within two radii, gone at twelve,
/// and fading over a second or so.
pub fn flash_alpha(t_ticks: f64, dist: f64, radius: u32) -> u8 {
    if !(0.0..80.0).contains(&t_ticks) {
        return 0;
    }
    let r = radius as f64;
    let peak = ((12.0 * r - dist) / (10.0 * r)).clamp(0.0, 1.0);
    (255.0 * peak * (-t_ticks / 20.0).exp()).round() as u8
}

/// A sound to play, `delay` ticks from now.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub event: &'static str,
    pub delay: i32,
    pub volume: f32,
}

/// What a player `dist` blocks from the center hears, `age` ticks after the
/// blast: the boom, near or far, and the roar of the wave if it reaches them,
/// all arriving with the front. What is already past is left out.
pub fn sounds_for(dist: f64, radius: u32, age: u64) -> Vec<Sound> {
    let r = radius as f64;
    if dist > 3000.0 {
        return vec![];
    }
    let delay = (dist / FRONT_SPEED).min(1200.0).round() as i64 - age as i64;
    if delay < 0 {
        return vec![];
    }
    let volume = (1.5 - dist / (10.0 * r)).clamp(0.2, 1.0) as f32;
    let boom = if dist < 1.5 * r { "big_nukes:blast.near" } else { "big_nukes:blast.far" };
    let mut out = vec![Sound { event: boom, delay: delay as i32, volume }];
    if dist <= r * (1.0 + BORDER) {
        out.push(Sound { event: "big_nukes:wave.roar", delay: delay as i32, volume });
    }
    out
}

/// The camera's shake: pitch, yaw and roll in degrees, then an offset in blocks.
/// It starts when the front reaches the player and dies out in a few seconds.
pub fn shake(t_ticks: f64, dist: f64, radius: u32, seed: u64) -> [f32; 6] {
    let since = t_ticks - dist / FRONT_SPEED;
    let strength = (1.3 - dist / (4.0 * radius as f64)).clamp(0.0, 1.0);
    if !(0.0..=100.0).contains(&since) || strength == 0.0 {
        return [0.0; 6];
    }
    let e = strength * (-since / 20.0).exp();
    let wobble = |k: u64, rate: f64| {
        let phase = (mix(seed ^ k) >> 11) as f64 / (1u64 << 53) as f64 * std::f64::consts::TAU;
        (t_ticks * rate + phase).sin()
    };
    [
        (6.0 * e * wobble(1, 1.9)) as f32,
        (6.0 * e * wobble(2, 2.3)) as f32,
        (3.0 * e * wobble(3, 1.3)) as f32,
        (0.3 * e * wobble(4, 2.9)) as f32,
        (0.3 * e * wobble(5, 3.1)) as f32,
        (0.3 * e * wobble(6, 2.7)) as f32,
    ]
}

/// Sixteen points on the front, two blocks over the center's height, while it runs.
pub fn front_ring(blast: &Blast, t: u64) -> Vec<[f64; 3]> {
    let r = t as f64 * FRONT_SPEED;
    if r < 1.0 || r > blast.radius as f64 * (1.0 + BORDER) {
        return vec![];
    }
    let turn = (mix(blast.seed ^ 7) >> 11) as f64 / (1u64 << 53) as f64;
    let [cx, cy, cz] = blast.center.map(|v| v as f64);
    (0..16)
        .map(|k| {
            let a = (k as f64 + turn) / 16.0 * std::f64::consts::TAU;
            [cx + r * a.cos(), cy + 2.0, cz + r * a.sin()]
        })
        .collect()
}

/// The damage of the ground the front crossed, from `from` to `to` blocks out:
/// `(from, to, amount)` bands, cut where the zones change. Death in the core,
/// a little at the edge, nothing past the reach.
pub fn damage_band(radius: u32, from: f64, to: f64) -> Vec<(f64, f64, f32)> {
    let r = radius as f64;
    let to = to.min(r * (1.0 + BORDER));
    let cuts = [CORE * r, MIDDLE * r];
    let mut edges = vec![from];
    edges.extend(cuts.iter().copied().filter(|c| *c > from && *c < to));
    edges.push(to);
    edges
        .windows(2)
        .filter(|e| e[1] > e[0])
        .map(|e| {
            let d = (e[0] + e[1]) / 2.0 / r;
            let amount = if d < CORE {
                1000.0
            } else if d < MIDDLE {
                20.0 - (d - CORE) / (MIDDLE - CORE) * 14.0
            } else {
                4.0 - (d - MIDDLE) / (1.0 + BORDER - MIDDLE) * 3.0
            };
            (e[0], e[1], amount as f32)
        })
        .collect()
}

/// Blocks thrown out of the crater at the start: where, how fast and what.
pub fn debris(blast: &Blast) -> Vec<([f64; 3], [f64; 3], &'static str)> {
    const BLOCKS: [&str; 3] = ["minecraft:stone", "minecraft:cobblestone", "minecraft:dirt"];
    let n = (blast.radius / 2).min(48) as u64;
    let [cx, cy, cz] = blast.center.map(|v| v as f64 + 0.5);
    (0..n)
        .map(|k| {
            let roll = |salt: u64| (mix(blast.seed ^ (k << 8) ^ salt) >> 11) as f64 / (1u64 << 53) as f64;
            let a = roll(1) * std::f64::consts::TAU;
            let out = 0.8 + 0.8 * roll(2);
            let up = 0.9 + 0.7 * roll(3);
            ([cx, cy - 0.5 + 3.0, cz], [out * a.cos(), up, out * a.sin()], BLOCKS[(k % 3) as usize])
        })
        .collect()
}

// ---- the mushroom cloud ---------------------------------------------------------
//
// Its shape over time, in blocks: the renderer gets these numbers and only draws.

/// A cloud at one moment: the fireball's height and size, the stem, the cap (a
/// torus around the fireball), how much fire still glows, how visible it all is,
/// and the dust ring on the ground with its own fade.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cloud {
    pub ball_h: f32,
    pub ball_r: f32,
    pub stem_r: f32,
    pub cap_major: f32,
    pub cap_minor: f32,
    pub fire: f32,
    pub fade: f32,
    pub ring: f32,
    pub ring_fade: f32,
}

pub fn cloud(age_ticks: f64, radius: u32) -> Cloud {
    if age_ticks < 0.0 {
        return Cloud::default();
    }
    let r = radius as f64;
    let t = age_ticks / 20.0;
    let top = 0.9 * r + 60.0;
    let ball_r = 0.12 * r + 8.0;
    let reach = r * (1.0 + BORDER);
    let out_at = reach / FRONT_SPEED;
    Cloud {
        ball_h: (top * (1.0 - (-t / 25.0).exp())) as f32,
        ball_r: ball_r as f32,
        stem_r: (0.35 * ball_r) as f32,
        cap_major: (0.5 * ball_r * (1.0 + t / 20.0)).min(1.4 * ball_r) as f32,
        cap_minor: (0.8 * ball_r) as f32,
        fire: (-t / 20.0).exp() as f32,
        fade: (1.0 - (t - 180.0) / (CLOUD_TICKS / 20.0 - 180.0)).clamp(0.0, 1.0) as f32,
        ring: (age_ticks * FRONT_SPEED).min(reach) as f32,
        ring_fade: (1.0 - (age_ticks - out_at).max(0.0) / 200.0).clamp(0.0, 1.0) as f32,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    fn file(path: &str) -> String {
        std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn language(code: &str) -> BTreeMap<String, String> {
        let path = format!("content/assets/big_nukes/lang/{code}.json");
        serde_json::from_str(&file(&path)).unwrap()
    }

    fn keys_in(source: &str) -> Vec<String> {
        source
            .split('"')
            .filter(|t| t.starts_with("big_nukes.") && !t.ends_with(".json"))
            .filter(|t| !t.contains(' ') && !t.ends_with('.'))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn every_line_is_in_both_languages_with_the_same_arguments() {
        let (en, ru) = (language("en_us"), language("ru_ru"));
        let used: BTreeSet<String> = [
            "mixins/big_nukes_native/runtime.rs",
            "mixins/big_nukes_native/glue.rs",
            "content/data/big_nukes/function/check.mcfunction",
        ]
        .iter()
        .flat_map(|s| keys_in(file(s).split("#[cfg(test)]").next().unwrap()))
        .collect();
        assert!(!used.is_empty());
        for key in &used {
            assert!(en.contains_key(key), "{key} isn't in en_us");
        }
        for (key, line) in &en {
            let theirs = ru.get(key).unwrap_or_else(|| panic!("{key} isn't in ru_ru"));
            assert_eq!(line.matches("%s").count(), theirs.matches("%s").count(), "{key}");
            assert!(
                key.starts_with("block.")
                    || key.starts_with("item.")
                    || key.starts_with("effect.big_nukes.")
                    || key.starts_with("death.attack.big_nukes.")
                    || key.starts_with("subtitles.big_nukes.")
                    || key == "big_nukes.mod.description"
                    || used.contains(key),
                "{key} is used nowhere"
            );
        }
        assert_eq!(en.len(), ru.len());
    }

    #[test]
    fn the_assets_bring_the_marker() {
        let (namespace, name) = super::ASSETS_MARKER.split_once(':').unwrap();
        assert!(exists(&format!("content/data/{namespace}/loot_table/{name}.json")), "{name}");
    }

    // ---- blast geometry (Task 2) -------------------------------------------

    use super::*;

    fn blast(center: [i32; 3], radius: u32, seed: u64, world: World) -> Blast {
        Blast { id: 1, dimension: "minecraft:overworld".into(), center, radius, seed, world }
    }

    fn columns_in(b: &Blast, wanted: Ring) -> BTreeSet<[i32; 2]> {
        let r = b.radius as i32 * 2;
        let mut out = BTreeSet::new();
        for x in b.center[0] - r..=b.center[0] + r {
            for z in b.center[2] - r..=b.center[2] + r {
                if ring(edge(b, x, z)) == wanted {
                    out.insert([x, z]);
                }
            }
        }
        out
    }

    #[test]
    fn chunk_of_rounds_down_for_negative_coordinates() {
        assert_eq!(chunk_of(-1, -1), [-1, -1]);
        assert_eq!(chunk_of(-16, 15), [-1, 0]);
        assert_eq!(chunk_of(-17, 16), [-2, 1]);
        assert_eq!(chunk_of(0, 31), [0, 1]);
    }

    #[test]
    fn edge_is_zero_at_the_center_and_stays_within_twelve_percent_of_the_radius() {
        let b = blast([0, 64, 0], 200, 7, World::OVERWORLD);
        assert_eq!(edge(&b, 0, 0), 0.0);
        for deg in 0..360 {
            let a = (deg as f64).to_radians();
            let at = |r: f64| edge(&b, (r * a.cos()).round() as i32, (r * a.sin()).round() as i32);
            assert!(at(200.0 * 0.87) <= 1.0, "{deg}");
            assert!(at(200.0 * 1.13) >= 1.0, "{deg}");
        }
    }

    #[test]
    fn the_border_is_uneven_but_repeatable() {
        let a = blast([0, 64, 0], 200, 7, World::OVERWORLD);
        let again = blast([0, 64, 0], 200, 7, World::OVERWORLD);
        let other = blast([0, 64, 0], 200, 8, World::OVERWORLD);
        let ring = |b: &Blast| (0..360).map(|d| edge(b, (200.0 * (d as f64).to_radians().cos()) as i32, (200.0 * (d as f64).to_radians().sin()) as i32)).collect::<Vec<_>>();
        assert_eq!(ring(&a), ring(&again));
        assert_ne!(ring(&a), ring(&other));
        let r = ring(&a);
        let (lo, hi) = r.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(hi - lo > 0.05, "the border is a circle: {lo}..{hi}");
    }

    #[test]
    fn zones_split_at_035_07_and_1() {
        assert_eq!(ring(0.0), Ring::Core);
        assert_eq!(ring(0.3499), Ring::Core);
        assert_eq!(ring(0.35), Ring::Middle);
        assert_eq!(ring(0.6999), Ring::Middle);
        assert_eq!(ring(0.7), Ring::Outer);
        assert_eq!(ring(0.9999), Ring::Outer);
        assert_eq!(ring(1.0), Ring::Untouched);
    }

    #[test]
    fn the_crater_is_deepest_at_the_center_and_never_reaches_bedrock() {
        let b = blast([0, 70, 0], 250, 1, World::OVERWORLD);
        assert_eq!(crater_floor(&b, 0.0), 70 - 62);
        assert!(crater_floor(&b, 0.2) > crater_floor(&b, 0.0));
        assert_eq!(crater_floor(&b, 0.35), 70);
        let deep = blast([0, -50, 0], 250, 1, World::OVERWORLD);
        assert_eq!(crater_floor(&deep, 0.0), -64 + 5);
    }

    #[test]
    fn a_blast_at_minus_one_is_symmetric() {
        let a = blast([-1, 64, -1], 24, 3, World::OVERWORLD);
        let b = blast([0, 64, 0], 24, 3, World::OVERWORLD);
        let shifted: BTreeSet<[i32; 2]> = columns_in(&b, Ring::Core).iter().map(|[x, z]| [x - 1, z - 1]).collect();
        assert_eq!(columns_in(&a, Ring::Core), shifted);
        assert!(columns_in(&a, Ring::Core).contains(&[4, -1]) && columns_in(&a, Ring::Core).contains(&[-6, -1]));
    }

    fn all_runs(b: &Blast) -> Vec<(usize, Run)> {
        let r = b.radius as i32 * 2 / 16 + 1;
        let c = chunk_of(b.center[0], b.center[2]);
        let mut out = vec![];
        for cx in c[0] - r..=c[0] + r {
            for cz in c[1] - r..=c[1] + r {
                for (i, e) in plan_chunk(b, [cx, cz]).into_iter().enumerate() {
                    out.extend(e.runs.into_iter().map(|run| (i, run)));
                }
            }
        }
        out
    }

    #[test]
    fn runs_stay_inside_the_world() {
        for (center, world) in [([5, 300, 5], World::OVERWORLD), ([5, 250, 5], World::NETHER), ([5, 2, 5], World::NETHER)] {
            let b = blast(center, 60, 9, world);
            let runs = all_runs(&b);
            assert!(!runs.is_empty());
            for (_, r) in runs.iter().filter(|(_, r)| r.surface.is_none()) {
                assert!(r.y >= world.min_y && r.y + r.height as i32 <= world.top(), "{r:?} in {world:?}");
                assert!(r.height > 0);
            }
        }
    }

    #[test]
    fn a_chunk_beyond_the_edge_plans_nothing() {
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        for e in plan_chunk(&b, chunk_of(230, 0)) {
            assert!(e.runs.is_empty());
        }
    }

    #[test]
    fn the_core_clears_whole_columns_and_the_rim_only_filters() {
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        let edits = plan_chunk(&b, [0, 0]);
        let air = edits[0].palette.iter().position(|p| p == "minecraft:air").unwrap() as u32;
        let core = edits[0].runs.iter().find(|r| r.x == 1 && r.z == 1 && r.only.is_none()).expect("a core column is cleared");
        assert_eq!(core.block, air);
        assert_eq!(core.y + core.height as i32, World::OVERWORLD.top());
        let outer = plan_chunk(&b, chunk_of(85, 0));
        let at_85: Vec<&Run> = outer.iter().flat_map(|e| &e.runs).filter(|r| r.x == 85 && r.z == 0).collect();
        assert!(!at_85.is_empty() && at_85.iter().all(|r| r.only.is_some()), "{at_85:?}");
    }

    #[test]
    fn the_middle_scorches_the_surface_after_the_plants_are_gone() {
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        let [plants, surface] = plan_chunk(&b, chunk_of(50, 0));
        let ash = surface.palette.iter().position(|p| p == ASH).unwrap() as u32;
        assert!(plants.runs.iter().any(|r| r.x == 50 && r.z == 0));
        assert!(surface.runs.iter().any(|r| r.x == 50 && r.z == 0 && r.block == ash && r.surface.is_some()));
    }

    #[test]
    fn the_scorch_uses_ash() {
        assert_eq!(ASH, "big_nukes:ash");
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        let [_, surface] = plan_chunk(&b, chunk_of(50, 0));
        assert!(!surface.palette.iter().any(|p| p == "minecraft:coarse_dirt"), "{:?}", surface.palette);
        let ash = surface.palette.iter().position(|p| p == ASH).expect("ash in the palette") as u32;
        let ground = surface.filters.iter().position(|f| f == "#minecraft:substrate_overworld").unwrap() as u32;
        let scorched: Vec<&Run> = surface.runs.iter().filter(|r| r.only == Some(ground)).collect();
        assert!(!scorched.is_empty());
        assert!(scorched.iter().all(|r| r.block == ash && r.surface == Some(Surface::MotionBlockingNoLeaves)));
    }

    #[test]
    fn without_the_ash_block_the_scorch_is_coarse_dirt() {
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        for chunk in [[0, 0], chunk_of(50, 0)] {
            for edit in plan_chunk(&b, chunk) {
                let bare = edit.clone().without_ash();
                assert_eq!((&bare.filters, &bare.runs), (&edit.filters, &edit.runs));
                assert_eq!(bare.palette.len(), edit.palette.len());
                for (was, is) in edit.palette.iter().zip(&bare.palette) {
                    assert_eq!(is, if was == ASH { "minecraft:coarse_dirt" } else { was.as_str() });
                }
                assert!(!bare.palette.iter().any(|p| p == ASH));
            }
        }
    }

    #[test]
    fn water_and_lava_are_never_taken_with_the_plants() {
        let b = blast([0, 64, 0], 100, 2, World::OVERWORLD);
        for chunk in [[0, 0], chunk_of(50, 0), chunk_of(85, 0)] {
            let [plants, surface] = plan_chunk(&b, chunk);
            for r in plants.runs.iter().filter(|r| r.only.is_some()) {
                let f = &plants.filters[r.only.unwrap() as usize];
                assert!(!f.contains("replaceable") && !f.contains("water") && !f.contains("lava"), "{f}");
            }
            for r in surface.runs.iter().filter(|r| surface.filters[r.only.unwrap() as usize].contains("water")) {
                assert_eq!((r.y, r.height, r.surface), (-3, 3, Some(Surface::WorldSurface)));
            }
        }
    }

    // ---- the wave (Task 3) -------------------------------------------------

    use std::time::Duration;

    fn waves_with(blasts: &[Blast]) -> Waves {
        let mut w = Waves::default();
        for b in blasts {
            w.start(b.clone(), 0);
        }
        w
    }

    fn drain(w: &mut Waves, dim: &str) -> Vec<(u64, [i32; 2])> {
        std::iter::from_fn(|| w.next_job(dim)).map(|(b, c)| (b.id, c)).collect()
    }

    const OW: &str = "minecraft:overworld";

    #[test]
    fn the_front_reaches_250_blocks_in_100_ticks() {
        let mut w = waves_with(&[blast([7, 64, 0], 300, 1, World::OVERWORLD)]);
        let target = [16, 0]; // its nearest block, x = 256, is 249 blocks out
        w.advance(OW, 99, &|_| true);
        assert!(!drain(&mut w, OW).iter().any(|(_, c)| *c == target));
        w.advance(OW, 100, &|_| true);
        assert!(drain(&mut w, OW).iter().any(|(_, c)| *c == target));
    }

    #[test]
    fn chunks_come_out_nearest_first() {
        let mut w = waves_with(&[blast([0, 64, 0], 100, 1, World::OVERWORLD)]);
        w.advance(OW, 1000, &|_| true);
        let near = |c: [i32; 2]| {
            let x = 0f64.clamp(c[0] as f64 * 16.0, c[0] as f64 * 16.0 + 15.0);
            let z = 0f64.clamp(c[1] as f64 * 16.0, c[1] as f64 * 16.0 + 15.0);
            x.hypot(z)
        };
        let out: Vec<f64> = drain(&mut w, OW).into_iter().map(|(_, c)| near(c)).collect();
        assert!(out.len() > 100);
        assert!(out.windows(2).all(|p| p[0] <= p[1]), "{out:?}");
    }

    #[test]
    fn an_unloaded_chunk_waits_and_comes_back_once_when_loaded() {
        let mut w = waves_with(&[blast([0, 64, 0], 60, 1, World::OVERWORLD)]);
        let far = [3, 0];
        w.advance(OW, 1000, &|c| c != far);
        assert!(!drain(&mut w, OW).iter().any(|(_, c)| *c == far));
        assert!(!w.finished());
        w.chunk_loaded(OW, far);
        w.chunk_loaded(OW, far);
        assert_eq!(drain(&mut w, OW), vec![(1, far)]);
        w.advance(OW, 1001, &|_| true);
        assert!(w.finished());
    }

    #[test]
    fn two_overlapping_blasts_each_process_a_shared_chunk_once() {
        let a = blast([0, 64, 0], 60, 1, World::OVERWORLD);
        let mut b = blast([40, 64, 0], 60, 2, World::OVERWORLD);
        b.id = 2;
        let mut w = waves_with(&[a, b]);
        w.advance(OW, 1000, &|_| true);
        let jobs = drain(&mut w, OW);
        let shared = [1, 0];
        assert_eq!(jobs.iter().filter(|j| **j == (1, shared)).count(), 1);
        assert_eq!(jobs.iter().filter(|j| **j == (2, shared)).count(), 1);
        let unique: BTreeSet<_> = jobs.iter().collect();
        assert_eq!(unique.len(), jobs.len());
    }

    #[test]
    fn work_stops_when_the_budget_is_spent() {
        let mut w = waves_with(&[blast([0, 64, 0], 200, 1, World::OVERWORLD)]);
        w.advance(OW, 1000, &|_| true);
        let done = std::cell::Cell::new(0u32);
        let clock = || Duration::from_millis(3 * done.get() as u64);
        let n = w.work(OW, Duration::from_millis(10), &clock, &mut |_, _| {
            done.set(done.get() + 1);
            true
        });
        assert_eq!(n, 4);
    }

    #[test]
    fn a_chunk_that_unloads_before_its_turn_waits_for_its_load() {
        let mut w = waves_with(&[blast([0, 64, 0], 40, 1, World::OVERWORLD)]);
        w.advance(OW, 1000, &|_| true);
        let first = std::cell::Cell::new(None);
        w.work(OW, Duration::from_secs(1), &|| Duration::ZERO, &mut |_, c| {
            if first.get().is_none() {
                first.set(Some(c));
                return false;
            }
            true
        });
        let c = first.get().unwrap();
        assert!(!w.finished());
        w.chunk_loaded(OW, c);
        assert_eq!(drain(&mut w, OW), vec![(1, c)]);
    }

    #[test]
    fn a_wave_is_finished_only_when_nothing_is_left() {
        let mut w = waves_with(&[blast([0, 64, 0], 40, 1, World::OVERWORLD)]);
        assert!(!w.finished());
        w.advance(OW, 5, &|_| true);
        assert!(!w.finished(), "the front isn't out yet");
        w.advance(OW, 1000, &|_| true);
        assert!(!w.finished(), "jobs are waiting");
        drain(&mut w, OW);
        w.advance(OW, 1001, &|_| true);
        assert!(w.finished());
    }

    #[test]
    fn other_dimensions_are_left_alone() {
        let mut w = waves_with(&[blast([0, 64, 0], 40, 1, World::OVERWORLD)]);
        w.advance("minecraft:the_nether", 1000, &|_| true);
        assert!(drain(&mut w, "minecraft:the_nether").is_empty());
        w.advance(OW, 1000, &|_| true);
        assert!(drain(&mut w, "minecraft:the_nether").is_empty());
        assert!(!drain(&mut w, OW).is_empty());
    }

    #[test]
    fn waves_survive_a_round_trip_through_json() {
        let mut w = waves_with(&[blast([0, 64, 0], 200, 1, World::OVERWORLD)]);
        w.advance(OW, 40, &|c| c != [2, 2]);
        let done: Vec<_> = (0..5).filter_map(|_| w.next_job(OW)).map(|(_, c)| c).collect();
        let json = serde_json::to_string(&w).unwrap();
        let mut back: Waves = serde_json::from_str(&json).unwrap();
        let rest = drain(&mut w, OW);
        let rest_back = drain(&mut back, OW);
        assert_eq!(rest, rest_back);
        assert!(!rest_back.iter().any(|(_, c)| done.contains(c)));
        back.advance(OW, 1000, &|_| true);
        w.advance(OW, 1000, &|_| true);
        assert_eq!(drain(&mut w, OW), drain(&mut back, OW));
        back.chunk_loaded(OW, [2, 2]);
        assert_eq!(drain(&mut back, OW), vec![(1, [2, 2])]);
    }

    // ---- the command (Task 4) ----------------------------------------------

    #[test]
    fn the_blast_command_reads_a_radius_and_an_optional_place() {
        let at = [-0.5, 70.9, 10.2];
        assert_eq!(parse_blast(&["50"], at), Some((50, [-1, 70, 10])));
        assert_eq!(parse_blast(&["250", "-7", "64", "3"], at), Some((250, [-7, 64, 3])));
        assert_eq!(parse_blast(&["7"], at), None);
        assert_eq!(parse_blast(&["1025"], at), None);
        assert_eq!(parse_blast(&["50", "1", "2"], at), None);
        assert_eq!(parse_blast(&["fifty"], at), None);
        assert_eq!(parse_blast(&[], at), None);
    }

    #[test]
    fn blasts_know_their_dimension_and_get_their_own_ids() {
        let mut saved = Saved::default();
        assert_eq!(saved.blast("minecraft:overworld", [0, 64, 0], 50), Some(1));
        assert_eq!(saved.blast("minecraft:the_nether", [0, 64, 0], 50), Some(2));
        assert_eq!(saved.blast("mymod:moon", [0, 64, 0], 50), None);
        let json = serde_json::to_string(&saved).unwrap();
        let mut back: Saved = serde_json::from_str(&json).unwrap();
        assert_eq!(back.blast("minecraft:overworld", [0, 64, 0], 50), Some(3));
        back.waves.advance("minecraft:the_nether", 1000, &|_| true);
        let (b, _) = back.waves.next_job("minecraft:the_nether").unwrap();
        assert_eq!((b.id, b.world), (2, World::NETHER));
    }

    #[test]
    fn a_dimension_is_idle_once_its_fronts_are_out_and_its_loaded_work_done() {
        let mut w = waves_with(&[blast([0, 64, 0], 60, 1, World::OVERWORLD)]);
        let far = [3, 0];
        assert!(!w.idle(OW));
        w.advance(OW, 1000, &|c| c != far);
        assert!(!w.idle(OW), "jobs are ready");
        drain(&mut w, OW);
        assert!(w.idle(OW), "only a deferred chunk is left");
        assert!(!w.finished());
        assert!(w.idle("minecraft:the_nether"));
    }

    // ---- review fixes ---------------------------------------------------------

    #[test]
    fn advance_and_loads_tell_whether_anything_moved() {
        let mut w = waves_with(&[blast([0, 64, 0], 60, 1, World::OVERWORLD)]);
        let far = [3, 0];
        assert!(w.advance(OW, 1000, &|c| c != far));
        drain(&mut w, OW);
        assert!(!w.advance(OW, 1001, &|_| true), "the front is out and nothing new is reached");
        assert!(!w.chunk_loaded(OW, [40, 40]), "no work waits there");
        assert!(w.chunk_loaded(OW, far));
        assert!(!w.chunk_loaded(OW, far));
    }

    #[test]
    fn nether_blasts_keep_the_bedrock_roof() {
        let b = Blast { dimension: "minecraft:the_nether".into(), ..blast([0, 70, 0], 200, 4, World::NETHER) };
        for (_, r) in all_runs(&b).iter().filter(|(_, r)| r.surface.is_none()) {
            assert!(r.y + r.height as i32 <= 123, "{r:?} reaches the roof");
        }
        assert_eq!(world_of("minecraft:the_end").map(|w| w.roof), Some(256));
        assert_eq!(world_of("minecraft:overworld").map(|w| w.roof), Some(320));
    }

    #[test]
    fn a_wave_ends_with_its_last_job_without_another_advance() {
        let mut w = waves_with(&[blast([0, 64, 0], 40, 1, World::OVERWORLD)]);
        w.advance(OW, 1000, &|_| true);
        w.work(OW, Duration::from_secs(1), &|| Duration::ZERO, &mut |_, _| true);
        assert!(w.finished());
    }

    // ---- the blast's effects (plan 2, Task 1) -------------------------------------

    #[test]
    fn the_flash_is_white_near_fades_with_distance_and_time() {
        assert_eq!(flash_alpha(0.0, 10.0, 100), 255);
        assert_eq!(flash_alpha(0.0, 1200.0, 100), 0);
        assert_eq!(flash_alpha(80.0, 10.0, 100), 0);
        assert!(flash_alpha(20.0, 10.0, 100) >= 90, "a second on, the flash still glares");
        assert_eq!(flash_alpha(-1.0, 10.0, 100), 0);
        let by_time: Vec<u8> = (0..80).map(|t| flash_alpha(t as f64, 10.0, 100)).collect();
        assert!(by_time.windows(2).all(|p| p[0] >= p[1]));
        let by_dist: Vec<u8> = (0..130).map(|d| flash_alpha(2.0, d as f64 * 10.0, 100)).collect();
        assert!(by_dist.windows(2).all(|p| p[0] >= p[1]));
        assert!(flash_alpha(0.0, 600.0, 100) > 0 && flash_alpha(0.0, 600.0, 100) < 255);
    }

    #[test]
    fn sounds_come_with_the_front_and_skip_what_is_past() {
        let far = sounds_for(250.0, 100, 0);
        assert_eq!(far.iter().map(|s| (s.event, s.delay)).collect::<Vec<_>>(), vec![("big_nukes:blast.far", 100)]);
        let near = sounds_for(50.0, 100, 0);
        let events: Vec<_> = near.iter().map(|s| (s.event, s.delay)).collect();
        assert_eq!(events, vec![("big_nukes:blast.near", 20), ("big_nukes:wave.roar", 20)]);
        assert!(sounds_for(50.0, 100, 30).is_empty());
        assert_eq!(sounds_for(50.0, 100, 5)[0].delay, 15);
        assert!(sounds_for(4000.0, 100, 0).is_empty());
        assert_eq!(sounds_for(2900.0, 1000, 0)[0].delay, 1160);
        for d in [0.0, 50.0, 500.0, 2999.0] {
            for s in sounds_for(d, 100, 0) {
                assert!((0.2..=1.0).contains(&s.volume), "{d} {}", s.volume);
                assert!((0..=1200).contains(&s.delay));
            }
        }
    }

    #[test]
    fn the_shake_starts_with_the_front_and_dies_out() {
        let arrive = 100.0 / FRONT_SPEED;
        assert_eq!(shake(arrive - 1.0, 100.0, 200, 3), [0.0; 6]);
        assert_eq!(shake(arrive + 101.0, 100.0, 200, 3), [0.0; 6]);
        assert!(shake(arrive + 1.0, 100.0, 200, 3).iter().any(|v| *v != 0.0));
        assert_eq!(shake(10_000.0, 1100.0, 200, 3), [0.0; 6]);
        assert_eq!(shake(1100.0 / FRONT_SPEED + 1.0, 1100.0, 200, 3), [0.0; 6], "beyond 5.2R nothing shakes");
        for t in 0..400 {
            let v = shake(t as f64, 10.0, 200, 9);
            assert!(v[..3].iter().all(|a| a.abs() <= 6.0) && v[3..].iter().all(|o| o.abs() <= 0.3), "{v:?}");
        }
    }

    #[test]
    fn the_ring_follows_the_front() {
        let b = blast([10, 64, -20], 100, 5, World::OVERWORLD);
        let ring = front_ring(&b, 20);
        assert_eq!(ring.len(), 16);
        for p in &ring {
            let d = (p[0] - 10.0).hypot(p[2] + 20.0);
            assert!((d - 20.0 * FRONT_SPEED).abs() < 0.01, "{d}");
            assert_eq!(p[1], 66.0);
        }
        assert!(front_ring(&b, 44).len() == 16 && front_ring(&b, 45).is_empty(), "the reach is 112 blocks");
        assert!(front_ring(&b, 0).is_empty());
    }

    #[test]
    fn damage_bands_split_at_the_zones_and_stop_at_the_reach() {
        let bands = damage_band(100, 30.0, 40.0);
        assert_eq!(bands.len(), 2);
        assert_eq!((bands[0].0, bands[0].1, bands[0].2), (30.0, 35.0, 1000.0));
        assert_eq!((bands[1].0, bands[1].1), (35.0, 40.0));
        assert!(bands[1].2 <= 20.0 && bands[1].2 >= 6.0);
        assert!(damage_band(100, 113.0, 120.0).is_empty());
        let edge = damage_band(100, 105.0, 120.0);
        assert_eq!(edge.len(), 1);
        assert!((edge[0].1 - 112.0).abs() < 1e-9);
        assert!(edge[0].2 <= 4.0 && edge[0].2 >= 1.0);
        assert!(damage_band(100, 40.0, 40.0).is_empty());
    }

    #[test]
    fn news_round_trips_and_lists_only_running_blasts_of_the_dimension() {
        let mut w = Waves::default();
        w.start(blast([0, 64, 0], 100, 1, World::OVERWORLD), 10);
        let mut other = blast([0, 64, 0], 100, 2, World::NETHER);
        other.id = 2;
        other.dimension = "minecraft:the_nether".into();
        w.start(other, 10);
        let news = w.news(OW, 40);
        assert_eq!(news.len(), 1);
        assert_eq!((news[0].id, news[0].age, news[0].radius), (1, 30, 100));
        let back: News = serde_json::from_slice(&serde_json::to_vec(&news[0]).unwrap()).unwrap();
        assert_eq!(back, news[0]);
        assert!(w.news(OW, 10 + 2400).is_empty(), "long past");
    }

    #[test]
    fn debris_flies_out_and_up_and_is_capped() {
        let small = debris(&blast([0, 64, 0], 30, 1, World::OVERWORLD));
        assert_eq!(small.len(), 15);
        let big = debris(&blast([0, 64, 0], 1000, 1, World::OVERWORLD));
        assert_eq!(big.len(), 48);
        for (pos, v, block) in &big {
            assert_eq!(pos[1], 67.0);
            assert!(v[1] >= 0.9 && v[1] <= 1.6);
            let out = v[0].hypot(v[2]);
            assert!((0.8..=1.6).contains(&out), "{out}");
            assert!(block.starts_with("minecraft:"));
        }
    }

    // ---- assets of the effects (plan 2, Task 2) -----------------------------------

    #[test]
    fn every_sound_event_has_its_files() {
        let sounds: serde_json::Value = serde_json::from_str(&file("content/assets/big_nukes/sounds.json")).unwrap();
        let click = GEIGER_CLICK.strip_prefix("big_nukes:").unwrap();
        for event in ["blast.near", "blast.far", "wave.roar", click] {
            let entry = &sounds[event];
            assert!(entry.is_object(), "{event} isn't declared");
            for s in entry["sounds"].as_array().unwrap() {
                let name = s["name"].as_str().unwrap().strip_prefix("big_nukes:").unwrap();
                assert!(std::path::Path::new(&format!("{}/content/assets/big_nukes/sounds/{name}.ogg", env!("CARGO_MANIFEST_DIR"))).exists(), "{name}");
            }
        }
        for d in [10.0, 200.0, 2000.0] {
            for s in sounds_for(d, 100, 0) {
                assert!(sounds[s.event.strip_prefix("big_nukes:").unwrap()].is_object(), "{}", s.event);
            }
        }
    }


    #[test]
    fn fronts_report_the_band_passed_this_tick() {
        let mut w = Waves::default();
        w.start(blast([0, 64, 0], 100, 1, World::OVERWORLD), 10);
        let f = w.fronts(OW, 14);
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].0.id, f[0].1, f[0].2, f[0].3), (1, 7.5, 10.0, 4));
        assert_eq!(w.fronts(OW, 10)[0].1, 0.0, "the first tick starts at the center");
        assert!(w.fronts("minecraft:the_nether", 14).is_empty());
        assert!(w.fronts(OW, 10 + 46).is_empty(), "past the reach of 112 blocks");
    }

    // ---- plan 2 review fixes ---------------------------------------------------------

    fn heard_news(radius: u32) -> News {
        News { id: 1, center: [0, 64, 0], radius, seed: 3, age: 0 }
    }

    #[test]
    fn a_heard_blast_is_felt_only_in_its_own_dimension() {
        let h = Heard { news: heard_news(500), dimension: OW.into(), at: 100.0 };
        assert!(h.felt(OW, [125.0, 64.0, 0.0], 150.0).is_some());
        assert!(h.felt("minecraft:the_nether", [125.0, 64.0, 0.0], 150.0).is_none());
    }

    #[test]
    fn a_heard_blast_uses_the_fraction_of_the_tick() {
        let h = Heard { news: heard_news(100), dimension: OW.into(), at: 100.0 };
        let (t, _) = h.felt(OW, [0.0, 64.0, 0.0], 120.4).unwrap();
        assert!((t - 20.4).abs() < 1e-9);
    }

    #[test]
    fn a_heard_blast_is_let_go_when_past_or_from_another_world() {
        let h = Heard { news: heard_news(100), dimension: OW.into(), at: 1000.0 };
        assert!(h.felt(OW, [50.0, 64.0, 0.0], 1000.0 + 50.0 / FRONT_SPEED + 201.0).is_some(), "the cloud still stands");
        assert!(h.felt(OW, [50.0, 64.0, 0.0], 1000.0 + 5999.0).is_some());
        assert!(h.felt(OW, [50.0, 64.0, 0.0], 1000.0 + 6001.0).is_none(), "the cloud is gone at five minutes");
        let far = Heard { news: heard_news(100), dimension: OW.into(), at: 0.0 };
        assert!(far.felt(OW, [16000.0, 64.0, 0.0], 6300.0).is_some(), "the sound is still on its way");
        assert!(h.felt(OW, [50.0, 64.0, 0.0], 500.0).is_none(), "a clock far behind: another world");
    }

    #[test]
    fn a_small_wave_lives_until_its_front_is_past_the_reach() {
        let mut w = waves_with(&[blast([0, 64, 0], 8, 1, World::OVERWORLD)]);
        w.advance(OW, 2, &|_| true);
        w.work(OW, Duration::from_secs(1), &|| Duration::ZERO, &mut |_, _| true);
        assert!(!w.finished(), "the front is at 5 blocks of 8.96");
        assert!(!w.idle(OW));
        assert_eq!(w.fronts(OW, 3).len(), 1);
        w.advance(OW, 5, &|_| true);
        assert!(w.finished(), "the front is out at 12.5");
    }

    // ---- the mushroom cloud (plan 3, Task 2) ------------------------------------------

    #[test]
    fn the_fireball_rises_and_slows() {
        let h = |s: f64| cloud(s * 20.0, 200).ball_h;
        assert!(h(0.0) < 1.0);
        assert!(h(10.0) > 50.0 && h(10.0) < h(30.0) && h(30.0) < h(120.0));
        assert!(h(120.0) <= 0.9 * 200.0 + 60.0 + 0.01);
        assert!(h(20.0) - h(10.0) > h(60.0) - h(50.0), "it slows");
    }

    #[test]
    fn the_cap_grows_to_a_limit() {
        let c = |s: f64| cloud(s * 20.0, 200);
        assert!(c(10.0).cap_major < c(20.0).cap_major);
        assert!((c(600.0).cap_major - 1.4 * c(600.0).ball_r).abs() < 0.01);
        assert!((c(30.0).cap_minor - 0.8 * c(30.0).ball_r).abs() < 0.01);
        assert!((c(30.0).stem_r - 0.35 * c(30.0).ball_r).abs() < 0.01);
    }

    #[test]
    fn the_fire_dies_in_a_minute() {
        assert!((cloud(0.0, 100).fire - 1.0).abs() < 1e-6);
        assert!(cloud(60.0 * 20.0, 100).fire < 0.06);
    }

    #[test]
    fn the_cloud_fades_after_three_minutes() {
        assert_eq!(cloud(170.0 * 20.0, 100).fade, 1.0);
        let mid = cloud(240.0 * 20.0, 100).fade;
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(cloud(300.0 * 20.0, 100).fade, 0.0);
        assert_eq!(cloud(400.0 * 20.0, 100).fade, 0.0);
    }

    #[test]
    fn the_ring_runs_to_the_reach_then_fades() {
        assert!((cloud(20.0, 100).ring - 50.0).abs() < 1e-3);
        assert_eq!(cloud(20.0, 100).ring_fade, 1.0);
        let reach = 100.0 * 1.12;
        assert!((cloud(1000.0, 100).ring - reach).abs() < 1e-3);
        let out_at = reach / FRONT_SPEED as f32;
        assert!(cloud(out_at as f64 + 100.0, 100).ring_fade > 0.0);
        assert_eq!(cloud(out_at as f64 + 201.0, 100).ring_fade, 0.0);
    }

    #[test]
    fn a_young_cloud_is_finite_at_age_zero() {
        let c = cloud(0.0, 8);
        for v in [c.ball_h, c.ball_r, c.stem_r, c.cap_major, c.cap_minor, c.fire, c.fade, c.ring, c.ring_fade] {
            assert!(v.is_finite() && v >= 0.0);
        }
        assert!(cloud(-5.0, 100).fade == 0.0, "not yet");
    }

    // ---- the shader (plan 3, Task 3) ------------------------------------------------

    #[test]
    fn the_sky_shader_validates() {
        let source = file("mixins/big_nukes_native/sky.rs");
        let start = source.find("const SHADER: &str = r#\"").expect("sky.rs holds the shader") + "const SHADER: &str = r#\"".len();
        let end = start + source[start..].find("\"#;").expect("the shader ends");
        let module = naga::front::wgsl::parse_str(&source[start..end]).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source[start..end])));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{e:?}"));
        for entry in ["vs", "fs"] {
            assert!(module.entry_points.iter().any(|e| e.name == entry), "{entry}");
        }
    }

    #[test]
    fn the_newest_blasts_are_drawn_first() {
        let n = |id| News { id, center: [0, 64, 0], radius: 100, seed: 1, age: 0 };
        let mut felt = vec![(5000.0, 10.0, n(1)), (30.0, 10.0, n(5)), (4000.0, 10.0, n(2)), (900.0, 10.0, n(4)), (3000.0, 10.0, n(3))];
        newest_first(&mut felt);
        let ids: Vec<u64> = felt.iter().take(4).map(|f| f.2.id).collect();
        assert_eq!(ids, vec![5, 4, 3, 2]);
    }

    // ---- what the mod adds (plan 4, Task 1) ---------------------------------------------

    /// Blocks the content half declares in files: the ores and the ash.
    const FILE_BLOCKS: [&str; 3] = ["big_nukes:uranium_ore", "big_nukes:deepslate_uranium_ore", "big_nukes:ash"];

    fn name(id: &str) -> &str {
        id.strip_prefix("big_nukes:").unwrap()
    }

    fn exists(path: &str) -> bool {
        std::path::Path::new(&format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).exists()
    }

    /// A PNG's width and height, from its header.
    fn png_size(path: &str) -> (u32, u32) {
        let b = std::fs::read(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(&b[1..4], b"PNG", "{path}");
        let int = |at: usize| u32::from_be_bytes(b[at..at + 4].try_into().unwrap());
        (int(16), int(20))
    }

    #[test]
    fn every_block_and_item_is_declared_once_with_its_assets() {
        let code_blocks: Vec<&str> = [CENTRIFUGE, ASSEMBLY_TABLE].into_iter().chain(BombKind::ALL.map(BombKind::id)).collect();
        let blocks: Vec<&str> = code_blocks.iter().copied().chain(FILE_BLOCKS).collect();
        let items: Vec<&str> = ITEMS.iter().map(|i| i.0).collect();
        let all: BTreeSet<&str> = blocks.iter().chain(&items).copied().collect();
        assert_eq!(all.len(), blocks.len() + items.len(), "an id is declared twice");
        let en = language("en_us");
        let base = "content/assets/big_nukes";
        for b in &blocks {
            let n = name(b);
            for f in [format!("{base}/blockstates/{n}.json"), format!("{base}/items/{n}.json"), format!("content/data/big_nukes/loot_table/blocks/{n}.json")] {
                assert!(exists(&f), "{f}");
            }
            assert!(en.contains_key(&format!("block.big_nukes.{n}")), "{b} has no name");
        }
        for b in FILE_BLOCKS {
            let decl: serde_json::Value = serde_json::from_str(&file(&format!("content/data/big_nukes/horizon/block/{}.json", name(b)))).unwrap();
            assert!(decl["template"].as_str().unwrap().starts_with("minecraft:"), "{b}");
        }
        for i in &items {
            assert!(exists(&format!("{base}/items/{}.json", name(i))), "{i}");
            assert!(en.contains_key(&format!("item.big_nukes.{}", name(i))), "{i} has no name");
            // The item's model file is the one its item model names.
            let item: serde_json::Value = serde_json::from_str(&file(&format!("{base}/items/{}.json", name(i)))).unwrap();
            let model = item["model"]["model"].as_str().unwrap().strip_prefix("big_nukes:").unwrap();
            assert!(exists(&format!("{base}/models/{model}.json")), "{i}: {model}");
        }
        // The suit, as the player wears it: one equipment asset with both layers, 64x32 each.
        let equipment: serde_json::Value = serde_json::from_str(&file(&format!("{base}/equipment/hazmat.json"))).unwrap();
        for layer in ["humanoid", "humanoid_leggings"] {
            assert_eq!(equipment["layers"][layer], serde_json::json!([{"texture": HAZMAT_ASSET}]), "{layer}");
            assert_eq!(png_size(&format!("{base}/textures/entity/equipment/{layer}/hazmat.png")), (64, 32), "{layer}");
        }
        // The radiation's mark: a harmful effect with a name and an 18x18 icon.
        let effect: serde_json::Value = serde_json::from_str(&file("content/data/big_nukes/horizon/mob_effect/radiation.json")).unwrap();
        assert_eq!(effect["category"], "harmful");
        assert!(effect["color"].as_u64().unwrap() <= 0xFF_FFFF);
        assert_eq!(effect.as_object().unwrap().len(), 2, "a mob effect takes a category and a colour only");
        assert!(en.contains_key("effect.big_nukes.radiation"));
        assert_eq!(png_size(&format!("{base}/textures/mob_effect/radiation.png")), (18, 18));
        let pickaxe: serde_json::Value = serde_json::from_str(&file("content/data/minecraft/tags/block/mineable/pickaxe.json")).unwrap();
        // Entries of blocks declared in code are optional: the assets half alone must still load.
        let mined: Vec<&str> = pickaxe["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap_or_else(|| {
                assert_eq!(v["required"], false, "{v}");
                v["id"].as_str().unwrap()
            }))
            .collect();
        for v in pickaxe["values"].as_array().unwrap() {
            let id = v.as_str().or(v["id"].as_str()).unwrap();
            if code_blocks.contains(&id) {
                assert_eq!(v["required"], false, "{id} must be optional");
            }
        }
        for b in blocks.iter().filter(|b| **b != "big_nukes:ash") {
            assert!(mined.contains(b), "{b} isn't mined with a pickaxe");
        }
        let shovel: serde_json::Value = serde_json::from_str(&file("content/data/minecraft/tags/block/mineable/shovel.json")).unwrap();
        assert_eq!(shovel["values"], serde_json::json!(["big_nukes:ash"]));
        // Every texture a model names is drawn.
        for dir in ["block", "item"] {
            for e in std::fs::read_dir(format!("{}/{base}/models/{dir}", env!("CARGO_MANIFEST_DIR"))).unwrap() {
                let model: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(e.unwrap().path()).unwrap()).unwrap();
                for t in model["textures"].as_object().unwrap().values() {
                    if let Some(t) = t.as_str().and_then(|t| t.strip_prefix("big_nukes:")) {
                        assert!(exists(&format!("{base}/textures/{t}.png")), "{t}.png");
                    }
                }
            }
        }
    }

    // ---- recipes (plan 4, Task 4) -----------------------------------------------------

    #[test]
    fn every_recipe_reads_and_names_known_items() {
        let known: BTreeSet<&str> = ITEMS
            .iter()
            .map(|i| i.0)
            .chain(BombKind::ALL.map(BombKind::id))
            .chain([CENTRIFUGE, ASSEMBLY_TABLE])
            .collect();
        let known_id = |id: &str| id.starts_with("minecraft:") || known.contains(id);
        let dir = format!("{}/content/data/big_nukes/recipe", env!("CARGO_MANIFEST_DIR"));
        let mut made = BTreeSet::new();
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            let file_name = path.file_stem().unwrap().to_str().unwrap().to_owned();
            let recipe: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|e| panic!("{file_name}: {e}"));
            let ingredients: Vec<&serde_json::Value> = match recipe["type"].as_str().unwrap() {
                "minecraft:crafting_shaped" => {
                    let pattern: Vec<&str> = recipe["pattern"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
                    assert!((1..=3).contains(&pattern.len()), "{file_name}: {} rows", pattern.len());
                    assert!(pattern.iter().all(|r| r.len() == pattern[0].len() && r.len() <= 3), "{file_name}: uneven rows");
                    let key = recipe["key"].as_object().unwrap();
                    let used: BTreeSet<char> = pattern.iter().flat_map(|r| r.chars()).filter(|c| *c != ' ').collect();
                    let keyed: BTreeSet<char> = key.keys().map(|k| k.chars().next().unwrap()).collect();
                    assert_eq!(used, keyed, "{file_name}: the pattern and the key differ");
                    key.values().collect()
                }
                "minecraft:crafting_shapeless" => {
                    let list: Vec<&serde_json::Value> = recipe["ingredients"].as_array().unwrap().iter().collect();
                    assert!((1..=9).contains(&list.len()), "{file_name}: {} ingredients", list.len());
                    list
                }
                other => panic!("{file_name}: {other} isn't a crafting table recipe"),
            };
            for id in ingredients {
                let id = id.as_str().unwrap_or_else(|| panic!("{file_name}: {id} isn't an item id"));
                assert!(known_id(id), "{file_name}: {id} is unknown");
            }
            let result = recipe["result"]["id"].as_str().unwrap();
            assert!(known_id(result), "{file_name}: {result} is unknown");
            assert_eq!(result, format!("big_nukes:{file_name}"), "the recipe is named after its result");
            assert!(recipe["result"]["count"].as_u64().unwrap() >= 1, "{file_name}");
            made.insert(result.to_owned());
        }
        // The bombs and their parts come only from the assembly table.
        for a in &ASSEMBLIES {
            assert!(!made.contains(a.result), "{} has a crafting table recipe too", a.result);
            made.insert(a.result.to_owned());
        }
        // Mined, or made in the centrifuge.
        let not_crafted = ["big_nukes:raw_uranium", "big_nukes:enriched_uranium"];
        let crafted: BTreeSet<String> = known.iter().filter(|id| !not_crafted.contains(id)).map(|id| id.to_string()).collect();
        assert_eq!(made, crafted);
    }

    // ---- the radiation's items (plan 5, Task 3) ------------------------------------------

    fn stack_of(id: &str) -> u8 {
        ITEMS.iter().find(|i| i.0 == id).unwrap_or_else(|| panic!("{id} isn't declared")).1
    }

    #[test]
    fn the_suit_is_four_wearable_pieces() {
        let slots: Vec<&str> = HAZMAT.iter().map(|p| p.1).collect();
        assert_eq!(slots, ["head", "chest", "legs", "feet"]);
        for (id, slot, template) in HAZMAT {
            let piece = name(id).strip_prefix("hazmat_").unwrap();
            assert_eq!(template, format!("minecraft:iron_{piece}"));
            assert_eq!(super::template(id), template);
            assert_eq!(stack_of(id), 1, "{id}");
            let c = components(id);
            assert_eq!(
                c["minecraft:equippable"],
                serde_json::json!({"slot": slot, "asset_id": HAZMAT_ASSET, "equip_sound": "minecraft:item.armor.equip_leather"}),
                "{id}"
            );
            assert_eq!(c["minecraft:max_damage"], 600, "{id}");
            assert_eq!(c["!minecraft:repairable"], serde_json::json!({}), "{id}: it isn't mended with iron");
            assert_eq!(
                c["minecraft:attribute_modifiers"],
                serde_json::json!([{"type": "minecraft:armor", "amount": 1.0, "operation": "add_value",
                    "id": format!("minecraft:armor.{piece}"), "slot": slot}]),
                "{id}"
            );
        }
    }

    #[test]
    fn the_anti_rad_is_a_drink_with_our_effect() {
        assert_eq!(super::template(ANTI_RAD), "minecraft:paper");
        assert_eq!(stack_of(ANTI_RAD), 16);
        let c = components(ANTI_RAD);
        let drink = &c["minecraft:consumable"];
        assert_eq!(drink["animation"], "drink");
        assert_eq!(drink["consume_seconds"], 1.6);
        assert_eq!(drink["sound"], "minecraft:entity.generic.drink");
        assert_eq!(drink["has_consume_particles"], false);
        let effects = drink["on_consume_effects"].as_array().unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0]["type"], ANTI_RADIATION);
        assert_eq!(anti_rad_amount(&effects[0]), Some(300.0));
        assert_eq!(c["minecraft:use_remainder"], serde_json::json!({"id": "minecraft:glass_bottle", "count": 1}));
        // The hook reads only a positive amount.
        assert_eq!(anti_rad_amount(&serde_json::json!({"type": ANTI_RADIATION})), None);
        assert_eq!(anti_rad_amount(&serde_json::json!({"type": ANTI_RADIATION, "amount": -5})), None);
    }

    #[test]
    fn the_geiger_counter_is_one_to_a_hand() {
        assert_eq!(super::template(GEIGER_COUNTER), "minecraft:paper");
        assert_eq!(stack_of(GEIGER_COUNTER), 1);
        assert!(components(GEIGER_COUNTER).is_empty());
    }

    /// What the game checks of an item's patch: namespaced types, removals as `{}`, and the
    /// stack size and rarity as the item's own fields.
    #[test]
    fn every_item_patch_is_one_the_game_takes() {
        for (id, _, _) in ITEMS {
            for (key, value) in components(id) {
                let kind = key.strip_prefix('!').unwrap_or(&key);
                assert!(kind.split_once(':').is_some_and(|(ns, path)| !ns.is_empty() && !path.is_empty()), "{id}: {key}");
                assert!(!["minecraft:max_stack_size", "minecraft:rarity"].contains(&kind), "{id}: {key}");
                if key.starts_with('!') {
                    assert_eq!(value, serde_json::json!({}), "{id}: {key}");
                }
            }
        }
    }

    // ---- the assembly table -------------------------------------------------------------

    /// A grid of 9 slots from (slot, item, count).
    fn grid<'a>(stacks: &[(usize, &'a str, u32)]) -> Vec<Option<(&'a str, u32)>> {
        let mut g = vec![None; GRID_SLOTS];
        for &(slot, item, count) in stacks {
            g[slot] = Some((item, count));
        }
        g
    }

    fn most(_: &str) -> u32 {
        16
    }

    const TNT: &str = "minecraft:tnt";
    const URANIUM: &str = "big_nukes:enriched_uranium";
    const CHARGE: &str = "big_nukes:nuclear_charge";

    #[test]
    fn assembly_recipes_name_known_items() {
        let known: BTreeSet<&str> = ITEMS.iter().map(|i| i.0).chain(BombKind::ALL.map(BombKind::id)).collect();
        let results: BTreeSet<&str> = ASSEMBLIES.iter().map(|a| a.result).collect();
        let wanted: BTreeSet<&str> = [CHARGE, "big_nukes:bomb_casing"].into_iter().chain(BombKind::ALL.map(BombKind::id)).collect();
        assert_eq!(results, wanted);
        assert_eq!(results.len(), ASSEMBLIES.len(), "a result is made twice");
        let en = language("en_us");
        let mut sets = BTreeSet::new();
        for a in &ASSEMBLIES {
            assert!(en.contains_key(&name_key(a.result)), "{} has no name", a.result);
            let ids: BTreeSet<&str> = a.parts.iter().map(|p| p.0).collect();
            assert_eq!(ids.len(), a.parts.len(), "{}: a part is named twice", a.result);
            for &(id, count) in a.parts {
                assert!(id.starts_with("minecraft:") || known.contains(id), "{}: {id} is unknown", a.result);
                assert!(count > 0, "{}: {id}", a.result);
            }
            // Recipes match by their set of items, so no two may share one.
            assert!(sets.insert(ids), "{}: another recipe has the same items", a.result);
        }
    }

    #[test]
    fn a_grid_with_the_right_items_assembles() {
        let exact = grid(&[(0, URANIUM, 1), (1, TNT, 2), (5, TNT, 2)]);
        assert_eq!(assembly_in(&exact).map(|i| ASSEMBLIES[i].result), Some(CHARGE));
        let done = assemble(&exact, None, &most).unwrap();
        assert_eq!(done.result, CHARGE);
        let more = grid(&[(0, URANIUM, 3), (8, TNT, 64)]);
        assert_eq!(assemble(&more, Some((CHARGE, 3)), &most).map(|d| d.result), Some(CHARGE));
        let atomic = grid(&[(0, CHARGE, 4), (1, "big_nukes:bomb_casing", 4), (4, "minecraft:redstone_block", 1)]);
        assert_eq!(assemble(&atomic, None, &most).map(|d| d.result), Some("big_nukes:atomic_bomb"));
        assert_eq!(assembly_data(&atomic), 1 + ASSEMBLIES.iter().position(|a| a.result == "big_nukes:atomic_bomb").unwrap() as i32);
        assert_eq!(assembly_of_data(assembly_data(&atomic)).map(|a| a.result), Some("big_nukes:atomic_bomb"));
    }

    #[test]
    fn foreign_items_assemble_nothing() {
        let stick = grid(&[(0, URANIUM, 1), (1, TNT, 4), (2, "minecraft:stick", 1)]);
        assert_eq!(assembly_in(&stick), None);
        assert!(assemble(&stick, None, &most).is_none());
        let short = grid(&[(0, URANIUM, 1), (1, TNT, 3)]);
        assert!(assemble(&short, None, &most).is_none());
        assert!(assemble(&grid(&[]), None, &most).is_none());
        assert_eq!(assembly_data(&grid(&[])), 0);
        assert!(assembly_of_data(0).is_none());
        assert!(assembly_of_data(99).is_none());
    }

    #[test]
    fn assembling_takes_only_what_the_recipe_needs() {
        let g = grid(&[(2, TNT, 3), (4, URANIUM, 2), (7, TNT, 3)]);
        let done = assemble(&g, None, &most).unwrap();
        let mut take = done.take.clone();
        take.sort();
        assert_eq!(take, vec![(2, 3), (4, 1), (7, 1)]);
    }

    #[test]
    fn a_full_output_refuses() {
        let g = grid(&[(0, URANIUM, 1), (1, TNT, 4)]);
        assert!(assemble(&g, Some((CHARGE, 16)), &most).is_none());
        assert!(assemble(&g, Some(("minecraft:stick", 1)), &most).is_none());
        assert!(assemble(&g, Some((CHARGE, 15)), &most).is_some());
    }

    #[test]
    fn uranium_ore_is_generated_deep_and_rare() {
        let feature: serde_json::Value = serde_json::from_str(&file("content/data/big_nukes/worldgen/configured_feature/ore_uranium.json")).unwrap();
        let states: Vec<&str> = feature["targets"].as_array().unwrap().iter().map(|t| t["state"].as_str().unwrap()).collect();
        assert_eq!(states, ["big_nukes:uranium_ore", "big_nukes:deepslate_uranium_ore"]);
        let placed: serde_json::Value = serde_json::from_str(&file("content/data/big_nukes/worldgen/placed_feature/ore_uranium.json")).unwrap();
        let p = placed["placement"].as_array().unwrap();
        assert_eq!(p[0]["count"], 2);
        assert_eq!(p[2]["height"]["max_inclusive"]["absolute"], 16);
        let modifier: serde_json::Value = serde_json::from_str(&file("content/data/big_nukes/neoforge/biome_modifier/add_uranium.json")).unwrap();
        assert_eq!((modifier["features"].as_str(), modifier["step"].as_str()), (Some("big_nukes:ore_uranium"), Some("underground_ores")));
    }

    // ---- the centrifuge (plan 4, Task 3) -------------------------------------

    const COAL: Option<u32> = Some(1600);

    #[test]
    fn no_fuel_no_spin() {
        let mut c = Centrifuge::default();
        for _ in 0..20 {
            assert_eq!(c.step(5, None, true), Step::IDLE);
            assert_eq!(c.step(5, Some(0), true), Step::IDLE, "what doesn't burn isn't fuel");
        }
        assert_eq!(c, Centrifuge::default());
        // Fuel but nothing to spin: the fuel stays in its slot.
        assert_eq!(c.step(0, COAL, true), Step::IDLE);
        assert_eq!(c, Centrifuge::default());
    }

    #[test]
    fn fuel_burns_down_and_refills() {
        let mut c = Centrifuge::default();
        assert_eq!(c.step(3, Some(10), true), Step { took_fuel: true, made: false });
        assert_eq!(c, Centrifuge { burn: 9, burn_max: 10, spin: 1 });
        for left in (0..9).rev() {
            assert_eq!(c.step(3, Some(10), true), Step { took_fuel: false, made: false });
            assert_eq!(c.burn, left);
        }
        // Burnt out: the next tick takes one more fuel.
        assert_eq!(c.step(3, Some(7), true), Step { took_fuel: true, made: false });
        assert_eq!(c, Centrifuge { burn: 6, burn_max: 7, spin: 11 });
        // Out of fuel: it stops where it is.
        c.burn = 0;
        assert_eq!(c.step(3, None, true), Step::IDLE);
        assert_eq!(c.spin, 11);
    }

    #[test]
    fn at_spin_max_input_becomes_output() {
        let mut c = Centrifuge::default();
        let mut made = 0;
        for tick in 1..=SPIN_TICKS {
            let step = c.step(1, COAL, true);
            assert_eq!(step.took_fuel, tick == 1);
            if step.made {
                made += 1;
                assert_eq!(tick, SPIN_TICKS);
            }
        }
        assert_eq!(made, 1);
        assert_eq!(c.spin, 0);
        // A coal's burn is four enrichments.
        for _ in 0..3 * SPIN_TICKS {
            assert!(!c.step(1, COAL, true).took_fuel);
        }
        assert_eq!(c.burn, 0);
        // The last tick of the fuel and the first of the next one can coincide with an output.
        let mut c = Centrifuge { burn: 0, burn_max: 1600, spin: SPIN_TICKS - 1 };
        assert_eq!(c.step(1, COAL, true), Step { took_fuel: true, made: true });
        // Taking the input out loses the progress, as a furnace does.
        let mut c = Centrifuge { burn: 50, burn_max: 100, spin: 200 };
        assert_eq!(c.step(0, COAL, true), Step::IDLE);
        assert_eq!(c, Centrifuge { burn: 50, burn_max: 100, spin: 0 });
    }

    #[test]
    fn a_full_output_waits() {
        let mut c = Centrifuge { burn: 30, burn_max: 100, spin: 250 };
        for _ in 0..50 {
            assert_eq!(c.step(4, COAL, false), Step::IDLE);
        }
        // Neither the progress nor the burning fuel is lost while it waits.
        assert_eq!(c, Centrifuge { burn: 30, burn_max: 100, spin: 250 });
        let mut c = Centrifuge::default();
        assert_eq!(c.step(4, COAL, false), Step::IDLE, "no fuel is taken for nothing");
        assert_eq!(c, Centrifuge::default());
    }

    #[test]
    fn state_round_trips() {
        let c = Centrifuge { burn: 1234, burn_max: 1600, spin: 321 };
        let back: Centrifuge = serde_json::from_slice(&serde_json::to_vec(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        // What an older or empty entity holds reads as a cold centrifuge.
        assert_eq!(serde_json::from_str::<Centrifuge>("{}").unwrap(), Centrifuge::default());
    }

    #[test]
    fn the_screen_shows_the_fire_and_the_spin() {
        let c = Centrifuge { burn: 800, burn_max: 1600, spin: 100 };
        assert_eq!(c.data(), [800, 1600, 100, SPIN_TICKS as i32]);
        // Data slots are 16 bits on the wire.
        let hot = Centrifuge { burn: 40_000, burn_max: 40_000, spin: 0 };
        assert_eq!(hot.data()[..2], [i32::from(i16::MAX); 2]);
        assert_eq!(flame_height(0, 0), 0);
        assert_eq!(flame_height(1600, 1600), FLAME_HEIGHT);
        assert_eq!(flame_height(1, 1600), 1, "a spark still shows");
        assert_eq!(flame_height(800, 1600), FLAME_HEIGHT / 2);
        assert_eq!(arrow_width(0, SPIN_TICKS as i32), 0);
        assert_eq!(arrow_width(200, SPIN_TICKS as i32), ARROW_WIDTH / 2);
        assert_eq!(arrow_width(400, SPIN_TICKS as i32), ARROW_WIDTH);
        assert_eq!(arrow_width(5, 0), 0, "before the server's first values");
    }

    // ---- the bombs (plan 4, Task 5) ------------------------------------------

    const OVERWORLD: &str = "minecraft:overworld";

    /// A bomb with a keypad, its code `code` set when it isn't empty.
    fn keypad(code: &str) -> Bomb {
        let mut b = Bomb::new(BombKind::Atomic);
        b.install(Detonator::Keypad).unwrap();
        if !code.is_empty() {
            type_in(&mut b, code, 0);
            assert_eq!(b.press(Key::Enter, 0), Press::CodeSet);
        }
        b
    }

    fn type_in(b: &mut Bomb, digits: &str, now: u64) {
        for d in digits.bytes() {
            assert_eq!(b.press(Key::Digit(d - b'0'), now), Press::Typed, "{digits}");
        }
    }

    #[test]
    fn bombs_know_their_radius() {
        assert_eq!(BombKind::ALL.map(BombKind::radius), [16, 50, 250]);
        for k in BombKind::ALL {
            assert_eq!(BombKind::of_block(k.id()), Some(k));
            assert!((MIN_RADIUS..=MAX_RADIUS).contains(&k.radius()), "{k:?}");
        }
        assert_eq!(BombKind::of_block(CENTRIFUGE), None);
    }

    #[test]
    fn a_detonator_goes_in_once() {
        let mut b = Bomb::new(BombKind::Atomic);
        assert_eq!(b.detonator, None);
        assert_eq!(b.install(Detonator::Keypad), Ok(()));
        for d in Detonator::ALL {
            assert_eq!(b.install(d), Err(Refusal::HasOne), "{d:?}");
        }
        assert_eq!(b.detonator, Some(Detonator::Keypad));
        // Each detonator is one of the mod's items, and only those are.
        for d in Detonator::ALL {
            assert_eq!(Detonator::of_item(d.item()), Some(d));
            assert!(ITEMS.iter().any(|i| i.0 == d.item()), "{d:?}");
        }
        assert_eq!(Detonator::of_item(FLINT_AND_STEEL), None);
        assert_eq!(Detonator::of_item(RAW_URANIUM), None);
        // A bomb without a detonator never goes off.
        let mut bare = Bomb::new(BombKind::Tactical);
        assert!(!bare.light(0));
        assert_eq!(bare.press(Key::Digit(1), 0), Press::Ignored);
        assert_eq!(bare.press(Key::Enter, 0), Press::Ignored);
        assert!((0..1000).all(|t| !bare.tick(t)));
    }

    #[test]
    fn the_demolition_charge_takes_only_a_fuse() {
        for d in [Detonator::Keypad, Detonator::Receiver] {
            let mut b = Bomb::new(BombKind::Demolition);
            assert_eq!(b.install(d), Err(Refusal::FuseOnly), "{d:?}");
            assert_eq!(b.detonator, None);
        }
        assert_eq!(Bomb::new(BombKind::Demolition).install(Detonator::Fuse), Ok(()));
        for kind in [BombKind::Tactical, BombKind::Atomic] {
            for d in Detonator::ALL {
                assert_eq!(Bomb::new(kind).install(d), Ok(()), "{kind:?} {d:?}");
            }
        }
    }

    #[test]
    fn the_fuse_counts_down_to_the_blast() {
        let mut b = Bomb::new(BombKind::Tactical);
        b.install(Detonator::Fuse).unwrap();
        assert_eq!(b.left(100), None);
        assert!(b.light(100));
        assert!(!b.light(101), "a lit fuse isn't lit again");
        assert_eq!(b.left(100), Some(FUSE_TICKS));
        assert_eq!(FUSE_TICKS, 80);
        for t in 100..180 {
            assert!(!b.tick(t), "{t}");
        }
        assert!(b.tick(180));
        assert_eq!(b.fires_at, None);
        assert!(!b.tick(181), "it goes off once");
        // Only a fuse is lit.
        let mut k = keypad("");
        assert!(!k.light(0));
        assert_eq!(k.fires_at, None);
        // Each lighting wears the flint and steel; its last use breaks it.
        assert_eq!(worn(0), Some(1));
        assert_eq!(worn(FLINT_AND_STEEL_USES - 2), Some(FLINT_AND_STEEL_USES - 1));
        assert_eq!(worn(FLINT_AND_STEEL_USES - 1), None);
    }

    #[test]
    fn a_code_has_at_least_three_digits() {
        let mut b = keypad("");
        type_in(&mut b, "12", 0);
        assert_eq!(b.press(Key::Enter, 0), Press::TooShort);
        // The digits stay, to go on typing.
        assert_eq!((b.code.as_str(), b.entry.as_str()), ("", "12"));
        type_in(&mut b, "3", 0);
        assert_eq!(b.press(Key::Enter, 0), Press::CodeSet);
        assert_eq!(b.code, "123");
        assert_eq!(CODE_MIN_DIGITS, 3);
    }

    #[test]
    fn the_first_code_is_set_then_checked() {
        let mut b = keypad("");
        assert_eq!(b.data(0), [PAD_SET, 0, 0, TRIES as i32]);
        assert_eq!(b.press(Key::Enter, 0), Press::Ignored, "an empty entry sets nothing");
        type_in(&mut b, "1234", 0);
        assert_eq!(b.data(0)[..2], [PAD_SET, 4]);
        assert_eq!(b.press(Key::Enter, 0), Press::CodeSet);
        assert_eq!((b.code.as_str(), b.entry.as_str(), b.fires_at), ("1234", "", None));
        assert_eq!(b.data(0), [PAD_ENTER, 0, 0, TRIES as i32]);
        // A wrong code counts, and doesn't go off.
        type_in(&mut b, "9", 1);
        assert_eq!(b.press(Key::Enter, 1), Press::Wrong);
        assert_eq!((b.tries, b.fires_at), (1, None));
        assert_eq!(b.data(1), [PAD_ENTER, 0, 0, TRIES as i32 - 1]);
        // Clear empties the entry.
        type_in(&mut b, "12", 2);
        assert_eq!(b.press(Key::Clear, 2), Press::Cleared);
        assert_eq!(b.entry, "");
        // The right code starts the countdown and forgets the wrong ones.
        type_in(&mut b, "1234", 10);
        assert_eq!(b.press(Key::Enter, 10), Press::Accepted);
        assert_eq!((b.tries, b.fires_at), (0, Some(10 + COUNTDOWN_TICKS)));
        assert_eq!(COUNTDOWN_TICKS, 100);
        assert_eq!(b.data(10), [PAD_COUNTING, 0, 5, TRIES as i32]);
        assert_eq!(b.data(81), [PAD_COUNTING, 0, 2, TRIES as i32], "seconds round up");
        // Presses while it counts are ignored.
        assert_eq!(b.press(Key::Digit(1), 11), Press::Ignored);
        assert_eq!(b.press(Key::Clear, 11), Press::Ignored);
        assert!((10..110).all(|t| !b.tick(t)));
        assert!(b.tick(110));
        // A code has at most 8 digits.
        let mut c = keypad("");
        type_in(&mut c, "12345678", 0);
        assert_eq!(c.press(Key::Digit(9), 0), Press::Ignored);
        assert_eq!(c.entry.len(), CODE_DIGITS);
        // The screen's buttons: the digits, then clear and enter.
        assert_eq!(Key::of_button(0), Some(Key::Digit(0)));
        assert_eq!(Key::of_button(9), Some(Key::Digit(9)));
        assert_eq!(Key::of_button(10), Some(Key::Clear));
        assert_eq!(Key::of_button(11), Some(Key::Enter));
        assert_eq!(Key::of_button(12), None);
        assert_eq!(Key::of_button(-1), None);
    }

    #[test]
    fn three_wrong_codes_lock_the_pad() {
        let mut b = keypad("420");
        for i in 1..TRIES {
            type_in(&mut b, "1", 5);
            assert_eq!(b.press(Key::Enter, 5), Press::Wrong);
            assert_eq!(b.tries, i);
        }
        type_in(&mut b, "1", 5);
        assert_eq!(b.press(Key::Enter, 5), Press::Locked);
        assert_eq!((b.tries, b.locked_until, b.fires_at), (0, 5 + LOCK_TICKS, None));
        assert_eq!(LOCK_TICKS, 600);
        assert_eq!(b.data(5), [PAD_LOCKED, 0, 30, TRIES as i32]);
        // Nothing is typed while it's locked, not even the right code.
        assert_eq!(b.press(Key::Digit(4), 604), Press::Ignored);
        assert_eq!(b.press(Key::Enter, 604), Press::Ignored);
        assert_eq!(b.entry, "");
        type_in(&mut b, "420", 605);
        assert_eq!(b.press(Key::Enter, 605), Press::Accepted);
        // A right code resets the count of wrong ones.
        let mut c = keypad("420");
        for _ in 1..TRIES {
            type_in(&mut c, "7", 0);
            assert_eq!(c.press(Key::Enter, 0), Press::Wrong);
        }
        type_in(&mut c, "420", 0);
        assert_eq!(c.press(Key::Enter, 0), Press::Accepted);
        assert_eq!(c.tries, 0);
        // A lock or a count from a clock that went back is cut to its length.
        let mut d = keypad("420");
        d.locked_until = 1_000_000;
        d.tick(700);
        assert_eq!(d.locked_until, 700 + LOCK_TICKS);
        d.fires_at = Some(1_000_000);
        d.tick(700);
        assert_eq!(d.fires_at, Some(700 + COUNTDOWN_TICKS));
    }

    #[test]
    fn a_bomb_over_maxpower_refuses() {
        assert_eq!(Saved::default().maxpower, MAX_RADIUS);
        let mut saved = Saved { maxpower: 50, ..Saved::default() };
        let atomic = BombKind::Atomic.radius();
        assert!(!saved.allows(atomic));
        assert!(saved.allows(BombKind::Tactical.radius()));
        assert_eq!(saved.fire(OVERWORLD, [0, 64, 0], atomic), Err(Refused::Maxpower { radius: 250, maxpower: 50 }));
        assert!(saved.waves.finished(), "no wave started");
        assert_eq!(saved.fire(OVERWORLD, [0, 64, 0], BombKind::Tactical.radius()), Ok(1));
        assert_eq!(saved.fire("mymod:moon", [0, 64, 0], 16), Err(Refused::Dimension));
        // A world saved before the limit existed gets the largest radius.
        let mut json = serde_json::to_value(Saved::default()).unwrap();
        json.as_object_mut().unwrap().remove("maxpower");
        let old: Saved = serde_json::from_value(json).unwrap();
        assert_eq!(old.maxpower, MAX_RADIUS);
    }

    #[test]
    fn bomb_state_round_trips() {
        let b = Bomb {
            kind: BombKind::Atomic,
            detonator: Some(Detonator::Keypad),
            code: "1234".into(),
            entry: "12".into(),
            tries: 2,
            locked_until: 700,
            fires_at: Some(900),
        };
        let back: Bomb = serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
        assert_eq!(back, b);
        // An empty entity reads as a bomb with nothing in it.
        let empty: Bomb = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, Bomb::default());
        assert_eq!((empty.detonator, empty.fires_at), (None, None));
    }

    // ---- the remote and the operator's limit (Task 6) -------------------------

    const NETHER: &str = "minecraft:the_nether";

    #[test]
    fn a_blasts_center_is_found_in_its_own_chunk() {
        let mut saved = Saved::default();
        // A bomb on the Nether's roof: its crater stops below the roof, so the
        // glue takes the bomb when the wave works the chunk of its center.
        let roof_bomb = [40, 126, -20];
        assert!(saved.record(NETHER, roof_bomb, BombKind::Atomic));
        assert_eq!(saved.fire_bomb(NETHER, roof_bomb), Ok(1));
        assert_eq!(saved.waves.centers_in(NETHER, chunk_of(40, -20)), vec![roof_bomb]);
        assert!(saved.waves.centers_in(NETHER, chunk_of(56, -20)).is_empty(), "another chunk");
        assert!(saved.waves.centers_in(OVERWORLD, chunk_of(40, -20)).is_empty(), "another dimension");
    }

    #[test]
    fn a_blast_sets_off_the_bombs_it_reaches() {
        let mut saved = Saved::default();
        let center = [0, 64, 0];
        assert!(saved.place(OVERWORLD, center, BombKind::Atomic));
        assert!(!saved.place(OVERWORLD, center, BombKind::Atomic), "a bomb is known once");
        let near = [10, 64, 3];
        assert!(saved.place(OVERWORLD, near, BombKind::Tactical));
        assert!(saved.place(OVERWORLD, [60, 64, 0], BombKind::Demolition));
        assert!(saved.place(NETHER, [10, 64, 4], BombKind::Tactical));
        let id = saved.fire(OVERWORLD, center, 50).unwrap();
        let blast = saved.waves.waves.iter().find(|w| w.blast.id == id).unwrap().blast.clone();
        // The chunk of the center: its own bomb is the blast, the other goes off.
        assert_eq!(reached(&saved.placed, &blast, chunk_of(0, 0)), vec![(near, BombKind::Tactical)]);
        // Beyond the radius, and in another dimension, nothing goes off.
        assert!(reached(&saved.placed, &blast, chunk_of(60, 0)).is_empty());
        // A bomb set off goes off once, within the operator's limit.
        assert!(saved.record(OVERWORLD, near, BombKind::Tactical));
        assert_eq!(saved.set_off(OVERWORLD, near), Ok(2));
        assert!(reached(&saved.placed, &blast, chunk_of(0, 0)).is_empty());
        assert_eq!(saved.recorded(OVERWORLD, near), None, "its remote finds it gone");
        assert_eq!(saved.set_off(OVERWORLD, near), Err(Refused::Gone));
        saved.maxpower = 16;
        assert!(saved.place(OVERWORLD, [12, 64, 0], BombKind::Atomic));
        assert_eq!(saved.set_off(OVERWORLD, [12, 64, 0]), Err(Refused::Maxpower { radius: 250, maxpower: 16 }));
        assert_eq!(reached(&saved.placed, &blast, chunk_of(0, 0)), vec![([12, 64, 0], BombKind::Atomic)], "it stays");
        // A bomb taken away is forgotten.
        assert!(saved.unplace(OVERWORLD, [12, 64, 0]));
        assert!(!saved.unplace(OVERWORLD, [12, 64, 0]));
        // A world's file from before the placed bombs still reads.
        let mut old = serde_json::to_value(Saved::default()).unwrap();
        old.as_object_mut().unwrap().remove("placed");
        assert!(serde_json::from_value::<Saved>(old).unwrap().placed.is_empty());
    }

    #[test]
    fn a_remote_fires_its_linked_bomb_once() {
        let mut saved = Saved::default();
        let at = [5, 64, -3];
        assert!(saved.record(OVERWORLD, at, BombKind::Tactical));
        assert_eq!(saved.recorded(OVERWORLD, at), Some(BombKind::Tactical));
        // Its chunk needn't be loaded: the wave starts at the bomb, with the bomb's radius.
        assert_eq!(saved.fire_bomb(OVERWORLD, at), Ok(1));
        let news = saved.waves.news(OVERWORLD, saved.tick);
        assert_eq!(news.iter().map(|n| (n.id, n.center, n.radius)).collect::<Vec<_>>(), vec![(1, at, 50)]);
        // The record goes with the blast: a second press finds no bomb.
        assert_eq!(saved.recorded(OVERWORLD, at), None);
        assert_eq!(saved.fire_bomb(OVERWORLD, at), Err(Refused::Gone));
        assert_eq!(saved.next_id, 1, "no second blast");
        // The bomb still standing while its wave runs isn't linked again.
        assert!(!saved.record(OVERWORLD, at, BombKind::Tactical));
        assert_eq!(saved.fire_bomb(OVERWORLD, at), Err(Refused::Gone));
        // A bomb is its dimension's and its place's: the same place elsewhere is another.
        assert_eq!(saved.fire_bomb(NETHER, at), Err(Refused::Gone));
        assert!(saved.record(NETHER, at, BombKind::Atomic));
        assert_eq!(saved.fire_bomb(NETHER, at), Ok(2));
        // A link is a dimension and a place.
        let link = Link { dimension: NETHER.into(), pos: at };
        assert_eq!(Link::of(&link.dimension, link.pos), Some(link));
        assert_eq!(Link::of("", at), None);
    }

    #[test]
    fn a_broken_bomb_is_forgotten() {
        let mut saved = Saved::default();
        let (a, b) = ([0, 64, 0], [40, 70, 40]);
        assert!(saved.record(OVERWORLD, a, BombKind::Atomic));
        assert!(saved.record(OVERWORLD, b, BombKind::Tactical));
        // A bomb linked again keeps one record, of what it is now.
        assert!(saved.record(OVERWORLD, b, BombKind::Atomic));
        assert_eq!(saved.bombs.len(), 2);
        assert_eq!(saved.recorded(OVERWORLD, b), Some(BombKind::Atomic));
        // The bombs of a chunk, for a blast that went through it.
        assert_eq!(saved.bombs_in(OVERWORLD, chunk_of(40, 40)), vec![b]);
        assert_eq!(saved.bombs_in(NETHER, chunk_of(40, 40)), Vec::<[i32; 3]>::new());
        assert!(saved.forget(OVERWORLD, a));
        assert!(!saved.forget(OVERWORLD, a), "forgotten once");
        assert!(!saved.forget(NETHER, b), "only in its own dimension");
        assert_eq!(saved.fire_bomb(OVERWORLD, a), Err(Refused::Gone));
        assert!(saved.waves.finished(), "a broken bomb doesn't go off");
        assert_eq!(saved.fire_bomb(OVERWORLD, b), Ok(1));
    }

    #[test]
    fn maxpower_caps_blasts_and_bombs() {
        let mut saved = Saved { maxpower: 50, ..Saved::default() };
        // The command's blasts.
        assert_eq!(saved.fire(OVERWORLD, [0, 64, 0], 51), Err(Refused::Maxpower { radius: 51, maxpower: 50 }));
        assert_eq!(saved.fire(OVERWORLD, [0, 64, 0], 50), Ok(1));
        // A remote's bomb over the limit stays, to go off once the limit allows it.
        let (atomic, tactical) = ([100, 64, 0], [200, 64, 0]);
        saved.record(OVERWORLD, atomic, BombKind::Atomic);
        saved.record(OVERWORLD, tactical, BombKind::Tactical);
        assert_eq!(saved.fire_bomb(OVERWORLD, atomic), Err(Refused::Maxpower { radius: 250, maxpower: 50 }));
        assert_eq!(saved.recorded(OVERWORLD, atomic), Some(BombKind::Atomic));
        assert_eq!(saved.fire_bomb(OVERWORLD, tactical), Ok(2));
        saved.maxpower = 16;
        assert!(saved.allows(BombKind::Demolition.radius()));
        assert!(!saved.allows(BombKind::Tactical.radius()));
        assert_eq!(saved.fire_bomb(OVERWORLD, atomic), Err(Refused::Maxpower { radius: 250, maxpower: 16 }));
        saved.maxpower = 250;
        assert_eq!(saved.fire_bomb(OVERWORLD, atomic), Ok(3));
    }

    #[test]
    fn maxpower_takes_only_the_four_steps() {
        assert_eq!(MAXPOWER_STEPS, [16, 50, 250, 1024]);
        assert_eq!(*MAXPOWER_STEPS.last().unwrap(), MAX_RADIUS);
        assert_eq!(parse_maxpower(&[]), Some(None), "no number: tell the limit");
        for step in MAXPOWER_STEPS {
            assert_eq!(parse_maxpower(&[&step.to_string()]), Some(Some(step)));
        }
        for bad in ["0", "15", "17", "100", "1023", "1025", "-16", "big", "16.0"] {
            assert_eq!(parse_maxpower(&[bad]), None, "{bad}");
        }
        assert_eq!(parse_maxpower(&["16", "50"]), None);
        // Each bomb's radius is a step, so a limit lets whole kinds through.
        for k in BombKind::ALL {
            assert!(MAXPOWER_STEPS.contains(&k.radius()), "{k:?}");
        }
    }

    #[test]
    fn saved_bombs_round_trip() {
        let mut saved = Saved { maxpower: 250, ..Saved::default() };
        saved.record(OVERWORLD, [1, -60, 2], BombKind::Tactical);
        saved.record(NETHER, [-30_000, 120, 7], BombKind::Atomic);
        let back: Saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        assert_eq!(back.bombs, saved.bombs);
        assert_eq!(back.maxpower, 250);
        assert_eq!(back.recorded(NETHER, [-30_000, 120, 7]), Some(BombKind::Atomic));
        // A world saved before the remote: no bombs, nothing else lost.
        let mut json = serde_json::to_value(&saved).unwrap();
        json.as_object_mut().unwrap().remove("bombs");
        let old: Saved = serde_json::from_value(json).unwrap();
        assert!(old.bombs.is_empty());
        assert_eq!(old.maxpower, 250);
    }

    #[test]
    fn the_keypad_menu_has_the_bombs_slot_and_data() {
        let (namespace, name) = KEYPAD_MENU.split_once(':').unwrap();
        let menu: serde_json::Value = serde_json::from_str(&file(&format!("content/data/{namespace}/horizon/menu/{name}.json"))).unwrap();
        assert_eq!(menu["slots"], BOMB_SLOTS);
        assert_eq!(menu["data_slots"], PAD_DATA);
    }

    // ---- radiation (plan 5, Task 2) ----------------------------------------------

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn a_zone_fades_from_center_and_with_time() {
        let z = zone_of(&blast([0, 64, 0], 120, 1, World::OVERWORLD), 1000).unwrap();
        assert_eq!((z.dimension.as_str(), z.center, z.start, z.half_life), (OW, [0, 64, 0], 1000, 12_000));
        assert!(close(z.radius, 144.0) && close(z.peak, 10.0) && close(z.floor, 0.0), "{z:?}");
        let zones = [z];
        let at = |pos: [f64; 3], now: u64| background(&zones, OW, pos, now);
        let mid = [0.5, 64.5, 0.5];
        assert!(close(at(mid, 1000), 10.0));
        // Half way out a quarter is gone; up and down count as across.
        assert!(close(at([72.5, 64.5, 0.5], 1000), 7.5));
        assert!(close(at([0.5, 136.5, 0.5], 1000), 7.5));
        assert!(at([60.5, 64.5, 60.5], 1000) < at([30.5, 64.5, 30.5], 1000));
        assert_eq!(at([144.5, 64.5, 0.5], 1000), 0.0);
        assert_eq!(at([500.5, 64.5, 0.5], 1000), 0.0);
        assert_eq!(background(&zones, "minecraft:the_nether", mid, 1000), 0.0);
        // Halved every half-life, as strong as new before it starts.
        assert!(close(at(mid, 13_000), 5.0));
        assert!(close(at(mid, 25_000), 2.5));
        assert!(close(at(mid, 0), 10.0));
        // Two zones add up.
        let two = [zones[0].clone(), zone_of(&blast([100, 64, 0], 120, 2, World::OVERWORLD), 1000).unwrap()];
        assert!(close(background(&two, OW, [50.5, 64.5, 0.5], 1000), 2.0 * 10.0 * (1.0 - (50.0f64 / 144.0).powi(2))));
        // Old zones go.
        let mut saved = Saved { zones: zones.to_vec(), ..Saved::default() };
        assert!(!saved.prune_zones(1000 + 5 * 12_000));
        assert_eq!(saved.zones.len(), 1);
        assert!(saved.prune_zones(1000 + 10 * 12_000 + 1));
        assert!(saved.zones.is_empty());
    }

    #[test]
    fn small_blasts_leave_no_zone() {
        for r in [16, 39] {
            assert_eq!(zone_of(&blast([0, 64, 0], r, 1, World::OVERWORLD), 0), None, "{r}");
        }
        assert!(zone_of(&blast([0, 64, 0], ZONE_MIN_RADIUS, 1, World::OVERWORLD), 0).is_some());
        let mut saved = Saved { tick: 77, ..Saved::default() };
        saved.blast(OW, [0, 64, 0], BombKind::Demolition.radius()).unwrap();
        assert!(saved.zones.is_empty());
        // Every blast goes through `blast`: the command, the bombs, the remote, a chain.
        saved.fire(OW, [5, 70, 5], BombKind::Tactical.radius()).unwrap();
        assert_eq!(saved.zones.len(), 1);
        assert_eq!((saved.zones[0].center, saved.zones[0].start), ([5, 70, 5], 77));
        assert_eq!(saved.blast("mymod:moon", [0, 64, 0], 250), None);
        assert_eq!(saved.zones.len(), 1, "no zone where no blast can go");
    }

    #[test]
    fn a_big_zone_keeps_a_floor() {
        let small = zone_of(&blast([0, 64, 0], 149, 1, World::OVERWORLD), 0).unwrap();
        assert_eq!((small.half_life, small.floor), (12_000, 0.0));
        let z = zone_of(&blast([0, 64, 0], 250, 1, World::OVERWORLD), 0).unwrap();
        assert_eq!(z.half_life, 48_000);
        assert!(close(z.floor, 0.05) && close(z.radius, 300.0) && close(z.peak, 250.0 / 12.0));
        let mid = [0.5, 64.5, 0.5];
        assert!(close(z.at(mid, 48_000), z.peak / 2.0));
        assert!(close(z.at(mid, 100 * 48_000), z.peak * 0.05));
        assert!(z.at(mid, 100 * 48_000) >= 1.0, "an atomic bomb's center still hurts mobs");
        let mut saved = Saved { zones: vec![z, small], ..Saved::default() };
        assert!(saved.prune_zones(100 * 48_000));
        assert_eq!(saved.zones.len(), 1, "the big zone stays");
        assert_eq!(saved.zones[0].half_life, 48_000);
    }

    #[test]
    fn a_dose_builds_and_heals() {
        assert!(close(dose_step(0.0, 10.0, 0.0), 9.5));
        assert!(close(dose_step(5.0, 0.0, 0.0), 4.5));
        assert_eq!(dose_step(0.2, 0.0, 0.0), 0.0);
        assert_eq!(dose_step(0.0, 0.3, 0.0), 0.0, "below the healing, nothing builds");
        let me: u128 = 0x1234_5678_9abc_def0_1234_5678_9abc_def0;
        let key = dose_key(me);
        assert_eq!(key, "123456789abcdef0123456789abcdef0");
        let mut saved = Saved::default();
        saved.blast(OW, [0, 64, 0], 120).unwrap();
        let mid = [0.5, 64.5, 0.5];
        // Once a second, however often the player ticks.
        assert!(saved.expose(me, OW, mid, 0.0).is_some());
        assert!(saved.expose(me, OW, mid, 0.0).is_none());
        for _ in 0..59 {
            saved.tick += SECOND;
            let bg = saved.expose(me, OW, mid, 0.0).unwrap();
            assert!(bg > 9.0, "the zone fades a little in a minute: {bg}");
        }
        let rad = saved.doses[&key].rad;
        assert!(rad > 540.0 && rad < 560.0, "{rad}");
        // It is kept with the world.
        let back: Saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        assert_eq!(back.doses, saved.doses);
        assert_eq!(back.zones, saved.zones);
        // Away from the zone it heals.
        let away = [1000.5, 64.5, 0.5];
        saved.tick += SECOND;
        assert_eq!(saved.expose(me, OW, away, 0.0), Some(0.0));
        assert!(close(saved.doses[&key].rad, rad - 0.5));
        // The drug takes some off, never below 0, and the entry goes at 0.
        assert!(saved.take_dose(me, 300.0));
        assert!(close(saved.doses[&key].rad, rad - 300.5));
        assert!(saved.take_dose(me, 1000.0));
        assert!(!saved.doses.contains_key(&key));
        assert!(!saved.take_dose(me, 300.0));
        // Nothing builds where there is nothing, and no entry is made.
        saved.tick += SECOND;
        assert_eq!(saved.expose(me, OW, away, 0.0), Some(0.0));
        assert!(saved.doses.is_empty());
        // Death clears it.
        saved.tick += SECOND;
        saved.expose(me, OW, mid, 0.0).unwrap();
        assert!(saved.died(me));
        assert!(saved.doses.is_empty());
        assert!(!saved.died(me));
    }

    #[test]
    fn a_full_suit_keeps_most_out() {
        assert!(close(protection(true, true, true, true), 0.95));
        assert_eq!(protection(false, false, false, false), 0.0);
        assert!(close(protection(true, false, false, false), 0.25));
        assert!(close(protection(false, true, false, false), 0.30));
        assert!(close(protection(false, false, true, true), 0.40));
        assert!(close(dose_step(0.0, 20.0, 0.95), 0.5));
        assert!(close(dose_step(0.0, 20.0, 0.0), 19.5));
        // The pieces by their armour slots.
        assert_eq!(hazmat_in(103), Some("big_nukes:hazmat_helmet"));
        assert_eq!(hazmat_in(102), Some("big_nukes:hazmat_chestplate"));
        assert_eq!(hazmat_in(101), Some("big_nukes:hazmat_leggings"));
        assert_eq!(hazmat_in(100), Some("big_nukes:hazmat_boots"));
        assert_eq!(hazmat_in(0), None);
        assert_eq!(hazmat_in(-106), None);
        // A piece wears a point at a time and breaks on its last.
        assert_eq!(wear(0, HAZMAT_DURABILITY), Some(1));
        assert_eq!(wear(598, HAZMAT_DURABILITY), Some(599));
        assert_eq!(wear(599, HAZMAT_DURABILITY), None);
    }

    #[test]
    fn stages_by_dose() {
        let stages = [0.0, 99.9, 100.0, 299.9, 300.0, 599.9, 600.0, 999.9, 1000.0, 5000.0].map(stage);
        assert_eq!(stages, [0, 0, 1, 1, 2, 2, 3, 3, 4, 4]);
        assert_eq!(harm(0), None);
        assert_eq!(harm(1), None);
        assert_eq!(harm(2), Some((1, 100)));
        assert_eq!(harm(3), Some((2, 40)));
        assert_eq!(harm(4), Some((1000, 20)));
        assert_eq!([0.0, 0.99, 1.0, 4.99, 5.0, 30.0].map(mob_harm), [None, None, Some(1), Some(1), Some(2), Some(2)]);
        // The damage comes at once, then on its clock.
        let me = 7u128;
        let mut saved = Saved::default();
        let set = |saved: &mut Saved, rad: f64| saved.doses.entry(dose_key(me)).or_default().rad = rad;
        assert_eq!(saved.sicken(me), None);
        set(&mut saved, 50.0);
        assert_eq!(saved.sicken(me), None);
        set(&mut saved, 150.0);
        assert_eq!(saved.sicken(me), Some((1, None)));
        set(&mut saved, 350.0);
        assert_eq!(saved.sicken(me), Some((2, Some(1))));
        saved.tick = 20;
        assert_eq!(saved.sicken(me), Some((2, None)));
        saved.tick = 100;
        assert_eq!(saved.sicken(me), Some((2, Some(1))));
        set(&mut saved, 700.0);
        saved.tick = 120;
        assert_eq!(saved.sicken(me), Some((3, None)));
        saved.tick = 140;
        assert_eq!(saved.sicken(me), Some((3, Some(2))));
        set(&mut saved, 1000.0);
        saved.tick = 160;
        assert_eq!(saved.sicken(me), Some((4, Some(1000))));
        saved.tick = 180;
        assert_eq!(saved.sicken(me), Some((4, Some(1000))), "until they die");
    }

    #[test]
    fn a_mob_is_named_by_its_uuid_as_commands_read_it() {
        assert_eq!(uuid_text(0x0123_4567_89ab_cdef_0011_2233_4455_6677), "01234567-89ab-cdef-0011-223344556677");
        assert_eq!(uuid_text(1), "00000000-0000-0000-0000-000000000001");
    }

    #[test]
    fn old_files_read() {
        let mut saved = Saved::default();
        saved.blast(OW, [0, 64, 0], 50).unwrap();
        let mut json = serde_json::to_value(&saved).unwrap();
        let o = json.as_object_mut().unwrap();
        assert!(o.remove("zones").is_some() && o.remove("doses").is_some());
        let old: Saved = serde_json::from_value(json).unwrap();
        assert!(old.zones.is_empty() && old.doses.is_empty());
        assert_eq!(old.next_id, 1);
        let dose: Dose = serde_json::from_str(r#"{"rad": 5.0}"#).unwrap();
        assert_eq!(dose, Dose { rad: 5.0, stepped: 0, hurt: None });
    }

    // ---- radiation in the window (plan 5, Task 4) ----------------------------------

    #[test]
    fn the_client_reads_the_same_background() {
        let me = 9u128;
        let mut saved = Saved { tick: 5_000, ..Saved::default() };
        saved.blast(OW, [0, 64, 0], 120).unwrap();
        saved.blast(OW, [100, 64, 0], 250).unwrap();
        saved.tick = 9_000;
        let rad = saved.rad_for(me, OW, [10.0, 64.0, 0.0]);
        // The window's clock is its own: it only counts on from the message.
        let told = Told { rad, at: 123_456 };
        for (pos, later) in [([10.5, 65.6, 0.5], 0), ([40.5, 70.0, -20.5], 300), ([150.5, 64.0, 0.5], 12_000)] {
            let server = background(&saved.zones, OW, pos, saved.tick + later);
            let client = told.background(OW, pos, 123_456 + later);
            assert!(close(server, client), "{pos:?} {later}: {server} {client}");
            assert!(server > 0.0);
        }
        assert_eq!(told.background("minecraft:the_nether", [0.5, 64.0, 0.5], 123_456), 0.0);
        // A clock behind the message's (another world) counts as no time passed.
        assert!(close(told.background(OW, [0.5, 64.0, 0.5], 0), background(&saved.zones, OW, [0.5, 64.0, 0.5], 9_000)));
    }

    #[test]
    fn rad_message_round_trips() {
        let me = 9u128;
        let mut saved = Saved::default();
        for k in 0..40 {
            saved.blast(OW, [k * 1000, 64, 0], 120).unwrap();
        }
        saved.blast("minecraft:the_nether", [0, 64, 0], 120).unwrap();
        saved.doses.entry(dose_key(me)).or_default().rad = 42.4;
        let rad = saved.rad_for(me, OW, [20_000.0, 64.0, 0.0]);
        // Only the dimension's zones, the nearest first, and no more than a message holds.
        assert_eq!(rad.zones.len(), RAD_ZONES);
        assert!(rad.zones.iter().all(|z| z.dimension == OW));
        assert_eq!(rad.zones[0].center, [20_000, 64, 0]);
        assert!(rad.zones.iter().any(|z| z.center == [19_000, 64, 0]) && rad.zones.iter().any(|z| z.center == [21_000, 64, 0]));
        assert!(!rad.zones.iter().any(|z| z.center == [39_000, 64, 0] || z.center == [0, 64, 0]));
        assert_eq!((rad.now, rad.dose), (saved.tick, 42.4));
        assert!(!rad.is_empty());
        let bytes = serde_json::to_vec(&rad).unwrap();
        assert!(bytes.len() < 30_000 / 4, "{} bytes", bytes.len());
        let back: Rad = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, rad);
        // Nothing to tell: no zone in the dimension and no dose.
        let other = saved.rad_for(7, "minecraft:the_end", [0.0, 64.0, 0.0]);
        assert!(other.is_empty() && other.zones.is_empty() && other.dose == 0.0);
        assert!(!saved.rad_for(me, "minecraft:the_end", [0.0, 64.0, 0.0]).is_empty(), "a dose is still news");
    }

    #[test]
    fn geiger_clicks_scale_with_background() {
        assert_eq!(click_chance(0.0), 0.0);
        assert!(close(click_chance(1.0), 0.25));
        assert!(close(click_chance(2.0), 0.5));
        assert!(close(click_chance(3.6), 0.9));
        assert!(close(click_chance(100.0), 0.9));
        assert_eq!(click_chance(-1.0), 0.0);
        assert_eq!(click_chance(f64::NAN), 0.0);
        let count = |bg: f64| (0..20_000u64).filter(|&t| clicks(bg, t)).count();
        assert_eq!(count(0.0), 0);
        let (low, mid, high) = (count(0.4), count(2.0), count(50.0));
        assert!((1_700..2_300).contains(&low), "{low}");
        assert!((9_500..10_500).contains(&mid), "{mid}");
        assert!((17_600..18_400).contains(&high), "{high}");
        // The same tick clicks the same way: no clock of its own.
        assert_eq!(clicks(2.0, 77), clicks(2.0, 77));
        // The pitch wanders a little around 1.
        for t in 0..100 {
            assert!((0.9..=1.1).contains(&click_pitch(t)));
        }
        // The reading's colour by danger, and its numbers.
        assert_eq!([0.0, 0.99, 1.0, 4.99, 5.0, 40.0].map(reading_colour), [GREEN, GREEN, YELLOW, YELLOW, RED, RED]);
        assert_eq!(reading_numbers(2.345, 151.6), ("2.3".to_owned(), "152".to_owned()));
        assert_eq!(reading_numbers(0.0, 0.0), ("0.0".to_owned(), "0".to_owned()));
    }

    fn zone_fog(bg: f64, color: [f32; 4], start: f32, end: f32) -> Option<([f32; 4], f32, f32)> {
        tint_fog(fog_strength(bg), color, start, end)
    }

    #[test]
    fn fog_by_background() {
        let sky = [0.6, 0.7, 0.9, 1.0];
        assert_eq!(zone_fog(0.0, sky, 0.0, 1024.0), None);
        assert_eq!(zone_fog(0.19, sky, 0.0, 1024.0), None);
        // Full from 5 rad/s: the zone's colour, the fog's end at 64 blocks.
        let (c, start, end) = zone_fog(5.0, sky, 0.0, 1024.0).unwrap();
        assert!(c.iter().zip([0.55, 0.62, 0.45, 1.0]).all(|(a, b)| (a - b).abs() < 1e-6), "{c:?}");
        assert_eq!((start, end), (0.0, 64.0));
        assert_eq!(zone_fog(50.0, sky, 0.0, 1024.0), zone_fog(5.0, sky, 0.0, 1024.0));
        // Half way at 2.5.
        let (c, _, end) = zone_fog(2.5, sky, 0.0, 1024.0).unwrap();
        assert!((c[2] - 0.675).abs() < 1e-6 && (end - 544.0).abs() < 1e-3, "{c:?} {end}");
        // A fog already nearer keeps its end; the start comes in to the eyes, so the end stays past it.
        let (_, start, end) = zone_fog(5.0, sky, 10.0, 48.0).unwrap();
        assert_eq!((start, end), (0.0, 48.0));
        let (_, start, end) = zone_fog(2.5, sky, 10.0, 48.0).unwrap();
        assert_eq!((start, end), (5.0, 48.0));
        let (_, start, end) = zone_fog(5.0, sky, 200.0, 1024.0).unwrap();
        assert!(end > start && end == 64.0, "{start} {end}");
        // Horizon's Fog option off: the distances are the player's, only the colour turns.
        let (_, start, end) = zone_fog(5.0, sky, 1e9, 2e9).unwrap();
        assert_eq!((start, end), (1e9, 2e9));
        // The result is always one the game takes.
        for bg in [0.2, 0.7, 3.0, 9.0, f64::INFINITY] {
            for (start, end) in [(0.0, 1024.0), (-8.0, 96.0), (60.0, 64.0), (300.0, 301.0)] {
                let (c, s, e) = zone_fog(bg, [1.0, 0.0, 1.0, 1.0], start, end).unwrap();
                assert!(c.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) && s.is_finite() && e > s, "{bg} {start} {end}");
            }
        }
        assert_eq!(zone_fog(f64::NAN, sky, 0.0, 1024.0), None);
        // The shown strength eases toward the target over time, not frames.
        assert_eq!(fog_strength(0.1), 0.0);
        assert!(close(fog_strength(1.0), 0.2));
        let one = ease(0.0, 1.0, 0.1);
        let two = ease(ease(0.0, 1.0, 0.05), 1.0, 0.05);
        assert!(close(one, two) && one > 0.0 && one < 0.5, "{one} {two}");
        assert!(ease(0.0, 1.0, 5.0) > 0.99);
        assert_eq!(ease(0.3, 0.3, 1.0), 0.3);
    }
}
