//! DRIG patch (F874): keeps bind groups between frames while their layout and
//! every bound resource are unchanged.
//!
//! Bevy 0.19.1 recreates the GPU preprocessing bind groups for every view,
//! phase and batch set each frame, although their buffers rarely change. The
//! cache key holds the layout and, per entry, the binding index and a clone of
//! the bound buffer (with offset and size), sampler or texture view. A clone
//! keeps its resource alive, so a reallocated buffer, a resized range or a new
//! view always gives a new key, and an old bind group is never handed out for
//! new resources. Entries not used in the last two frames are dropped: Bevy
//! keeps two sets of preprocessing buffers and swaps them every frame
//! (`collect_buffers_for_phase`), so a bind group comes back every other frame.
//! Resources the key cannot hold (binding arrays, acceleration structures and
//! external textures) bypass the cache. `GRID_BEVY_BIND_GROUP_CACHE=0` turns
//! the DRIG bind group caches off (A/B timing, image checks).

use core::num::NonZeroU64;
use std::sync::{Mutex, MutexGuard, OnceLock};

use bevy_platform::collections::HashMap;
use bevy_render::{
    render_resource::{BindGroup, BindGroupEntry, BindGroupLayout, BindingResource},
    renderer::RenderDevice,
};

/// Whether the DRIG bind group caches are on: `GRID_BEVY_BIND_GROUP_CACHE`,
/// read once; any value but `0` (or no value) means on.
pub fn bind_group_cache_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("GRID_BEVY_BIND_GROUP_CACHE").map_or(true, |value| value.trim() != "0")
    })
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum ResourceKey {
    Buffer(wgpu::Buffer, u64, Option<NonZeroU64>),
    Sampler(wgpu::Sampler),
    TextureView(wgpu::TextureView),
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    layout: BindGroupLayout,
    entries: Vec<(u32, ResourceKey)>,
}

fn key(layout: &BindGroupLayout, entries: &[BindGroupEntry<'_>]) -> Option<Key> {
    let mut keys = Vec::with_capacity(entries.len());
    for entry in entries {
        let resource = match &entry.resource {
            BindingResource::Buffer(binding) => {
                ResourceKey::Buffer(binding.buffer.clone(), binding.offset, binding.size)
            }
            BindingResource::Sampler(sampler) => ResourceKey::Sampler((*sampler).clone()),
            BindingResource::TextureView(view) => ResourceKey::TextureView((*view).clone()),
            _ => return None,
        };
        keys.push((entry.binding, resource));
    }
    Some(Key {
        layout: layout.clone(),
        entries: keys,
    })
}

/// Counters of a [`BindGroupCache`], for tests and diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BindGroupCacheStats {
    /// Bind groups handed out from the cache.
    pub hits: u64,
    /// Bind groups created (cache misses and uncacheable entries).
    pub created: u64,
    /// Entries held now.
    pub entries: usize,
}

#[derive(Default)]
struct Inner {
    frame: u64,
    map: HashMap<Key, (BindGroup, u64)>,
    hits: u64,
    created: u64,
}

/// A bind group cache keyed on the layout and all bound resources.
#[derive(Default)]
pub struct BindGroupCache {
    inner: Mutex<Inner>,
}

impl BindGroupCache {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Starts a frame: drops the entries that were not used in the last two
    /// frames.
    pub fn begin_frame(&self) {
        let mut inner = self.lock();
        inner.frame += 1;
        let keep_from = inner.frame.saturating_sub(2);
        inner.map.retain(|_, (_, used)| *used >= keep_from);
    }

    /// Like [`RenderDevice::create_bind_group`], but returns the bind group
    /// made earlier for the same layout and resources, if there is one.
    pub fn create_bind_group<'a>(
        &self,
        render_device: &RenderDevice,
        label: impl Into<wgpu::Label<'a>>,
        layout: &'a BindGroupLayout,
        entries: &'a [BindGroupEntry<'a>],
    ) -> BindGroup {
        let mut inner = self.lock();
        let Some(key) = bind_group_cache_enabled()
            .then(|| key(layout, entries))
            .flatten()
        else {
            inner.created += 1;
            return render_device.create_bind_group(label, layout, entries);
        };
        let frame = inner.frame;
        if let Some((bind_group, used)) = inner.map.get_mut(&key) {
            *used = frame;
            let bind_group = bind_group.clone();
            inner.hits += 1;
            return bind_group;
        }
        let bind_group = render_device.create_bind_group(label, layout, entries);
        inner.created += 1;
        inner.map.insert(key, (bind_group.clone(), frame));
        bind_group
    }

    /// The counters so far.
    pub fn stats(&self) -> BindGroupCacheStats {
        let inner = self.lock();
        BindGroupCacheStats {
            hits: inner.hits,
            created: inner.created,
            entries: inner.map.len(),
        }
    }
}

/// A [`RenderDevice`] that creates bind groups through a [`BindGroupCache`].
/// It lets the patched Bevy call sites keep their `create_bind_group` calls.
#[derive(Clone, Copy)]
pub struct CachingRenderDevice<'a> {
    /// The device.
    pub device: &'a RenderDevice,
    /// The cache.
    pub cache: &'a BindGroupCache,
}

impl CachingRenderDevice<'_> {
    /// See [`BindGroupCache::create_bind_group`].
    pub fn create_bind_group<'b>(
        &self,
        label: impl Into<wgpu::Label<'b>>,
        layout: &'b BindGroupLayout,
        entries: &'b [BindGroupEntry<'b>],
    ) -> BindGroup {
        self.cache
            .create_bind_group(self.device, label, layout, entries)
    }
}
