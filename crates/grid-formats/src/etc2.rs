//! ETC2 RGB8 and ETC2 RGBA8 (EAC alpha) 4x4 block encoder and decoder.
//! Layouts follow the Khronos Data Format Specification 1.3, ETC2 section.
//! All five RGB modes are encoded and decoded: individual, differential,
//! T, H and planar. Blocks are 64-bit big-endian words. Texels are given
//! row-major (`y * 4 + x`); ETC2 numbers pixels column-major (`x * 4 + y`).

/// ETC1/ETC2 intensity modifier tables (a, b): indices 0..3 are +a, +b, -a, -b.
pub const MODIFIERS: [[i32; 2]; 8] = [
    [2, 8],
    [5, 17],
    [9, 29],
    [13, 42],
    [18, 60],
    [24, 80],
    [33, 106],
    [47, 183],
];

/// T and H mode distances.
pub const DISTANCES: [i32; 8] = [3, 6, 11, 16, 23, 32, 41, 64];

/// EAC alpha modifier tables.
pub const ALPHA_MODIFIERS: [[i32; 8]; 16] = [
    [-3, -6, -9, -15, 2, 5, 8, 14],
    [-3, -7, -10, -13, 2, 6, 9, 12],
    [-2, -5, -8, -13, 1, 4, 7, 12],
    [-2, -4, -6, -13, 1, 3, 5, 12],
    [-3, -6, -8, -12, 2, 5, 7, 11],
    [-3, -7, -9, -11, 2, 6, 8, 10],
    [-4, -7, -8, -11, 3, 6, 7, 10],
    [-3, -5, -8, -11, 2, 4, 7, 10],
    [-2, -6, -8, -10, 1, 5, 7, 9],
    [-2, -5, -8, -10, 1, 4, 7, 9],
    [-2, -4, -8, -10, 1, 3, 7, 9],
    [-2, -5, -7, -10, 1, 4, 6, 9],
    [-3, -4, -7, -10, 2, 3, 6, 9],
    [-1, -2, -3, -10, 0, 1, 2, 9],
    [-4, -6, -8, -9, 3, 5, 7, 8],
    [-3, -5, -7, -9, 2, 4, 6, 8],
];

type Rgb = [i32; 3];
type Texels = [[u8; 4]; 16];

fn modifier(table: usize, index: u32) -> i32 {
    let [a, b] = MODIFIERS[table];
    [a, b, -a, -b][index as usize]
}

fn bits(word: u64, high: u32, count: u32) -> u32 {
    ((word >> (high + 1 - count)) & ((1 << count) - 1)) as u32
}

fn signed3(v: u32) -> i32 {
    if v >= 4 { v as i32 - 8 } else { v as i32 }
}

fn expand(v: u32, count: u32) -> i32 {
    (v << (8 - count) | v >> (2 * count - 8)) as i32
}

fn quantize(v: f32, count: u32) -> u32 {
    let max = (1u32 << count) - 1;
    (v * max as f32 / 255.0).round().clamp(0.0, max as f32) as u32
}

fn error(a: Rgb, b: [u8; 4]) -> u32 {
    let d = [
        a[0] - i32::from(b[0]),
        a[1] - i32::from(b[1]),
        a[2] - i32::from(b[2]),
    ];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as u32
}

fn add(c: Rgb, d: i32) -> Rgb {
    c.map(|v| (v + d).clamp(0, 255))
}

/// Pixel `p` (row-major) as its column-major ETC2 number.
fn etc_number(p: usize) -> usize {
    (p % 4) * 4 + p / 4
}

/// Sets the two index planes for 16 two-bit indices (row-major).
fn pack_indices(indices: &[u32; 16]) -> u64 {
    let mut word = 0;
    for (p, &index) in indices.iter().enumerate() {
        let n = etc_number(p);
        word |= u64::from(index & 1) << n | u64::from(index >> 1) << (16 + n);
    }
    word
}

