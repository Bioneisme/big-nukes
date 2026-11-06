// A press on a bomb's keypad screen: the key under it, as a menu button.
fn big_nukes_screen_click(block: &str, at: [f32; 2], right: bool, button: &mut Option<i32>) {
    crate::game::big_nukes_glue::screen_click(block, at, right, button);
}
