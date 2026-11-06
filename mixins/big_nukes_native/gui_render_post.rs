// Over everything on the screen, even under F1: the flash.
fn big_nukes_gui_render_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    gui_hidden: bool,
    focused: bool,
) {
    let _ = (gui_hidden, focused);
    crate::game::big_nukes_glue::flash(game, batch);
}
