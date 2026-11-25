//! Standing official source bodies and their matched hand pose samples.
use super::{
    AnyResult, MAX_VARIANTS,
    budget::{
        self,
        planner::{ClipCost, plan_lod},
    },
    foot_offset, sample_clip_frame,
};
use crate::{
    crowd_population::{Prepared as Metadata, Resolution, Template},
    crowd_pose::{PosePolicy, Source, VertexBuffers},
    crowd_preview::{shader_texture, unique_texture},
    crowd_selected_clip::{self, BoundClip, ExplicitClipBinding},
    replay::flag_phase,
};
use bevy::prelude::{Vec3, Vec4};
use grid_formats::{
    crowd_placements::{Placements, Standing},
    material, mesh, pssg, scene, texture,
};
use grid_tools::{
    crowd_flags::{self, Action, Role},
    crowd_selection::Library,
};
use std::{collections::BTreeMap, path::Path};

const HAND: &[u8] = b"RH_Prop_Helper";

pub(in crate::crowd_billboards) struct ActorGroup {
    pub(in crate::crowd_billboards) role: Role,
    pub(in crate::crowd_billboards) source_group: usize,
    pub(in crate::crowd_billboards) placements: Vec<Standing>,
    pub(in crate::crowd_billboards) colours: [Vec4; 3],
    /// Normal idle, transition in, held action and transition out.
    pub(in crate::crowd_billboards) clips: [usize; 4],
}

pub(in crate::crowd_billboards) struct Frame {
    pub(in crate::crowd_billboards) body: VertexBuffers,
    pub(in crate::crowd_billboards) hand: [f32; 16],
}

pub(in crate::crowd_billboards) struct Clip {
    pub(in crate::crowd_billboards) id: String,
    pub(in crate::crowd_billboards) first: f32,
    pub(in crate::crowd_billboards) duration: f32,
    pub(in crate::crowd_billboards) frames: Vec<Frame>,
}

pub(in crate::crowd_billboards) struct Prepared {
    pub(in crate::crowd_billboards) modifier: Vec<u8>,
    pub(in crate::crowd_billboards) uv: Vec<[f32; 2]>,
    pub(in crate::crowd_billboards) indices: Vec<u16>,
    pub(in crate::crowd_billboards) diffuse: Vec<u8>,
    pub(in crate::crowd_billboards) tint: Vec<u8>,
    pub(in crate::crowd_billboards) foot_offset: Vec3,
    pub(in crate::crowd_billboards) groups: Vec<ActorGroup>,
    pub(in crate::crowd_billboards) clips: BTreeMap<usize, Clip>,
    pub(in crate::crowd_billboards) total_vertices: usize,
}

impl Prepared {
    pub(in crate::crowd_billboards) fn phase_clips(
        &self,
        group: &ActorGroup,
    ) -> Option<flag_phase::Clips> {
        let [Some(idle), Some(start), Some(mid), Some(end)] = group.clips.map(|id| {
            self.clips.get(&id).map(|clip| flag_phase::Clip {
                duration: f64::from(clip.duration),
                samples: clip.frames.len(),
            })
        }) else {
            return None;
        };
        flag_phase::Clips::new(
            [idle, start, mid, end],
            f64::from(grid_tools::source_vehicle::DELTA),
        )
    }

    #[cfg(test)]
    pub(in crate::crowd_billboards) fn chosen_frame(
        &self,
        group: &ActorGroup,
        frame: flag_phase::Frame,
    ) -> Option<&Frame> {
        self.clips
            .get(&group.clips[frame.phase as usize])?
            .frames
            .get(frame.sample)
    }


}

struct Selected<'a> {
    id: String,
    ordinal: usize,
    first: f32,
    duration: f32,
    samples: usize,
    bound: BoundClip<'a>,
}

fn authored_prop(role: Role) -> &'static str {
    match role {
        Role::Start => "Start Flag",
        Role::Danger => "Marshall Flag",
        Role::Finish => "Finish Flag",
    }
}

