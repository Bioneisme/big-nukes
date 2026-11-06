// A right click on a block: on a bomb, a detonator goes in or a fuse is lit.
fn big_nukes_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::big_nukes_glue::right_click(game, pos, canceled);
}
