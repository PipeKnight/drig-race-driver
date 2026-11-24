//! Authored vehicle AO volumes sampled by the source road material.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        render_resource::{
            Extent3d, ShaderType, TextureDimension, TextureFormat, TextureViewDescriptor,
            TextureViewDimension,
        },
        storage::ShaderBuffer,
    },
};
use grid_formats::{pssg, scene, texture};
use grid_tools::{environment::Environment, route::Route};
use std::{error::Error, fs, path::Path};

const ATLAS_SIZE: u32 = 128;
const MAX_CARS: usize = 20;
pub(crate) const BUFFER_BYTES: usize = MAX_CARS * 64;
const HALF_HEIGHT: f32 = 0.7;

pub(crate) fn enabled() -> bool {
    crate::dev::var("GRID_SOURCE_ROAD_DYNAMIC_AO").as_deref() != Ok("off")
}

#[derive(Clone, Copy)]
struct Shape {
    placement: Mat4,
    min: Vec3,
    max: Vec3,
    layer: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
struct Volume {
    u: Vec4,
    v: Vec4,
    height: Vec4,
    // Texture layer and strength. A zero strength skips absent volumes.
    controls: Vec4,
}

#[derive(Clone, Debug, Default, PartialEq, ShaderType)]
struct Volumes {
    entries: [Volume; MAX_CARS],
}

impl Shape {
    fn for_pose(self, pose: Mat4, strength: f32) -> Volume {
        let inverse = (pose * self.placement).inverse();
        Volume {
            u: (inverse.row(0) - Vec4::W * self.min.x) / (self.max.x - self.min.x),
            v: (Vec4::W * self.max.z - inverse.row(2)) / (self.max.z - self.min.z),
            height: Vec4::W * 0.5 - inverse.row(1) / (2.0 * HALF_HEIGHT),
            controls: Vec4::new(self.layer as f32, strength, 0.0, 0.0),
        }
    }
}

#[derive(Resource)]
pub(crate) struct GroundAo {
    pub(crate) atlas: Handle<Image>,
    pub(crate) buffer: Handle<ShaderBuffer>,
    shapes: Vec<Option<Shape>>,
    strength: f32,
    last: Volumes,
}

fn read_shape(path: &Path, layer: u32) -> Result<(Shape, Vec<u8>), Box<dyn Error>> {
    let data = fs::read(path)?;
    let tree = pssg::read_tree(&data)?;
    let root = (0..tree.nodes.len())
        .find(|&node| {
            tree.name(node) == b"ROOTNODE" && tree.string(node, b"id").ok() == Some(b"box")
        })
        .ok_or("vehicle AO box is missing")?;
    let bound = tree.nodes[root]
        .children
        .iter()
        .copied()
        .find(|&node| tree.name(node) == b"BOUNDINGBOX")
        .and_then(|node| tree.nodes[node].data)
        .filter(|data| data.len() == 24)
        .ok_or("vehicle AO box bounds are invalid")?;
    let mut values = bound
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(f32::from_be_bytes);
    let min = Vec3::from_array(std::array::from_fn(|_| values.next().unwrap()));
    let max = Vec3::from_array(std::array::from_fn(|_| values.next().unwrap()));
    if !min.is_finite() || !max.is_finite() || max.x <= min.x || max.z <= min.z {
        return Err("vehicle AO box has no horizontal extent".into());
    }
    let placement = Mat4::from_cols_array(&scene::matrix(&tree, root)?);
    if !placement.inverse().is_finite() {
        return Err("vehicle AO box transform is not invertible".into());
    }
    let textures = texture::read_textures(&tree)?;
    let [atlas] = textures.as_slice() else {
        return Err("vehicle AO package requires one texture".into());
    };
    if atlas.width != ATLAS_SIZE || atlas.height != ATLAS_SIZE {
        return Err("vehicle AO atlas has an unsupported size".into());
    }
    let rgba = super::texture_upload::decode_format_levels(
        atlas.base_blocks,
        atlas.width,
        atlas.height,
        1,
        atlas.format,
    )
    .ok_or("vehicle AO atlas pixels are invalid")?;
    let red = rgba.as_chunks::<4>().0.iter().map(|rgba| rgba[0]).collect();
    Ok((
        Shape {
            placement,
            min,
            max,
            layer,
        },
        red,
    ))
}

pub(crate) fn install(
    app: &mut App,
    game_root: &Path,
    route: &Route,
    field: &super::Field,
    player: Mat4,
) -> Result<(), Box<dyn Error>> {
    let environment = Environment::for_route(game_root, route)?;
    let strength = environment.lighting.scalar("dao_scale").unwrap_or(0.7);
    let mut atlas = Vec::new();
    let mut layers = 0;
    let models: Vec<_> = field
        .models
        .iter()
        .map(|model| {
            let path = game_root.join(model.file("_AO.pssg"));
            match read_shape(&path, layers) {
                Ok((shape, pixels)) => {
                    atlas.extend(pixels);
                    layers += 1;
                    Some(shape)
                }
                Err(error) => {
                    warn!("Vehicle AO unavailable for {}: {error}", model.stem());
                    None
                }
            }
        })
        .collect();
    if layers == 0 {
        return Ok(());
    }
    let shapes: Vec<_> = field
        .car_model
        .iter()
        .take(MAX_CARS)
        .map(|&index| models[index])
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: layers,
        },
        TextureDimension::D2,
        atlas,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    let atlas = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let player = app
        .world()
        .get_resource::<super::Driving>()
        .map_or(player, |driving| {
            player * super::model_offset(&driving.runtime)
        });
    let mut last = Volumes::default();
    if let Some(Some(shape)) = shapes.first() {
        last.entries[0] = shape.for_pose(player, strength);
    }
    let buffer = app
        .world_mut()
        .resource_mut::<Assets<ShaderBuffer>>()
        .add(ShaderBuffer::from(last.clone()));
    println!(
        "source_vehicle_ao models={layers} cars={} strength={strength}",
        shapes.len()
    );
    app.insert_resource(GroundAo {
        atlas,
        buffer,
        shapes,
        strength,
        last,
    })
    .add_systems(PostUpdate, update);
    Ok(())
}

