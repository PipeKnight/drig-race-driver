//! Opt-in CPU number composition from purchased glyphs/UV slots.
//! Equal-width digit layout is authored; original runtime selection is not recovered.
use super::CarModel;
use grid_formats::{
    car_decals::{Number, Placement},
    pssg,
    texture::{self, Format, Texture},
};
use std::{collections::BTreeMap, error::Error, path::Path};

struct Glyph {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}
fn linear(v: u8) -> f32 {
    let v = v as f32 / 255.;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn encoded(v: f32) -> u8 {
    let v = if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    };
    (v * 255.).round().clamp(0., 255.) as u8
}
fn table() -> [f32; 256] {
    std::array::from_fn(|i| linear(i as u8))
}

// Interpolate premultiplied linear RGB, with a transparent border; no white fringe.
fn sample(g: &Glyph, u: f32, v: f32, lut: &[f32; 256]) -> [f32; 4] {
    let x = u * g.width as f32 - 0.5;
    let y = v * g.height as f32 - 0.5;
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let mut out = [0.; 4];
    for (dx, wx) in [(0, 1. - fx), (1, fx)] {
        for (dy, wy) in [(0, 1. - fy), (1, fy)] {
            let (x, y) = (ix + dx, iy + dy);
            if x < 0 || y < 0 || x as usize >= g.width || y as usize >= g.height {
                continue;
            }
            let p = &g.rgba[(y as usize * g.width + x as usize) * 4..][..4];
            let a = p[3] as f32 / 255. * wx * wy;
            for c in 0..3 {
                out[c] += lut[p[c] as usize] * a;
            }
            out[3] += a;
        }
    }
    out
}
fn coordinate(p: Placement, u: f32, v: f32) -> [f32; 2] {
    let (sin, cos) = p.rotation_degrees.to_radians().sin_cos();
    let (dx, dy) = (u - p.centre[0], v - p.centre[1]);
    let x = (cos * dx + sin * dy) / p.size[0] + 0.5;
    let y = 0.5 - (-sin * dx + cos * dy) / p.size[1];
    [if p.flip { 1. - x } else { x }, y]
}
fn bounds(p: Placement, width: usize, height: usize) -> Result<[usize; 4], Box<dyn Error>> {
    let (s, c) = p.rotation_degrees.to_radians().sin_cos();
    let rx = (c.abs() * p.size[0] + s.abs() * p.size[1]) * 0.5;
    let ry = (s.abs() * p.size[0] + c.abs() * p.size[1]) * 0.5;
    let b = [
        p.centre[0] - rx,
        1. - p.centre[1] - ry,
        p.centre[0] + rx,
        1. - p.centre[1] + ry,
    ];
    if b.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
        return Err("number decal crosses atlas boundary".into());
    }
    Ok([
        (b[0] * width as f32).floor() as usize,
        (b[1] * height as f32).floor() as usize,
        ((b[2] * width as f32).ceil() as usize).min(width),
        ((b[3] * height as f32).ceil() as usize).min(height),
    ])
}
fn base(dds: &[u8]) -> Result<(usize, usize, Vec<u8>), Box<dyn Error>> {
    if dds.len() < 128 || &dds[..4] != b"DDS " {
        return Err("number base needs classic DDS".into());
    }
    let word = |i| u32::from_le_bytes(dds[i..i + 4].try_into().unwrap());
    let (w, h) = (word(16), word(12));
    if word(4) != 124
        || word(76) != 32
        || !w.is_power_of_two()
        || !h.is_power_of_two()
        || w > 8192
        || h > 8192
    {
        return Err("number base dimensions/envelope unsupported".into());
    }
    let rgba = match (&dds[84..88], word(80)) {
        (b"DXT1", 4) => super::texture_upload::decode_levels(&dds[128..], w, h, 1, false),
        (b"DXT5", 4) => super::texture_upload::decode_levels(&dds[128..], w, h, 1, true),
        ([0, 0, 0, 0], 0x41)
            if word(88) == 32
                && word(92) == 0xff0000
                && word(96) == 0xff00
                && word(100) == 0xff
                && word(104) == 0xff000000 =>
        {
            dds.get(128..128 + w as usize * h as usize * 4).map(|b| {
                let mut b = b.to_vec();
                for p in b.as_chunks_mut::<4>().0 {
                    p.swap(0, 2);
                }
                b
            })
        }
        _ => None,
    }
    .ok_or("unsupported/truncated number base pixels")?;
    if rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255) {
        return Err("number composition only supports opaque body diffuse".into());
    }
    Ok((w as usize, h as usize, rgba))
}
fn mip_dds(mut rgba: Vec<u8>, width: usize, height: usize, lut: &[f32; 256]) -> Vec<u8> {
    let base_size = rgba.len();
    let (mut w, mut h, mut offset, mut levels) = (width, height, 0, 1);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let next_offset = rgba.len();
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..3 {
                    let mut sum = 0.;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let at = offset
                                + (((y * 2 + dy).min(h - 1) * w + (x * 2 + dx).min(w - 1)) * 4)
                                + c;
                            sum += lut[rgba[at] as usize];
                        }
                    }
                    rgba.push(encoded(sum * 0.25));
                }
                rgba.push(255);
            }
        }
        (w, h, offset) = (nw, nh, next_offset);
        levels += 1;
    }
    for p in rgba.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    Texture {
        id: b"numbered_body",
        width: width as u32,
        height: height as u32,
        format: Format::Bgra8,
        declared_mip_levels: levels - 1,
        base_blocks: &rgba[..base_size],
        remaining_data: &rgba[base_size..],
    }
    .validated_dds()
    .0
}
fn compose(
    dds: &[u8],
    placements: &[Placement],
    glyphs: &BTreeMap<u8, Glyph>,
    number: Number,
) -> Result<Vec<u8>, Box<dyn Error>> {
    if placements.is_empty() || placements.len() > 70 {
        return Err("invalid number or number placements".into());
    }
    let digits = number.digits();
    if digits.iter().any(|d| !glyphs.contains_key(d)) {
        return Err("number digit missing".into());
    }
    let (width, height, mut rgba) = base(dds)?;
    // Validate all slots before changing any CPU texel.
    let boxes = placements
        .iter()
        .map(|p| bounds(*p, width, height))
        .collect::<Result<Vec<_>, _>>()?;
    let lut = table();
    for (p, b) in placements.iter().zip(boxes) {
        for y in b[1]..b[3] {
            for x in b[0]..b[2] {
                let uv = coordinate(
                    *p,
                    (x as f32 + 0.5) / width as f32,
                    1. - (y as f32 + 0.5) / height as f32,
                );
                if !(0.0..1.0).contains(&uv[0]) || !(0.0..1.0).contains(&uv[1]) {
                    continue;
                }
                let digit_x = uv[0] * digits.len() as f32;
                let digit = digits[digit_x.floor() as usize];
                let src = sample(&glyphs[&digit], digit_x.fract(), uv[1], &lut);
                if src[3] <= 0. {
                    continue;
                }
                let pixel = &mut rgba[(y * width + x) * 4..][..4];
                for c in 0..3 {
                    pixel[c] = encoded(src[c] + lut[pixel[c] as usize] * (1. - src[3]));
                }
            }
        }
    }
    Ok(mip_dds(rgba, width, height, &lut))
}
fn font(root: &Path, number: Number) -> Result<BTreeMap<u8, Glyph>, Box<dyn Error>> {
    let bytes = super::read(&root.join("cars/user_liveries/numbers/font0.pssg"))?;
    let tree = pssg::read_tree(&bytes)?;
    let names: Vec<_> = number
        .digits()
        .iter()
        .copied()
        .map(|b| format!("{}.tga", b as char))
        .collect();
    let ids: Vec<_> = names.iter().map(|s| s.as_bytes()).collect();
    let mut glyphs = BTreeMap::new();
    for t in texture::read_named(&tree, &ids)? {
        if t.format != Format::Bc3 || t.width > 512 || t.height > 512 {
            return Err("unsupported number font texture".into());
        }
        let rgba = super::texture_upload::decode_levels(t.base_blocks, t.width, t.height, 1, true)
            .ok_or("invalid number glyph")?;
        glyphs.insert(
            t.id[0],
            Glyph {
                width: t.width as usize,
                height: t.height as usize,
                rgba,
            },
        );
    }
    Ok(glyphs)
}
/// Stage a single player image atomically; opponent livery banks stay untouched.
pub fn apply(
    root: &Path,
    model: &CarModel,
    car: &mut super::Car,
    number: Number,
) -> Result<(), Box<dyn Error>> {
    let key = format!(
        "{}{}",
        if car.track { "car0/" } else { "" },
        model.texture("main")
    );
    let dds = car
        .textures
        .get(&key)
        .ok_or("player body diffuse absent for number composition")?;
    // A matching model opponent must have its own base, otherwise material fallback
    // could show the player's number. Fail before mutation rather than leak it.
    for livery in &car.liveries {
        if livery.keys().any(|k| k.starts_with("car0/")) && !livery.contains_key(&key) {
            return Err("opponent body fallback would inherit player number".into());
        }
    }
    let text = super::read(&root.join(format!("cars/{}/decals.xml", model.stem())))?;
    let placements = grid_formats::car_decals::read_numbers(std::str::from_utf8(&text)?)?;
    let glyphs = font(root, number)?;
    let result = compose(dds, &placements, &glyphs, number)?;
    println!(
        "car_race_number player=0 value={number} key={key:?} slots={} glyphs={} bytes={} selection=manual-prototype layout=authored-equal-width source_uv=true upload=shared-helper",
        placements.len(),
        glyphs.len(),
        result.len()
    );
    car.textures.insert(key, result);
    Ok(())
}

