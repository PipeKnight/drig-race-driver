//! Private ETC2 texture cache for GPUs without BC (P8, docs/PLATFORMS_AND_INPUT_RESEARCH.md 7.2).
//! The first load of each BC texture decodes its levels, encodes them as
//! ETC2 RGB8 (or ETC2 RGBA8 with EAC alpha when any texel is not opaque)
//! and writes one KTX2 file. Later loads read that file. Files are a
//! derivative of the user's own installation and stay in
//! `<data dir>/texture-cache` (`local/texture-cache` in the owner's checkout).
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

/// Bump when the encoder output changes, so old cache files are not used.
const ENCODER_VERSION: u32 = 1;

static HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);
static ENCODE_NANOS: AtomicU64 = AtomicU64::new(0);
static TEMP: AtomicU64 = AtomicU64::new(0);

pub(crate) fn dir() -> PathBuf {
    crate::settings::data_dir().join("texture-cache")
}

pub(crate) fn summary() -> String {
    format!(
        "etc2_cache_hits={} etc2_cache_misses={} etc2_encode_ms={:.1}",
        HITS.load(Ordering::Relaxed),
        MISSES.load(Ordering::Relaxed),
        ENCODE_NANOS.load(Ordering::Relaxed) as f64 / 1e6
    )
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// KTX2 bytes of the ETC2 form of one DDS whose BC levels are `bc`.
pub(crate) fn ktx2(
    dds: &[u8],
    bc: &[u8],
    width: u32,
    height: u32,
    levels: u32,
    bc_alpha: bool,
) -> Result<Vec<u8>, String> {
    let name = format!("{:016x}-{}-v{ENCODER_VERSION}.ktx2", fnv1a(dds), dds.len());
    let path = dir().join(name);
    if let Ok(bytes) = std::fs::read(&path) {
        HITS.fetch_add(1, Ordering::Relaxed);
        return Ok(bytes);
    }
    let start = Instant::now();
    let rgba = crate::texture_upload::decode_levels(bc, width, height, levels, bc_alpha)
        .ok_or("truncated BC levels")?;
    let alpha = rgba.as_chunks::<4>().0.iter().any(|t| t[3] != 255);
    let mut encoded = Vec::with_capacity(levels as usize);
    let mut offset = 0;
    for level in 0..levels {
        let w = (width >> level).max(1) as usize;
        let h = (height >> level).max(1) as usize;
        encoded.push(encode_level(&rgba[offset..offset + w * h * 4], w, h, alpha));
        offset += w * h * 4;
    }
    let bytes = write_ktx2(width, height, &encoded, alpha);
    ENCODE_NANOS.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
    MISSES.fetch_add(1, Ordering::Relaxed);
    // Write then rename, so a partly written file is never read.
    let temp = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let written = std::fs::create_dir_all(dir())
        .and_then(|()| std::fs::write(&temp, &bytes))
        .and_then(|()| std::fs::rename(&temp, &path));
    if let Err(error) = written {
        bevy::log::warn!("texture cache {}: {error}", path.display());
        let _ = std::fs::remove_file(&temp);
    }
    Ok(bytes)
}

/// One level, block rows split over the available threads.
fn encode_level(rgba: &[u8], width: usize, height: usize, alpha: bool) -> Vec<u8> {
    let rows = height.div_ceil(4);
    let row_bytes = width.div_ceil(4) * grid_formats::etc2::block_bytes(alpha);
    let mut out = vec![0; rows * row_bytes];
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(rows);
    let per = rows.div_ceil(threads.max(1));
    if threads <= 1 || rows * width < 4096 {
        grid_formats::etc2::encode_level_rows(rgba, width, height, alpha, 0..rows, &mut out);
        return out;
    }
    std::thread::scope(|scope| {
        for (i, chunk) in out.chunks_mut(per * row_bytes).enumerate() {
            let first = i * per;
            let count = chunk.len() / row_bytes;
            scope.spawn(move || {
                grid_formats::etc2::encode_level_rows(
                    rgba,
                    width,
                    height,
                    alpha,
                    first..first + count,
                    chunk,
                );
            });
        }
    });
    out
}

/// KTX2 container (no supercompression) for sRGB ETC2 levels, level 0 first.
pub(crate) fn write_ktx2(width: u32, height: u32, levels: &[Vec<u8>], alpha: bool) -> Vec<u8> {
    // VK_FORMAT_ETC2_R8G8B8_SRGB_BLOCK / VK_FORMAT_ETC2_R8G8B8A8_SRGB_BLOCK.
    let vk_format: u32 = if alpha { 152 } else { 148 };
    let block = grid_formats::etc2::block_bytes(alpha) as u32;
    let samples: &[(u16, u8)] = if alpha {
        &[(0, 15), (64, 2)]
    } else {
        &[(0, 2)]
    };
    let mut dfd = Vec::new();
    let block_size = 24 + 16 * samples.len() as u32;
    dfd.extend((4 + block_size).to_le_bytes());
    dfd.extend(0u32.to_le_bytes()); // vendor 0 (Khronos), basic descriptor type 0
    dfd.extend(2u16.to_le_bytes()); // version 1.3
    dfd.extend((block_size as u16).to_le_bytes());
    // KHR_DF_MODEL_ETC2, BT709 primaries, sRGB transfer, straight alpha.
    dfd.extend([161, 1, 2, 0]);
    dfd.extend([3, 3, 0, 0]); // 4x4x1x1 texel block
    dfd.extend([block as u8, 0, 0, 0, 0, 0, 0, 0]);
    for &(offset, channel) in samples {
        dfd.extend(offset.to_le_bytes());
        dfd.extend([63, channel, 0, 0, 0, 0]);
        dfd.extend(0u32.to_le_bytes());
        dfd.extend(u32::MAX.to_le_bytes());
    }
    let count = levels.len();
    let dfd_offset = 80 + 24 * count;
    let mut out = Vec::new();
    out.extend([
        0xAB, b'K', b'T', b'X', b' ', b'2', b'0', 0xBB, b'\r', b'\n', 0x1A, b'\n',
    ]);
    for value in [vk_format, 1, width, height, 0, 0, 1, count as u32, 0] {
        out.extend(value.to_le_bytes());
    }
    out.extend((dfd_offset as u32).to_le_bytes());
    out.extend((dfd.len() as u32).to_le_bytes());
    out.extend([0u8; 8]); // no key/value data
    out.extend([0u8; 16]); // no supercompression global data
    // Level data from the smallest level up, each 8- or 16-byte aligned.
    let mut at = dfd_offset + dfd.len();
    let mut placed = vec![0usize; count];
    for level in (0..count).rev() {
        at = at.next_multiple_of(block as usize);
        placed[level] = at;
        at += levels[level].len();
    }
    for (level, data) in levels.iter().enumerate() {
        for value in [placed[level], data.len(), data.len()] {
            out.extend((value as u64).to_le_bytes());
        }
    }
    out.extend(&dfd);
    for level in (0..count).rev() {
        out.resize(placed[level], 0);
        out.extend(&levels[level]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        image::{CompressedImageFormats, Image, ImageSampler, ImageType},
        render::render_resource::TextureFormat,
    };

    #[test]
    fn bevy_reads_the_ktx2_container() {
        for alpha in [false, true] {
            let size = grid_formats::etc2::block_bytes(alpha);
            // 8x4 with four levels: 2, 1, 1 and 1 blocks.
            let levels = vec![
                vec![1u8; 2 * size],
                vec![2; size],
                vec![3; size],
                vec![4; size],
            ];
            let bytes = write_ktx2(8, 4, &levels, alpha);
            let image = Image::from_buffer(
                &bytes,
                ImageType::Extension("ktx2"),
                CompressedImageFormats::ETC2,
                true,
                ImageSampler::Default,
                RenderAssetUsages::RENDER_WORLD,
            )
            .unwrap();
            assert_eq!(
                image.texture_descriptor.format,
                if alpha {
                    TextureFormat::Etc2Rgba8UnormSrgb
                } else {
                    TextureFormat::Etc2Rgb8UnormSrgb
                }
            );
            assert_eq!(image.texture_descriptor.mip_level_count, 4);
            assert_eq!(image.texture_descriptor.size.width, 8);
            let data = image.data.unwrap();
            let expected: Vec<u8> = levels.concat();
            assert_eq!(data, expected);
        }
    }

    #[test]
    fn encoding_splits_rows_across_threads_identically() {
        let (w, h) = (64, 128);
        let rgba: Vec<u8> = (0..w * h * 4).map(|i| (i * 7 % 251) as u8).collect();
        let mut single = vec![0; (h / 4) * (w / 4) * 8];
        grid_formats::etc2::encode_level_rows(&rgba, w, h, false, 0..h / 4, &mut single);
        assert_eq!(encode_level(&rgba, w, h, false), single);
    }

    /// Base-level ETC2 quality against the decoded BC source, per texture,
    /// for the Lacetti, one livery and the Donington track and objects.
    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn etc2_quality_on_installed_textures() {
        use grid_formats::texture::Format;
        let root = PathBuf::from(std::env::var("GRID_GAME_ROOT").unwrap());
        let psnr = |se: f64, n: f64| 10.0 * (255.0f64.powi(2) * n / se.max(1e-9)).log10();
        let (mut total_se, mut total_n, mut worst, mut count) =
            (0f64, 0f64, (f64::MAX, String::new()), 0);
        let mut encode_seconds = 0.0;
        for file in [
            "cars/lac/lac_gen_high.pssg",
            "cars/lac/livery_00/textures_high/lac_tex_high_00.pssg",
            "tracks/europe/donington/tracksplit.pssg",
            "tracks/europe/donington/objectstextures.pssg",
        ] {
            let bytes = std::fs::read(root.join(file)).unwrap();
            let tree = grid_formats::pssg::read_tree(&bytes).unwrap();
            for t in grid_formats::texture::read_textures(&tree).unwrap() {
                if t.format == Format::Bgra8 {
                    continue;
                }
                let (w, h) = (t.width as usize, t.height as usize);
                let rgba = crate::texture_upload::decode_levels(
                    t.base_blocks,
                    t.width,
                    t.height,
                    1,
                    t.format == Format::Bc3,
                )
                .unwrap();
                let alpha = rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255);
                let start = Instant::now();
                let mut blocks =
                    vec![0; w.div_ceil(4) * h.div_ceil(4) * grid_formats::etc2::block_bytes(alpha)];
                grid_formats::etc2::encode_level_rows(
                    &rgba,
                    w,
                    h,
                    alpha,
                    0..h.div_ceil(4),
                    &mut blocks,
                );
                encode_seconds += start.elapsed().as_secs_f64();
                let back = grid_formats::etc2::decode_level(&blocks, w, h, alpha);
                let channels = if alpha { 4 } else { 3 };
                let se: f64 = rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(back.as_chunks::<4>().0)
                    .map(|(a, b)| {
                        (0..channels)
                            .map(|c| (f64::from(a[c]) - f64::from(b[c])).powi(2))
                            .sum::<f64>()
                    })
                    .sum();
                let n = (w * h * channels) as f64;
                let value = psnr(se, n);
                let name = format!("{file}#{}", String::from_utf8_lossy(t.id));
                println!("etc2 psnr {value:6.2} dB {w}x{h} alpha={alpha} {name}");
                if value < worst.0 {
                    worst = (value, name);
                }
                total_se += se;
                total_n += n;
                count += 1;
            }
        }
        let overall = psnr(total_se, total_n);
        println!("etc2 base-level encode, single-threaded: {encode_seconds:.2} s");
        println!(
            "etc2 textures={count} overall_psnr={overall:.2} worst={:.2} {}",
            worst.0, worst.1
        );
        assert!(count > 100);
        assert!(overall > 35.0 && worst.0 > 25.0);
    }
}
