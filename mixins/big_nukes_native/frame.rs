// Every frame the window draws: the renderer's list of blasts, and the launches the console's screen queued.
fn big_nukes_frame(game: &mut crate::game::Game<'_>, elapsed: std::time::Duration) {
    let _ = elapsed;
    crate::game::big_nukes_glue::frame(game);
    crate::game::big_nukes_silo::frame(game);
}