/// Overflow pattern the decoder sees in the three differential channels.
fn overflow(word: u64) -> [bool; 3] {
    std::array::from_fn(|c| {
        let high = 63 - 8 * c as u32;
        let sum = bits(word, high, 5) as i32 + signed3(bits(word, high - 5, 3));
        !(0..32).contains(&sum)
    })
}

/// Chooses the unused bits `free` so the decoder selects the wanted mode:
/// T is R overflow; H is R in range and G overflow; planar is R and G in
/// range and B overflow.
fn steer(word: u64, free: &[u32], mode: usize) -> u64 {
    for combo in 0..1u64 << free.len() {
        let mut w = word;
        for (i, &bit) in free.iter().enumerate() {
            w |= (combo >> i & 1) << bit;
        }
        let o = overflow(w);
        let selected = o.iter().position(|&v| v);
        if selected == Some(mode) {
            return w;
        }
    }
    unreachable!("free bits always allow mode {mode}")
}

/// Best table and per-pixel indices for one 8-texel sub-block.
fn fit_sub(texels: &Texels, members: &[usize; 8], base: Rgb) -> (u32, u32, [u32; 8]) {
    let mut best = (u32::MAX, 0, [0; 8]);
    for (table, &[a, b]) in MODIFIERS.iter().enumerate() {
        let paints = [0, 1, 2, 3].map(|i| add(base, modifier(table, i)));
        // Without clamping, err(m) = 3 (m - D/3)^2 + const for D = sum of
        // (texel - base) over the channels, so the nearest modifier to D/3
        // is the exact best; clamped paints use the full comparison.
        let clamped = base.iter().any(|&c| c - b < 0 || c + b > 255);
        let mut total = 0;
        let mut indices = [0; 8];
        for (slot, &p) in members.iter().enumerate() {
            let (index, e) = if clamped {
                nearest(&paints, texels[p])
            } else {
                let d: i32 = (0..3).map(|c| i32::from(texels[p][c]) - base[c]).sum();
                let index = if 2 * d >= 3 * (a + b) {
                    1
                } else if d >= 0 {
                    0
                } else if 2 * d >= -3 * (a + b) {
                    2
                } else {
                    3
                };
                (index, error(paints[index as usize], texels[p]))
            };
            indices[slot] = index;
            total += e;
            if total >= best.0 {
                break;
            }
        }
        if total < best.0 {
            best = (total, table as u32, indices);
        }
    }
    best
}

fn nearest(paints: &[Rgb; 4], texel: [u8; 4]) -> (u32, u32) {
    let mut best = (0, u32::MAX);
    for (i, &paint) in paints.iter().enumerate() {
        let e = error(paint, texel);
        if e < best.1 {
            best = (i as u32, e);
        }
    }
    best
}

fn members(flip: bool, sub: usize) -> [usize; 8] {
    std::array::from_fn(|i| {
        let (x, y) = if flip {
            (i % 4, sub * 2 + i / 4)
        } else {
            (sub * 2 + i / 4, i % 4)
        };
        y * 4 + x
    })
}

fn mean(texels: &Texels, members: impl Iterator<Item = usize> + Clone) -> [f32; 3] {
    let count = members.clone().count().max(1) as f32;
    std::array::from_fn(|c| {
        members
            .clone()
            .map(|p| f32::from(texels[p][c]))
            .sum::<f32>()
            / count
    })
}

/// Encodes one opaque 4x4 block (alpha ignored) as the best of the five
/// modes by squared RGB error.
pub fn encode_rgb(texels: &Texels) -> u64 {
    // Cheaper modes first; stop once a block is already close (squared RGB
    // error at most GOOD, about 1.2 levels RMS per channel).
    const GOOD: u32 = 16 * 3 * 3 / 2;
    let mut best = encode_planar(texels);
    if best.0 <= GOOD {
        return best.1;
    }
    let etc1 = encode_etc1(texels);
    if etc1.0 < best.0 {
        best = etc1;
    }
    if best.0 <= GOOD {
        return best.1;
    }
    let t_h = encode_t_h(texels);
    if t_h.0 < best.0 {
        best = t_h;
    }
    best.1
}

