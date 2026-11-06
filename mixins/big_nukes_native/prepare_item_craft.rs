// A crafting grid's result, before it is shown: the bombs only on the assembly table.
fn big_nukes_prepare_item_craft(
    game: &mut crate::game::Game<'_>,
    player: u128,
    pos: Option<[i32; 3]>,
    result: &mut Option<horizon_client::items::ItemStack>,
) {
    crate::game::big_nukes_glue::prepare_item_craft(game, player, pos, result);
}
