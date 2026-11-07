// A key over the guide: the arrows turn its pages.
fn big_nukes_guide_key(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    canceled: &mut bool,
) {
    crate::game::big_nukes_glue::guide_key(game, screen, key, canceled);
}
