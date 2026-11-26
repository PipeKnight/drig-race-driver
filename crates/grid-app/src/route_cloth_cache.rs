//! Restore the authored startup pose of route cloth from the installed cache.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use grid_formats::{binxml, entities, material, pssg, scene};
use grid_tools::route::Route;
use std::{collections::BTreeMap, error::Error, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) const MATERIAL_PREFIX: &str = "cloth/";

#[derive(Default)]
struct ClothBatch {
    positions: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

struct Shape {
    uv: Vec<[[f32; 2]; 2]>,
    quads: Vec<[usize; 4]>,
}

fn field<'a>(row: &binxml::Element<'a>, name: &str) -> Result<&'a str> {
    row.attributes
        .iter()
        .find_map(|&(key, value)| (key == name).then_some(value))
        .ok_or_else(|| format!("cloth row {} lacks {name}", row.name).into())
}

fn shapes(bytes: &[u8]) -> Result<BTreeMap<&str, Shape>> {
    let doc = binxml::read(bytes)?;
    let root = doc
        .elements
        .first()
        .filter(|row| row.name == "CLOTHS")
        .ok_or("expected CLOTHS root")?;
    let mut out = BTreeMap::new();
    for id in &doc.elements[root.children.clone()] {
        let template = doc
            .elements
            .get(id.children.start)
            .filter(|row| row.name == "CLOTHTEMPLATE")
            .ok_or("cloth template absent")?;
        let rows = |name| -> Result<&[binxml::Element<'_>]> {
            let section = doc.elements[template.children.clone()]
                .iter()
                .find(|row| row.name == name)
                .ok_or_else(|| format!("cloth section {name} absent"))?;
            Ok(&doc.elements[section.children.clone()])
        };
        let links = rows("CLOTHLINKS")?
            .iter()
            .map(|r| -> Result<[usize; 2]> {
                Ok([field(r, "vert0")?.parse()?, field(r, "vert1")?.parse()?])
            })
            .collect::<Result<Vec<_>>>()?;
        let uv = rows("CLOTHRENDERQUADVERTICES")?
            .iter()
            .map(|r| -> Result<[[f32; 2]; 2]> {
                Ok([
                    [field(r, "u0")?.parse()?, field(r, "v0")?.parse()?],
                    [field(r, "u1")?.parse()?, field(r, "v1")?.parse()?],
                ])
            })
            .collect::<Result<Vec<_>>>()?;
        let quads = rows("CLOTHRENDERQUADS")?
            .iter()
            .map(|r| -> Result<[usize; 4]> {
                let mut corners = [0; 4];
                let mut ends = [0; 4];
                for (i, key) in ["link0", "link1", "link2", "link3"].into_iter().enumerate() {
                    let signed: i32 = field(r, key)?.parse()?;
                    let Some(index) = signed.unsigned_abs().checked_sub(1) else {
                        return Err("cloth quad link zero".into());
                    };
                    let &[a, b] = links
                        .get(index as usize)
                        .ok_or("cloth quad link out of bounds")?;
                    [corners[i], ends[i]] = if signed < 0 { [b, a] } else { [a, b] };
                }
                if (0..4).any(|i| ends[i] != corners[(i + 1) % 4] || corners[i] >= uv.len()) {
                    return Err("invalid cloth quad boundary".into());
                }
                Ok(corners)
            })
            .collect::<Result<Vec<_>>>()?;
        if out.insert(field(id, "ID")?, Shape { uv, quads }).is_some() {
            return Err("duplicate cloth template ID".into());
        }
    }
    Ok(out)
}

struct State {
    origin: Vec3,
    positions: Vec<[f32; 3]>,
}

