// The end of a level's tick: the waves of its dimension move on.
fn big_nukes_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::big_nukes_glue::level_tick(game, dimension);
}
