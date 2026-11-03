//! The original's impostor-card shading on the forest's `_x2` cards
//! (`tree_lowlod.cg.vp`, `tree_lowlod_shadowed.cg.fp`,
//! docs/SOURCE_VEGETATION.md): two neighbouring atlas views blended by the
//! eye's angle (colour, alpha and normal), each texel's normal from the
//! card's baked normal atlas, bent towards the sun by `LightWrap`, and its
//! N.L lifted by `InvFoliageDensity`.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use std::collections::BTreeMap;

const SHADER: Handle<Shader> = uuid_handle!("6b1d2f0e-6a39-4c55-9f4e-1c8a2e7d5b31");
pub(crate) type CardMaterial = ExtendedMaterial<StandardMaterial, CardLight>;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct CardLight {
    #[texture(100)]
    #[sampler(101)]
    normals: Handle<Image>,
    // LightWrap, InvFoliageDensity, card frame sign, unused.
    #[uniform(102)]
    lighting: Vec4,
}
impl MaterialExtension for CardLight {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    // The depth and motion prepasses test the same blended alpha.
    fn prepass_fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// What the forest loader found for its cards.
#[derive(Resource, Default)]
pub struct Cards {
    /// Part name: (normal atlas key, LightWrap, InvFoliageDensity).
    pub parts: BTreeMap<String, (String, f32, f32)>,
    /// Normal atlases by key (DDS bytes, linear).
    pub normals: BTreeMap<String, Vec<u8>>,
}

/// The atlas's tangent-space X runs along `A x up`, with `A` level towards
/// the eye [I: with this sign Spa's side-lit conifer cards are bright on the
/// sun's side, as the full trees are; the other sign lights the far side].
const FRAME_SIGN: f32 = 1.;

pub fn install(app: &mut App, cards: Cards) {
    // GRID_CARD_LIGHTING=0 keeps the cards on the plain material (F718), for review.
    if cards.parts.is_empty() || std::env::var("GRID_CARD_LIGHTING").is_ok_and(|v| v == "0") {
        return;
    }
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(SHADER.id(), Shader::from_wgsl(include_str!("tree_cards.wgsl"), "grid_tree_cards.wgsl"))
        .expect("tree card shader");
    app.insert_resource(cards)
        .add_plugins(MaterialPlugin::<CardMaterial>::default())
        .add_systems(PostStartup, attach);
}

fn attach(
    mut commands: Commands,
    cards: Res<Cards>,
    settings: Res<super::graphics::Settings>,
    query: Query<(Entity, &Name, &MeshMaterial3d<StandardMaterial>)>,
    standard: Res<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CardMaterial>>,
) {
    let normals: BTreeMap<_, _> = cards
        .normals
        .iter()
        .map(|(key, dds)| {
            let image = super::texture_upload::image_linear(dds, settings.sampler(true), RenderAssetUsages::default())
                .expect("validated card normal atlas");
            (key.clone(), images.add(image))
        })
        .collect();
    let mut handles = BTreeMap::new();
    let mut attached = 0;
    for (entity, name, base) in &query {
        let Some((key, wrap, density)) = cards.parts.get(name.as_str()) else {
            continue;
        };
        let handle = handles
            .entry((base.0.id(), key.clone()))
            .or_insert_with(|| {
                materials.add(ExtendedMaterial {
                    base: standard.get(&base.0).expect("card base material ready").clone(),
                    extension: CardLight {
                        normals: normals[key].clone(),
                        lighting: Vec4::new(*wrap, *density, FRAME_SIGN, 0.),
                    },
                })
            })
            .clone();
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));
        attached += 1;
    }
    println!(
        "tree_cards attached={attached} parts={} materials={} normal_atlases={} lighting=source-lowlod",
        cards.parts.len(),
        handles.len(),
        normals.len()
    );
}
