fn big_nukes_server_tick(game: &mut crate::game::Game<'_>, phase: horizon_client::mod_api::ServerTickPhase) {
    crate::game::big_nukes_glue::server_tick(game, phase);
}
