// Big Nukes in the renderer: the blasts the window feels, drawn over the
// finished world. Added to the renderer as `gfx::native_render::big_nukes_sky`;
// the window's glue fills `SKY` every frame.

use super::{RenderNeeds, WorldDrawn, wgpu};
use std::sync::Mutex;

/// A blast as the renderer draws it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SkyBlast {
    pub center: [f64; 3],
    pub radius: f32,
    pub seed: u32,
    /// Ticks since the blast, with the part of the tick gone.
    pub age: f32,
}

/// The blasts to draw this frame.
pub static SKY: Mutex<Vec<SkyBlast>> = Mutex::new(Vec::new());

struct Cache {
    device: wgpu::Device,
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

fn blasts() -> Vec<SkyBlast> {
    SKY.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn needs(needs: &mut RenderNeeds) {
    if !blasts().is_empty() {
        needs.world_depth = true;
    }
}

fn pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("big_nukes sky"),
        source: wgpu::ShaderSource::Wgsl(
            "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                 return vec4(f32(i & 1u) * 4.0 - 1.0, f32(i >> 1u) * 4.0 - 1.0, 0.0, 1.0);
             }
             @fragment fn fs() -> @location(0) vec4<f32> {
                 return vec4(0.2, 0.0, 0.0, 0.2);
             }"
            .into(),
        ),
    });
    let blend = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("big_nukes sky"),
        layout: None,
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

pub fn drawn(world: &mut WorldDrawn<'_>) {
    if blasts().is_empty() {
        return;
    }
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if cache.as_ref().is_none_or(|c| c.format != world.color_format || c.device != *world.device) {
        *cache = Some(Cache { device: world.device.clone(), format: world.color_format, pipeline: pipeline(world.device, world.color_format) });
    }
    let cache = cache.as_ref().expect("built");
    let mut pass = world.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("big_nukes sky"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: world.color_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
        })],
        ..Default::default()
    });
    pass.set_pipeline(&cache.pipeline);
    pass.draw(0..3, 0..1);
}
