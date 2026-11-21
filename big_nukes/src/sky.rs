//! The mushroom clouds and the dust rings: what the renderer draws, filled every frame by `client::frame`
//! (`render_frame_pre`). The drawing itself (old `mixins/big_nukes_native/sky.rs`: the WGSL pass over the finished
//! world) is stage 3: two mixins on `horizon_mod_render::render::{world_drawn, render_needs}` that read `SKY`
//! through `mod_big_nukes::sky` (see `bn-api-mapping.md` §3).
// TODO(stage 3): the mixins' helpers read `SKY` (`render_needs` asks for the scene's depth while it isn't empty).

use std::sync::Mutex;

/// A blast as the renderer draws it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SkyBlast {
    pub center: [f64; 3],
    pub seed: u32,
    /// Ticks since the blast, with the part of the tick gone.
    pub age: f32,
    /// The cloud: fireball height and radius, stem radius, cap major and minor radius, fire, fade, ring radius and
    /// ring fade.
    pub cloud: [f32; 9],
}

/// The blasts the window feels this frame, newest first.
pub static SKY: Mutex<Vec<SkyBlast>> = Mutex::new(Vec::new());

/// The blasts to draw this frame: a copy, so the renderer never holds the lock.
#[allow(dead_code)] // TODO(stage 3): read by the world_drawn and render_needs mixins.
pub fn blasts() -> Vec<SkyBlast> {
    SKY.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
