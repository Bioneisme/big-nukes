// A menu button pressed on a machine, on the server: the assembly table's Assemble.
fn big_nukes_menu_button(game: &crate::game::Game<'_>, entity: &mut horizon_client::containers::ModBlockEntity, id: i32) {
    crate::game::big_nukes_glue::menu_button(game, entity, id);
}
