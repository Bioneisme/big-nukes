fn big_nukes_container(block: &str, container: &mut Option<horizon_client::containers::ModContainer>) {
    crate::game::big_nukes_glue::container(block, container);
}
