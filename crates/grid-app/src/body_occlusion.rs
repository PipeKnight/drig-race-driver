//! Opt-in ambient-only adaptation of the source paint mask's green channel.
//! Original shaders also weight direct/reflected light; this is not their full model.
use bevy::{asset::RenderAssetUsages, image::ImageSampler, prelude::*};
use grid_formats::texture::{Format, Texture};
use std::{collections::BTreeMap, error::Error};

#[derive(Default, Resource)]
pub struct Prepared(BTreeMap<usize, Vec<u8>>);

impl Prepared {
    pub(super) fn drain(&mut self) -> BTreeMap<usize, Vec<u8>> {
        std::mem::take(&mut self.0)
    }
}

type MaskResult = Result<Vec<u8>, Box<dyn Error>>;

fn derive(dds: &[u8]) -> MaskResult {
    derive_with(dds, |p| [p[1], p[1], p[1], 255])
}

/// Shared bounded DDS validation; conversions preserve each authored mip.
pub(super) fn derive_with(dds: &[u8], convert: fn([u8; 4]) -> [u8; 4]) -> MaskResult {
    if dds.len() < 128 || &dds[..4] != b"DDS " {
        return Err("source paint mask requires a classic BC3 DDS".into());
    }
    let word = |i| u32::from_le_bytes(dds[i..i + 4].try_into().unwrap());
    let (w, h, levels) = (word(16), word(12), word(28));
    if word(4) != 124
        || word(76) != 32
        || word(80) != 4
        || &dds[84..88] != b"DXT5"
        || !w.is_power_of_two()
        || !h.is_power_of_two()
        || w > 4096
        || h > 4096
        || levels == 0
        || levels > w.max(h).ilog2() + 1
    {
        return Err("unsupported source paint mask dimensions/format/mips".into());
    }
    let expected: usize = (0..levels)
        .map(|i| Format::Bc3.level_size((w >> i).max(1), (h >> i).max(1)))
        .sum();
    if dds.len() != 128 + expected {
        return Err("source paint mask rejects truncated or trailing DDS data".into());
    }
    let mut rgba = super::texture_upload::decode_levels(&dds[128..], w, h, levels, true)
        .ok_or("source paint mask BC3 decode failed")?;
    for p in rgba.as_chunks_mut::<4>().0 {
        *p = convert(*p);
    }
    let base = w as usize * h as usize * 4;
    let (out, actual_levels) = Texture {
        id: b"source_paint_mask",
        width: w,
        height: h,
        format: Format::Bgra8,
        declared_mip_levels: levels - 1,
        base_blocks: &rgba[..base],
        remaining_data: &rgba[base..],
    }
    .validated_dds();
    if actual_levels != levels {
        return Err("source paint mask mip envelope mismatch".into());
    }
    Ok(out)
}

/// Stage each vehicle's own livery mask; never reuse the player's mask on peers.
pub fn prepare(car: &super::Car, field: &super::Field) -> Result<Prepared, Box<dyn Error>> {
    prepare_with(car, field, derive)
}

pub(super) fn prepare_with(
    car: &super::Car,
    field: &super::Field,
    convert: fn(&[u8]) -> MaskResult,
) -> Result<Prepared, Box<dyn Error>> {
    let mut staged = BTreeMap::new();
    for (index, &model) in field.car_model.iter().enumerate() {
        let model = field
            .models
            .get(model)
            .ok_or("source paint mask model missing")?;
        let prefix = if car.track {
            super::model_prefix(field.car_model[index])
        } else {
            String::new()
        };
        let key = format!("{prefix}{}", model.texture("main_m1"));
        let bank = if index == 0 {
            &car.textures
        } else {
            car.liveries
                .get(index - 1)
                .ok_or("source paint mask opponent bank missing")?
        };
        let dds = bank
            .get(&key)
            .ok_or_else(|| format!("source paint mask own mask absent: car={index} key={key}"))?;
        staged.insert(index, convert(dds)?);
    }
    if staged.is_empty() {
        return Err("source paint mask requires a car scene".into());
    }
    Ok(Prepared(staged))
}

