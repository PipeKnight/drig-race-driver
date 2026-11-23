//! DDS texture upload for GPUs with and without BC compression (P8,
//! docs/PLATFORMS_AND_INPUT_RESEARCH.md 7.2). Most Mali, Xclipse and
//! PowerVR phones lack `TEXTURE_COMPRESSION_BC`. The path is chosen once:
//! - `bc`: the image is exactly Bevy's DDS image, as before;
//! - `etc2`: BC1/BC3 textures come from the private ETC2 KTX2 cache
//!   (`texture_cache`), encoded on first use, same GPU memory as BC;
//! - `rgba8`: BC1/BC3 levels are decoded to RGBA8 at load (mips kept).
//!
//! Without BC the order is ETC2, then RGBA8. Diagnostics `texture_fallback`
//! `on`/`rgba8` forces RGBA8, `etc2` forces ETC2 (when the GPU has it); the
//! legacy `GRID_TEXTURE_FALLBACK` takes the same values. A forced value also
//! leaves BC off the device ([`render_creation`]), so the desktop fails like
//! a phone without BC whenever a BC texture slips through.
//!
//! The device, and so the path, exists only once the app runs; loaders that
//! build images while the app is assembled (the OSD scene) get the BC image.
//! [`convert_existing`] puts those on the chosen path before the first frame
//! and [`convert_added`] any BC image added later by any loader (F603).
use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType, TextureError},
    prelude::*,
    render::{
        render_resource::{TextureFormat, WgpuFeatures},
        renderer::RenderDevice,
    },
};
use std::{
    sync::atomic::{AtomicU8, AtomicU64, Ordering},
    time::Instant,
};

const BC: u8 = 0;
const ETC2: u8 = 1;
const RGBA8: u8 = 2;

static PATH: AtomicU8 = AtomicU8::new(BC);
static IMAGES: AtomicU64 = AtomicU64::new(0);
static DECODED: AtomicU64 = AtomicU64::new(0);
static UPLOAD_BYTES: AtomicU64 = AtomicU64::new(0);
static SOURCE_BYTES: AtomicU64 = AtomicU64::new(0);
static DECODE_NANOS: AtomicU64 = AtomicU64::new(0);
static LATE: AtomicU64 = AtomicU64::new(0);

fn choose(bc: bool, etc2: bool, forced: Option<&str>) -> (u8, &'static str) {
    match forced {
        Some("rgba8") => (RGBA8, "rgba8-cpu-decode (forced by texture_fallback)"),
        Some("etc2") if etc2 => (ETC2, "etc2-cache (forced by texture_fallback)"),
        Some("etc2") => (
            RGBA8,
            "rgba8-cpu-decode (etc2 forced but the GPU lacks ETC2)",
        ),
        _ if bc => (BC, "bc"),
        _ if etc2 => (ETC2, "etc2-cache (no BC support)"),
        _ => (RGBA8, "rgba8-cpu-decode (no BC or ETC2 support)"),
    }
}

/// The renderer's device settings. A forced `texture_fallback` also leaves
/// `TEXTURE_COMPRESSION_BC` off the device, so a desktop GPU rejects every BC
/// texture as a Mali or Xclipse phone does (F603: the OSD's 48 BC3 textures
/// were only caught on a friend's Mali-G77). `GRID_NO_BINDLESS=1` leaves the
/// binding-array features off, so every bindless material (the car reflection
/// overlay, F865) takes its non-bindless bind group and shader path, as on a
/// GPU without descriptor indexing.
pub(crate) fn render_creation() -> bevy::render::settings::RenderCreation {
    let forced = crate::settings::diagnostics().texture_fallback.is_some();
    let no_bindless = std::env::var("GRID_NO_BINDLESS").is_ok_and(|v| v == "1");
    let mut disabled = WgpuFeatures::empty();
    disabled.set(WgpuFeatures::TEXTURE_COMPRESSION_BC, forced);
    disabled.set(
        WgpuFeatures::TEXTURE_BINDING_ARRAY | WgpuFeatures::BUFFER_BINDING_ARRAY,
        no_bindless,
    );
    let mut settings = bevy::render::settings::WgpuSettings {
        disabled_features: (!disabled.is_empty()).then_some(disabled),
        ..default()
    };
    drop_unreachable_dx12(&mut settings);
    bevy::render::settings::RenderCreation::Automatic(Box::new(settings))
}

