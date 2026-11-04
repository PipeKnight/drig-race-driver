//! Source-table association only: no lamp activation, emission, damage or optical policy.
use grid_formats::{material, pssg, texture};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
const SUFFIX: &str = "_lights_on.";
const SAMPLER: &str = "TEmissiveMap";
const FILTERS: [&[u8]; 5] = [
    b"Head Lights Pod",
    b"Rear Lights Pod",
    b"Lights Pod Matt",
    b"Head Lights Glass",
    b"Rear Lights Glass",
];

// Original material filter is strnicmp(name,filter,strlen(filter)); no trimming/boundary.
fn eligible(name: &str) -> bool {
    FILTERS.iter().any(|filter| {
        name.as_bytes()
            .get(..filter.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(filter))
    })
}
#[derive(Debug)]
pub(crate) struct Bank {
    source: String,
    ids: Result<Vec<String>, String>,
}
impl Bank {
    /// Caller supplies IDs from this actual general package, never a merged livery dictionary.
    pub(crate) fn from_ids(source: &str, ids: Vec<String>) -> Self {
        Self {
            source: source.into(),
            ids: Ok(ids),
        }
    }
    pub(crate) fn load(root: &Path, source: &str) -> Self {
        let ids = (|| {
            let bytes = crate::read(&root.join(source)).map_err(|e| e.to_string())?;
            let tree = pssg::read_tree(&bytes).map_err(|e| e.to_string())?;
            texture::read_textures(&tree)
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|t| text(t.id))
                .collect::<Result<Vec<_>, _>>()
        })();
        Self {
            source: source.into(),
            ids,
        }
    }
    fn unique_texture(&self) -> Result<String, String> {
        let matches: Vec<_> = self
            .ids
            .as_ref()
            .map_err(Clone::clone)?
            .iter()
            .filter(|id| id.contains(SUFFIX))
            .collect();
        match matches.as_slice() {
            [id] => Ok((*id).clone()),
            [] => Err("general bank has no source-substring emissive resource".into()),
            _ => Err("ambiguous general-bank resources: original bucket ordering unproved".into()),
        }
    }
}
#[derive(Debug)]
struct Binding {
    group: String,
    texture: Result<String, String>,
}
/// Actual bank identity; same-named high/low resources are distinct.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct BankKey {
    pub source: String,
    pub resource: String,
}
#[derive(Clone, Debug)]
pub(crate) struct Receipt {
    pub bank: BankKey,
    pub section: crate::lamp_sections::Section,
    pub geometry_source: String,
    pub namespaced_texture: String,
}
#[derive(Debug)]
pub(crate) struct Package {
    source: String,
    namespace: String,
    bank: Bank,
    materials: Result<BTreeMap<String, Binding>, String>,
    sections: Result<Vec<crate::lamp_sections::Section>, String>,
}
impl Package {
    pub(crate) fn read(tree: &pssg::Tree<'_>, source: &str, bank: Bank) -> Self {
        let materials = associations(tree, &bank);
        Self {
            source: source.into(),
            namespace: String::new(),
            bank,
            materials,
            sections: Ok(Vec::new()),
        }
    }
    pub(crate) fn with_sections(mut self, sections: Result<Vec<crate::lamp_sections::Section>, String>) -> Self {
        self.sections = sections;
        self
    }
    fn section_key(&self, draw: &str, joint: &str, material: &str) -> Result<Option<&crate::lamp_sections::Section>, String> {
        let Some(raw_material) = material.strip_prefix(&self.namespace) else { return Ok(None) };
        let matches: Vec<_> = self.sections.as_ref().map_err(Clone::clone)?.iter().filter(|s| s.draw == draw && s.joint == joint && s.material == raw_material).collect();
        if matches.len() > 1 { return Err("ambiguous admitted lamp draw metadata".into()); }
        let Some(section) = matches.first() else { return Ok(None) };
        if self.texture_key(material)?.is_none() { return Ok(None) }
        Ok(Some(*section))
    }
    pub(crate) fn receipt(&self, part: &crate::Part) -> Result<Option<Receipt>, String> {
        self.receipt_key(&part.name, &part.joint, &part.material)
    }
    fn receipt_key(&self, draw: &str, joint: &str, material: &str) -> Result<Option<Receipt>, String> {
        let Some(section) = self.section_key(draw, joint, material)? else { return Ok(None) };
        let raw = material.strip_prefix(&self.namespace).ok_or("lamp namespace changed")?;
        let binding = self.materials.as_ref().map_err(Clone::clone)?.get(raw).ok_or("admitted material disappeared")?;
        let resource = binding.texture.as_ref().map_err(Clone::clone)?.clone();
        Ok(Some(Receipt {
            bank: BankKey { source: self.bank.source.clone(), resource },
            section: section.clone(), geometry_source: self.source.clone(),
            namespaced_texture: self.texture_key(material)?.ok_or("admitted texture disappeared")?,
        }))
    }
    pub(crate) fn in_namespace(mut self, namespace: &str) -> Self {
        self.namespace = namespace.into();
        self
    }
    /// Returns a namespaced candidate key after source role/declaration/resource admission.
    /// The caller must verify upload; low-bank metadata alone is not a loaded GPU resource.
    /// Raw source IDs, group and bank path remain unchanged in retained metadata.
    pub(crate) fn texture_key(&self, material: &str) -> Result<Option<String>, String> {
        let Some(id) = material.strip_prefix(&self.namespace) else {
            return Ok(None);
        };
        if !eligible(id) {
            return Ok(None);
        }
        let materials = self.materials.as_ref().map_err(Clone::clone)?;
        let Some(binding) = materials.get(id) else {
            return Ok(None);
        };
        binding
            .texture
            .as_ref()
            .map(|id| Some(format!("{}{id}", self.namespace)))
            .map_err(Clone::clone)
    }
    pub(crate) fn report(&self) {
        if let Err(error) = &self.sections {
            println!("lamp_section_unavailable source={:?} namespace={:?} reason={error:?} emission_admission=false", self.source, self.namespace);
        }
        if let Ok(sections) = &self.sections && !sections.is_empty() {
            let identity: Vec<_> = sections.iter().map(|s| (s.semantic, s.palette, s.cluster, s.lod, s.node, s.bundle.as_str())).collect();
            println!("lamp_section_metadata source={:?} namespace={:?} identities={identity:?} policy=unique-selected-bundle-not-native-root-order", self.source, self.namespace);
        }
        match &self.materials {
            Err(error) => println!(
                "lamp_association source={:?} namespace={:?} general_bank={:?} unavailable={error:?} emission_policy=false",
                self.source, self.namespace, self.bank.source
            ),
            Ok(materials) => {
                let groups: BTreeSet<_> = materials.values().map(|b| b.group.as_str()).collect();
                let resolved = materials.values().filter(|b| b.texture.is_ok()).count();
                println!(
                    "lamp_association source={:?} namespace={:?} general_bank={:?} bank_resources={:?} materials={} resolved={resolved} groups={groups:?} unresolved={:?} emission_policy=false",
                    self.source,
                    self.namespace,
                    self.bank.source,
                    self.bank.ids.as_ref().map(Vec::len),
                    materials.len(),
                    materials
                        .iter()
                        .filter_map(|(id, b)| b.texture.as_ref().err().map(|e| (id, e)))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
fn text(bytes: &[u8]) -> Result<String, String> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|e| e.to_string())
}
fn associations(tree: &pssg::Tree<'_>, bank: &Bank) -> Result<BTreeMap<String, Binding>, String> {
    // Tree handles framing. Inspect existing declarations, not saved inputs (all404 source
    // eligible instances in audit128 omit a saved emissive assignment).
    let mut groups = BTreeMap::new();
    for (i, node) in tree.nodes.iter().enumerate() {
        if tree.name(i) != b"SHADERGROUP" {
            continue;
        }
        let id = tree.string(i, b"id").map_err(|e| e.to_string())?;
        let mut declarations = vec![];
        for &j in &node.children {
            if tree.name(j) == b"SHADERINPUTDEFINITION" {
                declarations.push((
                    tree.string(j, b"name").map_err(|e| e.to_string())?,
                    tree.string(j, b"type").map_err(|e| e.to_string())?,
                ));
            }
        }
        if declarations.len()
            != tree
                .integer(i, b"parameterCount")
                .map_err(|e| e.to_string())? as usize
            || groups.insert(id, declarations).is_some()
        {
            return Err("invalid/duplicate source shader-group definitions".into());
        }
    }
    let resource = bank.unique_texture();
    let mut result = BTreeMap::new();
    for shader in material::read_constants(tree).map_err(|e| e.to_string())? {
        let id = text(shader.shader)?;
        if !eligible(&id) {
            continue;
        }
        let declarations = groups
            .get(shader.group)
            .ok_or("source shader group unavailable")?;
        let matching: Vec<_> = declarations
            .iter()
            .filter(|(name, _)| *name == SAMPLER.as_bytes())
            .collect();
        let texture = match matching.as_slice() {
            [(_, kind)] if *kind == b"texture" => resource.clone(),
            [] => Err("source exact-case TEmissiveMap declaration missing".into()),
            [_] => Err("source TEmissiveMap is not declared texture".into()),
            _ => Err("duplicate source TEmissiveMap declaration".into()),
        };
        if result
            .insert(
                id,
                Binding {
                    group: text(shader.group)?,
                    texture,
                },
            )
            .is_some()
        {
            return Err("duplicate source material ID".into());
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    const NAMES: &[&str] = &[
        "ROOT",
        "SHADERGROUP",
        "SHADERINPUTDEFINITION",
        "SHADERINSTANCE",
        "SHADERINPUT",
    ];
    const ATTRS: &[&str] = &[
        "id",
        "name",
        "type",
        "shaderGroup",
        "parameterCount",
        "parameterSavedCount",
        "parameterID",
        "texture",
    ];
    fn u(n: u32) -> Vec<u8> {
        n.to_be_bytes().to_vec()
    }
    fn string(s: &str) -> Vec<u8> {
        let mut b = u(s.len() as u32);
        b.extend(s.as_bytes());
        b
    }
    fn node(name: &str, attrs: &[(&str, Vec<u8>)], payload: Vec<u8>) -> Vec<u8> {
        let mut block = vec![];
        for (name, value) in attrs {
            block.extend(u(ATTRS.iter().position(|n| n == name).unwrap() as u32 + 1));
            block.extend(u(value.len() as u32));
            block.extend(value);
        }
        let mut b = u(NAMES.iter().position(|n| *n == name).unwrap() as u32 + 1);
        b.extend(u((4 + block.len() + payload.len()) as u32));
        b.extend(u(block.len() as u32));
        b.extend(block);
        b.extend(payload);
        b
    }

    fn fixture(id: &str, declaration: Option<(&str, &str)>) -> Vec<u8> {
        let definition = declaration.map_or_else(Vec::new, |(name, kind)| {
            node(
                "SHADERINPUTDEFINITION",
                &[("name", string(name)), ("type", string(kind))],
                vec![],
            )
        });
        let group = node(
            "SHADERGROUP",
            &[
                ("id", string("any_group.fx")),
                ("parameterCount", u(u32::from(declaration.is_some()))),
            ],
            definition,
        );
        let shader = node(
            "SHADERINSTANCE",
            &[
                ("id", string(id)),
                ("shaderGroup", string("#any_group.fx")),
                ("parameterSavedCount", u(0)),
            ],
            vec![],
        );
        let mut b = b"PSSG".to_vec();
        b.extend(u(0));
        b.extend(u(ATTRS.len() as u32));
        b.extend(u(NAMES.len() as u32));
        for (i, name) in NAMES.iter().enumerate() {
            b.extend(u(i as u32 + 1));
            b.extend(string(name));
            b.extend(u(if i == 0 { ATTRS.len() as u32 } else { 0 }));
            if i == 0 {
                for (j, name) in ATTRS.iter().enumerate() {
                    b.extend(u(j as u32 + 1));
                    b.extend(string(name));
                }
            }
        }
        b.extend(node("ROOT", &[], [group, shader].concat()));
        let size = b.len() as u32 - 8;
        b[4..8].copy_from_slice(&size.to_be_bytes());
        b
    }
    fn package(id: &str, declaration: Option<(&str, &str)>, resources: &[&str]) -> Package {
        let bytes = fixture(id, declaration);
        let tree = pssg::read_tree(&bytes).unwrap();
        Package::read(
            &tree,
            "source_highLOD.pssg",
            Bank::from_ids(
                "source_gen_high.pssg",
                resources.iter().map(|s| (*s).into()).collect(),
            ),
        )
    }
    #[test]
    fn actual_parser_source_prefix_and_substring_are_not_exact_or_stem_guesses() {
        for name in [
            "Head Lights Pod",
            "head lights podSuffix",
            "Rear Lights Glass TAB",
        ] {
            let p = package(
                name,
                Some((SAMPLER, "texture")),
                &["unrelated_lights_on.custom.extra"],
            );
            assert_eq!(
                p.texture_key(name).unwrap(),
                Some("unrelated_lights_on.custom.extra".into())
            );
        }
        for name in ["Discs", "Grills", "STK Discs", " Head Lights Pod"] {
            let p = package(name, Some((SAMPLER, "texture")), &["stk_lights_on.tga"]);
            assert_eq!(p.texture_key(name).unwrap(), None);
        }
    }
    #[test]
    fn actual_parser_requires_exact_case_declared_texture_sampler_without_saved_input() {
        for declaration in [
            None,
            Some(("temissivemap", "texture")),
            Some((SAMPLER, "constant")),
        ] {
            let p = package("Head Lights Pod", declaration, &["model_lights_on.tga"]);
            assert!(p.texture_key("Head Lights Pod").is_err());
        }
        let p = package(
            "Head Lights Pod",
            Some((SAMPLER, "texture")),
            &["model_lights_on.tga"],
        );
        assert!(p.texture_key("Head Lights Pod").unwrap().is_some());
    }
    #[test]
    fn bank_missing_case_and_duplicate_resources_refuse_without_native_order_guess() {
        for resources in [
            vec![],
            vec!["model_LIGHTS_ON.tga"],
            vec!["first_lights_on.tga", "second_lights_on.tga"],
            vec!["same_lights_on.tga", "same_lights_on.tga"],
        ] {
            let p = package("Rear Lights Pod", Some((SAMPLER, "texture")), &resources);
            assert!(p.texture_key("Rear Lights Pod").is_err());
        }
    }
    #[test]
    fn append_namespace_never_changes_source_bank_or_raw_resource_and_high_low_stay_distinct() {
        let high = package(
            "Head Lights Glass",
            Some((SAMPLER, "texture")),
            &["source_lights_on.tga"],
        )
        .in_namespace("car3/");
        assert_eq!(
            high.texture_key("car3/Head Lights Glass").unwrap(),
            Some("car3/source_lights_on.tga".into())
        );
        assert_eq!(high.texture_key("car1/Head Lights Glass").unwrap(), None);
        assert_eq!(high.bank.source, "source_gen_high.pssg");
        assert_eq!(high.bank.ids.as_ref().unwrap(), &["source_lights_on.tga"]);
        let bytes = fixture("Head Lights Glass", Some((SAMPLER, "texture")));
        let tree = pssg::read_tree(&bytes).unwrap();
        let low = Package::read(
            &tree,
            "source_lowLOD.pssg",
            Bank::from_ids("source_gen_low.pssg", vec!["source_lights_on.tga".into()]),
        )
        .in_namespace("car3/");
        assert_eq!(
            low.texture_key("car3/Head Lights Glass").unwrap(),
            Some("car3/source_lights_on.tga".into())
        );
        assert_eq!(high.source, "source_highLOD.pssg");
        assert_eq!(low.source, "source_lowLOD.pssg");
        assert_ne!(high.bank.source, low.bank.source);
    }
    #[test]
    fn consumed_section_namespace_keeps_raw_identity_and_material_admission() {
        let section = crate::lamp_sections::Section {
            semantic: 108, palette: 9, role: crate::lamp_sections::Role::Front,
            cluster: 0, lod: 0, node: 3, bundle: "LOD0_".into(),
            draw: "actual_draw".into(), material: "Head Lights Glass".into(), joint: "x0_light_glass_fl".into(),
        };
        let p = package("Head Lights Glass", Some((SAMPLER, "texture")), &["source_lights_on.tga"])
            .with_sections(Ok(vec![section.clone()])).in_namespace("car3/");
        let found = p.section_key("actual_draw", "x0_light_glass_fl", "car3/Head Lights Glass").unwrap().unwrap();
        assert_eq!(found, &section);
        let receipt=p.receipt_key("actual_draw", "x0_light_glass_fl", "car3/Head Lights Glass").unwrap().unwrap();
        assert_eq!(receipt.bank.source,"source_gen_high.pssg");assert_eq!(receipt.bank.resource,"source_lights_on.tga");
        assert_eq!(receipt.geometry_source,"source_highLOD.pssg");assert_eq!(receipt.namespaced_texture,"car3/source_lights_on.tga");assert_eq!(receipt.section,section);
        let duplicate = package("Head Lights Glass", Some((SAMPLER,"texture")), &["source_lights_on.tga"])
            .with_sections(Ok(vec![section.clone(),section.clone()])).in_namespace("car3/");
        assert!(duplicate.section_key("actual_draw", "x0_light_glass_fl", "car3/Head Lights Glass").is_err());
        let unavailable = package("Head Lights Glass", Some((SAMPLER,"texture")), &["source_lights_on.tga"])
            .with_sections(Err("actual source metadata refusal".into()));
        assert!(unavailable.section_key("actual_draw", "x0_light_glass_fl", "Head Lights Glass").is_err());
        assert_eq!(p.section_key("actual_draw", "x0_light_glass_fl", "car2/Head Lights Glass").unwrap(), None);
        assert_eq!(p.section_key("actual_draw", "x0_light_glass_fl\t", "car3/Head Lights Glass").unwrap(), None);
        assert_eq!(p.section_key("different_draw", "x0_light_glass_fl", "car3/Head Lights Glass").unwrap(), None);
        let absent = package("Head Lights Glass", None, &["source_lights_on.tga"]).with_sections(Ok(vec![section]));
        assert!(absent.section_key("actual_draw", "x0_light_glass_fl", "Head Lights Glass").is_err());
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_packages_declare_and_uniquely_resolve_source_filtered_materials() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let root = Path::new(&root);
        let mut packages = 0;
        let mut resolved = 0;
        for entry in std::fs::read_dir(root.join("cars")).unwrap() {
            let entry = entry.unwrap();
            let stem = entry.file_name();
            let Some(stem) = stem.to_str() else { continue };
            let Ok(model) = grid_tools::car::Car::new(stem) else {
                continue;
            };
            for (detail, bankdetail) in [
                ("_highLOD.pssg", "_gen_high.pssg"),
                ("_lowLOD.pssg", "_gen_low.pssg"),
            ] {
                let source = model.file(detail);
                if !root.join(&source).is_file() {
                    continue;
                }
                let bytes = crate::read(&root.join(&source)).unwrap();
                let tree = pssg::read_tree(&bytes).unwrap();
                let p = Package::read(&tree, &source, Bank::load(root, &model.file(bankdetail)));
                assert!(p.bank.ids.is_ok(), "{} {:?}", p.bank.source, p.bank.ids);
                let materials = p.materials.as_ref().unwrap();
                for (id, binding) in materials {
                    assert!(
                        binding.texture.is_ok(),
                        "{source} {id}: {:?}",
                        binding.texture
                    );
                    assert!(p.texture_key(id).unwrap().is_some());
                    resolved += 1;
                }
                assert_eq!(p.texture_key("Discs").unwrap(), None);
                assert_eq!(p.texture_key("Grills").unwrap(), None);
                packages += 1;
            }
        }
        assert!(packages > 0 && resolved > 0, "purchased fixture empty");
        println!(
            "lamp_association_purchased packages={packages} resolved_materials={resolved} policy=unique-source-table-no-emission"
        );
    }
}
