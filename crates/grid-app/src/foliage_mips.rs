//! Optional authored alpha-mip coverage adaptation. Source RGB and base alpha stay intact.
use grid_formats::texture::{Format, Texture};
use std::error::Error;

fn adjust(rgba: &mut [u8], width: u32, height: u32, levels: u32) -> usize {
    let base_size = width as usize * height as usize * 4;
    let base_count = base_size / 4;
    let target = rgba[..base_size]
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] >= 128)
        .count() as f64
        / base_count as f64;
    let mut offset = base_size;
    let mut changed = 0;
    for level in 1..levels {
        let count = (width >> level).max(1) as usize * (height >> level).max(1) as usize;
        let pixels = &mut rgba[offset..offset + count * 4];
        let mut histogram = [0usize; 256];
        for p in pixels.as_chunks::<4>().0.iter() {
            histogram[p[3] as usize] += 1;
        }
        let mut tail = [0usize; 256];
        let mut total = 0;
        for i in (0..256).rev() {
            total += histogram[i];
            tail[i] = total;
        }
        let mut best = 128usize;
        let mut error = (tail[best] as f64 / count as f64 - target).abs();
        // Bound the scale to [128/255, 2]. Discrete tiny levels cannot in
        // general preserve coverage exactly; keep source values on equal error.
        for (candidate, &covered) in tail.iter().enumerate().skip(64) {
            let next = (covered as f64 / count as f64 - target).abs();
            if next + 1e-12 < error
                || (next < error + 1e-12 && candidate.abs_diff(128) < best.abs_diff(128))
            {
                best = candidate;
                error = next;
            }
        }
        if best != 128 {
            for p in pixels.as_chunks_mut::<4>().0.iter_mut() {
                p[3] = (usize::from(p[3]) * 128 / best).min(255) as u8;
            }
            changed += 1;
        }
        offset += count * 4;
    }
    changed
}

/// Only called for explicitly selected leaf diffuse BC3 images; never normal/data maps.
/// Output still goes through texture_upload::image after device format selection.
pub fn prepare(source: &Texture<'_>) -> Result<(Vec<u8>, usize), Box<dyn Error>> {
    let (dds, levels) = source.validated_dds();
    if source.format != Format::Bc3 || levels <= 1 {
        return Ok((dds, 0));
    }
    if !source.width.is_power_of_two()
        || !source.height.is_power_of_two()
        || source.width > 4096
        || source.height > 4096
        || levels > 13
    {
        return Err("foliage coverage requires bounded power-of-two BC3 mips".into());
    }
    let mut rgba = crate::texture_upload::decode_levels(
        &dds[128..],
        source.width,
        source.height,
        levels,
        true,
    )
    .ok_or("invalid foliage mip payload")?;
    let changed = adjust(&mut rgba, source.width, source.height, levels);
    if changed == 0 {
        return Ok((dds, 0));
    }
    for p in rgba.as_chunks_mut::<4>().0.iter_mut() {
        p.swap(0, 2);
    }
    let base = source.width as usize * source.height as usize * 4;
    let result = Texture {
        id: source.id,
        width: source.width,
        height: source.height,
        format: Format::Bgra8,
        declared_mip_levels: levels - 1,
        base_blocks: &rgba[..base],
        remaining_data: &rgba[base..],
    }
    .validated_dds();
    if result.1 != levels {
        return Err("foliage mip envelope lost levels".into());
    }
    Ok((result.0, changed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minified_leaf_coverage_recovers_without_touching_base_or_rgb() {
        let mut data = Vec::new();
        for i in 0..16 {
            data.extend_from_slice(&[11, 22, 33, if i < 8 { 255 } else { 0 }]);
        }
        for a in [96, 96, 0, 0, 48] {
            data.extend_from_slice(&[44, 55, 66, a]);
        }
        let original = data.clone();
        assert_eq!(adjust(&mut data, 4, 4, 3), 1);
        assert_eq!(&data[..64], &original[..64]);
        assert_eq!(data[67], 128);
        assert_eq!(data[75], 0);
        assert_eq!(data[83], 48); // A one-texel level cannot represent 50% binary coverage.
        for (a, b) in data
            .as_chunks::<4>()
            .0
            .iter()
            .zip(original.as_chunks::<4>().0.iter())
        {
            assert_eq!(&a[..3], &b[..3]);
            if b[3] == 0 {
                assert_eq!(a[3], 0);
            }
        }
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_maple_keeps_base_and_decoded_rgb_and_full_mips() {
        use bevy::{asset::RenderAssetUsages, image::ImageSampler};
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let bytes = std::fs::read(
            std::path::Path::new(&root).join("tracks/europe/donington/treestextures.pssg"),
        )
        .unwrap();
        let tree = grid_formats::pssg::read_tree(&bytes).unwrap();
        let textures = grid_formats::texture::read_named(&tree, &[b"maple_green_d.tga"]).unwrap();
        let source = &textures[0];
        let (old, levels) = source.validated_dds();
        let old = crate::texture_upload::decode_levels(
            &old[128..],
            source.width,
            source.height,
            levels,
            true,
        )
        .unwrap();
        let (new, changed) = prepare(source).unwrap();
        assert!(changed > 0);
        let image = crate::texture_upload::image(
            &new,
            ImageSampler::Default,
            RenderAssetUsages::MAIN_WORLD,
        )
        .unwrap();
        assert_eq!(image.texture_descriptor.mip_level_count, levels);
        let mut decoded = image.data.unwrap();
        assert_eq!(decoded.len(), old.len());
        for p in decoded.as_chunks_mut::<4>().0.iter_mut() {
            p.swap(0, 2);
        }
        let base = source.width as usize * source.height as usize * 4;
        assert_eq!(&decoded[..base], &old[..base]);
        for (a, b) in decoded
            .as_chunks::<4>()
            .0
            .iter()
            .zip(old.as_chunks::<4>().0.iter())
        {
            assert_eq!(&a[..3], &b[..3]);
        }
    }
}
