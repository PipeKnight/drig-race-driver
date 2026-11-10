//! The team livery on a car (docs/LIVERY.md): the original's GPU livery
//! compositor (0x6C7D40, shaders `livery_map_livery_and_blend` and
//! `fill_gaps`) done on the CPU for any model with a `livery_user` package.
//! Each triangle of `<car>_livery.pssg` is drawn in its first ST space onto
//! the body texture; the pattern is sampled at the second ST, tinted by the
//! team's three colours and laid under the base by the base's alpha. Not
//! ported: the sponsor and number passes.
use super::CarModel;
use grid_formats::{livery_map, pssg, texture};
use std::{collections::BTreeMap, error::Error, path::Path};

/// The livery a team designed: the `livery_pattern.livery_image_name` file
/// and the primary, secondary and tertiary `livery_colour` RGB.
#[derive(Clone, Debug, PartialEq)]
pub struct Design {
    pub pattern: String,
    pub colours: [[u8; 3]; 3],
}

impl Design {
    /// `simple_01.pssg,RRGGBB,RRGGBB,RRGGBB` (the race profile's `team_livery`).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut parts = text.split(',').map(str::trim);
        let pattern = parts.next().filter(|p| !p.is_empty() && !p.contains(['/', '\\'])).ok_or("team_livery needs a pattern file")?;
        let mut colours = [[0; 3]; 3];
        for c in &mut colours {
            let hex = parts.next().filter(|h| h.len() == 6).ok_or("team_livery needs three RRGGBB colours")?;
            let v = u32::from_str_radix(hex, 16).map_err(|_| "bad team_livery colour")?;
            *c = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
        }
        if parts.next().is_some() {
            return Err("team_livery has extra fields".into());
        }
        Ok(Self { pattern: pattern.to_owned(), colours })
    }

    pub fn write(&self) -> String {
        let [a, b, c] = self.colours.map(|[r, g, b]| format!("{r:02x}{g:02x}{b:02x}"));
        format!("{},{a},{b},{c}", self.pattern)
    }
}

/// `cars/<car>/livery_user/textures_high/<car>_tex_high_user.pssg`.
pub fn package(model: &CarModel) -> String {
    format!("cars/{0}/livery_user/textures_high/{0}_tex_high_user.pssg", model.stem())
}

