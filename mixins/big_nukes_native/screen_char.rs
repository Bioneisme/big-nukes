// A character typed at a screen: the launch console's fields take it.
fn big_nukes_screen_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    crate::game::big_nukes_silo::char_typed(game, screen, c, canceled);
}
