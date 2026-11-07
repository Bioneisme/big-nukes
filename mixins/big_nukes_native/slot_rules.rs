// The centrifuge's slots, for a player's clicks: raw uranium only in the input,
// anything but raw uranium in the fuel slot (fuel is the game's data, out of reach
// here, and what isn't fuel just doesn't burn), nothing into the output. A
// bomb's keypad has one hidden slot, its detonator's, which takes nothing, as a launch console's.
// On the library's side of the game, so it can't reach the rules or the glue.
fn big_nukes_slot_rules(
    menu: &str,
    slot: usize,
    stack: &horizon_client::items::ItemStack,
    may_place: &mut bool,
) {
    if menu == "big_nukes:keypad" || menu == "big_nukes:console" {
        *may_place = false;
        return;
    }
    // The launch pad: the missile in its slot, a tactical warhead or an atomic bomb in the other.
    if menu == "big_nukes:launch_pad" {
        let item = &*stack.item;
        *may_place = match slot {
            0 => item == "big_nukes:missile",
            1 => item == "big_nukes:tactical_warhead" || item == "big_nukes:atomic_bomb",
            _ => false,
        };
        return;
    }
    if menu != "big_nukes:centrifuge" {
        return;
    }
    let raw = &*stack.item == "big_nukes:raw_uranium";
    *may_place = match slot {
        0 => raw,
        1 => !raw,
        _ => false,
    };
}
