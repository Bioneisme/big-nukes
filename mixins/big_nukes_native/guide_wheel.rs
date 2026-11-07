// The wheel over the guide turns its pages.
fn big_nukes_guide_wheel(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    crate::game::big_nukes_glue::guide_wheel(game, screen, scroll, canceled);
}
