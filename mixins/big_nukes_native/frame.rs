// Every frame the window draws: the renderer's list of blasts, and the guide's page turns heard.
fn big_nukes_frame(game: &mut crate::game::Game<'_>, elapsed: std::time::Duration) {
    let _ = elapsed;
    crate::game::big_nukes_glue::frame(game);
    crate::game::big_nukes_glue::guide_frame(game);
}