/// Leaves DX12 out of the backends when wgpu would not choose a DX12 adapter
/// anyway, so release builds lose wgpu's `VALIDATION_INDIRECT_CALL` (F868).
/// Bevy keeps that flag whenever DX12 is among the backends, because the
/// D3D12 backend needs it for its draw built-ins; `Backends::all()` always
/// contains DX12, even on Android. The flag adds a validation compute pass
/// per command buffer and per-draw tracking of every indirect draw to
/// `CommandEncoder::finish` (docs/PERFORMANCE.md, F868).
///
/// The chosen adapter stays the same: DX12 exists only on Windows, and there
/// wgpu sorts the adapters of all backends by device type with a stable sort
/// and lists Vulkan before DX12, so a Vulkan adapter of the best-ranked type
/// can never lose to a DX12 one. The probe enumerates only Vulkan (DX12
/// enumeration is what makes a full probe slow) and drops DX12 only when such
/// an adapter exists. `WGPU_BACKEND`, `WGPU_ADAPTER_NAME` and
/// `WGPU_VALIDATION_INDIRECT_CALL` keep Bevy's behaviour, and debug builds
/// keep the validation.
fn drop_unreachable_dx12(settings: &mut bevy::render::settings::WgpuSettings) {
    use wgpu::{Backends, InstanceFlags};
    let Some(backends) = settings.backends else {
        return;
    };
    let env_set = |name| std::env::var_os(name).is_some();
    if cfg!(debug_assertions)
        || !backends.contains(Backends::DX12)
        || env_set("WGPU_BACKEND")
        || env_set("WGPU_ADAPTER_NAME")
        || env_set("WGPU_VALIDATION_INDIRECT_CALL")
    {
        return;
    }
    let started = std::time::Instant::now();
    if cfg!(windows) && !vulkan_cannot_lose(&vulkan_adapter_types(), settings.power_preference) {
        println!("render backends: no top-ranked Vulkan adapter, DX12 and indirect-call validation kept");
        return;
    }
    settings.backends = Some(backends - Backends::DX12);
    settings
        .instance_flags
        .remove(InstanceFlags::VALIDATION_INDIRECT_CALL);
    if cfg!(windows) {
        println!(
            "render backends: DX12 left out (top-ranked Vulkan adapter), probe {:.0} ms",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
}

/// Device types of this machine's Vulkan adapters.
fn vulkan_adapter_types() -> Vec<wgpu::DeviceType> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(desc);
    bevy::tasks::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN))
        .iter()
        .map(|a| a.get_info().device_type)
        .collect()
}

/// wgpu-core `Instance::request_adapter` without a surface lists Vulkan
/// adapters before DX12 ones and stable-sorts them by device type for the
/// power preference (no sort for `None`); the first one wins. A Vulkan adapter
/// of the type ranked first therefore wins whatever DX12 offers.
fn vulkan_cannot_lose(vulkan: &[wgpu::DeviceType], power: wgpu::PowerPreference) -> bool {
    use wgpu::{DeviceType, PowerPreference};
    match power {
        PowerPreference::None => !vulkan.is_empty(),
        PowerPreference::HighPerformance => vulkan.contains(&DeviceType::DiscreteGpu),
        PowerPreference::LowPower => vulkan.contains(&DeviceType::IntegratedGpu),
    }
}

