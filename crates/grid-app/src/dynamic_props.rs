//! Presentation of breakable route props (`grid_tools::dynamic_props`,
//! DYNAMIC_PROPS_RESEARCH.md). Each breakable entity's intact renderable is
//! drawn as its own part and hidden once the entity breaks; the damage
//! renderable's pieces (draws under each piece body's scene anchor) start
//! hidden and then follow their physical bodies.
use crate::{Car, Driving, Part};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::{
        Indices, PrimitiveTopology, VertexAttributeValues,
        skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    },
    prelude::*,
};
use grid_formats::{mesh, pssg, scene};
use grid_tools::{dynamic_props::Catalog, route::Route};
use std::{collections::BTreeMap, path::Path};

const INTACT: &str = "dynamic/intact/";
/// An entity placement and its breakable index, if breakable.
pub(crate) type Placement = ([f32; 16], Option<usize>);
type Untagged = (Added<Name>, Without<IntactProp>, Without<PropPiece>);
type JointFilter = (Without<PropPiece>, Without<IntactProp>);
const PIECE: &str = "dynamic/piece/";

/// Breakable catalog of the route, or none when the route has no route props
/// or physics properties this port can read.
pub(crate) fn catalog(root: &Path, route: &Route) -> Option<Catalog> {
    let props = grid_tools::entity_geometry::Props::read_route(root, route).ok()?;
    match Catalog::read(root, route, &props) {
        Ok(catalog) => Some(catalog),
        Err(error) => {
            println!("dynamic_props presentation unavailable: {error}");
            None
        }
    }
}

/// Breakable index by entity placement index.
pub(crate) fn by_entity(catalog: &Catalog) -> BTreeMap<usize, usize> {
    catalog
        .breakables
        .iter()
        .enumerate()
        .map(|(i, b)| (b.entity, i))
        .collect()
}

/// Name of an intact breakable's part.
pub(crate) fn intact_name(breakable: usize, node: &[u8], draw: &[u8]) -> String {
    format!(
        "{INTACT}{breakable}/{}/{}",
        String::from_utf8_lossy(node),
        String::from_utf8_lossy(draw)
    )
}

pub(crate) fn draw_mesh(source: &mesh::Mesh<'_>, indices: Vec<u16>) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, source.positions.clone());
    if !source.texcoords.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, source.texcoords.clone());
    }
    mesh.insert_indices(Indices::U16(indices));
    if source.normals.is_empty() || source.nonfinite_normals != 0 {
        mesh.compute_smooth_normals();
    } else {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals.clone());
    }
    mesh
}

/// The source mesh and draw of a piece part's name (`PIECE` b/s/source/draw
/// id). Pieces with the same source and indices have the same mesh
/// ([`draw_mesh`]).
pub(crate) fn piece_draw(name: &str) -> Option<&str> {
    let mut fields = name.strip_prefix(PIECE)?.splitn(3, '/');
    fields.next()?;
    fields.next()?;
    fields.next()
}

