//! Track sky domes (`sky.pssg`, docs/SKY.md).
//!
//! Every shipped sky holds one draw. Its material is either `sky.fx` with one
//! `TDiffuseAlphaMap` (every track but Le Mans), or Le Mans' `nightsky.fx`
//! with six maps `TFirstMap` .. `TSixthMap`. The original looks the six up by
//! those names into its slot table (0x9852E8..0x985377) and binds slots `i`
//! and `i + 1` of the time of day as the shader's `TFirstMap`/`TSecondMap`
//! (0x95FCE0, `grid_tools::time_of_day::sky_textures`) [V static].
use crate::{
    Result, fail,
    material::{self, TextureBinding},
    mesh, pssg, scene,
    texture::{self, Texture},
};

/// `nightsky.fx` slot names in slot order [V 0x9852E8].
pub const NIGHT_SLOTS: [&[u8]; 6] = [
    b"TFirstMap",
    b"TSecondMap",
    b"TThirdMap",
    b"TFourthMap",
    b"TFifthMap",
    b"TSixthMap",
];

/// The sky's texture per slot: one (`sky.fx`) or six (`nightsky.fx`).
pub fn slots<'a>(bindings: &[TextureBinding<'a>], material: &[u8]) -> Result<Vec<&'a [u8]>> {
    let texture = |sampler: &[u8]| {
        bindings
            .iter()
            .find(|b| b.shader == material && b.sampler == sampler)
            .map(|b| b.texture)
    };
    if let Some(day) = texture(b"TDiffuseAlphaMap") {
        return Ok(vec![day]);
    }
    let night: Option<Vec<_>> = NIGHT_SLOTS.iter().map(|s| texture(s)).collect();
    match night {
        Some(night) => Ok(night),
        None => fail(
            0,
            "sky material has neither TDiffuseAlphaMap nor six night maps",
        ),
    }
}

#[derive(Debug)]
pub struct Sky<'a> {
    /// Dome vertices with the draw's transform applied.
    pub positions: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
    /// Texture ID per slot (see [`slots`]).
    pub slots: Vec<&'a [u8]>,
    /// The distinct textures the slots name.
    pub textures: Vec<Texture<'a>>,
}

impl<'a> Sky<'a> {
    pub fn texture(&self, slot: usize) -> &Texture<'a> {
        let id = self.slots[slot];
        self.textures
            .iter()
            .find(|t| t.id == id)
            .expect("validated slot texture")
    }
}

pub fn read<'a>(tree: &pssg::Tree<'a>) -> Result<Sky<'a>> {
    let sources = mesh::read_meshes(tree)?;
    let draws = scene::read_static_draws(tree, &sources)?;
    let [draw] = draws.as_slice() else {
        return fail(0, "expected one sky draw");
    };
    let source = &sources[draw.source];
    if source.texcoords.len() != source.positions.len() {
        return fail(0, "sky UV count mismatch");
    }
    let positions: Vec<_> = source.positions[draw.vertices.clone()]
        .iter()
        .map(|p| scene::transform_point(&draw.transform, *p))
        .collect();
    let texcoords = source.texcoords[draw.vertices.clone()].to_vec();
    if positions
        .iter()
        .flatten()
        .chain(texcoords.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return fail(0, "nonfinite sky vertex");
    }
    if draw.indices.is_empty()
        || !draw.indices.len().is_multiple_of(3)
        || draw
            .indices
            .iter()
            .any(|&i| usize::from(i) >= positions.len())
    {
        return fail(0, "sky indices invalid");
    }
    let bindings = material::read_texture_bindings(tree)?;
    let slots = slots(&bindings, draw.material)?;
    let mut ids = slots.clone();
    ids.sort_unstable();
    ids.dedup();
    let textures = texture::read_named(tree, &ids)?;
    if textures.len() != ids.len() {
        return fail(0, "sky texture missing");
    }
    Ok(Sky {
        positions,
        texcoords,
        indices: draw.indices.clone(),
        slots,
        textures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding<'a>(shader: &'a [u8], sampler: &'a [u8], texture: &'a [u8]) -> TextureBinding<'a> {
        TextureBinding {
            shader,
            sampler,
            texture,
            container: None,
        }
    }

    #[test]
    fn day_sky_has_one_slot() {
        let b = [binding(b"sky!0", b"TDiffuseAlphaMap", b"sky_blue_06b.dds")];
        assert_eq!(slots(&b, b"sky!0").unwrap(), [b"sky_blue_06b.dds"]);
    }

    #[test]
    fn night_sky_slots_follow_the_map_names_not_file_order() {
        // Le Mans saves the inputs in parameter order 3,1,2,1,3,4; here they
        // come shuffled and must still land in TFirstMap..TSixthMap order.
        let b = [
            binding(b"sky!0", b"TSixthMap", b"4"),
            binding(b"sky!0", b"TSecondMap", b"1"),
            binding(b"sky!0", b"TFirstMap", b"3"),
            binding(b"sky!0", b"TFifthMap", b"3"),
            binding(b"sky!0", b"TThirdMap", b"2"),
            binding(b"sky!0", b"TFourthMap", b"1"),
            binding(b"other", b"TDiffuseAlphaMap", b"x"),
        ];
        let s = slots(&b, b"sky!0").unwrap();
        assert_eq!(s, [b"3" as &[u8], b"1", b"2", b"1", b"3", b"4"]);
    }

    #[test]
    fn incomplete_night_sky_is_rejected() {
        let b = [
            binding(b"sky!0", b"TFirstMap", b"3"),
            binding(b"sky!0", b"TSecondMap", b"1"),
        ];
        assert!(slots(&b, b"sky!0").is_err());
    }
}