/// Chooses the upload path before any Startup system creates images.
pub(crate) fn configure(device: Res<RenderDevice>) {
    let features = device.features();
    let bc = features.contains(WgpuFeatures::TEXTURE_COMPRESSION_BC);
    let etc2 = features.contains(WgpuFeatures::TEXTURE_COMPRESSION_ETC2);
    let forced = crate::settings::diagnostics().texture_fallback.as_deref();
    let (path, text) = choose(bc, etc2, forced);
    PATH.store(path, Ordering::Relaxed);
    println!("texture upload: gpu_bc={bc} gpu_etc2={etc2} path={text}");
    if path == ETC2 {
        println!("texture cache: {}", crate::texture_cache::dir().display());
    }
}

/// The original crowd loader retains the complete BC-block prefix of these
/// rectangular atlases. Their declared full mip chains have a trailing payload
/// that is larger than the tightly packed chain would be.
pub(crate) fn crowd_billboard_dds(texture: &grid_formats::texture::Texture<'_>) -> Vec<u8> {
    use grid_formats::texture::Format::{Bc1, Bc3};

    let (base, levels) = texture.validated_dds();
    if levels > 1
        || !matches!(texture.format, Bc1 | Bc3)
        || texture.width == texture.height
        || !texture.width.is_power_of_two()
        || !texture.height.is_power_of_two()
        || texture.declared_mip_levels != texture.width.max(texture.height).ilog2()
    {
        return base;
    }

    let mut width = texture.width / 2;
    let mut height = texture.height / 2;
    let mut mip_levels = 0;
    let mut used = 0;
    while width.min(height) >= 4 {
        used += texture.format.level_size(width, height);
        mip_levels += 1;
        width /= 2;
        height /= 2;
    }
    if mip_levels == 0 || texture.remaining_data.len() <= used {
        return base;
    }

    grid_formats::texture::Texture {
        declared_mip_levels: mip_levels,
        remaining_data: &texture.remaining_data[..used],
        ..*texture
    }
    .validated_dds()
    .0
}

#[test]
fn rectangular_crowd_atlas_retains_only_the_authored_bc_prefix() {
    use grid_formats::texture::{Format, Texture};

    for (width, height, format, expected_levels, surplus) in [
        (512, 256, Format::Bc3, 7, 1056),
        (512, 256, Format::Bc1, 7, 528),
    ] {
        let base_bytes = format.level_size(width, height);
        let tail_bytes = (1..expected_levels)
            .map(|i| format.level_size(width >> i, height >> i))
            .sum::<usize>();
        let base_blocks = vec![0x12; base_bytes];
        let mut remaining = vec![0x5a; tail_bytes + surplus];
        remaining[tail_bytes - 1] = 0x23;
        remaining[tail_bytes..].fill(0xcd);
        let texture = Texture {
            id: b"crowd.tga",
            width,
            height,
            format,
            declared_mip_levels: width.ilog2(),
            base_blocks: &base_blocks,
            remaining_data: &remaining,
        };
        assert_eq!(texture.validated_dds().1, 1);

        let dds = crowd_billboard_dds(&texture);
        assert_eq!(u32::from_le_bytes(dds[28..32].try_into().unwrap()), expected_levels);
        assert_eq!(&dds[128 + base_bytes..], &remaining[..tail_bytes]);

        let incomplete = Texture {
            remaining_data: &remaining[..tail_bytes - 1],
            ..texture
        };
        assert_eq!(crowd_billboard_dds(&incomplete), incomplete.base_dds());
    }
}

/// The sRGB image of one validated DDS (BC1, BC3 or BGRA8, with mips).
pub(crate) fn image(
    dds: &[u8],
    sampler: ImageSampler,
    usage: RenderAssetUsages,
) -> Result<Image, TextureError> {
    image_with_color_space(dds, sampler, usage, true)
}

/// Linear data maps (normals and weights) use the same portable upload path.
pub(crate) fn image_linear(
    dds: &[u8],
    sampler: ImageSampler,
    usage: RenderAssetUsages,
) -> Result<Image, TextureError> {
    image_with_color_space(dds, sampler, usage, false)
}

