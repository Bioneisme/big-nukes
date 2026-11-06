// A key of a bomb's keypad, pressed on its screen.
fn big_nukes_menu_button(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
    id: i32,
) {
    crate::game::big_nukes_glue::menu_button(game, pos, entity, id);
}
