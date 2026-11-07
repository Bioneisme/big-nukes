// A key over a screen: the guide's arrows turn its pages; the launch console's Tab, Backspace and Enter, and no
// other key closes it.
fn big_nukes_screen_key(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    canceled: &mut bool,
) {
    crate::game::big_nukes_glue::guide_key(game, screen, key, canceled);
    crate::game::big_nukes_silo::key_pressed(game, screen, key, canceled);
}
