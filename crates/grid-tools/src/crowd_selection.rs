//! Pure, authored categorical projection of the source HLA selector.
//! No I/O, engine clock, runtime actor binding, RNG ownership or pose policy.
//! Source contracts: CROWD_ANIMATION_RESEARCH and private factor-evaluation150.
use grid_formats::binxml::{Document, Element};

const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 16_384;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Schema(&'static str),
    Limit,
    InvalidDefault,
    GameOverrideNeedsProvider,
    MissingRawName,
    UnresolvedRawName,
    AmbiguousRawName,
}

#[derive(Clone, Debug)]
pub struct RawAnimation<'a> {
    /// Source rawAnim ordinal, never sorted/deduplicated.
    pub ordinal: usize,
    /// Original Document element; caller retains all unconsumed raw metadata.
    pub element: usize,
    pub name: &'a str,
    pub description: Option<&'a str>,
}

#[derive(Clone, Debug)]
struct Leaf<'a> {
    raw: Option<&'a str>,
    game: Option<&'a str>,
    mirrored: bool,
}

#[derive(Clone, Debug)]
enum Selector<'a> {
    Empty,
    Factor {
        name: &'a str,
        default: &'a str,
        branches: Vec<(&'a str, Selector<'a>)>,
    },
    Random(Vec<Selector<'a>>),
    Anim(Leaf<'a>),
}

#[derive(Clone, Debug)]
pub struct Library<'a> {
    pub raw: Vec<RawAnimation<'a>>,
    tree: Selector<'a>,
}

/// Explicit named categories. Runtime conversion from the native five actor
/// indices is deliberately outside this pure module and remains experimental.
pub type CategoricalInputs<'a> = [(&'a str, &'a str)];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate<'a> {
    pub raw_name: &'a str,
    /// Every matching source ordinal, including ambiguous duplicates.
    pub raw_ordinals: Vec<usize>,
    pub mirrored: bool,
    pub used_default: bool,
}
impl Candidate<'_> {
    /// Safe authored admission; not an invented native duplicate-name policy.
    pub fn unique_ordinal(&self) -> Result<usize, Error> {
        match self.raw_ordinals.as_slice() {
            [only] => Ok(*only),
            [] => Err(Error::UnresolvedRawName),
            _ => Err(Error::AmbiguousRawName),
        }
    }
}

fn attr<'a>(e: &Element<'a>, name: &str) -> Result<Option<&'a str>, Error> {
    let mut values = e.attributes.iter().filter(|(key, _)| *key == name);
    let result = values.next().map(|(_, value)| *value);
    if values.next().is_some() {
        return Err(Error::Schema("duplicate attribute"));
    }
    Ok(result)
}
fn required<'a>(e: &Element<'a>, name: &str) -> Result<&'a str, Error> {
    attr(e, name)?.ok_or(Error::Schema("missing attribute"))
}
// Document is public; check every traversed range even for caller-built input.
fn children(d: &Document<'_>, index: usize) -> Result<std::ops::Range<usize>, Error> {
    let e = d
        .elements
        .get(index)
        .ok_or(Error::Schema("element index out of bounds"))?;
    if e.children.start > e.children.end || e.children.end > d.elements.len() {
        return Err(Error::Schema("child range out of bounds"));
    }
    Ok(e.children.clone())
}
fn child_value<'a>(d: &Document<'a>, parent: usize, name: &str) -> Result<Option<&'a str>, Error> {
    let mut result = None;
    let mut found = false;
    for i in children(d, parent)? {
        let e = d
            .elements
            .get(i)
            .ok_or(Error::Schema("element index out of bounds"))?;
        if e.name == name {
            if found {
                return Err(Error::Schema("duplicate raw field"));
            }
            found = true;
            result = e.value;
        }
    }
    Ok(result)
}

