//! Presentation of breakable route props (`grid_tools::dynamic_props`,
//! DYNAMIC_PROPS_RESEARCH.md). Each breakable entity's intact renderable is
//! drawn as its own part and hidden once the entity breaks; the damage
//! renderable's pieces (draws under each piece body's scene anchor) start
//! hidden and then follow their physical bodies.
use crate::{Car, Driving, Part};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use grid_formats::{mesh, pssg, scene};
use grid_tools::{dynamic_props::Catalog, route::Route};
use std::{collections::BTreeMap, path::Path};

const INTACT: &str = "dynamic/intact/";
/// An entity placement and its breakable index, if breakable.
pub(crate) type Placement = ([f32; 16], Option<usize>);
type Untagged = (Added<Name>, Without<IntactProp>, Without<PropPiece>);
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
    // One index of the object library for every piece (F445).
    let renderables = scene::Renderables::new(objects, sources);
    for (b, breakable) in catalog.breakables.iter().enumerate() {
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
            for draw in draws {
                let source = &sources[draw.source];
                let material = format!("objects/{}", String::from_utf8(draw.material.to_vec())?);
                if !car.bindings.contains_key(&material) {
                    return Err(format!("unresolved piece material {material}").into());
                }
                let matrix =
                    Mat4::from_cols_array(&scene::compose(&draw.transform, &breakable.placement));
                car.parts.push(Part {
                    name: format!("{PIECE}{b}/{s}/{}", String::from_utf8_lossy(draw.id)),
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
        "dynamic_props breakables={} drawn_pieces={drawn} pieces_without_draws={missing} kept_static={:?}",
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
    mut logged: Local<usize>,
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
}