fn selected_clip<'a>(
    metadata: &Metadata<'_, '_, '_, '_>,
    template: &Template,
    library: &Library<'_>,
    bank: &pssg::Tree<'a>,
    state: &str,
) -> AnyResult<Selected<'a>> {
    let performance =
        crowd_selected_clip::performance_index(state).ok_or("unknown flag action performance")?;
    let mut random = 1u32;
    let mut draw = || {
        random = random.wrapping_mul(214_013).wrapping_add(2_531_011);
        (random >> 16) & 0x7fff
    };
    for _ in 0..MAX_VARIANTS {
        let Some(selected) = crowd_selected_clip::select_template(
            metadata,
            template,
            library,
            Some(performance),
            &mut draw,
        )?
        else {
            continue;
        };
        if selected.candidate.mirrored {
            continue;
        }
        let ordinal = selected.raw_ordinal;
        let id = selected.candidate.raw_name;
        let binding = ExplicitClipBinding {
            raw_ordinal: ordinal,
            clip_id: id.as_bytes(),
        };
        let Ok(bound) = BoundClip::read(&selected, binding, bank) else {
            continue;
        };
        let (first, last) = bound.key_range()?;
        let duration = last - first;
        let samples = budget::frame_count(duration).ok_or("invalid flag clip duration")?;
        return Ok(Selected {
            ordinal,
            id: id.to_owned(),
            first,
            duration,
            samples,
            bound,
        });
    }
    Err(format!("no supported source flag clip for {state}").into())
}

fn action_for<'a>(
    actions: &'a [Action<'a>],
    template: &Template,
) -> AnyResult<Option<&'a Action<'a>>> {
    let Some(name) = crowd_selected_clip::field(&template.map_fields, "action")? else {
        return Ok(None);
    };
    Ok(actions
        .iter()
        .find(|action| action.role.action_name().eq_ignore_ascii_case(name)))
}

