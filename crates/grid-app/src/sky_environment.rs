//! Authored source-sky cubemap projection. Geometry/UVs come from purchased sky.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
    },
};
use grid_formats::texture::{Format, Texture};

struct Triangle {
    a: Vec3,
    e1: Vec3,
    e2: Vec3,
    uv: [Vec2; 3],
}

fn direction(face: u32, u: f32, v: f32) -> Vec3 {
    let (u, v) = (u * 2. - 1., v * 2. - 1.);
    let mut d = match face {
        0 => Vec3::new(1., -v, -u),
        1 => Vec3::new(-1., -v, u),
        2 => Vec3::new(u, 1., v),
        3 => Vec3::new(u, -1., -v),
        4 => Vec3::new(u, -v, 1.),
        5 => Vec3::new(-u, -v, -1.),
        _ => unreachable!(),
    };
    // Bevy's environment sampler flips Z for left-handed cubemaps.
    // Match installed atmosphere/environment.wgsl's generation convention.
    d.z = -d.z;
    d.normalize()
}

fn hit(d: Vec3, tri: &Triangle) -> Option<(f32, Vec2)> {
    let p = d.cross(tri.e2);
    let det = tri.e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = det.recip();
    let tvec = -tri.a;
    let u = tvec.dot(p) * inv;
    if !(-1e-5..=1.00001).contains(&u) {
        return None;
    }
    let q = tvec.cross(tri.e1);
    let v = d.dot(q) * inv;
    if v < -1e-5 || u + v > 1.00001 {
        return None;
    }
    let t = tri.e2.dot(q) * inv;
    (t > 0.).then(|| (t, tri.uv[0] * (1. - u - v) + tri.uv[1] * u + tri.uv[2] * v))
}

fn bc1_pixel(tex: &Texture<'_>, x: u32, y: u32) -> [u8; 3] {
    let i = ((y / 4) * tex.width.div_ceil(4) + x / 4) as usize * 8;
    let b = &tex.base_blocks[i..i + 8];
    let a = u16::from_le_bytes([b[0], b[1]]);
    let z = u16::from_le_bytes([b[2], b[3]]);
    fn rgb(c: u16) -> [u8; 3] {
        let r = (c >> 11) & 31;
        let g = (c >> 5) & 63;
        let b = c & 31;
        [
            ((r << 3) | (r >> 2)) as u8,
            ((g << 2) | (g >> 4)) as u8,
            ((b << 3) | (b >> 2)) as u8,
        ]
    }
    let (c0, c1) = (rgb(a), rgb(z));
    let index =
        (u32::from_le_bytes(b[4..8].try_into().unwrap()) >> (2 * ((y % 4) * 4 + x % 4))) & 3;
    match index {
        0 => c0,
        1 => c1,
        2 if a > z => {
            std::array::from_fn(|i| ((2 * u16::from(c0[i]) + u16::from(c1[i])) / 3) as u8)
        }
        3 if a > z => {
            std::array::from_fn(|i| ((u16::from(c0[i]) + 2 * u16::from(c1[i])) / 3) as u8)
        }
        2 => std::array::from_fn(|i| ((u16::from(c0[i]) + u16::from(c1[i])) / 2) as u8),
        _ => [0; 3],
    }
}

fn sample(tex: &Texture<'_>, uv: Vec2, linear: &[f32; 256]) -> [u8; 3] {
    let p = Vec2::new(
        uv.x.rem_euclid(1.) * tex.width as f32 - 0.5,
        uv.y.clamp(0., 1.) * tex.height as f32 - 0.5,
    );
    let base = p.floor();
    let f = p - base;
    let colors: [Vec3; 4] = std::array::from_fn(|i| {
        let x = (base.x as i32 + (i % 2) as i32).rem_euclid(tex.width as i32) as u32;
        let y = (base.y as i32 + (i / 2) as i32).clamp(0, tex.height as i32 - 1) as u32;
        Vec3::from_array(bc1_pixel(tex, x, y).map(|v| linear[v as usize]))
    });
    colors[0]
        .lerp(colors[1], f.x)
        .lerp(colors[2].lerp(colors[3], f.x), f.y)
        .to_array()
        .map(|v| {
            let s = if v <= 0.0031308 {
                v * 12.92
            } else {
                1.055 * v.powf(1. / 2.4) - 0.055
            };
            (s.clamp(0., 1.) * 255.).round() as u8
        })
}

