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
    /// The superbomb built in the world round its core (see "the sunbreaker" below).
    Sunbreaker,
}

impl BombKind {
    pub const ALL: [BombKind; 4] = [BombKind::Demolition, BombKind::Tactical, BombKind::Atomic, BombKind::Sunbreaker];

    pub fn id(self) -> &'static str {
        match self {
            BombKind::Demolition => "big_nukes:demolition_charge",
            BombKind::Tactical => "big_nukes:tactical_warhead",
            BombKind::Atomic => "big_nukes:atomic_bomb",
            BombKind::Sunbreaker => SUNBREAKER_CORE,
        }
    }

    /// How far the blast reaches, before the operator's limit: the Sunbreaker's with its whole
    /// structure, which `power` reads.
    pub fn radius(self) -> u32 {
        match self {
            BombKind::Demolition => 16,
            BombKind::Tactical => 50,
            BombKind::Atomic => 250,
            BombKind::Sunbreaker => SUNBREAKER_BASE + SUNBREAKER_REFLECTOR,
        }
    }

    /// How far the blast of the bomb at `core` reaches now: the Sunbreaker's by the
    /// structure round it as `block` reads it, any other's its own radius.
    pub fn power(self, core: [i32; 3], block: &dyn Fn([i32; 3]) -> String) -> Result<u32, StructureProblem> {
        match self {
            BombKind::Sunbreaker => read_structure(core, block).radius(),
            _ => Ok(self.radius()),
        }
    }

    /// How long its lit fuse burns: the Sunbreaker's long enough to start running.
    pub fn fuse(self) -> u64 {
        if self == BombKind::Sunbreaker { SUNBREAKER_FUSE } else { FUSE_TICKS }
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
pub const ITEMS: [(&str, u8, bool); 18] = [
    ("big_nukes:raw_uranium", 64, false),
    ("big_nukes:enriched_uranium", 64, true),
    (WEAPONS_GRADE_URANIUM, 64, true),
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
    (IODINE, 16, false),
    (GEIGER_COUNTER, 1, false),
    (MISSILE, 1, true),
    (GUIDE, 1, false),
];

/// The guide: a book of the mod's own, whose screen is drawn by the mod (see "the guide" below).
pub const GUIDE: &str = "big_nukes:guide";
/// A player who was given the guide when they first joined.
pub const GUIDE_TAG: &str = "big_nukes_guide_book";

/// The command that gives `name` the guide.
pub fn guide_give(name: &str) -> String {
    format!("give {name} {GUIDE}")
}

/// What each block and item says under its name about how to use it: lang keys, one line each.
const TIPS: [(&str, &[&str]); 33] = [
    ("big_nukes:demolition_charge", &["big_nukes.tip.demolition_charge", "big_nukes.tip.demolition_charge.2"]),
    ("big_nukes:tactical_warhead", &["big_nukes.tip.tactical_warhead", "big_nukes.tip.tactical_warhead.2"]),
    ("big_nukes:atomic_bomb", &["big_nukes.tip.atomic_bomb", "big_nukes.tip.atomic_bomb.2"]),
    ("big_nukes:fuse_detonator", &["big_nukes.tip.fuse_detonator"]),
    ("big_nukes:keypad_detonator", &["big_nukes.tip.keypad_detonator", "big_nukes.tip.keypad_detonator.2"]),
    ("big_nukes:receiver_detonator", &["big_nukes.tip.receiver_detonator"]),
    ("big_nukes:remote", &["big_nukes.tip.remote"]),
    ("big_nukes:centrifuge", &["big_nukes.tip.centrifuge"]),
    ("big_nukes:assembly_table", &["big_nukes.tip.assembly_table"]),
    ("big_nukes:uranium_ore", &["big_nukes.tip.uranium_ore"]),
    ("big_nukes:deepslate_uranium_ore", &["big_nukes.tip.uranium_ore"]),
    ("big_nukes:raw_uranium", &["big_nukes.tip.raw_uranium"]),
    ("big_nukes:enriched_uranium", &["big_nukes.tip.enriched_uranium"]),
    ("big_nukes:nuclear_charge", &["big_nukes.tip.nuclear_charge"]),
    ("big_nukes:bomb_casing", &["big_nukes.tip.bomb_casing"]),
    ("big_nukes:geiger_counter", &["big_nukes.tip.geiger_counter"]),
    ("big_nukes:hazmat_helmet", &["big_nukes.tip.hazmat"]),
    ("big_nukes:hazmat_chestplate", &["big_nukes.tip.hazmat"]),
    ("big_nukes:hazmat_leggings", &["big_nukes.tip.hazmat"]),
    ("big_nukes:hazmat_boots", &["big_nukes.tip.hazmat"]),
    ("big_nukes:anti_rad", &["big_nukes.tip.anti_rad"]),
    (MISSILE, &["big_nukes.tip.missile"]),
    (LAUNCH_PAD, &["big_nukes.tip.launch_pad", "big_nukes.tip.launch_pad.2"]),
    (SILO_HATCH, &["big_nukes.tip.silo_hatch"]),
    (LAUNCH_CONSOLE, &["big_nukes.tip.launch_console", "big_nukes.tip.launch_console.2"]),
    (STRIKE_BEACON, &["big_nukes.tip.strike_beacon", "big_nukes.tip.strike_beacon.2"]),
    ("big_nukes:guide", &["big_nukes.tip.guide"]),
    (SUNBREAKER_CORE, &["big_nukes.tip.sunbreaker_core", "big_nukes.tip.sunbreaker_core.2"]),
    (IMPLOSION_SHELL, &["big_nukes.tip.implosion_shell", "big_nukes.tip.sunbreaker_part"]),
    (NEUTRON_REFLECTOR, &["big_nukes.tip.neutron_reflector", "big_nukes.tip.sunbreaker_part"]),
    (WEAPONS_GRADE_URANIUM, &["big_nukes.tip.weapons_grade_uranium"]),
    (IODINE, &["big_nukes.tip.iodine_tablets"]),
    (DECONTAMINATOR, &["big_nukes.tip.decontaminator", "big_nukes.tip.decontaminator.2"]),
];

/// The tip lines of `id`, empty for anything else.
pub fn tips(id: &str) -> &'static [&'static str] {
    TIPS.iter().find(|t| t.0 == id).map_or(&[], |t| t.1)
}

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

/// The vanilla item an item copies: a suit piece its iron armour, the guide a book, the rest paper.
pub fn template(id: &str) -> &'static str {
    if id == GUIDE {
        return "minecraft:book";
    }
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
    } else if id == IODINE {
        // Eaten as fast as dried kelp, hungry or not.
        c.insert(
            "minecraft:consumable".into(),
            json!({"consume_seconds": 0.8, "animation": "eat", "sound": "minecraft:entity.generic.eat",
                "on_consume_effects": [{"type": IODINE_EFFECT}]}),
        );
    } else if id == GUIDE {
        // A book goes on the enchanting table; the guide stays the guide.
        c.insert("!minecraft:enchantable".into(), json!({}));
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
/// Its slots: uranium in, fuel, the richer uranium out. Its menu type is
/// content/data/big_nukes/horizon/menu/centrifuge.json, named as the block.
pub const INPUT_SLOT: usize = 0;
pub const FUEL_SLOT: usize = 1;
pub const OUTPUT_SLOT: usize = 2;
pub const CENTRIFUGE_SLOTS: usize = 3;
/// Ticks of work that turn one raw uranium into one enriched.
pub const SPIN_TICKS: u32 = 400;

/// A step of the centrifuge: `takes` of `input` make one `output` in `ticks` of work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enrichment {
    pub input: &'static str,
    pub takes: u32,
    pub output: &'static str,
    pub ticks: u32,
}

/// What the centrifuge makes, by what lies in its input: raw uranium is enriched, and four
/// enriched make one weapons-grade, three times slower.
pub const ENRICHMENTS: [Enrichment; 2] = [
    Enrichment { input: RAW_URANIUM, takes: 1, output: ENRICHED_URANIUM, ticks: SPIN_TICKS },
    Enrichment { input: ENRICHED_URANIUM, takes: 4, output: WEAPONS_GRADE_URANIUM, ticks: SPIN_TICKS * 3 },
];

/// The step an item in the input starts: its index in `ENRICHMENTS`.
pub fn enrichment(item: &str) -> Option<usize> {
    ENRICHMENTS.iter().position(|e| e.input == item)
}
/// The screen's flame is this tall and its arrow this wide, in GUI pixels.
pub const FLAME_HEIGHT: i32 = 14;
pub const ARROW_WIDTH: i32 = 24;

/// A centrifuge's state, kept in its block entity: the ticks left of the fuel
/// it burns and that fuel's whole burn, the ticks of work on the current input,
/// and the step (`ENRICHMENTS`) that work is on: one saved without it was enriching.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Centrifuge {
    pub burn: u32,
    pub burn_max: u32,
    pub spin: u32,
    pub stage: u8,
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
    /// One tick. `input` and `output` are what their slots hold, `fuel` the burn
    /// ticks of the fuel in its slot (none, or 0, when nothing there burns), and
    /// `max_stack` how many of an item stack. `Step::made` takes the step's
    /// (`ENRICHMENTS[stage]`) input and gives its output.
    ///
    /// The fire burns only while it spins: an input it can't work (too little of
    /// it, or nothing it takes) or a full output keeps the rest of the burning
    /// fuel, and takes no new fuel. Taking the input out or changing it for
    /// another loses the progress, as a furnace does.
    pub fn step(
        &mut self,
        input: Option<(&str, u32)>,
        fuel: Option<u32>,
        output: Option<(&str, u32)>,
        max_stack: &dyn Fn(&str) -> u32,
    ) -> Step {
        let stage = input.and_then(|(item, count)| enrichment(item).filter(|&i| count >= ENRICHMENTS[i].takes));
        let Some(stage) = stage else {
            self.spin = 0;
            return Step::IDLE;
        };
        if usize::from(self.stage) != stage {
            self.stage = stage as u8;
            self.spin = 0;
        }
        let e = ENRICHMENTS[stage];
        let room = match output {
            None => true,
            Some((item, count)) => item == e.output && count < max_stack(e.output),
        };
        if !room {
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
        if self.spin >= e.ticks {
            self.spin = 0;
            step.made = true;
        }
        step
    }

    /// The step its work is on.
    pub fn enrichment(&self) -> Enrichment {
        ENRICHMENTS.get(usize::from(self.stage)).copied().unwrap_or(ENRICHMENTS[0])
    }

    /// The menu's data slots: `[burn, burn_max, spin, the step's ticks]`, kept
    /// within the 16 bits a data slot has on the wire.
    pub fn data(&self) -> [i32; 4] {
        let short = |v: u32| v.min(i16::MAX as u32) as i32;
        [short(self.burn), short(self.burn_max), short(self.spin), short(self.enrichment().ticks)]
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

/// A recipe of the assembly table: its result and how many it makes, and how many of
/// each item it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assembly {
    pub result: &'static str,
    pub count: u32,
    pub parts: &'static [(&'static str, u32)],
}

pub const ASSEMBLIES: [Assembly; 11] = [
    Assembly { result: "big_nukes:nuclear_charge", count: 1, parts: &[(ENRICHED_URANIUM, 1), ("minecraft:tnt", 4)] },
    Assembly { result: "big_nukes:bomb_casing", count: 1, parts: &[("minecraft:iron_block", 4), ("minecraft:redstone", 4)] },
    Assembly {
        result: "big_nukes:demolition_charge",
        count: 1,
        parts: &[("minecraft:tnt", 4), ("minecraft:iron_ingot", 4), ("minecraft:redstone", 1)],
    },
    Assembly {
        result: "big_nukes:tactical_warhead",
        count: 1,
        parts: &[
            ("big_nukes:nuclear_charge", 1),
            ("big_nukes:bomb_casing", 1),
            ("minecraft:iron_ingot", 6),
            ("minecraft:redstone", 1),
        ],
    },
    Assembly {
        result: "big_nukes:atomic_bomb",
        count: 1,
        parts: &[("big_nukes:nuclear_charge", 4), ("big_nukes:bomb_casing", 4), ("minecraft:redstone_block", 1)],
    },
    Assembly {
        result: MISSILE,
        count: 1,
        parts: &[("big_nukes:bomb_casing", 2), ("minecraft:iron_block", 3), ("minecraft:blaze_powder", 4)],
    },
    Assembly {
        result: STRIKE_BEACON,
        count: 1,
        parts: &[("minecraft:iron_ingot", 4), ("minecraft:redstone", 4), ("minecraft:compass", 1)],
    },
    Assembly {
        result: SUNBREAKER_CORE,
        count: 1,
        parts: &[(WEAPONS_GRADE_URANIUM, 4), ("big_nukes:nuclear_charge", 4), ("minecraft:diamond_block", 1)],
    },
    Assembly {
        result: IMPLOSION_SHELL,
        count: 1,
        parts: &[("minecraft:tnt", 4), ("minecraft:iron_block", 2), ("minecraft:redstone", 2)],
    },
    Assembly { result: NEUTRON_REFLECTOR, count: 4, parts: &[("minecraft:iron_block", 4), (ENRICHED_URANIUM, 1)] },
    Assembly {
        result: DECONTAMINATOR,
        count: 1,
        parts: &[("minecraft:iron_block", 4), ("minecraft:water_bucket", 2), ("minecraft:sponge", 1), ("minecraft:redstone", 2)],
    },
];

/// What a part leaves in its grid slot when Assemble takes it: a bucket's bucket.
pub fn remainder(item: &str) -> Option<&'static str> {
    matches!(item, "minecraft:water_bucket" | "minecraft:lava_bucket").then_some("minecraft:bucket")
}

/// What Assemble does: the result and how many of it to add to the output, and how
/// many to take from which grid slots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assembled {
    pub result: &'static str,
    pub count: u32,
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
        Some((item, count)) => item == recipe.result && count + recipe.count <= max_stack(recipe.result),
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
    Some(Assembled { result: recipe.result, count: recipe.count, take })
}

/// The screen's data slot: 0 for no recipe, else the recipe's index plus one.
pub fn assembly_data(grid: &[Option<(&str, u32)>]) -> i32 {
    assembly_in(grid).map_or(0, |i| i as i32 + 1)
}

/// The recipe a data slot names, as `assembly_data` wrote it.
pub fn assembly_of_data(data: i32) -> Option<&'static Assembly> {
    usize::try_from(data).ok()?.checked_sub(1).and_then(|i| ASSEMBLIES.get(i))
}

/// The language key of a block's or an item's name: the bombs, the Sunbreaker's parts,
/// the silo's blocks and the decontaminator are blocks.
pub fn name_key(id: &str) -> String {
    let (namespace, name) = id.split_once(':').unwrap_or(("minecraft", id));
    let block = BombKind::ALL.iter().any(|k| k.id() == id)
        || SUNBREAKER_PARTS.contains(&id)
        || SILO_BLOCKS.contains(&id)
        || id == DECONTAMINATOR;
    let kind = if block { "block" } else { "item" };
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

/// The game repeats a right click held down every 4 ticks: a click on the same thing this many
/// ticks or fewer after the last one heard is the same click, held.
pub const HELD_CLICK_TICKS: u64 = 8;

/// The clicks heard lately, by what each was (the player, the block, the item in hand) and its
/// last tick: a click held down is answered once, not at each of the game's repeats.
#[derive(Debug, Default)]
pub struct Clicks<K> {
    last: Vec<(K, u64)>,
}

impl<K: PartialEq> Clicks<K> {
    pub const fn new() -> Self {
        Clicks { last: Vec::new() }
    }

    /// A click `key` at `now`: None for a new click, else the ticks since the one it goes on
    /// from (0 for the other hand's turn of the same click). Each repeat keeps the click held;
    /// one let go longer than `HELD_CLICK_TICKS` is forgotten.
    pub fn heard(&mut self, key: K, now: u64) -> Option<u64> {
        self.last.retain(|(_, t)| *t <= now && now - *t <= HELD_CLICK_TICKS);
        match self.last.iter_mut().find(|(k, _)| *k == key) {
            Some((_, t)) => {
                let since = now - *t;
                *t = now;
                Some(since)
            }
            None => {
                self.last.push((key, now));
                None
            }
        }
    }
}

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
        self.detonator == Some(Detonator::Fuse) && self.arm(now, self.kind.fuse())
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
        self.fires_at = self.fires_at.map(|at| at.min(now + self.kind.fuse().max(COUNTDOWN_TICKS)));
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

// ---- the sunbreaker ---------------------------------------------------------------
//
// The largest bomb is built in the world: its core, a shell round it and a
// reflector round that, each a cube's surface. Its radius is read from the
// blocks when it goes off, so a broken part weakens it, and without a whole
// shell it doesn't go off. Hidden in its shell, the core is reached through any
// block of its structure (`core_near`).

pub const SUNBREAKER_CORE: &str = "big_nukes:sunbreaker_core";
pub const IMPLOSION_SHELL: &str = "big_nukes:implosion_shell";
pub const NEUTRON_REFLECTOR: &str = "big_nukes:neutron_reflector";
/// The structure's blocks, declared in code beside the core.
pub const SUNBREAKER_PARTS: [&str; 2] = [IMPLOSION_SHELL, NEUTRON_REFLECTOR];
/// The core's uranium, from the centrifuge's second step.
pub const WEAPONS_GRADE_URANIUM: &str = "big_nukes:weapons_grade_uranium";
/// The shell: the 26 neighbours of the core, the 3×3×3 cube round it.
pub const SHELL_BLOCKS: u32 = 26;
/// The reflector: the surface of the 5×5×5 cube round the core.
pub const REFLECTOR_BLOCKS: u32 = 98;
/// The radius of a whole shell, and what a whole reflector adds, a block's share each.
pub const SUNBREAKER_BASE: u32 = 600;
pub const SUNBREAKER_REFLECTOR: u32 = 400;
/// A blast this large takes the bedrock at its center, in a circle this wide, down to the
/// world's floor: under it is the void.
pub const BEDROCK_RADIUS: u32 = 1000;
pub const BEDROCK_HOLE: u32 = 24;
/// The Sunbreaker's fuse, and its last ticks, when an alarm sounds over the hiss.
pub const SUNBREAKER_FUSE: u64 = 300;
pub const ALARM_TICKS: u64 = 100;

/// Why a Sunbreaker doesn't go off: its shell has only so many of its blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructureProblem {
    Shell(u32),
}

/// A Sunbreaker's structure as it stands: the right blocks of each layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Structure {
    pub shell: u32,
    pub reflector: u32,
}

impl Structure {
    /// Its blast's radius: `SUNBREAKER_BASE` for a whole shell and the reflector's share of
    /// `SUNBREAKER_REFLECTOR`, rounded down; none without a whole shell.
    pub fn radius(&self) -> Result<u32, StructureProblem> {
        if self.shell < SHELL_BLOCKS {
            return Err(StructureProblem::Shell(self.shell));
        }
        Ok(SUNBREAKER_BASE + SUNBREAKER_REFLECTOR * self.reflector.min(REFLECTOR_BLOCKS) / REFLECTOR_BLOCKS)
    }
}

/// The places of a structure round `core` and their layer: 1 the shell, 2 the reflector.
pub fn structure_cells(core: [i32; 3]) -> impl Iterator<Item = ([i32; 3], u8)> {
    (-2..=2i32).flat_map(move |dx| {
        (-2..=2i32).flat_map(move |dy| {
            (-2..=2i32).filter_map(move |dz| {
                let layer = dx.abs().max(dy.abs()).max(dz.abs()) as u8;
                (layer > 0).then_some(([core[0] + dx, core[1] + dy, core[2] + dz], layer))
            })
        })
    })
}

/// The structure round `core`, its blocks read with `block`: a wrong block, or one
/// that can't be read (air, where nothing is loaded), counts as missing.
pub fn read_structure(core: [i32; 3], block: &dyn Fn([i32; 3]) -> String) -> Structure {
    let mut s = Structure::default();
    for (pos, layer) in structure_cells(core) {
        match (layer, block(pos).as_str()) {
            (1, IMPLOSION_SHELL) => s.shell += 1,
            (2, NEUTRON_REFLECTOR) => s.reflector += 1,
            _ => {}
        }
    }
    s
}

/// The core whose structure has a place at `pos`, or the core at `pos` itself: the
/// nearest within 2 blocks on each axis.
pub fn core_near(pos: [i32; 3], block: &dyn Fn([i32; 3]) -> String) -> Option<[i32; 3]> {
    std::iter::once(pos)
        .chain(structure_cells(pos).filter(|c| c.1 == 1).map(|c| c.0))
        .chain(structure_cells(pos).filter(|c| c.1 == 2).map(|c| c.0))
        .find(|&p| block(p) == SUNBREAKER_CORE)
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

/// The crater's floor under column (`x`, `z`), at distance `d` (`edge`): a bowl,
/// deepest at the center, never into bedrock, but for the hole a blast of
/// `BEDROCK_RADIUS` or more makes at its center, down to the world's floor. The
/// hole is a circle of `BEDROCK_HOLE` blocks round the center's column, whatever the
/// border's swing.
pub fn crater_floor(blast: &Blast, x: i32, z: i32, d: f64) -> i32 {
    let (dx, dz) = (i64::from(x - blast.center[0]), i64::from(z - blast.center[2]));
    let hole = i64::from(BEDROCK_HOLE);
    if blast.radius >= BEDROCK_RADIUS && dx * dx + dz * dz <= hole * hole {
        return blast.world.min_y;
    }
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
                Ring::Core => plants.runs.extend(absolute(crater_floor(blast, x, z, d), AIR, None)),
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
    /// The missiles launched and not yet gone off.
    #[serde(default)]
    pub flights: Vec<Flight>,
    /// The launch consoles' codes, by `console_key`.
    #[serde(default)]
    pub consoles: BTreeMap<String, Console>,
    /// The charged strike beacons, their bombers not yet come and gone.
    #[serde(default)]
    pub beacons: Vec<Flight>,
    /// The chunks the flights keep loaded with `/forceload`.
    #[serde(default)]
    pub forced: Vec<Forced>,
    /// The circles the decontaminators cleaned, theirs still where they were broken.
    #[serde(default)]
    pub cleans: Vec<Clean>,
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
            flights: Vec::new(),
            consoles: BTreeMap::new(),
            beacons: Vec::new(),
            forced: Vec::new(),
            cleans: Vec::new(),
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
    /// A Sunbreaker far off whose core hasn't read its structure yet.
    Unread,
    /// A Sunbreaker whose structure isn't whole enough.
    Structure(StructureProblem),
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

/// The limits `/bignukes maxpower` takes: each bomb's radius (the Sunbreaker's whole), then the largest blast.
pub const MAXPOWER_STEPS: [u32; 5] = [16, 50, 250, 1000, 1024];

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
    /// A Sunbreaker's structure as its core last read it, for a remote that fires it from afar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<Result<u32, StructureProblem>>,
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
            None => self.bombs.push(Receiver { dimension: dimension.to_owned(), pos, kind, power: None }),
        }
        true
    }

    /// The bomb recorded at `pos`, if there is one.
    pub fn recorded(&self, dimension: &str, pos: [i32; 3]) -> Option<BombKind> {
        self.bombs.iter().find(|b| b.dimension == dimension && b.pos == pos).map(|b| b.kind)
    }

    /// What a recorded Sunbreaker's core read of its structure: kept for a remote that fires it from afar.
    /// Whether it changed.
    pub fn read_power(&mut self, dimension: &str, pos: [i32; 3], power: Result<u32, StructureProblem>) -> bool {
        match self.bombs.iter_mut().find(|b| b.dimension == dimension && b.pos == pos) {
            Some(b) if b.power != Some(power) => {
                b.power = Some(power);
                true
            }
            _ => false,
        }
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
        self.placed.push(Receiver { dimension: dimension.to_owned(), pos, kind, power: None });
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
    /// A Sunbreaker's radius is its structure's as `block` reads it; without a whole
    /// shell it fizzles: it is forgotten, and the glue takes it away.
    pub fn set_off(&mut self, dimension: &str, pos: [i32; 3], block: &dyn Fn([i32; 3]) -> String) -> Result<u64, Refused> {
        let kind = self.placed.iter().find(|b| b.dimension == dimension && b.pos == pos).map(|b| b.kind).ok_or(Refused::Gone)?;
        // One without a whole shell doesn't go off and stays: a crater that reaches its core takes it.
        let radius = kind.power(pos, block).map_err(Refused::Structure)?;
        let id = self.fire(dimension, pos, radius)?;
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
    /// operator's limit, and is forgotten. One over the limit stays recorded, as
    /// does a Sunbreaker whose structure, read with `block`, has no whole shell.
    /// `block` reads the bomb's place, None while its chunk isn't loaded: a Sunbreaker then goes off with the
    /// radius its core last read (`read_power`), which can't have changed since, its blocks unloaded too.
    pub fn fire_bomb(&mut self, dimension: &str, pos: [i32; 3], block: Option<&dyn Fn([i32; 3]) -> String>) -> Result<u64, Refused> {
        let kind = self.recorded(dimension, pos).ok_or(Refused::Gone)?;
        let radius = match block {
            Some(block) => kind.power(pos, block).map_err(Refused::Structure)?,
            None if kind == BombKind::Sunbreaker => {
                let record = self.bombs.iter().find(|b| b.dimension == dimension && b.pos == pos);
                match record.and_then(|b| b.power) {
                    Some(Ok(radius)) => radius,
                    Some(Err(problem)) => return Err(Refused::Structure(problem)),
                    None => return Err(Refused::Unread),
                }
            }
            None => kind.radius(),
        };
        let id = self.fire(dimension, pos, radius)?;
        self.forget(dimension, pos);
        self.unplace(dimension, pos);
        Ok(id)
    }
}

// ---- flights -----------------------------------------------------------------
//
// A missile counts down in its silo, climbs out of it, flies unseen and falls
// on its target; a bomber passes over a beacon and drops its bomb. Where each
// is at a tick is a rule of the tick alone, so the glue only asks and moves a
// display, and a flight saved mid-way goes on from the same tick.

/// The missile, an item put on the launch pad.
pub const MISSILE: &str = "big_nukes:missile";
/// The block over the shaft, taken away while the missile goes out.
pub const SILO_HATCH: &str = "big_nukes:silo_hatch";
/// The shortest and longest flight across the ground.
pub const MISSILE_MIN_RANGE: u32 = 100;
pub const MISSILE_MAX_RANGE: u32 = 4096;
/// The countdown after the launch, and the ticks before its end that the hatch opens.
pub const LAUNCH_COUNTDOWN: u64 = 100;
pub const HATCH_OPEN_AT: u64 = 40;
/// The ticks into the climb that the hatch shuts again: the missile is 75 blocks up.
pub const HATCH_SHUT_AT: u64 = 60;
/// The climb: its ticks and its height.
pub const ASCENT_TICKS: u64 = 120;
pub const ASCENT_HEIGHT: f64 = 300.0;
/// The last ticks of the climb lean the missile toward its target, this far from upright.
const TILT_TICKS: u64 = 40;
const TILT_DEGREES: f64 = 30.0;
/// The unseen part: blocks a tick, and the fewest ticks it takes.
pub const CRUISE_SPEED: f64 = 16.0;
pub const CRUISE_MIN: u64 = 40;
/// The fall: its ticks, from this far short of the target and this high over its ground.
pub const DESCENT_TICKS: u64 = 80;
pub const DESCENT_BACK: f64 = 200.0;
pub const DESCENT_HEIGHT: f64 = 320.0;
/// The air a shaft needs over its pad, under its hatch.
pub const SHAFT_ROOM: u32 = 6;
/// How far from a console its pad may be.
pub const CONSOLE_REACH: i32 = 8;
/// A charged beacon's countdown to the bomber's pass.
pub const STRIKE_COUNTDOWN: u64 = 600;
/// The bomber: how high over the beacon, blocks a tick, and how far out it comes in.
pub const PLANE_HEIGHT: f64 = 120.0;
pub const PLANE_SPEED: f64 = 3.0;
pub const PLANE_START: f64 = 150.0;
/// A dropped bomb: its speed kept a tick, and the speed down it gains a tick.
pub const BOMB_DRAG: f64 = 0.9;
pub const BOMB_GRAVITY: f64 = 0.08;
/// The silo's blocks and the bomber's beacon, declared in code like the machines.
pub const LAUNCH_PAD: &str = "big_nukes:launch_pad";
pub const LAUNCH_CONSOLE: &str = "big_nukes:launch_console";
pub const STRIKE_BEACON: &str = "big_nukes:strike_beacon";
pub const SILO_BLOCKS: [&str; 4] = [LAUNCH_PAD, SILO_HATCH, LAUNCH_CONSOLE, STRIKE_BEACON];
/// The pad's slots: the missile and its warhead. Its menu type is
/// content/data/big_nukes/horizon/menu/launch_pad.json, named as the block.
pub const PAD_MISSILE_SLOT: usize = 0;
pub const PAD_WARHEAD_SLOT: usize = 1;
pub const PAD_SLOTS: usize = 2;
/// The console's menu type, content/data/big_nukes/horizon/menu/console.json: one hidden slot, as a keypad's.
pub const CONSOLE_MENU: &str = "big_nukes:console";
/// What the flying displays show: items of their own, out of the creative tab, whose 3D models lie along z,
/// nose forward. The display scales them up; the missile's goes forward by `MISSILE_LIFT` so its tail is
/// the display's place.
pub const MISSILE_MODEL: &str = "big_nukes:missile_model";
pub const BOMBER_MODEL: &str = "big_nukes:bomber_model";
pub const MODEL_ITEMS: [&str; 2] = [MISSILE_MODEL, BOMBER_MODEL];
pub const MISSILE_SCALE: f32 = 2.0;
pub const MISSILE_LIFT: f32 = 3.0;
pub const BOMBER_SCALE: f32 = 4.0;
/// The launch's roar and the bomber's drone: the assets half's sounds.
pub const MISSILE_LAUNCH: &str = "big_nukes:missile_launch";
pub const BOMBER_FLYBY: &str = "big_nukes:bomber_flyby";
/// The tag of every display a flight shows, so the ones a closed world left are found.
pub const FLIGHT_TAG: &str = "big_nukes_flight";
/// The farthest place from the world's middle, as `/summon` and `/forceload` take it.
const WORLD_EDGE: i64 = 29_999_984;

/// What carries a bomb to its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Carrier {
    Missile,
    Bomber,
}

/// A bomb on its way, as the world keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flight {
    pub id: u64,
    pub carrier: Carrier,
    pub dimension: String,
    /// A missile's: the block over its pad, where its tail stands. A bomber's: its beacon.
    pub from: [i32; 3],
    pub to: [i32; 2],
    pub kind: BombKind,
    /// The tick of the launch, or of the beacon's charge.
    pub start: u64,
    /// The height of the target's ground, once its chunk has been read.
    pub surface: Option<i32>,
    pub seed: u64,
    /// The hatch's height while it is to be opened or shut again.
    #[serde(default)]
    pub hatch: Option<i32>,
    /// Whether the flight took the hatch away and owes it back; None for a flight saved before this was kept,
    /// which puts a hatch back on the air as it used to.
    #[serde(default)]
    pub hatch_moved: Option<bool>,
    /// A bomber's: its bomb has been taken out of the beacon's slot.
    #[serde(default)]
    pub emptied: bool,
}

/// What a flight does to its hatch (`Flight::hatch_act`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HatchAct {
    /// Taken away: the shaft opens.
    Open,
    /// Put back.
    Shut,
    /// Left as it is, and forgotten.
    Forget,
}

/// Where a missile is at a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leg {
    Countdown { left: u64 },
    /// `pos` is the missile's tail; `yaw` and `pitch` where its nose points, as the game reckons a look.
    Ascent { pos: [f64; 3], yaw: f32, pitch: f32 },
    Cruise,
    Descent { pos: [f64; 3], yaw: f32, pitch: f32 },
    Impact { at: [i32; 3] },
}

/// The yaw of a look along (`dx`, `dz`): 0 south, -90 east.
fn yaw_of(dx: f64, dz: f64) -> f32 {
    (-dx).atan2(dz).to_degrees() as f32
}

impl Flight {
    /// The tick the missile leaves its pad.
    pub fn liftoff(&self) -> u64 {
        self.start + LAUNCH_COUNTDOWN
    }

    /// The tick its climb ends, out of sight.
    pub fn climbed(&self) -> u64 {
        self.liftoff() + ASCENT_TICKS
    }

    /// The blocks across the ground to the target.
    fn range(&self) -> f64 {
        f64::from(self.to[0] - self.from[0]).hypot(f64::from(self.to[1] - self.from[2]))
    }

    /// The way to the target across the ground, a unit; south when it is straight up.
    fn heading(&self) -> [f64; 2] {
        let r = self.range();
        if r == 0.0 {
            return [0.0, 1.0];
        }
        [f64::from(self.to[0] - self.from[0]) / r, f64::from(self.to[1] - self.from[2]) / r]
    }

    /// The tick it shows again over its target.
    pub fn descends_at(&self) -> u64 {
        self.climbed() + ((self.range() / CRUISE_SPEED).ceil() as u64).max(CRUISE_MIN)
    }

    /// The tick it hits, when its target's ground is known.
    pub fn lands_at(&self) -> u64 {
        self.descends_at() + DESCENT_TICKS
    }

    /// Whether the hatch stands open: from `HATCH_OPEN_AT` before the liftoff to `HATCH_SHUT_AT` after it.
    pub fn hatch_open(&self, now: u64) -> bool {
        (self.liftoff() - HATCH_OPEN_AT..=self.liftoff() + HATCH_SHUT_AT).contains(&now)
    }

    /// What is done to the hatch at `now`, `here` being the block in its place, None while its chunk isn't
    /// loaded (where everything reads as air): it waits for it. The hatch is taken away while it stands open,
    /// and after, only the one taken is put back, on the air: one mined before, or a block put in its place,
    /// and the flight forgets it.
    pub fn hatch_act(&self, now: u64, here: Option<&str>) -> Option<HatchAct> {
        self.hatch?;
        let here = here?;
        if self.hatch_open(now) {
            (self.hatch_moved != Some(true) && here == SILO_HATCH).then_some(HatchAct::Open)
        } else if now > self.liftoff() {
            Some(if self.hatch_moved != Some(false) && is_air(here) { HatchAct::Shut } else { HatchAct::Forget })
        } else {
            None
        }
    }

    /// The ground a bomber's bomb falls to: its beacon's column, under the Nether's roof; the beacon's own
    /// height while the column isn't read.
    pub fn bomb_ground(&self) -> i32 {
        self.ground().unwrap_or(self.from[1])
    }

    /// The target's ground, under the Nether's roof.
    fn ground(&self) -> Option<i32> {
        let surface = self.surface?;
        Some(world_of(&self.dimension).map_or(surface, |w| surface.min(w.roof)))
    }

