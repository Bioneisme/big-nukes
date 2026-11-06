fn big_nukes_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::big_nukes_glue::block_entity(block, entity);
}
