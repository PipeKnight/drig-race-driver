//! Wheel kick-up data: `effects/particleMaterials.xml` `<kickup><dry>` records
//! (plain XML) and the emitters/births of `effects/particle_systems/*.xml`
//! (binary XML). Only the dry block is read, like the original parser 5BBDC0.
//! See docs/GROUND_EFFECTS_RESEARCH.md for field meanings.
use crate::{Error, Result, binxml, fail};
use quick_xml::{Reader, events::Event};

#[derive(Clone, Debug, PartialEq)]
pub struct Kickup {
    /// PARTICLE material name, e.g. `GRASS`.
    pub material: String,
    pub effect: String,
    pub ai_effect: Option<String>,
    /// Speed-band multipliers high, medium, low, static.
    pub multipliers: [f32; 4],
    pub rate: f32,
    pub speed: f32,
    pub tyreslip: f32,
}

fn number(value: &str, at: usize) -> Result<f32> {
    match value.trim().parse::<f32>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => fail(at, "invalid kick-up number"),
    }
}

/// Every PARTICLE material that has a `<kickup><dry><effect>`.
pub fn read_materials(text: &str) -> Result<Vec<Kickup>> {
    if text.len() > 4 << 20 {
        return fail(0, "particle materials exceed bound");
    }
    let mut reader = Reader::from_str(text);
    let mut path: Vec<Vec<u8>> = Vec::new();
    let mut material: Option<String> = None;
    let mut current: Option<Kickup> = None;
    let mut out = Vec::new();
    loop {
        let at = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(|_| Error {
            offset: at,
            message: "malformed particle materials XML",
        })?;
        let (element, empty) = match &event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(_) => {
                if path.pop().as_deref() == Some(b"PARTICLE")
                    && let Some(k) = current.take()
                {
                    out.push(k);
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let name = element.name().as_ref().to_vec();
        let mut attributes = Vec::new();
        for attribute in element.attributes() {
            let attribute = attribute.map_err(|_| Error {
                offset: at,
                message: "malformed particle materials attribute",
            })?;
            let value = std::str::from_utf8(&attribute.value).map_err(|_| Error {
                offset: at,
                message: "non-UTF-8 particle materials attribute",
            })?;
            attributes.push((attribute.key.as_ref().to_vec(), value.to_owned()));
        }
        let get = |key: &str| {
            attributes
                .iter()
                .find(|(k, _)| k == key.as_bytes())
                .map(|(_, v)| v.as_str())
        };
        let within = |parents: &[&[u8]]| {
            path.len() >= parents.len()
                && path[path.len() - parents.len()..]
                    .iter()
                    .zip(parents)
                    .all(|(a, b)| a == b)
        };
        match name.as_slice() {
            b"PARTICLE" => {
                material = get("name").map(str::to_owned);
            }
            b"effect" if within(&[b"kickup", b"dry"]) => {
                let effect = get("name").unwrap_or("none").to_owned();
                current = Some(Kickup {
                    material: material.clone().ok_or(Error {
                        offset: at,
                        message: "kick-up outside a named PARTICLE",
                    })?,
                    effect,
                    ai_effect: get("AIName").map(str::to_owned),
                    multipliers: [0.; 4],
                    rate: 1.,
                    speed: 1.,
                    tyreslip: 0.,
                });
            }
            b"multipliers" if within(&[b"kickup", b"dry"]) => {
                if let Some(k) = &mut current {
                    for (slot, key) in k
                        .multipliers
                        .iter_mut()
                        .zip(["high", "medium", "low", "static"])
                    {
                        if let Some(v) = get(key) {
                            *slot = number(v, at)?;
                        }
                    }
                    if let Some(v) = get("rate") {
                        k.rate = number(v, at)?;
                    }
                    if let Some(v) = get("speed") {
                        k.speed = number(v, at)?;
                    }
                }
            }
            b"special" if within(&[b"kickup", b"dry"]) => {
                if let Some(k) = &mut current
                    && let Some(v) = get("tyreslip")
                {
                    k.tyreslip = number(v, at)?;
                }
            }
            _ => {}
        }
        if !empty {
            path.push(name);
        }
        if out.len() > 4096 {
            return fail(at, "too many kick-up materials");
        }
    }
    Ok(out)
}

/// One decoded emitter of a particle system file.
#[derive(Clone, Debug, PartialEq)]
pub struct Emitter {
    pub id: u32,
    pub name: String,
    pub particle_type: String,
    pub texture: String,
    /// Atlas frames, grid and normalised selection weights (`frequencies`).
    pub frames: u32,
    pub columns: u32,
    pub rows: u32,
    pub weights: Vec<f32>,
    pub collision: bool,
    pub bounce: f32,
    /// Gravity and drag `influence` in percent; drag `rangeX`.
    pub gravity: f32,
    pub drag: f32,
    pub drag_range: f32,
    pub rate: f32,
    pub life: f32,
    /// Life variance in percent.
    pub life_variance: f32,
    pub speed: f32,
    pub speed_variation: f32,
    /// Cone half-angle in degrees.
    pub divergence: f32,
    pub inherit: f32,
    /// `agescale` initial and final width (defaults 1, emitter defaults 875620).
    pub width: [f32; 2],
    /// `agescale` scaleType as the original's code (parser 8A49A0): 0 absent,
    /// 1 linear, 2 cubicAccelerate, 3 cubicDecelerate, 4 quadraticAccelerate,
    /// 5 quadraticDecelerate.
    pub scale_type: u8,
    /// Width variance in percent (default 1); used only without a scale type.
    pub scale_variance: f32,
    /// `scale` growthPercentage (default 1); used only without a scale type.
    pub growth: f32,
    pub alpha: [f32; 3],
    pub in_time: f32,
    pub out_time: f32,
    pub colour: [[f32; 3]; 2],
    pub initial_rotation: f32,
    pub spin_variance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Birth {
    pub emitter: u32,
    pub position: [f32; 3],
    /// Degrees.
    pub orientation: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct System {
    pub name: String,
    pub births: Vec<Birth>,
}

fn attr<'a>(e: &binxml::Element<'a>, key: &str) -> Option<&'a str> {
    e.attributes
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}
fn float(e: &binxml::Element<'_>, key: &str, default: f32) -> Result<f32> {
    attr(e, key).map_or(Ok(default), |v| number(v, 0))
}
fn floats<const N: usize>(
    e: &binxml::Element<'_>,
    key: &str,
    default: [f32; N],
) -> Result<[f32; N]> {
    let Some(text) = attr(e, key) else {
        return Ok(default);
    };
    let values: Vec<f32> = text
        .split_whitespace()
        .map(|v| number(v, 0))
        .collect::<Result<_>>()?;
    values.try_into().map_err(|_| Error {
        offset: 0,
        message: "wrong particle vector length",
    })
}

/// Emitters and systems of one particle system document.
pub fn read_systems(doc: &binxml::Document<'_>) -> Result<(Vec<Emitter>, Vec<System>)> {
    let children = |i: usize| {
        doc.elements[i]
            .children
            .clone()
            .map(|c| (c, &doc.elements[c]))
    };
    let mut emitters = Vec::new();
    let mut systems = Vec::new();
    for (i, e) in doc.elements.iter().enumerate() {
        match e.name {
            "emitter" => {
                let mut emitter = Emitter {
                    id: attr(e, "id").and_then(|v| v.parse().ok()).ok_or(Error {
                        offset: 0,
                        message: "emitter without id",
                    })?,
                    name: attr(e, "name").unwrap_or("").to_owned(),
                    particle_type: String::new(),
                    texture: String::new(),
                    frames: 1,
                    columns: 1,
                    rows: 1,
                    weights: vec![1.],
                    collision: false,
                    bounce: 0.,
                    gravity: 0.,
                    drag: 0.,
                    drag_range: 0.,
                    rate: 0.,
                    life: 1.,
                    life_variance: 0.,
                    speed: 0.,
                    speed_variation: 0.,
                    divergence: 0.,
                    inherit: 0.,
                    width: [1., 1.],
                    scale_type: 0,
                    scale_variance: 1.,
                    growth: 1.,
                    alpha: [1., 1., 1.],
                    in_time: 0.,
                    out_time: 0.,
                    colour: [[1.; 3]; 2],
                    initial_rotation: 0.,
                    spin_variance: 0.,
                };
                let mut pending = vec![i];
                while let Some(j) = pending.pop() {
                    for (k, c) in children(j) {
                        pending.push(k);
                        match c.name {
                            "model" => {
                                emitter.particle_type =
                                    attr(c, "particleType").unwrap_or("").to_owned();
                            }
                            "material" => {
                                if let Some(t) = attr(c, "textureName") {
                                    emitter.texture = t.to_owned();
                                }
                            }
                            "materialtexture" => {
                                emitter.texture = attr(c, "filename").unwrap_or("").to_owned();
                                emitter.frames = float(c, "numFrames", 1.)? as u32;
                                emitter.columns = float(c, "widthInBlocks", 1.)? as u32;
                                emitter.rows = float(c, "heightInBlocks", 1.)? as u32;
                            }
                            "properties" => {
                                if let Some(text) = attr(c, "frequencies") {
                                    emitter.weights = text
                                        .split_whitespace()
                                        .map(|v| number(v, 0))
                                        .collect::<Result<_>>()?;
                                }
                            }
                            "physics" => {
                                emitter.collision = attr(c, "collisiondetection") == Some("true");
                                emitter.bounce = float(c, "bounce", 0.)?;
                            }
                            "force" => match attr(c, "type") {
                                Some("gravity") => emitter.gravity = float(c, "influence", 0.)?,
                                Some("drag") => {
                                    emitter.drag = float(c, "influence", 0.)?;
                                    emitter.drag_range = float(c, "rangeX", 0.)?;
                                }
                                _ => {}
                            },
                            "spawn" => emitter.rate = float(c, "rate", 0.)?,
                            "life" => {
                                emitter.life = float(c, "life", 1.)?;
                                emitter.life_variance = float(c, "variance", 0.)?;
                            }
                            "speed" => {
                                emitter.speed = float(c, "speed", 0.)?;
                                emitter.speed_variation = float(c, "variation", 0.)?;
                                emitter.divergence = float(c, "divergence", 0.)?;
                                if attr(c, "inheritemitter") == Some("true") {
                                    emitter.inherit = float(c, "inheritamount", 0.)?;
                                }
                            }
                            "spin" => {
                                emitter.initial_rotation = float(c, "initialRotation", 0.)?;
                                emitter.spin_variance = float(c, "variance", 0.)?;
                            }
                            "agescale" => {
                                emitter.width =
                                    [float(c, "initialWidth", 1.)?, float(c, "finalWidth", 1.)?];
                                emitter.scale_variance = float(c, "scaleVariance", 1.)?;
                                emitter.scale_type = match attr(c, "scaleType") {
                                    Some("linear") => 1,
                                    Some("cubicAccelerate") => 2,
                                    Some("cubicDecelerate") => 3,
                                    Some("quadraticAccelerate") => 4,
                                    Some("quadraticDecelerate") => 5,
                                    _ => 0,
                                };
                            }
                            "scale" => emitter.growth = float(c, "growthPercentage", 1.)?,
                            "alpha" => {
                                emitter.alpha = [
                                    float(c, "start", 1.)?,
                                    float(c, "middle", 1.)?,
                                    float(c, "end", 1.)?,
                                ];
                                emitter.in_time = float(c, "inTime", 0.)?;
                                emitter.out_time = float(c, "outTime", 0.)?;
                            }
                            "colour" => {
                                emitter.colour =
                                    [floats(c, "start", [1.; 3])?, floats(c, "end", [1.; 3])?];
                            }
                            _ => {}
                        }
                    }
                }
                let total: f32 = emitter.weights.iter().sum();
                if total.is_nan() || total <= 0. {
                    return fail(0, "emitter frame weights sum to zero");
                }
                emitter.weights.iter_mut().for_each(|w| *w /= total);
                emitters.push(emitter);
            }
            "System" => {
                let mut births = Vec::new();
                let mut pending = vec![i];
                while let Some(j) = pending.pop() {
                    for (k, c) in children(j) {
                        pending.push(k);
                        if c.name == "birth" {
                            births.push(Birth {
                                emitter: float(c, "emitterID", 0.)? as u32,
                                position: floats(c, "relPosition", [0.; 3])?,
                                orientation: floats(c, "relOrientation", [0.; 3])?,
                            });
                        }
                    }
                }
                births.sort_by_key(|b| b.emitter);
                systems.push(System {
                    name: attr(e, "name").unwrap_or("").to_owned(),
                    births,
                });
            }
            _ => {}
        }
    }
    Ok((emitters, systems))
}

/// Surface code -> PARTICLE material name from `surface_materials.xml`.
pub fn surface_particles(doc: &binxml::Document<'_>) -> Vec<([u8; 4], String)> {
    let mut out = Vec::new();
    for e in doc.elements.iter().filter(|e| e.name == "MATERIAL") {
        let Some(code) = attr(e, "name").and_then(|n| <[u8; 4]>::try_from(n.as_bytes()).ok())
        else {
            continue;
        };
        if let Some(particle) = e
            .children
            .clone()
            .map(|c| &doc.elements[c])
            .find(|c| c.name == "PARTICLE")
            .and_then(|c| attr(c, "name"))
        {
            out.push((code, particle.to_owned()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_dry_kickup_only() {
        let k = read_materials(
            r#"<?xml version="1.0"?><PARTICLEMATERIALS>
<PARTICLE name="CAR_" effectName="x"><effects high="a"/></PARTICLE>
<PARTICLE name="GRASS" effectName="None"><kickup><dry>
<effect name="K_Grass" AIName="K_AI_Grass" />
<multipliers high="0.5" medium="1.0" low="0.5" static="0.0" rate="1.0" speed="1.0" />
<special tyreslip="1.0" downforce="2.0" />
</dry><wet><effect name="K_Wet"/></wet></kickup></PARTICLE>
<PARTICLE name="SAND"><kickup><dry><effect name="K_Sand"/></dry></kickup></PARTICLE>
</PARTICLEMATERIALS>"#,
        )
        .unwrap();
        assert_eq!(k.len(), 2);
        assert_eq!(k[0].material, "GRASS");
        assert_eq!(k[0].ai_effect.as_deref(), Some("K_AI_Grass"));
        assert_eq!(k[0].multipliers, [0.5, 1.0, 0.5, 0.0]);
        assert_eq!(k[0].tyreslip, 1.0);
        assert_eq!(k[1].effect, "K_Sand");
        assert!(k[1].ai_effect.is_none());
        assert!(read_materials("<PARTICLE><kickup><dry><effect name=\"x\"/>").is_err());
    }
}