    /// The climb `t` ticks after the liftoff, between ticks too (the window's camera): the tail, yaw and pitch.
    pub fn climb(&self, t: f64) -> ([f64; 3], f32, f32) {
        let height = |t: f64| ASCENT_HEIGHT * ((t + 1.0) / ASCENT_TICKS as f64).clamp(0.0, 1.0).powi(2);
        let lean_from = (ASCENT_TICKS - TILT_TICKS) as f64;
        let tilt = |t: f64| TILT_DEGREES * ((t - lean_from) / TILT_TICKS as f64).clamp(0.0, 1.0);
        // It drifts the way it leans, by what it climbs each tick.
        let mut drift = 0.0;
        let mut k = lean_from;
        while k < t {
            let next = (k + 1.0).min(t);
            drift += (height(next) - height(k)) * tilt(k).to_radians().tan();
            k = next;
        }
        let h = self.heading();
        let pos = [
            f64::from(self.from[0]) + 0.5 + h[0] * drift,
            f64::from(self.from[1]) + height(t),
            f64::from(self.from[2]) + 0.5 + h[1] * drift,
        ];
        (pos, yaw_of(h[0], h[1]), (tilt(t) - 90.0) as f32)
    }

    /// The fall `t` ticks after it shows over the target, straight to its ground.
    fn fall(&self, ground: i32, t: u64) -> ([f64; 3], f32, f32) {
        let h = self.heading();
        let end = [f64::from(self.to[0]) + 0.5, f64::from(ground), f64::from(self.to[1]) + 0.5];
        let top = [end[0] - h[0] * DESCENT_BACK, end[1] + DESCENT_HEIGHT, end[2] - h[1] * DESCENT_BACK];
        let s = t as f64 / (DESCENT_TICKS - 1) as f64;
        let pos = [0, 1, 2].map(|i| top[i] + (end[i] - top[i]) * s);
        (pos, yaw_of(h[0], h[1]), DESCENT_HEIGHT.atan2(DESCENT_BACK).to_degrees() as f32)
    }

    /// Where the missile is at `now`. Without its target's ground it stays out of sight.
    pub fn leg(&self, now: u64) -> Leg {
        if now < self.liftoff() {
            return Leg::Countdown { left: self.liftoff() - now };
        }
        if now < self.climbed() {
            let (pos, yaw, pitch) = self.climb((now - self.liftoff()) as f64);
            return Leg::Ascent { pos, yaw, pitch };
        }
        let Some(ground) = self.ground() else {
            return Leg::Cruise;
        };
        if now < self.descends_at() {
            return Leg::Cruise;
        }
        if now < self.lands_at() {
            let (pos, yaw, pitch) = self.fall(ground, now - self.descends_at());
            return Leg::Descent { pos, yaw, pitch };
        }
        Leg::Impact { at: [self.to[0], ground, self.to[1]] }
    }
}

/// What a shaft lacks for a launch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaftProblem {
    NoPad,
    /// Only this much air over the pad.
    LowRoom(u32),
    NoHatch,
    NoSky,
    NoMissile,
    NoWarhead,
}

fn is_air(block: &str) -> bool {
    matches!(block, "air" | "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// The blocks over a pad, from the one above it up to the sky (the Nether's roof): the hatch's height over the
/// pad, or what is wrong. The shaft is `SHAFT_ROOM` or more of air under the hatch, and only air is above it.
pub fn shaft(column_above_pad: &[&str]) -> Result<i32, ShaftProblem> {
    let room = column_above_pad.iter().take_while(|b| is_air(b)).count();
    let low = (room as u32) < SHAFT_ROOM;
    match column_above_pad.get(room) {
        Some(&b) if b == SILO_HATCH && !low => {
            if column_above_pad[room + 1..].iter().all(|b| is_air(b)) {
                Ok(room as i32 + 1)
            } else {
                Err(ShaftProblem::NoSky)
            }
        }
        _ if low => Err(ShaftProblem::LowRoom(room as u32)),
        _ => Err(ShaftProblem::NoHatch),
    }
}

impl BombKind {
    /// Whether it goes on a missile or a bomber: the demolition charge is too small for
    /// either, and the Sunbreaker is built where it stands.
    pub fn flies(self) -> bool {
        matches!(self, BombKind::Tactical | BombKind::Atomic)
    }
}

/// What a pad's slot takes: the missile in its slot, a warhead that flies in the other.
pub fn pad_takes(slot: usize, item: &str) -> bool {
    match slot {
        PAD_MISSILE_SLOT => item == MISSILE,
        PAD_WARHEAD_SLOT => BombKind::of_block(item).is_some_and(BombKind::flies),
        _ => false,
    }
}

/// What a pad holds in its two slots: the warhead it launches, or what it lacks.
pub fn pad_load(missile: Option<&str>, warhead: Option<&str>) -> Result<BombKind, ShaftProblem> {
    if missile != Some(MISSILE) {
        return Err(ShaftProblem::NoMissile);
    }
    warhead.and_then(BombKind::of_block).filter(|k| k.flies()).ok_or(ShaftProblem::NoWarhead)
}

/// A launch console's code and its wrong tries, as the world keeps them (`Saved.consoles`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Console {
    /// Kept from the first launch that goes.
    pub code: Option<String>,
    pub tries: u8,
    pub locked_until: u64,
}

/// Why a console doesn't launch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchRefusal {
    Empty,
    NotANumber,
    /// A code shorter than `CODE_MIN_DIGITS` or longer than `CODE_DIGITS`.
    CodeLength,
    OutsideWorld,
    /// The target's distance across the ground.
    TooNear(u32),
    TooFar(u32),
    /// The tries left before the lock.
    WrongCode(u8),
    /// The ticks left of the lock.
    Locked(u64),
    Maxpower,
}

/// The key of a console in `Saved.consoles`.
pub fn console_key(dimension: &str, pos: [i32; 3]) -> String {
    format!("{dimension} {} {} {}", pos[0], pos[1], pos[2])
}

/// A code's digits: `CODE_MIN_DIGITS` to `CODE_DIGITS` of them.
fn code_shape(code: &str) -> Result<(), LaunchRefusal> {
    if code.is_empty() {
        Err(LaunchRefusal::Empty)
    } else if !code.bytes().all(|b| b.is_ascii_digit()) {
        Err(LaunchRefusal::NotANumber)
    } else if !(CODE_MIN_DIGITS..=CODE_DIGITS).contains(&code.len()) {
        Err(LaunchRefusal::CodeLength)
    } else {
        Ok(())
    }
}

/// A launch typed on a console whose pad stands at `from` (x, z): the target and the code, or why not.
pub fn launch_request(from: [i32; 2], x: &str, z: &str, code: &str) -> Result<([i32; 2], String), LaunchRefusal> {
    if x.is_empty() || z.is_empty() || code.is_empty() {
        return Err(LaunchRefusal::Empty);
    }
    let coordinate = |s: &str| {
        let digits = s.strip_prefix('-').unwrap_or(s);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(LaunchRefusal::NotANumber);
        }
        match s.parse::<i64>() {
            Ok(v) if v.abs() <= WORLD_EDGE => Ok(v as i32),
            _ => Err(LaunchRefusal::OutsideWorld),
        }
    };
    let to = [coordinate(x)?, coordinate(z)?];
    let far = (f64::from(to[0]) - f64::from(from[0])).hypot(f64::from(to[1]) - f64::from(from[1])).round();
    let far = far.min(f64::from(u32::MAX)) as u32;
    if far < MISSILE_MIN_RANGE {
        return Err(LaunchRefusal::TooNear(far));
    }
    if far > MISSILE_MAX_RANGE {
        return Err(LaunchRefusal::TooFar(far));
    }
    code_shape(code)?;
    Ok((to, code.to_owned()))
}

impl Console {
    /// A launch's code. The first one a console is given becomes its code; then only that one passes, and
    /// `TRIES` wrong ones in a row lock it for `LOCK_TICKS`.
    pub fn check(&mut self, code: &str, now: u64) -> Result<(), LaunchRefusal> {
        self.locked_until = self.locked_until.min(now + LOCK_TICKS);
        if now < self.locked_until {
            return Err(LaunchRefusal::Locked(self.locked_until - now));
        }
        code_shape(code)?;
        match &self.code {
            // No code yet: any passes, and the first launch keeps its own (`keep`).
            None => {}
            Some(c) if c == code => {}
            Some(_) => {
                self.tries += 1;
                if self.tries < TRIES {
                    return Err(LaunchRefusal::WrongCode(TRIES - self.tries));
                }
                self.tries = 0;
                self.locked_until = now + LOCK_TICKS;
                return Err(LaunchRefusal::Locked(LOCK_TICKS));
            }
        }
        self.tries = 0;
        Ok(())
    }

    /// A launch went with `code`: a console without a code keeps it from now on.
    pub fn keep(&mut self, code: &str) {
        self.code.get_or_insert_with(|| code.to_owned());
    }
}

/// Whether the console's screen keeps `key` from the game: all but Escape, which closes it, and the function
/// keys (screenshot, debug screen, full screen).
pub fn console_takes(key: &str) -> bool {
    let function = key.strip_prefix("key.keyboard.f").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    key != "key.keyboard.escape" && !function
}

/// A bomber's pass over its beacon, worked out tick by tick when it is made: the plane comes in from
/// `PLANE_START` blocks out, from a way the seed picks, `PLANE_HEIGHT` over the beacon, and flies straight
/// over it; the bomb leaves it at the tick whose fall ends nearest the beacon.
#[derive(Clone, Debug, PartialEq)]
pub struct Pass {
    start: u64,
    /// The plane's place at each tick from `start`.
    plane: Vec<[f64; 3]>,
    yaw: f32,
    /// The tick the bomb leaves the plane.
    pub release: u64,
    /// The bomb's place at each tick from `release`.
    bomb: Vec<[f64; 3]>,
}

/// How far below the beacon a fall is worked out.
const FALL_DEPTH: f64 = 256.0;

impl Pass {
    pub fn new(beacon: [i32; 3], start: u64, seed: u64) -> Pass {
        let angle = (mix(seed ^ 0xb0b0) >> 11) as f64 / (1u64 << 53) as f64 * std::f64::consts::TAU;
        let out = [angle.cos(), angle.sin()];
        let over = [f64::from(beacon[0]) + 0.5, f64::from(beacon[1]) + PLANE_HEIGHT, f64::from(beacon[2]) + 0.5];
        let v = [-out[0] * PLANE_SPEED, 0.0, -out[1] * PLANE_SPEED];
        // Until it is PLANE_START past the beacon.
        let ticks = (2.0 * PLANE_START / PLANE_SPEED).ceil() as usize;
        let plane: Vec<[f64; 3]> = (0..ticks)
            .map(|k| [over[0] + out[0] * PLANE_START + v[0] * k as f64, over[1], over[2] + out[1] * PLANE_START + v[2] * k as f64])
            .collect();
        // A fall from the plane, the same from any tick: its steps from where the bomb leaves.
        let mut fall = vec![[0.0; 3]];
        let (mut at, mut speed) = ([0.0; 3], v);
        while at[1] > -(PLANE_HEIGHT + FALL_DEPTH) {
            // As a falling item: the pull, the step, the drag.
            speed[1] -= BOMB_GRAVITY;
            at = [0, 1, 2].map(|i| at[i] + speed[i]);
            speed = speed.map(|s| s * BOMB_DRAG);
            fall.push(at);
        }
        let lands = fall.iter().find(|s| s[1] <= -PLANE_HEIGHT).copied().unwrap_or([0.0; 3]);
        let miss = |k: usize| (plane[k][0] + lands[0] - over[0]).hypot(plane[k][2] + lands[2] - over[2]);
        let best = (0..plane.len()).min_by(|a, b| miss(*a).total_cmp(&miss(*b))).unwrap_or(0);
        let bomb = fall.iter().map(|s| [0, 1, 2].map(|i| plane[best][i] + s[i])).collect();
        Pass { start, plane, yaw: yaw_of(v[0], v[2]), release: start + best as u64, bomb }
    }

    /// The plane's place and yaw at `now`, while it is in the sky.
    pub fn plane(&self, now: u64) -> Option<([f64; 3], f32)> {
        let k = usize::try_from(now.checked_sub(self.start)?).ok()?;
        self.plane.get(k).map(|p| (*p, self.yaw))
    }

    /// The bomb's place at `now`, from its release.
    pub fn bomb(&self, now: u64) -> Option<[f64; 3]> {
        let k = usize::try_from(now.checked_sub(self.release)?).ok()?;
        self.bomb.get(k).copied()
    }

    /// The tick the bomb reaches ground at height `surface`.
    pub fn impact(&self, surface: i32) -> Option<u64> {
        self.bomb.iter().position(|b| b[1] <= f64::from(surface)).map(|k| self.release + k as u64)
    }

    /// The tick the plane comes into the sky.
    pub fn arrives(&self) -> u64 {
        self.start
    }
}

// ---- the launch console ----------------------------------------------------------
//
// Its screen is the window's: the typed fields live there, and a launch goes to
// the server as a mod message on `LAUNCH_CHANNEL`. The server answers on
// `CONSOLE_CHANNEL`, and the menu's data slots show the shaft's state.

/// A launch, from the window to the server.
pub const LAUNCH_CHANNEL: &str = "big_nukes:launch";
/// The console's news, from the server to the window: where it is, the answer to a launch, the missile for
/// the camera.
pub const CONSOLE_CHANNEL: &str = "big_nukes:console";
/// The console screen's data slots: its state, a detail and a number (`ConsoleState::data`).
pub const CONSOLE_DATA: usize = 3;
/// The screen's fields: the target's x and z, and the code.
pub const FIELD_X: usize = 0;
pub const FIELD_Z: usize = 1;
pub const FIELD_CODE: usize = 2;
/// The most characters a field takes.
pub const FIELD_CHARS: usize = 8;

/// What the console screen has typed, and where.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConsoleInput {
    pub fields: [String; 3],
    pub focus: usize,
}

/// What a key did on the console screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyed {
    Edited,
    Launch,
    /// Not one of the console's keys.
    Other,
}

impl ConsoleInput {
    /// A character typed into the focused field: digits, and a minus first in x or z. Whether it went in.
    pub fn typed(&mut self, c: char) -> bool {
        let field = &mut self.fields[self.focus.min(FIELD_CODE)];
        let fits = c.is_ascii_digit() || (c == '-' && field.is_empty() && self.focus != FIELD_CODE);
        if !fits || field.len() >= FIELD_CHARS {
            return false;
        }
        field.push(c);
        true
    }

    /// A key by the game's name: Tab goes to the next field, Backspace takes the last character, Enter launches.
    pub fn key(&mut self, key: &str) -> Keyed {
        match key {
            "key.keyboard.tab" => {
                self.focus = (self.focus + 1) % self.fields.len();
                Keyed::Edited
            }
            "key.keyboard.backspace" => {
                self.fields[self.focus.min(FIELD_CODE)].pop();
                Keyed::Edited
            }
            "key.keyboard.enter" | "key.keyboard.keypad.enter" => Keyed::Launch,
            _ => Keyed::Other,
        }
    }

    pub fn clear(&mut self) {
        self.fields[self.focus.min(FIELD_CODE)].clear();
    }

    pub fn set_focus(&mut self, field: usize) {
        if field < self.fields.len() {
            self.focus = field;
        }
    }

    /// The launch the fields ask of the console at `console`.
    pub fn request(&self, console: [i32; 3], camera: bool) -> LaunchMessage {
        let [x, z, code] = self.fields.clone();
        LaunchMessage { console, x, z, code, camera }
    }
}

/// A launch typed on a console: its place, the fields as typed, and whether the window wants the camera.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchMessage {
    pub console: [i32; 3],
    pub x: String,
    pub z: String,
    pub code: String,
    pub camera: bool,
}

impl LaunchMessage {
    pub fn bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// A launch as a client sent it: none when it isn't one, or a field is longer than the screen types.
pub fn read_launch(data: &[u8]) -> Option<LaunchMessage> {
    let m: LaunchMessage = serde_json::from_slice(data).ok()?;
    [&m.x, &m.z, &m.code].iter().all(|f| f.len() <= FIELD_CHARS).then_some(m)
}

/// A line to show or tell: a language key and its arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub key: String,
    pub args: Vec<String>,
}

impl Reply {
    pub fn new(key: &str, args: &[&dyn ToString]) -> Reply {
        Reply { key: key.to_owned(), args: args.iter().map(|a| a.to_string()).collect() }
    }
}

/// What the server tells a window of its console.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConsoleNews {
    /// The console it opened.
    pub at: Option<[i32; 3]>,
    /// The answer to its launch.
    pub reply: Option<Reply>,
    /// The missile it launched, for the camera, with the server's tick then.
    pub missile: Option<Flight>,
    pub now: u64,
}

/// Seconds, rounded up, as the screen shows them.
fn seconds_of(ticks: u64) -> u64 {
    ticks.div_ceil(SECOND)
}

impl LaunchRefusal {
    /// The line that says why.
    pub fn line(self) -> Reply {
        let digits = (CODE_MIN_DIGITS, CODE_DIGITS);
        match self {
            LaunchRefusal::Empty => Reply::new("big_nukes.console.empty", &[]),
            LaunchRefusal::NotANumber => Reply::new("big_nukes.console.not_a_number", &[]),
            LaunchRefusal::CodeLength => Reply::new("big_nukes.console.code_length", &[&digits.0, &digits.1]),
            LaunchRefusal::OutsideWorld => Reply::new("big_nukes.console.outside_world", &[]),
            LaunchRefusal::TooNear(d) => Reply::new("big_nukes.console.too_near", &[&d, &MISSILE_MIN_RANGE]),
            LaunchRefusal::TooFar(d) => Reply::new("big_nukes.console.too_far", &[&d, &MISSILE_MAX_RANGE]),
            LaunchRefusal::WrongCode(left) => Reply::new("big_nukes.console.wrong_code", &[&left]),
            LaunchRefusal::Locked(ticks) => Reply::new("big_nukes.console.locked", &[&seconds_of(ticks)]),
            LaunchRefusal::Maxpower => Reply::new("big_nukes.console.maxpower", &[]),
        }
    }
}

/// What a console's screen shows: the shaft's state, a countdown or a lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleState {
    Ready,
    Shaft(ShaftProblem),
    /// Seconds to the liftoff.
    Countdown(u64),
    Flying,
    /// Seconds of the lock left.
    Locked(u64),
}

const SHAFT_PROBLEMS: [ShaftProblem; 6] = [
    ShaftProblem::NoPad,
    ShaftProblem::LowRoom(0),
    ShaftProblem::NoHatch,
    ShaftProblem::NoSky,
    ShaftProblem::NoMissile,
    ShaftProblem::NoWarhead,
];

impl ConsoleState {
    pub fn counting(ticks: u64) -> ConsoleState {
        ConsoleState::Countdown(seconds_of(ticks))
    }

    pub fn locked(ticks: u64) -> ConsoleState {
        ConsoleState::Locked(seconds_of(ticks))
    }

    /// The menu's data slots, within the 16 bits a slot has on the wire.
    pub fn data(self) -> [i32; CONSOLE_DATA] {
        let short = |v: u64| v.min(i16::MAX as u64) as i32;
        match self {
            ConsoleState::Ready => [0, 0, 0],
            ConsoleState::Shaft(p) => {
                let which = SHAFT_PROBLEMS.iter().position(|q| std::mem::discriminant(q) == std::mem::discriminant(&p)).unwrap_or(0);
                let room = if let ShaftProblem::LowRoom(n) = p { short(u64::from(n)) } else { 0 };
                [1, which as i32, room]
            }
            ConsoleState::Countdown(s) => [2, short(s), 0],
            ConsoleState::Flying => [3, 0, 0],
            ConsoleState::Locked(s) => [4, short(s), 0],
        }
    }

    pub fn of_data(data: &[i32]) -> Option<ConsoleState> {
        let get = |i: usize| data.get(i).copied();
        let n = |i: usize| get(i).map(|v| v.max(0) as u64);
        Some(match get(0)? {
            0 => ConsoleState::Ready,
            1 => match *SHAFT_PROBLEMS.get(usize::try_from(get(1)?).ok()?)? {
                ShaftProblem::LowRoom(_) => ConsoleState::Shaft(ShaftProblem::LowRoom(n(2)? as u32)),
                p => ConsoleState::Shaft(p),
            },
            2 => ConsoleState::Countdown(n(1)?),
            3 => ConsoleState::Flying,
            4 => ConsoleState::Locked(n(1)?),
            _ => return None,
        })
    }

    /// The screen's status line.
    pub fn line(&self) -> Reply {
        match *self {
            ConsoleState::Ready => Reply::new("big_nukes.console.ready", &[]),
            ConsoleState::Shaft(p) => p.line(),
            ConsoleState::Countdown(s) => Reply::new("big_nukes.console.countdown", &[&s]),
            ConsoleState::Flying => Reply::new("big_nukes.console.flying", &[]),
            ConsoleState::Locked(s) => Reply::new("big_nukes.console.locked", &[&s]),
        }
    }
}

impl ShaftProblem {
    pub fn line(self) -> Reply {
        match self {
            ShaftProblem::NoPad => Reply::new("big_nukes.console.no_pad", &[&CONSOLE_REACH]),
            ShaftProblem::LowRoom(n) => Reply::new("big_nukes.console.low_room", &[&n, &SHAFT_ROOM]),
            ShaftProblem::NoHatch => Reply::new("big_nukes.console.no_hatch", &[]),
            ShaftProblem::NoSky => Reply::new("big_nukes.console.no_sky", &[]),
            ShaftProblem::NoMissile => Reply::new("big_nukes.console.no_missile", &[]),
            ShaftProblem::NoWarhead => Reply::new("big_nukes.console.no_warhead", &[]),
        }
    }
}

impl Saved {
    /// A missile launched now from the pad under `from`, the hatch at height `hatch`, within the operator's limit.
    pub fn launch(&mut self, dimension: &str, from: [i32; 3], to: [i32; 2], kind: BombKind, hatch: i32) -> Result<Flight, LaunchRefusal> {
        if !self.allows(kind.radius()) {
            return Err(LaunchRefusal::Maxpower);
        }
        self.next_id += 1;
        let id = self.next_id;
        let flight = Flight {
            id,
            carrier: Carrier::Missile,
            dimension: dimension.to_owned(),
            from,
            to,
            kind,
            start: self.tick,
            surface: None,
            seed: mix(self.tick ^ mix(id)),
            hatch: Some(hatch),
            hatch_moved: Some(false),
            emptied: false,
        };
        self.flights.push(flight.clone());
        Ok(flight)
    }

    /// The missile still in the shaft over the pad under `from`: counting down or climbing out.
    pub fn launching(&self, dimension: &str, from: [i32; 3], now: u64) -> Option<&Flight> {
        self.flights.iter().find(|f| f.dimension == dimension && f.from == from && now < f.climbed())
    }

    pub fn pad_busy(&self, dimension: &str, from: [i32; 3], now: u64) -> bool {
        self.launching(dimension, from, now).is_some()
    }
}

// ---- the target's chunk ------------------------------------------------------------
//
// A missile's target is kept loaded with `/forceload` from its liftoff, so its
// ground can be read and its blast's center worked, and its pad from the launch
// until the hatch is back, so the hatch isn't lost; the world keeps the forced
// chunks (`Saved.forced`), and each is let go once its wave has changed its
// center's chunk, or at once for a flight that never went off.

/// Ticks past its descent a missile waits for its target's ground before it lands on a guess.
pub const SURFACE_WAIT: u64 = 200;
/// The guess: about the sea's height.
pub const SURFACE_GUESS: i32 = 64;

impl Flight {
    /// Whether the target's ground is still unread long after the missile should have come down.
    pub fn surface_overdue(&self, now: u64) -> bool {
        self.surface.is_none() && now >= self.descends_at() + SURFACE_WAIT
    }

    pub fn chunk_to(&self) -> [i32; 2] {
        chunk_of(self.to[0], self.to[1])
    }
}

/// A chunk a flight keeps loaded, and the blast's center once it went off.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forced {
    pub id: u64,
    pub dimension: String,
    pub chunk: [i32; 2],
    #[serde(default)]
    pub landed: bool,
    #[serde(default)]
    pub center: Option<[i32; 3]>,
    /// Forced already when the mod came to it (a player's `/forceload`, one flag a chunk): the mod never
    /// lets it go.
    #[serde(default)]
    pub theirs: bool,
}

impl Waves {
    /// Whether the wave of the blast at `center` has changed its center's chunk; true when there is none.
    pub fn center_worked(&self, dimension: &str, center: [i32; 3]) -> bool {
        let chunk = chunk_of(center[0], center[2]);
        self.waves
            .iter()
            .filter(|w| w.blast.dimension == dimension && w.blast.center == center)
            .all(|w| w.reached > 0 && !w.ready.contains(&chunk) && !w.deferred.contains(&chunk))
    }
}

impl Saved {
    /// Flight `id` keeps `chunk` loaded (a missile its pad's and its target's): true when no flight did yet,
    /// so the glue forces it.
    pub fn force(&mut self, id: u64, dimension: &str, chunk: [i32; 2]) -> bool {
        if self.forced.iter().any(|f| f.id == id && f.dimension == dimension && f.chunk == chunk) {
            return false;
        }
        let held = self.forced.iter().find(|f| f.dimension == dimension && f.chunk == chunk);
        let (new, theirs) = (held.is_none(), held.is_some_and(|f| f.theirs));
        self.forced.push(Forced { id, dimension: dimension.to_owned(), chunk, landed: false, center: None, theirs });
        new
    }

    /// The game had `chunk` forced before the mod asked for it: whoever forced it lets it go, not the mod.
    pub fn theirs(&mut self, dimension: &str, chunk: [i32; 2]) {
        for f in self.forced.iter_mut().filter(|f| f.dimension == dimension && f.chunk == chunk) {
            f.theirs = true;
        }
    }

    /// Flight `id` no longer needs `chunk` (a pad's, once its hatch is back): true when the glue is to let it
    /// go, as no other flight holds it and it was the mod's to force.
    pub fn release(&mut self, id: u64, dimension: &str, chunk: [i32; 2]) -> bool {
        let Some(i) = self.forced.iter().position(|f| f.id == id && f.dimension == dimension && f.chunk == chunk) else {
            return false;
        };
        let gone = self.forced.remove(i);
        !gone.theirs && !self.forced.iter().any(|f| f.dimension == dimension && f.chunk == chunk)
    }

    /// Flight `id` went off at `center`, or never will.
    pub fn landed(&mut self, id: u64, center: Option<[i32; 3]>) {
        for f in self.forced.iter_mut().filter(|f| f.id == id) {
            f.landed = true;
            f.center = center;
        }
    }

    /// The dimensions to open when the world is opened again: those other than the overworld where a flight or a
    /// strike is on its way, or a forced chunk waits for its wave. A level nobody has entered isn't open, and its
    /// forced chunks don't tick until it is.
    pub fn to_wake(&self) -> Vec<String> {
        let dimensions = self.flights.iter().chain(&self.beacons).map(|f| &f.dimension).chain(self.forced.iter().map(|f| &f.dimension));
        let set: BTreeSet<&String> = dimensions.filter(|d| d.as_str() != "minecraft:overworld").collect();
        set.into_iter().cloned().collect()
    }

    /// The chunks to let go now: their flights' waves have changed their centers' chunks. A chunk another flight
    /// still keeps stays forced.
    pub fn unforce(&mut self) -> Vec<(String, [i32; 2])> {
        let waves = &self.waves;
        let (done, kept): (Vec<Forced>, Vec<Forced>) = std::mem::take(&mut self.forced)
            .into_iter()
            .partition(|f| f.landed && f.center.is_none_or(|c| waves.center_worked(&f.dimension, c)));
        self.forced = kept;
        let mut free: Vec<(String, [i32; 2])> = Vec::new();
        for f in done {
            let held = self.forced.iter().any(|k| k.dimension == f.dimension && k.chunk == f.chunk);
            if !held && !f.theirs && !free.iter().any(|(d, c)| *d == f.dimension && *c == f.chunk) {
                free.push((f.dimension, f.chunk));
            }
        }
        free
    }
}

// ---- the launch camera -------------------------------------------------------------
//
// For the one who launched it, the window's camera watches the hatch through the
// countdown, then follows the missile up until it is high in the sky. The window
// works it out from the flight and its own clock, between ticks too.

/// Through the countdown the camera stands this far beside the hatch and this far above it.
const CAMERA_ASIDE: f64 = 16.0;
const CAMERA_ABOVE: f64 = 6.0;
/// On the climb it follows this far beside the missile and this far below, until the missile is this high.
const FOLLOW_ASIDE: f64 = 12.0;
const FOLLOW_BELOW: f64 = 20.0;
const FOLLOW_UP_TO: f64 = 200.0;
/// The ticks it takes to close in from the hatch's view to the climb's.
const CLOSE_IN: f64 = 20.0;

/// Where a yaw and pitch look, as the game reckons them.
fn look(yaw: f32, pitch: f32) -> [f64; 3] {
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}

/// The yaw and pitch that look from `from` at `to`.
fn look_at(from: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    (yaw_of(d[0], d[2]), (-d[1].atan2(d[0].hypot(d[2]))).to_degrees() as f32)
}

/// The camera at `now` (the world's ticks, between them too) for a player whose eyes were at `eye` when they
/// launched `flight`: its place, yaw and pitch, beside the hatch on the player's side, then following the missile.
/// None before the launch and once the missile is `FOLLOW_UP_TO` blocks up.
pub fn launch_camera(flight: &Flight, now: f64, eye: [f64; 3]) -> Option<([f64; 3], f32, f32)> {
    if now < flight.start as f64 {
        return None;
    }
    let hatch_y = flight.hatch.unwrap_or(flight.from[1] + SHAFT_ROOM as i32);
    let hatch = [f64::from(flight.from[0]) + 0.5, f64::from(hatch_y) + 0.5, f64::from(flight.from[2]) + 0.5];
    let side = {
        let d = [eye[0] - hatch[0], eye[2] - hatch[2]];
        let len = d[0].hypot(d[1]);
        if len < 1e-6 { [1.0, 0.0] } else { [d[0] / len, d[1] / len] }
    };
    let t = now - flight.liftoff() as f64;
    if t < 0.0 {
        let pos = [hatch[0] + side[0] * CAMERA_ASIDE, hatch[1] + CAMERA_ABOVE, hatch[2] + side[1] * CAMERA_ASIDE];
        let (yaw, pitch) = look_at(pos, hatch);
        return Some((pos, yaw, pitch));
    }
    let (tail, yaw, pitch) = followed_climb(flight, t)?;
    let nose = look(yaw, pitch);
    let middle = [0, 1, 2].map(|i| tail[i] + nose[i] * f64::from(MISSILE_LIFT));
    let aside = CAMERA_ASIDE + (FOLLOW_ASIDE - CAMERA_ASIDE) * (t / CLOSE_IN).min(1.0);
    let pos = [tail[0] + side[0] * aside, (tail[1] - FOLLOW_BELOW).max(hatch[1] + CAMERA_ABOVE), tail[2] + side[1] * aside];
    let (yaw, pitch) = look_at(pos, middle);
    Some((pos, yaw, pitch))
}

/// The climb `t` ticks after the liftoff, while the camera follows it.
fn followed_climb(flight: &Flight, t: f64) -> Option<([f64; 3], f32, f32)> {
    let climb = flight.climb(t);
    (t < ASCENT_TICKS as f64 && climb.0[1] - f64::from(flight.from[1]) <= FOLLOW_UP_TO).then_some(climb)
}

// ---- the strike beacon -------------------------------------------------------------
//
// A right click with a tactical warhead or an atomic bomb charges a beacon: the
// bomb goes into its hidden slot and the world records the strike
// (`Saved.beacons`), whose bomber passes `STRIKE_COUNTDOWN` later. The pass is
// worked out again from the record whenever it is wanted.

/// What a right click on a beacon did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charge {
    Taken(BombKind),
    /// A bomber is already called to it.
    Busy,
    /// Nothing that flies in the hand.
    Not,
    Maxpower { radius: u32, maxpower: u32 },
}

/// The slot of `hotbar` that holds `held`, the stack seen in the main hand, whole (its name and
/// all): the hand's own, where its bomb is taken from. The first, should two be the same.
pub fn hand_slot<T: PartialEq>(hotbar: &[Option<T>], held: &T) -> Option<usize> {
    hotbar.iter().position(|s| s.as_ref() == Some(held))
}

impl Flight {
    /// A bomber's pass over its beacon.
    pub fn pass(&self) -> Pass {
        Pass::new(self.from, self.start + STRIKE_COUNTDOWN, self.seed)
    }

    /// Whether a bomber's bomb is to be taken out of its beacon's slot at `now`: from the release on,
    /// until that is done (`Saved::emptied`).
    pub fn to_empty(&self, now: u64) -> bool {
        self.carrier == Carrier::Bomber && !self.emptied && now >= self.pass().release
    }
}

impl Saved {
    /// The strike called to the beacon at `pos`.
    pub fn beacon_at(&self, dimension: &str, pos: [i32; 3]) -> Option<&Flight> {
        self.beacons.iter().find(|f| f.dimension == dimension && f.from == pos)
    }

    /// A right click on the beacon at `pos` with `held` in hand: a strike recorded now, within the operator's limit.
    pub fn charge(&mut self, dimension: &str, pos: [i32; 3], held: Option<&str>) -> Charge {
        if self.beacon_at(dimension, pos).is_some() {
            return Charge::Busy;
        }
        let Some(kind) = held.and_then(BombKind::of_block).filter(|k| k.flies()) else {
            return Charge::Not;
        };
        if !self.allows(kind.radius()) {
            return Charge::Maxpower { radius: kind.radius(), maxpower: self.maxpower };
        }
        self.next_id += 1;
        let id = self.next_id;
        self.beacons.push(Flight {
            id,
            carrier: Carrier::Bomber,
            dimension: dimension.to_owned(),
            from: pos,
            to: [pos[0], pos[2]],
            kind,
            start: self.tick,
            surface: None,
            seed: mix(self.tick ^ mix(id)),
            hatch: None,
            hatch_moved: None,
            emptied: false,
        });
        Charge::Taken(kind)
    }

    /// The bomb of beacon flight `id` is out of its beacon's slot: whether that is news.
    pub fn emptied(&mut self, id: u64) -> bool {
        match self.beacons.iter_mut().find(|f| f.id == id) {
            Some(f) if !f.emptied => {
                f.emptied = true;
                true
            }
            _ => false,
        }
    }

