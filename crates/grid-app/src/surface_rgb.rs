//! Explicit surface RGB derivatives; alpha always comes from the purchased map.
//! CPU-only preparation. Device-dependent uploads remain in texture_upload.
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    path::Path,
};

pub fn apply(
    manifest: &Path,
    textures: &mut BTreeMap<String, Vec<u8>>,
    used: &BTreeSet<String>,
) -> Result<(), Box<dyn Error>> {
    super::texture_overrides::apply_with(
        manifest,
        textures,
        used,
        "GRID_SURFACE_RGB_OVERRIDES_V1",
        Some(retain_alpha),
    )?;
    Ok(())
}

struct Alpha {
    width: usize,
    height: usize,
    bytes: Vec<u8>,
}

// Pixel centres, periodic addressing and one final integer rounding. This
// authors the extra close-up levels, never neural-generates lighting masks.
fn enlarged(a: &Alpha, scale: usize) -> Vec<u8> {
    let d = (scale * 2) as i64;
    let mut out = vec![0; a.width * a.height * scale * scale];
    let sample = |x: i64, y: i64| -> i64 {
        i64::from(
            a.bytes[y.rem_euclid(a.height as i64) as usize * a.width
                + x.rem_euclid(a.width as i64) as usize],
        )
    };
    for y in 0..a.height * scale {
        let n = 2 * y as i64 + 1 - scale as i64;
        let (y0, fy) = (n.div_euclid(d), n.rem_euclid(d));
        for x in 0..a.width * scale {
            let n = 2 * x as i64 + 1 - scale as i64;
            let (x0, fx) = (n.div_euclid(d), n.rem_euclid(d));
            let top = sample(x0, y0) * (d - fx) + sample(x0 + 1, y0) * fx;
            let bottom = sample(x0, y0 + 1) * (d - fx) + sample(x0 + 1, y0 + 1) * fx;
            out[y * a.width * scale + x] =
                ((top * (d - fy) + bottom * fy + d * d / 2) / (d * d)) as u8;
        }
    }
    out
}

fn smaller(a: &Alpha) -> Alpha {
    let (w, h) = ((a.width / 2).max(1), (a.height / 2).max(1));
    let (dx, dy) = (
        if a.width > 1 { 2 } else { 1 },
        if a.height > 1 { 2 } else { 1 },
    );
    let mut bytes = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0usize;
            for j in 0..dy {
                for i in 0..dx {
                    sum += usize::from(a.bytes[(y * dy + j) * a.width + x * dx + i]);
                }
            }
            bytes.push(((sum + dx * dy / 2) / (dx * dy)) as u8);
        }
    }
    Alpha {
        width: w,
        height: h,
        bytes,
    }
}

