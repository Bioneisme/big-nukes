//! The mushroom clouds and the dust rings of the blasts the window feels, drawn over the finished world.
//!
//! `client::frame` (`render_frame_pre`) fills `SKY` every frame. The drawing is two native mixins on the renderer
//! (`horizon-mod.toml`): `src/mixins/render_needs.rs` asks for the scene's depth while a blast shows, and
//! `src/mixins/world_drawn.rs` runs one full-screen pass: each pixel's ray is marched through the clouds it crosses,
//! up to the scene's depth, so terrain hides them at any distance. wgpu stays in those helpers (a crate's own wgpu
//! would be another crate than the renderer's); what they ask of the mod is here, in plain numbers.

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

/// The most blasts one frame draws.
const MOST: usize = 4;
/// Beyond this the clouds are left out.
const FARTHEST: f64 = 6000.0;

/// The blasts to draw this frame: a copy, so the renderer never holds the lock.
pub fn blasts() -> Vec<SkyBlast> {
    SKY.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Whether a blast shows: the renderer then keeps the world pass's depth for the clouds.
pub fn needs_depth() -> bool {
    !blasts().is_empty()
}

/// The blasts drawn from a camera at `camera`: a cloud or a ring still showing, nearer than the farthest, at most four.
pub fn shown(camera: [f64; 3]) -> Vec<SkyBlast> {
    blasts()
        .into_iter()
        .filter(|b| b.cloud[6] > 0.0 || b.cloud[8] > 0.0)
        .filter(|b| (b.center[0] - camera[0]).hypot(b.center[2] - camera[2]) < FARTHEST)
        .take(MOST)
        .collect()
}

pub const SHADER: &str = r#"
struct Blast {
    // center relative to the camera, w = seed
    center: vec4<f32>,
    // fireball height, fireball radius, stem radius, cap major radius
    a: vec4<f32>,
    // cap minor radius, fire, fade, ring radius
    b: vec4<f32>,
    // ring fade, age in ticks, -, -
    c: vec4<f32>,
}

struct Frame {
    inv_view_proj: mat4x4<f32>,
    inv_jittered: mat4x4<f32>,
    size: vec2<f32>,
    depth_size: vec2<f32>,
    count: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
    blasts: array<Blast, 4>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var depth: texture_depth_2d;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4(f32(i & 1u) * 4.0 - 1.0, f32(i >> 1u) * 4.0 - 1.0, 0.0, 1.0);
}

fn hash3(p: vec3<f32>) -> f32 {
    let q = fract(p * 0.3183099 + vec3(0.71, 0.113, 0.419));
    let r = q * 17.0;
    return fract(r.x * r.y * r.z * (r.x + r.y + r.z));
}

fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n000 = hash3(i);
    let n100 = hash3(i + vec3(1.0, 0.0, 0.0));
    let n010 = hash3(i + vec3(0.0, 1.0, 0.0));
    let n110 = hash3(i + vec3(1.0, 1.0, 0.0));
    let n001 = hash3(i + vec3(0.0, 0.0, 1.0));
    let n101 = hash3(i + vec3(1.0, 0.0, 1.0));
    let n011 = hash3(i + vec3(0.0, 1.0, 1.0));
    let n111 = hash3(i + vec3(1.0, 1.0, 1.0));
    let x00 = mix(n000, n100, u.x);
    let x10 = mix(n010, n110, u.x);
    let x01 = mix(n001, n101, u.x);
    let x11 = mix(n011, n111, u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

fn fbm(p: vec3<f32>) -> f32 {
    return 0.55 * noise(p) + 0.3 * noise(p * 2.03 + vec3(3.1, 1.7, 4.2)) + 0.15 * noise(p * 4.11 + vec3(7.3, 2.9, 0.4));
}

// The ray's point at reversed depth z, relative to the camera.
fn unproject(m: mat4x4<f32>, ndc: vec2<f32>, z: f32) -> vec3<f32> {
    let p = m * vec4(ndc, z, 1.0);
    return p.xyz / p.w;
}

// Where the ray (origin o, direction d) is inside a sphere: (near, far), near > far when it misses.
fn sphere(o: vec3<f32>, d: vec3<f32>, c: vec3<f32>, r: f32) -> vec2<f32> {
    let oc = o - c;
    let b = dot(oc, d);
    let h = b * b - (dot(oc, oc) - r * r);
    if h < 0.0 {
        return vec2(1.0, -1.0);
    }
    let s = sqrt(h);
    return vec2(-b - s, -b + s);
}

// The cloud's density at q, relative to the blast's ground center.
fn density(q: vec3<f32>, bl: Blast) -> f32 {
    let ball_h = bl.a.x;
    let ball_r = max(bl.a.y, 1.0);
    let rho = length(q.xz);
    let up = q.y;
    let ball = length(q - vec3(0.0, ball_h, 0.0)) - ball_r;
    let stem_r = bl.a.z * (1.0 + 0.6 * clamp(1.0 - up / max(ball_h, 1.0), 0.0, 1.0));
    let stem = max(rho - stem_r, max(-up, up - ball_h));
    let cap = length(vec2(rho - bl.a.w, (up - ball_h) * 1.4)) - bl.b.x;
    let shape = min(ball, min(stem, cap));
    let drift = vec3(0.0, -bl.c.y * 0.02, 0.0);
    let n = fbm(q / (ball_r * 0.45) + drift + vec3(bl.center.w));
    return clamp((-shape + (n - 0.5) * ball_r * 0.9) / (ball_r * 0.35), 0.0, 1.0) * bl.b.z;
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3(0.0)), vec3(1.0 / 2.2));
}

@fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let ndc = vec2(at.x / frame.size.x * 2.0 - 1.0, 1.0 - at.y / frame.size.y * 2.0);
    let near = unproject(frame.inv_view_proj, ndc, 1.0);
    let far = unproject(frame.inv_view_proj, ndc, 0.0);
    let dir = normalize(far - near);
    let texel = vec2<i32>(at.xy * frame.depth_size / frame.size);
    let z = textureLoad(depth, texel, 0);
    var scene = 1e9;
    if z > 0.0 {
        scene = length(unproject(frame.inv_jittered, ndc, z));
    }
    let jitter = hash3(vec3(at.xy, f32(frame.count) * 0.37));
    var color = vec3(0.0);
    var trans = 1.0;
    for (var i = 0u; i < frame.count; i = i + 1u) {
        let bl = frame.blasts[i];
        let c = bl.center.xyz;
        // The dust ring: on the ground where the terrain is drawn, and a low wall of dust everywhere.
        let ring = bl.b.w;
        let ring_fade = bl.c.x;
        if ring_fade > 0.0 && ring > 1.0 {
            if scene < 1e8 {
                let p = dir * scene - c;
                let off = (length(p.xz) - ring) / 6.0;
                let band = exp(-off * off) * ring_fade * step(abs(p.y), 40.0);
                let a = 0.45 * band * trans;
                color = color + a * vec3(0.55, 0.5, 0.45);
                trans = trans * (1.0 - 0.45 * band);
            }
            // The wall: the ray against a cylinder of the ring's radius, 25 blocks high.
            let o = -c;
            let dxz = dir.xz;
            let a2 = dot(dxz, dxz);
            if a2 > 1e-6 {
                let b2 = dot(o.xz, dxz);
                let h2 = b2 * b2 - a2 * (dot(o.xz, o.xz) - ring * ring);
                if h2 > 0.0 {
                    let tw = (-b2 - sqrt(h2)) / a2;
                    let y = o.y + dir.y * tw;
                    if tw > 0.0 && tw < scene && y > 0.0 && y < 25.0 {
                        let a = 0.35 * ring_fade * (1.0 - y / 25.0) * trans;
                        color = color + a * vec3(0.6, 0.55, 0.5);
                        trans = trans * (1.0 - 0.35 * ring_fade * (1.0 - y / 25.0));
                    }
                }
            }
        }
        // The cloud.
        if bl.b.z <= 0.0 {
            continue;
        }
        let ball_h = bl.a.x;
        let ball_r = max(bl.a.y, 1.0);
        let reach = max(ball_h, bl.a.w + bl.b.x) + ball_r * 1.6;
        let mid = c + vec3(0.0, ball_h * 0.5, 0.0);
        let hit = sphere(vec3(0.0), dir, mid, reach);
        let t0 = max(hit.x, 0.0);
        let t1 = min(hit.y, scene);
        if t1 <= t0 {
            continue;
        }
        // From inside the cloud every pixel marches it, and there is little to see but haze.
        var steps = 32.0;
        if hit.x < 0.0 {
            steps = 16.0;
        }
        let dt = (t1 - t0) / steps;
        let fire = bl.b.y;
        for (var s = 0.0; s < steps; s = s + 1.0) {
            let t = t0 + (s + jitter) * dt;
            let q = dir * t - c;
            let d = density(q, bl);
            if d <= 0.001 {
                continue;
            }
            let ext = d * 5.0 / ball_r;
            let a = 1.0 - exp(-ext * dt);
            let height = clamp(q.y / max(ball_h + ball_r, 1.0), 0.0, 1.0);
            // Darker under the cap and at the foot, lit by the sky on top.
            let smoke = vec3(0.30, 0.27, 0.25) * (0.45 + 0.75 * height);
            let core = exp(-length(q - vec3(0.0, ball_h, 0.0)) / ball_r * 1.5);
            let glow = fire * (vec3(2.4, 0.9, 0.25) * core + vec3(0.7, 0.28, 0.08) * exp(-max(q.y - ball_h, 0.0) / ball_r));
            let lit = smoke + glow;
            color = color + trans * a * lit;
            trans = trans * (1.0 - a);
            if trans < 0.02 {
                break;
            }
        }
    }
    let alpha = 1.0 - trans;
    if alpha <= 0.002 {
        discard;
    }
    // Far away the cloud takes on the sky's haze.
    let haze = 1.0 - exp(-min(scene, 6000.0) / 9000.0);
    // A soft tone curve, so the fire glows without burning out to white.
    let mapped = vec3(1.0) - exp(-color / alpha * 1.6);
    let shown = mix(mapped, vec3(0.72, 0.8, 0.9), haze * 0.25);
    return vec4(to_srgb(shown) * alpha, alpha);
}
"#;

