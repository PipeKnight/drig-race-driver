//! Complete decoded Standard property layout, compared against the original decoder.
//! CSV framing, live ownership and car construction are separate operations.
use crate::{Error, Result, fail, tuning::Row, vehicle_properties_fields::DIRECT};

#[derive(Clone)]
pub struct Properties {
    bytes: [u8; 0x65c],
}
impl Properties {
    pub fn bytes(&self) -> &[u8; 0x65c] {
        &self.bytes
    }
    fn word(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn set(&mut self, offset: usize, value: f32) -> Result<()> {
        if !value.is_finite() {
            return fail(offset, "nonfinite Standard property");
        }
        self.word(offset, value.to_bits());
        Ok(())
    }
    fn get(&self, offset: usize) -> f32 {
        f32::from_bits(u32::from_le_bytes(
            self.bytes[offset..offset + 4].try_into().unwrap(),
        ))
    }
    fn string(&mut self, offset: usize, text: &str) {
        self.bytes[offset..offset + text.len()].copy_from_slice(text.as_bytes());
    }
}
fn number(row: &Row<'_>, column: usize) -> Result<f64> {
    // Column49 is also the source revision string (e.g. an initial numeric
    // model code followed by text). Its legacy numeric use consumes a prefix.
    // Other numeric fields keep strict parsing; this is not a general CRT parser.
    if column == 49 {
        let text = row.fields[column].trim_start_matches(|c: char| c.is_ascii_whitespace());
        for length in (1..=text.len()).rev() {
            if let Ok(value) = text[..length].parse::<f64>() {
                if !value.is_finite() {
                    return fail(column, "nonfinite Standard revision prefix");
                }
                return Ok(value);
            }
        }
        return Ok(0.);
    }
    Ok(row.number(column)?.unwrap_or(0.))
}
fn integer(row: &Row<'_>, column: usize) -> Result<i32> {
    let text = row.fields[column];
    if text.is_empty() {
        return Ok(0);
    }
    text.parse::<i32>().map_err(|_| Error {
        offset: column,
        message: "invalid Standard integer",
    })
}

/// Decode an owned Standard source row. Metadata is caller-supplied source identity.
/// Strict finite/integer/string bounds exclude malformed original-parser behavior.
pub fn read(row: &Row<'_>, metadata: u32) -> Result<Properties> {
    crate::vehicle::read_standard_body(row)?;
    if row.fields[..360]
        .iter()
        .any(|s| !s.is_ascii() || s.len() > 63 || s.as_bytes().contains(&0))
    {
        return fail(0, "invalid Standard source cell");
    }
    let mut out = Properties { bytes: [0; 0x65c] };
    let n = |column| number(row, column);
    for &(offset, column) in DIRECT {
        out.set(offset, n(column)? as f32)?;
    }
    out.string(0, row.fields[1]);
    out.string(0x298, row.fields[57]);
    out.string(0x5dc, row.fields[49]);
    out.word(0x140, 1);
    let orientation = [
        "Unknown",
        "Transverse",
        "Inline",
        "Rear Transverse",
        "Rear Inline",
    ]
    .iter()
    .position(|s| row.fields[63].eq_ignore_ascii_case(s))
    .unwrap_or(0);
    out.word(0x2e4, orientation as u32);
    let drive = if row.fields[67].eq_ignore_ascii_case("Front") {
        1
    } else if row.fields[67].eq_ignore_ascii_case("Rear") {
        3
    } else if ["Four", "All"]
        .iter()
        .any(|s| row.fields[67].eq_ignore_ascii_case(s))
    {
        2
    } else {
        0
    };
    out.word(0x304, drive);
    out.bytes[0x308] = u8::from(drive != 0);
    for (offset, column, count) in [(0x1f4, 17, 32), (0x274, 50, 7), (0x31c, 69, 8)] {
        for i in 0..count {
            out.set(offset + 4 * i, n(column + i)? as f32)?;
        }
    }
    out.set(0x33c, n(68)? as f32)?;
    out.bytes[0x1d4] = u8::from(out.get(0x1dc) != 0.);
    out.bytes[0x1d5] = u8::from(out.get(0x1ec) != 0.);
    if n(139)? > 0.5 {
        out.bytes[0x1d5] |= 1;
        for i in 0..7 {
            out.set(0x274 + 4 * i, 1.)?;
        }
    }
    out.bytes[0x360] = u8::from(row.fields[78].eq_ignore_ascii_case("Simple"));
    let differential = if row.fields[78].eq_ignore_ascii_case("Full") {
        [1., 1., 1.]
    } else if row.fields[78].eq_ignore_ascii_case("Half") {
        [1., 0.5, 0.5]
    } else {
        [0.; 3]
    };
    for (base, values) in [(0x348, differential), (0x354, differential)] {
        for (i, v) in values.into_iter().enumerate() {
            out.set(base + 4 * i, v)?;
        }
    }
    if row.fields[78].eq_ignore_ascii_case("Custom") {
        let scale = f32::from_bits(0x3c23_d70a);
        for (column, base) in [(262, 0x348), (263, 0x354)] {
            let value = integer(row, column)?;
            for (offset, digit) in [
                (base, (value / 101 / 101) % 101),
                (base + 4, value % 101),
                (base + 8, (value / 101) % 101),
            ] {
                out.set(offset, digit as f32 * scale)?;
            }
            if column == 262 {
                out.bytes[0x360] = u8::from(value / 101 / 101 / 101 != 0);
            }
        }
    }
    let exponent = f64::from_bits(0x4017_a0a0_2000_0000);
    for offset in [0x34c, 0x350] {
        out.set(
            offset,
            (f64::from(out.get(offset)).powf(exponent) as f32) * 60.,
        )?;
    }
    if drive == 1 {
        for offset in [0x348, 0x34c, 0x358] {
            out.word(offset, 0);
        }
    }
    if drive == 3 {
        for offset in [0x348, 0x350, 0x35c] {
            out.word(offset, 0);
        }
    }
    for (o, v) in [(0x364, 1.), (0x368, 0.1), (0x36c, 5.)] {
        out.set(o, v)?;
    }
    let degrees = f64::from(f32::from_bits(0x3c8e_fa35));
    for (offset, column) in [
        (0x340, 77),
        (0x180, 255),
        (0x184, 256),
        (0x188, 257),
        (0x190, 259),
        (0x194, 260),
        (0x198, 261),
        (0x1c4, 306),
        (0x1c8, 307),
        (0x1ac, 305),
        (0x1b0, 310),
        (0x1b4, 132),
        (0x3c4, 201),
        (0x3ac, 195),
        (0x3b0, 196),
    ] {
        out.set(offset, (n(column)? * degrees) as f32)?;
    }
    let percent = f64::from(f32::from_bits(0x3c23_d70a));
    for (offset, column) in [
        (0x58c, 151),
        (0x5a8, 158),
        (0x5ac, 159),
        (0x1a0, 131),
        (0x1a4, 303),
    ] {
        out.set(offset, (n(column)? * percent) as f32)?;
    }
    out.set(
        0x5b0,
        (n(160)? * f64::from(f32::from_bits(0x3ba3_d70a))) as f32,
    )?;
    out.set(0x640, (n(346)? + 1.) as f32)?;
    out.set(0x644, (n(347)? + 1.) as f32)?;
    for (o, c) in [(0x17c, 254), (0x18c, 258)] {
        out.set(o, (n(c)? - 1.) as f32)?;
    }
    out.set(
        0x4e0,
        if n(271)? - 1. < 0. {
            1.
        } else {
            (n(271)? - 1.) as f32
        },
    )?;
    out.set(0x19c, (1. - n(269)?) as f32)?;
    out.set(0x504, (n(58)? * degrees).sin() as f32)?;
    for (o, c) in [(0x1b8, 133), (0x1bc, 134)] {
        out.set(o, (n(c)? * degrees).tan() as f32)?;
    }
    for (offset, column) in [
        (0x4d1, 311),
        (0x4d2, 107),
        (0x4d3, 108),
        (0x4d4, 109),
        (0x4d8, 117),
        (0x4d9, 112),
        (0x4e5, 187),
        (0x4d5, 190),
        (0x4d6, 127),
        (0x5bc, 135),
        (0x5bd, 142),
        (0x4f0, 148),
        (0x4d7, 153),
        (0x4e4, 164),
        (0x4f1, 172),
        (0x4f2, 176),
        (0x4d0, 334),
        (0x290, 335),
    ] {
        out.bytes[offset] = u8::from(n(column)? > 0.5);
    }
    out.set(0x170, f32::from(u8::from(n(332)? > 0.5)))?;
    for (offset, column) in [(0x344, 276), (0x4ec, 147), (0x4f4, 149)] {
        out.word(offset, integer(row, column)? as u32);
    }
    let bits = n(60)? as i64;
    for i in 0..10 {
        out.bytes[0x468 + i] = ((bits >> i) & 1) as u8;
    }
    for i in 0..10 {
        let o = 0x384 + i * 4;
        out.set(o, n(281 + i)? as f32)?;
        if out.get(o) == 0. {
            out.set(o, 1.)?;
        }
    }
    if out.get(0x17c) < 0. {
        for base in [0x17c, 0x18c] {
            for (i, v) in [0., 0.1, 0., f32::from_bits(0x3fc9_0fdb)]
                .into_iter()
                .enumerate()
            {
                out.set(base + 4 * i, v)?;
            }
        }
    }
    for (o, c, default) in [
        (0x430, 270, 1.),
        (0x1f0, 272, 1.),
        (0x510, 273, 1.),
        (0x500, 277, 6.),
        (0x588, 150, 3600.),
        (0x5d0, 173, 1.),
    ] {
        if n(c)? == 0. {
            out.set(o, default)?;
        }
    }
    if out.get(0x43c) == 0. {
        out.set(0x43c, 1.)?;
    }
    if out.get(0x438) == 0. {
        out.set(0x438, out.get(0x43c))?;
    }
    out.set(
        0x528,
        if n(120)? < -2.5 {
            -2.5
        } else {
            (n(120)? as f32).min(12.5)
        },
    )?;
    if n(208)? == 0. {
        out.set(0x584, out.get(0x580) + 10.)?;
        for (o, s) in [(0x570, 0x564), (0x574, 0x568), (0x578, 0x56c)] {
            out.set(o, out.get(s))?;
        }
    }
    out.set(
        0x2e0,
        (if n(175)? == 0. { 1. } else { n(175)? as f32 }) * f32::from_bits(0x39da_740e),
    )?;
    let limit = n(338)? - 10.;
    out.set(
        0x524,
        if limit < -1. {
            out.get(0x154) - f32::from_bits(0x3e38_51ec) / out.get(0x158)
        } else {
            limit as f32
        },
    )?;
    for (o, bits) in [
        (0x2e8, 0x4270_0000),
        (0x2ec, 0x3f19_999a),
        (0x2f0, 0x3e99_999a),
        (0x2f4, 0x3ecc_cccd),
        (0x2f8, 0x3e4c_cccd),
        (0x300, 0x3f80_0000),
        (0x558, 0x447a_0000),
        (0x554, 0x3f80_0000),
        (0x5d4, 0x3f80_0000),
        (0x5d8, 0x3f80_0000),
        (0x54c, 0x3f7c_28f6),
        (0x550, 0x3f5d_b22d),
        (0x53c, 0x3f00_0000),
        (0x540, 0x3f00_0000),
        (0x544, 0x47c3_4f80),
        (0x4e8, 0x3f80_0000),
        (0x42c, 0x3f80_0000),
        (0x538, 0x3f80_0000),
        (0x654, 0x3f80_0000),
    ] {
        if out.get(o) == 0. {
            out.word(o, bits);
        }
    }
    for (a, b) in [(0x4ac, 0x4b0), (0x4b8, 0x4bc), (0x4c4, 0x4c8)] {
        if out.get(a) == out.get(b) {
            out.set(b, out.get(b) + 1.)?;
        }
    }
    if out.get(0x55c) == 0. {
        out.set(0x55c, out.get(0x558))?;
    }
    if out.get(0x5c4) == 0. && out.get(0x5c0) == 0. {
        out.set(0x5c8, 1.)?;
        out.set(0x5cc, 3.)?;
    }
    if out.get(0x568) == 0. {
        out.set(0x568, out.get(0x560))?;
        out.set(0x57c, 10.)?;
        out.set(0x580, 20.)?;
    }
    for o in [0x564, 0x56c] {
        if out.get(o) == 0. {
            out.set(o, out.get(0x560))?;
        }
    }
    out.set(
        0x424,
        if n(91)? < -5. {
            -5.
        } else {
            (n(91)? as f32).min(5.)
        },
    )?;
    out.set(
        0x428,
        if n(92)? < -2. {
            -2.
        } else {
            (n(92)? as f32).min(2.)
        },
    )?;
    for o in [0x598, 0x59c] {
        out.set(o, out.get(o) * f32::from_bits(0x3ee4_e26d))?;
    }
    if out.get(0x5a0) < 0. {
        out.set(0x5a0, -0. - out.get(0x5a0))?;
        out.bytes[0x5a4] = 1;
    }
    out.set(
        0x520,
        if n(128)? == 0. {
            out.get(0x51c)
        } else {
            n(128)? as f32
        },
    )?;
    setup(row, &mut out)?;
    out.word(0x658, metadata);
    Ok(out)
}

fn setup(row: &Row<'_>, out: &mut Properties) -> Result<()> {
    out.bytes[0x40..0x130].fill(255);
    let set = |out: &mut Properties, i: usize, v: f32| out.set(0x40 + 4 * i, v);
    let explicit = (216..254)
        .chain(264..268)
        .all(|i| !row.fields[i].is_empty());
    if explicit {
        for (i, c) in [
            (0, 217),
            (1, 219),
            (2, 221),
            (3, 223),
            (4, 229),
            (5, 231),
            (6, 233),
            (7, 237),
            (8, 235),
            (9, 239),
            (10, 241),
            (11, 243),
            (12, 225),
            (27, 245),
            (28, 247),
            (29, 249),
        ] {
            set(out, i, number(row, c)? as f32)?;
            set(out, i + 30, number(row, c - 1)? as f32)?;
        }
        for i in 13..=20 {
            set(out, i, number(row, 253)? as f32)?;
            set(out, i + 30, number(row, 252)? as f32)?;
        }
        for (i, c) in [(22, 251), (23, 265), (24, 267), (25, 265), (26, 267)] {
            set(out, i, number(row, c)? as f32)?;
            set(out, i + 30, number(row, c - 1)? as f32)?;
        }
    } else {
        for (i, lo, hi) in [
            (0, 0xbdd6_7750, 0x3d7a_35dd),
            (1, 0xbdd6_7750, 0x3d7a_35dd),
            (2, 0xbdb2_b8c3, 0x3db2_b8c3),
            (3, 0xbdb2_b8c3, 0x3db2_b8c3),
        ] {
            out.word(0x40 + 4 * i, hi);
            out.word(0x40 + 4 * (i + 30), lo);
        }
        for i in 22..=26 {
            set(out, i, 1.)?;
            set(out, i + 30, 0.)?;
        }
        let hi = f32::from_bits(0x3f99_999a);
        let lo = f32::from_bits(0x3f4c_cccd);
        set(out, 12, out.get(0x3e0) + 0.5)?;
        set(out, 42, out.get(0x3e0) - 0.5)?;
        let mut gears = 1;
        while gears < 8 && out.get(0x31c + 4 * gears) != 0. {
            gears += 1;
        }
        for i in 13..=20 {
            set(out, i, out.get(0x318 + 4 * gears) * hi)?;
            set(out, i + 30, out.get(0x31c) * lo)?;
        }
        for (i, value) in [
            (4, out.get(0x40c)),
            (5, out.get(0x410)),
            (6, out.get(0x440)),
            (7, out.get(0x438) * out.get(0x440)),
            (8, out.get(0x444)),
            (9, out.get(0x444) * out.get(0x43c)),
            (27, out.get(0x158)),
            (28, out.get(0x414)),
            (29, out.get(0x418)),
        ] {
            set(out, i, value * hi)?;
            set(out, i + 30, value * lo)?;
        }
        for (i, a, b) in [
            (
                10,
                out.get(0x450) * out.get(0x440),
                out.get(0x454) * out.get(0x444),
            ),
            (11, out.get(0x448), out.get(0x44c)),
        ] {
            set(out, i, a.max(b) * hi)?;
            set(out, i + 30, a.min(b) * lo)?;
        }
    }
    set(out, 21, 1.)?;
    set(out, 51, 0.)?;
    Ok(())
}
