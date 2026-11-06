// The mod's item components, registered before the content loads: the remote's link.
fn big_nukes_component_types(types: &mut Vec<String>) {
    crate::game::big_nukes_glue::component_types(types);
}
