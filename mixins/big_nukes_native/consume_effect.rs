// A consumable's effect of the mod's own kind: the anti-rad's.
fn big_nukes_consume_effect(game: &mut crate::game::Game<'_>, kind: &str, data: &serde_json::Value) {
    crate::game::big_nukes_glue::consume_effect(game, kind, data);
}