fn image_with_color_space(
    dds: &[u8],
    sampler: ImageSampler,
    usage: RenderAssetUsages,
    srgb: bool,
) -> Result<Image, TextureError> {
    let mut image = Image::from_buffer(
        dds,
        ImageType::Extension("dds"),
        CompressedImageFormats::BC,
        srgb,
        sampler,
        usage,
    )?;
    let source = image.data.as_ref().map_or(0, Vec::len) as u64;
    convert(&mut image, Some(dds))?;
    IMAGES.fetch_add(1, Ordering::Relaxed);
    SOURCE_BYTES.fetch_add(source, Ordering::Relaxed);
    UPLOAD_BYTES.fetch_add(
        image.data.as_ref().map_or(0, Vec::len) as u64,
        Ordering::Relaxed,
    );
    Ok(image)
}

/// Puts one BC1/BC3 image on the chosen path; false when nothing changed
/// (the path is BC, or another format). `key` names the ETC2 cache entry:
/// the DDS file, or without one the BC levels themselves.
fn convert(image: &mut Image, key: Option<&[u8]>) -> Result<bool, TextureError> {
    let path = PATH.load(Ordering::Relaxed);
    if path == BC || !is_bc(image.texture_descriptor.format) {
        return Ok(false);
    }
    let start = Instant::now();
    let etc2 = if path == ETC2 {
        let key = key.unwrap_or_else(|| image.data.as_deref().unwrap_or_default());
        etc2_image(key, image).unwrap_or_else(|error| {
            warn!("ETC2 texture cache failed, using RGBA8: {error}");
            None
        })
    } else {
        None
    };
    let changed = match etc2 {
        Some(etc2) => {
            *image = etc2;
            true
        }
        None => decode_in_place(image)?,
    };
    if changed {
        DECODED.fetch_add(1, Ordering::Relaxed);
    }
    DECODE_NANOS.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
    Ok(changed)
}

fn is_bc(format: TextureFormat) -> bool {
    matches!(
        format,
        TextureFormat::Bc1RgbaUnorm
            | TextureFormat::Bc1RgbaUnormSrgb
            | TextureFormat::Bc2RgbaUnorm
            | TextureFormat::Bc2RgbaUnormSrgb
            | TextureFormat::Bc3RgbaUnorm
            | TextureFormat::Bc3RgbaUnormSrgb
    )
}

/// PreStartup, after [`configure`]: the BC images already made (while the
/// app was built) on the chosen path.
pub(crate) fn convert_existing(mut images: ResMut<Assets<Image>>) {
    println!(
        "texture upload: images_before_startup={} bc={}",
        images.len(),
        images.iter().filter(|(_, i)| is_bc(i.texture_descriptor.format)).count()
    );
    let ids: Vec<AssetId<Image>> = images
        .iter()
        .filter(|(_, image)| is_bc(image.texture_descriptor.format))
        .map(|(id, _)| id)
        .collect();
    convert_ids(&mut images, ids, "built");
}

/// Last, before extraction: BC images added this frame by any loader.
pub(crate) fn convert_added(
    mut events: MessageReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
) {
    let added: Vec<AssetId<Image>> = events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    if PATH.load(Ordering::Relaxed) == BC {
        return;
    }
    let ids = added
        .into_iter()
        .filter(|id| images.get(*id).is_some_and(|i| is_bc(i.texture_descriptor.format)))
        .collect();
    convert_ids(&mut images, ids, "added");
}

fn convert_ids(images: &mut Assets<Image>, ids: Vec<AssetId<Image>>, when: &str) {
    if ids.is_empty() || PATH.load(Ordering::Relaxed) == BC {
        return;
    }
    let (mut converted, mut failed) = (0, 0);
    for id in ids {
        let Some(mut image) = images.get_mut(id) else {
            continue;
        };
        match convert(&mut image, None) {
            Ok(true) => converted += 1,
            Ok(false) => {}
            Err(error) => {
                failed += 1;
                error!("texture upload: BC image {id:?} cannot be converted: {error}");
            }
        }
    }
    LATE.fetch_add(converted, Ordering::Relaxed);
    println!("texture upload: late_converted={converted} failed={failed} when={when}");
}

