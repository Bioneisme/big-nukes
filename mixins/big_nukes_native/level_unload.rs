// The window drops a level (another dimension, or the world left): what it was told of the radiation goes.
fn big_nukes_client_level_unload(dimension: &str) {
    let _ = dimension;
    crate::game::big_nukes_glue::forget_rad();
}
