//! Track start-light sequence (`tracks/<region>/<track>/start_lights.xml`):
//! `startLightSequence` > `lights object=` and `sequence numStages= time=`
//! (siblings) > `stage id= texture=` in id order. How the game times the
//! stages is separate.
use crate::{
    Result,
    binxml::{Document, Element},
    fail,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    /// Scene object that carries the board (`lights object=`).
    pub object: String,
    /// Shipped sequence length in seconds (`sequence time=`).
    pub seconds: f32,
    /// Emissive texture of stages 1..=numStages.
    pub stages: Vec<String>,
}

fn attr<'a>(e: &Element<'a>, name: &str) -> Result<&'a str> {
    match e.attributes.iter().find(|(key, _)| *key == name) {
        Some((_, value)) => Ok(value),
        None => fail(0, "required start-light attribute absent"),
    }
}

fn only<'a>(doc: &'a Document<'a>, parent: usize, name: &str) -> Result<&'a Element<'a>> {
    let mut found = doc.elements[parent]
        .children
        .clone()
        .filter(|&i| doc.elements.get(i).is_some_and(|e| e.name == name));
    match (found.next(), found.next()) {
        (Some(i), None) => Ok(&doc.elements[i]),
        _ => fail(parent, "expected one start-light element"),
    }
}

pub fn read(doc: &Document<'_>) -> Result<Sequence> {
    match doc.elements.first() {
        Some(e) if e.name == "startLightSequence" => {}
        _ => return fail(0, "expected startLightSequence root"),
    }
    let object = attr(only(doc, 0, "lights")?, "object")?;
    let sequence = only(doc, 0, "sequence")?;
    let count: usize = attr(sequence, "numStages")?
        .parse()
        .or(fail(0, "invalid start-light stage count"))?;
    let seconds: f32 = attr(sequence, "time")?
        .parse()
        .or(fail(0, "invalid start-light time"))?;
    if !(1..=16).contains(&count) || !seconds.is_finite() || !(0.5..=30.).contains(&seconds) {
        return fail(0, "start-light sequence out of range");
    }
    let stages: Vec<&Element<'_>> = sequence
        .children
        .clone()
        .filter_map(|i| doc.elements.get(i))
        .collect();
    if stages.len() != count {
        return fail(0, "start-light stage count mismatch");
    }
    let mut textures = Vec::with_capacity(count);
    for (ordinal, stage) in stages.into_iter().enumerate() {
        if stage.name != "stage" || attr(stage, "id")? != (ordinal + 1).to_string() {
            return fail(0, "start-light stages must follow declared order");
        }
        textures.push(attr(stage, "texture")?.to_owned());
    }
    Ok(Sequence {
        object: object.to_owned(),
        seconds,
        stages: textures,
    })
}