/// The ETC2 image of a BC1/BC3 DDS image; `None` for other formats.
fn etc2_image(dds: &[u8], bc: &Image) -> Result<Option<Image>, String> {
    let alpha = match bc.texture_descriptor.format {
        TextureFormat::Bc1RgbaUnormSrgb => false,
        TextureFormat::Bc3RgbaUnormSrgb => true,
        _ => return Ok(None),
    };
    let size = bc.texture_descriptor.size;
    if size.depth_or_array_layers != 1 {
        return Err("ETC2 cache supports single 2D images".into());
    }
    let ktx2 = crate::texture_cache::ktx2(
        dds,
        bc.data.as_deref().unwrap_or_default(),
        size.width,
        size.height,
        bc.texture_descriptor.mip_level_count,
        alpha,
    )?;
    Image::from_buffer(
        &ktx2,
        ImageType::Extension("ktx2"),
        CompressedImageFormats::ETC2,
        true,
        bc.sampler.clone(),
        bc.asset_usage,
    )
    .map(Some)
    .map_err(|e| e.to_string())
}

/// One line with the texel bytes created so far (GPU texture memory, and the
/// same again in the main world for MAIN_WORLD images).
pub(crate) fn summary() -> String {
    format!(
        "texture upload summary: path={} images={} decoded={} late_converted={} source_bytes={} upload_bytes={} decode_ms={:.1} {}",
        ["bc", "etc2", "rgba8"][usize::from(PATH.load(Ordering::Relaxed))],
        IMAGES.load(Ordering::Relaxed),
        DECODED.load(Ordering::Relaxed),
        LATE.load(Ordering::Relaxed),
        SOURCE_BYTES.load(Ordering::Relaxed),
        UPLOAD_BYTES.load(Ordering::Relaxed),
        DECODE_NANOS.load(Ordering::Relaxed) as f64 / 1e6,
        crate::texture_cache::summary()
    )
}

/// Replaces BC1/BC2/BC3 levels by RGBA8 levels; other formats are kept.
pub(crate) fn decode_in_place(image: &mut Image) -> Result<bool, TextureError> {
    let (decoded, kind) = match image.texture_descriptor.format {
        TextureFormat::Bc1RgbaUnormSrgb => (TextureFormat::Rgba8UnormSrgb, Block::Bc1),
        TextureFormat::Bc1RgbaUnorm => (TextureFormat::Rgba8Unorm, Block::Bc1),
        TextureFormat::Bc2RgbaUnormSrgb => (TextureFormat::Rgba8UnormSrgb, Block::Bc2),
        TextureFormat::Bc2RgbaUnorm => (TextureFormat::Rgba8Unorm, Block::Bc2),
        TextureFormat::Bc3RgbaUnormSrgb => (TextureFormat::Rgba8UnormSrgb, Block::Bc3),
        TextureFormat::Bc3RgbaUnorm => (TextureFormat::Rgba8Unorm, Block::Bc3),
        _ => return Ok(false),
    };
    let size = image.texture_descriptor.size;
    if size.depth_or_array_layers != 1 {
        return Err(TextureError::InvalidData(
            "BC fallback supports single 2D images".into(),
        ));
    }
    let levels = image.texture_descriptor.mip_level_count;
    let data = image.data.take().unwrap_or_default();
    let out = decode_blocks(&data, size.width, size.height, levels, kind)
        .ok_or_else(|| TextureError::InvalidData("truncated BC levels".into()))?;
    image.data = Some(out);
    image.texture_descriptor.format = decoded;
    Ok(true)
}

/// Tightly packed BC1 (`alpha` false) or BC3 levels to tightly packed RGBA8.
pub(crate) fn decode_levels(
    data: &[u8],
    width: u32,
    height: u32,
    levels: u32,
    alpha: bool,
) -> Option<Vec<u8>> {
    decode_blocks(data, width, height, levels, if alpha { Block::Bc3 } else { Block::Bc1 })
}

