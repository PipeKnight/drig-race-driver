//! The drivers' suit and helmet colours (docs/COCKPIT.md "Suit tint").
//!
//! 0x861C50 (player) and 0x862300 (opponents) set `tintColour1..3` on the
//! driver's body and head shaders from the car's livery:
//! `driver_suit[vehicle_livery.driver_suit_id].tint1..3`, "R,G,B" in 0..255
//! (0x60E310, parsed by 0x5F5240). The pixel programs (`fp_new_body`,
//! `fp_new_head`) multiply the diffuse by
//! saturate(t1·m.r + t2·m.g + t3·m.b + saturate(1 − (m.r + m.g + m.b))) with
//! m the TTintColourMask texel, in gamma space; specular and fog come after,
//! untinted. The port bakes that product into the diffuse texture once per
//! suit, which is the same in gamma space.

use grid_formats::texture::{Format, Texture};
use std::{error::Error, path::Path};

/// tint1..3, RGB 0..255.
pub type Tints = [[u8; 3]; 3];

/// A texture's decoded mip chain: (width, height, RGBA8) per level.
pub type Levels = Vec<(u32, u32, Vec<u8>)>;

/// The suit of each livery `file_string` (`lac_00`): `None` when the livery
/// or its suit is not in the database.
pub fn read(root: &Path, liveries: &[String]) -> Result<Vec<Option<Tints>>, Box<dyn Error>> {
    let db = grid_tools::read_database(root)?;
    let table = |name: &str| db.table(name).ok_or_else(|| format!("database table {name} absent"));
    let (liveries_table, suits) = (table("vehicle_livery")?, table("driver_suit")?);
    Ok(liveries
        .iter()
        .map(|file| {
            let id = liveries_table.rows().find(|r| r.text("file_string") == Some(file.as_str()))?.int("driver_suit_id")?;
            let suit = suits.rows().find(|r| r.int("id") == Some(id))?;
            Some([parse(suit.text("tint1")?)?, parse(suit.text("tint2")?)?, parse(suit.text("tint3")?)?])
        })
        .collect())
}

/// "R,G,B" in 0..255.
fn parse(text: &str) -> Option<[u8; 3]> {
    let mut parts = text.split(',').map(|p| p.trim().parse::<u8>().ok());
    let rgb = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(rgb)
}

/// The tinted diffuse texel (RGBA8, alpha kept) under mask texel `m`.
pub fn tint(diffuse: [u8; 4], m: [u8; 4], tints: &Tints) -> [u8; 4] {
    let m = [m[0], m[1], m[2]].map(|c| c as f32 / 255.);
    let rest = (1. - (m[0] + m[1] + m[2])).clamp(0., 1.);
    let mut out = diffuse;
    for c in 0..3 {
        let t = |k: usize| tints[k][c] as f32 / 255.;
        let factor = (t(0) * m[0] + t(1) * m[1] + t(2) * m[2] + rest).clamp(0., 1.);
        out[c] = (diffuse[c] as f32 * factor).round() as u8;
    }
    out
}

/// A texture's mip chain as RGBA8 (BC1, BC3 or BGRA8).
pub fn decode(t: &Texture<'_>) -> Option<Levels> {
    let levels = t.declared_mip_levels + 1;
    let data = [t.base_blocks, t.remaining_data].concat();
    let pixels = match t.format {
        Format::Bc1 => crate::texture_upload::decode_levels(&data, t.width, t.height, levels, false)?,
        Format::Bc3 => crate::texture_upload::decode_levels(&data, t.width, t.height, levels, true)?,
        Format::Bgra8 => data.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect(),
    };
    let mut out = Vec::new();
    let mut offset = 0;
    for level in 0..levels {
        let (w, h) = ((t.width >> level).max(1), (t.height >> level).max(1));
        let size = (w * h * 4) as usize;
        out.push((w, h, pixels.get(offset..offset + size)?.to_vec()));
        offset += size;
    }
    Some(out)
}

/// `diffuse` tinted by `mask` per mip level, as a BGRA8 DDS. A mask level of
/// another size is sampled at the nearest texel of its base level.
pub fn tinted_dds(diffuse: &Levels, mask: &Levels, tints: &Tints) -> Vec<u8> {
    let mut bgra = Vec::new();
    for (w, h, texels) in diffuse {
        let (mw, mh, m) = mask.iter().find(|l| (l.0, l.1) == (*w, *h)).unwrap_or(&mask[0]);
        for y in 0..*h {
            for x in 0..*w {
                let at = ((y * w + x) * 4) as usize;
                let (sx, sy) = ((x * mw / w).min(mw - 1), (y * mh / h).min(mh - 1));
                let mt = ((sy * mw + sx) * 4) as usize;
                let p = tint(texels[at..at + 4].try_into().unwrap(), m[mt..mt + 4].try_into().unwrap(), tints);
                bgra.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
    }
    let (w, h, base) = &diffuse[0];
    let base_size = base.len();
    Texture {
        id: b"driver_suit",
        width: *w,
        height: *h,
        format: Format::Bgra8,
        declared_mip_levels: diffuse.len() as u32 - 1,
        base_blocks: &bgra[..base_size],
        remaining_data: &bgra[base_size..],
    }
    .validated_dds()
    .0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_channels_pick_the_tints() {
        let nevada: Tints = [[201, 0, 44], [0, 0, 0], [0, 0, 0]];
        // Red mask: diffuse × tint1; green: × tint2 (black); none: unchanged.
        assert_eq!(tint([200, 200, 200, 255], [255, 0, 0, 0], &nevada), [158, 0, 35, 255]);
        assert_eq!(tint([200, 200, 200, 7], [0, 255, 0, 0], &nevada), [0, 0, 0, 7]);
        assert_eq!(tint([200, 100, 50, 255], [0, 0, 0, 0], &nevada), [200, 100, 50, 255]);
        // Half red: half the tint plus half the remainder.
        assert_eq!(tint([200, 200, 200, 255], [128, 0, 0, 0], &nevada)[1], 100);
        assert_eq!(parse("201,0,44"), Some([201, 0, 44]));
        assert_eq!(parse("1,2"), None);
    }

    /// The shipped suits (GRID_GAME_ROOT): Lacetti livery 00 is "nevada".
    #[test]
    #[ignore]
    fn liveries_name_their_suits() {
        let root = std::path::PathBuf::from(std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        let suits = read(&root, &["lac_00".into(), "bmw_00".into(), "nope_00".into()]).unwrap();
        assert_eq!(suits[0], Some([[201, 0, 44], [0, 0, 0], [0, 0, 0]]));
        assert_eq!(suits[1], Some([[255; 3]; 3]));
        assert_eq!(suits[2], None);
    }
}
