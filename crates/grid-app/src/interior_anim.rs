//! The cockpit's interior animation set (docs/COCKPIT.md "Steering wheel",
//! "Pedals"): `int_<stem>_anims.pssg`, set 0 (0x84C5F0 looks up Steering,
//! change_up, change_down, HandBrake_on, accel and Brake in it). The interior
//! update (0x859FC0) samples each playing clip at its timer and the key
//! replaces the local transform of every node the clip keys; parts below a
//! keyed node follow it.
use super::read;
use bevy::prelude::*;
use grid_formats::{neanim, pssg, scene};
use std::{collections::BTreeMap, error::Error, path::Path};

/// The interior clips and the cockpit nodes they key.
#[derive(Clone, Debug, Default)]
pub struct Interior {
    /// Clip names and durations (end − start, 0x856080), in set order.
    clips: Vec<(String, f32)>,
    /// Every keyed node, parents first.
    nodes: Vec<Node>,
    /// Each part below a keyed node: that node and the part's frame
    /// relative to it.
    pub parts: BTreeMap<String, (usize, Mat4)>,
}

#[derive(Clone, Debug)]
struct Node {
    name: String,
    /// Tree index of the node, for finding the parts below it.
    tree: usize,
    /// The nearest keyed ancestor, and the frame from it (or from the car
    /// frame) to this node's parent.
    above: Option<usize>,
    base: Mat4,
    /// The file's local transform, kept while no playing clip keys it.
    file: Mat4,
    /// Per clip: the node's rotation and translation channels.
    keyed: Vec<Option<Keyed>>,
}

#[derive(Clone, Debug)]
struct Keyed {
    rotation: Option<neanim::Keys>,
    translation: Vec<neanim::Keys>,
}

/// One playing clip: its index, local time and weight.
pub type Play = (usize, f32, f32);

impl Interior {
    /// The set of `stem`'s cockpit on the nodes of `tree` placed by
    /// `anchor`; `None` without the file.
    pub fn load(root: &Path, stem: &str, tree: &pssg::Tree, anchor: Mat4) -> Result<Option<Self>, Box<dyn Error>> {
        let path = root.join(format!("interiors/{stem}/int_{stem}_anims.pssg"));
        if !path.is_file() {
            return Ok(None);
        }
        let sets = neanim::read_sets(&pssg::read_tree(&read(&path)?)?)?;
        let Some(set) = sets.first() else {
            return Ok(None);
        };
        let mut targets: Vec<usize> = Vec::new();
        for c in set.clips.iter().flat_map(|c| &c.channels) {
            if let Some(n) = crate::cockpit::named(tree, b"NODE", c.target.as_bytes())
                && !targets.contains(&n)
            {
                targets.push(n);
            }
        }
        targets.sort_unstable();
        let world = |n: usize| -> Result<Mat4, Box<dyn Error>> { Ok(Mat4::from_cols_array(&scene::node_matrix(tree, n)?)) };
        let mut nodes: Vec<Node> = Vec::new();
        for &n in &targets {
            let name = String::from_utf8_lossy(tree.string(n, b"id")?).into_owned();
            let parent = tree.nodes[n].parent.ok_or("keyed interior node without a parent")?;
            let above = (0..nodes.len()).rev().find(|&a| crate::cockpit::descends(tree, parent, nodes[a].tree));
            let base = match above {
                Some(a) => world(nodes[a].tree)?.inverse() * world(parent)?,
                None => anchor * world(parent)?,
            };
            let keyed = (set.clips.iter())
                .map(|clip| {
                    let target = name.as_str();
                    let of = |semantic: &'static str| {
                        clip.channels.iter().filter(move |c| c.target == target && c.semantic == semantic)
                    };
                    let rotation = of("rotation_quaternion").next().map(|c| c.keys.clone());
                    let translation: Vec<_> = of("translation").map(|c| c.keys.clone()).collect();
                    (rotation.is_some() || !translation.is_empty()).then_some(Keyed { rotation, translation })
                })
                .collect();
            nodes.push(Node { name, tree: n, above, base, file: Mat4::from_cols_array(&scene::matrix(tree, n)?), keyed });
        }
        let clips = set.clips.iter().map(|c| (c.name.clone(), c.duration())).collect();
        Ok(Some(Self { clips, nodes, parts: BTreeMap::new() }))
    }

    /// Registers a part drawn from RENDERNODE `render` with scene world
    /// `transform` when it is below a keyed node (the deepest one).
    pub fn register(&mut self, tree: &pssg::Tree, name: String, render: usize, transform: Mat4) -> Result<(), Box<dyn Error>> {
        if let Some((n, frame)) = self.below(tree, render, transform)? {
            self.parts.insert(name, (n, frame));
        }
        Ok(())
    }

    /// The deepest keyed node above tree node `n` (or `n` itself), and
    /// `transform`, `n`'s scene world, relative to it.
    pub fn below(&self, tree: &pssg::Tree, n: usize, transform: Mat4) -> Result<Option<(usize, Mat4)>, Box<dyn Error>> {
        let Some(k) = (0..self.nodes.len()).rev().find(|&k| crate::cockpit::descends(tree, n, self.nodes[k].tree)) else {
            return Ok(None);
        };
        let world = Mat4::from_cols_array(&scene::node_matrix(tree, self.nodes[k].tree)?);
        Ok(Some((k, world.inverse() * transform)))
    }

    /// The index of clip `name` (any case, as the original's lookup).
    pub fn clip(&self, name: &str) -> Option<usize> {
        self.clips.iter().position(|(n, _)| n.eq_ignore_ascii_case(name))
    }

    pub fn duration(&self, clip: usize) -> f32 {
        self.clips[clip].1
    }

    /// The index of node `name`.
    pub fn node(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    /// Every keyed node in the car frame while `play` runs. A node's local
    /// transform is the weighted sum of the playing clips that key it, the
    /// quaternion normalised (0xACD760, 0xAC67A0), else the file's.
    #[cfg(test)]
    pub fn worlds(&self, play: &[Play]) -> Vec<Mat4> {
        self.worlds_with(play, None)
    }

    /// [`Self::worlds`] with `knob = (node, m)`: `m` between the node's
    /// parent and its local transform (the H-pattern knob's HPatternDummy
    /// above the gear lever, 0x85C380).
    pub fn worlds_with(&self, play: &[Play], knob: Option<(usize, Mat4)>) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(self.nodes.len());
        for (i, node) in self.nodes.iter().enumerate() {
            let (_, file_q, file_t) = node.file.to_scale_rotation_translation();
            let (mut q, mut t) = (None, None);
            for &(clip, time, weight) in play {
                let Some(Some(k)) = node.keyed.get(clip) else { continue };
                if let Some(r) = &k.rotation {
                    *q.get_or_insert(Vec4::ZERO) += weight * Vec4::from_array(r.sample(time));
                }
                for c in &k.translation {
                    *t.get_or_insert(Vec3::ZERO) += weight * Vec4::from_array(c.sample(time)).truncate();
                }
            }
            let q = q.map_or(file_q, |q: Vec4| Quat::from_vec4(q).normalize());
            let mut local = Mat4::from_rotation_translation(q, t.unwrap_or(file_t));
            if let Some((n, m)) = knob
                && n == i
            {
                local = m * local;
            }
            out.push(node.above.map_or(Mat4::IDENTITY, |a| out[a]) * node.base * local);
        }
        out
    }

    /// The parts below `node` (logging).
    pub fn parts_below(&self, name: &str) -> usize {
        self.node(name).map_or(0, |n| self.parts.values().filter(|(p, _)| *p == n).count())
    }
}
