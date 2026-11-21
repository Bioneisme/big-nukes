fn world_drawn(world: &mut WorldDrawn<'_>) {
    // Big Nukes: the mushroom clouds and the dust rings, one full-screen pass over the finished world
    // (`mod_big_nukes::sky` holds the shader and packs the uniforms).
    struct Cache {
        device: wgpu::Device,
        format: wgpu::TextureFormat,
        pipeline: wgpu::RenderPipeline,
        layout: wgpu::BindGroupLayout,
        uniforms: wgpu::Buffer,
    }
    // The pipeline, or None after it failed to build: then the clouds are left out rather than the game brought down.
    static CACHE: ::std::sync::Mutex<Option<Option<Cache>>> = ::std::sync::Mutex::new(None);
    fn build(device: &wgpu::Device, format: wgpu::TextureFormat) -> Option<Cache> {
        // A backend that can't compile the shader raises an internal error, not a validation one: both are caught.
        let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("big_nukes sky"),
            source: wgpu::ShaderSource::Wgsl(::mod_big_nukes::sky::SHADER.into()),
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("big_nukes sky"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
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
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("big_nukes sky"),
            size: ::mod_big_nukes::sky::UNIFORM_BYTES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = pipeline.get_bind_group_layout(0);
        let failed = [
            ::mod_big_nukes::sky::block_on(validation.pop()),
            ::mod_big_nukes::sky::block_on(internal.pop()),
        ];
        if let Some(e) = failed.into_iter().flatten().next() {
            ::mod_big_nukes::sky::warn(&format!("the sky's pipeline failed, clouds are left out: {e}"));
            return None;
        }
        Some(Cache { device: device.clone(), format, pipeline, layout, uniforms })
    }

    let camera = [world.camera.x, world.camera.y, world.camera.z];
    let shown = ::mod_big_nukes::sky::shown(camera);
    if shown.is_empty() {
        return;
    }
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let stale = match cache.as_ref() {
        None => true,
        Some(None) => false,
        Some(Some(c)) => c.format != world.color_format || c.device != *world.device,
    };
    if stale {
        *cache = Some(build(world.device, world.color_format));
    }
    let Some(Some(cache)) = cache.as_ref() else {
        return;
    };
    let bytes = ::mod_big_nukes::sky::uniforms(
        world.view_proj.inverse().to_cols_array(),
        world.jittered.inverse().to_cols_array(),
        world.size,
        world.depth_size,
        camera,
        &shown,
    );
    world.queue.write_buffer(&cache.uniforms, 0, &bytes);
    let group = world.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("big_nukes sky"),
        layout: &cache.layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: cache.uniforms.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(world.depth) },
        ],
    });
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
    pass.set_bind_group(0, &group, &[]);
    pass.draw(0..3, 0..1);
}