fn states(mut bytes: &[u8]) -> Result<Vec<State>> {
    fn take<'a>(bytes: &mut &'a [u8], size: usize) -> Result<&'a [u8]> {
        let (head, tail) = bytes
            .split_at_checked(size)
            .ok_or("truncated cloth cache")?;
        *bytes = tail;
        Ok(head)
    }
    fn word(bytes: &[u8]) -> u32 {
        u32::from_be_bytes(bytes.try_into().expect("four-byte cloth field"))
    }
    fn xyz(bytes: &[u8]) -> Vec3 {
        Vec3::new(
            f32::from_bits(word(&bytes[..4])),
            f32::from_bits(word(&bytes[4..8])),
            f32::from_bits(word(&bytes[8..12])),
        )
    }
    let count = word(take(&mut bytes, 4)?) as usize;
    if count > bytes.len() / 28 {
        return Err("impossible cloth state count".into());
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let header = take(&mut bytes, 28)?;
        let origin = xyz(header);
        let valid = word(&header[12..16]);
        let mut payload = take(&mut bytes, word(&header[16..20]) as usize)?;
        if valid == 0 {
            continue;
        }
        if valid != 1 || !origin.is_finite() {
            return Err("invalid cloth state header".into());
        }
        let counts = take(&mut payload, 2)?
            .try_into()
            .expect("two-byte cloth count");
        let particles = usize::from(u16::from_be_bytes(counts));
        let offset = xyz(take(&mut payload, 12)?);
        let scale = xyz(take(&mut payload, 12)?);
        if !offset.is_finite() || !scale.is_finite() || scale.cmpeq(Vec3::ZERO).any() {
            return Err("invalid cloth state quantization".into());
        }
        let compact = take(&mut payload, particles * 3)?;
        let (triples, _) = compact.as_chunks::<3>();
        let positions = triples
            .iter()
            .map(|&[x, y, z]| (offset + Vec3::new(x.into(), y.into(), z.into()) / scale).to_array())
            .collect();
        out.push(State { origin, positions });
    }
    if !bytes.is_empty() {
        return Err("trailing cloth cache bytes".into());
    }
    Ok(out)
}