struct Image {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

fn decode(t: &texture::Texture<'_>) -> Result<Image, Box<dyn Error>> {
    let rgba = match t.format {
        texture::Format::Bgra8 => {
            let mut rgba = t.base_blocks.to_vec();
            rgba.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
            rgba
        }
        format => super::texture_upload::decode_levels(t.base_blocks, t.width, t.height, 1, format == texture::Format::Bc3)
            .ok_or("undecodable livery texture")?,
    };
    if rgba.len() != t.width as usize * t.height as usize * 4 {
        return Err("livery texture size".into());
    }
    Ok(Image { width: t.width as usize, height: t.height as usize, rgba })
}

/// Bilinear, clamped to the edge (sampler state not recovered).
fn sample(img: &Image, u: f32, v: f32) -> [f32; 3] {
    let x = (u * img.width as f32 - 0.5).clamp(0., (img.width - 1) as f32);
    let y = (v * img.height as f32 - 0.5).clamp(0., (img.height - 1) as f32);
    let (x0, y0) = (x as usize, y as usize);
    let (x1, y1) = ((x0 + 1).min(img.width - 1), (y0 + 1).min(img.height - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let p = |x: usize, y: usize, c: usize| img.rgba[(y * img.width + x) * 4 + c] as f32 / 255.;
    std::array::from_fn(|c| {
        let top = p(x0, y0, c) * (1. - fx) + p(x1, y0, c) * fx;
        let bottom = p(x0, y1, c) * (1. - fx) + p(x1, y1, c) * fx;
        top * (1. - fy) + bottom * fy
    })
}

/// `map_livery_and_alpha_blend` [V, shader disassembly]: the pattern's red,
/// green and blue weigh the three colours (clamped to 1); the base covers
/// the result by its alpha.
fn paint(pattern: [f32; 3], colours: &[[f32; 3]; 3], base: [u8; 4]) -> [u8; 3] {
    let a = base[3] as f32 / 255.;
    std::array::from_fn(|c| {
        let tinted = (pattern[0] * colours[0][c] + pattern[1] * colours[1][c] + pattern[2] * colours[2][c]).min(1.);
        let out = tinted + (base[c] as f32 / 255. - tinted) * a;
        (out * 255.).round().clamp(0., 255.) as u8
    })
}

/// The composited body texture: RGBA8 of the base's size, opaque. `covered`
/// counts the texels the livery map drew.
pub struct Composite {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub covered: usize,
}

/// Draws `maps` onto `base` with `pattern` and `colours`.
fn composite(base: &Image, pattern: &Image, maps: &[livery_map::LiveryMap<'_>], colours: [[u8; 3]; 3]) -> Composite {
    let (w, h) = (base.width, base.height);
    let tint = colours.map(|c| c.map(|v| v as f32 / 255.));
    let mut rgba = base.rgba.clone();
    let mut drawn = vec![false; w * h];
    for map in maps {
        for tri in map.indices.as_chunks::<3>().0 {
            let p = tri.map(|i| {
                let [u, v] = map.uv0[i as usize];
                [u * w as f32, v * h as f32]
            });
            let q = tri.map(|i| map.uv1[i as usize]);
            let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
            if !area.is_finite() || area.abs() < 1e-12 {
                continue;
            }
            let lo = |k: usize, n: usize| (p.iter().map(|p| p[k]).fold(f32::MAX, f32::min).floor().max(0.) as usize).min(n);
            let hi = |k: usize, n: usize| (p.iter().map(|p| p[k]).fold(f32::MIN, f32::max).ceil().max(0.) as usize).min(n);
            for y in lo(1, h)..hi(1, h) {
                for x in lo(0, w)..hi(0, w) {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let edge = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]) * (py - a[1]) - (b[1] - a[1]) * (px - a[0]);
                    let (w0, w1, w2) = (edge(p[1], p[2]) / area, edge(p[2], p[0]) / area, edge(p[0], p[1]) / area);
                    if w0 < 0. || w1 < 0. || w2 < 0. {
                        continue;
                    }
                    let u = w0 * q[0][0] + w1 * q[1][0] + w2 * q[2][0];
                    let v = w0 * q[0][1] + w1 * q[1][1] + w2 * q[2][1];
                    let at = (y * w + x) * 4;
                    let base_texel: [u8; 4] = base.rgba[at..at + 4].try_into().unwrap();
                    rgba[at..at + 3].copy_from_slice(&paint(sample(pattern, u, v), &tint, base_texel));
                    drawn[y * w + x] = true;
                }
            }
        }
    }
    let covered = drawn.iter().filter(|&&d| d).count();
    fill_gaps(&mut rgba, &drawn, w, h);
    for p in rgba.as_chunks_mut::<4>().0 {
        p[3] = 255;
    }
    Composite { width: w, height: h, rgba, covered }
}

/// `fill_gaps` [V, shader disassembly], one pass: a texel the map did not
/// draw takes the first drawn of its eight neighbours (right, left, down,
/// up, then the diagonals). The pass count is [I].
fn fill_gaps(rgba: &mut [u8], drawn: &[bool], w: usize, h: usize) {
    const ORDER: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)];
    let source = rgba.to_vec();
    for y in 0..h {
        for x in 0..w {
            if drawn[y * w + x] {
                continue;
            }
            let found = ORDER.iter().find_map(|&(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                (nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && drawn[ny as usize * w + nx as usize])
                    .then(|| ny as usize * w + nx as usize)
            });
            if let Some(n) = found {
                rgba[(y * w + x) * 4..][..3].copy_from_slice(&source[n * 4..n * 4 + 3]);
            }
        }
    }
}

