// The centrifuge's work, once a server tick.
fn big_nukes_block_entity_tick(game: &mut crate::game::Game<'_>, entity: &mut horizon_client::containers::ModBlockEntity) {
    crate::game::big_nukes_glue::tick(game, entity);
}