    /// The beacon at `pos` is gone: while its bomb is in its slot the strike is called off, and the bomb is the
    /// beacon's to drop; once it is out (`emptied`, at the release), the bomb falls anyway. Whatever tick the
    /// beacon was broken at and this is heard, one bomb goes on.
    pub fn uncharge(&mut self, dimension: &str, pos: [i32; 3]) -> Option<BombKind> {
        let f = self.beacon_at(dimension, pos)?;
        if f.emptied {
            return None;
        }
        let (id, kind) = (f.id, f.kind);
        self.beacons.retain(|f| f.id != id);
        self.landed(id, None);
        Some(kind)
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

/// The background at `pos` in `dimension`: every zone's share added up, each less what the decontaminators'
/// circles (`cleans`) cleaned of it.
pub fn background(zones: &[Zone], cleans: &[Clean], dimension: &str, pos: [f64; 3], now: u64) -> f64 {
    zones
        .iter()
        .filter(|z| z.dimension == dimension)
        .map(|z| {
            let bg = z.at(pos, now);
            if bg > 0.0 { bg * clean_factor(cleans, z, dimension, pos) } else { 0.0 }
        })
        .sum()
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
    /// The zones that faded out for good are dropped, and with them the decontaminators' circles no zone
    /// reaches any more: they clean nothing, and never will, as every zone to come is newer than they are.
    /// Whether any zone was.
    pub fn prune_zones(&mut self, now: u64) -> bool {
        let before = self.zones.len();
        self.zones.retain(|z| !z.gone(now));
        if self.zones.len() == before {
            return false;
        }
        let zones = &self.zones;
        self.cleans.retain(|c| zones.iter().any(|z| c.meets(z)));
        true
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
        let bg = background(&self.zones, &self.cleans, dimension, pos, now);
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
            cleans: self.cleans_near(dimension, pos),
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
    /// The decontaminators' circles of the dimension nearest to the player, at most `RAD_ZONES`.
    pub cleans: Vec<Clean>,
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
        background(&self.rad.zones, &self.rad.cleans, dimension, pos, self.rad.now + clock.saturating_sub(self.at))
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

/// The sky's light at the world's time of day `day_time`, 0 to 1, as vanilla's
/// fog colour follows it (`FogRenderer`, from `DimensionType.timeOfDay`): full by
/// day, none at night, eased at dusk and dawn.
pub fn sky_light(day_time: i64) -> f32 {
    let d = (day_time.rem_euclid(24_000) as f64 / 24_000.0 - 0.25).rem_euclid(1.0);
    let e = 0.5 - (d * std::f64::consts::PI).cos() / 2.0;
    let angle = (d * 2.0 + e) / 3.0;
    ((angle * std::f64::consts::TAU).cos() * 2.0 + 0.5).clamp(0.0, 1.0) as f32
}

/// The world's fog (its colour, the start and the end of the fog of the
/// surroundings) with a zone's of `strength` over it: the colour toward the
/// zone's, darkened by the sky's `light` (`sky_light`) as the world's own is, the
/// end in toward 64 blocks and the start toward the eyes. None for no fog, and
/// when the result isn't one the game takes.
pub fn tint_fog(strength: f64, color: [f32; 4], start: f32, end: f32, light: f32) -> Option<([f32; 4], f32, f32)> {
    let s = strength.min(1.0) as f32;
    if !(s > 0.0) || !start.is_finite() || !end.is_finite() {
        return None;
    }
    let light = if light.is_finite() { light.clamp(0.0, 1.0) } else { 1.0 };
    let tint = [FOG_TINT[0] * (light * 0.94 + 0.06), FOG_TINT[1] * (light * 0.94 + 0.06), FOG_TINT[2] * (light * 0.91 + 0.09)];
    let mut tinted = color;
    for i in 0..3 {
        tinted[i] = (color[i] + (tint[i] - color[i]) * s).clamp(0.0, 1.0);
    }
    if start >= FOG_OFF {
        return Some((tinted, start, end));
    }
    let start = start + (start.min(0.0) - start) * s;
    let end = end + (end.min(FOG_END) - end) * s;
    (end > start).then_some((tinted, start, end))
}

// ---- iodine and the decontaminator (plan 9) -------------------------------------
//
// Iodine tablets give an effect of the mod's own: while it is on, the dose takes half
// of what the suit lets in. The decontaminator burns fuel and grows a circle round it,
// kept in `Saved.cleans`: inside it the zones older than the circle give no background,
// and its passes turn the ash on the open ground back to grass.

pub const IODINE: &str = "big_nukes:iodine_tablets";
/// The tablets' consume effect, which `native_consume_effect` hears, and the mob effect
/// it gives: the assets half's `horizon/mob_effect/iodine.json`.
pub const IODINE_EFFECT: &str = "big_nukes:iodine";
/// How long a tablet's effect lasts: another tablet makes it this long again, never longer.
pub const IODINE_TICKS: u64 = 6000;
/// The share of the background the effect keeps out.
pub const IODINE_PROTECTION: f64 = 0.5;

/// The share of the background kept out by a suit that keeps out `suit`, with the iodine's
/// effect on or not: each lets in its share of what the other lets in.
pub fn shielded(suit: f64, iodine: bool) -> f64 {
    let suit = suit.clamp(0.0, 1.0);
    if iodine { 1.0 - (1.0 - suit) * (1.0 - IODINE_PROTECTION) } else { suit }
}

/// The command a tablet runs: the effect on `target` for `IODINE_TICKS`, without its
/// particles. Without the assets half the effect isn't there and the command fails.
pub fn iodine_command(target: &str) -> String {
    format!("effect give {target} {IODINE_EFFECT} {} 0 true", IODINE_TICKS / SECOND)
}

pub const DECONTAMINATOR: &str = "big_nukes:decontaminator";
/// Its one slot, the fuel's. Its menu type is
/// content/data/big_nukes/horizon/menu/decontaminator.json, named as the block.
pub const DECON_FUEL_SLOT: usize = 0;
pub const DECON_SLOTS: usize = 1;
/// The circle's largest radius, in blocks.
pub const CLEAN_RADIUS: u32 = 48;
/// The ticks of burning that grow the circle a block.
pub const CLEAN_GROW_TICKS: u32 = 40;
/// It burns its fuel this many times as fast as a furnace.
pub const CLEAN_FUEL_RATE: u32 = 4;
/// Past the circle the background comes back over this many blocks.
pub const CLEAN_EDGE: f64 = 8.0;
/// The columns a pass looks at, once a second.
pub const CLEAN_BLOCKS_PER_PASS: usize = 64;
/// The screen's radius bar is this wide, in GUI pixels.
pub const RADIUS_BAR: i32 = 108;

/// A decontaminator's circle: the columns within `radius` of `center` (the block's
/// place) are cleaned of every zone no newer than `start`, from bedrock to the sky.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clean {
    pub dimension: String,
    pub center: [i32; 3],
    pub radius: u32,
    pub start: u64,
}

impl Clean {
    /// How far `pos` is from the middle of the center's column, across only.
    fn across(&self, pos: [f64; 3]) -> f64 {
        (pos[0] - (self.center[0] as f64 + 0.5)).hypot(pos[2] - (self.center[2] as f64 + 0.5))
    }

    /// The share of a zone's background it leaves at `pos`: none within its radius, all
    /// `CLEAN_EDGE` blocks past it, eased between. A circle of radius 0 cleans nothing.
    fn left(&self, pos: [f64; 3]) -> f64 {
        if self.radius == 0 {
            return 1.0;
        }
        let t = ((self.across(pos) - self.radius as f64) / CLEAN_EDGE).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Whether `zone` reaches into the circle or its edge.
    fn meets(&self, zone: &Zone) -> bool {
        let centers = f64::from(zone.center[0] - self.center[0]).hypot(f64::from(zone.center[2] - self.center[2]));
        zone.dimension == self.dimension && centers < zone.radius + self.radius as f64 + CLEAN_EDGE
    }
}

/// The share of `zone`'s background left at `pos` in `dimension` by the circles: the
/// lowest any one leaves, so overlapping circles clean their union. A circle cleans only
/// the zones no newer than itself: a later blast's zone is whole over it.
pub fn clean_factor(cleans: &[Clean], zone: &Zone, dimension: &str, pos: [f64; 3]) -> f64 {
    cleans
        .iter()
        .filter(|c| c.dimension == dimension && c.dimension == zone.dimension && zone.start <= c.start)
        .map(|c| c.left(pos))
        .fold(1.0, f64::min)
}

impl Saved {
    /// The decontaminator at `center` has cleaned out to `radius`: its circle is made or
    /// grown, and the radius the circle has is answered. One placed where a circle is takes
    /// that circle on. A zone newer than the circle that reaches it starts a new circle from
    /// nothing (0 is answered): the old one still cleans the zones older than it, and goes
    /// once the new one is as large. Nothing is kept of a circle of radius 0. One grown past its first block
    /// with no circle had it pruned (`prune_zones`) when no zone reached it: it is made again as it was, or,
    /// if a zone has come over it since, which is newer than it, started again from nothing.
    pub fn cleaned(&mut self, dimension: &str, center: [i32; 3], radius: u32) -> u32 {
        let now = self.tick;
        let here = |c: &Clean| c.dimension == dimension && c.center == center;
        let Some(i) = self.cleans.iter().rposition(here) else {
            let new = Clean { dimension: dimension.to_owned(), center, radius, start: now };
            if radius > 1 && self.zones.iter().any(|z| new.meets(z)) {
                return 0;
            }
            if radius > 0 {
                self.cleans.push(new);
            }
            return radius;
        };
        if self.zones.iter().any(|z| z.start > self.cleans[i].start && self.cleans[i].meets(z)) {
            self.cleans.push(Clean { dimension: dimension.to_owned(), center, radius: 0, start: now });
            return 0;
        }
        let c = &mut self.cleans[i];
        c.radius = c.radius.max(radius);
        let (radius, start) = (c.radius, c.start);
        self.cleans.retain(|c| !(here(c) && c.start < start && c.radius <= radius));
        radius
    }

    /// The circles of `dimension` nearest to `pos`, at most `RAD_ZONES`, for the window.
    fn cleans_near(&self, dimension: &str, pos: [f64; 3]) -> Vec<Clean> {
        let outside = |c: &Clean| (c.across(pos) - c.radius as f64).max(0.0);
        let mut near: Vec<&Clean> = self.cleans.iter().filter(|c| c.dimension == dimension && c.radius > 0).collect();
        near.sort_by(|a, b| outside(a).total_cmp(&outside(b)));
        near.truncate(RAD_ZONES);
        near.into_iter().cloned().collect()
    }
}

/// A decontaminator's state, kept in its block entity: the ticks left of the fuel it
/// burns and that fuel's whole burn, the ticks toward the next block, its circle's
/// radius, and how far its passes have gone along `spiral(0, radius)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Decontaminator {
    pub burn: u32,
    pub burn_max: u32,
    pub grow: u32,
    pub radius: u32,
    pub next_column: u32,
}

impl Decontaminator {
    /// One tick. `fuel` is the burn ticks of the fuel in its slot, in a furnace (none, or 0,
    /// when nothing there burns). While it burns the circle grows a block each
    /// `CLEAN_GROW_TICKS` (`Step::made`); a full circle burns nothing more. Without fuel it
    /// stands, keeping what it cleaned.
    pub fn step(&mut self, fuel: Option<u32>) -> Step {
        // A full circle: the cleaning is over, and the flame goes out.
        if self.radius >= CLEAN_RADIUS {
            self.burn = 0;
            return Step::IDLE;
        }
        let mut step = Step::IDLE;
        if self.burn == 0 {
            match fuel {
                Some(ticks) if ticks > 0 => {
                    self.burn = (ticks / CLEAN_FUEL_RATE).max(1);
                    self.burn_max = self.burn;
                    step.took_fuel = true;
                }
                _ => return Step::IDLE,
            }
        }
        self.burn -= 1;
        self.grow += 1;
        if self.grow >= CLEAN_GROW_TICKS {
            self.grow = 0;
            self.radius += 1;
            step.made = true;
            if self.radius >= CLEAN_RADIUS {
                self.burn = 0;
            }
        }
        step
    }

    /// The circle the world keeps for it (`Saved::cleaned`): a larger one is taken on, a
    /// smaller one is a new start, and its passes start again from the center.
    pub fn keep_circle(&mut self, radius: u32) {
        if radius < self.radius {
            self.grow = 0;
            self.next_column = 0;
        }
        self.radius = radius;
    }

    /// The next pass's columns, as offsets from the center: the next
    /// `CLEAN_BLOCKS_PER_PASS` of its circle, none once it has been through it all until
    /// it grows.
    pub fn regrow_pass(&mut self) -> Vec<[i32; 2]> {
        let pass: Vec<[i32; 2]> = spiral(0, self.radius).skip(self.next_column as usize).take(CLEAN_BLOCKS_PER_PASS).collect();
        self.next_column += pass.len() as u32;
        pass
    }

    /// The menu's data slots: `[burn, burn_max, radius, the background at the block in
    /// tenths of a rad a second]`, within the 16 bits a data slot has on the wire.
    pub fn data(&self, background: f64) -> [i32; 4] {
        let short = |v: u32| v.min(i16::MAX as u32) as i32;
        let tenths = (background.max(0.0) * 10.0).round().min(f64::from(i16::MAX)) as i32;
        [short(self.burn), short(self.burn_max), short(self.radius), tenths]
    }
}

/// The filled part of the screen's radius bar, from the left: at least 1 once it has grown.
pub fn radius_bar(radius: i32) -> i32 {
    if radius <= 0 {
        return 0;
    }
    (radius.min(CLEAN_RADIUS as i32) * RADIUS_BAR / CLEAN_RADIUS as i32).max(1)
}

/// The columns of rings `radius_from` to `radius_to` round a center, as offsets, ring by
/// ring outward and round each ring: column (x, z) is in ring k when k − 1 < √(x² + z²) ≤ k,
/// ring 0 being the center alone. So `spiral(0, r)` is every column within r of the center.
pub fn spiral(radius_from: u32, radius_to: u32) -> impl Iterator<Item = [i32; 2]> {
    (radius_from..=radius_to).flat_map(ring_columns)
}

fn ring_columns(k: u32) -> Vec<[i32; 2]> {
    if k == 0 {
        return vec![[0, 0]];
    }
    let k = i64::from(k);
    let (inner, outer) = ((k - 1) * (k - 1), k * k);
    let mut columns = Vec::new();
    for x in -k..=k {
        let most = (outer - x * x).isqrt();
        let rest = inner - x * x;
        let least = if rest < 0 { 0 } else { rest.isqrt() + 1 };
        for z in least..=most {
            columns.push([x as i32, z as i32]);
            if z != 0 {
                columns.push([x as i32, -z as i32]);
            }
        }
    }
    let angle = |c: &[i32; 2]| f64::from(c[1]).atan2(f64::from(c[0])).rem_euclid(std::f64::consts::TAU);
    columns.sort_by(|a, b| angle(a).total_cmp(&angle(b)).then(a.cmp(b)));
    columns
}

/// How deep under each column's surface (`WorldSurface`: the highest block of any kind) a pass
/// looks for the blast's ground.
pub const REGROW_DEPTH: u32 = 24;

/// The blast's ground a pass turns back to grass: its ash, or without the assets half the coarse
/// dirt the ash is a copy of (with the assets half, natural coarse dirt stays).
pub fn regrow_target(ash: bool) -> &'static str {
    if ash { ASH } else { ASH_STAND_IN }
}

/// What the wave leaves over its ash and a pass looks through: air, the snow and fire on it, the
/// leaves of broken trees, plants.
fn see_through(block: &str) -> bool {
    let name = block.split('[').next().unwrap_or(block);
    matches!(
        name,
        "minecraft:air"
            | "minecraft:cave_air"
            | "minecraft:snow"
            | "minecraft:fire"
            | "minecraft:soul_fire"
            | "minecraft:short_grass"
            | "minecraft:tall_grass"
            | "minecraft:fern"
            | "minecraft:large_fern"
            | "minecraft:dead_bush"
            | "minecraft:vine"
    ) || name.ends_with("_leaves")
        || name.ends_with("_sapling")
}

/// In a column read from the top down, the index of the ash a pass turns: the first block that
/// isn't seen through, if it is `target`. Water or any real block over it (a roof) keeps it.
pub fn ash_under<'a>(top_down: impl Iterator<Item = &'a str>, target: &str) -> Option<usize> {
    for (i, block) in top_down.enumerate() {
        if see_through(block) {
            continue;
        }
        return (block.split('[').next() == Some(target)).then_some(i);
    }
    None
}

/// A pass's edit: grass at each of `found` (absolute places a column read found the ash at), only
/// where it is still the blast's ground. An edit naming a block the game lacks is refused whole.
pub fn regrow_edit(found: &[[i32; 3]], ash: bool) -> ChunkEdit {
    let runs = found.iter().map(|&[x, y, z]| Run { x, z, y, height: 1, surface: None, block: 0, only: Some(0) }).collect();
    ChunkEdit { palette: vec!["minecraft:grass_block".to_owned()], filters: vec![regrow_target(ash).to_owned()], runs }
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

// ---- the guide ---------------------------------------------------------------------
//
// A book of the mod's own. A right click with it asks the player's window to open the
// mod's screen (API 111), which draws a two-page spread on `textures/gui/guide.png` with
// a bookmark per chapter down its left edge. Each chapter is laid out as lines (its
// heading, a row of item icons, its text wrapped to the page, the assembly table's
// recipes), the lines are cut into pages, and every chapter starts on a left page.
// The first spread is the contents: a row a chapter, which opens it; Home and a button
// in the footer lead back there.
// Positions are GUI pixels from the book's top left corner unless they say otherwise.

/// The message that opens the guide in a player's window.
pub const GUIDE_CHANNEL: &str = "big_nukes:guide";
/// The screen's texture, in the GUI atlas.
pub const GUIDE_GUI: &str = "big_nukes:gui/guide";

/// A chapter: its title (the bookmark's tooltip and the heading) and text as lang keys,
/// the item on its bookmark, the items pictured under its heading, and whether it shows
/// the assembly table's recipes after its text.
#[derive(Clone, Copy, Debug)]
pub struct Chapter {
    pub title: &'static str,
    pub text: &'static str,
    pub tab: &'static str,
    pub icons: &'static [&'static str],
    pub recipes: bool,
}

pub const CHAPTERS: [Chapter; 9] = [
    Chapter {
        title: "big_nukes.guide.uranium",
        text: "big_nukes.guide.uranium.text",
        tab: RAW_URANIUM,
        icons: &["big_nukes:uranium_ore", "big_nukes:deepslate_uranium_ore", RAW_URANIUM],
        recipes: false,
    },
    Chapter {
        title: "big_nukes.guide.centrifuge",
        text: "big_nukes.guide.centrifuge.text",
        tab: CENTRIFUGE,
        icons: &[CENTRIFUGE, RAW_URANIUM, "minecraft:coal", ENRICHED_URANIUM, WEAPONS_GRADE_URANIUM],
        recipes: false,
    },
    Chapter {
        title: "big_nukes.guide.assembly",
        text: "big_nukes.guide.assembly.text",
        tab: ASSEMBLY_TABLE,
        icons: &[ASSEMBLY_TABLE],
        recipes: true,
    },
    Chapter {
        title: "big_nukes.guide.bombs",
        text: "big_nukes.guide.bombs.text",
        tab: "big_nukes:atomic_bomb",
        icons: &[
            "big_nukes:demolition_charge",
            "big_nukes:tactical_warhead",
            "big_nukes:atomic_bomb",
            SUNBREAKER_CORE,
            IMPLOSION_SHELL,
            NEUTRON_REFLECTOR,
        ],
        recipes: false,
    },
    Chapter {
        title: "big_nukes.guide.detonators",
        text: "big_nukes.guide.detonators.text",
        tab: "big_nukes:keypad_detonator",
        icons: &["big_nukes:fuse_detonator", FLINT_AND_STEEL, "big_nukes:keypad_detonator"],
        recipes: false,
    },
    Chapter {
        title: "big_nukes.guide.remote",
        text: "big_nukes.guide.remote.text",
        tab: REMOTE,
        icons: &["big_nukes:receiver_detonator", REMOTE],
        recipes: false,
    },
    Chapter {
        title: "big_nukes.guide.carriers",
        text: "big_nukes.guide.carriers.text",
        tab: MISSILE,
        icons: &[MISSILE, LAUNCH_PAD, SILO_HATCH, LAUNCH_CONSOLE, STRIKE_BEACON],
        recipes: false,
    },
    Chapter {
        title: "big_nukes.guide.radiation",
        text: "big_nukes.guide.radiation.text",
        tab: GEIGER_COUNTER,
        icons: &[GEIGER_COUNTER, HAZMAT[0].0, HAZMAT[1].0, HAZMAT[2].0, HAZMAT[3].0, ANTI_RAD, IODINE, DECONTAMINATOR],
        recipes: false,
    },
    Chapter {
        title: "big_nukes.guide.commands",
        text: "big_nukes.guide.commands.text",
        tab: "minecraft:command_block",
        icons: &["minecraft:command_block", GUIDE],
        recipes: false,
    },
];

/// The closed book's spread: a dark cover round two pages and a footer.
pub const BOOK_SIZE: [f32; 2] = [288.0, 192.0];
/// The parchment of the left and the right page, `[x, y, w, h]`.
pub const PAGES: [[f32; 4]; 2] = [[6.0, 6.0, 136.0, 164.0], [146.0, 6.0, 136.0, 164.0]];
/// Where each page's text starts, and how wide it may run.
pub const PAGE_X: [f32; 2] = [16.0, 156.0];
pub const PAGE_WIDTH: f32 = 116.0;
pub const PAGE_TOP: f32 = 12.0;
/// A line of the game's font.
pub const ROW: f32 = 9.0;
pub const PAGE_ROWS: usize = 17;
/// A row of item icons, 16 pixels each.
pub const ICON_STEP: f32 = 18.0;
/// The most icons a row holds: a chapter with more pictures them in two rows.
pub const ICONS_A_ROW: usize = 6;
/// A recipe: its 3×3 grid from the page's text, and its arrow and result right of the grid.
pub const RECIPE_X: f32 = 10.0;
pub const RECIPE_ARROW_X: f32 = 58.0;
pub const RECIPE_RESULT_X: f32 = 78.0;
/// A bookmark: closed, it sticks out `-TAB_X` pixels left of the cover; the open
/// chapter's sticks out to `TAB_OPEN_X`. They are drawn under the book, so their
/// right ends tuck under the cover.
pub const TAB_SIZE: [f32; 2] = [26.0, 18.0];
pub const TAB_X: f32 = -22.0;
pub const TAB_OPEN_X: f32 = -26.0;
pub const TAB_TOP: f32 = 11.0;
pub const TAB_STEP: f32 = 19.0;
/// The arrows, in the footer's corners, and the page number's line between them.
pub const PREV_AT: [f32; 4] = [12.0, 176.0, 18.0, 10.0];
pub const NEXT_AT: [f32; 4] = [258.0, 176.0, 18.0, 10.0];
pub const NUMBER_Y: f32 = 177.0;

/// The contents: the book's first spread, and the lang key of its heading and of the
/// footer's button back to it.
pub const CONTENTS_SPREAD: usize = 0;
pub const CONTENTS: &str = "big_nukes.guide.contents";
/// The contents' rows, under the heading, the first half of the chapters on the left
/// page: a row is the chapter's bookmark item, its title, dots and the number of the
/// spread it starts on, at the row's right end.
pub const CONTENTS_TOP: f32 = PAGE_TOP + 2.0 * ROW;
pub const CONTENTS_STEP: f32 = 22.0;
pub const CONTENTS_ROW_H: f32 = 20.0;
/// How far a row stays from its page's edges, and its parts from the row's left end.
pub const CONTENTS_MARGIN: f32 = 3.0;
pub const CONTENTS_ICON: [f32; 2] = [1.0, 2.0];
pub const CONTENTS_TITLE: [f32; 2] = [19.0, 6.0];
/// The room kept between a title and its number.
pub const CONTENTS_GAP: f32 = 3.0;
/// The footer's button back to the contents: its label's left end on the number's line,
/// right of the arrow back, and the widest the label may be.
pub const CONTENTS_BUTTON_X: f32 = 40.0;
pub const CONTENTS_BUTTON_MAX: f32 = 72.0;

/// Contents row `i`'s place, the mouse's too.
pub fn contents_row(i: usize) -> [f32; 4] {
    let left = CHAPTERS.len().div_ceil(2);
    let (page, k) = if i < left { (PAGES[0], i) } else { (PAGES[1], i - left) };
    [page[0] + CONTENTS_MARGIN, CONTENTS_TOP + k as f32 * CONTENTS_STEP, page[2] - 2.0 * CONTENTS_MARGIN, CONTENTS_ROW_H]
}

/// The footer's button for a label `width` wide, the mouse's place too. It covers the
/// gold rule under it, as the page number's gap does.
pub fn contents_button(width: f32) -> [f32; 4] {
    [CONTENTS_BUTTON_X - 3.0, NUMBER_Y - 2.0, width + 6.0, 12.0]
}

/// The texture: the spread at its top left, then a row of closed bookmarks and a row
/// of open ones (one colour a chapter), the arrows (lit ones below), a slot and a
/// recipe's arrow; under them the bookmarks past the eighth, a closed and an open row.
/// tools/make_textures.py draws it with the same numbers.
pub const GUIDE_TEXTURE: [f32; 2] = [288.0, 264.0];
/// The bookmarks a pair of rows holds.
pub const TABS_A_ROW: usize = 8;
pub const SLOT_SRC: [f32; 4] = [244.0, 192.0, 18.0, 18.0];
pub const RECIPE_ARROW_SRC: [f32; 4] = [262.0, 192.0, 16.0, 10.0];

/// Chapter `i`'s bookmark in the texture.
pub fn tab_src(i: usize, open: bool) -> [f32; 4] {
    let rows = (i / TABS_A_ROW) as f32 * 2.0 * TAB_SIZE[1];
    let y = 192.0 + rows + if open { TAB_SIZE[1] } else { 0.0 };
    [(i % TABS_A_ROW) as f32 * TAB_SIZE[0], y, TAB_SIZE[0], TAB_SIZE[1]]
}

/// An arrow in the texture: back or on, lit while the mouse is over it.
pub fn arrow_src(next: bool, lit: bool) -> [f32; 4] {
    let x = if next { 226.0 } else { 208.0 };
    [x, if lit { 202.0 } else { 192.0 }, PREV_AT[2], PREV_AT[3]]
}

pub const GUIDE_TEXT: u32 = 0x3A_2A1A;
pub const GUIDE_HEADING: u32 = 0x7A_1E12;
pub const GUIDE_FOOTER: u32 = 0xE6_D3A0;
/// The footer's button under the mouse: the lit arrows' gold.
pub const GUIDE_FOOTER_LIT: u32 = 0xFF_E89C;
/// The dots between a contents title and its number.
pub const GUIDE_LEADER: u32 = 0x9A_8A6A;
/// The contents row under the mouse: a wash of the headings' red.
pub const CONTENTS_HOVER: u32 = 0x2A7A_1E12;
/// The footer's leather behind its button, and lit under the mouse.
pub const GUIDE_LEATHER: u32 = 0xFF4A_2C1A;
pub const GUIDE_LEATHER_LIT: u32 = 0xFF70_482A;
const TOOLTIP_TEXT: u32 = 0xFF_FFFF;
const TOOLTIP_BACK: u32 = 0xF010_0010;
const TOOLTIP_EDGE: u32 = 0xFF50_00A0;
/// A shade over the world behind the book.
pub const GUIDE_DIM: u32 = 0x6000_0000;

/// Where a row of `n` icons starts on a page: centred under the heading.
pub fn icons_x(n: usize) -> f32 {
    ((PAGE_WIDTH - (n as f32 * ICON_STEP - 2.0)) / 2.0).floor()
}

/// Where the book's top left corner goes in a window of GUI size `window`: the book
/// and its bookmarks centred, on whole pixels.
pub fn guide_origin(window: [f32; 2]) -> [f32; 2] {
    let width = BOOK_SIZE[0] - TAB_OPEN_X;
    [((window[0] - width) / 2.0).floor() - TAB_OPEN_X, ((window[1] - BOOK_SIZE[1]) / 2.0).floor()]
}

/// A line of a page.
#[derive(Clone, Debug, PartialEq)]
pub enum GuideLine {
    /// A chapter's title, centred, with a blank line under it.
    Heading(String),
    Text(String),
    /// Item icons in a row.
    Icons(&'static [&'static str]),
    /// One of `ASSEMBLIES`: the result's name over a 3×3 grid, an arrow and the result.
    Recipe(usize),
}

impl GuideLine {
    /// How many of a page's rows the line takes.
    pub fn rows(&self) -> usize {
        match self {
            GuideLine::Heading(_) | GuideLine::Icons(_) => 2,
            GuideLine::Text(_) => 1,
            GuideLine::Recipe(_) => 7,
        }
    }

    fn blank(&self) -> bool {
        matches!(self, GuideLine::Text(t) if t.is_empty())
    }
}

/// `text` in lines no wider than `width`, as `measure` reckons widths: its own line
/// breaks kept (a blank line stays blank), words moved down whole, and a word wider
/// than `width` cut where it overflows.
pub fn wrap(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ').filter(|w| !w.is_empty()) {
            let joined = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if measure(&joined) <= width {
                line = joined;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let mut rest = word;
            while measure(rest) > width {
                let mut ends = rest.char_indices().map(|(i, _)| i).skip(1).chain([rest.len()]);
                let first = ends.next().unwrap_or(rest.len());
                let cut = ends.take_while(|&end| measure(&rest[..end]) <= width).last().unwrap_or(first);
                lines.push(rest[..cut].to_owned());
                rest = &rest[cut..];
            }
            line = rest.to_owned();
        }
        lines.push(line);
    }
    lines
}

/// `text` as it fits in `width`: whole, or cut between letters and ended with "…".
pub fn fit(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> String {
    if measure(text) <= width {
        return text.to_owned();
    }
    let mut shown = "…".to_owned();
    for (end, _) in text.char_indices().skip(1) {
        let cut = format!("{}…", text[..end].trim_end());
        if measure(&cut) > width {
            break;
        }
        shown = cut;
    }
    shown
}

/// The lines cut into pages of `lines_per_page` rows. A picture is never cut: it
/// starts the next page when it doesn't fit, and a blank line never starts a page.
/// There is always one page, if an empty one.
pub fn paginate(text_lines: &[GuideLine], lines_per_page: usize) -> Vec<Vec<GuideLine>> {
    let mut pages = vec![Vec::new()];
    let mut used = 0;
    for line in text_lines {
        if used > 0 && used + line.rows() > lines_per_page {
            pages.push(Vec::new());
            used = 0;
        }
        if used == 0 && line.blank() {
            continue;
        }
        pages.last_mut().expect("one page at least").push(line.clone());
        used += line.rows();
    }
    pages
}

/// A chapter's lines, its texts read with `tr`.
pub fn chapter_lines(chapter: &Chapter, tr: &dyn Fn(&str) -> String, measure: &dyn Fn(&str) -> f32) -> Vec<GuideLine> {
    let mut lines = vec![GuideLine::Heading(tr(chapter.title))];
    lines.extend(chapter.icons.chunks(ICONS_A_ROW).map(GuideLine::Icons));
    lines.push(GuideLine::Text(String::new()));
    lines.extend(wrap(&tr(chapter.text), PAGE_WIDTH, measure).into_iter().map(GuideLine::Text));
    if chapter.recipes {
        for i in 0..ASSEMBLIES.len() {
            lines.push(GuideLine::Text(String::new()));
            lines.push(GuideLine::Recipe(i));
        }
    }
    lines
}

/// A recipe's parts laid in its grid, row by row in the order it names them: the
/// assembly table takes them anywhere.
pub fn recipe_cells(recipe: &Assembly) -> [Option<&'static str>; GRID_SLOTS] {
    let mut cells = [None; GRID_SLOTS];
    let parts = recipe.parts.iter().flat_map(|&(id, n)| std::iter::repeat_n(id, n as usize));
    for (cell, id) in cells.iter_mut().zip(parts) {
        *cell = Some(id);
    }
    cells
}

/// What a click on the book does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideClick {
    /// A bookmark, or a row of the contents.
    Chapter(usize),
    Prev,
    Next,
    /// The footer's button back to the contents.
    Contents,
}

fn within([x, y]: [f32; 2], [rx, ry, rw, rh]: [f32; 4]) -> bool {
    x >= rx && x < rx + rw && y >= ry && y < ry + rh
}

/// Chapter `i`'s bookmark as the mouse finds it: its whole length out of the cover.
fn tab_at(i: usize) -> [f32; 4] {
    [TAB_OPEN_X, TAB_TOP + i as f32 * TAB_STEP, -TAB_OPEN_X, TAB_SIZE[1]]
}

/// What is at `at`, from the book's top left corner, on every spread: a bookmark or an arrow.
pub fn guide_hit(at: [f32; 2]) -> Option<GuideClick> {
    if within(at, PREV_AT) {
        return Some(GuideClick::Prev);
    }
    if within(at, NEXT_AT) {
        return Some(GuideClick::Next);
    }
    (0..CHAPTERS.len()).find(|&i| within(at, tab_at(i))).map(GuideClick::Chapter)
}

/// Something to draw, in GUI pixels of the window.
#[derive(Clone, Debug, PartialEq)]
pub enum GuideDraw {
    /// Part `src` of the guide's texture, stretched over `dst`.
    Sprite([f32; 4], [f32; 4]),
    Fill([f32; 4], u32),
    Text(String, [f32; 2], u32),
    /// An item's icon, 16 pixels square.
    Item(&'static str, [f32; 2]),
}

/// The open guide: its pages in the window's language, and the spread shown.
#[derive(Clone, Debug, Default)]
pub struct Guide {
    pub titles: Vec<String>,
    /// The footer button's label, cut to its room, and its width.
    pub contents: String,
    contents_width: f32,
    /// The contents' two pages (the heading only: the rows are drawn over them), then
    /// the chapters'.
    pub pages: Vec<Vec<GuideLine>>,
    /// Each chapter's first page, always a left one.
    pub starts: Vec<usize>,
    /// The spread shown: pages `2 * spread` and `2 * spread + 1`.
    pub spread: usize,
    /// The window's GUI size at the last frame, where the clicks are found.
    pub window: [f32; 2],
    /// The wheel's turn not yet a whole notch, up positive.
    pub scrolled: f64,
}

impl Guide {
    /// A book open at its contents.
    pub fn new(tr: &dyn Fn(&str) -> String, measure: &dyn Fn(&str) -> f32) -> Guide {
        let label = tr(CONTENTS);
        let contents = fit(&label, CONTENTS_BUTTON_MAX, measure);
        let mut guide = Guide {
            contents_width: measure(&contents),
            contents,
            pages: vec![vec![GuideLine::Heading(fit(&label, PAGE_WIDTH, measure))], Vec::new()],
            ..Guide::default()
        };
        for chapter in &CHAPTERS {
            guide.starts.push(guide.pages.len());
            guide.titles.push(tr(chapter.title));
            let mut pages = paginate(&chapter_lines(chapter, tr, measure), PAGE_ROWS);
            if pages.len() % 2 == 1 {
                pages.push(Vec::new());
            }
            guide.pages.extend(pages);
        }
        guide
    }

    pub fn spreads(&self) -> usize {
        self.pages.len().div_ceil(2).max(1)
    }

    /// Shows `spread`, or the last one past the end.
    pub fn go(&mut self, spread: usize) {
        self.spread = spread.min(self.spreads() - 1);
    }

    pub fn next(&mut self) {
        self.go(self.spread + 1);
    }

    pub fn prev(&mut self) {
        self.go(self.spread.saturating_sub(1));
    }