/// A package texture's tightly packed levels (any [`Format`]) to tightly
/// packed RGBA8.
///
/// [`Format`]: grid_formats::texture::Format
pub(crate) fn decode_format_levels(
    data: &[u8],
    width: u32,
    height: u32,
    levels: u32,
    format: grid_formats::texture::Format,
) -> Option<Vec<u8>> {
    use grid_formats::texture::Format;
    let kind = match format {
        Format::Bc1 => Block::Bc1,
        Format::Bc2 => Block::Bc2,
        Format::Bc3 => Block::Bc3,
        Format::Bgra8 => {
            let size: usize = (0..levels).map(|l| format.level_size((width >> l).max(1), (height >> l).max(1))).sum();
            let data = data.get(..size)?;
            return Some(data.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect());
        }
    };
    decode_blocks(data, width, height, levels, kind)
}

/// The block-compressed layouts: BC1 (8 bytes), BC2 (explicit 4-bit alpha,
/// then a four-colour BC1 block) and BC3 (interpolated alpha, then the same).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    Bc1,
    Bc2,
    Bc3,
}

fn decode_blocks(data: &[u8], width: u32, height: u32, levels: u32, kind: Block) -> Option<Vec<u8>> {
    let block = if kind == Block::Bc1 { 8 } else { 16 };
    let mut out = Vec::new();
    let mut offset = 0;
    for level in 0..levels {
        let w = (width >> level).max(1) as usize;
        let h = (height >> level).max(1) as usize;
        let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
        let level_data = data.get(offset..offset + bw * bh * block)?;
        offset += bw * bh * block;
        let base = out.len();
        out.resize(base + w * h * 4, 0);
        for by in 0..bh {
            for bx in 0..bw {
                let bytes = &level_data[(by * bw + bx) * block..][..block];
                let texels = match kind {
                    Block::Bc1 => colour_block(bytes.try_into().unwrap(), false),
                    Block::Bc2 | Block::Bc3 => {
                        let mut t = colour_block(bytes[8..].try_into().unwrap(), true);
                        let alpha = bytes[..8].try_into().unwrap();
                        let alpha = if kind == Block::Bc2 { explicit_alpha(alpha) } else { alpha_block(alpha) };
                        for (texel, a) in t.iter_mut().zip(alpha) {
                            texel[3] = a;
                        }
                        t
                    }
                };
                for (i, texel) in texels.iter().enumerate() {
                    let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                    if x < w && y < h {
                        let at = base + (y * w + x) * 4;
                        out[at..at + 4].copy_from_slice(texel);
                    }
                }
            }
        }
    }
    Some(out)
}

fn rgb565(c: u16) -> [u32; 3] {
    let (r, g, b) = (
        u32::from(c >> 11),
        u32::from(c >> 5 & 63),
        u32::from(c & 31),
    );
    [r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2]
}

/// One 4x4 colour block in texel order. BC3 colour blocks always use the
/// four-colour mode; BC1 with `c0 <= c1` has a transparent black index 3.
fn colour_block(bytes: &[u8; 8], opaque: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([bytes[0], bytes[1]]);
    let c1 = u16::from_le_bytes([bytes[2], bytes[3]]);
    let (p0, p1) = (rgb565(c0), rgb565(c1));
    let mix = |f: fn(u32, u32) -> u32| -> [u8; 4] {
        let c: [u32; 3] = std::array::from_fn(|i| f(p0[i], p1[i]));
        [c[0] as u8, c[1] as u8, c[2] as u8, 255]
    };
    let solid = |p: [u32; 3]| [p[0] as u8, p[1] as u8, p[2] as u8, 255];
    let palette = if opaque || c0 > c1 {
        [
            solid(p0),
            solid(p1),
            mix(|a, b| (2 * a + b + 1) / 3),
            mix(|a, b| (a + 2 * b + 1) / 3),
        ]
    } else {
        [
            solid(p0),
            solid(p1),
            mix(|a, b| (a + b).div_ceil(2)),
            [0; 4],
        ]
    };
    let indices = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    std::array::from_fn(|i| palette[(indices >> (2 * i) & 3) as usize])
}