/// Piece parts: every piece's draws under its body anchor in the damage
/// renderable, placed with the entity transform. Returns the piece count drawn.
pub(crate) fn append_pieces(
    car: &mut Car,
    catalog: &Catalog,
    objects: &pssg::Tree<'_>,
    sources: &[mesh::Mesh<'_>],
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut drawn = 0;
    let mut missing = 0;
    let mut skinned = 0;
    // One index of the object library for every piece (F445).
    let renderables = scene::Renderables::new(objects, sources);
    for (b, breakable) in catalog.breakables.iter().enumerate() {
        // The damage renderable's skinned meshes, drawn whole and blended
        // over the pieces' joints; their per-joint draws are skipped below.
        let mut skin_ids = Vec::new();
        if let Ok(r) = &renderables {
            for renderable in breakable.pieces.iter().map(|p| p.renderable.as_str()).collect::<std::collections::BTreeSet<_>>() {
                let anchors: Vec<&[u8]> = (breakable.pieces.iter())
                    .filter(|p| p.renderable == renderable)
                    .map(|p| p.anchor.as_bytes())
                    .collect();
                let specs: Vec<usize> = (0..breakable.pieces.len())
                    .filter(|&s| breakable.pieces[s].renderable == renderable)
                    .collect();
                let Ok(skins) = r.skins_in(renderable.as_bytes(), &anchors) else {
                    continue;
                };
                for skin in skins {
                    skin_ids.push(skin.id);
                    let joints: Vec<(Option<usize>, Mat4)> = (skin.joints.iter())
                        .map(|j| {
                            let rest = scene::compose(&j.to_root, &breakable.placement);
                            (j.anchor.map(|a| specs[a]), Mat4::from_cols_array(&rest))
                        })
                        .collect();
                    // The part follows the first piece with a joint (pieces
                    // go live together when their breakable breaks).
                    let Some(spec) = joints.iter().find_map(|j| j.0) else {
                        continue;
                    };
                    let material = format!("objects/{}", String::from_utf8(skin.material.to_vec())?);
                    if !car.bindings.contains_key(&material) {
                        return Err(format!("unresolved piece material {material}").into());
                    }
                    let source = &sources[skin.source];
                    let Some((bound, weights)) = &source.skin_weights else {
                        continue;
                    };
                    let mut mesh = draw_mesh(source, source.indices.clone());
                    mesh.insert_attribute(
                        Mesh::ATTRIBUTE_JOINT_INDEX,
                        VertexAttributeValues::Uint16x4(bound.iter().map(|j| j.map(u16::from)).collect()),
                    );
                    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights.clone());
                    let name = format!("{PIECE}{b}/{spec}/{}/{}", skin.source, String::from_utf8_lossy(skin.id));
                    car.piece_skins.insert(
                        name.clone(),
                        PieceSkin {
                            breakable: b,
                            inverse_bind: skin.joints.iter().map(|j| Mat4::from_cols_array(&j.inverse_bind)).collect(),
                            joints,
                        },
                    );
                    car.parts.push(Part {
                        name,
                        material,
                        mesh,
                        matrices: vec![Mat4::from_cols_array(&breakable.placement)],
                        wheel: None,
                        joint: String::new(),
                    });
                    skinned += 1;
                }
            }
        }
        for (s, piece) in breakable.pieces.iter().enumerate() {
            let Ok(draws) = renderables
                .as_ref()
                .map_err(|_| ())
                // The anchor's draws in the damage renderable's frame: a piece
                // drawn with only its anchor's own transform sat 0.5-0.6 m
                // off its body (F564), under the road or in the air.
                .and_then(|r| {
                    r.draws_in(piece.renderable.as_bytes(), piece.anchor.as_bytes())
                        .map_err(|_| ())
                })
            else {
                missing += 1;
                continue;
            };
            if draws.is_empty() {
                missing += 1;
                continue;
            }
            drawn += 1;
            for draw in draws.into_iter().filter(|d| !skin_ids.contains(&d.id)) {
                let source = &sources[draw.source];
                let material = format!("objects/{}", String::from_utf8(draw.material.to_vec())?);
                if !car.bindings.contains_key(&material) {
                    return Err(format!("unresolved piece material {material}").into());
                }
                let matrix =
                    Mat4::from_cols_array(&scene::compose(&draw.transform, &breakable.placement));
                car.parts.push(Part {
                    name: format!("{PIECE}{b}/{s}/{}/{}", draw.source, String::from_utf8_lossy(draw.id)),
                    material,
                    mesh: draw_mesh(source, draw.indices),
                    matrices: vec![matrix],
                    wheel: None,
                    joint: String::new(),
                });
            }
        }
    }
    println!(
        "dynamic_props breakables={} drawn_pieces={drawn} pieces_without_draws={missing} skinned_meshes={skinned} kept_static={:?}",
        catalog.breakables.len(),
        catalog.unported
    );
    Ok(drawn)
}

/// Scripted validation drive into a breakable prop (`GRID_PROP_SCRIPT`).
#[derive(Resource)]
pub(crate) struct PropScript(pub(crate) grid_tools::dynamic_props::PropPursuit);

#[derive(Component)]
pub(crate) struct IntactProp(pub(crate) usize);

/// A skinned damage mesh's joints (tyre walls, Armco sections, flags).
pub(crate) struct PieceSkin {
    breakable: usize,
    inverse_bind: Vec<Mat4>,
    /// Per skin joint: its piece, if under one, and its world matrix at the
    /// intact placement.
    joints: Vec<(Option<usize>, Mat4)>,
}

/// Gives the skinned piece part `entity` its joints and skin in the command
/// flush that spawns it. A mesh with joint attributes drawn without its skin
/// fails wgpu validation (the prepass pipeline's skinned layout against the
/// plain mesh bind group); F710 attached the skin a frame later in [`tag`],
/// which never runs in static scenes.
pub(crate) fn attach_skin(commands: &mut Commands, entity: Entity, skin: &PieceSkin) {
    let joints: Vec<Entity> = (skin.joints.iter())
        .map(|&(spec, rest)| {
            let joint = PieceJoint { breakable: skin.breakable, spec, rest };
            commands.spawn((joint, Transform::from_matrix(rest))).id()
        })
        .collect();
    let inverse_bind = skin.inverse_bind.clone();
    commands.queue(move |world: &mut World| {
        let inverse_bindposes = (world.resource_mut::<Assets<SkinnedMeshInverseBindposes>>())
            .add(SkinnedMeshInverseBindposes::from(inverse_bind));
        world.entity_mut(entity).insert((SkinnedMesh { inverse_bindposes, joints }, NoFrustumCulling));
    });
}

