// A block changed, whoever changed it: a bomb with a receiver that is gone is forgotten.
fn big_nukes_block_changed(
    level: &crate::game::GameLevel<'_>,
    dimension: crate::game::LevelDimension,
    pos: horizon_core::position::BlockPos,
    old: horizon_world::registry::BlockStateId,
    new: horizon_world::registry::BlockStateId,
) {
    crate::game::big_nukes_glue::block_changed(level, dimension, pos, old, new);
}
