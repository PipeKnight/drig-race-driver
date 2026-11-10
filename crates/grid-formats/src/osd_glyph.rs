//! Front-end glyph instancing (docs/RACE_HUD.md §6.6): a front-end package
//! defines each glyph once, as the subtree of a node `<glyph>_root`, and
//! places it with instance nodes `kind/glyph=instance` (`screen/`, `gadget/`,
//! `object/`, `component/`, `kitpart/`) that have no children of their own.
//! Drawing a screen therefore walks its tree and, at every instance, grafts
//! the glyph's definition below the instance's transform, recursively. A
//! definition is looked up in the instancing package first, then in the other
//! loaded packages (feNonPersistentDatabase instances fePersistentDatabase's
//! glyphs).
//!
//! The grafting itself is [I]: the packages hold no other link between an
//! instance and its glyph, and the definition root's own transform (identity
//! in every traced glyph) is not applied.
use crate::osd::Scene;

/// The deepest chain of instances grafted into each other.
pub const MAX_DEPTH: usize = 16;

/// The glyph an instance node draws: `gadget/list_items=1` → `list_items`.
pub fn instance_glyph(id: &str) -> Option<&str> {
    let name = id.split('!').next()?;
    let (head, _) = name.split_once('=')?;
    let (kind, glyph) = head.split_once('/')?;
    (!kind.is_empty() && !glyph.is_empty()).then_some(glyph)
}

/// One node as drawn: the package it comes from, its index there, its world
/// matrix after grafting, how many instances lead to it and the index (in
/// the expansion) of the node it hangs below (`None` for the root).
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub scene: usize,
    pub node: usize,
    pub world: [f32; 16],
    pub depth: usize,
    pub parent: Option<usize>,
}

