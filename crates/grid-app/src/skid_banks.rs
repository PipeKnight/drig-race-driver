//! Shared per-model skid profile/bank preparation; no model/type whitelist.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        render_resource::{
            TextureDataOrder, TextureDimension, TextureViewDescriptor, TextureViewDimension,
        },
        renderer::RenderDevice,
    },
};
use grid_tools::car::Car as CarModel;
use std::{collections::BTreeMap, path::Path};
#[derive(Clone)]
pub(crate) struct Bank {
    pub label: String,
    pub profile: grid_formats::skidmarks::Profile,
    pub diffuse: Vec<u8>,
    normal: Option<Vec<u8>>,
}
pub(crate) struct Prepared {
    pub banks: Vec<Bank>,
    pub slots: Vec<u32>,
}
fn source_label(db: &grid_formats::database::Database, model: &CarModel) -> Result<String, String> {
    let vehicle = db.table("vehicle").ok_or("vehicle table absent")?;
    let types = db
        .table("vehicle_skid_type")
        .ok_or("vehicle_skid_type table absent")?;
    let mut found = None;
    for row in vehicle
        .rows()
        .filter(|r| r.text("file_string") == Some(model.stem()))
    {
        let id = row
            .int("vehicle_skid_type_id")
            .ok_or("vehicle skid FK absent")?;
        let mut matches = types.rows().filter(|r| r.int("id") == Some(id));
        let ty = matches
            .next()
            .and_then(|r| r.text("type"))
            .ok_or("skid type row/name absent")?;
        if matches.next().is_some() {
            return Err("ambiguous skid type id".into());
        }
        if ty.is_empty()
            || ty.len() > 32
            || !ty
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("unsafe skid source label".into());
        }
        if found.as_deref().is_some_and(|v| v != ty) {
            return Err("vehicle rows disagree on skid source type".into());
        }
        found = Some(ty.to_owned());
    }
    found.ok_or_else(|| format!("no database vehicle row for {}", model.stem()))
}
pub(crate) fn prepare(
    root: &Path,
    models: &[CarModel],
    slots: &[usize],
) -> Result<Prepared, Box<dyn std::error::Error>> {
    if models.is_empty() || slots.is_empty() {
        return Err("actual skid model/slot field empty".into());
    }
    let db = grid_tools::read_database(root)?;
    let mut banks = Vec::new();
    let mut ids = BTreeMap::new();
    let mut model_banks = Vec::new();
    for model in models {
        let label = source_label(&db, model)?;
        let bank = if let Some(&id) = ids.get(&label) {
            id
        } else {
            let profile = grid_formats::skidmarks::read(&std::fs::read_to_string(
                root.join(format!("skidmarks/{label}_tm.xml")),
            )?)?;
            let relative = format!("skidmarks/{label}_tm.pssg");
            let package = |ids: &[&str]| crate::ground_effect_assets::package(root, &relative, ids);
            let mut textures = if normal_atlas_requested() {
                package(&["diffuse.tga", "normal.tga"])
                    .or_else(|_| package(&["diffuse.tga"]))?
            } else {
                package(&["diffuse.tga"])?
            };
            // DXT5's alpha is a normal component. Do not admit RGB-only atlases.
            let normal = textures.remove("normal.tga").filter(|dds| dds.get(84..88) == Some(b"DXT5"));
            let diffuse = textures
                .remove("diffuse.tga")
                .ok_or("actual skid diffuse absent")?;
            let id = u32::try_from(banks.len())?;
            ids.insert(label.clone(), id);
            banks.push(Bank {
                label,
                profile,
                diffuse,
                normal,
            });
            id
        };
        model_banks.push(bank);
    }
    let slots = slots
        .iter()
        .map(|&model| {
            model_banks
                .get(model)
                .copied()
                .ok_or("skid slot model outside actual Field")
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Prepared { banks, slots })
}
/// Called in Startup AFTER texture_upload::configure. Every layer goes through
/// the SAME existing portable upload path before compatibility/concatenation.
pub(crate) fn upload(prepared: &Prepared, device: &RenderDevice) -> Result<Image, String> {
    let count = u32::try_from(prepared.banks.len()).map_err(|_| "skid bank count overflow")?;
    let limits = device.limits();
    if count == 0
        || count > limits.max_texture_array_layers
        || limits.max_vertex_attributes < 9
        || limits.max_inter_stage_shader_variables < 9
        || limits.max_vertex_buffer_array_stride < 60
    {
        return Err("actual device lacks skid array/flat-bank interface limits".into());
    }
    let images = prepared
        .banks
        .iter()
        .map(|bank| {
            crate::ground_effect_assets::image(&bank.diffuse, true).map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let image = join_images(images, limits.max_texture_array_layers)?;
    if image.width() > limits.max_texture_dimension_2d
        || image.height() > limits.max_texture_dimension_2d
    {
        return Err("actual skid image exceeds device D2 dimensions".into());
    }
    if image
        .texture_descriptor
        .format
        .sample_type(None, Some(device.features()))
        != Some(bevy::render::render_resource::TextureSampleType::Float { filterable: true })
    {
        return Err("actual skid array format is not filterable float".into());
    }
    Ok(image)
}
/// Keep one policy for bank preparation, upload and binding layout.
pub(crate) fn supports_normal_layout(device: &RenderDevice) -> bool {
    use bevy::render::render_resource::{AsBindGroup, BindingType, ShaderStages};
    if !normal_atlas_requested() {
        return false;
    }
    let limits = device.limits();
    let base_textures: u32 = StandardMaterial::bind_group_layout_entries(device, true)
        .iter()
        .filter(|e| e.visibility.contains(ShaderStages::FRAGMENT)
            && matches!(e.ty, BindingType::Texture { .. }))
        .map(|e| e.count.map_or(1, std::num::NonZeroU32::get))
        .sum();
    // Bevy 0.19's debug-only MESH_PIPELINE_VIEW_LAYOUT_SAFE_MAX_TEXTURES
    // describes the reserved view textures on downlevel adapters.
    const BEVY_VIEW_TEXTURE_BUDGET: u32 = 10;
    limits.max_vertex_attributes >= 11
        && limits.max_inter_stage_shader_variables >= 11
        && limits.max_vertex_buffer_array_stride >= 84
        && limits.max_bindings_per_bind_group >= 103
        && limits.max_sampled_textures_per_shader_stage >= base_textures + BEVY_VIEW_TEXTURE_BUDGET + 2
}

fn normal_atlas_requested() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("GRID_SKID_NORMAL_ATLAS").as_deref() != Ok("off"))
}

/// Optional paired linear banks. The diffuse slot index is also the normal slot index.
pub(crate) fn upload_normals(
    prepared: &Prepared, device: &RenderDevice, diffuse: &Image,
) -> Result<Option<Image>, String> {
    if !supports_normal_layout(device) {
        return Ok(None);
    }
    let mut images = Vec::with_capacity(prepared.banks.len());
    for bank in &prepared.banks {
        let Some(dds) = &bank.normal else {
            return Err(format!("source skid normal atlas absent or has no DXT5 alpha: {}", bank.label));
        };
        let image = crate::texture_upload::image_linear(
            dds, diffuse.sampler.clone(), RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        ).map_err(|e| e.to_string())?;
        images.push(image);
    }
    let image = join_images(images, device.limits().max_texture_array_layers)?;
    if image.texture_descriptor.size != diffuse.texture_descriptor.size
        || image.texture_descriptor.mip_level_count != diffuse.texture_descriptor.mip_level_count
        || image.texture_descriptor.format.is_srgb()
        || image.texture_descriptor.format.sample_type(None, Some(device.features()))
            != Some(bevy::render::render_resource::TextureSampleType::Float { filterable: true })
    {
        return Err("source skid diffuse/linear normal bank layout disagrees".into());
    }
    Ok(Some(image))
}

fn join_images(mut images: Vec<Image>, max_layers: u32) -> Result<Image, String> {
    let count = u32::try_from(images.len()).map_err(|_| "skid image count overflow")?;
    if count == 0 || count > max_layers {
        return Err("skid array exceeds admitted layer count".into());
    }
    let mut first = images.remove(0);
    let shape = first.texture_descriptor.size;
    let format = first.texture_descriptor.format;
    let mips = first.texture_descriptor.mip_level_count;
    if first.texture_descriptor.dimension != TextureDimension::D2
        || shape.depth_or_array_layers != 1
    {
        return Err("skid source image requires a single D2 layer".into());
    }
    let mut bytes = first.data.take().ok_or("skid first image data absent")?;
    let layer_bytes = bytes.len();
    if layer_bytes == 0 || mips == 0 || mips > 32 {
        return Err("skid source image data/mip count invalid".into());
    }
    let (bw, bh) = format.block_dimensions();
    let block = format
        .block_copy_size(None)
        .ok_or("skid format has no colour block size")? as usize;
    let (mut width, mut height) = (shape.width, shape.height);
    let mut expected = 0usize;
    for _ in 0..mips {
        let level = (width.div_ceil(bw) as usize)
            .checked_mul(height.div_ceil(bh) as usize)
            .and_then(|n| n.checked_mul(block))
            .ok_or("skid image size overflow")?;
        expected = expected
            .checked_add(level)
            .ok_or("skid mip size overflow")?;
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    if layer_bytes != expected {
        return Err("skid prepared image byte layout disagrees with descriptor".into());
    }
    for mut image in images {
        if image.texture_descriptor.size != shape
            || image.texture_descriptor.dimension != TextureDimension::D2
            || image.texture_descriptor.format != format
            || image.texture_descriptor.mip_level_count != mips
        {
            return Err(
                "portable skid bank images have incompatible actual shape/format/mips".into(),
            );
        }
        let data = image.data.take().ok_or("skid bank data absent")?;
        if data.len() != layer_bytes {
            return Err("skid bank byte layout mismatch".into());
        }
        bytes.extend(data);
    }
    // A single D2 layer has the same contiguous mip bytes in both data
    // orders. DDS often arrives as LayerMajor and the portable KTX2 loader
    // reports MipMajor. Here all mips of each input precede the next layer.
    first.data = Some(bytes);
    first.data_order = TextureDataOrder::LayerMajor;
    first.texture_descriptor.size.depth_or_array_layers = count;
    first.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    Ok(first)
}
#[cfg(test)]
mod tests {
    use super::*;
    use grid_formats::database::{Database, Table, Value};
    fn db(rows: Vec<(&str, i32)>) -> Database {
        Database {
            tables: vec![
                Table {
                    name: "vehicle".into(),
                    columns: vec!["file_string".into(), "vehicle_skid_type_id".into()],
                    rows: rows
                        .into_iter()
                        .map(|(n, id)| vec![Value::Text(n.into()), Value::Int(id)])
                        .collect(),
                },
                Table {
                    name: "vehicle_skid_type".into(),
                    columns: vec!["id".into(), "type".into()],
                    rows: vec![
                        vec![Value::Int(41), Value::Text("custom_profile".into())],
                        vec![Value::Int(42), Value::Text("another_profile".into())],
                    ],
                },
            ],
        }
    }
    #[test]
    fn actual_database_row_api_resolves_fk_without_known_labels_and_refuses_conflicts() {
        let car = CarModel::new("aaa").unwrap();
        assert_eq!(
            source_label(&db(vec![("aaa", 41), ("aaa", 41)]), &car).unwrap(),
            "custom_profile"
        );
        assert!(source_label(&db(vec![("aaa", 41), ("aaa", 42)]), &car).is_err());
        assert!(source_label(&db(vec![("aaa", 999)]), &car).is_err());
        assert!(source_label(&db(vec![("bbb", 41)]), &car).is_err());
    }
    fn image(value: u8) -> Image {
        Image::new_fill(
            bevy::render::render_resource::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[value, 2, 3, 4],
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::default(),
        )
    }
    #[test]
    fn single_layer_mip_major_and_layer_major_become_layer_major_array() {
        let mut portable = image(1);
        let mut dds = image(2);
        portable.texture_descriptor.mip_level_count = 2;
        dds.texture_descriptor.mip_level_count = 2;
        portable.data_order = TextureDataOrder::MipMajor;
        dds.data_order = TextureDataOrder::LayerMajor;
        // For a 4x4 RGBA8 image, mip 0 contributes 64 bytes and mip 1 contributes 16.
        portable.data = Some([vec![11; 64], vec![12; 16]].concat());
        dds.data = Some([vec![21; 64], vec![22; 16]].concat());
        let joined = join_images(vec![portable, dds], 2).unwrap();
        assert_eq!(joined.data_order, TextureDataOrder::LayerMajor);
        assert_eq!(joined.texture_descriptor.mip_level_count, 2);
        assert_eq!(joined.texture_descriptor.size.depth_or_array_layers, 2);
        assert_eq!(
            joined.data.unwrap(),
            [vec![11; 64], vec![12; 16], vec![21; 64], vec![22; 16]].concat(),
        );
    }

    #[test]
    fn real_image_array_preserves_source_rgba_bytes_and_rejects_mismatch_or_limits() {
        let a = image(1);
        let b = image(9);
        let mut expected = a.data.clone().unwrap();
        expected.extend(b.data.clone().unwrap());
        let joined = join_images(vec![a, b], 2).unwrap();
        assert_eq!(joined.data.unwrap(), expected);
        assert_eq!(joined.texture_descriptor.size.depth_or_array_layers, 2);
        assert_eq!(
            joined.texture_view_descriptor.unwrap().dimension,
            Some(TextureViewDimension::D2Array)
        );
        assert!(join_images(vec![image(1), image(2)], 1).is_err());
        let mut b = image(2);
        b.texture_descriptor.format = bevy::render::render_resource::TextureFormat::Rgba8Unorm;
        assert!(join_images(vec![image(1), b], 2).is_err());
    }
}
