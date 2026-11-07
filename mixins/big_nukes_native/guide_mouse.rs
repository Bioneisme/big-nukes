// A mouse button over the guide: a bookmark or an arrow.
fn big_nukes_guide_mouse(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::big_nukes_glue::guide_mouse(game, screen, button, canceled);
}