fn update(
    mut ao: ResMut<GroundAo>,
    driving: Option<Res<super::Driving>>,
    replay: Option<Res<super::replay::Replay>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let Some(driving) = driving else {
        return;
    };
    let replay_frame = replay
        .as_ref()
        .and_then(|replay| replay.session.as_ref())
        .and_then(|session| session.shown.as_ref());
    let mut current = Volumes::default();
    for (slot, &shape) in ao.shapes.iter().enumerate() {
        let Some(shape) = shape else {
            continue;
        };
        let runtime = if slot == 0 {
            Some(&driving.runtime)
        } else {
            driving
                .opponents
                .as_ref()
                .and_then(|opponents| opponents.multi.cars.get(slot))
        };
        let Some(runtime) = runtime else {
            continue;
        };
        let pose = replay_frame
            .and_then(|frame| frame.cars.get(slot))
            .map(|car| car.pose.to_mat4())
            .or_else(|| driving.shown_pose(slot))
            .unwrap_or_else(|| Mat4::from_cols_array(&runtime.pose()));
        // The atlas is authored in the exterior model's frame. Runtime poses
        // are in the physical frame, whose origin can lie above the AO box.
        let model_pose = pose * super::model_offset(runtime);
        if model_pose.is_finite() {
            current.entries[slot] = shape.for_pose(model_pose, ao.strength);
        }
    }
    if current != ao.last
        && let Some(mut buffer) = buffers.get_mut(&ao.buffer)
    {
        buffer.set_data(current.clone());
        ao.last = current;
    }
}
