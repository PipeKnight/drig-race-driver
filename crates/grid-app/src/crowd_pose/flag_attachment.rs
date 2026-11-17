//! Purchased CPU acceptance for the three original rigid marshal props.
use super::{IDENTITY, PoseChannel, PosePolicy, Source, node_world};
use crate::{
    crowd_population::ResourceRef,
    crowd_preview::unique_texture,
    crowd_selected_clip::{BoundClip, ExplicitClipBinding, Selected},
};
use bevy::prelude::Vec3;
use grid_formats::{binxml, material, mesh, pssg, scene, texture};
use grid_tools::crowd_selection::{Candidate, Library};

type Matrix = [f32; 16];
const PIVOT: &[u8] = b"RH_Prop_Helper";

fn attr<'a>(element: &binxml::Element<'a>, name: &str) -> &'a str {
    let matches: Vec<_> = element
        .attributes
        .iter()
        .filter(|(key, _)| *key == name)
        .collect();
    let [one] = matches.as_slice() else {
        panic!("expected one {name} attribute")
    };
    one.1
}

fn unique_node(tree: &pssg::Tree<'_>, name: &[u8], id: &[u8]) -> usize {
    let nodes: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == name && tree.string(i, b"id").ok() == Some(id))
        .collect();
    let [node] = nodes.as_slice() else {
        panic!("expected unique source node {id:?}")
    };
    *node
}

fn point(matrix: &Matrix) -> Vec3 {
    Vec3::from_array(scene::transform_point(matrix, [0.; 3]))
}

