//! Optional authored source-alpha base dielectric mask, not original shader parity.
//! The coat stays independent; original GRID weights multiple lighting terms.
use bevy::{asset::RenderAssetUsages, image::ImageSampler, prelude::*};
use std::{collections::BTreeMap, error::Error};

#[derive(Default, Resource)]
pub struct Prepared {
    maps: super::body_occlusion::Prepared,
    white_control: bool,
}

fn encode(alpha: u8) -> u8 {
    // Bevy samples alpha then halves reflectance. With scalar1, sqrt(A)
    // produces sampled-point F0=.04*A, retaining the existing white F0=.04.
    ((alpha as f32 / 255.).sqrt() * 255.).round() as u8
}

fn derive(dds: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    super::body_occlusion::derive_with(dds, |p| [255, 255, 255, encode(p[3])])
}

pub fn prepare(
    car: &super::Car,
    field: &super::Field,
    white_control: bool,
) -> Result<Prepared, Box<dyn Error>> {
    fn white(dds: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        super::body_occlusion::derive_with(dds, |_| [255; 4])
    }
    Ok(Prepared {
        maps: super::body_occlusion::prepare_with(
            car,
            field,
            if white_control { white } else { derive },
        )?,
        white_control,
    })
}

pub fn upload(
    prepared: &mut Prepared,
    images: &mut Assets<Image>,
    sampler: ImageSampler,
) -> BTreeMap<usize, Handle<Image>> {
    prepared.maps.drain().into_iter().map(|(car, dds)| {
        let image = super::texture_upload::image_linear(&dds, sampler.clone(), RenderAssetUsages::default())
            .expect("validated paint specular DDS through shared upload");
        println!("body_specular car={car} white_control={} channel=alpha encoding=sqrt srgb=false format={:?} mips={} reflectance_scalar=1 white_f0=0.04 scope=authored-base-only coat=unchanged source_shader_parity=false", prepared.white_control, image.texture_descriptor.format, image.texture_descriptor.mip_level_count);
        (car, images.add(image))
    }).collect()
}

pub fn apply(material: &mut StandardMaterial, image: Option<&Handle<Image>>) {
    if let Some(image) = image {
        material.specular_texture = Some(image.clone());
        material.reflectance = 1.;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_formats::texture::{Format, Texture};

    fn source(alphas: [u8; 3]) -> Vec<u8> {
        let mut blocks = Vec::new();
        for alpha in alphas {
            let mut block = [0u8; 16];
            block[0] = alpha;
            block[1] = alpha;
            block[8..10].copy_from_slice(&0x07e0u16.to_le_bytes());
            blocks.extend_from_slice(&block);
        }
        Texture {
            id: b"specular_test",
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
    fn compensated_transfer_preserves_white_and_bounded_pointwise_f0() {
        assert_eq!(encode(0), 0);
        assert_eq!(encode(255), 255);
        let mut previous = 0;
        for alpha in 0..=255 {
            let encoded = encode(alpha);
            assert!(encoded >= previous);
            previous = encoded;
            let f0 = 0.16 * (encoded as f32 / 255. * 0.5).powi(2);
            let target = 0.04 * alpha as f32 / 255.;
            assert!((f0 - target).abs() < 0.000157);
        }
        assert!(!super::super::graphics::Settings::default().source_body_specular);
    }

    #[test]
    fn selects_alpha_and_preserves_all_mips_without_touching_source() {
        let dds = source([255, 64, 0]);
        let before = dds.clone();
        let out = derive(&dds).unwrap();
        assert_eq!(dds, before);
        assert_eq!(u32::from_le_bytes(out[28..32].try_into().unwrap()), 3);
        assert_eq!(out.len(), 128 + 84);
        assert_eq!(&out[128..132], &[255, 255, 255, 255]);
        assert_eq!(&out[192..196], &[255, 255, 255, 128]);
        assert_eq!(&out[208..212], &[255, 255, 255, 0]);
        assert!(derive(&dds[..dds.len() - 1]).is_err());
    }

    #[test]
    fn per_vehicle_masks_follow_own_livery_and_fail_before_publishing() {
        let field = super::super::Field {
            models: vec![
                super::super::CarModel::new("lac").unwrap(),
                super::super::CarModel::new("bmw").unwrap(),
            ],
            car_model: vec![0, 1, 0],
        };
        let mut car = super::super::Car {
            parts: vec![],
            textures: BTreeMap::from([("car0/lac_main_m1.tga".into(), source([255; 3]))]),
            bindings: BTreeMap::new(),
            decal_materials: Default::default(),
            object_glass_materials: Default::default(),
            center: Vec3::ZERO,
            radius: 1.,
            track: true,
            liveries: vec![
                BTreeMap::from([("car1/bmw_main_m1.tga".into(), source([0; 3]))]),
                BTreeMap::from([("car0/lac_main_m1.tga".into(), source([64; 3]))]),
            ],
        };
        let original = car.textures.clone();
        let banks = car.liveries.clone();
        let mut prepared = prepare(&car, &field, false).unwrap();
        let derived = prepared.maps.drain();
        assert_eq!(derived[&0][131], 255);
        assert_eq!(derived[&1][131], 0);
        assert_eq!(derived[&2][131], 128);
        assert!(prepared.maps.drain().is_empty());
        let mut white = prepare(&car, &field, true).unwrap();
        assert!(white.white_control);
        for dds in white.maps.drain().values() {
            assert!(dds[128..].iter().all(|&b| b == 255));
        }
        assert_eq!(car.textures, original);
        assert_eq!(car.liveries, banks);
        car.liveries[1].clear();
        let broken = car.liveries.clone();
        assert!(prepare(&car, &field, false).is_err());
        assert_eq!(car.textures, original);
        assert_eq!(car.liveries, broken);
        let mut material = StandardMaterial::default();
        let scalar = material.reflectance;
        apply(&mut material, None);
        assert_eq!(material.reflectance, scalar);
        assert!(material.specular_texture.is_none());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_alpha_mips_match_the_authored_conversion() {
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
        let decoded =
            super::super::texture_upload::decode_levels(&dds[128..], 1024, 1024, levels, true)
                .unwrap();
        let out = derive(&dds).unwrap();
        assert_eq!(out.len(), 128 + decoded.len());
        for (src, dst) in decoded
            .as_chunks::<4>()
            .0
            .iter()
            .zip(out[128..].as_chunks::<4>().0)
        {
            assert_eq!(*dst, [255, 255, 255, encode(src[3])]);
        }
    }
}
