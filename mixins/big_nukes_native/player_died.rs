// A player died: their dose goes.
fn big_nukes_player_died(game: &crate::game::Game<'_>, player: u128) {
    crate::game::big_nukes_glue::player_died(game, player);
}
