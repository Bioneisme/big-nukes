// The end of a living player's server tick: their dose takes the background where they stand.
fn big_nukes_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::big_nukes_glue::player_tick(game);
}
