// The centrifuge's work and the bombs' countdowns, once a server tick.
fn big_nukes_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    crate::game::big_nukes_glue::tick(game, pos, entity);
}
