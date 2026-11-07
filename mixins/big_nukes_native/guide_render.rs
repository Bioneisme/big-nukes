// The guide's screen, over the world.
fn big_nukes_guide_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::big_nukes_glue::guide_render(game, batch, screen);
}