/// Every node at or below `root` of `scenes[scene]`, with each instance's
/// glyph grafted under it. An instance whose glyph is not defined in any
/// package, or which would graft a glyph into itself (directly or through
/// other glyphs), stays a leaf.
pub fn expand(scenes: &[&Scene<'_>], scene: usize, root: usize) -> Vec<Placed> {
    let parent_world = match scenes[scene].nodes[root].parent {
        Some(p) => scenes[scene].world(p),
        None => IDENTITY,
    };
    expand_with(scenes, scene, root, &parent_world, &|s, n| scenes[s].nodes[n].local)
}

/// [`expand`] below `parent_world`, with each node's local matrix from
/// `local(scene, node)` (an animated pose instead of the saved one).
pub fn expand_with(
    scenes: &[&Scene<'_>],
    scene: usize,
    root: usize,
    parent_world: &[f32; 16],
    local: &dyn Fn(usize, usize) -> [f32; 16],
) -> Vec<Placed> {
    let children: Vec<Vec<Vec<usize>>> = scenes
        .iter()
        .map(|s| {
            let mut c = vec![Vec::new(); s.nodes.len()];
            for (i, n) in s.nodes.iter().enumerate() {
                if let Some(p) = n.parent {
                    c[p].push(i);
                }
            }
            c
        })
        .collect();
    let mut out = Vec::new();
    // The expanded root is on the stack, so a glyph that instances itself stays a leaf.
    let mut stack = vec![(scene, root)];
    let mut walker = Walker { scenes, children: &children, local, stack: &mut stack, out: &mut out };
    walker.walk(scene, root, parent_world, None);
    out
}

const IDENTITY: [f32; 16] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];

/// The definition root of `glyph`: in `scene` first, then in the others in order.
fn definition(scenes: &[&Scene<'_>], scene: usize, glyph: &str) -> Option<(usize, usize)> {
    let id = format!("{glyph}_root");
    std::iter::once(scene)
        .chain((0..scenes.len()).filter(|&s| s != scene))
        .find_map(|s| scenes[s].node(&id).map(|n| (s, n)))
}

struct Walker<'a, 'b> {
    scenes: &'a [&'a Scene<'b>],
    children: &'a [Vec<Vec<usize>>],
    local: &'a dyn Fn(usize, usize) -> [f32; 16],
    stack: &'a mut Vec<(usize, usize)>,
    out: &'a mut Vec<Placed>,
}

impl Walker<'_, '_> {
    fn walk(&mut self, scene: usize, node: usize, parent_world: &[f32; 16], parent: Option<usize>) {
        let world = crate::scene::compose(&(self.local)(scene, node), parent_world);
        let me = self.out.len();
        self.out.push(Placed { scene, node, world, depth: self.stack.len() - 1, parent });
        let children = self.children;
        for &c in &children[scene][node] {
            self.walk(scene, c, &world, Some(me));
        }
        if !children[scene][node].is_empty() || self.stack.len() > MAX_DEPTH {
            return;
        }
        let Some(glyph) = instance_glyph(&self.scenes[scene].nodes[node].id) else {
            return;
        };
        let Some(def) = definition(self.scenes, scene, glyph) else {
            return;
        };
        if self.stack.contains(&def) {
            return;
        }
        self.stack.push(def);
        for &c in &children[def.0][def.1] {
            self.walk(def.0, c, &world, Some(me));
        }
        self.stack.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osd::{Node, Scene};

    fn at(x: f32) -> [f32; 16] {
        let mut m = IDENTITY;
        m[12] = x;
        m
    }

    fn scene(nodes: &[(&str, Option<usize>, f32)]) -> Scene<'static> {
        Scene {
            textures: vec![],
            wraps: vec![],
            cubemaps: vec![],
            materials: vec![],
            nodes: nodes
                .iter()
                .map(|&(id, parent, x)| Node { id: id.into(), parent, local: at(x), bounds: [0.; 6], render: None, text: None })
                .collect(),
            camera: None,
            cameras: vec![],
            animations: vec![],
        }
    }

    #[test]
    fn instances_name_their_glyph() {
        assert_eq!(instance_glyph("gadget/list_items=1"), Some("list_items"));
        assert_eq!(instance_glyph("object/leaderboard_bg=bg!2"), Some("leaderboard_bg"));
        assert_eq!(instance_glyph("null=table"), None);
        assert_eq!(instance_glyph("leaderboards_root"), None);
        assert_eq!(instance_glyph("/x=y"), None);
    }

    #[test]
    fn instances_graft_their_glyph_under_their_transform() {
        let library = scene(&[("pill_root", None, 500.), ("null=face", Some(0), 1.), ("text=label", Some(1), 0.5)]);
        let screen = scene(&[
            ("results_root", None, 0.),
            ("object/pill=a", Some(0), 10.),
            ("object/pill=b!1", Some(0), 20.),
            ("object/missing=c", Some(0), 30.),
            ("object/results=loop", Some(0), 40.),
        ]);
        let placed = expand(&[&screen, &library], 0, 0);
        let drawn: Vec<(usize, &str, f32, usize)> = placed
            .iter()
            .map(|p| (p.scene, [&screen, &library][p.scene].nodes[p.node].id.as_str(), p.world[12], p.depth))
            .collect();
        // The definition root's own transform (500) is not applied; a missing glyph and a
        // glyph that would contain itself stay leaves.
        assert_eq!(
            drawn,
            [
                (0, "results_root", 0., 0),
                (0, "object/pill=a", 10., 0),
                (1, "null=face", 11., 1),
                (1, "text=label", 11.5, 1),
                (0, "object/pill=b!1", 20., 0),
                (1, "null=face", 21., 1),
                (1, "text=label", 21.5, 1),
                (0, "object/missing=c", 30., 0),
                (0, "object/results=loop", 40., 0),
            ]
        );
        // Each node names the one it hangs below; a grafted child hangs below its instance.
        let parents: Vec<Option<usize>> = placed.iter().map(|p| p.parent).collect();
        assert_eq!(parents, [None, Some(0), Some(1), Some(2), Some(0), Some(4), Some(5), Some(0), Some(0)]);
        // A posed expansion moves every instance of a glyph node alike.
        let posed = expand_with(&[&screen, &library], 0, 0, &IDENTITY, &|s, n| {
            if (s, n) == (1, 1) { at(3.) } else { [&screen, &library][s].nodes[n].local }
        });
        let xs: Vec<f32> = posed.iter().map(|p| p.world[12]).collect();
        assert_eq!(xs, [0., 10., 13., 13.5, 20., 23., 23.5, 30., 40.]);
    }

    /// Needs GRID_GAME_ROOT. The post-race glyphs of RACE_HUD §6.6 expand to more drawn
    /// meshes than their own trees hold, and every instance below them resolves.
    #[test]
    #[ignore]
    fn post_race_glyphs_expand_through_the_front_end_packages() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let files = ["fePersistentDatabase.pssg", "feNonPersistentDatabase.pssg", "fe_DLC_0.pssg"];
        let bytes: Vec<Vec<u8>> = files.iter().map(|f| std::fs::read(root.join("frontend").join(f)).unwrap()).collect();
        let trees: Vec<_> = bytes.iter().map(|b| crate::pssg::read_tree(b).unwrap()).collect();
        let scenes: Vec<Scene> = trees.iter().map(|t| crate::osd::read_scene(t).unwrap()).collect();
        let refs: Vec<&Scene> = scenes.iter().collect();
        for glyph in ["leaderboards_root", "post_race_stats_root", "drift_scoreboard_root"] {
            let node = scenes[0].node(glyph).unwrap();
            let placed = expand(&refs, 0, node);
            let meshes = |p: &[Placed], grafted: bool| {
                p.iter().filter(|p| (p.depth > 0) == grafted && refs[p.scene].nodes[p.node].render.is_some()).count()
            };
            let unresolved: Vec<&str> = placed
                .iter()
                .map(|p| &refs[p.scene].nodes[p.node])
                .filter(|n| instance_glyph(&n.id).is_some_and(|g| definition(&refs, 0, g).is_none()))
                .map(|n| n.id.as_str())
                .collect();
            let deepest = placed.iter().map(|p| p.depth).max().unwrap_or(0);
            println!(
                "{glyph}: {} nodes, {} own meshes, {} grafted meshes, depth {deepest}, unresolved {unresolved:?}",
                placed.len(),
                meshes(&placed, false),
                meshes(&placed, true)
            );
            assert!(meshes(&placed, true) > 0, "{glyph}");
            assert!(unresolved.is_empty(), "{glyph}: {unresolved:?}");
        }
        // The loading world (grid-app fe_world, `loading_root`) for comparison: how many
        // of its nodes are instances and what grafting would add to it.
        let loading = expand(&refs, 0, scenes[0].node("loading_root").unwrap());
        let instances = loading.iter().filter(|p| instance_glyph(&refs[p.scene].nodes[p.node].id).is_some()).count();
        let grafted = loading.iter().filter(|p| p.depth > 0 && refs[p.scene].nodes[p.node].render.is_some()).count();
        println!("loading_root: {} nodes, {instances} instances, {grafted} grafted meshes", loading.len());
    }
}
