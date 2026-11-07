// A message from a player's window: a launch typed on a console.
fn big_nukes_net_from_client(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::big_nukes_silo::net_from_client(game, player, channel, data);
}
