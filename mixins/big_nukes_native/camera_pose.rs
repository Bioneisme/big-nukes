// The camera of a frame: the launch camera for the one who launched a missile.
fn big_nukes_camera_pose(
    game: &crate::game::Game<'_>,
    partial: f32,
    position: &mut [f64; 3],
    yaw: &mut f32,
    pitch: &mut f32,
) {
    crate::game::big_nukes_silo::camera_pose(game, partial, position, yaw, pitch);
}