/// A joint of a skinned piece part; it follows its piece's body.
#[derive(Component)]
pub(crate) struct PieceJoint {
    breakable: usize,
    spec: Option<usize>,
    rest: Mat4,
}

#[derive(Component)]
pub(crate) struct PropPiece {
    pub(crate) breakable: usize,
    pub(crate) spec: usize,
    /// Draw world matrix at the intact placement.
    pub(crate) rest: Mat4,
}

/// Tag the spawned prop parts by name (pieces start hidden).
pub(crate) fn tag(mut commands: Commands, parts: Query<(Entity, &Name, &Transform), Untagged>) {
    for (entity, name, transform) in &parts {
        if let Some(rest) = name.as_str().strip_prefix(INTACT) {
            if let Some(b) = rest.split('/').next().and_then(|v| v.parse().ok()) {
                commands.entity(entity).insert(IntactProp(b));
            }
        } else if let Some(rest) = name.as_str().strip_prefix(PIECE) {
            let mut fields = rest.split('/');
            if let (Some(Ok(breakable)), Some(Ok(spec))) =
                (fields.next().map(str::parse), fields.next().map(str::parse))
            {
                commands.entity(entity).insert((
                    PropPiece {
                        breakable,
                        spec,
                        rest: transform.to_matrix(),
                    },
                    Visibility::Hidden,
                ));
            }
        }
    }
}