/// Every texture of `model`'s `livery_user` package, keyed `prefix` + id,
/// with `<car>_main.tga` replaced by the composite of `design`.
pub fn load(root: &Path, model: &CarModel, prefix: &str, design: &Design) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn Error>> {
    let bytes = super::read(&root.join(package(model)))?;
    let tree = pssg::read_tree(&bytes)?;
    let main = model.texture("main");
    let mut out = BTreeMap::new();
    let mut base = None;
    for t in texture::read_textures(&tree)? {
        if t.id == main.as_bytes() {
            base = Some(decode(&t)?);
        }
        out.insert(format!("{prefix}{}", String::from_utf8(t.id.to_vec())?), t.validated_dds().0);
    }
    let base = base.ok_or("livery_user package without the main texture")?;
    let pattern_bytes = super::read(&root.join("cars/user_liveries/liveries").join(&design.pattern))?;
    let pattern_tree = pssg::read_tree(&pattern_bytes)?;
    let pattern = texture::read_textures(&pattern_tree)?.first().map(decode).ok_or("pattern file without a texture")??;
    let map_bytes = super::read(&root.join(model.file("_livery.pssg")))?;
    let map_tree = pssg::read_tree(&map_bytes)?;
    let maps = livery_map::read(&map_tree)?;
    let c = composite(&base, &pattern, &maps, design.colours);
    println!(
        "team_livery car={} pattern={} colours={} sources={} covered={} size={}x{}",
        model.stem(),
        design.pattern,
        design.write().split_once(',').map_or("", |s| s.1),
        maps.len(),
        c.covered,
        c.width,
        c.height
    );
    out.insert(format!("{prefix}{main}"), super::car_numbers::opaque_dds(c.rgba, c.width, c.height));
    Ok(out)
}

/// Whether `model` ships a `livery_user` package (the `*_user` rows of
/// `vehicle_livery`).
pub fn available(root: &Path, model: &CarModel) -> bool {
    root.join(package(model)).is_file()
}