fn parse_node<'a>(
    d: &Document<'a>,
    index: usize,
    depth: usize,
    budget: &mut usize,
) -> Result<Selector<'a>, Error> {
    if depth > MAX_DEPTH || *budget == 0 {
        return Err(Error::Limit);
    }
    *budget -= 1;
    let child_indices = children(d, index)?;
    let e = d
        .elements
        .get(index)
        .ok_or(Error::Schema("element index out of bounds"))?;
    match e.name {
        // AD4780 converts childless Random to Empty before parsing distribution.
        "animSelectionNodeEmpty" => Ok(Selector::Empty),
        "animSelectionNodeRandom" if child_indices.is_empty() => Ok(Selector::Empty),
        "animSelectionNodeRandom" => {
            // Actual E52200 names enumerate 0/1/2; B76EB0 uses rand()%N for all.
            match required(e, "distribution")? {
                "uniform" | "skewed" | "userDefined" => {}
                _ => return Err(Error::Schema("unsupported random distribution")),
            }
            let nodes: Vec<Selector<'a>> = child_indices
                .map(|i| parse_node(d, i, depth + 1, budget))
                .collect::<Result<_, _>>()?;
            if nodes.iter().any(|n| matches!(n, Selector::Empty)) {
                return Err(Error::Schema("unsupported null Random child"));
            }
            Ok(Selector::Random(nodes))
        }
        "animSelectionNodeFactor" => {
            let name = required(e, "factor")?;
            let default = required(e, "default")?;
            let mut branches = Vec::new();
            for i in child_indices {
                let branch_children = children(d, i)?;
                let value = d
                    .elements
                    .get(i)
                    .ok_or(Error::Schema("element index out of bounds"))?;
                if value.name != "animSelectionNodeFactorValue" || branch_children.len() > 1 {
                    return Err(Error::Schema("factor branch must contain at most one node"));
                }
                let label = required(value, "value")?;
                if branches.iter().any(|(old, _)| *old == label) {
                    return Err(Error::Schema("duplicate factor branch"));
                }
                let child = if branch_children.is_empty() {
                    // Native child-view name lookup returns null and AD4780
                    // produces Empty. Count authored null admission in our budget.
                    if *budget == 0 {
                        return Err(Error::Limit);
                    }
                    *budget -= 1;
                    Selector::Empty
                } else {
                    parse_node(d, branch_children.start, depth + 1, budget)?
                };
                branches.push((label, child));
            }
            Ok(Selector::Factor {
                name,
                default,
                branches,
            })
        }
        "animSelectionNodeAnim" => {
            if !child_indices.is_empty() {
                return Err(Error::Schema("unexpected Anim child"));
            }
            let mirrored = match required(e, "mirrored")? {
                "true" => true,
                "false" => false,
                _ => return Err(Error::Schema("unsupported mirrored value")),
            };
            Ok(Selector::Anim(Leaf {
                raw: attr(e, "rawAnimNameId")?,
                game: attr(e, "gameAnimNameId")?,
                mirrored,
            }))
        }
        _ => Err(Error::Schema("unsupported selection node")),
    }
}

impl<'a> Library<'a> {
    /// Reuses binary XML text values. Traversed indices/ranges are checked even
    /// for caller-built Documents; selector depth/node limits bound cycles.
    /// This does not replace the byte reader's full tree/parent validation.
    pub fn parse(d: &Document<'a>) -> Result<Self, Error> {
        let root = d.elements.first().ok_or(Error::Schema("empty document"))?;
        if root.name != "HighLevelAnimation" {
            return Err(Error::Schema("wrong root"));
        }
        let mut raw = Vec::new();
        let mut tree = None;
        let mut budget = MAX_NODES;
        for i in children(d, 0)? {
            let e = d
                .elements
                .get(i)
                .ok_or(Error::Schema("element index out of bounds"))?;
            match e.name {
                "rawAnim" => raw.push(RawAnimation {
                    ordinal: raw.len(),
                    element: i,
                    name: child_value(d, i, "nameId")?.ok_or(Error::MissingRawName)?,
                    description: child_value(d, i, "AnimDescription")?,
                }),
                "AnimSelectionTree" => {
                    let tree_children = children(d, i)?;
                    if tree.is_some() || tree_children.len() != 1 {
                        return Err(Error::Schema("need exactly one selection root"));
                    }
                    tree = Some(parse_node(d, tree_children.start, 0, &mut budget)?);
                }
                _ => return Err(Error::Schema("unsupported HLA root child")),
            }
        }
        Ok(Self {
            raw,
            tree: tree.ok_or(Error::Schema("missing selection tree"))?,
        })
    }

