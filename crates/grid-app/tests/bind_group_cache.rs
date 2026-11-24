//! DRIG patch (F874) check: the vendored bevy_pbr bind group cache
//! (third_party/README.md) hands out a cached bind group only while the layout
//! and every bound resource are the same. Each case forces one change that the
//! renderer meets in play: a reallocated buffer (resize, more views or
//! instances), a different bound range (view and shadow-view counts set the
//! work item and uniform ranges), a new texture view (resize) and a new layout.
//! Needs a GPU adapter, so it is ignored by default and runs in prepush's full
//! tier (`--include-ignored`).

use bevy::pbr::drig_bind_group_cache::{BindGroupCache, CachingRenderDevice};
use bevy::render::render_resource::{
    binding_types::{storage_buffer_read_only_sized, texture_2d},
    BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, BindingResource, Buffer,
    BufferBinding, BufferDescriptor, BufferUsages, Extent3d, ShaderStages, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
    TextureViewDescriptor,
};
use bevy::render::renderer::RenderDevice;
use bevy::tasks::block_on;
use std::num::NonZeroU64;

fn render_device() -> RenderDevice {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("a GPU adapter");
    let (device, _queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device");
    RenderDevice::from(device)
}

fn layout(device: &RenderDevice) -> BindGroupLayout {
    device.create_bind_group_layout(
        "drig_cache_test_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_read_only_sized(false, None),
                texture_2d(TextureSampleType::Float { filterable: true }),
            ),
        ),
    )
}

fn buffer(device: &RenderDevice, size: u64) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("drig_cache_test_buffer"),
        size,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    })
}

fn view(device: &RenderDevice) -> TextureView {
    device
        .create_texture(&TextureDescriptor {
            label: Some("drig_cache_test_texture"),
            size: Extent3d { width: 4, height: 4, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&TextureViewDescriptor::default())
}

fn bind(
    device: CachingRenderDevice<'_>,
    layout: &BindGroupLayout,
    buffer: &Buffer,
    offset: u64,
    size: Option<u64>,
    view: &TextureView,
) -> BindGroup {
    device.create_bind_group(
        "drig_cache_test",
        layout,
        &BindGroupEntries::sequential((
            BindingResource::Buffer(BufferBinding {
                buffer,
                offset,
                size: size.and_then(NonZeroU64::new),
            }),
            view,
        )),
    )
}

#[test]
#[ignore = "needs a GPU adapter"]
fn cached_bind_group_only_for_identical_inputs() {
    let render_device = render_device();
    let cache = BindGroupCache::default();
    let device = CachingRenderDevice { device: &render_device, cache: &cache };
    let layout_a = layout(&render_device);
    let buffer_a = buffer(&render_device, 512);
    let view_a = view(&render_device);

    cache.begin_frame();
    let first = bind(device, &layout_a, &buffer_a, 0, None, &view_a);
    let same = bind(device, &layout_a, &buffer_a, 0, None, &view_a);
    assert_eq!(first.id(), same.id(), "same inputs reuse the bind group");

    // Next frame, nothing changed: still the same bind group.
    cache.begin_frame();
    assert_eq!(bind(device, &layout_a, &buffer_a, 0, None, &view_a).id(), first.id());

    // Resize or more views/instances: the buffer is reallocated.
    let buffer_b = buffer(&render_device, 512);
    let realloc = bind(device, &layout_a, &buffer_b, 0, None, &view_a);
    assert_ne!(realloc.id(), first.id(), "a reallocated buffer gets a new bind group");

    // View or shadow-view count changes the bound range of the same buffer.
    let sized = bind(device, &layout_a, &buffer_a, 0, Some(256), &view_a);
    let offset = bind(device, &layout_a, &buffer_a, 256, Some(256), &view_a);
    let resized = bind(device, &layout_a, &buffer_a, 0, Some(128), &view_a);
    let ids = [first.id(), realloc.id(), sized.id(), offset.id(), resized.id()];
    for (i, a) in ids.iter().enumerate() {
        for b in &ids[i + 1..] {
            assert_ne!(a, b, "each bound range gets its own bind group");
        }
    }

    // Resize: a new view of a new texture, and a new view of the same texture.
    let view_b = view(&render_device);
    assert_ne!(bind(device, &layout_a, &buffer_a, 0, None, &view_b).id(), first.id());
    let view_a2 = TextureView::from(view_a.texture().create_view(&TextureViewDescriptor::default()));
    assert_ne!(bind(device, &layout_a, &buffer_a, 0, None, &view_a2).id(), first.id());

    // A new layout with the same entries (pipeline change).
    let layout_b = layout(&render_device);
    assert_ne!(bind(device, &layout_b, &buffer_a, 0, None, &view_a).id(), first.id());

    // The original inputs still hit.
    assert_eq!(bind(device, &layout_a, &buffer_a, 0, None, &view_a).id(), first.id());
    let stats = cache.stats();
    assert_eq!(stats.hits, 3, "{stats:?}");
    assert_eq!(stats.created, 8, "{stats:?}");

    // Bevy swaps two sets of preprocessing buffers every frame: a bind group
    // comes back every other frame and must still hit.
    cache.begin_frame();
    assert_eq!(bind(device, &layout_a, &buffer_b, 0, None, &view_a).id(), realloc.id());
    cache.begin_frame();
    assert_eq!(bind(device, &layout_a, &buffer_a, 0, None, &view_a).id(), first.id());
    let stats = cache.stats();
    assert_eq!((stats.hits, stats.created), (5, 8), "{stats:?}");

    // Entries unused for two whole frames are dropped, then created anew.
    cache.begin_frame();
    cache.begin_frame();
    assert_eq!(cache.stats().entries, 1, "only the bind group used last frame is kept");
    cache.begin_frame();
    assert_eq!(cache.stats().entries, 0);
    drop(first);
    let again = bind(device, &layout_a, &buffer_a, 0, None, &view_a);
    assert_eq!(cache.stats().created, 9);
    drop(again);
}