fn encode_etc1(texels: &Texels) -> (u32, u64) {
    let mut best = (u32::MAX, 0);
    for flip in [false, true] {
        let m = [members(flip, 0), members(flip, 1)];
        let avg = [
            mean(texels, m[0].iter().copied()),
            mean(texels, m[1].iter().copied()),
        ];
        // Individual: two 4-bit colours.
        let q: [[u32; 3]; 2] = std::array::from_fn(|s| avg[s].map(|v| quantize(v, 4)));
        let fits: [_; 2] =
            std::array::from_fn(|s| fit_sub(texels, &m[s], q[s].map(|v| expand(v, 4))));
        let total = fits[0].0.saturating_add(fits[1].0);
        if total < best.0 {
            let mut word = 0u64;
            for (c, (&first, &second)) in q[0].iter().zip(&q[1]).enumerate() {
                word |= u64::from(first) << (60 - 8 * c) | u64::from(second) << (56 - 8 * c);
            }
            best = (total, pack_etc1(word, false, flip, &m, &fits));
        }
        // Differential: a 5-bit colour and a 3-bit signed delta.
        let q1 = avg[0].map(|v| quantize(v, 5));
        let q2: [u32; 3] = std::array::from_fn(|c| {
            (quantize(avg[1][c], 5) as i32).clamp(q1[c] as i32 - 4, q1[c] as i32 + 3) as u32
        });
        let fits = [
            fit_sub(texels, &m[0], q1.map(|v| expand(v, 5))),
            fit_sub(texels, &m[1], q2.map(|v| expand(v, 5))),
        ];
        let total = fits[0].0.saturating_add(fits[1].0);
        if total < best.0 {
            let mut word = 0u64;
            for c in 0..3 {
                let delta = (q2[c] as i32 - q1[c] as i32) as u64 & 7;
                word |= u64::from(q1[c]) << (59 - 8 * c) | delta << (56 - 8 * c);
            }
            best = (total, pack_etc1(word, true, flip, &m, &fits));
        }
    }
    best
}

fn pack_etc1(
    mut word: u64,
    differential: bool,
    flip: bool,
    members: &[[usize; 8]; 2],
    fits: &[(u32, u32, [u32; 8]); 2],
) -> u64 {
    word |= u64::from(fits[0].1) << 37 | u64::from(fits[1].1) << 34;
    word |= u64::from(differential) << 33 | u64::from(flip) << 32;
    let mut indices = [0u32; 16];
    for s in 0..2 {
        for (slot, &p) in members[s].iter().enumerate() {
            indices[p] = fits[s].2[slot];
        }
    }
    word | pack_indices(&indices)
}

/// Two clusters of the block (2-means from the farthest pair).
fn clusters(texels: &Texels) -> [[f32; 3]; 2] {
    let mut far = (0, 0, 0);
    for a in 0..16 {
        for b in a + 1..16 {
            let e = error(texels[a].map(i32::from)[..3].try_into().unwrap(), texels[b]);
            if e > far.2 {
                far = (a, b, e);
            }
        }
    }
    let mut centres = [far.0, far.1]
        .map(|p| texels[p].map(f32::from))
        .map(|c| [c[0], c[1], c[2]]);
    for _ in 0..4 {
        let side: [bool; 16] = std::array::from_fn(|p| {
            let d = |c: [f32; 3]| {
                (0..3)
                    .map(|i| (c[i] - f32::from(texels[p][i])).powi(2))
                    .sum::<f32>()
            };
            d(centres[1]) < d(centres[0])
        });
        for (k, centre) in centres.iter_mut().enumerate() {
            if side.contains(&(k == 1)) {
                *centre = mean(texels, (0..16).filter(|&p| side[p] == (k == 1)));
            }
        }
    }
    centres
}

