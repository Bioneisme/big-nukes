// A server tick of a stack a player carries: the held stack takes the mod's queued changes.
fn big_nukes_inventory_tick(game: &mut crate::game::Game<'_>, slot: i32, selected: bool) {
    crate::game::big_nukes_glue::inventory_tick(game, slot, selected);
}