    /// Selects through explicit categorical (one-hot) factors only. This is not
    /// the general multi-positive weighted Factor interface or a native clock.
    /// `draw` supplies a nonnegative CRT-style random result; no RNG is owned here.
    pub fn select(
        &self,
        inputs: &CategoricalInputs<'_>,
        draw: &mut impl FnMut() -> u32,
    ) -> Result<Option<Candidate<'a>>, Error> {
        for (i, (name, _)) in inputs.iter().enumerate() {
            if inputs[..i].iter().any(|(old, _)| old == name) {
                return Err(Error::Schema("duplicate categorical input"));
            }
        }
        let mut used_default = false;
        let Some(leaf) = select_node(&self.tree, inputs, draw, &mut used_default)? else {
            return Ok(None);
        };
        if leaf.game.is_some() {
            return Err(Error::GameOverrideNeedsProvider);
        }
        let name = leaf.raw.ok_or(Error::MissingRawName)?;
        let raw_ordinals = self
            .raw
            .iter()
            .filter(|r| r.name == name)
            .map(|r| r.ordinal)
            .collect();
        Ok(Some(Candidate {
            raw_name: name,
            raw_ordinals,
            mirrored: leaf.mirrored,
            used_default,
        }))
    }
}

fn select_node<'a, 's>(
    node: &'s Selector<'a>,
    inputs: &CategoricalInputs<'_>,
    draw: &mut impl FnMut() -> u32,
    used_default: &mut bool,
) -> Result<Option<&'s Leaf<'a>>, Error> {
    match node {
        Selector::Empty => Ok(None),
        Selector::Anim(leaf) => Ok(Some(leaf)),
        Selector::Random(children) => {
            let index = (u64::from(draw()) % children.len() as u64) as usize;
            select_node(&children[index], inputs, draw, used_default)
        }
        Selector::Factor {
            name,
            default,
            branches,
        } => {
            let requested = inputs
                .iter()
                .find(|(factor, _)| factor == name)
                .map(|(_, value)| *value);
            let child = requested.and_then(|v| branches.iter().find(|(label, _)| *label == v));
            let chosen = match child {
                Some((_, node)) if !matches!(node, Selector::Empty) => node,
                _ => {
                    *used_default = true;
                    let (_, node) = branches
                        .iter()
                        .find(|(label, _)| label == default)
                        .ok_or(Error::InvalidDefault)?;
                    if matches!(node, Selector::Empty) {
                        return Err(Error::InvalidDefault);
                    }
                    node
                }
            };
            select_node(chosen, inputs, draw, used_default)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, ops::Range};
    #[derive(Clone)]
    struct S {
        name: &'static str,
        value: Option<&'static str>,
        attrs: Vec<(&'static str, &'static str)>,
        children: Vec<S>,
    }
    fn n(name: &'static str, attrs: &[(&'static str, &'static str)], children: Vec<S>) -> S {
        S {
            name,
            value: None,
            attrs: attrs.to_vec(),
            children,
        }
    }
    fn leaf(name: &'static str) -> S {
        n(
            "animSelectionNodeAnim",
            &[("rawAnimNameId", name), ("mirrored", "false")],
            vec![],
        )
    }
    fn raw(name: &'static str) -> S {
        n(
            "rawAnim",
            &[],
            vec![S {
                name: "nameId",
                value: Some(name),
                attrs: vec![],
                children: vec![],
            }],
        )
    }
    fn branch(value: &'static str, node: S) -> S {
        n(
            "animSelectionNodeFactorValue",
            &[("value", value)],
            vec![node],
        )
    }
    fn factor(children: Vec<S>) -> S {
        n(
            "animSelectionNodeFactor",
            &[("factor", "F"), ("default", "A")],
            children,
        )
    }
    fn random(children: Vec<S>) -> S {
        n(
            "animSelectionNodeRandom",
            &[("distribution", "uniform")],
            children,
        )
    }
    fn document(tree: S, names: &[&'static str]) -> Document<'static> {
        let mut children: Vec<_> = names.iter().copied().map(raw).collect();
        children.push(n("AnimSelectionTree", &[], vec![tree]));
        let root = n("HighLevelAnimation", &[], children);
        fn elem(s: &S, parent: Option<usize>) -> Element<'static> {
            Element {
                name: s.name,
                value: s.value,
                attributes: s.attrs.clone(),
                children: Range { start: 0, end: 0 },
                parent,
            }
        }
        let mut elements = vec![elem(&root, None)];
        let mut todo = VecDeque::from([(&root, 0)]);
        while let Some((s, index)) = todo.pop_front() {
            let first = elements.len();
            for child in &s.children {
                let next = elements.len();
                elements.push(elem(child, Some(index)));
                todo.push_back((child, next));
            }
            elements[index].children = first..elements.len();
        }
        Document {
            strings: vec![],
            elements,
            string_padding: &[],
        }
    }
    #[test]
    fn categorical_selection_and_empty_child_default_are_source_behaviors() {
        let d = document(
            factor(vec![
                branch("A", leaf("a")),
                branch("B", random(vec![])),
                branch("C", leaf("c")),
            ]),
            &["a", "c"],
        );
        let l = Library::parse(&d).unwrap();
        let no_draw = || -> u32 { panic!("Factor/Anim must not draw random") };
        let a = l.select(&[("F", "A")], &mut { no_draw }).unwrap().unwrap();
        assert_eq!(
            (a.raw_name, a.unique_ordinal(), a.used_default),
            ("a", Ok(0), false)
        );
        let c = l.select(&[("F", "C")], &mut { no_draw }).unwrap().unwrap();
        assert_eq!(
            (c.raw_name, c.unique_ordinal(), c.used_default),
            ("c", Ok(1), false)
        );
        for inputs in [vec![], vec![("F", "B")], vec![("F", "unknown")]] {
            let c = l.select(&inputs, &mut { no_draw }).unwrap().unwrap();
            assert_eq!((c.raw_name, c.used_default), ("a", true));
        }
    }
    #[test]
    fn empty_random_does_not_read_distribution_or_draw() {
        let d = document(n("animSelectionNodeRandom", &[], vec![]), &[]);
        assert_eq!(
            Library::parse(&d)
                .unwrap()
                .select(&[], &mut || panic!("empty node drew RNG")),
            Ok(None)
        );
    }
    #[test]
    fn caller_draw_modulo_and_all_registered_modes_are_explicit() {
        for mode in ["uniform", "skewed", "userDefined"] {
            let d = document(
                n(
                    "animSelectionNodeRandom",
                    &[("distribution", mode)],
                    vec![leaf("z"), leaf("a")],
                ),
                &["z", "a"],
            );
            let l = Library::parse(&d).unwrap();
            let mut calls = 0;
            let c = l
                .select(&[], &mut || {
                    calls += 1;
                    3
                })
                .unwrap()
                .unwrap();
            assert_eq!((c.raw_name, c.unique_ordinal(), calls), ("a", Ok(1), 1));
        }
    }
    #[test]
    fn raw_catalogue_preserves_duplicate_ordinals_and_requires_explicit_resolution() {
        let d = document(leaf("same"), &["other", "same", "same"]);
        let l = Library::parse(&d).unwrap();
        assert_eq!(
            l.raw.iter().map(|r| r.ordinal).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        let c = l.select(&[], &mut || 0).unwrap().unwrap();
        assert_eq!(c.raw_ordinals, [1, 2]);
        assert_eq!(c.unique_ordinal(), Err(Error::AmbiguousRawName));
    }
    #[test]
    fn unknown_raw_name_and_game_override_are_not_silently_guessed() {
        let d = document(leaf("missing"), &["other"]);
        let c = Library::parse(&d)
            .unwrap()
            .select(&[], &mut || 0)
            .unwrap()
            .unwrap();
        assert_eq!(c.unique_ordinal(), Err(Error::UnresolvedRawName));
        let d = document(
            n(
                "animSelectionNodeAnim",
                &[
                    ("rawAnimNameId", "a"),
                    ("gameAnimNameId", "dynamic"),
                    ("mirrored", "false"),
                ],
                vec![],
            ),
            &["a"],
        );
        assert_eq!(
            Library::parse(&d).unwrap().select(&[], &mut || 0),
            Err(Error::GameOverrideNeedsProvider)
        );
    }
    #[test]
    fn absent_default_and_duplicate_branches_are_explicit_authored_admission_failures() {
        let d = document(factor(vec![branch("B", leaf("a"))]), &["a"]);
        assert_eq!(
            Library::parse(&d).unwrap().select(&[], &mut || 0),
            Err(Error::InvalidDefault)
        );
        let d = document(
            factor(vec![branch("A", leaf("a")), branch("A", leaf("b"))]),
            &["a", "b"],
        );
        assert!(matches!(
            Library::parse(&d),
            Err(Error::Schema("duplicate factor branch"))
        ));
    }
    #[test]
    fn public_document_bad_ranges_and_cycles_refuse_without_panicking() {
        let mut d = document(leaf("a"), &["a"]);
        d.elements[0].children = 1..usize::MAX;
        assert!(matches!(
            Library::parse(&d),
            Err(Error::Schema("child range out of bounds"))
        ));
        let mut d = document(leaf("a"), &["a"]);
        d.elements[0].children = std::ops::Range { start: 3, end: 2 };
        assert!(matches!(
            Library::parse(&d),
            Err(Error::Schema("child range out of bounds"))
        ));
        let mut d = document(factor(vec![branch("A", leaf("a"))]), &["a"]);
        let i = d
            .elements
            .iter()
            .position(|e| e.name == "animSelectionNodeFactorValue")
            .unwrap();
        d.elements[i].children = usize::MAX..usize::MAX;
        assert!(matches!(
            Library::parse(&d),
            Err(Error::Schema("child range out of bounds"))
        ));
        let mut d = document(random(vec![leaf("a")]), &["a"]);
        let i = d
            .elements
            .iter()
            .position(|e| e.name == "animSelectionNodeRandom")
            .unwrap();
        d.elements[i].children = i..i + 1;
        assert!(matches!(Library::parse(&d), Err(Error::Limit)));
    }
    #[test]
    fn depth_and_node_budget_have_exact_boundaries() {
        fn nested(n: usize) -> S {
            let mut s = leaf("a");
            for _ in 0..n {
                s = random(vec![s]);
            }
            s
        }
        let d = document(nested(MAX_DEPTH), &["a"]);
        assert!(Library::parse(&d).is_ok());
        let d = document(nested(MAX_DEPTH + 1), &["a"]);
        assert!(matches!(Library::parse(&d), Err(Error::Limit)));
        let d = document(random(vec![leaf("a"); MAX_NODES - 1]), &["a"]);
        assert!(Library::parse(&d).is_ok());
        let d = document(random(vec![leaf("a"); MAX_NODES]), &["a"]);
        assert!(matches!(Library::parse(&d), Err(Error::Limit)));
    }
    #[test]
    fn duplicate_attributes_and_categorical_inputs_refuse() {
        let d = document(
            n(
                "animSelectionNodeAnim",
                &[
                    ("rawAnimNameId", "a"),
                    ("mirrored", "false"),
                    ("mirrored", "true"),
                ],
                vec![],
            ),
            &["a"],
        );
        assert!(matches!(
            Library::parse(&d),
            Err(Error::Schema("duplicate attribute"))
        ));
        let d = document(leaf("a"), &["a"]);
        assert_eq!(
            Library::parse(&d)
                .unwrap()
                .select(&[("F", "A"), ("F", "B")], &mut || panic!("bad inputs drew")),
            Err(Error::Schema("duplicate categorical input"))
        );
    }
    #[test]
    fn nonempty_random_rejects_unknown_distribution_and_null_child() {
        let d = document(
            n(
                "animSelectionNodeRandom",
                &[("distribution", "unknown")],
                vec![leaf("a")],
            ),
            &["a"],
        );
        assert!(matches!(
            Library::parse(&d),
            Err(Error::Schema("unsupported random distribution"))
        ));
        for empty in [n("animSelectionNodeEmpty", &[], vec![]), random(vec![])] {
            let d = document(random(vec![leaf("a"), empty]), &["a"]);
            assert!(matches!(
                Library::parse(&d),
                Err(Error::Schema("unsupported null Random child"))
            ));
        }
        let d = document(
            n(
                "animSelectionNodeRandom",
                &[("distribution", "unknown")],
                vec![],
            ),
            &[],
        );
        assert_eq!(
            Library::parse(&d)
                .unwrap()
                .select(&[], &mut || panic!("empty drew")),
            Ok(None)
        );
    }
    #[test]
    fn anim_absent_empty_or_malformed_fields_have_explicit_outcomes() {
        let d = document(
            n("animSelectionNodeAnim", &[("mirrored", "false")], vec![]),
            &[],
        );
        assert_eq!(
            Library::parse(&d).unwrap().select(&[], &mut || 0),
            Err(Error::MissingRawName)
        );
        let d = document(leaf(""), &[]);
        assert_eq!(
            Library::parse(&d)
                .unwrap()
                .select(&[], &mut || 0)
                .unwrap()
                .unwrap()
                .unique_ordinal(),
            Err(Error::UnresolvedRawName)
        );
        let d = document(
            n(
                "animSelectionNodeAnim",
                &[("rawAnimNameId", "a"), ("mirrored", "yes")],
                vec![],
            ),
            &["a"],
        );
        assert!(matches!(
            Library::parse(&d),
            Err(Error::Schema("unsupported mirrored value"))
        ));
        let d = document(
            n("animSelectionNodeAnim", &[("rawAnimNameId", "a")], vec![]),
            &["a"],
        );
        assert!(matches!(
            Library::parse(&d),
            Err(Error::Schema("missing attribute"))
        ));
    }
    #[test]
    fn empty_default_refuses_but_empty_selection_uses_nonempty_default() {
        let d = document(
            factor(vec![
                branch("A", n("animSelectionNodeEmpty", &[], vec![])),
                branch("B", leaf("b")),
            ]),
            &["b"],
        );
        let l = Library::parse(&d).unwrap();
        assert_eq!(l.select(&[], &mut || 0), Err(Error::InvalidDefault));
        assert_eq!(
            l.select(&[("F", "A")], &mut || 0),
            Err(Error::InvalidDefault)
        );
        assert!(
            !l.select(&[("F", "B")], &mut || 0)
                .unwrap()
                .unwrap()
                .used_default
        );
        let d = document(
            factor(vec![
                branch("A", leaf("a")),
                branch("B", n("animSelectionNodeEmpty", &[], vec![])),
            ]),
            &["a"],
        );
        let c = Library::parse(&d)
            .unwrap()
            .select(&[("F", "B")], &mut || panic!("default drew"))
            .unwrap()
            .unwrap();
        assert_eq!((c.raw_name, c.used_default), ("a", true));
    }
    #[test]
    fn source_childless_factor_value_is_null_and_obeys_declared_default() {
        let d = document(
            factor(vec![
                branch("A", leaf("a")),
                n("animSelectionNodeFactorValue", &[("value", "B")], vec![]),
            ]),
            &["a"],
        );
        let c = Library::parse(&d)
            .unwrap()
            .select(&[("F", "B")], &mut || panic!("null child drew RNG"))
            .unwrap()
            .unwrap();
        assert_eq!((c.raw_name, c.used_default), ("a", true));
        let d = document(
            factor(vec![n(
                "animSelectionNodeFactorValue",
                &[("value", "A")],
                vec![],
            )]),
            &["a"],
        );
        assert_eq!(
            Library::parse(&d).unwrap().select(&[], &mut || 0),
            Err(Error::InvalidDefault)
        );
        let i = d
            .elements
            .iter()
            .position(|e| e.name == "animSelectionNodeFactor")
            .unwrap();
        assert!(matches!(parse_node(&d, i, 0, &mut 1), Err(Error::Limit)));
        let d = document(
            factor(vec![n(
                "animSelectionNodeFactorValue",
                &[("value", "A")],
                vec![leaf("a"), leaf("a")],
            )]),
            &["a"],
        );
        assert!(matches!(
            Library::parse(&d),
            Err(Error::Schema("factor branch must contain at most one node"))
        ));
    }
}
