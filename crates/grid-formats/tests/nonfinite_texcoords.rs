//! Where the NaN half-float texture coordinates of the shipped object
//! libraries sit (docs/SKY.md section 5): every triangle grid-app draws from a
//! track's `objects.pssg` (closest LOD), `tracksplit.pssg` and `land.pssg` is
//! checked for vertices whose UV0 is not finite.
use grid_formats::{mesh, pssg, scene};

const ROOT: &str = r"C:\Program Files (x86)\Steam\steamapps\common\Grid";

const TRACKS: [&str; 15] = [
    "europe/donington",
    "europe/istanbul_park",
    "europe/jarama",
    "europe/le_mans",
    "europe/milan",
    "europe/nurburgring",
    "europe/spa_francorchamps",
    "japan/haruna",
    "japan/okutama",
    "japan/shibuya",
    "japan/yokohama_docks",
    "usa/detroit",
    "usa/long_beach",
    "usa/san_fran",
    "usa/washington",
];

#[test]
#[ignore = "requires owner's purchased GRID installation"]
fn purchased_nonfinite_texcoords_are_drawn_only_by_world_mapped_infield_or_a_few_object_triangles()
{
    let root = std::env::var("GRID_GAME_ROOT").unwrap_or_else(|_| ROOT.into());
    let mut total_nonfinite = 0;
    let mut referenced_total = 0;
    for track in TRACKS {
        for package in ["objects.pssg", "tracksplit.pssg", "land.pssg"] {
            let path = std::path::Path::new(&root)
                .join("tracks")
                .join(track)
                .join(package);
            let bytes = std::fs::read(&path).unwrap();
            let tree = pssg::read_tree(&bytes).unwrap();
            let sources = mesh::read_static_meshes(&tree)
                .unwrap_or_else(|e| panic!("{track}/{package}: {e}"));
            let nonfinite: usize = sources.iter().map(|s| s.nonfinite_texcoords).sum();
            total_nonfinite += nonfinite;
            if nonfinite == 0 {
                continue;
            }
            let mut referenced = 0;
            let mut triangles = 0;
            let mut materials = std::collections::BTreeSet::new();
            // The object library draws its closest LOD the way grid-app does
            // (whole source, absolute indices); the track packages draw static
            // instances over their vertex range.
            let objects = package == "objects.pssg";
            let draws = if objects {
                scene::read_object_draws(&tree, &sources)
            } else {
                scene::read_static_draws(&tree, &sources)
            }
            .unwrap();
            for draw in draws {
                let source = &sources[draw.source];
                for &i in &draw.indices {
                    let base = if objects { 0 } else { draw.vertices.start };
                    let v = base + usize::from(i);
                    if source
                        .texcoords
                        .get(v)
                        .is_some_and(|uv| uv.iter().any(|c| !c.is_finite()))
                    {
                        referenced += 1;
                        materials.insert(String::from_utf8_lossy(draw.material).into_owned());
                    }
                }
                triangles += draw.indices.len() / 3;
            }
            println!(
                "nonfinite_texcoords {track}/{package} vertices={nonfinite} referenced_indices={referenced} drawn_triangles={triangles} materials={materials:?}"
            );
            if objects {
                referenced_total += referenced;
            } else {
                // UV0 is not read by track_infield_wsm (world-position UVs).
                for m in &materials {
                    let group = (0..tree.nodes.len())
                        .find(|&i| {
                            tree.name(i) == b"SHADERINSTANCE"
                                && tree.string(i, b"id").is_ok_and(|id| id == m.as_bytes())
                        })
                        .map(|i| tree.string(i, b"shaderGroup").unwrap());
                    assert_eq!(
                        group,
                        Some(b"#track_infield_wsm.fx".as_slice()),
                        "{track}/{package} {m}"
                    );
                }
            }
        }
    }
    assert!(
        total_nonfinite > 0,
        "the survey should meet the known NaN sources"
    );
    // Spa 21, Yokohama 2, Washington 2 object indices: the documented residue.
    println!(
        "nonfinite_texcoords total vertices={total_nonfinite} object_indices={referenced_total}"
    );
    assert_eq!(referenced_total, 25);
}
