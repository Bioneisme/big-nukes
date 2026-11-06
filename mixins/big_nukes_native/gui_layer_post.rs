// After each of the HUD's layers: the Geiger counter's reading over the hotbar.
fn big_nukes_gui_layer_post(game: &crate::game::Game<'_>, layer: &str, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::big_nukes_glue::hud(game, layer, batch);
}