pub(crate) fn append(
    root: &Path,
    route: &Route,
    car: &mut crate::Car,
    types: &pssg::Tree<'_>,
    objects: &pssg::Tree<'_>,
    entities: &[entities::Entity<'_>],
) -> Result<()> {
    if crate::dev::var("GRID_ROUTE_CLOTH_CACHE").as_deref() == Ok("off")
        || entities.iter().all(|entity| entity.cloth.is_empty())
    {
        return Ok(());
    }
    let asset_path = root.join(route.track_file("cloth.xml"));
    let cache_path = root.join(route.file("clothFile.bin"));
    if !asset_path.is_file() || !cache_path.is_file() {
        return Ok(());
    }
    let xml = std::fs::read(asset_path)?;
    let shapes = shapes(&xml)?;
    let states = states(&std::fs::read(cache_path)?)?;
    if states.is_empty() {
        return Ok(());
    }
    let mut strings = BTreeMap::new();
    for (i, node) in objects
        .nodes
        .iter()
        .enumerate()
        .filter(|&(i, _)| objects.name(i) == b"PNSTRING")
    {
        let Some(data) = node.children.iter().find_map(|&j| objects.nodes[j].data) else {
            continue;
        };
        let id = objects.string(i, b"id")?;
        let name = data.strip_suffix(&[0]).unwrap_or(data);
        strings.insert(id, name);
    }
    let pools = types
        .nodes
        .iter()
        .enumerate()
        .filter(|&(i, _)| types.name(i) == b"TEMPLATECLOTHTYPEPOOL")
        .map(|(i, _)| Ok((types.string(i, b"id")?, i)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let components = types
        .nodes
        .iter()
        .enumerate()
        .filter(|&(i, _)| types.name(i) == b"TEMPLATECLOTH")
        .map(|(i, _)| Ok((types.string(i, b"id")?, i)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let materials = material::read_texture_bindings(objects)?;
    let mut batches = BTreeMap::<(&[u8], usize), ClothBatch>::new();
    let mut used = vec![false; states.len()];
    let mut unmatched = 0;
    for (entity, ordinal, component) in entities.iter().flat_map(|entity| {
        entity
            .cloth
            .iter()
            .enumerate()
            .map(move |(ordinal, component)| (entity, ordinal, component))
    }) {
        let id = *components.get(component).ok_or("cloth component missing")?;
        let pool_ref = types
            .string(id, b"uri")?
            .strip_prefix(b"#")
            .ok_or("cloth pool URI")?;
        let pool = *pools.get(pool_ref).ok_or("cloth pool missing")?;
        let shape_ref = types
            .string(pool, b"data")?
            .strip_prefix(b"objects.pssg#")
            .ok_or("cloth data URI")?;
        let shape_name = std::str::from_utf8(strings.get(shape_ref).ok_or("cloth ID missing")?)?;
        let shape = shapes.get(shape_name).ok_or("cloth shape missing")?;
        let transform = types.nodes[id]
            .children
            .iter()
            .copied()
            .find(|&j| types.name(j) == b"TEMPLATETRANSFORM")
            .ok_or("cloth transform missing")?;
        let node = &types.nodes[transform];
        let local = scene::decode_matrix(
            node.data.ok_or("cloth transform bytes missing")?,
            node.offset,
        )?;
        let placed = scene::compose(&local, &entity.transform);
        // Components on one entity can share the same placed origin. The
        // authored cache keys the nth cloth by the origin multiplied by n.
        // Decoded particles already use world coordinates.
        let origin = Vec3::from_slice(&placed[12..15]) * (ordinal + 1) as f32;
        let mut matches = states.iter().enumerate().filter(|&(i, s)| {
            !used[i]
                && s.positions.len() == shape.uv.len()
                && s.origin.distance_squared(origin) < 1e-6
        });
        let Some((index, state)) = matches.next() else {
            unmatched += 1;
            continue;
        };
        // Coincident entities can own duplicate records. Equivalent points
        // are interchangeable, so consume a single record per component.
        if matches.any(|(_, alternative)| alternative.positions != state.positions) {
            unmatched += 1;
            continue;
        }
        used[index] = true;
        for (side, attribute) in [b"shaderfront".as_slice(), b"shaderback"]
            .into_iter()
            .enumerate()
        {
            let link = types.string(pool, attribute)?;
            let shader = link
                .strip_prefix(b"objects.pssg#")
                .or_else(|| link.strip_prefix(b"#"))
                .unwrap_or(link);
            let batch = batches.entry((shader, side)).or_default();
            let end = u32::try_from(batch.positions.len() + state.positions.len())?;
            let base = end - state.positions.len() as u32;
            batch.positions.extend_from_slice(&state.positions);
            batch.uv.extend(shape.uv.iter().map(|uv| uv[side]));
            let corners = if side == 0 {
                [0, 1, 2, 0, 2, 3]
            } else {
                [2, 1, 0, 3, 2, 0]
            };
            for quad in &shape.quads {
                batch
                    .indices
                    .extend(corners.map(|corner| base + quad[corner] as u32));
            }
        }
    }
    let mut parts = Vec::new();
    for (
        (shader, side),
        ClothBatch {
            positions,
            uv,
            indices,
        },
    ) in batches
    {
        if positions.is_empty() {
            continue;
        }
        let Some(binding) = materials
            .iter()
            .find(|b| b.shader == shader && b.sampler == b"TDiffuseAlphaMap")
        else {
            return Err(format!(
                "cloth diffuse binding missing for {}",
                String::from_utf8_lossy(shader)
            )
            .into());
        };
        let texture = format!(
            "{}#{}",
            std::str::from_utf8(binding.container.unwrap_or(b"objectstextures.pssg"))?,
            std::str::from_utf8(binding.texture)?
        );
        if !car.textures.contains_key(&texture) {
            return Err(format!("cloth diffuse texture missing {texture}").into());
        }
        let name = format!("{MATERIAL_PREFIX}{}-{side}", std::str::from_utf8(shader)?);

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        // The source accumulates one unit normal per incident triangle, then
        // normalizes the sum at each render point. Each face has its own UVs.
        let mut normal_sum = vec![Vec3::ZERO; positions.len()];
        for &[a, b, c] in indices.as_chunks::<3>().0 {
            let vertices = [a, b, c].map(|index| Vec3::from_array(positions[index as usize]));
            let face_normal = (vertices[1] - vertices[0]).cross(vertices[2] - vertices[0]);
            if face_normal.length_squared() > 1e-8 {
                let unit = face_normal.normalize();
                for index in [a, b, c] {
                    normal_sum[index as usize] += unit;
                }
            }
        }
        let normals: Vec<_> = normal_sum
            .into_iter()
            .map(|normal| normal.normalize_or_zero().to_array())
            .collect();
        mesh.insert_indices(Indices::U32(indices));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        parts.push((
            texture,
            crate::Part {
                name: name.clone(),
                material: name,
                joint: String::new(),
                mesh,
                matrices: vec![Mat4::IDENTITY],
                wheel: None,
            },
        ));
    }
    let draws = parts.len();
    for (texture, part) in parts {
        car.bindings.insert(part.material.clone(), texture);
        car.parts.push(part);
    }
    println!(
        "cloth_cache states={} restored={} unmatched={} draws={draws}",
        states.len(),
        used.iter().filter(|&&v| v).count(),
        unmatched
    );
    Ok(())
}
