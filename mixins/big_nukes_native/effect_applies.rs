// Whether a content mod's effect acts this tick: the iodine's does, every tick.
fn big_nukes_effect_applies(game: &crate::game::Game<'_>, effect: &str, apply: &mut bool) {
    crate::game::big_nukes_decon::effect_applies(game, effect, apply);
}