fn retain_alpha(original: &[u8], candidate: &mut [u8]) -> Result<(), Box<dyn Error>> {
    let (cw, ch, levels) = super::texture_overrides::validate(candidate)?;
    // Reuse the bounded BC3/mip validator and the renderer's canonical CPU
    // decoder. Only alpha is kept, so its RGBA-to-BGRA colour order is irrelevant.
    let mask = super::body_occlusion::derive_with(original, |p| [255, 255, 255, p[3]])
        .map_err(|e| format!("surface source alpha: {e}"))?;
    let u = |at| u32::from_le_bytes(mask[at..at + 4].try_into().unwrap());
    let (sw, sh, source_levels) = (u(16), u(12), u(28));
    let shift = match (cw / sw, ch / sh) {
        (2, 2) if cw == sw * 2 && ch == sh * 2 => 1,
        (4, 4) if cw == sw * 4 && ch == sh * 4 => 2,
        _ => return Err("surface RGB override requires exact 2x or 4x source dimensions".into()),
    };
    let mut alphas = Vec::new();
    let mut offset = 128;
    for i in 0..source_levels {
        let (w, h) = ((sw >> i).max(1) as usize, (sh >> i).max(1) as usize);
        let size = w * h * 4;
        alphas.push(Alpha {
            width: w,
            height: h,
            bytes: mask[offset..offset + size]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect(),
        });
        offset += size;
    }
    assert_eq!(offset, mask.len());
    while alphas.last().is_some_and(|a| a.width > 1 || a.height > 1) {
        alphas.push(smaller(alphas.last().unwrap()));
    }
    offset = 128;
    for i in 0..levels {
        let (w, h) = ((cw >> i).max(1) as usize, (ch >> i).max(1) as usize);
        let high;
        let alpha = if i < shift {
            high = enlarged(&alphas[0], 1usize << (shift - i));
            high.as_slice()
        } else {
            alphas[(i - shift) as usize].bytes.as_slice()
        };
        if alpha.len() != w * h {
            return Err("surface source alpha mip dimensions differ".into());
        }
        for (p, a) in candidate[offset..offset + w * h * 4]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(alpha)
        {
            p[3] = *a;
        }
        offset += w * h * 4;
    }
    assert_eq!(offset, candidate.len());
    println!(
        "surface_rgb source_alpha_mips={source_levels} derived_large_levels={shift} alpha_decode=shared-BC3 RGB=unchanged source_GPU_bit_parity=unverified"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_formats::texture::{Format, Texture};
    fn rgb(w: u32, h: u32) -> Vec<u8> {
        let n = w.max(h).ilog2() + 1;
        let size: usize = (0..n)
            .map(|i| (w >> i).max(1) as usize * (h >> i).max(1) as usize * 4)
            .sum();
        let mut p = vec![0; size];
        for (i, c) in p.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            *c = [i as u8, 37, 189, 255];
        }
        let base = (w * h * 4) as usize;
        Texture {
            id: b"test",
            width: w,
            height: h,
            format: Format::Bgra8,
            declared_mip_levels: n - 1,
            base_blocks: &p[..base],
            remaining_data: &p[base..],
        }
        .validated_dds()
        .0
    }
    fn source() -> Vec<u8> {
        let mut blocks = vec![0u8; 48];
        for (i, b) in blocks.as_chunks_mut::<16>().0.iter_mut().enumerate() {
            b[0] = [0, 93, 217][i];
            b[1] = b[0];
        }
        Texture {
            id: b"test",
            width: 4,
            height: 4,
            format: Format::Bc3,
            declared_mip_levels: 2,
            base_blocks: &blocks[..16],
            remaining_data: &blocks[16..],
        }
        .validated_dds()
        .0
    }
    #[test]
    fn restores_authored_mips_and_preserves_rgb_without_accepting_packed_candidates() {
        let original = source();
        let mut candidate = rgb(16, 16);
        let before = candidate.clone();
        retain_alpha(&original, &mut candidate).unwrap();
        assert!(candidate[..128] == before[..128]);
        assert!(
            candidate[128..]
                .as_chunks::<4>()
                .0
                .iter()
                .zip(before[128..].as_chunks::<4>().0)
                .all(|(a, b)| a[..3] == b[..3])
        );
        let mut offset = 128;
        for (level, alpha) in [0, 0, 0, 93, 217].into_iter().enumerate() {
            let size = (16usize >> level).max(1).pow(2) * 4;
            assert!(
                candidate[offset..offset + size]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[3] == alpha)
            );
            offset += size;
        }
        assert!(super::super::texture_overrides::validate(&candidate).is_err());
        assert!(retain_alpha(&original, &mut rgb(16, 8)).is_err());
        assert!(retain_alpha(&original, &mut rgb(32, 32)).is_err());
        let mut bad = original.clone();
        bad.pop();
        assert!(retain_alpha(&bad, &mut rgb(16, 16)).is_err());
    }
    #[test]
    fn transform_failure_in_later_row_keeps_all_purchased_cpu_maps() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };
        let private = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../local")
            .canonicalize()
            .unwrap();
        let dir = private.join(format!(
            "surface-rgb-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        assert!(dir.canonicalize().unwrap().starts_with(&private));
        let original = source();
        let mut bad = source();
        bad.push(0);
        fs::write(dir.join("source.dds"), &original).unwrap();
        fs::write(dir.join("bad.dds"), &bad).unwrap();
        fs::write(dir.join("rgb.dds"), rgb(16, 16)).unwrap();
        let manifest = dir.join("rgb.tsv");
        fs::write(
            &manifest,
            "GRID_SURFACE_RGB_OVERRIDES_V1\nroad\tsource.dds\trgb.dds\nother\tbad.dds\trgb.dds\n",
        )
        .unwrap();
        let mut maps = BTreeMap::from([("road".into(), original), ("other".into(), bad)]);
        let before = maps.clone();
        let used = BTreeSet::from(["road".into(), "other".into()]);
        assert!(
            super::super::texture_overrides::apply_in(
                &manifest,
                &mut maps,
                &used,
                "GRID_SURFACE_RGB_OVERRIDES_V1",
                Some(retain_alpha),
                &private
            )
            .is_err()
        );
        assert_eq!(maps, before);
        // This is the unique directory created by this test, not user content.
        fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    #[ignore = "requires owner's purchased GRID installation"]
    fn purchased_curb_rgb_keeps_each_authored_source_alpha_mip() {
        use grid_formats::{pssg, texture};
        let root = std::env::var_os("GRID_GAME_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from("C:/Program Files (x86)/Steam/steamapps/common/Grid")
            });
        let bytes = std::fs::read(root.join("tracks/europe/donington/tracksplit.pssg")).unwrap();
        let tree = pssg::read_tree(&bytes).unwrap();
        let maps = texture::read_named(&tree, &[b"don_rumble_strip_03_d.tga"]).unwrap();
        assert_eq!(maps.len(), 1);
        let (original, n) = maps[0].validated_dds();
        assert_eq!((maps[0].width, maps[0].height, n), (256, 512, 10));
        let expected =
            super::super::texture_upload::decode_levels(&original[128..], 256, 512, 10, true)
                .unwrap();
        let mut candidate = rgb(1024, 2048);
        retain_alpha(&original, &mut candidate).unwrap();
        let (mut at, mut source_at) = (128, 0);
        for i in 0..12 {
            let (w, h) = ((1024usize >> i).max(1), (2048usize >> i).max(1));
            if i >= 2 {
                assert!(
                    candidate[at..at + w * h * 4]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .zip(
                            expected[source_at..source_at + w * h * 4]
                                .as_chunks::<4>()
                                .0
                        )
                        .all(|(a, b)| a[3] == b[3])
                );
                source_at += w * h * 4;
            }
            at += w * h * 4;
        }
        assert_eq!(source_at, expected.len());
        assert_eq!(at, candidate.len());
    }
    #[test]
    fn periodic_pixel_centres_and_missing_tail_are_bounded() {
        let a = Alpha {
            width: 2,
            height: 1,
            bytes: vec![0, 255],
        };
        assert_eq!(enlarged(&a, 2), vec![64, 64, 191, 191, 64, 64, 191, 191]);
        assert_eq!(smaller(&a).bytes, vec![128]);
        let a = Alpha {
            width: 1,
            height: 1,
            bytes: vec![177],
        };
        assert_eq!(enlarged(&a, 4), vec![177; 16]);
    }
}