pub fn upload(
    prepared: &mut Prepared,
    images: &mut Assets<Image>,
    sampler: ImageSampler,
) -> BTreeMap<usize, Handle<Image>> {
    prepared.drain().into_iter().map(|(car, dds)| {
        let image = super::texture_upload::image_linear(&dds, sampler.clone(), RenderAssetUsages::default())
            .expect("validated source paint mask DDS through shared upload");
        println!("body_occlusion car={car} channel=green srgb=false format={:?} mips={} scope=authored-ambient-only source_shader_parity=false", image.texture_descriptor.format, image.texture_descriptor.mip_level_count);
        (car, images.add(image))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> Vec<u8> {
        let mut blocks = Vec::new();
        // Alpha 17 is deliberately different from green. Two distinct colour mips.
        for colour in [0x07e0u16, 0xf800, 0x001f] {
            let mut block = [0u8; 16];
            block[0] = 17;
            block[1] = 17;
            block[8..10].copy_from_slice(&colour.to_le_bytes());
            blocks.extend_from_slice(&block);
        }
        Texture {
            id: b"mask",
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
    fn selects_green_without_alpha_or_srgb_and_preserves_mips() {
        let input = source();
        let before = input.clone();
        let output = derive(&input).unwrap();
        assert_eq!(input, before);
        assert_eq!(u32::from_le_bytes(output[28..32].try_into().unwrap()), 3);
        assert_eq!(&output[128..132], &[255, 255, 255, 255]);
        assert_eq!(&output[128 + 64..128 + 68], &[0, 0, 0, 255]);
        assert_eq!(&output[128 + 80..128 + 84], &[0, 0, 0, 255]);
        assert_eq!(output.len(), 128 + 84);
    }
    #[test]
    fn rejects_bad_envelopes_before_publication() {
        let input = source();
        for i in [0, 4, 76, 80, 84] {
            let mut bad = input.clone();
            bad[i] ^= 1;
            assert!(derive(&bad).is_err());
        }
        assert!(derive(&input[..input.len() - 1]).is_err());
        let mut bad = input.clone();
        bad.push(0);
        assert!(derive(&bad).is_err());
        let mut bad = input;
        bad[28..32].copy_from_slice(&8u32.to_le_bytes());
        assert!(derive(&bad).is_err());
    }
    #[test]
    fn per_vehicle_masks_follow_models_and_liveries_without_mutating_inputs() {
        let field = super::super::Field {
            models: vec![
                super::super::CarModel::new("lac").unwrap(),
                super::super::CarModel::new("bmw").unwrap(),
            ],
            car_model: vec![0, 1, 0],
        };
        let mut red = source();
        red[136..138].copy_from_slice(&0xf800u16.to_le_bytes());
        let mut medium = source();
        medium[136..138].copy_from_slice(&0x03e0u16.to_le_bytes());
        let mut car = super::super::Car {
            parts: vec![],
            textures: BTreeMap::from([("car0/lac_main_m1.tga".to_owned(), source())]),
            bindings: BTreeMap::new(),
            decal_materials: Default::default(),
            object_glass_materials: Default::default(),
            object_two_sided_materials: Default::default(),
            center: Vec3::ZERO,
            radius: 1.,
            track: true,
            liveries: vec![
                BTreeMap::from([("car1/bmw_main_m1.tga".to_owned(), red)]),
                BTreeMap::from([("car0/lac_main_m1.tga".to_owned(), medium)]),
            ],
            low_lods: Vec::new(),
            ornament_lods: Default::default(),
            reflections: Default::default(),
        };
        let original = car.textures.clone();
        let banks = car.liveries.clone();
        let prepared = prepare(&car, &field).unwrap();
        assert_eq!(prepared.0.len(), 3);
        assert_eq!(prepared.0[&0][128], 255);
        assert_eq!(prepared.0[&1][128], 0);
        assert_eq!(prepared.0[&2][128], 125);
        assert_eq!(car.textures, original);
        assert_eq!(car.liveries, banks);
        car.liveries[1].clear();
        let broken = car.liveries.clone();
        assert!(prepare(&car, &field).is_err());
        assert_eq!(car.textures, original);
        assert_eq!(car.liveries, broken);
        assert!(!super::super::graphics::Settings::default().source_body_occlusion);
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_paint_mask_keeps_every_source_green_mip() {
        use grid_formats::{pssg, texture};
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let bytes =
            std::fs::read(root.join("cars/lac/livery_00/textures_high/lac_tex_high_00.pssg"))
                .unwrap();
        let tree = pssg::read_tree(&bytes).unwrap();
        let textures = texture::read_named(&tree, &[b"lac_main_m1.tga"]).unwrap();
        let (dds, levels) = textures[0].validated_dds();
        assert_eq!(levels, 11);
        let green =
            super::super::texture_upload::decode_levels(&dds[128..], 1024, 1024, levels, true)
                .unwrap();
        let output = derive(&dds).unwrap();
        assert_eq!(output.len(), 128 + green.len());
        for (source, out) in green
            .as_chunks::<4>()
            .0
            .iter()
            .zip(output[128..].as_chunks::<4>().0)
        {
            assert_eq!(*out, [source[1], source[1], source[1], 255]);
        }
    }
}
