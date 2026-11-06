// A player's command on the server: the mod's own, when it is one.
fn big_nukes_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &horizon_client::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::big_nukes_glue::perform(game, player, command, ran);
}
