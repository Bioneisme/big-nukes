// The server stopping, its players still there: the state is saved, and they are told if that fails.
fn big_nukes_server_stopping(game: &mut crate::game::Game<'_>) {
    crate::game::big_nukes_glue::server_stopping(game);
}
