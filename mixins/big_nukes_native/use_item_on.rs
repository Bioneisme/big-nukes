// A right click on a block with an item: a remote never fires at a block.
fn big_nukes_use_item_on(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    crate::game::big_nukes_glue::use_item_on(game, stack, claimed);
}
