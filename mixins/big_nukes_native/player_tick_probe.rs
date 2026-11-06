// A probe for plan 5: the game ticks each living player.
fn big_nukes_player_tick_probe(game: &mut crate::game::Game<'_>) {
    crate::game::big_nukes_glue::player_tick_probe(game);
}
