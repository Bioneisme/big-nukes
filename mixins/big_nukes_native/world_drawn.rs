// The world is drawn: the clouds and the rings go over it.
fn big_nukes_world_drawn(world: &mut crate::gfx::native_render::WorldDrawn<'_>) {
    crate::gfx::native_render::big_nukes_sky::drawn(world);
}
