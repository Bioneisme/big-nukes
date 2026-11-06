fn big_nukes_logged_in(game: &mut crate::game::Game<'_>, player: u128, name: &str) {
    crate::game::big_nukes_glue::logged_in(game, player, name);
}
