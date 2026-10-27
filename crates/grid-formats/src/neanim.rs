//! NeAnim animation sets in PSSG (`interiors/<stem>/int_<stem>_anims.pssg`,
//! `int_<stem>_driver_anims.pssg`, `characters/anims/driver_anims.pssg`;
//! docs/COCKPIT.md "Animation files"). Libraries `NeAnimPacket` (raw
//! `NeAnimPacketData_<type>` payloads), `NeAnimClip` (packet references) and
//! `NeAnimSet` (clip references, in the order the game indexes them).
//! Big-endian; strings are a u32 length and the bytes.
use crate::pssg::Tree;
use crate::{Cursor, Result, fail};
use std::collections::HashMap;

/// One animated value of one node.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// Node name (`steering_wheel`, `Bip01 L Hand`, ...).
    pub target: String,
    /// `rotation_quaternion` or `translation`.
    pub semantic: String,
    pub keys: Keys,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Keys {
    /// `L4_KA`: four components linearly interpolated between shared key
    /// times (a quaternion x, y, z, w or a translation x, y, z, 0).
    Linear { times: Vec<f32>, values: Vec<[f32; 4]> },
    /// `B1`: one component (`node[x|y|z]`) of a 2D Bezier curve; each key is
    /// time, value, in value, in time, out value, out time.
    Bezier { component: usize, keys: Vec<[f32; 6]> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub name: String,
    pub channels: Vec<Channel>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimSet {
    pub name: String,
    pub clips: Vec<Clip>,
}

impl Clip {
    /// The latest key time of any channel (the game's timer duration,
    /// end − start with start 0).
    pub fn duration(&self) -> f32 {
        self.channels
            .iter()
            .map(|c| match &c.keys {
                Keys::Linear { times, .. } => times.last().copied().unwrap_or(0.),
                Keys::Bezier { keys, .. } => keys.last().map_or(0., |k| k[0]),
            })
            .fold(0., f32::max)
    }
}

impl Keys {
    /// The value at `t`, held at the first and last keys. A quaternion is
    /// normalised by the caller.
    pub fn sample(&self, t: f32) -> [f32; 4] {
        match self {
            Keys::Linear { times, values } => {
                let i = times.partition_point(|&k| k <= t);
                if i == 0 {
                    return values[0];
                }
                if i == times.len() {
                    return values[i - 1];
                }
                let (a, b) = (times[i - 1], times[i]);
                let f = if b > a { (t - a) / (b - a) } else { 0. };
                let (p, q) = (values[i - 1], values[i]);
                std::array::from_fn(|j| p[j] + (q[j] - p[j]) * f)
            }
            Keys::Bezier { component, keys } => {
                let mut out = [0.; 4];
                out[*component] = bezier(keys, t);
                out
            }
        }
    }
}

/// The Bezier through key k, its out handle, key k+1's in handle and key
/// k+1. The handles sit a third of the way in time, so the curve parameter
/// is the time fraction [I: the evaluator was not decompiled].
fn bezier(keys: &[[f32; 6]], t: f32) -> f32 {
    let i = keys.partition_point(|k| k[0] <= t);
    if i == 0 {
        return keys[0][1];
    }
    if i == keys.len() {
        return keys[i - 1][1];
    }
    let (a, b) = (keys[i - 1], keys[i]);
    let u = if b[0] > a[0] { (t - a[0]) / (b[0] - a[0]) } else { 0. };
    let v = 1. - u;
    v * v * v * a[1] + 3. * v * v * u * a[4] + 3. * v * u * u * b[2] + u * u * u * b[1]
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn f32_at(c: &mut Cursor) -> Result<f32> {
    Ok(f32::from_bits(c.be()?))
}

/// The channels of one `NeAnimPacketData_<kind>` payload.
fn read_packet(kind: &[u8], bytes: &[u8]) -> Result<Vec<Channel>> {
    let mut c = Cursor { bytes, pos: 0 };
    let mut out = Vec::new();
    match kind {
        b"L4_KA" => {
            let count = c.be()? as usize;
            let keys = c.be()? as usize;
            c.take(12)?;
            if count > 4096 || keys > 65536 || count * keys * 16 > bytes.len() {
                return fail(0, "NeAnim L4_KA counts exceed the payload");
            }
            let mut names = Vec::with_capacity(count);
            for _ in 0..count {
                names.push((text(c.name()?), text(c.name()?)));
            }
            let times = (0..keys).map(|_| f32_at(&mut c)).collect::<Result<Vec<_>>>()?;
            let mut values = vec![Vec::with_capacity(keys); count];
            for _ in 0..keys {
                for v in values.iter_mut() {
                    v.push([f32_at(&mut c)?, f32_at(&mut c)?, f32_at(&mut c)?, f32_at(&mut c)?]);
                }
            }
            for ((target, semantic), values) in names.into_iter().zip(values) {
                out.push(Channel { target, semantic, keys: Keys::Linear { times: times.clone(), values } });
            }
        }
        b"B1" => {
            let count = c.be()? as usize;
            c.take(4)?;
            if count > 4096 {
                return fail(0, "NeAnim B1 channel count exceeds bound");
            }
            for _ in 0..count {
                let name = text(c.name()?);
                let semantic = text(c.name()?);
                let n = c.be()? as usize;
                c.take(8)?;
                if n == 0 || n * 24 > bytes.len() {
                    return fail(c.pos, "NeAnim B1 key count exceeds the payload");
                }
                let mut keys = Vec::with_capacity(n);
                for _ in 0..n {
                    keys.push(std::array::from_fn::<_, 6, _>(|_| 0.));
                    let k = keys.last_mut().unwrap();
                    for v in k.iter_mut() {
                        *v = f32_at(&mut c)?;
                    }
                }
                let (target, component) = match name.strip_suffix(']').and_then(|n| n.rsplit_once('[')) {
                    Some((node, "x")) => (node.to_string(), 0),
                    Some((node, "y")) => (node.to_string(), 1),
                    Some((node, "z")) => (node.to_string(), 2),
                    _ => return fail(c.pos, "NeAnim B1 target lacks a component"),
                };
                out.push(Channel { target, semantic, keys: Keys::Bezier { component, keys } });
            }
        }
        _ => return fail(0, "unsupported NeAnim packet type"),
    }
    if c.pos != bytes.len() {
        return fail(c.pos, "NeAnim packet has trailing bytes");
    }
    if out.iter().any(|ch| matches!(&ch.keys, Keys::Linear { times, .. } if times.is_empty())) {
        return fail(0, "NeAnim channel without keys");
    }
    Ok(out)
}

/// Every `NeAnimSet` of the file, its clips resolved through their packets.
pub fn read_sets(tree: &Tree) -> Result<Vec<AnimSet>> {
    let reference = |node: usize, name: &[u8]| -> Result<String> {
        let value = tree.string(node, name)?;
        Ok(text(value.strip_prefix(b"#").unwrap_or(value)))
    };
    let mut packets: HashMap<String, Vec<Channel>> = HashMap::new();
    let mut clips: HashMap<String, Vec<String>> = HashMap::new();
    let mut sets = Vec::new();
    for (index, node) in tree.nodes.iter().enumerate() {
        let name = tree.name(index);
        if name.starts_with(b"NeAnimPacketData_") {
            let Some(parent) = node.parent else { continue };
            let id = text(tree.string(parent, b"id")?);
            let kind = &name[b"NeAnimPacketData_".len()..];
            packets.insert(id, read_packet(kind, node.data.unwrap_or_default())?);
        } else if name == b"NeAnimClip" {
            let refs = node.children.iter().map(|&r| reference(r, b"packet")).collect::<Result<_>>()?;
            clips.insert(text(tree.string(index, b"id")?), refs);
        } else if name == b"NeAnimSet" {
            let refs: Vec<String> = node.children.iter().map(|&r| reference(r, b"clip")).collect::<Result<_>>()?;
            sets.push((text(tree.string(index, b"id")?), refs, node.offset));
        }
    }
    sets.into_iter()
        .map(|(name, refs, offset)| {
            let clips = refs
                .into_iter()
                .map(|clip| {
                    let Some(packet_ids) = clips.get(&clip) else {
                        return fail(offset, "NeAnimSet names a missing clip");
                    };
                    let mut channels = Vec::new();
                    for id in packet_ids {
                        let Some(p) = packets.get(id) else {
                            return fail(offset, "NeAnimClip names a missing packet");
                        };
                        channels.extend(p.iter().cloned());
                    }
                    Ok(Clip { name: clip, channels })
                })
                .collect::<Result<_>>()?;
            Ok(AnimSet { name, clips })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_keys_hold_at_the_ends_and_interpolate_between() {
        let k = Keys::Linear { times: vec![0., 1.], values: vec![[0., 0., 0., 1.], [1., 2., 3., 1.]] };
        assert_eq!(k.sample(-1.), [0., 0., 0., 1.]);
        assert_eq!(k.sample(0.5), [0.5, 1., 1.5, 1.]);
        assert_eq!(k.sample(2.), [1., 2., 3., 1.]);
    }

    #[test]
    fn bezier_with_flat_handles_eases_between_keys() {
        let keys = vec![[0., 0., 0., 0., 0., 1. / 3.], [1., 1., 1., 2. / 3., 1., 1.]];
        let k = Keys::Bezier { component: 1, keys };
        assert_eq!(k.sample(0.5)[1], 0.5);
        assert!(k.sample(0.25)[1] < 0.25);
        assert_eq!(k.sample(5.), [0., 1., 0., 0.]);
    }

    /// Every shipped animation file decodes: the interiors' `_anims` and
    /// `_driver_anims` sets and the two character sets.
    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn shipped_animation_files_decode() {
        let root = std::path::PathBuf::from(std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        let mut files = vec![
            root.join("characters/anims/driver_anims.pssg"),
            root.join("characters/anims/small_cockpit_anims.pssg"),
        ];
        for dir in std::fs::read_dir(root.join("interiors")).unwrap() {
            let dir = dir.unwrap().path();
            let stem = dir.file_name().unwrap().to_string_lossy().into_owned();
            for suffix in ["anims", "driver_anims"] {
                let file = dir.join(format!("int_{stem}_{suffix}.pssg"));
                if file.is_file() {
                    files.push(file);
                }
            }
        }
        assert!(files.len() >= 88, "{}", files.len());
        for file in files {
            let bytes = std::fs::read(&file).unwrap();
            let tree = crate::pssg::read_tree(&bytes).unwrap();
            let sets = read_sets(&tree).unwrap_or_else(|e| panic!("{}: {e:?}", file.display()));
            assert!(sets.iter().all(|s| !s.clips.is_empty()), "{}", file.display());
        }
    }

    /// Lacetti's interior "Steering" clip: 4 s, 121 keys, and the wheel
    /// turns about its local Z from 0 (full left) through 88.5° at the
    /// centre to 180° (full right) (docs/COCKPIT.md "Steering wheel").
    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn shipped_lacetti_steering_clip() {
        let root = std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT");
        let bytes = std::fs::read(std::path::Path::new(&root).join("interiors/lac/int_lac_anims.pssg")).unwrap();
        let tree = crate::pssg::read_tree(&bytes).unwrap();
        let sets = read_sets(&tree).unwrap();
        let names: Vec<&str> = sets[0].clips.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            ["Steering", "accel", "Brake", "Clutch", "change_up", "change_down", "HandBrake_on", "HandBrake_off"]
        );
        let steering = &sets[0].clips[0];
        assert!((steering.duration() - 4.).abs() < 1e-5);
        let wheel = steering
            .channels
            .iter()
            .find(|c| c.target == "steering_wheel" && c.semantic == "rotation_quaternion")
            .unwrap();
        let Keys::Linear { times, .. } = &wheel.keys else { panic!() };
        assert_eq!(times.len(), 121);
        let angle = |t: f32| {
            let q = wheel.keys.sample(t);
            let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
            assert!(q[0].abs() < 1e-4 && q[1].abs() < 1e-4);
            2. * (q[2] / n).atan2(q[3] / n).to_degrees()
        };
        // The centre key (t = 61/30 s) is the file's bind: a quaternion
        // (x, y, z, w) is the rotation whose row-vector matrix has the rows
        // R^T, as `scene::matrix` stores them.
        let interior = std::fs::read(std::path::Path::new(&root).join("interiors/lac/int_lac.pssg")).unwrap();
        let tree = crate::pssg::read_tree(&interior).unwrap();
        let node = (0..tree.nodes.len())
            .find(|&i| tree.name(i) == b"NODE" && tree.string(i, b"id").ok() == Some(b"steering_wheel".as_slice()))
            .unwrap();
        let bind = crate::scene::matrix(&tree, node).unwrap();
        let [x, y, z, w] = wheel.keys.sample(61. / 30.);
        let n = (x * x + y * y + z * z + w * w).sqrt();
        let (x, y, z, w) = (x / n, y / n, z / n, w / n);
        // Column-vector rotation R; row-vector rows are its columns.
        let r = [
            [1. - 2. * (y * y + z * z), 2. * (x * y - z * w), 2. * (x * z + y * w)],
            [2. * (x * y + z * w), 1. - 2. * (x * x + z * z), 2. * (y * z - x * w)],
            [2. * (x * z - y * w), 2. * (y * z + x * w), 1. - 2. * (x * x + y * y)],
        ];
        for row in 0..3 {
            for col in 0..3 {
                assert!((bind[row * 4 + col] - r[col][row]).abs() < 1e-3, "bind {bind:?} r {r:?}");
            }
        }
        assert!(angle(0.).abs() < 0.05, "{}", angle(0.));
        assert!((angle(2.) - 88.48).abs() < 0.05, "{}", angle(2.));
        assert!((angle(4.) - 179.95).abs() < 0.05, "{}", angle(4.));
    }
}
