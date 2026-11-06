// An item's tooltip: how to use it, and the bomb a remote is linked to.
fn big_nukes_tooltip(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    crate::game::big_nukes_glue::tooltip(game, stack, lines);
}