    /// Opens chapter `i` at its first page; there is no such chapter, nothing changes.
    pub fn open(&mut self, i: usize) {
        if let Some(start) = self.starts.get(i) {
            self.go(start / 2);
        }
    }

    /// Back to the contents.
    pub fn home(&mut self) {
        self.go(CONTENTS_SPREAD);
    }

    /// Opened again: at the spread the book before was left at, in its window.
    pub fn keep(&mut self, old: &Guide) {
        self.window = old.window;
        self.go(old.spread);
    }

    /// The chapter of the spread shown; none on the contents.
    pub fn chapter(&self) -> Option<usize> {
        self.starts.iter().rposition(|&s| s / 2 <= self.spread)
    }

    /// The footer's button back to the contents, from the book's top left corner.
    pub fn contents_button(&self) -> [f32; 4] {
        contents_button(self.contents_width)
    }

    /// What is at `at`, from the book's top left corner, on the spread shown: the
    /// contents' rows there, the button back to them elsewhere.
    fn hit(&self, at: [f32; 2]) -> Option<GuideClick> {
        if let Some(found) = guide_hit(at) {
            return Some(found);
        }
        if self.spread == CONTENTS_SPREAD {
            (0..self.starts.len()).find(|&i| within(at, contents_row(i))).map(GuideClick::Chapter)
        } else {
            within(at, self.contents_button()).then_some(GuideClick::Contents)
        }
    }

    /// A left click at `mouse` in the window: whether it did something.
    pub fn click(&mut self, mouse: [f32; 2]) -> bool {
        let o = guide_origin(self.window);
        match self.hit([mouse[0] - o[0], mouse[1] - o[1]]) {
            Some(GuideClick::Chapter(i)) => self.open(i),
            Some(GuideClick::Prev) => self.prev(),
            Some(GuideClick::Next) => self.next(),
            Some(GuideClick::Contents) => self.home(),
            None => return false,
        }
        true
    }

    /// The wheel: up turns back, down turns on.
    /// A touchpad sends fractions of a notch, which add up: a spread turns per whole notch.
    pub fn wheel(&mut self, y: f64) {
        if y == 0.0 {
            return;
        }
        if self.scrolled * y < 0.0 {
            self.scrolled = 0.0;
        }
        self.scrolled += y;
        while self.scrolled >= 1.0 - 1e-9 {
            self.scrolled -= 1.0;
            self.prev();
        }
        while self.scrolled <= -1.0 + 1e-9 {
            self.scrolled += 1.0;
            self.next();
        }
    }

    /// A key, by the game's name: whether it is the book's (it turns a page, or Home goes to the contents).
    /// Escape is left to the game, which closes the screen.
    pub fn key(&mut self, key: &str) -> bool {
        match key {
            "key.keyboard.left" | "key.keyboard.page.up" => self.prev(),
            "key.keyboard.right" | "key.keyboard.page.down" => self.next(),
            "key.keyboard.home" => self.home(),
            _ => return false,
        }
        true
    }

    /// The screen with the mouse at `mouse`: `measure` is the font's width of a text,
    /// `name` an item's name.
    pub fn draw(&self, mouse: [f32; 2], measure: &dyn Fn(&str) -> f32, name: &dyn Fn(&str) -> String) -> Vec<GuideDraw> {
        let [ox, oy] = guide_origin(self.window);
        let rel = [mouse[0] - ox, mouse[1] - oy];
        let mut out = Vec::new();
        let mut tip: Option<String> = None;
        let open = self.chapter();
        for (i, chapter) in CHAPTERS.iter().enumerate() {
            let hovered = within(rel, tab_at(i));
            if hovered {
                tip = self.titles.get(i).cloned();
            }
            let open = open == Some(i);
            let x = if open {
                TAB_OPEN_X
            } else if hovered {
                TAB_X - 2.0
            } else {
                TAB_X
            };
            let [x, y] = [ox + x, oy + TAB_TOP + i as f32 * TAB_STEP];
            out.push(GuideDraw::Sprite(tab_src(i, open), [x, y, TAB_SIZE[0], TAB_SIZE[1]]));
            out.push(GuideDraw::Item(chapter.tab, [x + 4.0, y + 1.0]));
        }
        out.push(GuideDraw::Sprite([0.0, 0.0, BOOK_SIZE[0], BOOK_SIZE[1]], [ox, oy, BOOK_SIZE[0], BOOK_SIZE[1]]));
        if self.spread == CONTENTS_SPREAD {
            self.draw_contents(&mut out, rel, &mut tip, measure);
        }
        let mut item = |out: &mut Vec<GuideDraw>, id: &'static str, at: [f32; 2]| {
            out.push(GuideDraw::Item(id, at));
            if within(mouse, [at[0], at[1], 16.0, 16.0]) {
                tip = Some(name(id));
            }
        };
        for side in 0..2 {
            let x = ox + PAGE_X[side];
            let mut y = oy + PAGE_TOP;
            for line in self.pages.get(2 * self.spread + side).into_iter().flatten() {
                match line {
                    GuideLine::Heading(t) => {
                        let at = [x + ((PAGE_WIDTH - measure(t)) / 2.0).floor(), y];
                        out.push(GuideDraw::Text(t.clone(), at, GUIDE_HEADING));
                    }
                    GuideLine::Text(t) => out.push(GuideDraw::Text(t.clone(), [x, y], GUIDE_TEXT)),
                    GuideLine::Icons(ids) => {
                        let left = x + icons_x(ids.len());
                        for (n, id) in ids.iter().enumerate() {
                            item(&mut out, id, [left + n as f32 * ICON_STEP, y]);
                        }
                    }
                    GuideLine::Recipe(r) => {
                        let recipe = &ASSEMBLIES[*r];
                        let caption = name(recipe.result);
                        let at = [x + ((PAGE_WIDTH - measure(&caption)) / 2.0).floor(), y];
                        out.push(GuideDraw::Text(caption, at, GUIDE_HEADING));
                        let (x, top) = (x + RECIPE_X, y + ROW);
                        for (cell, id) in recipe_cells(recipe).iter().enumerate() {
                            let at = [x + (cell % 3) as f32 * 18.0, top + (cell / 3) as f32 * 18.0];
                            out.push(GuideDraw::Sprite(SLOT_SRC, [at[0], at[1], 18.0, 18.0]));
                            if let Some(id) = id {
                                item(&mut out, id, [at[0] + 1.0, at[1] + 1.0]);
                            }
                        }
                        let [_, _, aw, ah] = RECIPE_ARROW_SRC;
                        out.push(GuideDraw::Sprite(RECIPE_ARROW_SRC, [x + RECIPE_ARROW_X, top + 22.0, aw, ah]));
                        let at = [x + RECIPE_RESULT_X, top + 18.0];
                        out.push(GuideDraw::Sprite(SLOT_SRC, [at[0], at[1], 18.0, 18.0]));
                        item(&mut out, recipe.result, [at[0] + 1.0, at[1] + 1.0]);
                        if recipe.count > 1 {
                            let count = recipe.count.to_string();
                            out.push(GuideDraw::Text(count.clone(), [at[0] + 17.0 - measure(&count), at[1] + 9.0], GUIDE_HEADING));
                        }
                    }
                }
                y += line.rows() as f32 * ROW;
            }
        }
        for (next, shown, [x, y, w, h]) in [(false, self.spread > 0, PREV_AT), (true, self.spread + 1 < self.spreads(), NEXT_AT)] {
            if shown {
                let lit = within(rel, [x, y, w, h]);
                out.push(GuideDraw::Sprite(arrow_src(next, lit), [ox + x, oy + y, w, h]));
            }
        }
        if self.spread != CONTENTS_SPREAD {
            let [x, y, w, h] = self.contents_button();
            let lit = within(rel, [x, y, w, h]);
            out.push(GuideDraw::Fill([ox + x, oy + y, w, h], if lit { GUIDE_LEATHER_LIT } else { GUIDE_LEATHER }));
            let colour = if lit { GUIDE_FOOTER_LIT } else { GUIDE_FOOTER };
            out.push(GuideDraw::Text(self.contents.clone(), [ox + CONTENTS_BUTTON_X, oy + NUMBER_Y], colour));
        }
        let number = format!("{} / {}", self.spread + 1, self.spreads());
        let at = [ox + ((BOOK_SIZE[0] - measure(&number)) / 2.0).floor(), oy + NUMBER_Y];
        out.push(GuideDraw::Text(number, at, GUIDE_FOOTER));
        if let Some(t) = tip {
            out.extend(tooltip(t, mouse, self.window, measure));
        }
        out
    }

    /// The contents' rows, the one under the mouse (`rel`, from the book's corner) lit;
    /// a title cut to its row says itself whole in a tooltip.
    fn draw_contents(&self, out: &mut Vec<GuideDraw>, rel: [f32; 2], tip: &mut Option<String>, measure: &dyn Fn(&str) -> f32) {
        let [ox, oy] = guide_origin(self.window);
        let dots = measure(" .");
        for (i, ((chapter, start), title)) in CHAPTERS.iter().zip(&self.starts).zip(&self.titles).enumerate() {
            let [x, y, w, h] = contents_row(i);
            let hovered = within(rel, [x, y, w, h]);
            let [x, y] = [ox + x, oy + y];
            if hovered {
                out.push(GuideDraw::Fill([x, y, w, h], CONTENTS_HOVER));
            }
            out.push(GuideDraw::Item(chapter.tab, [x + CONTENTS_ICON[0], y + CONTENTS_ICON[1]]));
            let number = (start / 2 + 1).to_string();
            let number_x = x + w - 1.0 - measure(&number);
            let [title_x, text_y] = [x + CONTENTS_TITLE[0], y + CONTENTS_TITLE[1]];
            let shown = fit(title, number_x - CONTENTS_GAP - title_x, measure);
            if hovered && shown != *title {
                *tip = Some(title.clone());
            }
            let title_end = title_x + measure(&shown);
            out.push(GuideDraw::Text(shown, [title_x, text_y], if hovered { GUIDE_HEADING } else { GUIDE_TEXT }));
            // Dots up to the number, ending clear of it; none when the font has no width to give them.
            if dots >= 1.0 {
                let n = ((number_x - 2.0 - title_end - 2.0) / dots).floor().clamp(0.0, 64.0) as usize;
                if n > 0 {
                    out.push(GuideDraw::Text(" .".repeat(n), [number_x - 2.0 - n as f32 * dots, text_y], GUIDE_LEADER));
                }
            }
            out.push(GuideDraw::Text(number, [number_x, text_y], GUIDE_TEXT));
        }
    }
}

