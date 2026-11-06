// A mod message in the window: news of a blast, or of the radiation.
fn big_nukes_net_from_server(game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::big_nukes_glue::heard(game, channel, data);
    crate::game::big_nukes_glue::told(game, channel, data);
}
