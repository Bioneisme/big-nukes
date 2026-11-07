// A content mod's effect acts: the iodine's on a player shields their dose.
fn big_nukes_effect_tick(game: &crate::game::Game<'_>, effect: &str, entity: Option<i32>) {
    crate::game::big_nukes_decon::effect_tick(game, effect, entity);
}