/// One 4x4 BC2 alpha block in texel order: 4 bits a texel, low nibble first.
fn explicit_alpha(bytes: &[u8; 8]) -> [u8; 16] {
    let bits = u64::from_le_bytes(*bytes);
    std::array::from_fn(|i| (bits >> (4 * i) & 15) as u8 * 17)
}

/// One 4x4 BC3 alpha block in texel order.
fn alpha_block(bytes: &[u8; 8]) -> [u8; 16] {
    let (a0, a1) = (u32::from(bytes[0]), u32::from(bytes[1]));
    let mut palette = [0u32; 8];
    palette[0] = a0;
    palette[1] = a1;
    if a0 > a1 {
        for i in 1..7 {
            palette[i + 1] = ((7 - i as u32) * a0 + i as u32 * a1 + 3) / 7;
        }
    } else {
        for i in 1..5 {
            palette[i + 1] = ((5 - i as u32) * a0 + i as u32 * a1 + 2) / 5;
        }
        palette[7] = 255;
    }
    let mut bits = [0u8; 8];
    bits[..6].copy_from_slice(&bytes[2..8]);
    let indices = u64::from_le_bytes(bits);
    std::array::from_fn(|i| palette[(indices >> (3 * i) & 7) as usize] as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx12_is_dropped_only_when_vulkan_would_win() {
        use wgpu::{DeviceType::*, PowerPreference::*};
        assert!(vulkan_cannot_lose(&[IntegratedGpu, DiscreteGpu], HighPerformance));
        // Vulkan only on the integrated GPU: a discrete DX12 adapter could win.
        assert!(!vulkan_cannot_lose(&[IntegratedGpu], HighPerformance));
        assert!(vulkan_cannot_lose(&[IntegratedGpu], LowPower));
        assert!(!vulkan_cannot_lose(&[DiscreteGpu], LowPower));
        assert!(vulkan_cannot_lose(&[Cpu], None));
        assert!(!vulkan_cannot_lose(&[], None));
        assert!(!vulkan_cannot_lose(&[], HighPerformance));
    }

    #[test]
    fn linear_normal_upload_and_rgba_fallback_preserve_mips_and_colour_space() {
        let block = [0x10, 0x84, 0, 0, 0, 0, 0, 0];
        let source = grid_formats::texture::Texture {
            id: b"linear-test",
            width: 4,
            height: 4,
            format: grid_formats::texture::Format::Bc1,
            declared_mip_levels: 1,
            base_blocks: &block,
            remaining_data: &block,
        };
        let (dds, levels) = source.validated_dds();
        assert_eq!(levels, 2);
        let mut image =
            image_linear(&dds, ImageSampler::Default, RenderAssetUsages::default()).unwrap();
        assert_eq!(image.texture_descriptor.format, TextureFormat::Bc1RgbaUnorm);
        assert!(decode_in_place(&mut image).unwrap());
        assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        assert_eq!(image.texture_descriptor.mip_level_count, 2);
        let pixels = image.data.as_ref().unwrap();
        assert_eq!(pixels.len(), 80);
        assert_eq!(&pixels[..4], &[132, 130, 132, 255]);
        assert_eq!(&pixels[64..68], &[132, 130, 132, 255]);
    }

    #[test]
    fn path_choice() {
        assert_eq!(choose(true, true, None).0, BC);
        assert_eq!(choose(false, true, None).0, ETC2);
        assert_eq!(choose(false, false, None).0, RGBA8);
        assert_eq!(choose(true, true, Some("rgba8")).0, RGBA8);
        assert_eq!(choose(true, true, Some("etc2")).0, ETC2);
        assert_eq!(choose(true, false, Some("etc2")).0, RGBA8);
        assert_eq!(choose(true, true, None).0, BC);
    }

    #[test]
    fn bc1_four_and_three_colour_modes() {
        // c0 = white, c1 = black, indices 0,1,2,3 in the first row.
        let block = [0xff, 0xff, 0, 0, 0b1110_0100, 0, 0, 0];
        let t = colour_block(&block, false);
        assert_eq!(t[0], [255; 4]);
        assert_eq!(t[1], [0, 0, 0, 255]);
        assert_eq!(t[2], [170, 170, 170, 255]);
        assert_eq!(t[3], [85, 85, 85, 255]);
        assert_eq!(t[4], [255; 4]);
        // c0 <= c1: midpoint and transparent black.
        let block = [0, 0, 0xff, 0xff, 0b1110_0100, 0, 0, 0];
        let t = colour_block(&block, false);
        assert_eq!(t[2], [128, 128, 128, 255]);
        assert_eq!(t[3], [0; 4]);
        // The same colour block inside BC3 stays opaque.
        assert_eq!(colour_block(&block, true)[3], [170, 170, 170, 255]);
    }

    #[test]
    fn bc1_565_expansion() {
        // Pure red, green and blue endpoints expand by bit replication.
        assert_eq!(rgb565(0xf800), [255, 0, 0]);
        assert_eq!(rgb565(0x07e0), [0, 255, 0]);
        assert_eq!(rgb565(0x001f), [0, 0, 255]);
        assert_eq!(rgb565(0x8410), [132, 130, 132]);
    }

    #[test]
    fn bc3_alpha_modes() {
        // Eight-value mode, indices 0..7 in texels 0..7.
        let mut block = [255, 0, 0, 0, 0, 0, 0, 0];
        let indices: u64 = (0..8).map(|i| (i as u64) << (3 * i)).sum();
        block[2..8].copy_from_slice(&indices.to_le_bytes()[..6]);
        let a = alpha_block(&block);
        assert_eq!(&a[..8], &[255, 0, 219, 182, 146, 109, 73, 36]);
        // Six-value mode with explicit 0 and 255.
        block[0] = 0;
        block[1] = 255;
        let a = alpha_block(&block);
        assert_eq!(&a[..8], &[0, 255, 51, 102, 153, 204, 0, 255]);
    }

    #[test]
    fn mip_chain_sizes_and_cropping() {
        // 8x4 BC1: 2 + 1 + 1 + 1 blocks for levels 8x4, 4x2, 2x1, 1x1.
        let data = vec![0xffu8; 5 * 8];
        let out = decode_levels(&data, 8, 4, 4, false).unwrap();
        assert_eq!(out.len(), (32 + 8 + 2 + 1) * 4);
        assert!(decode_levels(&data[..32], 8, 4, 4, false).is_none());
        // 4x4 BC3: alpha from the first half, colour from the second.
        let mut block = [0u8; 16];
        block[0] = 77;
        block[1] = 77;
        block[8..10].copy_from_slice(&0xf800u16.to_le_bytes());
        let out = decode_levels(&block, 4, 4, 1, true).unwrap();
        assert_eq!(&out[..4], &[255, 0, 0, 77]);
    }

    #[test]
    fn bc2_explicit_alpha() {
        // Alpha nibbles 0..15 in texel order, then white over black with
        // c0 <= c1, which BC2 still reads in four-colour mode (index 3 opaque).
        let mut block = [0u8; 16];
        for (i, b) in block[..8].iter_mut().enumerate() {
            *b = (2 * i as u8) | (2 * i as u8 + 1) << 4;
        }
        block[8..].copy_from_slice(&[0, 0, 0xff, 0xff, 0xff, 0, 0, 0]);
        let out = decode_format_levels(&block, 4, 4, 1, grid_formats::texture::Format::Bc2).unwrap();
        let alpha: Vec<u8> = out.chunks(4).map(|t| t[3]).collect();
        assert_eq!(alpha, (0..16).map(|n| n * 17).collect::<Vec<u8>>());
        assert_eq!(&out[..3], &[170, 170, 170]);
        assert!(decode_format_levels(&block[..15], 4, 4, 1, grid_formats::texture::Format::Bc2).is_none());
    }
}
