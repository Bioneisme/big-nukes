// Every tick of every game, the window's included: the Geiger counter clicks there.
fn big_nukes_tick(game: &mut crate::game::Game<'_>) {
    crate::game::big_nukes_glue::client_tick(game);
}
