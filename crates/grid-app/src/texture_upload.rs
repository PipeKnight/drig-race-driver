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
//! legacy `GRID_TEXTURE_FALLBACK` takes the same values.
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
    let path = PATH.load(Ordering::Relaxed);
    if path != BC {
        let start = Instant::now();
        let etc2 = if path == ETC2 {
            etc2_image(dds, &image).unwrap_or_else(|error| {
                warn!("ETC2 texture cache failed, using RGBA8: {error}");
                None
            })
        } else {
            None
        };
        if let Some(etc2) = etc2 {
            image = etc2;
            DECODED.fetch_add(1, Ordering::Relaxed);
        } else if decode_in_place(&mut image)? {
            DECODED.fetch_add(1, Ordering::Relaxed);
        }
        DECODE_NANOS.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
    IMAGES.fetch_add(1, Ordering::Relaxed);
    SOURCE_BYTES.fetch_add(source, Ordering::Relaxed);
    UPLOAD_BYTES.fetch_add(
        image.data.as_ref().map_or(0, Vec::len) as u64,
        Ordering::Relaxed,
    );
    Ok(image)
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
        "texture upload summary: path={} images={} decoded={} source_bytes={} upload_bytes={} decode_ms={:.1} {}",
        ["bc", "etc2", "rgba8"][usize::from(PATH.load(Ordering::Relaxed))],
        IMAGES.load(Ordering::Relaxed),
        DECODED.load(Ordering::Relaxed),
        SOURCE_BYTES.load(Ordering::Relaxed),
        UPLOAD_BYTES.load(Ordering::Relaxed),
        DECODE_NANOS.load(Ordering::Relaxed) as f64 / 1e6,
        crate::texture_cache::summary()
    )
}

/// Replaces BC1/BC3 levels by RGBA8 levels; other formats are kept.
fn decode_in_place(image: &mut Image) -> Result<bool, TextureError> {
    let (decoded, alpha) = match image.texture_descriptor.format {
        TextureFormat::Bc1RgbaUnormSrgb => (TextureFormat::Rgba8UnormSrgb, false),
        TextureFormat::Bc1RgbaUnorm => (TextureFormat::Rgba8Unorm, false),
        TextureFormat::Bc3RgbaUnormSrgb => (TextureFormat::Rgba8UnormSrgb, true),
        TextureFormat::Bc3RgbaUnorm => (TextureFormat::Rgba8Unorm, true),
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
    let out = decode_levels(&data, size.width, size.height, levels, alpha)
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
    let block = if alpha { 16 } else { 8 };
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
                let texels = if alpha {
                    let mut t = colour_block(bytes[8..].try_into().unwrap(), true);
                    for (texel, a) in t
                        .iter_mut()
                        .zip(alpha_block(bytes[..8].try_into().unwrap()))
                    {
                        texel[3] = a;
                    }
                    t
                } else {
                    colour_block(bytes.try_into().unwrap(), false)
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
}