/// Puts `design` on the player (`car.textures`) and on opponent `teammate`
/// (from 1; `car.liveries[teammate - 1]`). A model without a `livery_user`
/// package keeps its shipped livery. Other opponents of the player's model
/// fall back to `car.textures` for a texture their bank lacks, so those
/// banks first get the player's shipped texture. Everything is staged
/// before any bank changes. Returns the number of cars repainted.
pub fn apply(
    root: &Path,
    models: &[CarModel],
    car_model: &[usize],
    car: &mut super::Car,
    teammate: Option<usize>,
    design: &Design,
) -> Result<usize, Box<dyn Error>> {
    let player = available(root, &models[0])
        .then(|| load(root, &models[0], if car.track { "car0/" } else { "" }, design))
        .transpose()?;
    let mate = match teammate.filter(|&k| k > 0 && k <= car.liveries.len()) {
        Some(k) if available(root, &models[car_model[k]]) => {
            Some((k, load(root, &models[car_model[k]], &super::model_prefix(car_model[k]), design)?))
        }
        _ => None,
    };
    let mut painted = 0;
    if let Some(player) = player {
        for (bank, _) in car.liveries.iter_mut().zip(1..).filter(|&(_, k)| car_model[k] == 0 && Some(k) != teammate) {
            for key in player.keys() {
                if !bank.contains_key(key) && let Some(shipped) = car.textures.get(key) {
                    bank.insert(key.clone(), shipped.clone());
                }
            }
        }
        car.textures.extend(player);
        painted += 1;
    }
    if let Some((k, textures)) = mate {
        car.liveries[k - 1].extend(textures);
        painted += 1;
    }
    Ok(painted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_round_trips_and_rejects_malformed_text() {
        let d = Design::parse("simple_01.pssg,ff0000,00Ff80,000001").unwrap();
        assert_eq!(d, Design { pattern: "simple_01.pssg".into(), colours: [[255, 0, 0], [0, 255, 128], [0, 0, 1]] });
        assert_eq!(Design::parse(&d.write()).unwrap(), d);
        for bad in ["", "simple_01.pssg", "a/b.pssg,000000,000000,000000", "p,000000,000000", "p,000000,000000,00000g", "p,0,0,0", "p,000000,000000,000000,000000"] {
            assert!(Design::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn paint_tints_the_pattern_under_the_base_alpha() {
        let colours = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
        // Base alpha 0: the pattern's channels pick the colours, clamped to 1.
        assert_eq!(paint([1., 0.5, 0.], &colours, [9, 9, 9, 0]), [255, 128, 0]);
        assert_eq!(paint([1., 1., 1.], &[[1., 1., 1.]; 3], [0, 0, 0, 0]), [255, 255, 255]);
        // Base alpha 1: the base stays.
        assert_eq!(paint([1., 1., 1.], &colours, [10, 20, 30, 255]), [10, 20, 30]);
    }

    #[test]
    fn composite_draws_in_body_space_and_fills_one_texel_of_gap() {
        let base = Image { width: 8, height: 8, rgba: [0, 0, 0, 0].repeat(64) };
        let pattern = Image { width: 1, height: 1, rgba: vec![255, 0, 0, 255] };
        // A triangle over the left half of the body texture.
        let map = livery_map::LiveryMap {
            id: b"t",
            uv0: vec![[0., 0.], [0.5, 0.], [0., 1.]],
            uv1: vec![[0.5, 0.5]; 3],
            indices: vec![0, 1, 2],
        };
        let c = composite(&base, &pattern, &[map], [[0, 0, 255], [0; 3], [0; 3]]);
        let at = |x: usize, y: usize| &c.rgba[(y * 8 + x) * 4..][..4];
        assert!(c.covered > 0 && c.covered < 32, "{}", c.covered);
        assert_eq!(at(0, 0), [0, 0, 255, 255], "drawn");
        assert_eq!(at(7, 7), [0, 0, 0, 255], "far texel keeps the base, made opaque");
        // The first undrawn texel right of row 0's edge takes its drawn neighbour.
        let edge = (0..8).find(|&x| at(x, 0) != [0, 0, 255, 255]).unwrap();
        assert!(edge < 7 && at(edge + 1, 0) == [0, 0, 0, 255], "one pass fills one texel");
    }


    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn every_livery_user_car_composites_every_pattern_file() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let mut models: Vec<CarModel> = std::fs::read_dir(root.join("cars"))
            .unwrap()
            .filter_map(|e| CarModel::new(e.ok()?.file_name().to_str()?).ok())
            .filter(|m| available(&root, m))
            .collect();
        models.sort_by(|a, b| a.stem().cmp(b.stem()));
        assert!(models.len() >= 40, "{} livery_user cars", models.len());
        let patterns: Vec<String> = std::fs::read_dir(root.join("cars/user_liveries/liveries"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        let png = std::env::var_os("GRID_TEAM_LIVERY_PNG").map(std::path::PathBuf::from);
        if let Some(dir) = &png {
            std::fs::create_dir_all(dir).unwrap();
        }
        for (i, model) in models.iter().enumerate() {
            // Each car with one pattern; the patterns rotate over the cars.
            let design = Design { pattern: patterns[i % patterns.len()].clone(), colours: [[200, 30, 30], [240, 240, 240], [20, 40, 160]] };
            let out = load(&root, model, "car0/", &design).unwrap_or_else(|e| panic!("{}: {e}", model.stem()));
            let main = &out[&format!("car0/{}", model.texture("main"))];
            let bevy_image = super::super::texture_upload::image(main, Default::default(), bevy::asset::RenderAssetUsages::MAIN_WORLD).unwrap();
            // Only the body-paint triangles are drawn: drawing every map
            // triangle (wheels, cabin, lights) painted almost the whole
            // texture in the primary colour.
            let texels = (bevy_image.width() * bevy_image.height()) as usize;
            let bgra = &bevy_image.data.as_ref().unwrap()[..texels * 4];
            let near = |c: [u8; 3]| {
                bgra.as_chunks::<4>().0.iter().filter(|p| (0..3).all(|k| p[2 - k].abs_diff(c[k]) <= 8)).count() as f64 / texels as f64
            };
            let (primary, tertiary) = (near(design.colours[0]), near(design.colours[2]));
            println!("team_livery_check car={} pattern={} primary={primary:.3} tertiary={tertiary:.4}", model.stem(), design.pattern);
            assert!(primary < 0.6, "{}: {primary:.3} of the texture in the primary colour", model.stem());
            if let Some(dir) = &png {
                let rgba = bevy_image.data.as_ref().unwrap();
                let (w, h) = (bevy_image.width(), bevy_image.height());
                let mut top: Vec<u8> = rgba[..(w * h * 4) as usize].to_vec();
                top.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
                let img = bevy::image::Image::new(
                    bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    bevy::render::render_resource::TextureDimension::D2,
                    top,
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    bevy::asset::RenderAssetUsages::MAIN_WORLD,
                );
                img.try_into_dynamic().unwrap().save(dir.join(format!("{}.png", model.stem()))).unwrap();
            }
        }
        for pattern in &patterns {
            let design = Design { pattern: pattern.clone(), colours: [[255, 0, 0], [0, 255, 0], [0, 0, 255]] };
            load(&root, &models[0], "", &design).unwrap_or_else(|e| panic!("{pattern}: {e}"));
        }
    }
}
