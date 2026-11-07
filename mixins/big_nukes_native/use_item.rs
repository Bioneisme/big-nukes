// A right click with an item: a linked remote sets its bomb off, the guide opens.
fn big_nukes_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    crate::game::big_nukes_glue::use_item(game, stack, claimed);
}