fn prepared(
    root: &Path,
    model: &CarModel,
    dds: &[u8],
    number: Number,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let text = super::read(&root.join(format!("cars/{}/decals.xml", model.stem())))?;
    let placements = grid_formats::car_decals::read_numbers(std::str::from_utf8(&text)?)?;
    compose(dds, &placements, &font(root, number)?, number)
}
/// Source database selection for each loaded bank. No guessed per-driver numbers.
/// Stage all outputs before replacing any player/opponent CPU image.
pub fn apply_source(
    root: &Path,
    field: &super::Field,
    car: &mut super::Car,
    manual: Option<Number>,
) -> Result<(), Box<dyn Error>> {
    let schema = super::read(&root.join("database/schema.bin"))?;
    let data = super::read(&root.join("database/database.bin"))?;
    let numbers = grid_formats::livery_numbers::read(&schema, &data)?;
    let model = &field.models[0];
    let file = format!("{}_00", model.stem());
    let player = match manual {
        Some(n) => Some(n),
        None => grid_formats::livery_numbers::select(&numbers, &file)?,
    };
    let mut staged = Vec::new();
    if let Some(number) = player {
        let key = format!(
            "{}{}",
            if car.track { "car0/" } else { "" },
            model.texture("main")
        );
        let original = car
            .textures
            .get(&key)
            .ok_or("player number body base absent")?;
        staged.push((
            None,
            key,
            prepared(root, model, original, number)?,
            number,
            file,
        ));
    }
    for (i, bank) in car.liveries.iter().enumerate() {
        let car_index = i + 1;
        let m = *field
            .car_model
            .get(car_index)
            .ok_or("opponent number model mapping absent")?;
        let model = field.models.get(m).ok_or("opponent number model absent")?;
        let index = car_index % super::livery_count(root, model).max(1);
        let file = format!("{}_{index:02}", model.stem());
        let key = format!("{}{}", super::model_prefix(m), model.texture("main"));
        let original = bank
            .get(&key)
            .ok_or("opponent number body base absent; player fallback forbidden")?;
        if let Some(number) = grid_formats::livery_numbers::select(&numbers, &file)? {
            staged.push((
                Some(i),
                key,
                prepared(root, model, original, number)?,
                number,
                file,
            ));
        }
    }
    for (bank, key, dds, number, file) in staged {
        println!(
            "car_race_number car={} value={number} livery={file} key={key:?} bytes={} selection=source-database layout=authored-equal-width source_uv=true upload=shared-helper",
            bank.map_or(0, |i| i + 1),
            dds.len()
        );
        match bank {
            None => {
                car.textures.insert(key, dds);
            }
            Some(i) => {
                car.liveries[i].insert(key, dds);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plain(size: usize) -> Vec<u8> {
        mip_dds(vec![255; size * size * 4], size, size, &table())
    }
    #[test]
    fn source_uv_quarter_turn_and_flip_do_not_reverse_v_twice() {
        let mut p = Placement {
            centre: [0.5, 0.5],
            size: [0.4, 0.4],
            rotation_degrees: 90.,
            flip: false,
        };
        let uv = coordinate(p, 0.6, 0.5);
        assert!((uv[0] - 0.5).abs() < 1e-6 && (uv[1] - 0.75).abs() < 1e-6);
        p.rotation_degrees = 0.;
        p.flip = true;
        assert!((coordinate(p, 0.6, 0.5)[0] - 0.25).abs() < 1e-6);
    }
    #[test]
    fn glyph_alpha_changes_only_its_slot_and_failures_preserve_input() {
        let dds = plain(8);
        let p = Placement {
            centre: [0.5, 0.5],
            size: [0.5, 0.5],
            rotation_degrees: 0.,
            flip: false,
        };
        let mut g = BTreeMap::from([(
            b'7',
            Glyph {
                width: 2,
                height: 2,
                rgba: [0, 0, 0, 255].repeat(4),
            },
        )]);
        let out = compose(&dds, &[p], &g, Number::parse("7").unwrap()).unwrap();
        let (_, _, pixels) = base(&out).unwrap();
        for y in 0..8 {
            for x in 0..8 {
                let pixel = &pixels[(y * 8 + x) * 4..][..4];
                if (2..6).contains(&x) && (2..6).contains(&y) {
                    assert!(pixel[0] < 255);
                } else {
                    assert_eq!(pixel, [255, 255, 255, 255]);
                }
            }
        }
        g.get_mut(&b'7').unwrap().rgba.fill(0);
        assert_eq!(
            compose(&dds, &[p], &g, Number::parse("7").unwrap()).unwrap(),
            dds
        );
        let mut invalid = p;
        invalid.centre = [0., 0.];
        assert!(compose(&dds, &[invalid], &g, Number::parse("7").unwrap()).is_err());
        assert!(compose(&dds, &[p], &g, Number::parse("8").unwrap()).is_err());
        assert!(compose(&dds[..127], &[p], &g, Number::parse("7").unwrap()).is_err());
        assert_eq!(dds, plain(8));
    }
    #[test]
    fn generated_mips_average_in_linear_light_and_keep_alpha_opaque() {
        let rgba = [
            [0, 0, 0, 255],
            [255, 255, 255, 255],
            [0, 0, 0, 255],
            [255, 255, 255, 255],
        ]
        .concat();
        let dds = mip_dds(rgba, 2, 2, &table());
        assert_eq!(&dds[144..148], [188, 188, 188, 255]);
        assert_eq!(u32::from_le_bytes(dds[28..32].try_into().unwrap()), 2);
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn source_banks_are_independent_and_a_late_failure_publishes_nothing() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let key = "car0/lac_main.tga".to_owned();
        let field = super::super::Field {
            models: vec![CarModel::new("lac").unwrap()],
            car_model: vec![0, 0],
        };
        let mut car = super::super::Car {
            parts: Vec::new(),
            textures: BTreeMap::from([
                (key.clone(), plain(128)),
                ("sentinel".to_owned(), vec![1, 2, 3]),
            ]),
            bindings: BTreeMap::new(),
            decal_materials: Default::default(),
            object_glass_materials: Default::default(),
            center: bevy::prelude::Vec3::ZERO,
            radius: 1.,
            track: true,
            liveries: vec![BTreeMap::new()],
            low_lods: Vec::new(),
            ornament_lods: Default::default(),
        };
        let before = car.textures.clone();
        let banks_before = car.liveries.clone();
        assert!(apply_source(&root, &field, &mut car, None).is_err());
        assert_eq!(car.textures, before);
        assert_eq!(car.liveries, banks_before);
        car.liveries[0].insert(key.clone(), before[&key].clone());
        car.liveries[0].insert("sentinel".to_owned(), vec![4, 5, 6]);
        apply_source(&root, &field, &mut car, None).unwrap();
        assert_ne!(car.textures[&key], before[&key]);
        assert_ne!(car.liveries[0][&key], before[&key]);
        assert_ne!(car.textures[&key], car.liveries[0][&key]);
        assert_eq!(car.textures["sentinel"], [1, 2, 3]);
        assert_eq!(car.liveries[0]["sentinel"], [4, 5, 6]);
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_glyphs_compose_all_saved_lacetti_slots() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let bytes =
            super::super::read(&root.join("cars/lac/livery_00/textures_high/lac_tex_high_00.pssg"))
                .unwrap();
        let tree = pssg::read_tree(&bytes).unwrap();
        let textures = texture::read_named(&tree, &[b"lac_main.tga"]).unwrap();
        let original = textures[0].validated_dds().0;
        let (w, h, before) = base(&original).unwrap();
        let text = std::fs::read_to_string(root.join("cars/lac/decals.xml")).unwrap();
        let p = grid_formats::car_decals::read_numbers(&text).unwrap();
        let glyphs = font(&root, Number::parse("01").unwrap()).unwrap();
        let output = compose(&original, &p, &glyphs, Number::parse("01").unwrap()).unwrap();
        let (_, _, after) = base(&output).unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(glyphs.len(), 2);
        assert_eq!((w, h), (2048, 2048));
        let mut changes = [0usize; 3];
        for (i, (a, b)) in before
            .as_chunks::<4>()
            .0
            .iter()
            .zip(after.as_chunks::<4>().0)
            .enumerate()
        {
            if a == b {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let mut found = false;
            for (j, p) in p.iter().enumerate() {
                let b = bounds(*p, w, h).unwrap();
                if (b[0]..b[2]).contains(&x) && (b[1]..b[3]).contains(&y) {
                    changes[j] += 1;
                    found = true;
                }
            }
            assert!(found, "changed texel outside source number slots");
        }
        assert!(changes.iter().all(|n| *n > 100));
        assert_eq!(u32::from_le_bytes(output[28..32].try_into().unwrap()), 12);
    }
}
