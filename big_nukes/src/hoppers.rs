//! What hoppers (and pipes) put into the machines: only what each slot works. The old `container_accepts` hook is
//! gone; a provider of `ITEMS` asked ahead of the game's answers each face instead. It takes no item a slot can't
//! use and hands what it takes to the game's own handler, asked without a side, which this provider leaves to the
//! game: the game's handler writes the block entity's slots through its journal, so an aborted transfer is undone.

use crate::{machines, rules};
use mod_api::{
    BlockSide, Host, ItemResource, ModContainer, Registrar, ResourceHandler, ServerHost, Transaction, ITEMS,
};

/// Whether `item` may go into slot `slot` of block `block` from a hopper.
fn accepts(host: &dyn Host, block: &str, slot: usize, item: &ItemResource) -> bool {
    let stack = item.as_stack();
    match block {
        rules::CENTRIFUGE => match slot {
            rules::INPUT_SLOT => rules::enrichment(&stack.item().id(host)).is_some(),
            rules::FUEL_SLOT => machines::burn_time(host, stack) > 0,
            _ => false,
        },
        rules::FUEL_GENERATOR => slot == rules::GENERATOR_FUEL_SLOT && machines::burn_time(host, stack) > 0,
        rules::DECONTAMINATOR => slot == rules::DECON_FUEL_SLOT && machines::burn_time(host, stack) > 0,
        _ => true,
    }
}

/// The blocks whose hoppers are filtered.
const FILTERED: [&str; 3] = [rules::CENTRIFUGE, rules::FUEL_GENERATOR, rules::DECONTAMINATOR];

/// Registers the filter for each filtered block, with its container's faces.
pub(crate) fn register(r: &mut dyn Registrar, container: fn(&str) -> Option<ModContainer>) {
    for block in FILTERED {
        let Some(c) = container(block) else { continue };
        let key = r.block_key(block);
        r.block_capability_before_game(&ITEMS, &key, move |_, at, _, _| {
            // No side: the game's own handler, the one the filter writes through.
            let side = at.side?;
            let slots = c.faces[side as usize].clone();
            let handler: Box<dyn ResourceHandler<ItemResource>> = Box::new(Face {
                at,
                block,
                slots,
                inputs: c.inputs.clone(),
                outputs: c.outputs.clone(),
            });
            Some(handler)
        });
    }
}

/// One face of a filtered machine: the slots it reaches, by index.
struct Face {
    at: BlockSide,
    block: &'static str,
    slots: Vec<usize>,
    inputs: Vec<usize>,
    outputs: Vec<usize>,
}

impl Face {
    /// The game's handler of all the block's slots, asked without a side.
    fn all(&self, host: &mut dyn ServerHost) -> Option<mod_api::Capability<dyn ResourceHandler<ItemResource>>> {
        self.at.level.capability(host, &ITEMS, self.at.pos, None)
    }

    fn slot(&self, index: usize) -> Option<usize> {
        self.slots.get(index).copied()
    }
}

impl ResourceHandler<ItemResource> for Face {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        self.slots.len()
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        let slot = self.slot(index)?;
        self.all(host)?.resource(host, slot)
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        let Some(slot) = self.slot(index) else { return 0 };
        self.all(host).map_or(0, |all| all.amount(host, slot))
    }

    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> u32 {
        let Some(slot) = self.slot(index) else { return 0 };
        self.all(host).map_or(0, |all| all.capacity(host, slot, resource))
    }

    fn is_valid(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> bool {
        let Some(slot) = self.slot(index) else { return false };
        self.inputs.contains(&slot)
            && accepts(&*host, self.block, slot, resource)
            && self.capacity(host, index, resource) > 0
    }

    fn insert(&self, host: &mut dyn ServerHost, tx: &Transaction, index: usize, resource: &ItemResource, amount: u32) -> u32 {
        let Some(slot) = self.slot(index) else { return 0 };
        if !self.inputs.contains(&slot) || !accepts(&*host, self.block, slot, resource) {
            return 0;
        }
        self.all(host).map_or(0, |all| all.insert(host, tx, slot, resource, amount))
    }

    fn extract(&self, host: &mut dyn ServerHost, tx: &Transaction, index: usize, resource: &ItemResource, amount: u32) -> u32 {
        let Some(slot) = self.slot(index) else { return 0 };
        if !self.outputs.contains(&slot) {
            return 0;
        }
        self.all(host).map_or(0, |all| all.extract(host, tx, slot, resource, amount))
    }
}
