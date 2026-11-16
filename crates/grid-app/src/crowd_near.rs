//! Baked ordinary source clip poses for nearby spectators.
use crate::{
    crowd_population::{Prepared as Metadata, Template},
    crowd_pose::{PoseChannel, PosePolicy, Source, VertexBuffers},
    crowd_preview::{shader_texture, unique_texture},
    crowd_selected_clip::{self, BoundClip, ExplicitClipBinding},
};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use grid_formats::{mesh, pssg, scene, texture};
use grid_tools::crowd_selection::Library;
use std::{error::Error, path::Path};

type AnyResult<T> = Result<T, Box<dyn Error>>;
const FRAME_RATE: f32 = 20.0;
const MAX_FRAMES: usize = 48;
const MAX_VARIANTS: usize = 24;

struct Clip {
    frames: Vec<VertexBuffers>,
    duration: f32,
    raw_ordinal: usize,
}

pub(super) enum Motion {
    Standing,
    Walking,
}

pub(super) struct Baked {
    idle: Clip,
    action: Option<Clip>,
    pub(super) diffuse: Vec<u8>,
    pub(super) tint: Vec<u8>,
    pub(super) foot_offset: Vec3,
    uv: Vec<[f32; 2]>,
    indices: Vec<u16>,
}

pub(super) struct PoseMeshes {
    idle: Vec<Handle<Mesh>>,
    idle_duration: f64,
    action: Vec<Handle<Mesh>>,
    action_duration: f64,
}

impl PoseMeshes {
    /// Stagger authored cheers. The cycle starts at an idle-loop boundary so
    /// the baked pose blend lands on the same idle timeline before and after.
    pub(super) fn frame(&self, seconds: f64, actor_phase: f64) -> &Handle<Mesh> {
        let interval = ((self.action_duration + 8.0) / self.idle_duration)
            .ceil()
            .max(1.0)
            * self.idle_duration;
        let phase = (seconds + interval * actor_phase).rem_euclid(interval);
        let (frames, duration, coord) = if !self.action.is_empty() && phase < self.action_duration {
            (&self.action, self.action_duration, phase)
        } else {
            (
                &self.idle,
                self.idle_duration,
                phase.rem_euclid(self.idle_duration),
            )
        };
        let frame = (coord / duration * frames.len() as f64) as usize;
        &frames[frame.min(frames.len() - 1)]
    }

    pub(super) fn walking_frame(&self, seconds: f64, actor_phase: f64) -> &Handle<Mesh> {
        let (frames, duration) = if actor_phase >= 0.5 && !self.action.is_empty() {
            (&self.action, self.action_duration)
        } else {
            (&self.idle, self.idle_duration)
        };
        let frame = (seconds.rem_euclid(duration) / duration * frames.len() as f64) as usize;
        &frames[frame.min(frames.len() - 1)]
    }
}

impl Baked {
    pub(super) fn frame_vertices(&self) -> usize {
        (self.idle.frames.len() + self.action.as_ref().map_or(0, |clip| clip.frames.len()))
            * self.uv.len()
    }

    #[cfg(test)]
    pub(super) fn has_animated_action(&self) -> bool {
        self.action.as_ref().is_some_and(|clip| {
            clip.frames
                .windows(2)
                .any(|pair| pair[0].positions != pair[1].positions)
        })
    }

    pub(super) fn meshes(&self, assets: &mut Assets<Mesh>) -> PoseMeshes {
        let mut meshes = |frames: &[VertexBuffers]| {
            frames
                .iter()
                .map(|frame| {
                    let mut mesh = Mesh::new(
                        PrimitiveTopology::TriangleList,
                        RenderAssetUsages::default(),
                    );
                    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, frame.positions.clone());
                    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, frame.normals.clone());
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uv.clone());
                    mesh.insert_indices(Indices::U16(self.indices.clone()));
                    assets.add(mesh)
                })
                .collect()
        };
        let idle = meshes(&self.idle.frames);
        let (action, action_duration) = self.action.as_ref().map_or_else(
            || (Vec::new(), 0.),
            |clip| (meshes(&clip.frames), f64::from(clip.duration)),
        );
        PoseMeshes {
            idle,
            idle_duration: f64::from(self.idle.duration),
            action,
            action_duration,
        }
    }
}