/// Best T or H block from the two clusters.
fn encode_t_h(texels: &Texels) -> (u32, u64) {
    let centres = clusters(texels);
    let q = centres.map(|c| c.map(|v| quantize(v, 4)));
    let base = q.map(|c| c.map(|v| expand(v, 4)));
    let mut best = (u32::MAX, 0);
    let score = |paints: &[Rgb; 4]| {
        let mut total = 0;
        let mut indices = [0; 16];
        for (p, &texel) in texels.iter().enumerate() {
            let (index, e) = nearest(paints, texel);
            indices[p] = index;
            total += e;
        }
        (total, indices)
    };
    // T: one colour alone, the other with +d, 0, -d.
    for single in 0..2 {
        let (one, other) = (single, 1 - single);
        for (di, &d) in DISTANCES.iter().enumerate() {
            let paints = [
                base[one],
                add(base[other], d),
                base[other],
                add(base[other], -d),
            ];
            let (total, indices) = score(&paints);
            if total < best.0 {
                let (c1, c2) = (q[one], q[other]);
                let word = u64::from(c1[0] >> 2) << 59
                    | u64::from(c1[0] & 3) << 56
                    | u64::from(c1[1]) << 52
                    | u64::from(c1[2]) << 48
                    | u64::from(c2[0]) << 44
                    | u64::from(c2[1]) << 40
                    | u64::from(c2[2]) << 36
                    | (di as u64 >> 1) << 34
                    | 1 << 33
                    | (di as u64 & 1) << 32
                    | pack_indices(&indices);
                best = (total, steer(word, &[63, 62, 61, 58], 0));
            }
        }
    }
    // H: both colours with +d and -d; the order of the colours encodes the
    // lowest distance bit.
    let packed = |c: [u32; 3]| c[0] << 8 | c[1] << 4 | c[2];
    for (di, &d) in DISTANCES.iter().enumerate() {
        let (first, second) = match (packed(q[0]).cmp(&packed(q[1])), di & 1) {
            (std::cmp::Ordering::Equal, 0) => continue,
            (std::cmp::Ordering::Less, 1) | (std::cmp::Ordering::Greater, 0) => (1, 0),
            _ => (0, 1),
        };
        let paints = [
            add(base[first], d),
            add(base[first], -d),
            add(base[second], d),
            add(base[second], -d),
        ];
        let (total, indices) = score(&paints);
        if total < best.0 {
            let (c1, c2) = (q[first], q[second]);
            let word = u64::from(c1[0]) << 59
                | u64::from(c1[1] >> 1) << 56
                | u64::from(c1[1] & 1) << 52
                | u64::from(c1[2] >> 3) << 51
                | u64::from(c1[2] & 7) << 47
                | u64::from(c2[0]) << 43
                | u64::from(c2[1]) << 39
                | u64::from(c2[2]) << 35
                | (di as u64 >> 2) << 34
                | 1 << 33
                | (di as u64 >> 1 & 1) << 32
                | pack_indices(&indices);
            best = (total, steer(word, &[63, 55, 54, 53, 50], 1));
        }
    }
    best
}

