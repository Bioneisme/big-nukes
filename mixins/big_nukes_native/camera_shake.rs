fn big_nukes_camera_shake(
    game: &crate::game::Game<'_>,
    partial: f32,
    shake: &mut horizon_client::mod_api::NativeShake,
) {
    crate::game::big_nukes_glue::shake(game, partial, shake);
}
