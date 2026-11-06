// The end of a level's tick: the waves of its dimension move on, and its mobs in the zones are hurt.
fn big_nukes_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::big_nukes_glue::level_tick(game, dimension);
}
