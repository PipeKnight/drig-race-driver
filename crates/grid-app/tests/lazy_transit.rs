//! DRIG patch (F943) check: with `GRID_WGPU_LAZY_TRANSIT=1` the vendored wgpu-core opens its
//! submit-time "Transit" command buffer only when a submitted command buffer needs a barrier or a
//! memory init. Everything that relies on that buffer must still happen: zero-init of a buffer and
//! a texture that were never written, a buffer written by one command buffer and read by the next
//! in the same submit, and a texture cleared by a render pass and copied out by the next command
//! buffer. Two rounds, so the second runs with resources already initialised and in use (the
//! steady state where most Transit buffers are left out). Needs a GPU adapter, so it is ignored
//! by default and runs in prepush's full tier (`--include-ignored`).

use bevy::tasks::block_on;

const WIDTH: u32 = 64; // 64 RGBA8 texels = 256 bytes, one aligned row

#[test]
#[ignore = "needs a GPU adapter"]
fn lazy_transit_keeps_inits_and_barriers() {
    // The only test in this binary, so no other thread reads the environment.
    unsafe { std::env::set_var("GRID_WGPU_LAZY_TRANSIT", "1") };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("a GPU adapter");
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device");
    let buffer = |label, size, usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let copy = wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
    let read = wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;
    let texture = |label, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: WIDTH, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        })
    };
    let src = buffer("src", 16, copy);
    let mid = buffer("mid", 16, copy);
    let never_written = buffer("never written", 16, copy);
    let target = texture(
        "target",
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let never_drawn = texture("never drawn", wgpu::TextureUsages::COPY_SRC);
    let target_view = target.create_view(&Default::default());
    let texture_to = |encoder: &mut wgpu::CommandEncoder, tex: &wgpu::Texture, out: &wgpu::Buffer| {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: out,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * WIDTH),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d { width: WIDTH, height: 1, depth_or_array_layers: 1 },
        );
    };

    for (round, (values, color, rgba)) in [
        ([1u32, 2, 3, 4], wgpu::Color::GREEN, [0u8, 255, 0, 255]),
        ([5u32, 6, 7, 8], wgpu::Color::RED, [255u8, 0, 0, 255]),
    ]
    .into_iter()
    .enumerate()
    {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        queue.write_buffer(&src, 0, &bytes);
        let out_zero = buffer("out zero", 16, read);
        let out_mid = buffer("out mid", 16, read);
        let out_drawn = buffer("out drawn", 4 * u64::from(WIDTH), read);
        let out_blank = buffer("out blank", 4 * u64::from(WIDTH), read);
        let new = || device.create_command_encoder(&Default::default());
        let mut buffers = Vec::new();
        // A buffer never written: its copy needs the zero-init in the Transit buffer.
        let mut e = new();
        e.copy_buffer_to_buffer(&never_written, 0, &out_zero, 0, 16);
        buffers.push(e.finish());
        // Written by one command buffer, read by the next: needs a barrier between them.
        let mut e = new();
        e.copy_buffer_to_buffer(&src, 0, &mid, 0, 16);
        buffers.push(e.finish());
        let mut e = new();
        e.copy_buffer_to_buffer(&mid, 0, &out_mid, 0, 16);
        buffers.push(e.finish());
        // Cleared by a render pass, copied out by the next command buffer: needs a layout change.
        let mut e = new();
        e.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        buffers.push(e.finish());
        let mut e = new();
        texture_to(&mut e, &target, &out_drawn);
        buffers.push(e.finish());
        // A texture never drawn: its copy needs the texture zero-init.
        let mut e = new();
        texture_to(&mut e, &never_drawn, &out_blank);
        buffers.push(e.finish());
        queue.submit(buffers);

        for out in [&out_zero, &out_mid, &out_drawn, &out_blank] {
            out.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        }
        device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let bytes_of = |out: &wgpu::Buffer| out.slice(..).get_mapped_range().to_vec();
        assert_eq!(bytes_of(&out_zero), vec![0u8; 16], "round {round}: buffer zero-init");
        assert_eq!(bytes_of(&out_mid), bytes, "round {round}: copy chain across command buffers");
        assert_eq!(
            bytes_of(&out_drawn),
            rgba.repeat(WIDTH as usize),
            "round {round}: render pass clear seen by the next command buffer"
        );
        assert_eq!(
            bytes_of(&out_blank),
            vec![0u8; 4 * WIDTH as usize],
            "round {round}: texture zero-init"
        );
    }
}