pub fn bake(
    positions: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u16],
    tex: &Texture<'_>,
) -> Result<Image, &'static str> {
    if tex.format != Format::Bc1
        || tex.width == 0
        || tex.height == 0
        || tex.width > 16384
        || tex.height > 16384
        || tex.base_blocks.len() as u64
            != u64::from(tex.width.div_ceil(4)) * u64::from(tex.height.div_ceil(4)) * 8
        || positions
            .iter()
            .flatten()
            .chain(uvs.iter().flatten())
            .any(|v| !v.is_finite())
        || positions.len() != uvs.len()
        || positions.len() > 4096
        || indices.len() > 6144
        || !indices.len().is_multiple_of(3)
        || indices.iter().any(|&i| i as usize >= positions.len())
    {
        return Err("unsupported sky projection inputs");
    }
    let triangles: Vec<_> = indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|ix| {
            let p = ix.map(|i| Vec3::from_array(positions[i as usize]) / 20000.);
            Triangle {
                a: p[0],
                e1: p[1] - p[0],
                e2: p[2] - p[0],
                uv: ix.map(|i| Vec2::from_array(uvs[i as usize])),
            }
        })
        .collect();
    let linear: [f32; 256] = std::array::from_fn(|i| {
        let s = i as f32 / 255.;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    });
    const SIZE: u32 = 256;
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 6 * 4) as usize);
    let (mut hits, mut upper_misses) = (0, 0);
    for face in 0..6 {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let d = direction(
                    face,
                    (x as f32 + 0.5) / SIZE as f32,
                    (y as f32 + 0.5) / SIZE as f32,
                );
                let nearest = triangles
                    .iter()
                    .filter_map(|tri| hit(d, tri))
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                let rgb = if let Some((_, uv)) = nearest {
                    hits += 1;
                    sample(tex, uv, &linear)
                } else {
                    if d.y > 0. {
                        upper_misses += 1;
                    }
                    // Authored lower-hemisphere ground approximation; no captured track reflections.
                    [67, 63, 52]
                };
                pixels.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    if upper_misses > SIZE * SIZE / 100 {
        return Err("source sky leaves excessive upper-hemisphere holes");
    }
    println!(
        "source_sky_ibl cube=256 faces=6 source_hits={hits} upper_misses={upper_misses} lower_fill=authored-ground radiometry=authored"
    );
    let mut image = Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_and_bc1_endpoints() {
        assert_eq!(direction(0, 0.5, 0.5), Vec3::X);
        assert_eq!(direction(2, 0.5, 0.5), Vec3::Y);
        assert_eq!(direction(4, 0.5, 0.5), Vec3::NEG_Z);
        let t = Triangle {
            a: Vec3::new(-1., -1., 2.),
            e1: Vec3::new(2., 0., 0.),
            e2: Vec3::new(0., 2., 0.),
            uv: [Vec2::ZERO, Vec2::X, Vec2::Y],
        };
        let (distance, uv) = hit(Vec3::Z, &t).unwrap();
        assert_eq!(distance, 2.);
        assert_eq!(uv, Vec2::splat(0.5));
        assert!(hit(Vec3::NEG_Z, &t).is_none());
        let tex = Texture {
            id: b"synthetic",
            width: 4,
            height: 4,
            format: Format::Bc1,
            declared_mip_levels: 0,
            base_blocks: &[0, 248, 224, 7, 0b11100100, 0, 0, 0],
            remaining_data: &[],
        };
        assert_eq!(bc1_pixel(&tex, 0, 0), [255, 0, 0]);
        assert_eq!(bc1_pixel(&tex, 1, 0), [0, 255, 0]);
        assert_eq!(bc1_pixel(&tex, 2, 0), [170, 85, 0]);
        assert_eq!(bc1_pixel(&tex, 3, 0), [85, 170, 0]);
    }
}
