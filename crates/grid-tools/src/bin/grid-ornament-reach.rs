//! Private diagnostic: for every placed ornament prototype, how many placements
//! protrude in front of the boundary sheet (car side) and by how much, using the
//! prototype's mesh-space AABB corners. Identifies visible barriers without collision.
use grid_formats::{mesh, ornaments, pssg, scene};
use std::{collections::BTreeMap, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: grid-ornament-reach ROOT")?;
    let root = Path::new(&root);
    let bytes = std::fs::read(root.join("tracks/europe/donington/objects.pssg"))?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let draws = scene::read_object_draws(&tree, &sources)?;
    let mut aabb: BTreeMap<Vec<u8>, ([f32; 3], [f32; 3])> = BTreeMap::new();
    for d in &draws {
        let Some(p) = d.prototype else { continue };
        let s = &sources[d.source];
        let e = aabb
            .entry(p.to_vec())
            .or_insert(([f32::MAX; 3], [f32::MIN; 3]));
        for &i in &d.indices {
            let w = scene::transform_point(&d.transform, s.positions[i as usize]);
            for ((low, high), x) in e.0.iter_mut().zip(e.1.iter_mut()).zip(w) {
                *low = low.min(x);
                *high = high.max(x);
            }
        }
    }
    let boundary = grid_tools::cqtc_geometry::Geometry::read(&std::fs::read(
        root.join("tracks/europe/donington/route_0/boundarylines.cqtc"),
    )?)?;
    let planes: Vec<([f32; 3], [f32; 3], [f32; 2])> = boundary
        .triangles
        .iter()
        .filter_map(|t| {
            let t = t.map(|i| boundary.positions[i as usize]);
            let u = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
            let v = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            (l > 1e-6).then(|| {
                (
                    t[0],
                    n.map(|c| c / l),
                    [
                        (t[0][0] + t[1][0] + t[2][0]) / 3.,
                        (t[0][2] + t[1][2] + t[2][2]) / 3.,
                    ],
                )
            })
        })
        .collect();
    let placement_bytes =
        std::fs::read(root.join("tracks/europe/donington/route_0/ornaments.bin"))?;
    let placements = ornaments::read(&placement_bytes)?;
    println!("prototype\tplacements\tsize\tprotruding\tmax_protrusion\tnear_sheet");
    for p in &placements.prototypes {
        let key = format!("{} Root", String::from_utf8_lossy(p.name)).into_bytes();
        let Some((lo, hi)) = aabb.get(&key) else {
            continue;
        };
        let (mut protruding, mut near, mut max_front) = (0, 0, f32::MIN);
        for m in &p.transforms {
            let corners: Vec<[f32; 3]> = (0..8)
                .map(|i| {
                    let local =
                        std::array::from_fn(|j| if i >> j & 1 == 0 { lo[j] } else { hi[j] });
                    scene::transform_point(m, local)
                })
                .collect();
            let c = [m[12], m[14]];
            let Some((o, n, _)) = planes.iter().min_by(|a, b| {
                let d = |q: &[f32; 2]| (q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2);
                d(&a.2).total_cmp(&d(&b.2))
            }) else {
                continue;
            };
            let offsets: Vec<f32> = corners
                .iter()
                .map(|w| (w[0] - o[0]) * n[0] + (w[1] - o[1]) * n[1] + (w[2] - o[2]) * n[2])
                .collect();
            let front = offsets.iter().copied().fold(f32::MIN, f32::max);
            let back = offsets.iter().copied().fold(f32::MAX, f32::min);
            if back > -1.5 && front < 6. {
                near += 1;
                if front > 0.05 {
                    protruding += 1;
                    max_front = max_front.max(front);
                }
            }
        }
        if near > 0 {
            println!(
                "{}\t{}\t{:.2}x{:.2}x{:.2}\t{protruding}\t{:.2}\t{near}",
                String::from_utf8_lossy(p.name),
                p.transforms.len(),
                hi[0] - lo[0],
                hi[1] - lo[1],
                hi[2] - lo[2],
                if protruding > 0 { max_front } else { 0. }
            );
        }
    }
    Ok(())
}
