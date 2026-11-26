//! DRIG patch (F935) check: the vendored bevy_render's `PendingCommandBuffers`
//! finishes encoders queued with `push_unfinished` on the compute pool
//! (`GRID_BEVY_PARALLEL_FINISH=1`), and must still hand the command buffers to
//! the queue in the order they were pushed, finished buffers and trailing
//! `push_encoder` encoders included. Every pushed item first copies the
//! current value of a shared word into its own history slot, then writes its
//! own number into that word, so the history shows the exact submit order.
//! Needs a GPU adapter, so it is ignored by default and runs in prepush's full
//! tier (`--include-ignored`).

use bevy::render::renderer::PendingCommandBuffers;
use bevy::tasks::{block_on, ComputeTaskPool, TaskPool};

const ITEMS: u32 = 24;

#[test]
#[ignore = "needs a GPU adapter"]
fn deferred_encoders_are_submitted_in_push_order() {
    ComputeTaskPool::get_or_init(TaskPool::default);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("a GPU adapter");
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device");
    let buffer = |label, size, usage, data: Option<&[u32]>| {
        let b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        });
        if let Some(data) = data {
            let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
            queue.write_buffer(&b, 0, &bytes);
        }
        b
    };
    let copy = wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
    let word = buffer("word", 4, copy, Some(&[0]));
    let history = buffer("history", 4 * u64::from(ITEMS), copy, None);
    let values: Vec<_> =
        (1..=ITEMS).map(|i| buffer("value", 4, copy, Some(&[i]))).collect();
    let encode = |i: u32| {
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&word, 0, &history, 4 * u64::from(i), 4);
        encoder.copy_buffer_to_buffer(&values[i as usize], 0, &word, 0, 4);
        encoder
    };

    // Mixed like the renderer's systems: mostly deferred encoders, some finished
    // buffers in between, and diagnostics-style trailing encoders at the end.
    let mut pending = PendingCommandBuffers::default();
    for i in 0..ITEMS - 2 {
        if i % 5 == 3 {
            pending.push([encode(i).finish()]);
        } else {
            pending.push_unfinished(encode(i));
        }
    }
    pending.push_encoder(encode(ITEMS - 2));
    pending.push_encoder(encode(ITEMS - 1));
    assert_eq!(pending.len(), ITEMS as usize);
    let buffers = pending.take();
    assert!(pending.is_empty());
    assert_eq!(buffers.len(), ITEMS as usize);
    queue.submit(buffers);

    let readback = buffer(
        "readback",
        4 * u64::from(ITEMS),
        wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        None,
    );
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(&history, 0, &readback, 0, 4 * u64::from(ITEMS));
    queue.submit([encoder.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let seen: Vec<u32> = readback
        .slice(..)
        .get_mapped_range()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect();
    let expected: Vec<u32> = (0..ITEMS).collect();
    assert_eq!(seen, expected, "item i must see item i-1's write: submit order kept");
}
