//! Explicit private diffuse candidates. CPU byte replacement only; the existing
//! texture_upload::image still owns every device-dependent image upload.
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::Path,
};

#[derive(bevy::prelude::Resource)]
pub struct Active(pub BTreeSet<String>);

fn read_private(path: &Path, private: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn Error>> {
    let path = path.canonicalize()?;
    if !path.starts_with(private) || !path.is_file() || path.metadata()?.len() > limit {
        return Err(
            "texture override file must stay within the private data directory and size bound"
                .into(),
        );
    }
    Ok(fs::read(path)?)
}

/// Version1 intentionally accepts only the writer's complete opaque BGRA8 DDS.
/// This avoids another image decoder/upload before GPU feature selection.
pub(crate) fn validate(bytes: &[u8]) -> Result<(u32, u32, u32), &'static str> {
    if bytes.len() < 128 || &bytes[..4] != b"DDS " {
        return Err("override is not a DDS envelope");
    }
    let u = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    for (offset, expected) in [
        (4, 124),
        (8, 0x2100f),
        (76, 32),
        (80, 0x41),
        (84, 0),
        (88, 32),
        (92, 0xff0000),
        (96, 0xff00),
        (100, 0xff),
        (104, 0xff000000),
        (108, 0x401008),
        (24, 0),
        (112, 0),
        (116, 0),
        (120, 0),
        (124, 0),
    ] {
        if u(offset) != expected {
            return Err("override v1 requires complete opaque BGRA8 mip envelope");
        }
    }
    let (width, height, levels) = (u(16), u(12), u(28));
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || !width.is_power_of_two()
        || !height.is_power_of_two()
        || u(20) != width * 4
    {
        return Err("invalid override texture dimensions or pitch");
    }
    if levels != width.max(height).ilog2() + 1 {
        return Err("override mip chain must be complete");
    }
    let (mut w, mut h, mut expected) = (width, height, 128u64);
    for _ in 0..levels {
        expected += u64::from(w) * u64::from(h) * 4;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    if expected != bytes.len() as u64 {
        return Err("override mip payload size mismatch");
    }
    if bytes[128..].as_chunks::<4>().0.iter().any(|p| p[3] != 255) {
        return Err("override v1 is opaque diffuse only");
    }
    Ok((width, height, levels))
}

pub fn apply(
    manifest: &Path,
    textures: &mut BTreeMap<String, Vec<u8>>,
    used: &BTreeSet<String>,
) -> Result<BTreeSet<String>, Box<dyn Error>> {
    apply_with(manifest, textures, used, "GRID_TEXTURE_OVERRIDES_V1", None)
}

pub(crate) type ChannelTransform = fn(&[u8], &mut [u8]) -> Result<(), Box<dyn Error>>;

/// Shared path/source/batch guards. The transform runs only after opaque RGB
/// validation; every row is staged before any purchased CPU texture is replaced.
pub(crate) fn apply_with(
    manifest: &Path,
    textures: &mut BTreeMap<String, Vec<u8>>,
    used: &BTreeSet<String>,
    version: &str,
    transform: Option<ChannelTransform>,
) -> Result<BTreeSet<String>, Box<dyn Error>> {
    apply_in(
        manifest,
        textures,
        used,
        version,
        transform,
        super::settings::data_dir(),
    )
}

/// Explicit root keeps filesystem tests independent of the process-wide profile.
pub(crate) fn apply_in(
    manifest: &Path,
    textures: &mut BTreeMap<String, Vec<u8>>,
    used: &BTreeSet<String>,
    version: &str,
    transform: Option<ChannelTransform>,
    private: &Path,
) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let private = private.canonicalize()?;
    let manifest = manifest.canonicalize()?;
    let text = String::from_utf8(read_private(&manifest, &private, 64 * 1024)?)?;
    let mut lines = text.lines();
    if lines.next().map(|s| s.trim_start_matches('\u{feff}')) != Some(version) {
        return Err("unknown texture override manifest version".into());
    }
    let parent = manifest
        .parent()
        .ok_or("override manifest parent missing")?;
    let mut staged = Vec::new();
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for (line_index, line) in lines.enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 || fields.iter().any(|s| s.is_empty()) || staged.len() >= 64 {
            return Err(format!("invalid/bounded override row{}", line_index + 2).into());
        }
        let key = fields[0];
        if !seen.insert(key.to_string()) || !used.contains(key) {
            return Err(format!("duplicate or unbound diffuse override key {key}").into());
        }
        let original = read_private(&parent.join(fields[1]), &private, 512 * 1024 * 1024)?;
        if textures.get(key).is_none_or(|bytes| bytes != &original) {
            return Err(format!(
                "override source DDS does not exactly match purchased texture {key}"
            )
            .into());
        }
        let mut candidate = read_private(&parent.join(fields[2]), &private, 512 * 1024 * 1024)?;
        let (w, h, levels) = validate(&candidate)?;
        if let Some(convert) = transform {
            convert(&original, &mut candidate)?;
        }
        total += candidate.len() as u64;
        if total > 1024 * 1024 * 1024 {
            return Err("override batch exceeds v1 byte bound".into());
        }
        staged.push((key.to_string(), candidate, w, h, levels));
    }
    if staged.is_empty() {
        return Err("texture override manifest has no entries".into());
    }
    // No mutation until every original, path and derivative has been checked.
    let count = staged.len();
    for (key, candidate, w, h, levels) in staged {
        println!(
            "texture_override key={key:?} source_match=exact-bytes format=BGRA8srgb width={w} height={h} mips={levels} bytes={} upload=shared contract={version}",
            candidate.len()
        );
        textures.insert(key, candidate);
    }
    println!(
        "texture_overrides active={count} bytes={total} manifest={} Steam_writes=false",
        manifest.display()
    );
    Ok(seen)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Vec<u8> {
        let mut d = vec![0u8; 128 + 44];
        d[..4].copy_from_slice(b"DDS ");
        for (o, v) in [
            (4, 124u32),
            (8, 0x2100f),
            (12, 2),
            (16, 4),
            (20, 16),
            (28, 3),
            (76, 32),
            (80, 0x41),
            (88, 32),
            (92, 0xff0000),
            (96, 0xff00),
            (100, 0xff),
            (104, 0xff000000),
            (108, 0x401008),
            (24, 0),
            (112, 0),
            (116, 0),
            (120, 0),
            (124, 0),
        ] {
            d[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        for pixel in d[128..].as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&[30, 20, 10, 255]);
        }
        d
    }
    #[test]
    fn rejects_partial_mips_bad_channel_layout_and_changed_alpha() {
        let source = sample();
        assert_eq!(validate(&source), Ok((4, 2, 3)));
        for (offset, value) in [(28, 2u32), (16, 3), (92, 0xff), (112, 0x200)] {
            let mut d = source.clone();
            d[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(validate(&d).is_err());
        }
        let mut d = source.clone();
        d.pop();
        assert!(validate(&d).is_err());
        let mut d = source.clone();
        d.push(0);
        assert!(validate(&d).is_err());
        let mut d = source;
        d[131] = 0;
        assert!(validate(&d).is_err());
    }
}
