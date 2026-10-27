//! Shipped `interiors/<stem>/int_<stem>.xml` (plain XML, parsed by 0xA13AC0):
//! the exterior nodes the cockpit view keeps drawing (`active_nodes`). The
//! backlight shaders and displays are not read yet.
use crate::{Error, Result, fail};
use quick_xml::{Reader, events::Event};

/// The `name`s of `interior_config/active_nodes/node`, in file order.
pub fn read_active_nodes(text: &str) -> Result<Vec<String>> {
    if text.len() > 65536 {
        return fail(0, "interior XML exceeds bound");
    }
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut nodes = Vec::new();
    loop {
        let at = reader.buffer_position() as usize;
        let malformed = |_| Error {
            offset: at,
            message: "malformed interior XML",
        };
        let (element, empty) = match reader.read_event().map_err(malformed)? {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(_) => {
                stack.pop();
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let name = element.name().as_ref().to_vec();
        if name == b"node" && stack.len() == 2 && stack[0] == b"interior_config" && stack[1] == b"active_nodes" {
            let mut value = None;
            for attr in element.attributes() {
                let attr = attr.map_err(|_| Error {
                    offset: at,
                    message: "malformed interior XML attribute",
                })?;
                if attr.key.as_ref() == b"name" {
                    let v = attr.normalized_value(quick_xml::XmlVersion::Implicit1_0).map_err(malformed)?;
                    value = Some(v.into_owned());
                }
            }
            let Some(value) = value else {
                return fail(at, "interior active node without a name");
            };
            nodes.push(value);
        }
        if !empty {
            stack.push(name);
        }
    }
    if !stack.is_empty() {
        return fail(text.len(), "unclosed interior XML element");
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_nodes_are_read_in_order() {
        let text = r#"<?xml version="1.0"?>
<interior_config>
    <active_nodes>
        <node name="x0_bonnet"/>
        <node name="x0_door_fl"/>
    </active_nodes>
    <displays type="digital">
        <display value="gear" node="int_indicator_gear"/>
    </displays>
    <node name="outside"/>
</interior_config>"#;
        assert_eq!(read_active_nodes(text).unwrap(), ["x0_bonnet", "x0_door_fl"]);
        assert!(read_active_nodes("<interior_config><active_nodes><node/></active_nodes></interior_config>").is_err());
        assert!(read_active_nodes("<interior_config><active_nodes>").is_err());
    }

    /// The shipped cockpits read as static scenes (half4 positions in the
    /// displays).
    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn shipped_cockpits_read_as_static_scenes() {
        let root = std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT");
        for stem in ["lac", "bmw", "n12"] {
            let path = std::path::Path::new(&root).join(format!("interiors/{stem}/int_{stem}.pssg"));
            let bytes = std::fs::read(path).unwrap();
            let tree = crate::pssg::read_tree(&bytes).unwrap();
            let sources = crate::mesh::read_static_meshes(&tree).unwrap();
            let draws = crate::scene::read_static_draws(&tree, &sources).unwrap();
            assert!(draws.len() > 20, "{stem} {}", draws.len());
        }
    }

    /// Both shipped touring cars keep the same nine exterior nodes.
    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn shipped_interiors_keep_nine_exterior_nodes() {
        let root = std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT");
        for stem in ["lac", "bmw"] {
            let path = std::path::Path::new(&root).join(format!("interiors/{stem}/int_{stem}.xml"));
            let text = std::fs::read_to_string(path).unwrap();
            assert_eq!(
                read_active_nodes(&text).unwrap(),
                [
                    "x0_bonnet", "x0_door_fl", "x0_door_fr", "x0_wing_fl", "x0_wing_fr",
                    "x0_wing_mirror_l", "x0_wing_mirror_r", "x0_engine", "x0_under_f",
                ],
                "{stem}"
            );
        }
    }
}