/// A tooltip at the mouse, as the game draws an item's: right of the pointer, or left
/// of it where the window ends.
fn tooltip(text: String, mouse: [f32; 2], window: [f32; 2], measure: &dyn Fn(&str) -> f32) -> Vec<GuideDraw> {
    let w = measure(&text);
    let mut x = mouse[0] + 12.0;
    if x + w + 4.0 > window[0] {
        x = (mouse[0] - 16.0 - w).max(4.0);
    }
    let y = (mouse[1] - 12.0).max(4.0);
    vec![
        GuideDraw::Fill([x - 4.0, y - 4.0, w + 8.0, 16.0], TOOLTIP_BACK),
        GuideDraw::Fill([x - 3.0, y - 3.0, w + 6.0, 1.0], TOOLTIP_EDGE),
        GuideDraw::Fill([x - 3.0, y + 10.0, w + 6.0, 1.0], TOOLTIP_EDGE),
        GuideDraw::Fill([x - 3.0, y - 3.0, 1.0, 14.0], TOOLTIP_EDGE),
        GuideDraw::Fill([x + w + 2.0, y - 3.0, 1.0, 14.0], TOOLTIP_EDGE),
        GuideDraw::Text(text, [x, y], TOOLTIP_TEXT),
    ]
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
            "mixins/big_nukes_native/silo.rs",
            "mixins/big_nukes_native/decon.rs",
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
                    || key.starts_with("itemGroup.big_nukes.")
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
        assert_eq!(crater_floor(&b, 0, 0, 0.0), 70 - 62);
        assert!(crater_floor(&b, 50, 0, 0.2) > crater_floor(&b, 0, 0, 0.0));
        assert_eq!(crater_floor(&b, 87, 0, 0.35), 70);
        let deep = blast([0, -50, 0], 250, 1, World::OVERWORLD);
        assert_eq!(crater_floor(&deep, 0, 0, 0.0), -64 + 5);
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
        let silo = [MISSILE_LAUNCH, BOMBER_FLYBY].map(|e| e.strip_prefix("big_nukes:").unwrap());
        for event in ["blast.near", "blast.far", "wave.roar", click].into_iter().chain(silo) {
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

    // ---- the guide (plan 6) ---------------------------------------------------------------

    /// The smallest GUI the game lays out: a window at its largest GUI scale.
    const MIN_GUI: [f32; 2] = [320.0, 240.0];

    /// Six pixels a character, as a stand-in for the game's font.
    fn six(text: &str) -> f32 {
        6.0 * text.chars().count() as f32
    }

    #[test]
    fn the_guide_is_a_book_of_the_mods_own() {
        assert_eq!(stack_of(GUIDE), 1);
        assert!(!ITEMS.iter().find(|i| i.0 == GUIDE).unwrap().2, "the guide isn't rare");
        assert_eq!(super::template(GUIDE), "minecraft:book");
        // A book can be enchanted: the guide can't.
        assert_eq!(components(GUIDE)["!minecraft:enchantable"], serde_json::json!({}));
        assert_eq!(guide_give("Steve"), "give Steve big_nukes:guide");
        assert_eq!(guide_give("@s"), "give @s big_nukes:guide");
        let recipe: serde_json::Value = serde_json::from_str(&file("content/data/big_nukes/recipe/guide.json")).unwrap();
        assert_eq!(recipe["type"], "minecraft:crafting_shapeless");
        assert_eq!(recipe["ingredients"], serde_json::json!(["minecraft:book", RAW_URANIUM]));
        assert_eq!(recipe["result"], serde_json::json!({"id": GUIDE, "count": 1}));
    }

    #[test]
    fn the_guide_has_nine_chapters_in_both_languages() {
        let (en, ru) = (language("en_us"), language("ru_ru"));
        let titles: Vec<&str> = CHAPTERS.iter().map(|c| en.get(c.title).map_or(c.title, String::as_str)).collect();
        assert_eq!(
            titles,
            ["Uranium", "Centrifuge", "Assembly", "Bombs", "Detonators", "Remote", "Missile & Bomber", "Radiation", "Commands"]
        );
        let known: BTreeSet<&str> = ITEMS
            .iter()
            .map(|i| i.0)
            .chain(BombKind::ALL.map(BombKind::id))
            .chain([CENTRIFUGE, ASSEMBLY_TABLE, DECONTAMINATOR])
            .chain(SUNBREAKER_PARTS)
            .chain(SILO_BLOCKS)
            .chain(FILE_BLOCKS)
            .collect();
        for c in &CHAPTERS {
            for key in [c.title, c.text] {
                assert!(en.contains_key(key) && ru.contains_key(key), "{key}");
            }
            // A bookmark's title is a tooltip, one short line.
            for lang in [&en, &ru] {
                assert!(lang[c.title].chars().count() <= 16 && !lang[c.title].contains('\n'), "{}", c.title);
            }
            for id in c.icons.iter().chain([&c.tab]) {
                assert!(id.starts_with("minecraft:") || known.contains(id), "{}: {id}", c.title);
            }
            assert!(!c.icons.is_empty(), "{} has no picture", c.title);
            assert!(c.icons.len() <= 2 * ICONS_A_ROW, "{}: more than two rows of icons", c.title);
        }
        let with_recipes: Vec<&str> = CHAPTERS.iter().filter(|c| c.recipes).map(|c| c.title).collect();
        assert_eq!(with_recipes, ["big_nukes.guide.assembly"]);
        // The missile and the beacon are made at the assembly table: their recipes show in its chapter.
        assert!([MISSILE, STRIKE_BEACON].iter().all(|id| ASSEMBLIES.iter().any(|a| a.result == *id)));
        // The carriers' chapter pictures the silo, its console and the beacon, and tells the code's numbers.
        let carriers = CHAPTERS.iter().find(|c| c.title == "big_nukes.guide.carriers").expect("the carriers' chapter");
        assert_eq!(carriers.tab, MISSILE);
        assert_eq!(carriers.icons, [MISSILE, LAUNCH_PAD, SILO_HATCH, LAUNCH_CONSOLE, STRIKE_BEACON]);
        let numbers = [
            SHAFT_ROOM.to_string(),
            CONSOLE_REACH.to_string(),
            CODE_MIN_DIGITS.to_string(),
            CODE_DIGITS.to_string(),
            (LOCK_TICKS / 20).to_string(),
            MISSILE_MIN_RANGE.to_string(),
            MISSILE_MAX_RANGE.to_string(),
            (LAUNCH_COUNTDOWN / 20).to_string(),
            (STRIKE_COUNTDOWN / 20).to_string(),
            (PLANE_HEIGHT as u32).to_string(),
        ];
        for lang in [&en, &ru] {
            let text = &lang[carriers.text];
            for n in &numbers {
                assert!(text.split(|c: char| !c.is_ascii_digit()).any(|w| w == n), "{n} in {text}");
            }
            assert!(text.contains("/bignukes maxpower"), "{text}");
        }
        // The written book's numbered pages are gone.
        assert!(!en.contains_key("big_nukes.guide.1"));
    }

    #[test]
    fn the_bombs_chapter_tells_of_the_sunbreaker() {
        let (en, ru) = (language("en_us"), language("ru_ru"));
        let bombs = CHAPTERS.iter().find(|c| c.title == "big_nukes.guide.bombs").expect("the bombs' chapter");
        assert_eq!(
            bombs.icons,
            ["big_nukes:demolition_charge", "big_nukes:tactical_warhead", "big_nukes:atomic_bomb", SUNBREAKER_CORE, IMPLOSION_SHELL, NEUTRON_REFLECTOR]
        );
        let numbers = [
            SHELL_BLOCKS.to_string(),
            REFLECTOR_BLOCKS.to_string(),
            SUNBREAKER_BASE.to_string(),
            BEDROCK_RADIUS.to_string(),
            (SUNBREAKER_FUSE / 20).to_string(),
            (ALARM_TICKS / 20).to_string(),
        ];
        for lang in [&en, &ru] {
            let text = &lang[bombs.text];
            for n in &numbers {
                assert!(text.split(|c: char| !c.is_ascii_digit()).any(|w| w == n), "{n} in {text}");
            }
        }
        // Where its uranium comes from: the centrifuge's chapter pictures it and tells its numbers.
        let centrifuge = CHAPTERS.iter().find(|c| c.title == "big_nukes.guide.centrifuge").expect("the centrifuge's chapter");
        assert!(centrifuge.icons.contains(&WEAPONS_GRADE_URANIUM));
        for lang in [&en, &ru] {
            let text = &lang[centrifuge.text];
            for n in [ENRICHMENTS[1].takes.to_string(), (ENRICHMENTS[1].ticks / 20).to_string()] {
                assert!(text.split(|c: char| !c.is_ascii_digit()).any(|w| w == n), "{n} in {text}");
            }
        }
        // The core and its parts are made at the assembly table: their recipes show in its chapter.
        assert!([SUNBREAKER_CORE, IMPLOSION_SHELL, NEUTRON_REFLECTOR].iter().all(|id| ASSEMBLIES.iter().any(|a| a.result == *id)));
        // Four reflectors to a recipe: the chapter shows the count by the result.
        let g = Guide::new(&|k| k.to_owned(), &|t| t.len() as f32 * 6.0);
        let assembly = CHAPTERS.iter().position(|c| c.recipes).unwrap();
        let mut g = g;
        g.open(assembly);
        let mut counts = 0;
        while g.spread < g.starts[assembly + 1] / 2 {
            let drawn = g.draw([0.0, 0.0], &|t| t.len() as f32 * 6.0, &|k| k.to_owned());
            counts += drawn.iter().filter(|d| matches!(d, GuideDraw::Text(t, _, GUIDE_HEADING) if t == "4")).count();
            g.next();
        }
        assert_eq!(counts, 1);
        // The commands' chapter names every limit.
        for lang in [&en, &ru] {
            let steps: Vec<String> = MAXPOWER_STEPS.iter().map(u32::to_string).collect();
            assert!(lang["big_nukes.guide.commands.text"].contains(&steps.join("|")));
        }
    }

    #[test]
    fn icons_go_in_rows_that_fit_the_page() {
        assert!(ICONS_A_ROW as f32 * ICON_STEP <= PAGE_WIDTH && (ICONS_A_ROW + 1) as f32 * ICON_STEP > PAGE_WIDTH);
        for c in &CHAPTERS {
            let lines = chapter_lines(c, &|k: &str| k.to_owned(), &six);
            let rows: Vec<&[&str]> = lines.iter().filter_map(|l| if let GuideLine::Icons(ids) = l { Some(*ids) } else { None }).collect();
            assert_eq!(rows.concat(), c.icons, "{}", c.title);
            assert!(rows.iter().all(|r| !r.is_empty() && r.len() <= ICONS_A_ROW), "{}", c.title);
            // The rows come together, under the heading.
            assert!(matches!(lines[0], GuideLine::Heading(_)) && lines[1..=rows.len()].iter().all(|l| matches!(l, GuideLine::Icons(_))));
        }
    }

    #[test]
    fn text_wraps_to_the_page_and_keeps_its_paragraphs() {
        assert_eq!(wrap("aaa bbb ccc", 42.0, &six), ["aaa bbb", "ccc"]);
        assert_eq!(wrap("one\n\ntwo", 42.0, &six), ["one", "", "two"]);
        assert_eq!(wrap("  spaced   out  ", 60.0, &six), ["spaced out"]);
        // A word wider than the page is cut where it overflows.
        assert_eq!(wrap("abcdefghij k", 24.0, &six), ["abcd", "efgh", "ij k"]);
        assert_eq!(wrap("", 42.0, &six), [""]);
        for line in wrap(&"word ".repeat(50), PAGE_WIDTH, &six) {
            assert!(six(&line) <= PAGE_WIDTH, "{line}");
        }
    }

    #[test]
    fn pages_fill_up_without_cutting_a_picture() {
        let t = |s: &str| GuideLine::Text(s.to_owned());
        assert_eq!(GuideLine::Heading("x".into()).rows(), 2);
        assert_eq!(t("x").rows(), 1);
        assert_eq!(GuideLine::Icons(&[GUIDE]).rows(), 2);
        assert_eq!(GuideLine::Recipe(0).rows(), 7);
        // The recipe doesn't fit under four lines: it starts the next page.
        let lines = vec![t("a"), t("b"), t("c"), t("d"), GuideLine::Recipe(0)];
        assert_eq!(paginate(&lines, 8), vec![vec![t("a"), t("b"), t("c"), t("d")], vec![GuideLine::Recipe(0)]]);
        // A blank line never starts a page.
        let lines = vec![t("a"), t("b"), t("c"), GuideLine::Recipe(0), t(""), t("e")];
        assert_eq!(paginate(&lines, 10), vec![vec![t("a"), t("b"), t("c"), GuideLine::Recipe(0)], vec![t("e")]]);
        assert_eq!(paginate(&[t("a"), t("b"), t("c")], 2), vec![vec![t("a"), t("b")], vec![t("c")]]);
        assert_eq!(paginate(&[], 5), vec![Vec::<GuideLine>::new()]);
        for page in paginate(&lines, PAGE_ROWS) {
            assert!(page.iter().map(GuideLine::rows).sum::<usize>() <= PAGE_ROWS);
        }
    }

    #[test]
    fn recipe_cells_hold_each_assemblys_parts() {
        for a in &ASSEMBLIES {
            let cells = recipe_cells(a);
            for &(id, need) in a.parts {
                assert_eq!(cells.iter().filter(|c| **c == Some(id)).count() as u32, need, "{}: {id}", a.result);
            }
            let total: u32 = a.parts.iter().map(|p| p.1).sum();
            assert_eq!(cells.iter().flatten().count() as u32, total, "{}", a.result);
        }
    }

    /// A book whose second chapter's text runs over several pages.
    fn long_guide() -> Guide {
        let tr = |key: &str| if key == CHAPTERS[1].text { "many words here ".repeat(60) } else { key.to_owned() };
        Guide::new(&tr, &six)
    }

    #[test]
    fn a_chapter_opens_on_a_left_page_and_the_pages_turn_within_the_book() {
        let mut g = long_guide();
        assert_eq!(g.starts.len(), CHAPTERS.len());
        assert_eq!(g.pages.len() % 2, 0);
        assert!(g.starts.iter().all(|s| s % 2 == 0), "{:?}", g.starts);
        assert!(g.starts.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(g.starts[0], 2, "the contents come first");
        assert!(g.starts[2] - g.starts[1] >= 4, "the long chapter takes two spreads at least");
        assert_eq!(g.spreads(), g.pages.len() / 2);
        assert_eq!((g.spread, g.chapter()), (CONTENTS_SPREAD, None));
        g.prev();
        assert_eq!(g.spread, 0, "nothing before the contents");
        g.next();
        assert_eq!((g.spread, g.chapter()), (1, Some(0)));
        g.next();
        assert_eq!((g.spread, g.chapter()), (2, Some(1)));
        g.next();
        assert_eq!(g.chapter(), Some(1), "the long chapter's second spread");
        g.open(6);
        assert_eq!((g.spread, g.chapter()), (g.starts[6] / 2, Some(6)));
        g.go(usize::MAX);
        assert_eq!(g.spread, g.spreads() - 1);
        g.next();
        assert_eq!(g.spread, g.spreads() - 1, "nothing past the last page");
        assert_eq!(g.chapter(), Some(CHAPTERS.len() - 1));
        g.open(99);
        assert_eq!(g.chapter(), Some(CHAPTERS.len() - 1), "no such chapter: the page stays");
        // The assembly chapter holds every recipe of the table, each once.
        let recipes: Vec<usize> = g.pages[g.starts[2]..g.starts[3]]
            .iter()
            .flatten()
            .filter_map(|l| if let GuideLine::Recipe(i) = l { Some(*i) } else { None })
            .collect();
        assert_eq!(recipes, (0..ASSEMBLIES.len()).collect::<Vec<_>>());
    }

    #[test]
    fn the_wheel_and_the_arrow_keys_turn_the_pages() {
        let mut g = long_guide();
        assert!(g.key("key.keyboard.right"));
        assert_eq!(g.spread, 1);
        assert!(g.key("key.keyboard.page.down"));
        assert_eq!(g.spread, 2);
        assert!(g.key("key.keyboard.left"));
        assert_eq!(g.spread, 1);
        // Escape is the game's: it closes the screen.
        assert!(!g.key("key.keyboard.escape"));
        assert!(!g.key("key.keyboard.e"));
        g.wheel(-1.0);
        assert_eq!(g.spread, 2);
        g.wheel(1.0);
        g.wheel(1.0);
        assert_eq!(g.spread, 0);
        g.wheel(0.0);
        assert_eq!(g.spread, 0);
    }

    #[test]
    fn a_touchpad_swipe_turns_one_spread_per_notch() {
        let mut g = long_guide();
        // A touchpad sends small fractions of a notch: a page turns once they add up to one.
        for _ in 0..9 {
            g.wheel(-0.1);
        }
        assert_eq!(g.spread, 0);
        g.wheel(-0.15);
        assert_eq!(g.spread, 1);
        // Turning the other way starts the count again.
        g.wheel(0.5);
        assert_eq!(g.spread, 1);
        g.wheel(0.5);
        assert_eq!(g.spread, 0);
    }

    #[test]
    fn the_guide_tag_is_not_the_written_books() {
        // The first guide was a written book given under "big_nukes_guide_given"; a new tag gives the new guide
        // to players who had that one.
        assert_ne!(GUIDE_TAG, "big_nukes_guide_given");
    }

    fn middle([x, y, w, h]: [f32; 4]) -> [f32; 2] {
        [x + w / 2.0, y + h / 2.0]
    }

    fn tab(i: usize) -> [f32; 4] {
        [TAB_OPEN_X, TAB_TOP + i as f32 * TAB_STEP, -TAB_OPEN_X, TAB_SIZE[1]]
    }

    #[test]
    fn clicks_find_the_bookmarks_and_the_arrows() {
        for i in 0..CHAPTERS.len() {
            assert_eq!(guide_hit(middle(tab(i))), Some(GuideClick::Chapter(i)), "{i}");
        }
        assert_eq!(guide_hit(middle(PREV_AT)), Some(GuideClick::Prev));
        assert_eq!(guide_hit(middle(NEXT_AT)), Some(GuideClick::Next));
        assert_eq!(guide_hit([PAGE_X[0] + 10.0, PAGE_TOP + 10.0]), None);
        assert_eq!(guide_hit([TAB_OPEN_X - 1.0, TAB_TOP + 4.0]), None);
        assert_eq!(guide_hit([TAB_OPEN_X + 2.0, TAB_TOP + TAB_SIZE[1] + 0.5]), None, "between two bookmarks");
        // A click through the screen: the origin is where the book is drawn.
        let mut g = long_guide();
        g.window = [480.0, 270.0];
        let o = guide_origin(g.window);
        let at = |p: [f32; 2]| [o[0] + p[0], o[1] + p[1]];
        assert!(g.click(at(middle(tab(4)))));
        assert_eq!(g.chapter(), Some(4));
        assert!(g.click(at(middle(NEXT_AT))));
        assert_eq!(g.spread, g.starts[4] / 2 + 1);
        assert!(g.click(at(middle(PREV_AT))));
        assert!(!g.click(at([PAGE_X[1] + 5.0, PAGE_TOP + 5.0])));
        assert_eq!(g.chapter(), Some(4));
        // Every bookmark opens its chapter, the last one too.
        for i in (0..CHAPTERS.len()).rev() {
            assert!(g.click(at(middle(tab(i)))), "{i}");
            assert_eq!((g.chapter(), g.spread), (Some(i), g.starts[i] / 2), "{i}");
        }
        // The first chapter's arrow back reaches the contents.
        assert!(g.click(at(middle(PREV_AT))));
        assert_eq!((g.spread, g.chapter()), (CONTENTS_SPREAD, None));
    }

    #[test]
    fn the_book_fits_the_smallest_gui_and_its_texture() {
        // The book and its bookmarks, sticking out to the left.
        let extent = [BOOK_SIZE[0] - TAB_OPEN_X, BOOK_SIZE[1]];
        assert!(extent[0] <= MIN_GUI[0] && extent[1] <= MIN_GUI[1], "{extent:?}");
        let o = guide_origin(MIN_GUI);
        assert!(o[0] + TAB_OPEN_X >= 0.0 && o[0] + BOOK_SIZE[0] <= MIN_GUI[0] && o[1] >= 0.0, "{o:?}");
        assert_eq!(guide_origin([1000.0, 500.0]), [((1000.0 - extent[0]) / 2.0).floor() - TAB_OPEN_X, 154.0]);
        // The text stays on the parchment, clear of the spine and the footer.
        for (x, page) in PAGE_X.iter().zip(PAGES) {
            assert!(*x >= page[0] + 4.0 && x + PAGE_WIDTH <= page[0] + page[2] - 4.0, "{x}");
        }
        let bottom = PAGE_TOP + PAGE_ROWS as f32 * ROW;
        assert!(bottom <= PAGES[0][1] + PAGES[0][3], "{bottom}");
        assert!(PREV_AT[1] >= PAGES[0][1] + PAGES[0][3] && NEXT_AT[1] + NEXT_AT[3] <= BOOK_SIZE[1]);
        assert!(NEXT_AT[0] + NEXT_AT[2] <= BOOK_SIZE[0] && PREV_AT[0] >= 0.0);
        // A recipe: the grid, the arrow and the result in a page's width, in its rows.
        assert!(RECIPE_X + RECIPE_RESULT_X + 18.0 <= PAGE_WIDTH);
        assert!(3.0 * 18.0 <= RECIPE_ARROW_X && RECIPE_ARROW_X + RECIPE_ARROW_SRC[2] <= RECIPE_RESULT_X);
        // A row of icons, centred: six fit.
        assert_eq!(icons_x(3), 32.0);
        assert!(icons_x(6) >= 0.0);
        assert!(ROW + 3.0 * 18.0 <= GuideLine::Recipe(0).rows() as f32 * ROW);
        // The bookmarks down the book's edge, clear of its corners, as far from the bottom as from the top.
        let bottom = TAB_TOP + (CHAPTERS.len() - 1) as f32 * TAB_STEP + TAB_SIZE[1];
        assert!(TAB_TOP >= 8.0 && bottom <= BOOK_SIZE[1] - 8.0, "{TAB_TOP} {bottom}");
        assert!((BOOK_SIZE[1] - bottom - TAB_TOP).abs() <= 1.0, "{TAB_TOP} {bottom}");
        assert!(TAB_STEP > TAB_SIZE[1]);
        assert!(TAB_X > TAB_OPEN_X && TAB_X + TAB_SIZE[0] > 0.0, "a bookmark tucks under the cover");
        // Every part the screen draws is in the texture.
        let size = png_size("content/assets/big_nukes/textures/gui/guide.png");
        assert_eq!(size, (GUIDE_TEXTURE[0] as u32, GUIDE_TEXTURE[1] as u32));
        let mut parts = vec![[0.0, 0.0, BOOK_SIZE[0], BOOK_SIZE[1]], SLOT_SRC, RECIPE_ARROW_SRC];
        for hover in [false, true] {
            parts.push(arrow_src(false, hover));
            parts.push(arrow_src(true, hover));
        }
        for i in 0..CHAPTERS.len() {
            parts.push(tab_src(i, false));
            parts.push(tab_src(i, true));
        }
        for [x, y, w, h] in parts.iter().copied() {
            assert!(x >= 0.0 && y >= 0.0 && x + w <= GUIDE_TEXTURE[0] && y + h <= GUIDE_TEXTURE[1], "{x} {y} {w} {h}");
        }
        // No two parts share a pixel: each bookmark has a colour of its own.
        for (n, a) in parts.iter().enumerate() {
            for b in &parts[n + 1..] {
                let apart = a[0] + a[2] <= b[0] || b[0] + b[2] <= a[0] || a[1] + a[3] <= b[1] || b[1] + b[3] <= a[1];
                assert!(apart, "{a:?} {b:?}");
            }
        }
        assert_eq!(GUIDE_GUI, "big_nukes:gui/guide");
    }

    fn texts(draws: &[GuideDraw]) -> Vec<&str> {
        draws.iter().filter_map(|d| if let GuideDraw::Text(t, _, _) = d { Some(t.as_str()) } else { None }).collect()
    }

    #[test]
    fn the_open_book_draws_its_bookmarks_pages_and_number() {
        let mut g = long_guide();
        g.window = [480.0, 270.0];
        let o = guide_origin(g.window);
        let name = |id: &str| format!("name of {id}");
        let away = [0.0, 0.0];
        // The contents: no bookmark sticks out, and no way back from there.
        let draws = g.draw(away, &six, &name);
        let sprites: Vec<([f32; 4], [f32; 4])> =
            draws.iter().filter_map(|d| if let GuideDraw::Sprite(s, d) = d { Some((*s, *d)) } else { None }).collect();
        for i in 0..CHAPTERS.len() {
            let at = sprites.iter().position(|s| s.0 == tab_src(i, false)).unwrap_or_else(|| panic!("bookmark {i}"));
            assert_eq!(sprites[at].1[..2], [o[0] + TAB_X, o[1] + TAB_TOP + i as f32 * TAB_STEP], "{i}");
        }
        assert!(!sprites.iter().any(|s| s.0 == arrow_src(false, false) || s.0 == arrow_src(false, true)));
        assert!(sprites.iter().any(|s| s.0 == arrow_src(true, false)));
        assert!(texts(&draws).contains(&format!("1 / {}", g.spreads()).as_str()), "{:?}", texts(&draws));
        g.open(0);
        let draws = g.draw(away, &six, &name);
        let sprites: Vec<([f32; 4], [f32; 4])> =
            draws.iter().filter_map(|d| if let GuideDraw::Sprite(s, d) = d { Some((*s, *d)) } else { None }).collect();
        assert!(sprites.contains(&([0.0, 0.0, BOOK_SIZE[0], BOOK_SIZE[1]], [o[0], o[1], BOOK_SIZE[0], BOOK_SIZE[1]])));
        // The open chapter's bookmark is lit and sticks out further; the bookmarks come before the book.
        let book = sprites.iter().position(|s| s.0[2] == BOOK_SIZE[0]).unwrap();
        for i in 0..CHAPTERS.len() {
            let open = i == 0;
            let x = o[0] + if open { TAB_OPEN_X } else { TAB_X };
            let at = sprites.iter().position(|s| s.0 == tab_src(i, open)).unwrap_or_else(|| panic!("bookmark {i}"));
            assert_eq!(sprites[at].1[..2], [x, o[1] + TAB_TOP + i as f32 * TAB_STEP], "{i}");
            assert!(at < book, "{i}");
        }
        assert!(texts(&draws).contains(&format!("2 / {}", g.spreads()).as_str()), "{:?}", texts(&draws));
        assert!(texts(&draws).contains(&CHAPTERS[0].title), "the heading");
        // The first chapter's way back leads to the contents.
        assert!(sprites.iter().any(|s| s.0 == arrow_src(false, false)));
        assert!(sprites.iter().any(|s| s.0 == arrow_src(true, false)));
        let items: Vec<&str> = draws.iter().filter_map(|d| if let GuideDraw::Item(id, _) = d { Some(*id) } else { None }).collect();
        for id in CHAPTERS.iter().map(|c| c.tab).chain(CHAPTERS[0].icons.iter().copied()) {
            assert!(items.contains(&id), "{id}");
        }
        // Over a bookmark: its title; over an icon: the item's name.
        let over = |at: [f32; 2]| texts(&g.draw(at, &six, &name)).last().map(|t| t.to_string());
        let tab2 = middle(tab(2));
        assert_eq!(over([o[0] + tab2[0], o[1] + tab2[1]]), Some(CHAPTERS[2].title.to_owned()));
        let icon = [o[0] + PAGE_X[0] + icons_x(CHAPTERS[0].icons.len()) + 8.0, o[1] + PAGE_TOP + 2.0 * ROW + 8.0];
        assert_eq!(over(icon), Some(format!("name of {}", CHAPTERS[0].icons[0])));
        // The last page: back, but no further.
        g.go(usize::MAX);
        let draws = g.draw(away, &six, &name);
        assert!(texts(&draws).contains(&format!("{0} / {0}", g.spreads()).as_str()));
        let sprites: Vec<[f32; 4]> = draws.iter().filter_map(|d| if let GuideDraw::Sprite(s, _) = d { Some(*s) } else { None }).collect();
        assert!(sprites.contains(&arrow_src(false, false)) && !sprites.contains(&arrow_src(true, false)));
        // The assembly chapter draws its grids: slots, the parts' icons and the result.
        let mut all = Vec::new();
        for s in g.starts[2] / 2..g.starts[3] / 2 {
            g.go(s);
            all.extend(g.draw(away, &six, &name));
        }
        let items: Vec<&str> = all.iter().filter_map(|d| if let GuideDraw::Item(id, _) = d { Some(*id) } else { None }).collect();
        for a in &ASSEMBLIES {
            assert!(items.contains(&a.result), "{}", a.result);
            for p in a.parts {
                assert!(items.contains(&p.0), "{}", p.0);
            }
        }
        assert!(all.iter().filter(|d| matches!(d, GuideDraw::Sprite(s, _) if *s == SLOT_SRC)).count() >= ASSEMBLIES.len() * 10);
    }

    /// The game's font, near enough: a space is narrower than a letter.
    fn mc(text: &str) -> f32 {
        text.chars().map(|c| if c == ' ' { 4.0 } else { 6.0 }).sum()
    }

    /// The book in a language, as `mc` measures it, in a window of the smallest GUI.
    fn book(code: &str) -> Guide {
        let lang = language(code);
        let tr = move |key: &str| lang.get(key).cloned().unwrap_or_else(|| key.to_owned());
        let mut g = Guide::new(&tr, &mc);
        g.window = MIN_GUI;
        g
    }

    fn moved([x, y, w, h]: [f32; 4], [ox, oy]: [f32; 2]) -> [f32; 4] {
        [x + ox, y + oy, w, h]
    }

    /// The texts drawn inside `rect` of the window, left to right.
    fn texts_in(draws: &[GuideDraw], rect: [f32; 4]) -> Vec<(String, [f32; 2], u32)> {
        let mut found: Vec<(String, [f32; 2], u32)> = draws
            .iter()
            .filter_map(|d| if let GuideDraw::Text(t, at, c) = d { Some((t.clone(), *at, *c)) } else { None })
            .filter(|(_, at, _)| within(*at, rect))
            .collect();
        found.sort_by(|a, b| a.1[0].total_cmp(&b.1[0]));
        found
    }

    #[test]
    fn the_book_opens_on_its_contents_with_every_chapter_and_its_spread() {
        for code in ["en_us", "ru_ru"] {
            let lang = language(code);
            let g = book(code);
            assert_eq!((g.spread, g.chapter()), (CONTENTS_SPREAD, None), "{code}");
            assert_eq!(g.pages[0], vec![GuideLine::Heading(lang[CONTENTS].clone())], "{code}: the heading");
            assert!(g.pages[1].is_empty());
            let o = guide_origin(g.window);
            let draws = g.draw([0.0, 0.0], &mc, &|id: &str| id.to_owned());
            let mut right_ends = BTreeSet::new();
            for (i, c) in CHAPTERS.iter().enumerate() {
                let row = moved(contents_row(i), o);
                // Its bookmark's item, its title whole, and the number of the spread it starts on, at the right.
                assert!(draws.iter().any(|d| matches!(d, GuideDraw::Item(id, at) if *id == c.tab && within(*at, row))), "{code} {i}");
                let found = texts_in(&draws, row);
                assert!(found.len() >= 2, "{code} {i}: {found:?}");
                assert_eq!(found[0].0, lang[c.title], "{code} {i}: the title fits");
                let (number, at, _) = found.last().unwrap();
                assert_eq!(*number, (g.starts[i] / 2 + 1).to_string(), "{code} {i}");
                right_ends.insert((at[0] + mc(number)) as i32);
                assert!(at[0] + mc(number) <= row[0] + row[2] && found[0].1[0] + mc(&found[0].0) < at[0], "{code} {i}");
            }
            assert_eq!(right_ends.len(), 2, "{code}: the numbers end at one line a page: {right_ends:?}");
            // The heading over the left page; the footer counts the contents as the first spread.
            assert!(texts(&draws).contains(&lang[CONTENTS].as_str()));
            assert!(texts(&draws).contains(&format!("1 / {}", g.spreads()).as_str()));
        }
    }

    #[test]
    fn the_contents_rows_lie_on_the_pages_under_the_heading() {
        let left = CHAPTERS.len().div_ceil(2);
        for i in 0..CHAPTERS.len() {
            let [x, y, w, h] = contents_row(i);
            let page = PAGES[usize::from(i >= left)];
            assert!(x >= page[0] + 2.0 && x + w <= page[0] + page[2] - 2.0, "{i}");
            assert!(y >= PAGE_TOP + 2.0 * ROW && y + h <= page[1] + page[3] - 4.0, "{i}");
            assert!(h >= 18.0, "{i}: an icon's height");
            if i > 0 && i != left {
                let above = contents_row(i - 1);
                assert!(above[1] + above[3] <= y && above[0] == x, "{i}");
            }
        }
        // The two pages' rows side by side.
        assert_eq!(contents_row(0)[1], contents_row(left)[1]);
        // The button back sits in the footer, right of the arrow back and clear of the page number's gap.
        let b = contents_button(CONTENTS_BUTTON_MAX);
        assert!(b[0] >= PREV_AT[0] + PREV_AT[2] + 2.0 && b[0] + b[2] <= 120.0, "{b:?}");
        assert!(b[1] >= PAGES[0][1] + PAGES[0][3] && b[1] + b[3] <= BOOK_SIZE[1], "{b:?}");
        assert!(guide_hit(middle(b)).is_none(), "nothing else is there");
    }

    #[test]
    fn a_click_on_a_contents_row_opens_its_chapter() {
        let mut g = book("ru_ru");
        let o = guide_origin(g.window);
        for i in 0..CHAPTERS.len() {
            g.home();
            let [x, y] = middle(contents_row(i));
            assert!(g.click([o[0] + x, o[1] + y]), "{i}");
            assert_eq!((g.chapter(), g.spread), (Some(i), g.starts[i] / 2), "{i}");
            // A row's edge counts too.
            g.home();
            let r = contents_row(i);
            assert!(g.click([o[0] + r[0] + 0.5, o[1] + r[1] + 0.5]), "{i}");
            assert_eq!(g.chapter(), Some(i), "{i}");
        }
        // Away from the contents the rows are gone: a click there is on a page.
        g.open(3);
        let [x, y] = middle(contents_row(0));
        assert!(!g.click([o[0] + x, o[1] + y]));
        assert_eq!(g.chapter(), Some(3));
        // On the heading and between two rows, nothing.
        g.home();
        let (r, below) = (contents_row(0), contents_row(1));
        assert!(!g.click([o[0] + r[0] + 10.0, o[1] + PAGE_TOP + 2.0]));
        assert!(below[1] > r[1] + r[3], "a gap between the rows");
        assert!(!g.click([o[0] + r[0] + 10.0, o[1] + r[1] + r[3] + 0.5]));
        assert_eq!(g.spread, CONTENTS_SPREAD);
    }

    #[test]
    fn a_hovered_contents_row_is_lit() {
        let g = book("en_us");
        let o = guide_origin(g.window);
        let name = |id: &str| id.to_owned();
        let lit = |draws: &[GuideDraw], i: usize| draws.contains(&GuideDraw::Fill(moved(contents_row(i), o), CONTENTS_HOVER));
        let draws = g.draw([0.0, 0.0], &mc, &name);
        assert!((0..CHAPTERS.len()).all(|i| !lit(&draws, i)));
        assert!(texts_in(&draws, moved(contents_row(0), o)).iter().all(|t| t.2 != GUIDE_HEADING));
        for i in [0, 4, 5, CHAPTERS.len() - 1] {
            let [x, y] = middle(contents_row(i));
            let draws = g.draw([o[0] + x, o[1] + y], &mc, &name);
            assert!(lit(&draws, i), "{i}");
            assert_eq!((0..CHAPTERS.len()).filter(|&j| lit(&draws, j)).count(), 1, "{i}");
            // The fill goes under the row: before its icon and its title, which turns the headings' colour.
            let row = moved(contents_row(i), o);
            let fill = draws.iter().position(|d| matches!(d, GuideDraw::Fill(_, c) if *c == CONTENTS_HOVER)).unwrap();
            let icon = draws.iter().position(|d| matches!(d, GuideDraw::Item(id, at) if *id == CHAPTERS[i].tab && within(*at, row))).unwrap();
            assert!(fill < icon, "{i}");
            assert_eq!(texts_in(&draws, row)[0].2, GUIDE_HEADING, "{i}");
        }
    }

    #[test]
    fn home_and_the_footer_button_lead_back_to_the_contents() {
        let mut g = book("en_us");
        let o = guide_origin(g.window);
        let lang = language("en_us");
        assert_eq!(g.contents, lang[CONTENTS]);
        assert_eq!(g.contents_button(), contents_button(mc(&lang[CONTENTS])));
        let button = moved(g.contents_button(), o);
        let at = middle(button);
        g.open(4);
        assert!(g.key("key.keyboard.home"));
        assert_eq!((g.spread, g.chapter()), (CONTENTS_SPREAD, None));
        assert!(g.key("key.keyboard.home"), "the key is the book's on the contents too");
        g.open(6);
        g.next();
        // The button is in the footer, in its colour, and lit under the mouse.
        let draws = g.draw([0.0, 0.0], &mc, &|id: &str| id.to_owned());
        let found = texts_in(&draws, button);
        assert_eq!(found.iter().map(|t| (t.0.as_str(), t.2)).collect::<Vec<_>>(), [(lang[CONTENTS].as_str(), GUIDE_FOOTER)]);
        let draws = g.draw(at, &mc, &|id: &str| id.to_owned());
        assert_eq!(texts_in(&draws, button)[0].2, GUIDE_FOOTER_LIT);
        assert!(g.click(at));
        assert_eq!((g.spread, g.chapter()), (CONTENTS_SPREAD, None));
        // On the contents there is no button: the click there does nothing.
        let draws = g.draw(at, &mc, &|id: &str| id.to_owned());
        assert!(texts_in(&draws, button).is_empty(), "{:?}", texts_in(&draws, button));
        assert!(!g.click(at));
        assert_eq!(g.spread, CONTENTS_SPREAD);
        // The arrows and the keys reach the contents as the first spread.
        g.open(0);
        assert!(g.key("key.keyboard.left"));
        assert_eq!(g.spread, CONTENTS_SPREAD);
        assert!(g.key("key.keyboard.right"));
        assert_eq!(g.chapter(), Some(0));
        // A label too long for the footer is cut.
        let tr = |key: &str| if key == CONTENTS { "very long word ".repeat(10) } else { key.to_owned() };
        let long = Guide::new(&tr, &mc);
        assert!(long.contents.ends_with('…') && mc(&long.contents) <= CONTENTS_BUTTON_MAX, "{}", long.contents);
        assert_eq!(long.contents_button(), contents_button(mc(&long.contents)));
        let heading = &long.pages[0][0];
        assert!(matches!(heading, GuideLine::Heading(t) if mc(t) <= PAGE_WIDTH), "{heading:?}");
    }

    #[test]
    fn the_footer_counts_spreads_with_the_contents() {
        let mut g = long_guide();
        g.window = [480.0, 270.0];
        let chapters = (g.pages.len() - g.starts[0]) / 2;
        assert_eq!(g.spreads(), chapters + 1);
        let name = |id: &str| id.to_owned();
        for s in 0..g.spreads() {
            g.go(s);
            assert!(texts(&g.draw([0.0, 0.0], &six, &name)).contains(&format!("{} / {}", s + 1, g.spreads()).as_str()), "{s}");
        }
    }

    #[test]
    fn a_title_too_wide_is_cut_with_an_ellipsis() {
        assert_eq!(fit("short", 60.0, &six), "short");
        assert_eq!(fit("abcdefghij", 30.0, &six), "abcd…");
        assert_eq!(fit("abcdefghij", 60.0, &six), "abcdefghij");
        assert_eq!(fit("абвгдежзик", 30.0, &six), "абвг…", "cut between letters, not bytes");
        assert_eq!(fit("abc", 2.0, &six), "…");
        // A long title in the contents: cut, and its tooltip says it whole.
        let long = "Missile, Bomber and every other way there is".to_owned();
        let tr = |key: &str| if key == CHAPTERS[6].title { long.clone() } else { key.to_owned() };
        let mut g = Guide::new(&tr, &mc);
        g.window = MIN_GUI;
        let o = guide_origin(g.window);
        let row = moved(contents_row(6), o);
        let draws = g.draw([0.0, 0.0], &mc, &|id: &str| id.to_owned());
        let title = &texts_in(&draws, row)[0].0;
        assert!(title.ends_with('…') && long.starts_with(title.trim_end_matches('…')), "{title}");
        let draws = g.draw(middle(row), &mc, &|id: &str| id.to_owned());
        assert_eq!(texts(&draws).last(), Some(&long.as_str()));
    }

    #[test]
    fn the_book_remembers_its_spread_and_a_new_one_opens_on_the_contents() {
        let mut old = long_guide();
        old.window = [480.0, 270.0];
        old.open(5);
        let mut g = long_guide();
        assert_eq!(g.spread, CONTENTS_SPREAD);
        g.keep(&old);
        assert_eq!((g.spread, g.chapter(), g.window), (old.spread, Some(5), old.window));
        // A shorter book than the one before: its last spread.
        let mut short = Guide::new(&|k: &str| k.to_owned(), &six);
        old.go(usize::MAX);
        short.keep(&old);
        assert_eq!(short.spread, short.spreads() - 1);
        old.home();
        short.keep(&old);
        assert_eq!(short.spread, CONTENTS_SPREAD);
    }

    #[test]
    fn nothing_breaks_in_a_tiny_window_or_an_empty_book() {
        let name = |id: &str| id.to_owned();
        let zero = |_: &str| 0.0;
        for mut g in [long_guide(), Guide::default()] {
            for window in [[0.0, 0.0], [1.0, 1.0], [10.0, 7.0], MIN_GUI] {
                g.window = window;
                for s in [0, 1, usize::MAX] {
                    g.go(s);
                    for mouse in [[0.0, 0.0], [-5.0, -5.0], [window[0] / 2.0, window[1] / 2.0], [1e6, 1e6]] {
                        g.draw(mouse, &six, &name);
                        g.draw(mouse, &zero, &name);
                        g.click(mouse);
                    }
                    g.key("key.keyboard.home");
                    g.wheel(-3.0);
                    g.wheel(2.5);
                    g.home();
                    assert!(g.spread < g.spreads());
                }
            }
        }
        // A measure that finds no width doesn't fill the page with dots.
        let g = Guide::new(&|k: &str| k.to_owned(), &zero);
        assert!(g.draw([0.0, 0.0], &zero, &name).len() < 500);
    }

    #[test]
    fn every_block_and_item_says_how_to_use_it() {
        let mut all: Vec<&str> = [CENTRIFUGE, ASSEMBLY_TABLE, DECONTAMINATOR].into_iter().chain(BombKind::ALL.map(BombKind::id)).chain(SUNBREAKER_PARTS).chain(SILO_BLOCKS).chain(FILE_BLOCKS).collect();
        all.extend(ITEMS.iter().map(|i| i.0));
        let (en, ru) = (language("en_us"), language("ru_ru"));
        for id in all.into_iter().filter(|id| *id != "big_nukes:ash") {
            let keys = tips(id);
            assert!(!keys.is_empty(), "{id} has no tip");
            for k in keys {
                assert!(en.contains_key(*k) && ru.contains_key(*k), "{k}");
            }
        }
        assert!(tips("minecraft:stone").is_empty());
    }

    #[test]
    fn every_block_and_item_is_in_the_creative_tab() {
        let tab: serde_json::Value = serde_json::from_str(&file("content/data/big_nukes/horizon/creative_tab/big_nukes.json")).unwrap();
        let listed: Vec<&str> = tab["items"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        let mut all: Vec<&str> = [CENTRIFUGE, ASSEMBLY_TABLE, DECONTAMINATOR].into_iter().chain(BombKind::ALL.map(BombKind::id)).chain(SUNBREAKER_PARTS).chain(SILO_BLOCKS).chain(FILE_BLOCKS).collect();
        all.extend(ITEMS.iter().map(|i| i.0));
        let (listed_set, all_set): (BTreeSet<&str>, BTreeSet<&str>) = (listed.iter().copied().collect(), all.iter().copied().collect());
        assert_eq!(listed.len(), listed_set.len(), "an id is listed twice");
        assert_eq!(listed_set, all_set);
        assert!(all_set.contains(tab["icon"].as_str().unwrap()));
        let title = tab["title"].as_str().unwrap();
        assert!(language("en_us").contains_key(title) && language("ru_ru").contains_key(title), "{title}");
    }

    #[test]
    fn every_block_and_item_is_declared_once_with_its_assets() {
        let code_blocks: Vec<&str> =
            [CENTRIFUGE, ASSEMBLY_TABLE, DECONTAMINATOR].into_iter().chain(BombKind::ALL.map(BombKind::id)).chain(SUNBREAKER_PARTS).chain(SILO_BLOCKS).collect();
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
            .chain([CENTRIFUGE, ASSEMBLY_TABLE, DECONTAMINATOR])
            .chain(SUNBREAKER_PARTS)
            .chain(SILO_BLOCKS)
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
        let not_crafted = ["big_nukes:raw_uranium", "big_nukes:enriched_uranium", WEAPONS_GRADE_URANIUM];
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
        let wanted: BTreeSet<&str> =
            [CHARGE, "big_nukes:bomb_casing", MISSILE, STRIKE_BEACON, DECONTAMINATOR].into_iter().chain(BombKind::ALL.map(BombKind::id)).chain(SUNBREAKER_PARTS).collect();
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
    /// An output slot full of enriched uranium (`most` stacks 16).
    const FULL: Option<(&'static str, u32)> = Some((ENRICHED_URANIUM, 64));

    /// `n` raw uranium in the input, none for 0.
    fn raw(n: u32) -> Option<(&'static str, u32)> {
        (n > 0).then_some((RAW_URANIUM, n))
    }

    #[test]
    fn no_fuel_no_spin() {
        let mut c = Centrifuge::default();
        for _ in 0..20 {
            assert_eq!(c.step(raw(5), None, None, &most), Step::IDLE);
            assert_eq!(c.step(raw(5), Some(0), None, &most), Step::IDLE, "what doesn't burn isn't fuel");
        }
        assert_eq!(c, Centrifuge::default());
        // Fuel but nothing to spin: the fuel stays in its slot.
        assert_eq!(c.step(raw(0), COAL, None, &most), Step::IDLE);
        assert_eq!(c, Centrifuge::default());
    }

    #[test]
    fn fuel_burns_down_and_refills() {
        let mut c = Centrifuge::default();
        assert_eq!(c.step(raw(3), Some(10), None, &most), Step { took_fuel: true, made: false });
        assert_eq!(c, Centrifuge { burn: 9, burn_max: 10, spin: 1, stage: 0 });
        for left in (0..9).rev() {
            assert_eq!(c.step(raw(3), Some(10), None, &most), Step { took_fuel: false, made: false });
            assert_eq!(c.burn, left);
        }
        // Burnt out: the next tick takes one more fuel.
        assert_eq!(c.step(raw(3), Some(7), None, &most), Step { took_fuel: true, made: false });
        assert_eq!(c, Centrifuge { burn: 6, burn_max: 7, spin: 11, stage: 0 });
        // Out of fuel: it stops where it is.
        c.burn = 0;
        assert_eq!(c.step(raw(3), None, None, &most), Step::IDLE);
        assert_eq!(c.spin, 11);
    }

    #[test]
    fn at_spin_max_input_becomes_output() {
        let mut c = Centrifuge::default();
        let mut made = 0;
        for tick in 1..=SPIN_TICKS {
            let step = c.step(raw(1), COAL, None, &most);
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
            assert!(!c.step(raw(1), COAL, None, &most).took_fuel);
        }
        assert_eq!(c.burn, 0);
        // The last tick of the fuel and the first of the next one can coincide with an output.
        let mut c = Centrifuge { burn: 0, burn_max: 1600, spin: SPIN_TICKS - 1, stage: 0 };
        assert_eq!(c.step(raw(1), COAL, None, &most), Step { took_fuel: true, made: true });
        // Taking the input out loses the progress, as a furnace does.
        let mut c = Centrifuge { burn: 50, burn_max: 100, spin: 200, stage: 0 };
        assert_eq!(c.step(raw(0), COAL, None, &most), Step::IDLE);
        assert_eq!(c, Centrifuge { burn: 50, burn_max: 100, spin: 0, stage: 0 });
    }

    #[test]
    fn a_full_output_waits() {
        let mut c = Centrifuge { burn: 30, burn_max: 100, spin: 250, stage: 0 };
        for _ in 0..50 {
            assert_eq!(c.step(raw(4), COAL, FULL, &most), Step::IDLE);
        }
        // Neither the progress nor the burning fuel is lost while it waits.
        assert_eq!(c, Centrifuge { burn: 30, burn_max: 100, spin: 250, stage: 0 });
        let mut c = Centrifuge::default();
        assert_eq!(c.step(raw(4), COAL, FULL, &most), Step::IDLE, "no fuel is taken for nothing");
        assert_eq!(c, Centrifuge::default());
    }

    #[test]
    fn state_round_trips() {
        let c = Centrifuge { burn: 1234, burn_max: 1600, spin: 321, stage: 0 };
        let back: Centrifuge = serde_json::from_slice(&serde_json::to_vec(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        // What an older or empty entity holds reads as a cold centrifuge.
        assert_eq!(serde_json::from_str::<Centrifuge>("{}").unwrap(), Centrifuge::default());
    }

    #[test]
    fn the_screen_shows_the_fire_and_the_spin() {
        let c = Centrifuge { burn: 800, burn_max: 1600, spin: 100, stage: 0 };
        assert_eq!(c.data(), [800, 1600, 100, SPIN_TICKS as i32]);
        // Data slots are 16 bits on the wire.
        let hot = Centrifuge { burn: 40_000, burn_max: 40_000, spin: 0, stage: 0 };
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

    /// A world with nothing in it, as a bomb's structure reads one.
    fn air(_: [i32; 3]) -> String {
        "minecraft:air".into()
    }

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
        assert_eq!(BombKind::ALL.map(BombKind::radius), [16, 50, 250, 1000]);
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
        assert_eq!(saved.fire_bomb(NETHER, roof_bomb, Some(&air)), Ok(1));
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
        assert_eq!(saved.set_off(OVERWORLD, near, &air), Ok(2));
        assert!(reached(&saved.placed, &blast, chunk_of(0, 0)).is_empty());
        assert_eq!(saved.recorded(OVERWORLD, near), None, "its remote finds it gone");
        assert_eq!(saved.set_off(OVERWORLD, near, &air), Err(Refused::Gone));
        saved.maxpower = 16;
        assert!(saved.place(OVERWORLD, [12, 64, 0], BombKind::Atomic));
        assert_eq!(saved.set_off(OVERWORLD, [12, 64, 0], &air), Err(Refused::Maxpower { radius: 250, maxpower: 16 }));
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
        assert_eq!(saved.fire_bomb(OVERWORLD, at, Some(&air)), Ok(1));
        let news = saved.waves.news(OVERWORLD, saved.tick);
        assert_eq!(news.iter().map(|n| (n.id, n.center, n.radius)).collect::<Vec<_>>(), vec![(1, at, 50)]);
        // The record goes with the blast: a second press finds no bomb.
        assert_eq!(saved.recorded(OVERWORLD, at), None);
        assert_eq!(saved.fire_bomb(OVERWORLD, at, Some(&air)), Err(Refused::Gone));
        assert_eq!(saved.next_id, 1, "no second blast");
        // The bomb still standing while its wave runs isn't linked again.
        assert!(!saved.record(OVERWORLD, at, BombKind::Tactical));
        assert_eq!(saved.fire_bomb(OVERWORLD, at, Some(&air)), Err(Refused::Gone));
        // A bomb is its dimension's and its place's: the same place elsewhere is another.
        assert_eq!(saved.fire_bomb(NETHER, at, Some(&air)), Err(Refused::Gone));
        assert!(saved.record(NETHER, at, BombKind::Atomic));
        assert_eq!(saved.fire_bomb(NETHER, at, Some(&air)), Ok(2));
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
        assert_eq!(saved.fire_bomb(OVERWORLD, a, Some(&air)), Err(Refused::Gone));
        assert!(saved.waves.finished(), "a broken bomb doesn't go off");
        assert_eq!(saved.fire_bomb(OVERWORLD, b, Some(&air)), Ok(1));
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
        assert_eq!(saved.fire_bomb(OVERWORLD, atomic, Some(&air)), Err(Refused::Maxpower { radius: 250, maxpower: 50 }));
        assert_eq!(saved.recorded(OVERWORLD, atomic), Some(BombKind::Atomic));
        assert_eq!(saved.fire_bomb(OVERWORLD, tactical, Some(&air)), Ok(2));
        saved.maxpower = 16;
        assert!(saved.allows(BombKind::Demolition.radius()));
        assert!(!saved.allows(BombKind::Tactical.radius()));
        assert_eq!(saved.fire_bomb(OVERWORLD, atomic, Some(&air)), Err(Refused::Maxpower { radius: 250, maxpower: 16 }));
        saved.maxpower = 250;
        assert_eq!(saved.fire_bomb(OVERWORLD, atomic, Some(&air)), Ok(3));
    }

    #[test]
    fn maxpower_takes_only_its_steps() {
        assert_eq!(MAXPOWER_STEPS, [16, 50, 250, 1000, 1024]);
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
        let at = |pos: [f64; 3], now: u64| background(&zones, &[], OW, pos, now);
        let mid = [0.5, 64.5, 0.5];
        assert!(close(at(mid, 1000), 10.0));
        // Half way out a quarter is gone; up and down count as across.
        assert!(close(at([72.5, 64.5, 0.5], 1000), 7.5));
        assert!(close(at([0.5, 136.5, 0.5], 1000), 7.5));
        assert!(at([60.5, 64.5, 60.5], 1000) < at([30.5, 64.5, 30.5], 1000));
        assert_eq!(at([144.5, 64.5, 0.5], 1000), 0.0);
        assert_eq!(at([500.5, 64.5, 0.5], 1000), 0.0);
        assert_eq!(background(&zones, &[], "minecraft:the_nether", mid, 1000), 0.0);
        // Halved every half-life, as strong as new before it starts.
        assert!(close(at(mid, 13_000), 5.0));
        assert!(close(at(mid, 25_000), 2.5));
        assert!(close(at(mid, 0), 10.0));
        // Two zones add up.
        let two = [zones[0].clone(), zone_of(&blast([100, 64, 0], 120, 2, World::OVERWORLD), 1000).unwrap()];
        assert!(close(background(&two, &[], OW, [50.5, 64.5, 0.5], 1000), 2.0 * 10.0 * (1.0 - (50.0f64 / 144.0).powi(2))));
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
            let server = background(&saved.zones, &[], OW, pos, saved.tick + later);
            let client = told.background(OW, pos, 123_456 + later);
            assert!(close(server, client), "{pos:?} {later}: {server} {client}");
            assert!(server > 0.0);
        }
        assert_eq!(told.background("minecraft:the_nether", [0.5, 64.0, 0.5], 123_456), 0.0);
        // A clock behind the message's (another world) counts as no time passed.
        assert!(close(told.background(OW, [0.5, 64.0, 0.5], 0), background(&saved.zones, &[], OW, [0.5, 64.0, 0.5], 9_000)));
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
        tint_fog(fog_strength(bg), color, start, end, 1.0)
    }

    #[test]
    fn the_zone_fog_darkens_with_the_sky() {
        // The sky's light by the time of day, as vanilla's fog colour follows it: full at noon, none at midnight.
        assert_eq!(sky_light(6000), 1.0);
        assert_eq!(sky_light(18_000), 0.0);
        assert_eq!(sky_light(6000 + 24_000 * 3), 1.0, "any day");
        assert!(sky_light(12_500) > 0.0 && sky_light(12_500) < 1.0, "dusk");
        for t in (0..24_000).step_by(250) {
            assert!((0.0..=1.0).contains(&sky_light(t)), "{t}");
        }
        // A full zone's fog at midnight is the night's, not a bright green over the dark.
        let night = [0.04, 0.05, 0.08, 1.0];
        let (c, _, _) = tint_fog(1.0, night, 0.0, 1024.0, sky_light(18_000)).unwrap();
        assert!(c[..3].iter().all(|v| *v < 0.06), "{c:?}");
        let (noon, _, _) = tint_fog(1.0, [0.6, 0.7, 0.9, 1.0], 0.0, 1024.0, sky_light(6000)).unwrap();
        assert!(noon.iter().zip([0.55, 0.62, 0.45, 1.0]).all(|(a, b)| (a - b).abs() < 1e-6), "{noon:?}");
        // At dusk it is between, and still green over grey.
        let (dusk, _, _) = tint_fog(1.0, [0.3, 0.3, 0.3, 1.0], 0.0, 1024.0, sky_light(12_800)).unwrap();
        assert!(dusk[1] < noon[1] && dusk[1] > c[1] && dusk[1] > dusk[2], "{dusk:?}");
        // The light doesn't touch the distances, nor the fog's alpha.
        assert_eq!(tint_fog(1.0, night, 0.0, 1024.0, 0.0).map(|(c, s, e)| (c[3], s, e)), Some((1.0, 0.0, 64.0)));
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

    // ---- the sunbreaker (plan 8, Task 1) ------------------------------------------------

    const CORE_AT: [i32; 3] = [100, 64, -40];

    /// The blocks of a Sunbreaker at `core` with its whole structure.
    fn sunbreaker(core: [i32; 3]) -> BTreeMap<[i32; 3], &'static str> {
        let mut w: BTreeMap<[i32; 3], &'static str> =
            structure_cells(core).map(|(p, layer)| (p, if layer == 1 { IMPLOSION_SHELL } else { NEUTRON_REFLECTOR })).collect();
        w.insert(core, SUNBREAKER_CORE);
        w
    }

    /// A world of `w`'s blocks and air.
    fn reader<'a>(w: &'a BTreeMap<[i32; 3], &'static str>) -> impl Fn([i32; 3]) -> String + 'a {
        move |p| w.get(&p).copied().unwrap_or("minecraft:air").to_owned()
    }

    /// The places of `core`'s structure in one layer.
    fn layer(core: [i32; 3], wanted: u8) -> Vec<[i32; 3]> {
        structure_cells(core).filter(|c| c.1 == wanted).map(|c| c.0).collect()
    }

    #[test]
    fn a_full_structure_gives_a_thousand() {
        let cells: Vec<([i32; 3], u8)> = structure_cells(CORE_AT).collect();
        let distinct: BTreeSet<[i32; 3]> = cells.iter().map(|c| c.0).collect();
        assert_eq!(distinct.len(), cells.len());
        assert!(!distinct.contains(&CORE_AT));
        assert_eq!((layer(CORE_AT, 1).len() as u32, layer(CORE_AT, 2).len() as u32), (SHELL_BLOCKS, REFLECTOR_BLOCKS));
        let w = sunbreaker(CORE_AT);
        let s = read_structure(CORE_AT, &reader(&w));
        assert_eq!(s, Structure { shell: 26, reflector: 98 });
        assert_eq!(s.radius(), Ok(1000));
        assert_eq!(BombKind::Sunbreaker.power(CORE_AT, &reader(&w)), Ok(1000));
        assert_eq!(BombKind::Sunbreaker.radius(), 1000);
        assert!(BombKind::Sunbreaker.radius() <= MAX_RADIUS && MAXPOWER_STEPS.iter().any(|s| *s >= 1000));
        // The others don't read the world.
        assert_eq!(BombKind::Atomic.power(CORE_AT, &air), Ok(250));
        assert_eq!(BombKind::of_block(SUNBREAKER_CORE), Some(BombKind::Sunbreaker));
    }

    #[test]
    fn a_full_shell_alone_gives_six_hundred() {
        let mut w = sunbreaker(CORE_AT);
        for p in layer(CORE_AT, 2) {
            w.remove(&p);
        }
        assert_eq!(read_structure(CORE_AT, &reader(&w)), Structure { shell: 26, reflector: 0 });
        assert_eq!(BombKind::Sunbreaker.power(CORE_AT, &reader(&w)), Ok(SUNBREAKER_BASE));
        assert_eq!(SUNBREAKER_BASE, 600);
    }

    #[test]
    fn each_reflector_block_adds_its_share() {
        let mut w = sunbreaker(CORE_AT);
        for p in layer(CORE_AT, 2).into_iter().take(25) {
            w.remove(&p);
        }
        // 73 of 98: the chat's example.
        assert_eq!(Structure { shell: 26, reflector: 73 }.radius(), Ok(897));
        assert_eq!(BombKind::Sunbreaker.power(CORE_AT, &reader(&w)), Ok(897));
        for p in layer(CORE_AT, 2).into_iter().take(49) {
            w.remove(&p);
        }
        assert_eq!(read_structure(CORE_AT, &reader(&w)).reflector, 49);
        assert_eq!(BombKind::Sunbreaker.power(CORE_AT, &reader(&w)), Ok(800));
        // Each block counts, rounded down.
        assert_eq!(Structure { shell: 26, reflector: 1 }.radius(), Ok(604));
        assert_eq!(Structure { shell: 26, reflector: 97 }.radius(), Ok(995));
    }

    #[test]
    fn a_gap_in_the_shell_refuses() {
        let mut w = sunbreaker(CORE_AT);
        w.remove(&layer(CORE_AT, 1)[7]);
        assert_eq!(read_structure(CORE_AT, &reader(&w)), Structure { shell: 25, reflector: 98 });
        assert_eq!(BombKind::Sunbreaker.power(CORE_AT, &reader(&w)), Err(StructureProblem::Shell(25)));
        // A whole reflector makes up for nothing.
        assert_eq!(Structure { shell: 0, reflector: 98 }.radius(), Err(StructureProblem::Shell(0)));
    }

    #[test]
    fn a_wrong_block_counts_as_missing() {
        let mut w = sunbreaker(CORE_AT);
        w.insert(layer(CORE_AT, 2)[0], "minecraft:glass");
        assert_eq!(read_structure(CORE_AT, &reader(&w)).reflector, 97);
        // Each part counts only in its own layer.
        w.insert(layer(CORE_AT, 2)[1], IMPLOSION_SHELL);
        w.insert(layer(CORE_AT, 1)[0], NEUTRON_REFLECTOR);
        assert_eq!(read_structure(CORE_AT, &reader(&w)), Structure { shell: 25, reflector: 96 });
        assert_eq!(BombKind::Sunbreaker.power(CORE_AT, &reader(&w)), Err(StructureProblem::Shell(25)));
    }

    #[test]
    fn an_unread_block_counts_as_missing() {
        assert_eq!(read_structure(CORE_AT, &air), Structure::default());
        assert_eq!(BombKind::Sunbreaker.power(CORE_AT, &air), Err(StructureProblem::Shell(0)));
        // A core two blocks from its chunk's edge, the next chunk not loaded: the
        // reflector's far face is lost, the bomb is weaker but goes off.
        let core = [14, 64, 8];
        let w = sunbreaker(core);
        let loaded = |p: [i32; 3]| if chunk_of(p[0], p[2]) == [0, 0] { reader(&w)(p) } else { "minecraft:air".to_owned() };
        assert_eq!(read_structure(core, &loaded), Structure { shell: 26, reflector: 73 });
        assert_eq!(BombKind::Sunbreaker.power(core, &loaded), Ok(897));
    }

    #[test]
    fn core_near_finds_the_core_from_any_cell() {
        let w = sunbreaker(CORE_AT);
        assert_eq!(core_near(CORE_AT, &reader(&w)), Some(CORE_AT));
        for (p, _) in structure_cells(CORE_AT) {
            assert_eq!(core_near(p, &reader(&w)), Some(CORE_AT), "{p:?}");
        }
        // Three blocks out is past the reflector.
        assert_eq!(core_near([CORE_AT[0] + 3, CORE_AT[1], CORE_AT[2]], &reader(&w)), None);
        assert_eq!(core_near(CORE_AT, &air), None);
        // Two cores: the nearer.
        let mut two = sunbreaker(CORE_AT);
        let other = [CORE_AT[0] + 4, CORE_AT[1], CORE_AT[2]];
        two.insert(other, SUNBREAKER_CORE);
        assert_eq!(core_near([CORE_AT[0] + 3, CORE_AT[1], CORE_AT[2]], &reader(&two)), Some(other));
        assert_eq!(core_near([CORE_AT[0] + 1, CORE_AT[1], CORE_AT[2]], &reader(&two)), Some(CORE_AT));
    }

    #[test]
    fn a_chain_detonation_reads_what_is_left_of_the_structure() {
        let mut saved = Saved::default();
        let atomic = [0, 64, 0];
        let (strong, broken) = ([40, 64, 0], [0, 64, 40]);
        assert!(saved.place(OVERWORLD, atomic, BombKind::Atomic));
        assert!(saved.place(OVERWORLD, strong, BombKind::Sunbreaker));
        assert!(saved.place(OVERWORLD, broken, BombKind::Sunbreaker));
        assert!(saved.record(OVERWORLD, broken, BombKind::Sunbreaker));
        let id = saved.fire(OVERWORLD, atomic, 250).unwrap();
        let blast = saved.waves.waves.iter().find(|w| w.blast.id == id).unwrap().blast.clone();
        assert_eq!(reached(&saved.placed, &blast, chunk_of(40, 0)), vec![(strong, BombKind::Sunbreaker)]);
        // The wave has taken half its reflector by the time it gets there.
        let mut w = sunbreaker(strong);
        w.extend(sunbreaker(broken));
        for p in layer(strong, 2).into_iter().take(49) {
            w.remove(&p);
        }
        let set_off = saved.set_off(OVERWORLD, strong, &reader(&w)).unwrap();
        let news = saved.waves.news(OVERWORLD, saved.tick);
        let n = news.iter().find(|n| n.id == set_off).unwrap();
        assert_eq!((n.center, n.radius), (strong, 800));
        assert!(saved.placed.iter().all(|b| b.pos != strong));
        // One without a whole shell doesn't go off, and stays: its core is only taken by a crater that reaches it.
        w.remove(&layer(broken, 1)[3]);
        let before = saved.next_id;
        assert_eq!(saved.set_off(OVERWORLD, broken, &reader(&w)), Err(Refused::Structure(StructureProblem::Shell(25))));
        assert_eq!(saved.next_id, before);
        assert!(saved.placed.iter().any(|b| b.pos == broken), "an unfinished core stays placed");
        assert_eq!(saved.recorded(OVERWORLD, broken), Some(BombKind::Sunbreaker), "and its remote link stays");
    }

    #[test]
    fn a_remote_fires_a_distant_sunbreaker_with_its_last_read_radius() {
        let mut saved = Saved::default();
        let core = [0, 64, 0];
        assert!(saved.place(OVERWORLD, core, BombKind::Sunbreaker));
        assert!(saved.record(OVERWORLD, core, BombKind::Sunbreaker));
        // Never read: no radius to go off with from afar.
        assert_eq!(saved.fire_bomb(OVERWORLD, core, None), Err(Refused::Unread));
        assert!(saved.read_power(OVERWORLD, core, Ok(897)));
        assert!(!saved.read_power(OVERWORLD, core, Ok(897)), "the same reading changes nothing");
        let id = saved.fire_bomb(OVERWORLD, core, None).unwrap();
        let news = saved.waves.news(OVERWORLD, saved.tick);
        assert_eq!(news.iter().find(|n| n.id == id).map(|n| n.radius), Some(897));
    }

    #[test]
    fn a_distant_sunbreaker_last_read_without_a_shell_stays() {
        let mut saved = Saved::default();
        let core = [0, 64, 0];
        assert!(saved.record(OVERWORLD, core, BombKind::Sunbreaker));
        saved.read_power(OVERWORLD, core, Ok(1000));
        saved.read_power(OVERWORLD, core, Err(StructureProblem::Shell(20)));
        assert_eq!(saved.fire_bomb(OVERWORLD, core, None), Err(Refused::Structure(StructureProblem::Shell(20))));
        assert_eq!(saved.recorded(OVERWORLD, core), Some(BombKind::Sunbreaker));
    }

    #[test]
    fn old_saved_receivers_read_without_a_power() {
        let json = r#"{"dimension":"minecraft:overworld","pos":[1,2,3],"kind":"Atomic"}"#;
        let r: Receiver = serde_json::from_str(json).unwrap();
        assert_eq!(r.power, None);
    }

    #[test]
    fn a_thousand_breaks_bedrock_at_the_center_only() {
        let w = World::OVERWORLD;
        let b = blast([0, 64, 0], BEDROCK_RADIUS, 9, w);
        let hole = BEDROCK_HOLE as i32;
        assert_eq!(crater_floor(&b, 0, 0, 0.0), w.min_y);
        assert_eq!(crater_floor(&b, hole, 0, edge(&b, hole, 0)), w.min_y);
        assert!(crater_floor(&b, 30, 0, edge(&b, 30, 0)) >= w.min_y + ABOVE_FLOOR);
        for x in 0..=1000 {
            let floor = crater_floor(&b, x, 0, edge(&b, x, 0));
            if x <= hole {
                assert_eq!(floor, w.min_y, "{x}");
            } else {
                assert!(floor >= w.min_y + ABOVE_FLOOR, "{x}");
            }
        }
        let short = blast([0, 64, 0], BEDROCK_RADIUS - 1, 9, w);
        for x in 0..=1000 {
            assert!(crater_floor(&short, x, 0, edge(&short, x, 0)) >= w.min_y + ABOVE_FLOOR, "{x}");
        }
        // The center's column is emptied from the world's floor up: the void shows under it.
        let [plants, _] = plan_chunk(&b, chunk_of(0, 0));
        let run = plants.runs.iter().find(|r| (r.x, r.z) == (0, 0) && r.only.is_none()).unwrap();
        assert_eq!((run.y, run.y + run.height as i32, run.block), (w.min_y, w.roof, AIR));
        let [plants, _] = plan_chunk(&short, chunk_of(0, 0));
        let run = plants.runs.iter().find(|r| (r.x, r.z) == (0, 0) && r.only.is_none()).unwrap();
        assert_eq!(run.y, w.min_y + ABOVE_FLOOR);
    }

    #[test]
    fn the_bedrock_hole_is_round_whatever_the_border() {
        let w = World::OVERWORLD;
        let r = BEDROCK_HOLE as i32;
        let round: BTreeSet<[i32; 2]> =
            (-r - 8..=r + 8).flat_map(|x| (-r - 8..=r + 8).map(move |z| [x, z])).filter(|[x, z]| x * x + z * z <= r * r).collect();
        for seed in [1, 9, 77, 4242] {
            let center = [100, 64, -300];
            let b = blast(center, BEDROCK_RADIUS, seed, w);
            let mut hole = BTreeSet::new();
            for cx in -4..=4 {
                for cz in -4..=4 {
                    let chunk = chunk_of(center[0], center[2]);
                    let [plants, _] = plan_chunk(&b, [chunk[0] + cx, chunk[1] + cz]);
                    for run in plants.runs.iter().filter(|r| r.only.is_none() && r.y == w.min_y) {
                        hole.insert([run.x - center[0], run.z - center[2]]);
                    }
                }
            }
            // Its rim is a circle of BEDROCK_HOLE round the center's column, the same for every blast.
            assert_eq!(hole, round, "seed {seed}");
        }
    }

    #[test]
    fn the_nether_keeps_its_roof_but_loses_its_floor() {
        let w = World::NETHER;
        let b = blast([0, 64, 0], 1000, 3, w);
        let [plants, _] = plan_chunk(&b, chunk_of(0, 0));
        let run = plants.runs.iter().find(|r| (r.x, r.z) == (0, 0) && r.only.is_none()).unwrap();
        // The floor's bedrock goes, the roof's (123 and up) stays.
        assert_eq!((run.y, run.y + run.height as i32), (w.min_y, w.roof));
        assert!(plants.runs.iter().chain(&plan_chunk(&b, chunk_of(0, 0))[1].runs).all(|r| r.y + r.height as i32 <= w.roof || r.surface.is_some()));
        // Past the hole the Nether's floor stays.
        let [plants, _] = plan_chunk(&b, chunk_of(40, 0));
        let off = plants.runs.iter().find(|r| (r.x, r.z) == (40, 0) && r.only.is_none()).unwrap();
        assert_eq!(off.y, w.min_y + ABOVE_FLOOR);
    }

    #[test]
    fn maxpower_holds_back_a_sunbreaker() {
        let mut saved = Saved { maxpower: 250, ..Saved::default() };
        let mut w = sunbreaker(CORE_AT);
        for p in layer(CORE_AT, 2).into_iter().take(25) {
            w.remove(&p);
        }
        assert!(saved.place(OVERWORLD, CORE_AT, BombKind::Sunbreaker));
        assert!(saved.record(OVERWORLD, CORE_AT, BombKind::Sunbreaker));
        let over = Err(Refused::Maxpower { radius: 897, maxpower: 250 });
        assert_eq!(saved.set_off(OVERWORLD, CORE_AT, &reader(&w)), over);
        assert_eq!(saved.fire_bomb(OVERWORLD, CORE_AT, Some(&reader(&w))), over);
        // Nothing went off and the bomb stays, for its remote and for the next wave.
        assert!(saved.waves.finished());
        assert_eq!(saved.recorded(OVERWORLD, CORE_AT), Some(BombKind::Sunbreaker));
        assert_eq!(saved.placed_in(OVERWORLD, chunk_of(CORE_AT[0], CORE_AT[2])), vec![CORE_AT]);
        // A remote's Sunbreaker without its shell stays too: it can be mended.
        saved.maxpower = MAX_RADIUS;
        assert_eq!(saved.fire_bomb(OVERWORLD, CORE_AT, Some(&air)), Err(Refused::Structure(StructureProblem::Shell(0))));
        assert_eq!(saved.recorded(OVERWORLD, CORE_AT), Some(BombKind::Sunbreaker));
        assert!(saved.fire_bomb(OVERWORLD, CORE_AT, Some(&reader(&w))).is_ok());
    }

    #[test]
    fn the_sunbreakers_fuse_burns_fifteen_seconds() {
        let mut b = Bomb::new(BombKind::Sunbreaker);
        for d in Detonator::ALL {
            assert!(BombKind::Sunbreaker.takes(d), "{d:?}");
        }
        b.install(Detonator::Fuse).unwrap();
        assert!(b.light(0));
        assert_eq!(b.left(0), Some(SUNBREAKER_FUSE));
        assert_eq!(SUNBREAKER_FUSE / 20, 15);
        assert!(!b.tick(SUNBREAKER_FUSE - 1));
        assert!(b.tick(SUNBREAKER_FUSE));
        // A clock gone back cuts the count to the Sunbreaker's own fuse, not the others'.
        let mut b = Bomb { fires_at: Some(100_000), ..Bomb::new(BombKind::Sunbreaker) };
        b.tick(0);
        assert_eq!(b.fires_at, Some(SUNBREAKER_FUSE));
        let mut a = Bomb { fires_at: Some(100_000), ..Bomb::new(BombKind::Atomic) };
        a.tick(0);
        assert_eq!(a.fires_at, Some(FUSE_TICKS.max(COUNTDOWN_TICKS)));
        assert!(ALARM_TICKS < SUNBREAKER_FUSE);
    }

    #[test]
    fn the_sunbreaker_neither_flies_nor_rides_a_bomber() {
        assert!(!BombKind::Sunbreaker.flies());
        assert!(!pad_takes(PAD_WARHEAD_SLOT, SUNBREAKER_CORE));
        assert_eq!(pad_load(Some(MISSILE), Some(SUNBREAKER_CORE)), Err(ShaftProblem::NoWarhead));
        let mut saved = Saved::default();
        assert_eq!(saved.charge(OVERWORLD, [0, 64, 0], Some(SUNBREAKER_CORE)), Charge::Not);
        assert_eq!(name_key(SUNBREAKER_CORE), "block.big_nukes.sunbreaker_core");
        assert_eq!(name_key(IMPLOSION_SHELL), "block.big_nukes.implosion_shell");
        assert_eq!(name_key(WEAPONS_GRADE_URANIUM), "item.big_nukes.weapons_grade_uranium");
    }

    #[test]
    fn the_centrifuge_makes_weapons_grade_from_four_enriched() {
        let enriched = |n: u32| Some((ENRICHED_URANIUM, n));
        let mut c = Centrifuge::default();
        // Three aren't enough to start.
        assert_eq!(c.step(enriched(3), COAL, None, &most), Step::IDLE);
        assert_eq!(c, Centrifuge::default());
        let mut made = 0;
        for tick in 1..=SPIN_TICKS * 3 {
            let step = c.step(enriched(4), COAL, None, &most);
            if step.made {
                made += 1;
                assert_eq!(tick, SPIN_TICKS * 3);
            }
            if tick == 1 {
                assert_eq!(c.data()[3], (SPIN_TICKS * 3) as i32, "the screen's arrow fills over the longer spin");
            }
        }
        assert_eq!(made, 1);
        assert_eq!(c.enrichment(), Enrichment { input: ENRICHED_URANIUM, takes: 4, output: WEAPONS_GRADE_URANIUM, ticks: 1200 });
        // Its output takes weapons-grade uranium only, up to a stack.
        let mut c = Centrifuge { burn: 100, burn_max: 100, spin: 0, stage: 0 };
        assert_eq!(c.step(enriched(4), COAL, Some((ENRICHED_URANIUM, 1)), &most), Step::IDLE);
        assert_eq!(c.step(enriched(4), COAL, Some((WEAPONS_GRADE_URANIUM, 16)), &most), Step::IDLE);
        assert!(!c.step(enriched(4), COAL, Some((WEAPONS_GRADE_URANIUM, 15)), &most).took_fuel);
        assert_eq!(c.spin, 1);
        // Changing the input for another loses the work on it.
        let mut c = Centrifuge { burn: 100, burn_max: 100, spin: 399, stage: 1 };
        assert_eq!(c.step(raw(1), COAL, None, &most), Step::default());
        assert_eq!((c.stage, c.spin), (0, 1));
        // What the centrifuge takes and gives.
        assert_eq!(enrichment(RAW_URANIUM), Some(0));
        assert_eq!(enrichment(ENRICHED_URANIUM), Some(1));
        assert_eq!(enrichment(WEAPONS_GRADE_URANIUM), None);
    }

    #[test]
    fn raw_uranium_still_makes_enriched() {
        let mut c = Centrifuge::default();
        for tick in 1..=SPIN_TICKS {
            assert_eq!(c.step(raw(1), COAL, Some((ENRICHED_URANIUM, 10)), &most).made, tick == SPIN_TICKS);
        }
        assert_eq!(c.enrichment().output, ENRICHED_URANIUM);
        assert_eq!(c.data()[3], SPIN_TICKS as i32);
        // Weapons-grade in the output keeps raw uranium waiting.
        let mut c = Centrifuge::default();
        assert_eq!(c.step(raw(1), COAL, Some((WEAPONS_GRADE_URANIUM, 1)), &most), Step::IDLE);
        // A centrifuge saved before the second step reads as enriching.
        let old: Centrifuge = serde_json::from_str(r#"{"burn":5,"burn_max":10,"spin":7}"#).unwrap();
        assert_eq!(old, Centrifuge { burn: 5, burn_max: 10, spin: 7, stage: 0 });
        // Something it doesn't take is nothing to spin.
        assert_eq!(c.step(Some(("minecraft:stone", 64)), COAL, None, &most), Step::IDLE);
    }

    #[test]
    fn the_three_parts_are_assembled() {
        let core = [Some((WEAPONS_GRADE_URANIUM, 4)), Some(("big_nukes:nuclear_charge", 4)), Some(("minecraft:diamond_block", 1))];
        let made = assemble(&core, None, &most).unwrap();
        assert_eq!((made.result, made.count), (SUNBREAKER_CORE, 1));
        let shell = [Some(("minecraft:tnt", 4)), Some(("minecraft:iron_block", 2)), Some(("minecraft:redstone", 2))];
        assert_eq!(assemble(&shell, None, &most).map(|a| (a.result, a.count)), Some((IMPLOSION_SHELL, 1)));
        let reflector = [Some(("minecraft:iron_block", 4)), Some((ENRICHED_URANIUM, 1))];
        assert_eq!(assemble(&reflector, None, &most).map(|a| (a.result, a.count)), Some((NEUTRON_REFLECTOR, 4)));
        // Four reflectors need room for four (`most` stacks 16).
        assert!(assemble(&reflector, Some((NEUTRON_REFLECTOR, 13)), &most).is_none());
        assert!(assemble(&reflector, Some((NEUTRON_REFLECTOR, 12)), &most).is_some());
        // Every recipe's own parts make it, and nothing else.
        for (i, a) in ASSEMBLIES.iter().enumerate() {
            let grid: Vec<Option<(&str, u32)>> = a.parts.iter().map(|&(id, n)| Some((id, n))).collect();
            assert_eq!(assembly_in(&grid), Some(i), "{}", a.result);
            assert!(a.count >= 1 && a.parts.iter().map(|p| p.1).sum::<u32>() as usize <= GRID_SLOTS, "{}", a.result);
        }
        // The core is the bomb, the parts are blocks, the uranium an item.
        assert!(ITEMS.iter().any(|i| i.0 == WEAPONS_GRADE_URANIUM && i.2));
        for id in [SUNBREAKER_CORE, IMPLOSION_SHELL, NEUTRON_REFLECTOR, WEAPONS_GRADE_URANIUM] {
            assert!(!tips(id).is_empty(), "{id}");
        }
    }

    #[test]
    fn a_held_click_is_heard_once_until_it_is_let_go() {
        let mut clicks = Clicks::new();
        let bomb = (7u128, [1, 2, 3], Some(REMOTE.to_owned()));
        assert_eq!(clicks.heard(bomb.clone(), 100), None, "a new click");
        assert_eq!(clicks.heard(bomb.clone(), 100), Some(0), "the other hand's turn of it");
        // The game repeats a held click every 4 ticks: each repeat keeps it held, however long.
        for t in (104..=200).step_by(4) {
            assert!(clicks.heard(bomb.clone(), t).is_some_and(|since| since > 0), "{t}");
        }
        // Let go a while, it is a new click.
        assert_eq!(clicks.heard(bomb.clone(), 200 + HELD_CLICK_TICKS + 1), None);
        // Another player, another block or another item is another click.
        let now = 300;
        assert_eq!(clicks.heard(bomb.clone(), now), None);
        assert_eq!(clicks.heard((8, [1, 2, 3], Some(REMOTE.to_owned())), now + 4), None);
        assert_eq!(clicks.heard((7, [1, 2, 4], Some(REMOTE.to_owned())), now + 4), None);
        assert_eq!(clicks.heard((7, [1, 2, 3], Some(FLINT_AND_STEEL.to_owned())), now + 4), None);
        assert_eq!(clicks.heard(bomb.clone(), now + 4), Some(4));
        // A clock that went back (another world) starts afresh.
        assert_eq!(clicks.heard(bomb, 10), None);
    }
}

// ---- flights (plan 7) ---------------------------------------------------------------

#[cfg(test)]
mod flight_tests {
    use super::*;
    use std::collections::BTreeSet;

    const OVERWORLD: &str = "minecraft:overworld";

    fn missile() -> Flight {
        Flight {
            id: 1,
            carrier: Carrier::Missile,
            dimension: OVERWORLD.into(),
            from: [0, 64, 0],
            to: [1000, 0],
            kind: BombKind::Atomic,
            start: 0,
            surface: Some(70),
            seed: 7,
            hatch: Some(71),
            hatch_moved: Some(false),
            emptied: false,
        }
    }

    fn near(a: f64, b: f64, by: f64) -> bool {
        (a - b).abs() <= by
    }

    /// How far apart two places are, across the ground.
    fn flat(a: [f64; 3], b: [f64; 3]) -> f64 {
        (a[0] - b[0]).hypot(a[2] - b[2])
    }

    fn ascent(f: &Flight, now: u64) -> ([f64; 3], f32, f32) {
        match f.leg(now) {
            Leg::Ascent { pos, yaw, pitch } => (pos, yaw, pitch),
            other => panic!("{now}: {other:?}"),
        }
    }

    fn descent(f: &Flight, now: u64) -> ([f64; 3], f32, f32) {
        match f.leg(now) {
            Leg::Descent { pos, yaw, pitch } => (pos, yaw, pitch),
            other => panic!("{now}: {other:?}"),
        }
    }

    /// Where yaw and pitch point, as the game reckons them: yaw 0 south, pitch -90 up.
    fn looks(yaw: f32, pitch: f32) -> [f64; 3] {
        let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
        [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
    }

    #[test]
    fn a_missile_counts_down_climbs_cruises_and_falls_on_its_target() {
        let f = missile();
        assert_eq!(f.leg(0), Leg::Countdown { left: 100 });
        assert_eq!(f.leg(99), Leg::Countdown { left: 1 });
        let (pos, _, _) = ascent(&f, 100);
        assert!(near(pos[1], 64.0, 0.5) && flat(pos, [0.5, 0.0, 0.5]) < 0.01, "{pos:?}");
        let (pos, _, _) = ascent(&f, 219);
        assert!(near(pos[1], 364.0, 0.5), "{pos:?}");
        // Faster and faster.
        let y = |t| ascent(&f, t).0[1];
        assert!(y(200) - y(199) > y(120) - y(119));
        let cruise = 1000_u64.div_ceil(16).max(40);
        assert_eq!(cruise, 63);
        assert_eq!(f.leg(220), Leg::Cruise);
        assert_eq!(f.leg(220 + cruise - 1), Leg::Cruise);
        let target = [1000.5, 0.0, 0.5];
        let (pos, yaw, pitch) = descent(&f, 220 + cruise);
        assert!(near(flat(pos, target), 200.0, 1.0) && pos[0] < 1000.0, "{pos:?}");
        assert!(near(pos[1], 390.0, 0.5), "{pos:?}");
        // Nose down, toward the target.
        let d = looks(yaw, pitch);
        assert!(d[0] > 0.0 && d[1] < 0.0 && d[2].abs() < 1e-6, "{d:?}");
        let last = 220 + cruise + 79;
        let (pos, _, _) = descent(&f, last);
        assert!(flat(pos, target) < 1.0 && near(pos[1], 70.0, 0.5), "{pos:?}");
        assert_eq!(f.leg(last + 1), Leg::Impact { at: [1000, 70, 0] });
        assert_eq!(f.leg(last + 5000), Leg::Impact { at: [1000, 70, 0] });
        // A flight started later keeps the same legs, later.
        let later = Flight { start: 1000, ..missile() };
        assert_eq!(later.leg(1000 + 150), f.leg(150));
        assert_eq!(later.leg(999), Leg::Countdown { left: 101 });
    }

    #[test]
    fn the_hatch_opens_forty_ticks_before_the_climb_and_closes_after() {
        let f = missile();
        assert!(!f.hatch_open(0) && !f.hatch_open(59));
        for t in 60..=160 {
            assert!(f.hatch_open(t), "{t}");
        }
        assert!(!f.hatch_open(161) && !f.hatch_open(5000));
        // By then the missile is well clear of any hatch a shaft has.
        assert!(ascent(&f, 160).0[1] > 64.0 + 40.0);
    }

    #[test]
    fn only_the_hatch_the_mod_took_comes_back() {
        let f = missile();
        let (before, open, after) = (59, 60, 161);
        // It is taken while it stands open, once, where it is.
        assert_eq!(f.hatch_act(before, Some(SILO_HATCH)), None);
        assert_eq!(f.hatch_act(open, Some(SILO_HATCH)), Some(HatchAct::Open));
        let taken = Flight { hatch_moved: Some(true), ..f.clone() };
        assert_eq!(taken.hatch_act(open + 1, Some("minecraft:air")), None);
        assert_eq!(taken.hatch_act(open + 1, Some(SILO_HATCH)), None, "a hatch put back by hand stays");
        // After, the one it took is put back on the air, and only that one.
        assert_eq!(taken.hatch_act(after, Some("minecraft:air")), Some(HatchAct::Shut));
        // One mined before it opened isn't made again: the flight forgets it.
        assert_eq!(f.hatch_act(open, Some("minecraft:air")), None);
        assert_eq!(f.hatch_act(after, Some("minecraft:air")), Some(HatchAct::Forget));
        assert_eq!(f.hatch_act(after, Some(SILO_HATCH)), Some(HatchAct::Forget));
        // A block put in its place keeps it out.
        assert_eq!(taken.hatch_act(after, Some("minecraft:stone")), Some(HatchAct::Forget));
        // A chunk not loaded reads as air: the hatch waits for it, open or shut.
        assert_eq!(f.hatch_act(open, None), None);
        assert_eq!(taken.hatch_act(after, None), None);
        assert_eq!(taken.hatch_act(after + 10_000, None), None);
        // A flight that forgot its hatch does nothing.
        assert_eq!(Flight { hatch: None, ..taken }.hatch_act(after, Some("minecraft:air")), None);
    }

    #[test]
    fn a_flight_saved_before_the_hatch_was_owed_puts_it_back_as_it_used_to() {
        // Saved in the open window by a version that didn't keep whether it took the hatch.
        let old = r#"{"id":4,"carrier":"Missile","dimension":"minecraft:overworld","from":[0,64,0],"to":[1000,0],
            "kind":"Atomic","start":0,"surface":null,"seed":7,"hatch":71}"#;
        let f: Flight = serde_json::from_str(old).unwrap();
        assert_eq!((f.hatch, f.hatch_moved), (Some(71), None));
        // As that version did: taken while it stands open, put back on the air after.
        assert_eq!(f.hatch_act(60, Some(SILO_HATCH)), Some(HatchAct::Open));
        assert_eq!(f.hatch_act(161, Some("minecraft:air")), Some(HatchAct::Shut));
        assert_eq!(f.hatch_act(161, Some("minecraft:stone")), Some(HatchAct::Forget));
        // A new launch keeps it, through a save.
        let mut saved = Saved::default();
        let launched = saved.launch(OVERWORLD, [0, 65, 0], [1000, 0], BombKind::Atomic, 72).unwrap();
        assert_eq!(launched.hatch_moved, Some(false));
        let back: Flight = serde_json::from_str(&serde_json::to_string(&launched).unwrap()).unwrap();
        assert_eq!(back, launched);
    }

    #[test]
    fn a_world_opened_again_wakes_the_dimensions_its_flights_are_in() {
        let mut saved = Saved::default();
        assert!(saved.to_wake().is_empty());
        let nether = Flight { id: 2, dimension: "minecraft:the_nether".into(), ..missile() };
        let end = Flight { id: 3, carrier: Carrier::Bomber, dimension: "minecraft:the_end".into(), hatch: None, ..missile() };
        saved.flights.extend([missile(), nether.clone()]);
        saved.beacons.push(end);
        // The overworld ticks anyway; another level ticks its forced chunks only once it is open.
        assert_eq!(saved.to_wake(), ["minecraft:the_end", "minecraft:the_nether"]);
        // A flight gone off whose wave still keeps its chunk forced.
        saved.flights.retain(|f| f.id != nether.id);
        saved.beacons.clear();
        assert!(saved.to_wake().is_empty());
        saved.force(nether.id, &nether.dimension, [3, 3]);
        saved.force(nether.id, &nether.dimension, [4, 4]);
        saved.landed(nether.id, Some([60, 70, 60]));
        assert_eq!(saved.to_wake(), ["minecraft:the_nether"]);
    }

    #[test]
    fn the_mod_lets_go_only_of_the_chunks_it_forced_and_as_soon_as_it_can() {
        let mut saved = Saved::default();
        // The pad's chunk goes as soon as the hatch is back; the target's stays.
        assert!(saved.force(1, OVERWORLD, [0, 0]) && saved.force(1, OVERWORLD, [62, 0]));
        assert!(saved.release(1, OVERWORLD, [0, 0]), "the mod's to let go");
        assert!(!saved.release(1, OVERWORLD, [0, 0]), "once");
        assert_eq!(saved.forced.iter().map(|f| f.chunk).collect::<Vec<_>>(), [[62, 0]]);
        // A chunk another flight holds stays forced until the last lets go.
        saved.force(2, OVERWORLD, [5, 5]);
        saved.force(3, OVERWORLD, [5, 5]);
        assert!(!saved.release(2, OVERWORLD, [5, 5]));
        assert!(saved.release(3, OVERWORLD, [5, 5]));
        assert!(!saved.release(9, OVERWORLD, [5, 5]), "never held");
        // A chunk the player had forced already (`/forceload` is one flag a chunk): the flights keep it, and
        // the mod never lets it go, neither by release nor once the wave has worked it.
        assert!(saved.force(4, OVERWORLD, [9, 9]));
        saved.theirs(OVERWORLD, [9, 9]);
        assert!(!saved.force(5, OVERWORLD, [9, 9]));
        assert!(!saved.release(4, OVERWORLD, [9, 9]));
        saved.landed(5, None);
        assert!(saved.unforce().is_empty());
        assert!(saved.forced.iter().all(|f| f.chunk != [9, 9]));
        // Forced by the mod again later, it is asked about again.
        assert!(saved.force(6, OVERWORLD, [9, 9]));
        saved.landed(6, None);
        saved.landed(1, None);
        let mut free = saved.unforce();
        free.sort();
        assert_eq!(free, [(OVERWORLD.to_owned(), [9, 9]), (OVERWORLD.to_owned(), [62, 0])]);
        // A chunk saved before this was the mod's own.
        saved.force(7, OVERWORLD, [1, 1]);
        saved.theirs(OVERWORLD, [1, 1]);
        let mut v = serde_json::to_value(&saved.forced[0]).unwrap();
        assert_eq!(v["theirs"], true);
        v.as_object_mut().unwrap().remove("theirs");
        assert!(!serde_json::from_value::<Forced>(v).unwrap().theirs);
    }

    #[test]
    fn a_flight_keeps_its_pad_and_its_target_loaded() {
        let mut saved = Saved::default();
        assert!(saved.force(1, OVERWORLD, [0, 0]), "the pad");
        assert!(saved.force(1, OVERWORLD, [62, 0]), "the target");
        assert!(!saved.force(1, OVERWORLD, [0, 0]) && !saved.force(1, OVERWORLD, [62, 0]));
        assert_eq!(saved.forced.len(), 2);
        saved.landed(1, None);
        let mut free = saved.unforce();
        free.sort();
        assert_eq!(free, vec![(OVERWORLD.to_owned(), [0, 0]), (OVERWORLD.to_owned(), [62, 0])]);
        assert!(saved.forced.is_empty());
    }

    #[test]
    fn the_missile_tilts_toward_its_target_only_late_in_the_climb() {
        for (to, toward) in [([1000, 0], [1.0, 0.0]), ([0, -1000], [0.0, -1.0]), ([-700, 700], [-0.7071, 0.7071])] {
            let f = Flight { to, ..missile() };
            for t in 100..180 {
                assert_eq!(ascent(&f, t).2, -90.0, "{t}");
            }
            let (pos, yaw, pitch) = ascent(&f, 219);
            let tilt = f64::from(pitch) + 90.0;
            assert!(tilt > 20.0 && tilt <= 30.0, "{tilt}");
            let d = looks(yaw, pitch);
            let across = d[0].hypot(d[2]);
            assert!(near(d[0] / across, toward[0], 1e-3) && near(d[2] / across, toward[1], 1e-3), "{to:?}: {d:?}");
            // It drifts the way it leans.
            let drift = [pos[0] - 0.5, pos[2] - 0.5];
            assert!(drift[0] * toward[0] + drift[1] * toward[1] > 5.0, "{to:?}: {pos:?}");
        }
    }

    #[test]
    fn a_descent_waits_for_the_targets_surface() {
        let f = Flight { surface: None, ..missile() };
        assert!(matches!(f.leg(150), Leg::Ascent { .. }));
        assert_eq!(f.leg(220 + 63), Leg::Cruise);
        assert_eq!(f.leg(100_000), Leg::Cruise);
    }

    #[test]
    fn a_flight_goes_on_by_the_clock_whatever_the_display_does() {
        let f = missile();
        let back: Flight = serde_json::from_slice(&serde_json::to_vec(&f).unwrap()).unwrap();
        for t in 0..600 {
            assert_eq!(f.leg(t), f.leg(t), "{t}");
            assert_eq!(back.leg(t), f.leg(t), "{t}");
        }
    }

    #[test]
    fn flights_survive_a_save_and_load() {
        let mut saved = Saved::default();
        saved.flights.push(missile());
        saved.beacons.push(Flight { carrier: Carrier::Bomber, id: 2, ..missile() });
        saved.consoles.insert(console_key(OVERWORLD, [1, 2, 3]), Console { code: Some("1234".into()), tries: 1, locked_until: 40 });
        let back: Saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        assert_eq!(back.flights, saved.flights);
        assert_eq!(back.beacons, saved.beacons);
        assert_eq!(back.consoles, saved.consoles);
        // A world from before the silo.
        let old: Saved = serde_json::from_str(r#"{"tick":5,"next_id":2,"waves":{"waves":[]}}"#).unwrap();
        assert!(old.flights.is_empty() && old.beacons.is_empty() && old.consoles.is_empty());
        // A flight saved without its hatch.
        let mut v = serde_json::to_value(missile()).unwrap();
        v.as_object_mut().unwrap().remove("hatch");
        let f: Flight = serde_json::from_value(v).unwrap();
        assert_eq!(f.hatch, None);
        assert_ne!(console_key(OVERWORLD, [1, 2, 3]), console_key("minecraft:the_nether", [1, 2, 3]));
    }

    #[test]
    fn a_shaft_needs_room_a_hatch_and_the_sky() {
        let with = |air: usize, top: &[&'static str]| -> Vec<&'static str> {
            let mut c = vec!["air"; air];
            c.extend(top);
            c
        };
        assert_eq!(shaft(&with(6, &[SILO_HATCH, "air"])), Ok(7));
        assert_eq!(shaft(&with(5, &[SILO_HATCH])), Err(ShaftProblem::LowRoom(5)));
        assert_eq!(shaft(&with(8, &[])), Err(ShaftProblem::NoHatch));
        assert_eq!(shaft(&with(6, &[SILO_HATCH, "air", "minecraft:stone"])), Err(ShaftProblem::NoSky));
        // The game's names for air.
        let mut named = vec!["minecraft:air", "minecraft:cave_air"];
        named.extend(["minecraft:air"; 5]);
        named.extend([SILO_HATCH, "minecraft:air", "minecraft:void_air"]);
        assert_eq!(shaft(&named), Ok(8));
        // Something in the shaft below the hatch.
        assert_eq!(shaft(&with(3, &["minecraft:stone", "air", "air", "air", SILO_HATCH])), Err(ShaftProblem::LowRoom(3)));
        assert_eq!(shaft(&with(7, &["minecraft:stone", SILO_HATCH])), Err(ShaftProblem::NoHatch));
        assert_eq!(shaft(&[]), Err(ShaftProblem::LowRoom(0)));
        // What the pad holds.
        let atomic = Some(BombKind::Atomic.id());
        assert_eq!(pad_load(Some(MISSILE), atomic), Ok(BombKind::Atomic));
        assert_eq!(pad_load(Some(MISSILE), Some(BombKind::Tactical.id())), Ok(BombKind::Tactical));
        assert_eq!(pad_load(None, atomic), Err(ShaftProblem::NoMissile));
        assert_eq!(pad_load(Some("minecraft:stick"), atomic), Err(ShaftProblem::NoMissile));
        assert_eq!(pad_load(Some(MISSILE), None), Err(ShaftProblem::NoWarhead));
        assert_eq!(pad_load(Some(MISSILE), Some(BombKind::Demolition.id())), Err(ShaftProblem::NoWarhead));
    }

    #[test]
    fn the_console_refuses_with_a_reason() {
        use LaunchRefusal::*;
        let at = [0, 0];
        assert_eq!(launch_request(at, "", "5", "123"), Err(Empty));
        assert_eq!(launch_request(at, "500", "", "123"), Err(Empty));
        assert_eq!(launch_request(at, "500", "5", ""), Err(Empty));
        assert_eq!(launch_request(at, "1a", "5", "123"), Err(NotANumber));
        assert_eq!(launch_request(at, "500", "5-", "123"), Err(NotANumber));
        assert_eq!(launch_request(at, "-", "500", "123"), Err(NotANumber));
        assert_eq!(launch_request(at, "+500", "5", "123"), Err(NotANumber));
        assert_eq!(launch_request(at, "40000000", "0", "123"), Err(OutsideWorld));
        assert_eq!(launch_request(at, "0", "-99999999999999999999999", "123"), Err(OutsideWorld));
        assert_eq!(launch_request(at, "30", "40", "123"), Err(TooNear(50)));
        assert_eq!(launch_request(at, "3000", "4000", "123"), Err(TooFar(5000)));
        assert_eq!(launch_request([100, 100], "130", "140", "123"), Err(TooNear(50)));
        assert_eq!(launch_request(at, "500", "5", "12a"), Err(NotANumber));
        assert_eq!(launch_request(at, "500", "5", "12"), Err(CodeLength));
        assert_eq!(launch_request(at, "500", "5", "123456789"), Err(CodeLength));
        assert_eq!(launch_request([100, 100], "-500", "-1000", "123"), Ok(([-500, -1000], "123".to_owned())));
        assert_eq!(launch_request(at, "4096", "0", "12345678"), Ok(([4096, 0], "12345678".to_owned())));
        assert_eq!(launch_request(at, "-100", "0", "000"), Ok(([-100, 0], "000".to_owned())));
    }

    #[test]
    fn a_try_on_an_empty_pad_sets_no_code() {
        let mut c = Console::default();
        // The first try passes the check, but the pad is empty: no launch, so no code.
        assert_eq!(c.check("123", 0), Ok(()));
        assert_eq!(c.check("4567", 1), Ok(()), "another code is still free to try");
        assert_eq!(c.code, None);
    }

    #[test]
    fn the_console_lets_the_function_keys_and_escape_through() {
        for key in ["key.keyboard.escape", "key.keyboard.f1", "key.keyboard.f2", "key.keyboard.f3", "key.keyboard.f11"] {
            assert!(!console_takes(key), "{key}");
        }
        for key in ["key.keyboard.e", "key.keyboard.1", "key.keyboard.tab", "key.keyboard.enter", "key.keyboard.q", "key.keyboard.f"] {
            assert!(console_takes(key), "{key}");
        }
    }

    #[test]
    fn a_bomb_dropped_in_the_nether_lands_under_the_roof() {
        let f = Flight {
            id: 1,
            carrier: Carrier::Bomber,
            dimension: "minecraft:the_nether".into(),
            from: [0, 70, 0],
            to: [0, 0],
            kind: BombKind::Tactical,
            start: 0,
            surface: Some(128),
            seed: 1,
            hatch: None,
            hatch_moved: None,
            emptied: false,
        };
        assert_eq!(f.bomb_ground(), 123);
        let over = Flight { dimension: "minecraft:overworld".into(), ..f.clone() };
        assert_eq!(over.bomb_ground(), 128);
        assert_eq!(Flight { surface: None, ..f }.bomb_ground(), 70, "unread ground: the beacon's own height");
    }

    #[test]
    fn a_console_takes_its_first_code_then_only_that_code() {
        use LaunchRefusal::*;
        let mut c = Console::default();
        assert_eq!(c.check("12", 0), Err(CodeLength));
        assert_eq!(c.check("", 0), Err(Empty));
        assert_eq!(c.check("12a4", 0), Err(NotANumber));
        assert_eq!(c.code, None);
        assert_eq!(c.check("1234", 0), Ok(()));
        assert_eq!(c.code, None, "a code is kept only once a launch goes");
        c.keep("1234");
        assert_eq!(c.code.as_deref(), Some("1234"));
        c.keep("5555");
        assert_eq!(c.code.as_deref(), Some("1234"), "the first launch's code stays");
        assert_eq!(c.check("1234", 1), Ok(()));
        assert_eq!(c.check("9999", 2), Err(WrongCode(2)));
        assert_eq!(c.check("1234", 3), Ok(()), "the right code clears the wrong ones");
        assert_eq!(c.check("9999", 4), Err(WrongCode(2)));
        assert_eq!(c.check("9999", 5), Err(WrongCode(1)));
        assert_eq!(c.check("9999", 6), Err(Locked(LOCK_TICKS)));
        assert_eq!(c.check("1234", 7), Err(Locked(LOCK_TICKS - 1)));
        assert_eq!(c.check("1234", 6 + LOCK_TICKS), Ok(()));
        // A lock read from a world whose clock went back is cut to its own length.
        let mut c = Console { locked_until: 1_000_000, ..c };
        assert_eq!(c.check("1234", 10), Err(Locked(LOCK_TICKS)));
    }

    #[test]
    fn a_target_in_the_nether_is_under_the_roof() {
        let f = Flight { dimension: "minecraft:the_nether".into(), surface: Some(128), ..missile() };
        let Leg::Impact { at } = f.leg(10_000) else { panic!("{:?}", f.leg(10_000)) };
        assert!(at[1] <= 123, "{at:?}");
        let (pos, _, _) = descent(&f, f.lands_at() - 1);
        assert!(pos[1] <= 123.5, "{pos:?}");
        let low = Flight { dimension: "minecraft:the_nether".into(), surface: Some(40), ..missile() };
        assert_eq!(low.leg(10_000), Leg::Impact { at: [1000, 40, 0] });
    }

    #[test]
    fn a_plane_flies_over_the_beacon_and_drops_its_bomb_close_to_it() {
        let beacon = [0.5, 64.0, 0.5];
        let mut ways = BTreeSet::new();
        for seed in [1, 2, 3, 99, 12345, u64::MAX] {
            let p = Pass::new([0, 64, 0], 1000, seed);
            assert!(p.plane(999).is_none());
            let (first, _) = p.plane(1000).unwrap();
            assert!(near(flat(first, beacon), 150.0, 1.0) && near(first[1], 184.0, 1e-9), "{first:?}");
            ways.insert([((first[0] - 0.5) / 15.0).round() as i64, ((first[2] - 0.5) / 15.0).round() as i64]);
            let closest = (1000..1300).filter_map(|t| p.plane(t)).map(|(at, _)| flat(at, beacon)).fold(f64::MAX, f64::min);
            assert!(closest <= 1.6, "{seed}: {closest}");
            // It faces the way it flies, level.
            let ((a, yaw), (b, _)) = (p.plane(1010).unwrap(), p.plane(1011).unwrap());
            let d = looks(yaw, 0.0);
            let step = [b[0] - a[0], b[2] - a[2]];
            assert!(near(step[0].hypot(step[1]), 3.0, 1e-9) && near(d[0] * step[0] + d[2] * step[1], 3.0, 1e-6), "{seed}");
            // The bomb leaves the plane and falls.
            assert!(p.release >= 1000 && p.plane(p.release).is_some());
            assert!(p.bomb(p.release - 1).is_none());
            assert!(flat(p.bomb(p.release).unwrap(), p.plane(p.release).unwrap().0) < 1e-9);
            for k in 1..60 {
                let (now, before) = (p.bomb(p.release + k).unwrap(), p.bomb(p.release + k - 1).unwrap());
                assert!(now[1] < before[1], "{seed}: {k}");
            }
            let t = p.impact(64).unwrap();
            let at = p.bomb(t).unwrap();
            assert!(flat(at, beacon) <= 4.0 && at[1] <= 64.0, "{seed}: {at:?}");
            assert!(p.bomb(t - 1).unwrap()[1] > 64.0);
            // Lower ground is met later.
            assert!(p.impact(40).unwrap() > t);
        }
        assert!(ways.len() > 3, "the planes come from different ways: {ways:?}");
    }

    #[test]
    fn a_plane_leaves_and_is_gone() {
        let beacon = [0.5, 64.0, 0.5];
        for seed in [4, 5, 6] {
            let p = Pass::new([0, 64, 0], 0, seed);
            let last = (0..1000).filter(|t| p.plane(*t).is_some()).max().unwrap();
            assert!((0..=last).all(|t| p.plane(t).is_some()));
            let ((start, _), (end, _)) = (p.plane(0).unwrap(), p.plane(last).unwrap());
            // Out on the other side of the beacon, not yet 150 blocks past it.
            assert!((start[0] - 0.5) * (end[0] - 0.5) + (start[2] - 0.5) * (end[2] - 0.5) < 0.0);
            assert!(flat(end, beacon) < 150.0 && flat(end, beacon) > 140.0, "{end:?}");
            assert!(p.plane(last + 1).is_none() && p.plane(last + 1000).is_none());
        }
    }

    // ---- the silo's blocks and items (Task 2) -------------------------------------------

    fn read(path: &str) -> String {
        std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn json(path: &str) -> serde_json::Value {
        serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn grid_of(stacks: &[(&'static str, u32)]) -> Vec<Option<(&'static str, u32)>> {
        let mut g: Vec<Option<(&str, u32)>> = stacks.iter().map(|s| Some(*s)).collect();
        g.resize(GRID_SLOTS, None);
        g
    }

    fn one(_: &str) -> u32 {
        1
    }

    #[test]
    fn the_missile_is_assembled_from_two_casings_three_iron_blocks_and_four_blaze_powder() {
        let parts = [("big_nukes:bomb_casing", 2), ("minecraft:iron_block", 3), ("minecraft:blaze_powder", 4)];
        let grid = grid_of(&parts);
        assert_eq!(assembly_in(&grid).map(|i| ASSEMBLIES[i].result), Some(MISSILE));
        let made = assemble(&grid, None, &one).unwrap();
        assert_eq!(made.result, MISSILE);
        assert_eq!(made.take, vec![(0, 2), (1, 3), (2, 4)]);
        // Spread over the grid, the same.
        let spread = grid_of(&[("minecraft:blaze_powder", 2), ("big_nukes:bomb_casing", 1), ("minecraft:iron_block", 3), ("minecraft:blaze_powder", 2), ("big_nukes:bomb_casing", 1)]);
        assert_eq!(assemble(&spread, None, &one).map(|a| a.result), Some(MISSILE));
        // One short, or a missile already in the output (they don't stack): nothing.
        assert!(assemble(&grid_of(&[parts[0], parts[1], ("minecraft:blaze_powder", 3)]), None, &one).is_none());
        assert!(assemble(&grid, Some((MISSILE, 1)), &one).is_none());
        assert_eq!(ITEMS.iter().find(|i| i.0 == MISSILE).map(|i| i.1), Some(1));
        assert_eq!(name_key(MISSILE), "item.big_nukes.missile");
    }

    #[test]
    fn the_beacon_is_assembled_from_iron_redstone_and_a_compass() {
        let grid = grid_of(&[("minecraft:iron_ingot", 4), ("minecraft:redstone", 4), ("minecraft:compass", 1)]);
        assert_eq!(assemble(&grid, None, &|_| 64).map(|a| a.result), Some(STRIKE_BEACON));
        assert!(assemble(&grid_of(&[("minecraft:iron_ingot", 4), ("minecraft:redstone", 4)]), None, &|_| 64).is_none());
        // A block: its name is a block's.
        assert_eq!(name_key(STRIKE_BEACON), "block.big_nukes.strike_beacon");
        for b in SILO_BLOCKS {
            assert!(name_key(b).starts_with("block."), "{b}");
        }
    }

    #[test]
    fn a_pad_takes_a_missile_and_a_tactical_or_atomic_warhead_only() {
        let (atomic, tactical, demolition) = (BombKind::Atomic.id(), BombKind::Tactical.id(), BombKind::Demolition.id());
        assert!(pad_takes(PAD_MISSILE_SLOT, MISSILE));
        assert!(pad_takes(PAD_WARHEAD_SLOT, atomic) && pad_takes(PAD_WARHEAD_SLOT, tactical));
        for item in [atomic, tactical, demolition, "minecraft:stone"] {
            assert!(!pad_takes(PAD_MISSILE_SLOT, item), "{item}");
        }
        for item in [MISSILE, demolition, "minecraft:tnt"] {
            assert!(!pad_takes(PAD_WARHEAD_SLOT, item), "{item}");
        }
        assert!(!pad_takes(PAD_SLOTS, MISSILE));
        // What it takes is what it launches.
        for missile in [MISSILE, "minecraft:stone"] {
            for warhead in [atomic, tactical, demolition, "minecraft:stone"] {
                let takes = pad_takes(PAD_MISSILE_SLOT, missile) && pad_takes(PAD_WARHEAD_SLOT, warhead);
                assert_eq!(pad_load(Some(missile), Some(warhead)).is_ok(), takes, "{missile} {warhead}");
            }
        }
        // Its menu, named as the block, and the window's slot rules, which can't reach the rules and name them.
        let menu = json(&format!("content/data/big_nukes/horizon/menu/{}.json", LAUNCH_PAD.strip_prefix("big_nukes:").unwrap()));
        assert_eq!(menu["slots"], PAD_SLOTS);
        let slot_rules = read("mixins/big_nukes_native/slot_rules.rs");
        for id in [LAUNCH_PAD, MISSILE, atomic, tactical] {
            assert!(slot_rules.contains(&format!("\"{id}\"")), "slot_rules.rs doesn't name {id}");
        }
    }

    #[test]
    fn the_flying_models_are_items_of_their_own_out_of_the_tab() {
        let base = "content/assets/big_nukes";
        let tab = read("content/data/big_nukes/horizon/creative_tab/big_nukes.json");
        let en = json(&format!("{base}/lang/en_us.json"));
        for (id, scale, long, wide) in [(MISSILE_MODEL, MISSILE_SCALE, 6.0, 1.0), (BOMBER_MODEL, BOMBER_SCALE, 12.0, 12.0)] {
            let n = id.strip_prefix("big_nukes:").unwrap();
            assert!(!tab.contains(&format!("\"{id}\"")), "{id} is in the tab");
            assert!(!ITEMS.iter().any(|i| i.0 == id) && MODEL_ITEMS.contains(&id));
            assert!(en[format!("item.big_nukes.{n}")].is_string(), "{id} has no name");
            let item = json(&format!("{base}/items/{n}.json"));
            let model = json(&format!("{base}/models/{}.json", item["model"]["model"].as_str().unwrap().strip_prefix("big_nukes:").unwrap()));
            let elements = model["elements"].as_array().unwrap_or_else(|| panic!("{id} has no elements"));
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for e in elements {
                for i in 0..3 {
                    let (a, b) = (e["from"][i].as_f64().unwrap(), e["to"][i].as_f64().unwrap());
                    assert!((-16.0..=32.0).contains(&a) && (-16.0..=32.0).contains(&b) && a < b, "{id}: {e}");
                    lo[i] = lo[i].min(a);
                    hi[i] = hi[i].max(b);
                }
                for face in e["faces"].as_object().unwrap().values() {
                    assert_eq!(face["texture"], "#t", "{id}");
                }
            }
            // Along z, as long and wide as the plan has it at the display's scale.
            let blocks = |i: usize| (hi[i] - lo[i]) * f64::from(scale) / 16.0;
            assert!((blocks(2) - long).abs() <= 0.5, "{id}: {} long", blocks(2));
            assert!((blocks(0) - wide).abs() <= 0.5 || id == MISSILE_MODEL && blocks(0) <= 2.0, "{id}: {} wide", blocks(0));
            let t = model["textures"]["t"].as_str().unwrap().strip_prefix("big_nukes:").unwrap();
            assert!(std::path::Path::new(&format!("{}/{base}/textures/{t}.png", env!("CARGO_MANIFEST_DIR"))).exists(), "{t}");
            if id == MISSILE_MODEL {
                // The display draws its item half a turn about y: the nose, at -z in the model, comes out along its
                // look, and the tail, at +z, at its place once the translation moves it forward.
                assert!(((8.0 - hi[2]) / 16.0 * f64::from(scale) + f64::from(MISSILE_LIFT)).abs() < 0.01, "{}", hi[2]);
                let nose = elements.iter().map(|e| e["from"][2].as_f64().unwrap()).fold(f64::MAX, f64::min);
                let tip = elements.iter().find(|e| e["from"][2].as_f64() == Some(nose)).unwrap();
                assert_eq!(tip["faces"]["north"]["uv"], serde_json::json!([8, 0, 16, 8]), "the red tip leads");
            }
        }
    }

    // ---- the console (Task 3) -------------------------------------------------------------

    #[test]
    fn the_console_screen_edits_its_three_fields() {
        let mut i = ConsoleInput::default();
        assert_eq!(i.focus, FIELD_X);
        for c in "-12a3".chars() {
            i.typed(c);
        }
        assert_eq!(i.fields[FIELD_X], "-123");
        assert!(!i.typed('-'), "a minus only first");
        for _ in 0..9 {
            i.typed('5');
        }
        assert_eq!(i.fields[FIELD_X], "-1235555", "8 characters at most");
        assert_eq!(i.key("key.keyboard.tab"), Keyed::Edited);
        assert_eq!(i.focus, FIELD_Z);
        i.typed('-');
        i.typed('7');
        assert_eq!(i.fields[FIELD_Z], "-7");
        i.key("key.keyboard.tab");
        assert_eq!(i.focus, FIELD_CODE);
        assert!(!i.typed('-'), "the code is digits");
        for c in "123456789".chars() {
            i.typed(c);
        }
        assert_eq!(i.fields[FIELD_CODE], "12345678");
        i.key("key.keyboard.tab");
        assert_eq!(i.focus, FIELD_X, "Tab goes round");
        assert_eq!(i.key("key.keyboard.backspace"), Keyed::Edited);
        assert_eq!(i.fields[FIELD_X], "-123555");
        assert_eq!(i.key("key.keyboard.enter"), Keyed::Launch);
        assert_eq!(i.key("key.keyboard.keypad.enter"), Keyed::Launch);
        assert_eq!(i.key("key.keyboard.e"), Keyed::Other);
        assert_eq!(i.fields[FIELD_X], "-123555");
        i.clear();
        assert_eq!(i.fields[FIELD_X], "");
        assert_eq!(i.key("key.keyboard.backspace"), Keyed::Edited, "nothing to erase is still the console's key");
        i.set_focus(FIELD_CODE);
        i.set_focus(7);
        assert_eq!(i.focus, FIELD_CODE);
        let m = i.request([1, 2, 3], false);
        assert_eq!(m, LaunchMessage { console: [1, 2, 3], x: String::new(), z: "-7".into(), code: "12345678".into(), camera: false });
    }

    #[test]
    fn the_launch_message_reads_back() {
        let m = LaunchMessage { console: [10, -64, -3], x: "-1500".into(), z: "20".into(), code: "1234".into(), camera: true };
        assert_eq!(read_launch(&m.bytes()), Some(m.clone()));
        let text = String::from_utf8(m.bytes()).unwrap();
        assert!(text.contains("\"console\":[10,-64,-3]") && text.contains("\"camera\":true"), "{text}");
        for broken in [
            &b"{"[..],
            b"",
            b"null",
            br#"{"console":[1,2,3],"x":"1","z":"2","code":"123"}"#,
            br#"{"console":[1,2],"x":"1","z":"2","code":"123","camera":true}"#,
            br#"{"console":[1,2,3],"x":"1","z":"2","code":"123","camera":true,"more":1}"#,
            br#"{"console":[1,2,3],"x":"123456789012345678","z":"2","code":"123","camera":true}"#,
            br#"{"console":[1,2,3],"x":1,"z":"2","code":"123","camera":true}"#,
        ] {
            assert_eq!(read_launch(broken), None, "{}", String::from_utf8_lossy(broken));
        }
        // The window's news of its console reads back too.
        let news = ConsoleNews { at: Some([1, 2, 3]), reply: Some(Reply::new("big_nukes.console.ready", &[])), ..ConsoleNews::default() };
        let back: ConsoleNews = serde_json::from_slice(&serde_json::to_vec(&news).unwrap()).unwrap();
        assert_eq!(back, news);
    }

    #[test]
    fn the_console_shows_its_state() {
        let states = [
            ConsoleState::Ready,
            ConsoleState::Shaft(ShaftProblem::NoPad),
            ConsoleState::Shaft(ShaftProblem::LowRoom(4)),
            ConsoleState::Shaft(ShaftProblem::NoHatch),
            ConsoleState::Shaft(ShaftProblem::NoSky),
            ConsoleState::Shaft(ShaftProblem::NoMissile),
            ConsoleState::Shaft(ShaftProblem::NoWarhead),
            ConsoleState::Countdown(3),
            ConsoleState::Flying,
            ConsoleState::Locked(25),
        ];
        for s in states {
            let data = s.data();
            assert_eq!(data.len(), CONSOLE_DATA);
            assert!(data.iter().all(|v| (0..=i32::from(i16::MAX)).contains(v)), "{s:?}: {data:?}");
            assert_eq!(ConsoleState::of_data(&data), Some(s), "{data:?}");
        }
        assert_eq!(ConsoleState::of_data(&[]), None);
        assert_eq!(ConsoleState::of_data(&[99, 0, 0]), None);
        // Seconds, rounded up, from the ticks.
        assert_eq!(ConsoleState::counting(41).data()[1], 3);
        assert_eq!(ConsoleState::locked(1).data()[1], 1);
        let menu = json("content/data/big_nukes/horizon/menu/console.json");
        assert_eq!((menu["slots"].as_u64(), menu["data_slots"].as_u64()), (Some(1), Some(CONSOLE_DATA as u64)));
        // Every line it shows or answers is in both languages, with its arguments.
        let (en, ru) = (json("content/assets/big_nukes/lang/en_us.json"), json("content/assets/big_nukes/lang/ru_ru.json"));
        let mut lines: Vec<Reply> = states.iter().map(ConsoleState::line).collect();
        lines.extend(
            [
                LaunchRefusal::Empty,
                LaunchRefusal::NotANumber,
                LaunchRefusal::CodeLength,
                LaunchRefusal::OutsideWorld,
                LaunchRefusal::TooNear(50),
                LaunchRefusal::TooFar(5000),
                LaunchRefusal::WrongCode(2),
                LaunchRefusal::Locked(30),
                LaunchRefusal::Maxpower,
            ]
            .map(|r| r.line()),
        );
        for l in lines {
            for lang in [&en, &ru] {
                let text = lang[&l.key].as_str().unwrap_or_else(|| panic!("{} isn't there", l.key));
                assert_eq!(text.matches("%s").count(), l.args.len(), "{}", l.key);
            }
        }
    }

    #[test]
    fn a_launch_is_recorded_within_maxpower() {
        let mut saved = Saved { tick: 500, maxpower: 50, ..Saved::default() };
        assert_eq!(saved.launch(OVERWORLD, [0, 65, 0], [1000, 0], BombKind::Atomic, 72), Err(LaunchRefusal::Maxpower));
        assert!(saved.flights.is_empty());
        let f = saved.launch(OVERWORLD, [0, 65, 0], [1000, 0], BombKind::Tactical, 72).unwrap();
        assert_eq!(saved.flights, vec![f.clone()]);
        assert_eq!((f.start, f.from, f.to, f.kind, f.carrier, f.surface, f.hatch), (500, [0, 65, 0], [1000, 0], BombKind::Tactical, Carrier::Missile, None, Some(72)));
        let g = saved.launch(OVERWORLD, [0, 65, 0], [-1000, 0], BombKind::Tactical, 72).unwrap();
        assert!(g.id != f.id && g.seed != f.seed);
        // The pad is busy until its missile is out of the shaft.
        assert!(saved.pad_busy(OVERWORLD, [0, 65, 0], 500 + LAUNCH_COUNTDOWN + ASCENT_TICKS - 1));
        assert!(!saved.pad_busy(OVERWORLD, [0, 65, 0], 500 + LAUNCH_COUNTDOWN + ASCENT_TICKS));
        assert!(!saved.pad_busy("minecraft:the_nether", [0, 65, 0], 500));
        assert_eq!(saved.launching(OVERWORLD, [0, 65, 0], 510).map(|f| f.id), Some(f.id));
    }

    // ---- the flight in the world (Task 4) ---------------------------------------------------

    #[test]
    fn a_target_stays_loaded_until_its_wave_works_its_center() {
        let mut saved = Saved::default();
        assert!(saved.force(1, OVERWORLD, [62, 0]), "asked for once");
        assert!(!saved.force(1, OVERWORLD, [62, 0]));
        assert!(saved.unforce().is_empty(), "still on its way");
        saved.fire(OVERWORLD, [1000, 70, 0], 250).unwrap();
        saved.landed(1, Some([1000, 70, 0]));
        assert!(saved.unforce().is_empty(), "the front hasn't reached the center's chunk");
        saved.waves.advance(OVERWORLD, saved.tick, &|_| true);
        assert!(saved.unforce().is_empty(), "the center's chunk waits for its turn");
        let (_, chunk) = saved.waves.next_job(OVERWORLD).unwrap();
        assert_eq!(chunk, [62, 0]);
        assert_eq!(saved.unforce(), vec![(OVERWORLD.to_owned(), [62, 0])]);
        assert!(saved.forced.is_empty() && saved.unforce().is_empty());
        // A center deferred for its load keeps the chunk forced.
        saved.force(4, OVERWORLD, [-10, -10]);
        saved.fire(OVERWORLD, [-155, 64, -155], 50).unwrap();
        saved.landed(4, Some([-155, 64, -155]));
        saved.waves.advance(OVERWORLD, saved.tick, &|_| false);
        assert!(saved.unforce().is_empty());
        // Two flights on one chunk: it goes with the last of them, and a flight that never went off lets go at once.
        saved.force(2, OVERWORLD, [5, 5]);
        saved.force(3, OVERWORLD, [5, 5]);
        saved.landed(2, None);
        assert!(saved.unforce().is_empty());
        assert_eq!(saved.forced.iter().filter(|f| f.chunk == [5, 5]).count(), 1);
        saved.landed(3, None);
        assert_eq!(saved.unforce(), vec![(OVERWORLD.to_owned(), [5, 5])]);
        // Kept with the world.
        let back: Saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        assert_eq!(back.forced, saved.forced);
        assert_eq!(saved.forced.len(), 1);
    }

    #[test]
    fn a_flight_whose_target_never_loads_lands_on_a_guess() {
        let f = Flight { surface: None, ..missile() };
        assert!(!f.surface_overdue(f.descends_at() + SURFACE_WAIT - 1));
        assert!(f.surface_overdue(f.descends_at() + SURFACE_WAIT));
        let guessed = Flight { surface: Some(SURFACE_GUESS), ..f.clone() };
        assert_eq!(guessed.leg(guessed.descends_at() + SURFACE_WAIT), Leg::Impact { at: [1000, SURFACE_GUESS, 0] });
        // The chunk the target is in.
        assert_eq!(f.chunk_to(), [62, 0]);
    }

    // ---- the launch camera (Task 5) -----------------------------------------------------------

    fn toward(from: [f64; 3], to: [f64; 3], yaw: f32, pitch: f32) -> f64 {
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let l = looks(yaw, pitch);
        (d[0] * l[0] + d[1] * l[1] + d[2] * l[2]) / len
    }

    #[test]
    fn the_launch_camera_watches_the_hatch_then_follows_up_to_two_hundred_and_lets_go() {
        let f = missile();
        let hatch = [0.5, 71.5, 0.5];
        // The player stands east of the shaft: the camera watches from their side.
        let eye = [10.0, 66.6, 0.5];
        for now in [0.0, 30.0, 60.0, 99.5] {
            let (pos, yaw, pitch) = launch_camera(&f, now, eye).unwrap_or_else(|| panic!("{now}"));
            assert!(near(flat(pos, hatch), 16.0, 0.01) && near(pos[1], hatch[1] + 6.0, 0.01), "{now}: {pos:?}");
            assert!(pos[0] > 10.0, "{now}: {pos:?}");
            assert!(toward(pos, hatch, yaw, pitch) > 0.999, "{now}");
        }
        // The climb: below and beside the missile, looking at it.
        for now in [100.0, 130.0, 150.5, 175.0] {
            let (tail, nose_yaw, nose_pitch) = f.climb(now - 100.0);
            let middle = [0, 1, 2].map(|i| tail[i] + looks(nose_yaw, nose_pitch)[i] * 3.0);
            let (pos, yaw, pitch) = launch_camera(&f, now, eye).unwrap_or_else(|| panic!("{now}"));
            assert!(toward(pos, middle, yaw, pitch) > 0.999, "{now}");
            assert!(flat(pos, tail) >= 11.9 && flat(pos, tail) <= 16.01, "{now}: {pos:?}");
            assert!(pos[1] >= hatch[1] + 5.99, "never under the ground: {now}");
        }
        let (tail, _, _) = f.climb(75.0);
        let (pos, _, _) = launch_camera(&f, 175.0, eye).unwrap();
        assert!(near(pos[1], tail[1] - 20.0, 0.01) && near(flat(pos, tail), 12.0, 0.01), "{pos:?} {tail:?}");
        // Two hundred blocks up it lets go, and the view is the player's again.
        let up = (100..220).find(|t| f.climb(f64::from(*t) - 100.0).0[1] > 64.0 + 200.0).unwrap();
        assert!(launch_camera(&f, f64::from(up) - 1.0, eye).is_some());
        assert!(launch_camera(&f, f64::from(up), eye).is_none());
        assert!(launch_camera(&f, 5000.0, eye).is_none());
        assert!(launch_camera(&f, -1.0, eye).is_none());
        // A player straight over the hatch still gets a view; a shut hatch is looked for where a shaft has one.
        assert!(launch_camera(&f, 10.0, [0.5, 90.0, 0.5]).is_some());
        let shut = Flight { hatch: None, ..missile() };
        let (pos, _, _) = launch_camera(&shut, 10.0, eye).unwrap();
        assert!(near(pos[1], 64.0 + f64::from(SHAFT_ROOM) + 0.5 + 6.0, 0.01), "{pos:?}");
    }

    // ---- the bomber (Task 6) ------------------------------------------------------------------

    #[test]
    fn a_beacon_takes_a_tactical_or_atomic_bomb_once() {
        let (atomic, tactical, demolition) = (BombKind::Atomic.id(), BombKind::Tactical.id(), BombKind::Demolition.id());
        let mut saved = Saved { tick: 40, ..Saved::default() };
        let b = [10, 64, 10];
        assert_eq!(saved.charge(OVERWORLD, b, Some("minecraft:stick")), Charge::Not);
        assert_eq!(saved.charge(OVERWORLD, b, None), Charge::Not);
        assert_eq!(saved.charge(OVERWORLD, b, Some(demolition)), Charge::Not);
        assert!(saved.beacons.is_empty());
        assert_eq!(saved.charge(OVERWORLD, b, Some(tactical)), Charge::Taken(BombKind::Tactical));
        assert_eq!(saved.charge(OVERWORLD, b, Some(atomic)), Charge::Busy);
        assert_eq!(saved.charge(OVERWORLD, b, Some("minecraft:stick")), Charge::Busy);
        assert_eq!(saved.beacons.len(), 1);
        let f = saved.beacon_at(OVERWORLD, b).unwrap().clone();
        assert_eq!((f.carrier, f.from, f.to, f.start, f.kind), (Carrier::Bomber, b, [10, 10], 40, BombKind::Tactical));
        // The bomber comes STRIKE_COUNTDOWN later.
        let pass = f.pass();
        assert!(pass.plane(40 + STRIKE_COUNTDOWN - 1).is_none() && pass.plane(40 + STRIKE_COUNTDOWN).is_some());
        assert_eq!(pass, Pass::new(b, 40 + STRIKE_COUNTDOWN, f.seed));
        // Another beacon's place, or another dimension, is another beacon.
        assert_eq!(saved.charge("minecraft:the_nether", b, Some(atomic)), Charge::Taken(BombKind::Atomic));
        assert!(saved.beacons.iter().all(|g| g.id == f.id || g.seed != f.seed));
        // A beacon broken before the release gives its bomb back and calls the bomber off; after, it comes anyway.
        assert_eq!(saved.uncharge(OVERWORLD, b), Some(BombKind::Tactical));
        assert!(saved.beacon_at(OVERWORLD, b).is_none() && saved.beacons.len() == 1);
        assert_eq!(saved.uncharge(OVERWORLD, b), None);
        assert_eq!(saved.charge(OVERWORLD, b, Some(tactical)), Charge::Taken(BombKind::Tactical));
        let id = saved.beacon_at(OVERWORLD, b).unwrap().id;
        assert!(saved.emptied(id));
        assert_eq!(saved.uncharge(OVERWORLD, b), None);
        assert!(saved.beacon_at(OVERWORLD, b).is_some());
    }

    #[test]
    fn the_strike_is_called_off_until_its_bomb_has_left_the_beacon() {
        let mut saved = Saved { tick: 40, ..Saved::default() };
        let b = [10, 64, 10];
        assert_eq!(saved.charge(OVERWORLD, b, Some(BombKind::Atomic.id())), Charge::Taken(BombKind::Atomic));
        let f = saved.beacon_at(OVERWORLD, b).unwrap().clone();
        let release = f.pass().release;
        // The bomb is taken out of the beacon's slot from the release on, until that is done.
        assert!(!f.to_empty(release - 1));
        assert!(f.to_empty(release) && f.to_empty(release + 5));
        // A beacon broken the tick before the release and heard at it dropped its bomb: no bomb falls too.
        assert_eq!(saved.uncharge(OVERWORLD, b), Some(BombKind::Atomic));
        assert!(saved.beacons.is_empty());
        // Once the slot is empty, a broken beacon drops nothing and the bomb falls anyway.
        assert_eq!(saved.charge(OVERWORLD, b, Some(BombKind::Atomic.id())), Charge::Taken(BombKind::Atomic));
        let id = saved.beacon_at(OVERWORLD, b).unwrap().id;
        assert!(saved.emptied(id));
        assert!(!saved.emptied(id), "once");
        let f = saved.beacon_at(OVERWORLD, b).unwrap();
        assert!(!f.to_empty(f.pass().release + 1));
        assert_eq!(saved.uncharge(OVERWORLD, b), None);
        // It is kept with the world.
        let back: Saved = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert!(back.beacon_at(OVERWORLD, b).unwrap().emptied);
        // A missile is no beacon's.
        assert!(!missile().to_empty(u64::MAX / 2));
    }

    #[test]
    fn a_bomb_is_taken_from_the_hand_it_was_seen_in() {
        // Stacks as the game has them: the item, the count and its name, if renamed.
        let named = |n: u32| Some((BombKind::Atomic.id(), n, Some("Big One")));
        let plain = |n: u32| Some((BombKind::Atomic.id(), n, None));
        let hotbar = [plain(3), None, named(1), plain(1), named(2)];
        // The hand holds the renamed one: its own slot, not a plain one before it.
        assert_eq!(hand_slot(&hotbar, &named(1).unwrap()), Some(2));
        assert_eq!(hand_slot(&hotbar, &named(2).unwrap()), Some(4));
        assert_eq!(hand_slot(&hotbar, &plain(1).unwrap()), Some(3));
        // A stack the hotbar doesn't hold (it changed) is in no hand.
        assert_eq!(hand_slot(&hotbar, &named(5).unwrap()), None);
        assert_eq!(hand_slot::<(&str, u32, Option<&str>)>(&[], &named(1).unwrap()), None);
    }

    #[test]
    fn the_strike_goes_within_maxpower() {
        let mut saved = Saved { maxpower: 50, ..Saved::default() };
        let b = [0, 70, 0];
        assert_eq!(saved.charge(OVERWORLD, b, Some(BombKind::Atomic.id())), Charge::Maxpower { radius: 250, maxpower: 50 });
        assert!(saved.beacons.is_empty());
        assert_eq!(saved.charge(OVERWORLD, b, Some(BombKind::Tactical.id())), Charge::Taken(BombKind::Tactical));
        // Its blast keeps within the limit too, should the operator lower it on the way.
        saved.maxpower = 16;
        assert_eq!(saved.fire(OVERWORLD, b, BombKind::Tactical.radius()), Err(Refused::Maxpower { radius: 50, maxpower: 16 }));
    }
}

// ---- iodine and the decontaminator (plan 9) -------------------------------------------

#[cfg(test)]
mod decon_tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    const OW: &str = "minecraft:overworld";

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// A zone of peak 10 rad a second, 300 blocks across, from `start`, that doesn't fade.
    fn zone(center: [i32; 3], start: u64) -> Zone {
        Zone { dimension: OW.into(), center, radius: 300.0, peak: 10.0, start, half_life: u64::MAX, floor: 1.0 }
    }

    fn clean(center: [i32; 3], radius: u32, start: u64) -> Clean {
        Clean { dimension: OW.into(), center, radius, start }
    }

    // ---- iodine ----

    #[test]
    fn iodine_halves_the_dose() {
        assert!(close(shielded(0.0, true), IODINE_PROTECTION) && close(IODINE_PROTECTION, 0.5));
        let bare = dose_step(0.0, 10.0, shielded(0.0, false));
        let iodine = dose_step(0.0, 10.0, shielded(0.0, true));
        // The healing comes off after the background: what the background adds is halved.
        assert!(close(iodine + HEAL, (bare + HEAL) / 2.0), "{bare} {iodine}");
        assert_eq!(IODINE_TICKS, 6000);
        assert_eq!(iodine_command("Steve"), format!("effect give Steve {IODINE_EFFECT} 300 0 true"));
    }

    #[test]
    fn iodine_and_the_suit_combine() {
        let suit = protection(true, true, true, true);
        assert!(close(shielded(suit, true), 1.0 - (1.0 - suit) * (1.0 - IODINE_PROTECTION)));
        assert!(close(shielded(suit, true), 0.975));
        for s in [0.0, 0.15, 0.5, suit, 1.0, 1.5, -0.5] {
            for i in [false, true] {
                let p = shielded(s, i);
                assert!((0.0..=1.0).contains(&p), "{s} {i}: {p}");
                assert!(p >= s.clamp(0.0, 1.0), "{s} {i}: iodine never lets more in");
            }
        }
        assert_eq!(shielded(1.0, true), 1.0);
        assert_eq!(shielded(2.0, true), 1.0);
    }

    #[test]
    fn iodine_without_its_effect_does_nothing() {
        // The protection comes from the effect on the player, not from the tablet: a tablet eaten without the
        // assets half gives no effect (its command fails), and then the suit alone counts.
        for s in [0.0, 0.25, 0.95, 1.0] {
            assert_eq!(shielded(s, false), s);
        }
        assert_eq!(dose_step(50.0, 10.0, shielded(0.0, false)), dose_step(50.0, 10.0, 0.0));
        // The tablets' consume effect is the kind the hook hears, the effect's own id.
        let c = components(IODINE);
        let effects = c["minecraft:consumable"]["on_consume_effects"].as_array().unwrap().clone();
        assert_eq!(effects, [serde_json::json!({"type": IODINE_EFFECT})]);
    }

    // ---- the cleaned circles ----

    #[test]
    fn a_clean_takes_the_background_to_nothing_inside() {
        let z = zone([0, 64, 0], 100);
        let c = [clean([0, 64, 0], CLEAN_RADIUS, 200)];
        let bg = |pos: [f64; 3]| background(std::slice::from_ref(&z), &c, OW, pos, 1000);
        // Inside the circle, up and down too: a circle of columns.
        for pos in [[0.5, 64.5, 0.5], [30.5, 64.5, 0.5], [0.5, 200.0, 48.5], [-33.0, -10.0, -33.0]] {
            assert_eq!(bg(pos), 0.0, "{pos:?}");
            assert_eq!(clean_factor(&c, &z, OW, pos), 0.0, "{pos:?}");
        }
        // Past the edge the zone is whole.
        let far = [0.5, 64.5, 60.5];
        assert!(close(bg(far), z.at(far, 1000)) && bg(far) > 0.0);
        assert_eq!(clean_factor(&c, &z, OW, far), 1.0);
        // Another dimension's circle cleans nothing here, and a circle of radius 0 nothing at all.
        let nether = [Clean { dimension: "minecraft:the_nether".into(), ..c[0].clone() }];
        assert_eq!(clean_factor(&nether, &z, OW, [0.5, 64.5, 0.5]), 1.0);
        assert_eq!(clean_factor(&[clean([0, 64, 0], 0, 200)], &z, OW, [0.5, 64.5, 0.5]), 1.0);
        // Without circles the background is the zones' as before.
        assert!(close(background(std::slice::from_ref(&z), &[], OW, [0.5, 64.5, 0.5], 1000), 10.0));
    }

    #[test]
    fn its_edge_is_smooth() {
        let z = zone([0, 64, 0], 100);
        let c = [clean([0, 64, 0], 20, 200)];
        let f = |d: f64| clean_factor(&c, &z, OW, [0.5 + d, 64.5, 0.5]);
        assert_eq!(f(20.0), 0.0);
        assert_eq!(f(20.0 + CLEAN_EDGE), 1.0);
        assert!(f(24.0) > 0.0 && f(24.0) < 1.0);
        let mut last = 0.0;
        for i in 0..=160 {
            let d = 18.0 + i as f64 * 0.1;
            let v = f(d);
            assert!(v >= last, "{d}: {v} < {last}");
            assert!(v - last <= 0.025, "{d}: a step of {}", v - last);
            last = v;
        }
        // Block to block across the edge the background never jumps by more than a fifth.
        for d in 18..32 {
            assert!(f(d as f64 + 1.0) - f(d as f64) <= 0.2, "{d}");
        }
    }

    #[test]
    fn overlapping_cleans_take_the_lower_background() {
        let z = zone([0, 64, 0], 100);
        let c = [clean([0, 64, 0], 20, 200), clean([30, 64, 0], 20, 200)];
        // Each point counts the circle that cleans it most: the union, not the product.
        for x in [-30.0, -21.0, 0.0, 15.0, 24.0, 26.0, 30.0, 45.0, 52.0] {
            let pos = [x + 0.5, 64.5, 0.5];
            let a = clean_factor(&c[..1], &z, OW, pos);
            let b = clean_factor(&c[1..], &z, OW, pos);
            assert!(close(clean_factor(&c, &z, OW, pos), a.min(b)), "{x}");
        }
        // Between them, where the edges overlap, the lower of the two.
        let pos = [24.5, 64.5, 0.5];
        assert!(clean_factor(&c[..1], &z, OW, pos) > 0.0 && clean_factor(&c[1..], &z, OW, pos) == 0.0);
        assert_eq!(clean_factor(&c, &z, OW, pos), 0.0);
        let mid = [26.5, 64.5, 10.5];
        let both = background(std::slice::from_ref(&z), &c, OW, mid, 1000);
        let one = background(std::slice::from_ref(&z), &c[..1], OW, mid, 1000)
            .min(background(std::slice::from_ref(&z), &c[1..], OW, mid, 1000));
        assert!(close(both, one));
        // Two edges over each other are not cleaner than either.
        let edges = [clean([0, 64, 0], 20, 200), clean([0, 64, 56], 20, 200)];
        let between = [0.5, 64.5, 28.5];
        let each = clean_factor(&edges[..1], &z, OW, between);
        assert!(each > 0.0 && close(clean_factor(&edges, &z, OW, between), each));
    }

    #[test]
    fn a_clean_does_not_cover_a_later_blast() {
        let old = zone([0, 64, 0], 100);
        let new = zone([50, 64, 0], 5000);
        let c = [clean([0, 64, 0], CLEAN_RADIUS, 1000)];
        let zones = [old.clone(), new.clone()];
        let pos = [0.5, 64.5, 0.5];
        assert_eq!(clean_factor(&c, &old, OW, pos), 0.0);
        assert_eq!(clean_factor(&c, &new, OW, pos), 1.0);
        assert!(close(background(&zones, &c, OW, pos, 6000), new.at(pos, 6000)) && new.at(pos, 6000) > 0.0);
        // A zone as old as the circle is cleaned: the circle started with it there.
        assert_eq!(clean_factor(&c, &zone([0, 64, 0], 1000), OW, pos), 0.0);
        // The decontaminator starts again once a newer zone reaches its circle: its old circle stays for the old
        // zone, and the new one grows from nothing for both.
        let mut saved = Saved { tick: 1000, zones: vec![old.clone()], ..Saved::default() };
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 1), 1);
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 30), 30);
        assert_eq!(saved.cleans, [clean([0, 64, 0], 30, 1000)]);
        saved.tick = 2000;
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 31), 31, "the same circle grows");
        assert_eq!(saved.cleans, [clean([0, 64, 0], 31, 1000)]);
        // A zone far from it changes nothing.
        saved.zones.push(Zone { radius: 10.0, ..zone([500, 64, 0], 2500) });
        saved.tick = 3000;
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 31), 31);
        saved.zones.push(new.clone());
        saved.tick = 6000;
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 32), 0, "a new blast over it: from nothing again");
        assert_eq!(saved.cleans, [clean([0, 64, 0], 31, 1000), clean([0, 64, 0], 0, 6000)]);
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 1), 1);
        assert_eq!(saved.cleans[1], clean([0, 64, 0], 1, 6000));
        // Once the new circle is as large, the old one says nothing more and goes.
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 31), 31);
        assert_eq!(saved.cleans, [clean([0, 64, 0], 31, 6000)]);
        // A decontaminator placed again where one stood takes its circle on; elsewhere it starts its own.
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 0), 31);
        assert_eq!(saved.cleaned(OW, [5, 64, 0], 0), 0);
        assert_eq!(saved.cleans.len(), 1, "nothing is cleaned before it grows");
        assert_eq!(saved.cleaned("minecraft:the_nether", [0, 64, 0], 2), 2);
        assert_eq!(saved.cleans.len(), 2);
        // The block takes the world's circle on: a larger one as it is, a smaller one as a new start.
        let mut d = Decontaminator { radius: 20, grow: 7, next_column: 300, ..Decontaminator::default() };
        d.keep_circle(31);
        assert_eq!((d.radius, d.grow, d.next_column), (31, 7, 300));
        d.keep_circle(0);
        assert_eq!((d.radius, d.grow, d.next_column), (0, 0, 0));
    }

    #[test]
    fn circles_that_clean_nothing_go_with_the_last_zone_they_met() {
        let fading = |center: [i32; 3], start: u64| Zone { radius: 100.0, half_life: 1000, floor: 0.0, ..zone(center, start) };
        let mut saved = Saved { tick: 1000, zones: vec![fading([0, 64, 0], 0), zone([2000, 64, 0], 0)], ..Saved::default() };
        saved.cleans = vec![clean([0, 64, 0], 20, 500), clean([2000, 64, 0], 20, 500), clean([9000, 64, 0], 5, 500)];
        // While no zone goes, nothing is looked at.
        assert!(!saved.prune_zones(2000));
        assert_eq!(saved.cleans.len(), 3);
        // The fading zone goes: the circles no zone reaches go with it, the one over the lasting zone stays.
        assert!(saved.prune_zones(20_000));
        assert_eq!(saved.cleans, [clean([2000, 64, 0], 20, 500)]);
        // Its decontaminator, still there, makes its circle again as it was while no zone reaches it...
        saved.tick = 21_000;
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 20), 20);
        assert_eq!(saved.cleans[1], clean([0, 64, 0], 20, 21_000));
        // ...and starts from nothing if a zone came over it meanwhile: that zone is newer than its old circle.
        saved.cleans.remove(1);
        saved.zones.push(zone([30, 64, 0], 20_500));
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 20), 0);
        assert_eq!(saved.cleans.len(), 1);
        // One that has just grown its first block starts its circle anywhere.
        assert_eq!(saved.cleaned(OW, [0, 64, 0], 1), 1);
        assert_eq!(saved.cleans[1], clean([0, 64, 0], 1, 21_000));
    }

    #[test]
    fn the_flame_goes_out_when_the_circle_is_full() {
        let mut d = Decontaminator { radius: CLEAN_RADIUS - 1, grow: CLEAN_GROW_TICKS - 1, burn: 300, burn_max: 400, next_column: 0 };
        assert!(d.step(Some(1600)).made);
        assert_eq!(d.radius, CLEAN_RADIUS);
        assert_eq!(d.data(0.0)[0], 0, "out as the cleaning ends");
        assert_eq!(flame_height(d.data(0.0)[0], d.data(0.0)[1]), 0);
        // One saved lit at its full circle goes out at its next tick, taking no fuel.
        let mut lit = Decontaminator { radius: CLEAN_RADIUS, burn: 300, burn_max: 400, ..Decontaminator::default() };
        assert_eq!(lit.step(Some(1600)), Step::IDLE);
        assert_eq!(lit.burn, 0);
        // Started again from nothing, it takes fuel.
        lit.keep_circle(0);
        assert!(lit.step(Some(1600)).took_fuel);
    }

    // ---- the decontaminator ----

    #[test]
    fn a_decontaminator_grows_a_block_every_forty_ticks_while_it_burns() {
        let coal = 1600;
        let mut d = Decontaminator::default();
        let first = d.step(Some(coal));
        assert!(first.took_fuel && !first.made);
        assert_eq!((d.burn_max, d.burn), (coal / CLEAN_FUEL_RATE, coal / CLEAN_FUEL_RATE - 1));
        let mut grew = 0;
        for t in 2..=400 {
            let s = d.step(Some(coal));
            assert!(!s.took_fuel, "{t}");
            if s.made {
                grew += 1;
                assert_eq!(t % CLEAN_GROW_TICKS, 0, "{t}");
            }
        }
        // One coal burns 400 ticks, four times as fast as in a furnace: 10 blocks.
        assert_eq!((grew, d.radius, d.burn), (10, 10, 0));
        assert!(d.step(Some(coal)).took_fuel);
        // Its screen: the fire, its radius and the background in tenths.
        assert_eq!(d.data(1.26), [d.burn as i32, d.burn_max as i32, 10, 13]);
        assert_eq!(d.data(1e9)[3], i16::MAX as i32);
        assert_eq!(d.data(-1.0)[3], 0);
        assert_eq!(radius_bar(0), 0);
        assert_eq!(radius_bar(CLEAN_RADIUS as i32), RADIUS_BAR);
        assert_eq!(radius_bar(99), RADIUS_BAR);
        assert!(radius_bar(1) >= 1);
    }

    #[test]
    fn it_stops_growing_at_forty_eight() {
        let mut d = Decontaminator::default();
        for _ in 0..100_000 {
            d.step(Some(1600));
        }
        assert_eq!(d.radius, CLEAN_RADIUS);
        assert_eq!(CLEAN_RADIUS, 48);
        // Full, it burns no more fuel.
        let burning = d.burn;
        assert_eq!(d.step(Some(1600)), Step::IDLE);
        assert_eq!(d.burn, burning);
    }

    #[test]
    fn without_fuel_it_keeps_what_it_cleaned() {
        let mut d = Decontaminator::default();
        for _ in 0..(5 * CLEAN_GROW_TICKS) {
            d.step(Some(1600));
        }
        d.burn = 0;
        let r = d.radius;
        assert_eq!(r, 5);
        for _ in 0..10_000 {
            assert_eq!(d.step(None), Step::IDLE);
            assert_eq!(d.step(Some(0)), Step::IDLE);
        }
        assert_eq!(d.radius, r);
        // A saved decontaminator reads back; one saved by an older version too.
        let back: Decontaminator = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
        assert_eq!(serde_json::from_str::<Decontaminator>("{}").unwrap(), Decontaminator::default());
    }

    #[test]
    fn the_spiral_covers_each_column_of_a_ring_once() {
        for k in 0..=50u32 {
            let ring: Vec<[i32; 2]> = spiral(k, k).collect();
            let set: BTreeSet<[i32; 2]> = ring.iter().copied().collect();
            assert_eq!(set.len(), ring.len(), "{k}: a column twice");
            let k = k as i32;
            let want: BTreeSet<[i32; 2]> = (-k..=k)
                .flat_map(|x| (-k..=k).map(move |z| [x, z]))
                .filter(|&[x, z]| x * x + z * z <= k * k && (k == 0 || x * x + z * z > (k - 1) * (k - 1)))
                .collect();
            assert_eq!(set, want, "{k}");
        }
        // Outward, ring by ring: the disc of 48 is every column within 48 of the center.
        let disc: Vec<[i32; 2]> = spiral(0, CLEAN_RADIUS).collect();
        assert_eq!(disc[0], [0, 0]);
        let d2 = |c: &[i32; 2]| c[0] * c[0] + c[1] * c[1];
        let ring = |c: &[i32; 2]| (d2(c) as f64).sqrt().ceil() as i32;
        assert!(disc.windows(2).all(|w| ring(&w[0]) <= ring(&w[1])));
        let r = CLEAN_RADIUS as i32;
        assert_eq!(disc.len(), (-r..=r).flat_map(|x| (-r..=r).map(move |z| [x, z])).filter(|c| d2(c) <= r * r).count());
        assert_eq!(spiral(3, 5).collect::<Vec<_>>(), spiral(0, 5).skip(spiral(0, 2).count()).collect::<Vec<_>>());
        assert_eq!(spiral(5, 3).count(), 0);
        // The passes go through the circle, CLEAN_BLOCKS_PER_PASS columns at a time, and no column twice until it grows.
        let mut d = Decontaminator { radius: 10, ..Decontaminator::default() };
        let mut seen = Vec::new();
        loop {
            let pass = d.regrow_pass();
            assert!(pass.len() <= CLEAN_BLOCKS_PER_PASS);
            if pass.is_empty() {
                break;
            }
            seen.extend(pass);
        }
        assert_eq!(seen, spiral(0, 10).collect::<Vec<_>>());
        assert!(d.regrow_pass().is_empty());
        d.radius = 11;
        assert_eq!(d.regrow_pass(), spiral(11, 11).take(CLEAN_BLOCKS_PER_PASS).collect::<Vec<_>>());
    }

    #[test]
    fn the_pass_finds_the_ash_under_snow_leaves_fire_and_plants() {
        // A column read top down: what the wave left over the ash (leaves of broken trees, snow on them, fire,
        // a plant) lets the pass through to it.
        let col = ["minecraft:snow[layers=1]", "minecraft:spruce_leaves[distance=2]", "minecraft:air", "minecraft:fire", ASH, "minecraft:dirt"];
        assert_eq!(ash_under(col.into_iter(), ASH), Some(4));
        assert_eq!(ash_under(["minecraft:short_grass", ASH].into_iter(), ASH), Some(1));
        assert_eq!(ash_under([ASH, ASH].into_iter(), ASH), Some(0), "the top one");
    }

    #[test]
    fn ash_under_water_or_a_roof_stays() {
        assert_eq!(ash_under(["minecraft:water[level=0]", ASH].into_iter(), ASH), None);
        assert_eq!(ash_under(["minecraft:stone", "minecraft:air", ASH].into_iter(), ASH), None);
        assert_eq!(ash_under(["minecraft:oak_planks", ASH].into_iter(), ASH), None);
        assert_eq!(ash_under(["minecraft:air", "minecraft:dirt"].into_iter(), ASH), None, "no ash");
        assert_eq!(ash_under(std::iter::empty(), ASH), None);
    }

    #[test]
    fn natural_coarse_dirt_stays_when_the_game_has_ash() {
        assert_eq!(regrow_target(true), ASH);
        assert_eq!(regrow_target(false), "minecraft:coarse_dirt", "it stands in for ash only without the assets half");
        assert_eq!(ash_under(["minecraft:coarse_dirt"].into_iter(), regrow_target(true)), None);
    }

    #[test]
    fn the_edit_turns_only_the_ash_it_was_given() {
        let e = regrow_edit(&[[3, 66, -4], [10, 70, 7]], true);
        assert_eq!(e.palette, ["minecraft:grass_block"]);
        assert_eq!(e.filters, [ASH], "only ash: a block changed since the read stays");
        assert_eq!(e.runs.len(), 2);
        for (r, [x, y, z]) in e.runs.iter().zip([[3, 66, -4], [10, 70, 7]]) {
            assert_eq!((r.x, r.y, r.z, r.height, r.surface, r.block, r.only), (x, y, z, 1, None, 0, Some(0)));
        }
        assert_eq!(regrow_edit(&[[0, 0, 0]], false).filters, ["minecraft:coarse_dirt"]);
        assert!(regrow_edit(&[], true).runs.is_empty());
    }

    #[test]
    fn cleans_survive_a_save_and_load() {
        let mut saved = Saved { tick: 77, ..Saved::default() };
        saved.cleaned(OW, [3, 70, -9], 12);
        saved.cleaned("minecraft:the_nether", [0, 40, 0], 48);
        let back: Saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        assert_eq!(back.cleans, saved.cleans);
        assert_eq!(back.cleans.len(), 2);
        // A world saved before the decontaminator has none.
        let mut old = serde_json::to_value(Saved::default()).unwrap();
        old.as_object_mut().unwrap().remove("cleans");
        let old: Saved = serde_json::from_value(old).unwrap();
        assert!(old.cleans.is_empty());
        // The window is told the circles of its dimension with the zones, and reckons the same background.
        saved.blast(OW, [0, 64, 0], 120).unwrap();
        saved.tick = 500;
        // It grows from its first block, as a decontaminator's does.
        saved.cleaned(OW, [0, 64, 0], 1);
        saved.cleaned(OW, [0, 64, 0], 48);
        let rad = saved.rad_for(1, OW, [0.0, 64.0, 0.0]);
        assert!(rad.cleans.iter().all(|c| c.dimension == OW) && rad.cleans.len() == 2);
        let back: Rad = serde_json::from_slice(&serde_json::to_vec(&rad).unwrap()).unwrap();
        assert_eq!(back, rad);
        let told = Told { rad, at: 0 };
        for pos in [[0.5, 64.5, 0.5], [40.5, 64.5, 30.5], [60.5, 64.5, 0.5]] {
            assert!(close(told.background(OW, pos, 0), background(&saved.zones, &saved.cleans, OW, pos, 500)), "{pos:?}");
        }
        assert_eq!(told.background(OW, [0.5, 64.5, 0.5], 0), 0.0);
        // An older server's message without circles still reads.
        let old: Rad = serde_json::from_str(r#"{"zones": [], "now": 3, "dose": 1.0}"#).unwrap();
        assert!(old.cleans.is_empty());
    }
    // ---- the items, the block and the guide (Task 2) ----

    fn file(path: &str) -> String {
        std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn json(path: &str) -> serde_json::Value {
        serde_json::from_str(&file(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn png_size(path: &str) -> (u32, u32) {
        let b = std::fs::read(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(&b[1..4], b"PNG", "{path}");
        let int = |at: usize| u32::from_be_bytes(b[at..at + 4].try_into().unwrap());
        (int(16), int(20))
    }

    fn language(code: &str) -> BTreeMap<String, String> {
        serde_json::from_str(&file(&format!("content/assets/big_nukes/lang/{code}.json"))).unwrap()
    }

    #[test]
    fn the_iodine_tablets_are_eaten_fast_and_made_on_the_crafting_table() {
        let item = ITEMS.iter().find(|i| i.0 == IODINE).expect("declared");
        assert_eq!((item.1, item.2), (16, false));
        assert_eq!(template(IODINE), "minecraft:paper");
        let eat = &components(IODINE)["minecraft:consumable"];
        assert_eq!(eat["consume_seconds"], 0.8, "16 ticks, as dried kelp");
        assert_eq!(eat["animation"], "eat");
        assert_eq!(eat["sound"], "minecraft:entity.generic.eat");
        let recipe = json("content/data/big_nukes/recipe/iodine_tablets.json");
        assert_eq!(recipe["type"], "minecraft:crafting_shapeless");
        let mut parts: Vec<&str> = recipe["ingredients"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        parts.sort();
        assert_eq!(parts, ["minecraft:dried_kelp", "minecraft:dried_kelp", "minecraft:dried_kelp", "minecraft:sugar"]);
        assert_eq!(recipe["result"], serde_json::json!({"id": IODINE, "count": 2}));
        // Its effect: a beneficial one with a name and an 18x18 icon.
        let (ns, name) = IODINE_EFFECT.split_once(':').unwrap();
        let effect = json(&format!("content/data/{ns}/horizon/mob_effect/{name}.json"));
        assert_eq!(effect["category"], "beneficial");
        assert!(effect["color"].as_u64().unwrap() <= 0xFF_FFFF);
        assert_eq!(effect.as_object().unwrap().len(), 2);
        let (en, ru) = (language("en_us"), language("ru_ru"));
        assert!(en.contains_key(&format!("effect.{ns}.{name}")) && ru.contains_key(&format!("effect.{ns}.{name}")));
        assert_eq!(png_size(&format!("content/assets/{ns}/textures/mob_effect/{name}.png")), (18, 18));
    }

    #[test]
    fn the_decontaminator_is_assembled_and_burns_from_one_slot() {
        let a = ASSEMBLIES.iter().find(|a| a.result == DECONTAMINATOR).expect("its recipe");
        assert_eq!(a.count, 1);
        let parts: BTreeMap<&str, u32> = a.parts.iter().copied().collect();
        assert_eq!(
            parts,
            BTreeMap::from([("minecraft:iron_block", 4), ("minecraft:water_bucket", 2), ("minecraft:sponge", 1), ("minecraft:redstone", 2)])
        );
        assert_eq!(name_key(DECONTAMINATOR), "block.big_nukes.decontaminator");
        // The water's buckets come back into the grid, as a crafting table gives them back.
        assert_eq!(remainder("minecraft:water_bucket"), Some("minecraft:bucket"));
        assert_eq!(remainder("minecraft:lava_bucket"), Some("minecraft:bucket"));
        assert_eq!(remainder("minecraft:iron_block"), None);
        let (ns, name) = DECONTAMINATOR.split_once(':').unwrap();
        let menu = json(&format!("content/data/{ns}/horizon/menu/{name}.json"));
        assert_eq!(menu["slots"], DECON_SLOTS);
        assert_eq!(menu["data_slots"], Decontaminator::default().data(0.0).len());
        assert_eq!(menu["results"], serde_json::json!([]));
        assert_eq!(DECON_FUEL_SLOT, 0);
        // Its screen: a furnace's panel, the lit flame right of it and the full bar under it.
        assert_eq!(png_size("content/assets/big_nukes/textures/gui/decontaminator.png"), (192, 176));
        assert!(RADIUS_BAR <= 176 - 2 * 8);
        let tab = json("content/data/big_nukes/horizon/creative_tab/big_nukes.json");
        let listed: Vec<&str> = tab["items"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        let at = |id: &str| listed.iter().position(|l| *l == id).unwrap();
        assert!(at(ANTI_RAD) < at(IODINE) && at(IODINE) < at(DECONTAMINATOR), "with the radiation's things");
    }

    #[test]
    fn the_radiation_chapter_tells_of_iodine_and_the_decontaminator() {
        let radiation = CHAPTERS.iter().find(|c| c.title == "big_nukes.guide.radiation").expect("the radiation's chapter");
        assert_eq!(radiation.icons, [GEIGER_COUNTER, HAZMAT[0].0, HAZMAT[1].0, HAZMAT[2].0, HAZMAT[3].0, ANTI_RAD, IODINE, DECONTAMINATOR]);
        let numbers = [
            ((IODINE_PROTECTION * 100.0) as u32).to_string(),
            (IODINE_TICKS / SECOND / 60).to_string(),
            CLEAN_FUEL_RATE.to_string(),
            (CLEAN_GROW_TICKS / SECOND as u32).to_string(),
            CLEAN_RADIUS.to_string(),
            (CLEAN_EDGE as u32).to_string(),
        ];
        let (en, ru) = (language("en_us"), language("ru_ru"));
        for lang in [&en, &ru] {
            let text = &lang[radiation.text];
            for n in &numbers {
                assert!(text.split(|c: char| !c.is_ascii_digit()).any(|w| w == n), "{n} in {text}");
            }
            // The paragraph is its own, after what was there.
            assert!(text.matches("\n\n").count() >= 4, "{text}");
        }
        // Its recipe is in the assembly chapter, as every assembly's is.
        assert!(CHAPTERS.iter().any(|c| c.recipes));
        // The chapter still fits the book: two rows of icons, and the book opens as before.
        let g = Guide::new(&|k: &str| en.get(k).cloned().unwrap_or_else(|| k.to_owned()), &|t: &str| 6.0 * t.chars().count() as f32);
        let i = CHAPTERS.iter().position(|c| c.title == radiation.title).unwrap();
        assert!(g.starts[i] < g.starts[i + 1]);
    }
}