#[test]
#[ignore = "requires purchased absolute GRID_GAME_ROOT, CPU only"]
fn purchased_rigid_marshal_rods_follow_the_same_animated_hand_hierarchy() {
    let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
    assert!(root.is_absolute());
    let read = |file| std::fs::read(root.join("anims").join(file)).unwrap();
    let props_bytes = read("props.pssg");
    let props = pssg::read_tree(&props_bytes).unwrap();
    let catalogue = mesh::read_static_meshes(&props).unwrap();
    let draws = scene::Renderables::new(&props, &catalogue).unwrap();
    let bindings = material::read_texture_bindings(&props).unwrap();
    let clip_resource = "anims/crowd_anims.pssg";
    let bank_bytes = std::fs::read(root.join(clip_resource)).unwrap();
    let bank = pssg::read_tree(&bank_bytes).unwrap();
    let hlaxml = read("crowd_hla.xml");
    let hla = binxml::read(&hlaxml).unwrap();
    let library = Library::parse(&hla).unwrap();
    let xml = read("crowd_props.xml");
    let props_definition = binxml::read(&xml).unwrap();
    let clothprops = props_definition.elements[props_definition.elements[0].children.clone()]
        .iter()
        .find(|e| e.name == "clothprops")
        .unwrap();
    let skin_bytes = read("marshall.pssg");
    let skin_tree = pssg::read_tree(&skin_bytes).unwrap();
    let skin_id = b"mar_LOD0";
    let skin_catalogue = mesh::read_object_meshes(&skin_tree).unwrap();
    let skin = scene::read_skin(&skin_tree, &skin_catalogue, skin_id).unwrap();
    let source = skin
        .instances
        .iter()
        .filter_map(|&(id, _, _)| Source::read(&skin_tree, skin_id, id).ok())
        .max_by_key(|rig| rig.indices.len())
        .unwrap();
    let helper = source
        .nodes
        .iter()
        .position(|node| node.name == PIVOT)
        .unwrap();
    assert_eq!(
        source.nodes[source.nodes[helper].parent.unwrap()]
            .name
            .as_slice(),
        b"Bip_01 R Hand"
    );
    let prior = source.saved_locals();

    let prop_pivot = unique_node(&props, b"NODE", PIVOT);
    assert_eq!(scene::node_matrix(&props, prop_pivot).unwrap(), IDENTITY);
    let roles: [(&str, &[&str]); 3] = [
        (
            "start",
            &[
                "mar_sta_tran_to_act_01",
                "mar_sta_act_01",
                "mar_sta_act_02",
                "mar_sta_tran_fr_act_01",
            ],
        ),
        (
            "finish",
            &[
                "mar_fin_tran_to_act_01",
                "mar_fin_act_01",
                "mar_fin_act_02",
                "mar_fin_tran_fr_act_01",
            ],
        ),
        (
            "danger",
            &[
                "mar_dan_tran_to_act_01",
                "mar_dan_act_01",
                "mar_dan_tran_fr_act_01",
            ],
        ),
    ];
    for (role, clip_ids) in roles {
        let name = format!("{role}flag");
        let prop = props_definition.elements[clothprops.children.clone()]
            .iter()
            .find(|e| e.name == "prop" && attr(e, "nodename") == name)
            .unwrap();
        assert_eq!(attr(prop, "pivotnode").as_bytes(), PIVOT);
        let parent = unique_node(&props, b"NODE", name.as_bytes());
        let source_draws = draws.draws(name.as_bytes()).unwrap();
        let [rod] = source_draws.as_slice() else {
            panic!("expected one rigid draw for {name}")
        };
        let stick = format!("{role}_flag_stick");
        assert_eq!(rod.joint, stick.as_bytes());
        assert_eq!(rod.material, b"flag");
        assert_eq!(rod.indices.len(), 36);
        let stick = unique_node(&props, b"RENDERNODE", rod.joint);
        assert_eq!(rod.transform, scene::node_matrix(&props, stick).unwrap());
        let geometry = &catalogue[rod.source];
        assert_eq!(geometry.positions.len(), 18);
        assert_eq!(geometry.normals.len(), 18);
        assert_eq!(geometry.texcoords.len(), 18);
        assert!(geometry.skin_weights.is_none());

        let texture_id = bindings
            .iter()
            .find(|b| b.shader == rod.material)
            .unwrap()
            .texture;
        assert_eq!(texture_id, b"flag_uk_d.tga");
        let source_textures = texture::read_named(&props, &[texture_id]).unwrap();
        let dds = unique_texture(&source_textures, texture_id)
            .unwrap()
            .validated_dds();
        assert!(!dds.0.is_empty());

        let cloth = unique_node(&props, b"RENDERNODE", format!("{name}_sim").as_bytes());
        assert_eq!(props.nodes[cloth].parent, props.nodes[parent].parent);
        let pins = [("attachpoint1", "7"), ("attachpoint2", "47")].map(|(key, vertex)| {
            let definition = props_definition.elements[prop.children.clone()]
                .iter()
                .find(|e| e.name == key)
                .unwrap();
            assert_eq!(attr(definition, "vertex"), vertex);
            let pin = unique_node(&props, b"NODE", attr(definition, "node").as_bytes());
            assert_eq!(props.nodes[pin].parent, Some(parent));
            scene::node_matrix(&props, pin).unwrap()
        });
        let pin_span = point(&pins[0]).distance(point(&pins[1]));
        assert!((pin_span - 0.351_837_3).abs() < 1e-6);

        for &clip_id in clip_ids {
            let raw_ids: Vec<_> = library
                .raw
                .iter()
                .filter(|raw| raw.name == clip_id)
                .map(|raw| raw.ordinal)
                .collect();
            let &[ordinal] = raw_ids.as_slice() else {
                panic!("unexpected HLA membership for {clip_id}")
            };
            let selected = Selected {
                candidate: Candidate {
                    raw_name: clip_id,
                    raw_ordinals: raw_ids,
                    mirrored: false,
                    used_default: false,
                },
                raw_ordinal: ordinal,
                animation_bank: ResourceRef {
                    userdata: "hla".to_owned(),
                    relative_path: clip_resource.to_owned(),
                },
            };
            let binding = ExplicitClipBinding {
                raw_ordinal: ordinal,
                clip_id: clip_id.as_bytes(),
            };
            let clip = BoundClip::read(&selected, binding, &bank).unwrap();
            let (first, last) = clip.key_range().unwrap();
            let positions = [first, (first + last) * 0.5, last].map(|tick| {
                let samples = clip.sample(tick).unwrap();
                let channels: Vec<_> = samples
                    .iter()
                    .map(|sample| PoseChannel {
                        target: sample.target.as_bytes(),
                        kind: sample.kind,
                        raw: sample.raw,
                    })
                    .collect();
                let locals = source
                    .sample_locals(
                        &prior,
                        &channels,
                        PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: false },
                    )
                    .unwrap();
                let mut nodes = source.nodes.clone();
                for (node, local) in nodes.iter_mut().zip(locals) {
                    node.local = local;
                }
                let hand = node_world(&nodes, helper).unwrap();
                let rod_in_actor = scene::compose(&rod.transform, &hand);
                let vertex =
                    Vec3::from_array(scene::transform_point(&rod_in_actor, geometry.positions[0]));
                let [a, b] = pins.map(|pin| point(&scene::compose(&pin, &hand)));
                assert!((a.distance(b) - pin_span).abs() < 2e-6);
                assert!(vertex.is_finite());
                [a, b, vertex]
            });
            assert!(
                positions[0]
                    .iter()
                    .zip(positions[1])
                    .any(|(a, b)| a.distance(b) > 0.01)
                    || positions[1]
                        .iter()
                        .zip(positions[2])
                        .any(|(a, b)| a.distance(b) > 0.01),
                "clip {clip_id} never moves the authored attachment"
            );
        }
    }
}