pub(crate) fn pose(m: &[[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array(&std::array::from_fn(|i| m[i / 4][i % 4]))
}

/// Hide broken intact props; place live pieces at their bodies. Each new
/// break is logged once (playtest evidence).
pub(crate) fn present(
    driving: Option<Res<Driving>>,
    mut intact: Query<(&IntactProp, &mut Visibility), Without<PropPiece>>,
    mut pieces: Query<(&PropPiece, &mut Transform, &mut Visibility), Without<IntactProp>>,
    mut joints: Query<(&PieceJoint, &mut Transform), JointFilter>,
    mut logged: Local<usize>,
    mut moved: Local<std::collections::BTreeSet<usize>>,
) {
    let _timer = grid_tools::perf::Timer::start(grid_tools::perf::Stage::PropPresent);
    let Some(driving) = driving else {
        return;
    };
    let (props, bodies) = match &driving.opponents {
        Some(o) => match o.multi.dynamic_props() {
            Some(p) => (p, o.multi.bodies()),
            None => return,
        },
        None => match driving.runtime.dynamic_props() {
            Some(p) => (p, &driving.runtime.state.world.groups.bodies[..]),
            None => return,
        },
    };
    // A restart clears the events.
    *logged = (*logged).min(props.events.len());
    for event in &props.events[*logged..] {
        log_line!(
            "dynamic_props event=break tick={} breakable={} template={} force={} pieces={} bodies={}",
            event.tick,
            event.breakable,
            event.template,
            event.force,
            props.pieces.len(),
            bodies.len()
        );
    }
    *logged = props.events.len();
    for (prop, mut visibility) in &mut intact {
        let broken = props.broken.get(prop.0).copied().unwrap_or(false);
        visibility.set_if_neq(if broken {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    let live: BTreeMap<(usize, usize), Mat4> = props
        .poses(bodies)
        .into_iter()
        .map(|(b, s, m)| ((b, s), pose(&m)))
        .collect();
    for (piece, mut transform, mut visibility) in &mut pieces {
        match live.get(&(piece.breakable, piece.spec)) {
            Some(now) => {
                let start =
                    pose(&props.catalog.breakables[piece.breakable].pieces[piece.spec].pose);
                *transform = Transform::from_matrix(*now * start.inverse() * piece.rest);
                visibility.set_if_neq(Visibility::Inherited);
            }
            None => {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
    // Joints of live breakables follow their pieces; those of a breakable
    // that stopped being live (a restart) go back to rest once.
    let moving: std::collections::BTreeSet<usize> = live.keys().map(|&(b, _)| b).collect();
    if moving.is_empty() && moved.is_empty() {
        return;
    }
    for (joint, mut transform) in &mut joints {
        if !moving.contains(&joint.breakable) && !moved.contains(&joint.breakable) {
            continue;
        }
        let at = match joint.spec.and_then(|s| live.get(&(joint.breakable, s)).map(|now| (s, now))) {
            Some((s, now)) => {
                let start = pose(&props.catalog.breakables[joint.breakable].pieces[s].pose);
                *now * start.inverse() * joint.rest
            }
            None => joint.rest,
        };
        transform.set_if_neq(Transform::from_matrix(at));
    }
    *moved = moving;
}

/// Diagnostic `GRID_FLOAT_CHECK=local/OUT.tsv` (backlog row 102): every 60
/// frames, each live piece's lowest drawn vertex and its collision hull's
/// lowest point against the road under them, rewritten in OUT. A piece whose
/// hull rests but whose mesh does not shows the drawn and simulated shapes
/// apart; one whose hull is above the road and that has no other piece
/// under it (`piece_gap`) floats in the simulation.
pub(crate) fn float_check(
    driving: Option<Res<Driving>>,
    pieces: Query<(&PropPiece, &Transform, &Visibility, &Mesh3d)>,
    meshes: Res<Assets<Mesh>>,
    mut frame: Local<u64>,
) {
    *frame += 1;
    let Some(out) = std::env::var_os("GRID_FLOAT_CHECK") else {
        return;
    };
    let Some(driving) = driving else {
        return;
    };
    if !frame.is_multiple_of(60) {
        return;
    }
    let (props, bodies, track) = match &driving.opponents {
        Some(o) => match o.multi.dynamic_props() {
            Some(p) => (p, o.multi.bodies(), o.multi.cars[0].track()),
            None => return,
        },
        None => match driving.runtime.dynamic_props() {
            Some(p) => (p, &driving.runtime.state.world.groups.bodies[..], driving.runtime.track()),
            None => return,
        },
    };
    let ground = |x: f32, y: f32, z: f32| {
        let start = [x, y + 1., z, 1.];
        let end = [x, y - 30., z, 1.];
        let mut cache = grid_world::rectangle_cull::Cache::default();
        let hit = track.query(start, end, &mut cache).ok()?.hit?;
        Some(start[1] + (end[1] - start[1]) * hit.contact.fraction)
    };
    let hulls: BTreeMap<usize, grid_tools::dynamic_props::RoadDepth> =
        props.road_depths(track, bodies).into_iter().map(|d| (d.body, d)).collect();
    let body_of: BTreeMap<(usize, usize), usize> =
        props.pieces.iter().map(|p| ((p.breakable, p.spec), p.body)).collect();
    // Each drawn piece's lowest vertex and world box.
    let mut drawn_pieces = Vec::new();
    for (piece, transform, visibility, mesh) in &pieces {
        if *visibility == Visibility::Hidden {
            continue;
        }
        let Some(mesh) = meshes.get(&mesh.0) else { continue };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        let used: Vec<usize> = match mesh.indices() {
            Some(i) => i.iter().collect(),
            None => (0..positions.len()).collect(),
        };
        let matrix = transform.to_matrix();
        let world: Vec<Vec3> = (used.iter().filter_map(|&i| positions.get(i)))
            .map(|&p| matrix.transform_point3(Vec3::from_array(p)))
            .collect();
        let Some(&low) = world.iter().min_by(|a, b| a.y.total_cmp(&b.y)) else {
            continue;
        };
        let min = world.iter().fold(Vec3::INFINITY, |m, &v| m.min(v));
        let max = world.iter().fold(Vec3::NEG_INFINITY, |m, &v| m.max(v));
        drawn_pieces.push((piece, low, min, max));
    }
    let mut text = String::from(
        "tick\tbreakable\tspec\ttemplate\tanchor\tdrawn_gap\thull_gap\tpiece_gap\tasleep\tx\ty\tz\n",
    );
    for (i, &(piece, low, ..)) in drawn_pieces.iter().enumerate() {
        let drawn = ground(low.x, low.y, low.z).map_or(f32::NAN, |g| low.y - g);
        // Approximate support by another piece: the top of a box under the
        // lowest vertex, at most 5 cm above it.
        let piece_gap = (drawn_pieces.iter().enumerate())
            .filter(|&(j, &(_, _, min, max))| {
                j != i
                    && (min.x - 0.05..=max.x + 0.05).contains(&low.x)
                    && (min.z - 0.05..=max.z + 0.05).contains(&low.z)
                    && max.y <= low.y + 0.05
            })
            .map(|(_, &(_, _, _, max))| low.y - max.y)
            .min_by(f32::total_cmp)
            .unwrap_or(f32::NAN);
        let hull = body_of.get(&(piece.breakable, piece.spec)).and_then(|b| hulls.get(b));
        let spec = &props.catalog.breakables[piece.breakable];
        text += &format!(
            "{}\t{}\t{}\t{}\t{}\t{drawn:.3}\t{:.3}\t{piece_gap:.3}\t{}\t{:.2}\t{:.2}\t{:.2}\n",
            *frame,
            piece.breakable,
            piece.spec,
            spec.template,
            spec.pieces[piece.spec].anchor,
            hull.map_or(f32::NAN, |h| -h.depth),
            hull.is_some_and(|h| h.asleep),
            low.x,
            low.y,
            low.z
        );
    }
    let _ = std::fs::write(out, text);
}