pub(in crate::crowd_billboards) fn prepare(
    root: &Path,
    metadata: &Metadata<'_, '_, '_, '_>,
    library: &Library<'_>,
    bank: &pssg::Tree<'_>,
    used_vertices: usize,
) -> AnyResult<Option<Prepared>> {
    let actions = crowd_flags::actions(metadata.definitions)?;
    let mut groups: Vec<ActorGroup> = Vec::new();
    let mut clips = BTreeMap::<usize, Selected<'_>>::new();
    let mut geometry_path = None;
    for (source_group, group) in metadata.groups.iter().enumerate() {
        let (Placements::Standing(placements), Resolution::StaticTemplates(templates)) =
            (&group.source.placements, &group.resolution)
        else {
            continue;
        };
        let [template] = templates.as_slice() else {
            for template in templates {
                if action_for(&actions, template)?.is_some() {
                    return Err("ambiguous authored standing flag template".into());
                }
            }
            continue;
        };
        let Some(action) = action_for(&actions, template)? else {
            continue;
        };
        let [variant, ..] = template.variants.as_slice() else {
            return Err("missing standing flag palette".into());
        };
        if variant.billboard_stance.is_some()
            || crowd_selected_clip::field(&template.map_fields, "hasSkin")? != Some("1")
            || !crowd_selected_clip::selected_prop(metadata, template)?
                .eq_ignore_ascii_case(authored_prop(action.role))
            || groups.iter().any(|group| group.role == action.role)
        {
            return Err("inconsistent authored standing flag group".into());
        }
        let path = &template
            .mesh
            .as_ref()
            .ok_or("missing standing flag rig")?
            .relative_path;
        if geometry_path.is_some_and(|previous| previous != path) {
            return Err("standing flag groups do not share the validated source rig".into());
        }
        geometry_path = Some(path);
        let mut ordinals = [0; 4];
        for (ordinal, state) in ordinals.iter_mut().zip([
            "Normal Idle",
            action.start_state,
            action.mid_state,
            action.end_state,
        ]) {
            let selected = selected_clip(metadata, template, library, bank, state)?;
            *ordinal = selected.ordinal;
            if let Some(old) = clips.get(&*ordinal)
                && (old.id != selected.id
                    || old.samples != selected.samples
                    || old.first != selected.first
                    || old.duration != selected.duration)
            {
                return Err("conflicting shared flag clip identity".into());
            }
            clips.entry(*ordinal).or_insert(selected);
        }
        groups.push(ActorGroup {
            role: action.role,
            source_group,
            placements: placements.to_vec(),
            colours: super::super::source_colours(variant)?,
            clips: ordinals,
        });
    }

    let Some(geometry_path) = geometry_path else {
        return Ok(None);
    };
    let data = std::fs::read(grid_tools::install::resolve_case(root, geometry_path))?;
    let geometry = pssg::read_tree(&data)?;
    let catalogue = mesh::read_object_meshes(&geometry)?;
    let mut skins = (0..geometry.nodes.len()).filter(|&i| geometry.name(i) == b"SKINNODE");
    let skin_node = skins.next().ok_or("missing flag skin")?;
    if skins.next().is_some() {
        return Err("ambiguous flag skin".into());
    }
    let skin_id = geometry.string(skin_node, b"id")?;
    let skin = scene::read_skin(&geometry, &catalogue, skin_id)?;
    let mut modifiers = skin.instances;
    // Same highest-triangle ordering as the ordinary near bake.
    modifiers.sort_by_key(|(_, _, index)| std::cmp::Reverse(catalogue[*index].indices.len()));
    let costs: Vec<_> = clips
        .values()
        .map(|clip| ClipCost {
            ordinal: clip.ordinal,
            samples: clip.samples,
        })
        .collect();
    let vertices: Vec<_> = modifiers
        .iter()
        .map(|(_, _, i)| catalogue[*i].positions.len())
        .collect();
    let plan = plan_lod(used_vertices, budget::MAX_FRAME_VERTICES, &vertices, &costs)
        .ok_or("authored flags exceed the shared baked pose allocation")?;
    let (modifier, _, _) = modifiers[plan.lod];
    let rig = Source::read(&geometry, skin_id, modifier)?;
    let prior = rig.saved_locals();
    let policy = || PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: false };
    let hand = rig.named_node(HAND)?;
    let anchor = if plan.lod == 0 {
        foot_offset(&rig.sample(&prior, &[], policy())?)?
    } else {
        let highest = Source::read(&geometry, skin_id, modifiers[0].0)?;
        foot_offset(&highest.sample(&highest.saved_locals(), &[], policy())?)?
    };
    let bindings = material::read_texture_bindings(&geometry)?;
    let diffuse_id = shader_texture(&bindings, &rig.material_shader, b"TDiffuseAlphaMap")?;
    let tint_id = shader_texture(&bindings, &rig.material_shader, b"TTintMap")?;
    let textures = texture::read_named(&geometry, &[diffuse_id, tint_id])?;
    let diffuse = unique_texture(&textures, diffuse_id)?.validated_dds().0;
    let tint = unique_texture(&textures, tint_id)?.validated_dds().0;

    let mut prepared = BTreeMap::new();
    let mut frame_count = 0;
    for clip in clips.into_values() {
        let frames: Vec<_> = (0..clip.samples)
            .map(|i| -> AnyResult<Frame> {
                let coordinate = clip.first + clip.duration * i as f32 / clip.samples as f32;
                let (body, hand) = sample_clip_frame(&clip.bound, coordinate, |channels| {
                    rig.sample_with_node(&prior, channels, policy(), hand)
                })?;
                Ok(Frame { body, hand })
            })
            .collect::<AnyResult<_>>()?;
        frame_count += frames.len();
        prepared.insert(
            clip.ordinal,
            Clip {
                id: clip.id,
                first: clip.first,
                duration: clip.duration,
                frames,
            },
        );
    }
    if frame_count != plan.samples || frame_count * rig.uv.len() != plan.vertices {
        return Err("prepared flag sample counts differ from allocation".into());
    }
    Ok(Some(Prepared {
        modifier: modifier.to_vec(),
        uv: rig.uv,
        indices: rig.indices,
        diffuse,
        tint,
        foot_offset: anchor,
        groups,
        clips: prepared,
        total_vertices: plan.total,
    }))
}
