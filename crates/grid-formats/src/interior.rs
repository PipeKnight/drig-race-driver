//! Shipped `interiors/<stem>/int_<stem>.xml` (plain XML, parsed by 0xA13AC0):
//! the exterior nodes the cockpit view keeps drawing (`active_nodes`), the
//! dash displays and the backlight shaders; and the simple fonts
//! (`simplefonts/<name>_font.xml`) the digital displays draw with.
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

/// Every element of a small XML file with its parent path and attributes,
/// in file order.
type Element = (Vec<String>, String, Vec<(String, String)>);

fn elements(text: &str, what: &'static str) -> Result<Vec<Element>> {
    if text.len() > 65536 {
        return fail(0, "XML exceeds bound");
    }
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<String> = Vec::new();
    let mut out = Vec::new();
    loop {
        let at = reader.buffer_position() as usize;
        let malformed = |_| Error { offset: at, message: what };
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
        let name = String::from_utf8_lossy(element.name().as_ref()).into_owned();
        let mut attributes = Vec::new();
        for attr in element.attributes() {
            let attr = attr.map_err(|_| Error { offset: at, message: what })?;
            let value = attr.normalized_value(quick_xml::XmlVersion::Implicit1_0).map_err(malformed)?;
            attributes.push((String::from_utf8_lossy(attr.key.as_ref()).into_owned(), value.into_owned()));
        }
        out.push((stack.clone(), name.clone(), attributes));
        if !empty {
            stack.push(name);
        }
    }
    if !stack.is_empty() {
        return fail(text.len(), what);
    }
    Ok(out)
}

/// One `interior_config/displays/display` (parser 0xA13AC0): the block's
/// `type` and the display's attributes as written (absent ones `None`; the
/// game's defaults differ per type, docs/COCKPIT.md "Gauges").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Display {
    /// analog, analog_dummy, digital, indicator_light or motec.
    pub kind: String,
    /// tach, speed, gear, drift_angle, drift_peak or boost.
    pub value: String,
    pub node: String,
    pub font: Option<String>,
    pub num_chars: Option<f32>,
    pub factor: Option<f32>,
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,
    pub long_way: Option<f32>,
    pub min_alpha: Option<f32>,
    pub max_alpha: Option<f32>,
    pub cond_min: Option<f32>,
    pub cond_max: Option<f32>,
    pub shift: Option<f32>,
    pub deviation: Option<f32>,
    pub speed: Option<f32>,
}

/// The displays, in file order.
pub fn read_displays(text: &str) -> Result<Vec<Display>> {
    let mut out = Vec::new();
    let mut kind = String::new();
    for (path, name, attributes) in elements(text, "malformed interior XML")? {
        let get = |key: &str| attributes.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
        // atof/atol: a value that does not parse reads as 0.
        let number = |key: &str| get(key).map(|v| v.trim().parse::<f32>().unwrap_or(0.));
        if path == ["interior_config"] && name == "displays" {
            kind = get("type").unwrap_or_default();
        }
        if path == ["interior_config", "displays"] && name == "display" {
            out.push(Display {
                kind: kind.clone(),
                value: get("value").unwrap_or_default(),
                node: get("node").unwrap_or_default(),
                font: get("font"),
                num_chars: number("num_chars"),
                factor: number("factor"),
                min_value: number("min_value"),
                max_value: number("max_value"),
                long_way: number("long_way"),
                min_alpha: number("min_alpha"),
                max_alpha: number("max_alpha"),
                cond_min: number("cond_min"),
                cond_max: number("cond_max"),
                shift: number("shift"),
                deviation: number("deviation"),
                speed: number("speed"),
            });
        }
    }
    Ok(out)
}

/// `backlight_shaders/shader` names and the scale the backlight gives them:
/// 0.3 whenever `backlight_parameters` has an `emissive_scale` attribute,
/// whatever its text (0xA13D99), else the constructor's 0.15.
pub fn read_backlight(text: &str) -> Result<(Vec<String>, f32)> {
    let mut shaders = Vec::new();
    let mut scale = 0.15;
    for (path, name, attributes) in elements(text, "malformed interior XML")? {
        if path == ["interior_config", "backlight_shaders"]
            && name == "shader"
            && let Some((_, v)) = attributes.iter().find(|(k, _)| k == "name")
        {
            shaders.push(v.clone());
        }
        if path == ["interior_config"] && name == "backlight_parameters" && attributes.iter().any(|(k, _)| k == "emissive_scale") {
            scale = 0.3;
        }
    }
    Ok((shaders, scale))
}

/// `simplefonts/<name>_font.xml`: a strip of glyphs (0x99DE70).
#[derive(Clone, Debug, PartialEq)]
pub struct SimpleFont {
    pub name: String,
    pub texture: String,
    pub glyphs: u32,
    pub char_size: [u32; 2],
    /// The character each glyph stands for, in strip order.
    pub translate: String,
}

pub fn read_simple_font(text: &str) -> Result<SimpleFont> {
    for (path, name, attributes) in elements(text, "malformed simple font XML")? {
        if path == ["simple_font"] && name == "font" {
            let get = |key: &str| attributes.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default();
            let int = |key: &str| get(key).trim().parse::<u32>().unwrap_or(0);
            let font = SimpleFont {
                name: get("name"),
                texture: get("texture"),
                glyphs: int("num_chars"),
                char_size: [int("char_size_x"), int("char_size_y")],
                translate: get("translate"),
            };
            if font.glyphs == 0 || font.glyphs > 256 || font.char_size.iter().any(|&s| s == 0 || s > 256) {
                return fail(0, "simple font sizes out of range");
            }
            return Ok(font);
        }
    }
    fail(0, "simple font without a font element")
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

    #[test]
    fn displays_backlight_and_fonts_are_read() {
        let text = r#"<?xml version="1.0"?>
<interior_config>
    <backlight_shaders>
        <shader name="dials" />
    </backlight_shaders>
    <backlight_parameters emissive_scale="1.5"/>
    <displays type="digital">
        <display value="speed" node="int_digi_speed" font="int_digi_speed" num_chars="3" factor="3.6"/>
    </displays>
    <displays type="indicator_light">
        <display value="tach" node="int_indicator_limit" factor="0.00955" cond_min="7.7" cond_max="x"/>
    </displays>
</interior_config>"#;
        let d = read_displays(text).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].kind.as_str(), d[0].value.as_str(), d[0].node.as_str()), ("digital", "speed", "int_digi_speed"));
        assert_eq!((d[0].font.as_deref(), d[0].num_chars, d[0].factor), (Some("int_digi_speed"), Some(3.), Some(3.6)));
        assert_eq!((d[1].kind.as_str(), d[1].cond_min, d[1].cond_max, d[1].shift), ("indicator_light", Some(7.7), Some(0.), None));
        // Any emissive_scale text gives 0.3.
        assert_eq!(read_backlight(text).unwrap(), (vec!["dials".to_string()], 0.3));
        assert_eq!(read_backlight("<interior_config/>").unwrap(), (vec![], 0.15));
        let font = read_simple_font(r#"<simple_font><font name="int_gear" texture="int_gear.dds" num_chars="10" char_size_x="32" char_size_y="32" translate="0123456789"/></simple_font>"#).unwrap();
        assert_eq!((font.glyphs, font.char_size, font.texture.as_str()), (10, [32, 32], "int_gear.dds"));
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
