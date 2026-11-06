// The world's phases: the waves are written when it closes.
fn big_nukes_world_lifecycle(phase: horizon_client::mod_api::WorldPhase) {
    crate::game::big_nukes_glue::lifecycle(phase);
}