/// Two matrices, two sizes, a count with padding, and four blasts of four vectors.
pub const UNIFORM_BYTES: usize = 4 * (16 + 16 + 4 + 4 + MOST * 16);

/// The pass's uniforms: the inverse camera-relative view-projection without and with TAA's jitter (column-major), the
/// frame's and the depth's sizes, and each blast relative to the camera.
pub fn uniforms(
    inv_view_proj: [f32; 16],
    inv_jittered: [f32; 16],
    size: [u32; 2],
    depth_size: [u32; 2],
    camera: [f64; 3],
    shown: &[SkyBlast],
) -> Vec<u8> {
    let mut f: Vec<f32> = Vec::with_capacity(UNIFORM_BYTES / 4);
    f.extend(inv_view_proj);
    f.extend(inv_jittered);
    f.extend([size[0] as f32, size[1] as f32, depth_size[0] as f32, depth_size[1] as f32]);
    let mut bytes: Vec<u8> = f.iter().flat_map(|v| v.to_le_bytes()).collect();
    bytes.extend((shown.len().min(MOST) as u32).to_le_bytes());
    bytes.extend([0u8; 12]);
    for i in 0..MOST {
        let b = shown.get(i).copied().unwrap_or_default();
        let rel = [b.center[0] - camera[0], b.center[1] - camera[1], b.center[2] - camera[2]];
        let k = b.cloud;
        let v: [f32; 16] = [
            rel[0] as f32, rel[1] as f32, rel[2] as f32, (b.seed % 997) as f32,
            k[0], k[1], k[2], k[3],
            k[4], k[5], k[6], k[7],
            k[8], b.age, 0.0, 0.0,
        ];
        bytes.extend(v.iter().flat_map(|x| x.to_le_bytes()));
    }
    bytes
}

/// Waits for a future on this thread: the error scopes the pass's helper pops as it builds the pipeline (wgpu
/// answers them at once on native backends).
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct Wake(std::thread::Thread);
    impl std::task::Wake for Wake {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(std::sync::Arc::new(Wake(std::thread::current())));
    let mut cx = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let std::task::Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::park();
    }
}

/// A warning from the renderer's helpers, in the mod's log.
pub fn warn(line: &str) {
    log::warn!("[big_nukes] {line}");
}