/// Least-squares planar block, scored by its decoded error.
fn encode_planar(texels: &Texels) -> (u32, u64) {
    // c(x, y) = O (1 - x/4 - y/4) + H x/4 + V y/4.
    let mut a = [[0f32; 3]; 3];
    let mut rhs = [[0f32; 3]; 3];
    for (p, texel) in texels.iter().enumerate() {
        let (x, y) = ((p % 4) as f32 / 4.0, (p / 4) as f32 / 4.0);
        let u = [1.0 - x - y, x, y];
        for (row, &ui) in a.iter_mut().zip(&u) {
            for (cell, &uj) in row.iter_mut().zip(&u) {
                *cell += ui * uj;
            }
        }
        for (channel, &value) in rhs.iter_mut().zip(texel) {
            for (cell, &ui) in channel.iter_mut().zip(&u) {
                *cell += ui * f32::from(value);
            }
        }
    }
    let det = |m: [[f32; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    // Cramer's rule; the matrix is symmetric, so columns equal rows.
    let solve = |b: [f32; 3]| -> [f32; 3] {
        std::array::from_fn(|k| {
            let mut m = a;
            for (row, value) in m.iter_mut().zip(b) {
                row[k] = value;
            }
            det(m) / d
        })
    };
    let widths = [6, 7, 6];
    // q[c] = [O, H, V] of channel c.
    let q: [[u32; 3]; 3] = std::array::from_fn(|c| solve(rhs[c]).map(|v| quantize(v, widths[c])));
    let [o, h, v] = [0, 1, 2].map(|k| [q[0][k], q[1][k], q[2][k]]);
    let word = u64::from(o[0]) << 57
        | u64::from(o[1] >> 6) << 56
        | u64::from(o[1] & 63) << 49
        | u64::from(o[2] >> 5) << 48
        | u64::from(o[2] >> 3 & 3) << 43
        | u64::from(o[2] & 7) << 39
        | u64::from(h[0] >> 1) << 34
        | 1 << 33
        | u64::from(h[0] & 1) << 32
        | u64::from(h[1]) << 25
        | u64::from(h[2]) << 19
        | u64::from(v[0]) << 13
        | u64::from(v[1]) << 6
        | u64::from(v[2]);
    let word = steer(word, &[63, 55, 47, 46, 45, 42], 2);
    let decoded = decode_rgb(word);
    let total = (0..16)
        .map(|p| {
            error(
                decoded[p].map(i32::from)[..3].try_into().unwrap(),
                texels[p],
            )
        })
        .sum();
    (total, word)
}

/// Decodes one ETC2 RGB block to row-major RGBA texels (alpha 255).
pub fn decode_rgb(word: u64) -> Texels {
    let differential = word >> 33 & 1 == 1;
    let index = |p: usize| {
        let n = etc_number(p);
        (word >> n & 1 | (word >> (16 + n) & 1) << 1) as usize
    };
    let paint = |paints: [Rgb; 4]| -> Texels {
        std::array::from_fn(|p| {
            let c = paints[index(p)];
            [c[0] as u8, c[1] as u8, c[2] as u8, 255]
        })
    };
    let o = overflow(word);
    if differential && o[0] {
        // T mode.
        let c1 = [
            bits(word, 60, 2) << 2 | bits(word, 57, 2),
            bits(word, 55, 4),
            bits(word, 51, 4),
        ]
        .map(|v| expand(v, 4));
        let c2 = [bits(word, 47, 4), bits(word, 43, 4), bits(word, 39, 4)].map(|v| expand(v, 4));
        let d = DISTANCES[(bits(word, 35, 2) << 1 | bits(word, 32, 1)) as usize];
        return paint([c1, add(c2, d), c2, add(c2, -d)]);
    }
    if differential && o[1] {
        // H mode.
        let q1 = [
            bits(word, 62, 4),
            bits(word, 58, 3) << 1 | bits(word, 52, 1),
            bits(word, 51, 1) << 3 | bits(word, 49, 3),
        ];
        let q2 = [bits(word, 46, 4), bits(word, 42, 4), bits(word, 38, 4)];
        let packed = |c: [u32; 3]| c[0] << 8 | c[1] << 4 | c[2];
        let low = u32::from(packed(q1) >= packed(q2));
        let d = DISTANCES[(bits(word, 34, 1) << 2 | bits(word, 32, 1) << 1 | low) as usize];
        let (c1, c2) = (q1.map(|v| expand(v, 4)), q2.map(|v| expand(v, 4)));
        return paint([add(c1, d), add(c1, -d), add(c2, d), add(c2, -d)]);
    }
    if differential && o[2] {
        // Planar mode.
        let o = [
            expand(bits(word, 62, 6), 6),
            expand(bits(word, 56, 1) << 6 | bits(word, 54, 6), 7),
            expand(
                bits(word, 48, 1) << 5 | bits(word, 44, 2) << 3 | bits(word, 41, 3),
                6,
            ),
        ];
        let h = [
            expand(bits(word, 38, 5) << 1 | bits(word, 32, 1), 6),
            expand(bits(word, 31, 7), 7),
            expand(bits(word, 24, 6), 6),
        ];
        let v = [
            expand(bits(word, 18, 6), 6),
            expand(bits(word, 12, 7), 7),
            expand(bits(word, 5, 6), 6),
        ];
        return std::array::from_fn(|p| {
            let (x, y) = ((p % 4) as i32, (p / 4) as i32);
            let c: [u8; 3] = std::array::from_fn(|c| {
                ((x * (h[c] - o[c]) + y * (v[c] - o[c]) + 4 * o[c] + 2) >> 2).clamp(0, 255) as u8
            });
            [c[0], c[1], c[2], 255]
        });
    }
    let channel = |c: u32| {
        let high = 63 - 8 * c;
        if differential {
            let first = bits(word, high, 5);
            let second = (first as i32 + signed3(bits(word, high - 5, 3))) as u32;
            [expand(first, 5), expand(second, 5)]
        } else {
            [
                expand(bits(word, high, 4), 4),
                expand(bits(word, high - 4, 4), 4),
            ]
        }
    };
    let [r, g, b] = [0, 1, 2].map(channel);
    let bases = [[r[0], g[0], b[0]], [r[1], g[1], b[1]]];
    let tables = [bits(word, 39, 3) as usize, bits(word, 36, 3) as usize];
    let flip = word >> 32 & 1 == 1;
    std::array::from_fn(|p| {
        let (x, y) = (p % 4, p / 4);
        let sub = usize::from(if flip { y >= 2 } else { x >= 2 });
        let c = add(bases[sub], modifier(tables[sub], index(p) as u32));
        [c[0] as u8, c[1] as u8, c[2] as u8, 255]
    })
}

/// Encodes the alpha of one 4x4 block (row-major) as an EAC block.
pub fn encode_alpha(alpha: &[u8; 16]) -> u64 {
    let lo = i32::from(*alpha.iter().min().unwrap());
    let hi = i32::from(*alpha.iter().max().unwrap());
    if lo == hi {
        // Table 13 has a zero modifier at index 4: exact, multiplier 1.
        return pack_alpha(lo as u32, 1, 13, &[4; 16]);
    }
    let mut best = (u32::MAX, 0u64);
    for (table, mods) in ALPHA_MODIFIERS.iter().enumerate() {
        let span = mods[7] - mods[3];
        let ideal = (hi - lo) as f32 / span as f32;
        for mul in [ideal.floor() as i32, ideal.ceil() as i32] {
            let mul = mul.clamp(1, 15);
            let centre =
                ((lo + hi) as f32 / 2.0 - (mods[3] + mods[7]) as f32 * mul as f32 / 2.0).round();
            for base in [centre as i32 - 1, centre as i32, centre as i32 + 1] {
                let base = base.clamp(0, 255);
                let mut total = 0u32;
                let mut indices = [0u32; 16];
                for (p, &a) in alpha.iter().enumerate() {
                    let mut pixel_best = u32::MAX;
                    for (index, &m) in mods.iter().enumerate() {
                        let d = (base + m * mul).clamp(0, 255) - i32::from(a);
                        let e = (d * d) as u32;
                        if e < pixel_best {
                            pixel_best = e;
                            indices[p] = index as u32;
                        }
                    }
                    total += pixel_best;
                }
                if total < best.0 {
                    best = (
                        total,
                        pack_alpha(base as u32, mul as u32, table as u32, &indices),
                    );
                }
            }
        }
    }
    best.1
}

fn pack_alpha(base: u32, mul: u32, table: u32, indices: &[u32; 16]) -> u64 {
    let mut word = u64::from(base) << 56 | u64::from(mul) << 52 | u64::from(table) << 48;
    for (p, &index) in indices.iter().enumerate() {
        word |= u64::from(index) << (45 - 3 * etc_number(p));
    }
    word
}

/// Decodes one EAC alpha block to row-major alpha values.
pub fn decode_alpha(word: u64) -> [u8; 16] {
    let base = bits(word, 63, 8) as i32;
    let mul = bits(word, 55, 4) as i32;
    let mods = ALPHA_MODIFIERS[bits(word, 51, 4) as usize];
    std::array::from_fn(|p| {
        let index = (word >> (45 - 3 * etc_number(p)) & 7) as usize;
        (base + mods[index] * mul).clamp(0, 255) as u8
    })
}

/// Block byte size: 8 for ETC2 RGB8, 16 for ETC2 RGBA8 (EAC alpha first).
pub fn block_bytes(alpha: bool) -> usize {
    if alpha { 16 } else { 8 }
}

/// Encodes block rows `rows` of one tightly packed RGBA8 level into `out`.
/// Blocks past the level edge replicate the edge texels.
pub fn encode_level_rows(
    rgba: &[u8],
    width: usize,
    height: usize,
    alpha: bool,
    rows: std::ops::Range<usize>,
    out: &mut [u8],
) {
    let blocks_x = width.div_ceil(4);
    let size = block_bytes(alpha);
    for by in rows.clone() {
        for bx in 0..blocks_x {
            let texels: Texels = std::array::from_fn(|i| {
                let x = (bx * 4 + i % 4).min(width - 1);
                let y = (by * 4 + i / 4).min(height - 1);
                rgba[(y * width + x) * 4..][..4].try_into().unwrap()
            });
            let at = ((by - rows.start) * blocks_x + bx) * size;
            let block = &mut out[at..at + size];
            if alpha {
                block[..8].copy_from_slice(&encode_alpha(&texels.map(|t| t[3])).to_be_bytes());
                block[8..].copy_from_slice(&encode_rgb(&texels).to_be_bytes());
            } else {
                block.copy_from_slice(&encode_rgb(&texels).to_be_bytes());
            }
        }
    }
}

/// Decodes one tightly packed ETC2 level to RGBA8 (for checks).
pub fn decode_level(blocks: &[u8], width: usize, height: usize, alpha: bool) -> Vec<u8> {
    let blocks_x = width.div_ceil(4);
    let size = block_bytes(alpha);
    let mut out = vec![0; width * height * 4];
    for (b, block) in blocks.chunks_exact(size).enumerate() {
        let (bx, by) = (b % blocks_x, b / blocks_x);
        let word = |i: usize| u64::from_be_bytes(block[i * 8..i * 8 + 8].try_into().unwrap());
        let mut texels = decode_rgb(word(usize::from(alpha)));
        if alpha {
            for (t, a) in texels.iter_mut().zip(decode_alpha(word(0))) {
                t[3] = a;
            }
        }
        for (i, texel) in texels.iter().enumerate() {
            let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
            if x < width && y < height {
                out[(y * width + x) * 4..][..4].copy_from_slice(texel);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> u8 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*seed >> 56) as u8
    }

    fn sse(a: &Texels, b: &Texels) -> u32 {
        (0..16)
            .map(|p| error(a[p].map(i32::from)[..3].try_into().unwrap(), b[p]))
            .sum()
    }

    fn mode(word: u64) -> &'static str {
        match (word >> 33 & 1, overflow(word)) {
            (0, _) => "individual",
            (_, [true, _, _]) => "T",
            (_, [_, true, _]) => "H",
            (_, [_, _, true]) => "planar",
            _ => "differential",
        }
    }

    #[test]
    fn individual_block_decodes_by_hand() {
        // R1=0xF, R2=0, tables 0 and 7, no flip; left pixels index 0 (+2),
        // right pixels index 3 (-183).
        let mut word = 0xF0u64 << 56 | 7 << 34;
        for x in 2..4 {
            for y in 0..4 {
                let n = x * 4 + y;
                word |= 1 << n | 1 << (16 + n);
            }
        }
        let t = decode_rgb(word);
        assert_eq!(t[0], [255, 2, 2, 255]);
        assert_eq!(t[2], [0, 0, 0, 255]);
    }

    #[test]
    fn solid_block_is_close() {
        let solid = [[120, 64, 200, 255]; 16];
        let d = decode_rgb(encode_rgb(&solid));
        assert!(sse(&d, &solid) <= 16 * 3 * 9, "{d:?}");
    }

    #[test]
    fn chroma_edges_use_t_or_h_modes() {
        // Two hues in each half of the block: ETC1 cannot hold them.
        let edge: Texels = std::array::from_fn(|p| {
            if p % 2 == 0 {
                [200, 30, 30, 255]
            } else {
                [20, 40, 220, 255]
            }
        });
        let word = encode_rgb(&edge);
        assert!(matches!(mode(word), "T" | "H"), "{}", mode(word));
        assert!(sse(&decode_rgb(word), &edge) <= 16 * 3 * 100);
        // Three colours: one alone plus a pair 2d apart favours T.
        let three: Texels = std::array::from_fn(|p| match p % 3 {
            0 => [250, 250, 250, 255],
            1 => [16, 120, 16, 255],
            _ => [80, 184, 80, 255],
        });
        let word = encode_rgb(&three);
        assert!(
            sse(&decode_rgb(word), &three) <= 16 * 3 * 100,
            "{}",
            mode(word)
        );
    }

    #[test]
    fn gradient_uses_planar_and_stays_close() {
        let ramp: Texels = std::array::from_fn(|p| {
            let (x, y) = ((p % 4) as u8, (p / 4) as u8);
            [40 + x * 40, 10 + y * 50, 200 - x * 20 - y * 20, 255]
        });
        let word = encode_rgb(&ramp);
        assert_eq!(mode(word), "planar");
        assert!(sse(&decode_rgb(word), &ramp) <= 16 * 3 * 4);
    }

    #[test]
    fn steering_reaches_every_mode_for_random_payloads() {
        let mut seed = 3;
        for _ in 0..500 {
            let w = (0..8).fold(0u64, |w, _| w << 8 | u64::from(lcg(&mut seed)));
            let cleared = w & !(1 << 63 | 0b111 << 61 | 1 << 58);
            assert_eq!(mode(steer(cleared | 1 << 33, &[63, 62, 61, 58], 0)), "T");
            let cleared = w & !(1 << 63 | 0b111 << 53 | 1 << 50);
            assert_eq!(
                mode(steer(cleared | 1 << 33, &[63, 55, 54, 53, 50], 1)),
                "H"
            );
            let cleared = w & !(1 << 63 | 1 << 55 | 0b111 << 45 | 1 << 42);
            assert_eq!(
                mode(steer(cleared | 1 << 33, &[63, 55, 47, 46, 45, 42], 2)),
                "planar"
            );
        }
    }

    #[test]
    fn random_blocks_beat_a_flat_fill() {
        let mut seed = 7;
        for _ in 0..2000 {
            let texels: Texels =
                std::array::from_fn(|_| [lcg(&mut seed), lcg(&mut seed), lcg(&mut seed), 255]);
            let d = decode_rgb(encode_rgb(&texels));
            let avg = mean(&texels, 0..16).map(|v| v.round() as i32);
            let flat: u32 = texels.iter().map(|&t| error(avg, t)).sum();
            assert!(sse(&d, &texels) <= flat);
        }
    }

    #[test]
    fn alpha_blocks_round_trip() {
        let constant = [77u8; 16];
        assert_eq!(decode_alpha(encode_alpha(&constant)), constant);
        let mask: [u8; 16] = std::array::from_fn(|p| if p % 3 == 0 { 0 } else { 255 });
        assert_eq!(decode_alpha(encode_alpha(&mask)), mask);
        // 16 distinct values in 8 levels: within one 17-step.
        let ramp: [u8; 16] = std::array::from_fn(|p| (p * 17) as u8);
        let d = decode_alpha(encode_alpha(&ramp));
        for (a, b) in d.iter().zip(ramp) {
            assert!((i32::from(*a) - i32::from(b)).abs() <= 17, "{d:?}");
        }
        // Column-major pixel order: only texel (x=1, y=0) is opaque.
        let mut one = [0u8; 16];
        one[1] = 255;
        assert_eq!(decode_alpha(encode_alpha(&one)), one);
    }

    #[test]
    fn small_levels_pad_and_round_trip() {
        let rgba = [10u8, 20, 30, 255, 200, 100, 50, 128];
        let mut out = [0u8; 16];
        encode_level_rows(&rgba, 2, 1, true, 0..1, &mut out);
        let d = decode_level(&out, 2, 1, true);
        for (a, b) in d.iter().zip(rgba) {
            assert!((i32::from(*a) - i32::from(b)).abs() <= 12, "{d:?}");
        }
    }
}