pub(super) fn bake(
    root: &Path,
    template: &Template,
    metadata: &Metadata<'_, '_, '_, '_>,
    library: &Library<'_>,
    bank: &pssg::Tree<'_>,
    motion: Motion,
) -> AnyResult<Option<Baked>> {
    let Some(resource) = &template.mesh else {
        return Ok(None);
    };
    let bytes = std::fs::read(root.join(&resource.relative_path))?;
    let geometry = pssg::read_tree(&bytes)?;
    let catalogue = mesh::read_object_meshes(&geometry)?;

    // Shipped spectator banks expose one source skin with several body detail
    // variants. Choose the highest triangle-count body from its own modifiers.
    // This port threshold is independent of source LOD policy.
    let skins: Vec<_> = (0..geometry.nodes.len())
        .filter(|&node| geometry.name(node) == b"SKINNODE")
        .collect();
    let [skin] = skins.as_slice() else {
        return Ok(None);
    };
    let skin_id = geometry.string(*skin, b"id")?;
    let skin = scene::read_skin(&geometry, &catalogue, skin_id)?;
    let mut biggest = None;
    for &(modifier, _, _) in &skin.instances {
        if let Ok(candidate) = Source::read(&geometry, skin_id, modifier)
            && biggest
                .as_ref()
                .is_none_or(|rig: &Source| rig.indices.len() < candidate.indices.len())
        {
            biggest = Some(candidate);
        }
    }
    let Some(rig) = biggest else { return Ok(None) };
    let policy = || PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: false };
    let prior = rig.saved_locals();
    let neutral = rig.sample(&prior, &[], policy())?;
    let min = neutral
        .positions
        .iter()
        .copied()
        .map(Vec3::from_array)
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let max = neutral
        .positions
        .iter()
        .copied()
        .map(Vec3::from_array)
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    let foot_offset = Vec3::new(-(min.x + max.x) * 0.5, -min.y, -(min.z + max.z) * 0.5);
    if !foot_offset.is_finite() {
        return Err("invalid source spectator mesh anchor".into());
    }

    let texture_bindings = grid_formats::material::read_texture_bindings(&geometry)?;
    let diffuse_id = shader_texture(&texture_bindings, &rig.material_shader, b"TDiffuseAlphaMap")?;
    let tint_id = shader_texture(&texture_bindings, &rig.material_shader, b"TTintMap")?;
    let textures = texture::read_named(&geometry, &[diffuse_id, tint_id])?;
    let diffuse = unique_texture(&textures, diffuse_id)?.validated_dds().0;
    let tint = unique_texture(&textures, tint_id)?.validated_dds().0;

    let pose_source = PoseSource {
        metadata,
        template,
        library,
        bank,
        rig: &rig,
        prior: &prior,
    };
    let sample = |performance, require_motion, exclude| {
        pose_source.sample(performance, require_motion, exclude)
    };
    let Some(idle) = sample(2, matches!(motion, Motion::Walking), None) else {
        return Ok(None);
    };
    let action = match motion {
        Motion::Standing => sample(3, true, None).map(|mut clip| {
            blend_action_with_idle(&mut clip, &idle);
            clip
        }),
        // Walkers declare two uniform Base Idle leaves. Keep the choices so
        // placements can select a stable variant across replay and restart.
        Motion::Walking => sample(2, true, Some(idle.raw_ordinal)),
    };
    Ok(Some(Baked {
        idle,
        action,
        diffuse,
        tint,
        foot_offset,
        uv: rig.uv.clone(),
        indices: rig.indices.clone(),
    }))
}

