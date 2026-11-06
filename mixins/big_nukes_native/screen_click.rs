// A press on a machine's screen, over no slot: the assembly table's button or a keypad key.
fn big_nukes_screen_click(block: &str, at: [f32; 2], right: bool, button: &mut Option<i32>) {
    crate::game::big_nukes_glue::screen_click(block, at, right, button);
}
