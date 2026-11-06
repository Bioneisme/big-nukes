// A mod message in the window: news of a blast.
fn big_nukes_net_from_server(game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::big_nukes_glue::heard(game, channel, data);
}