struct PoseSource<'r, 'lib, 'tree, 'm, 'p, 'layout, 'd, 'def> {
    metadata: &'m Metadata<'p, 'layout, 'd, 'def>,
    template: &'m Template,
    library: &'r Library<'lib>,
    bank: &'r pssg::Tree<'tree>,
    rig: &'r Source,
    prior: &'r [[f32; 16]],
}

impl PoseSource<'_, '_, '_, '_, '_, '_, '_, '_> {
    fn sample(
        &self,
        performance: u32,
        require_motion: bool,
        exclude: Option<usize>,
    ) -> Option<Clip> {
        let mut random = 1u32;
        let mut draw = || {
            random = random.wrapping_mul(214_013).wrapping_add(2_531_011);
            (random >> 16) & 0x7fff
        };
        let mut attempted = std::collections::BTreeSet::new();
        for _ in 0..MAX_VARIANTS {
            let Ok(Some(selected)) = crowd_selected_clip::select_template(
                self.metadata,
                self.template,
                self.library,
                Some(performance),
                &mut draw,
            ) else {
                continue;
            };
            if exclude == Some(selected.raw_ordinal)
                || !attempted.insert(selected.raw_ordinal)
                || selected.candidate.mirrored
            {
                continue;
            }
            let bind = ExplicitClipBinding {
                raw_ordinal: selected.raw_ordinal,
                clip_id: selected.candidate.raw_name.as_bytes(),
            };
            let Ok(clip) = BoundClip::read(&selected, bind, self.bank) else {
                continue;
            };
            let Ok((left, right)) = clip.key_range() else {
                continue;
            };
            let duration = right - left;
            if !duration.is_finite() || duration <= 0. || duration > 60. {
                continue;
            }
            let count = ((duration * FRAME_RATE).ceil() as usize).clamp(2, MAX_FRAMES);
            let frames = (0..count)
                .map(|i| -> AnyResult<VertexBuffers> {
                    let coordinate = left + duration * i as f32 / count as f32;
                    let samples = clip.sample(coordinate)?;
                    let channels: Vec<_> = samples
                        .iter()
                        .map(|channel| PoseChannel {
                            target: channel.target.as_bytes(),
                            kind: channel.kind,
                            raw: channel.raw,
                        })
                        .collect();
                    self.rig.sample(
                        self.prior,
                        &channels,
                        PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: false },
                    )
                })
                .collect::<AnyResult<Vec<_>>>();
            if let Ok(frames) = frames
                && (!require_motion
                    || frames
                        .windows(2)
                        .any(|pair| pair[0].positions != pair[1].positions))
            {
                println!(
                    "crowd_source_pose map={} performance={performance} clip={} samples={} duration={duration}",
                    self.template.map_name,
                    selected.candidate.raw_name,
                    frames.len()
                );
                return Some(Clip {
                    frames,
                    duration,
                    raw_ordinal: selected.raw_ordinal,
                });
            }
        }
        None
    }
}

fn blend_action_with_idle(action: &mut Clip, idle: &Clip) {
    let count = action.frames.len();
    let blend_duration = 0.2f32.min(action.duration * 0.25);
    for (i, frame) in action.frames.iter_mut().enumerate() {
        let seconds = action.duration * i as f32 / count as f32;
        let remaining = action.duration - seconds;
        let weight = (seconds.min(remaining) / blend_duration).min(1.);
        if weight >= 1. {
            continue;
        }
        let coordinate = seconds.rem_euclid(idle.duration) / idle.duration;
        let base = &idle.frames
            [((coordinate * idle.frames.len() as f32) as usize).min(idle.frames.len() - 1)];
        for (action, idle) in frame.positions.iter_mut().zip(&base.positions) {
            *action = Vec3::from_array(*idle)
                .lerp(Vec3::from_array(*action), weight)
                .to_array();
        }
        for (action, idle) in frame.normals.iter_mut().zip(&base.normals) {
            *action = Vec3::from_array(*idle)
                .lerp(Vec3::from_array(*action), weight)
                .normalize_or_zero()
                .to_array();
        }
    }
}
