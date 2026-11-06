// The world's fog, every frame: a zone's turns it green and brings it in.
fn big_nukes_render_fog(
    game: &crate::game::Game<'_>,
    camera: [f64; 3],
    fog_type: horizon_client::fog::FogType,
    fog: &mut horizon_client::fog::FogData,
) {
    crate::game::big_nukes_glue::fog(game, camera, fog_type, fog);
}
