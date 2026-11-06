// What a hopper may put into the centrifuge's slots.
fn big_nukes_container_accepts(
    game: &crate::game::Game<'_>,
    entity: &horizon_client::containers::ModBlockEntity,
    slot: usize,
    stack: &horizon_client::items::ItemStack,
    cancel: &mut bool,
) {
    crate::game::big_nukes_glue::accepts(game, entity, slot, stack, cancel);
}
