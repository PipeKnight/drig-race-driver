//! The flag officials use the same admitted body and rigid rod sample.
use super::{AnyResult, near};
use crate::{
    crowd_preview::{CrowdMaterial, Tint},
    graphics,
    replay::{Replay, flag_demand, flag_phase},
    texture_upload,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    pbr::ExtendedMaterial,
    prelude::*,
};
use grid_tools::crowd_flags::Role;
use std::collections::BTreeMap;

pub(super) struct Placement {
    group: usize,
    pub(super) position: Vec3,
    body: Transform,
}

pub(super) struct Prepared {
    bank: near::flag_bank::Prepared,
    rods: near::flag_rod::Rods,
    local_rod: Vec<[Vec<Transform>; 4]>,
    pub(super) rows: Vec<Placement>,
}

impl Prepared {
    pub(super) fn new(
        bank: near::flag_bank::Prepared,
        rods: near::flag_rod::Rods,
    ) -> AnyResult<Self> {
        let mut rows = Vec::new();
        let mut local_rod = Vec::new();
        for (index, group) in bank.groups.iter().enumerate() {
            bank.phase_clips(group)
                .ok_or("invalid source marshal flag clock")?;
            let mut phases = [const { Vec::new() }; 4];
            for (phase, slot) in phases.iter_mut().enumerate() {
                let clip = &bank.clips[&group.clips[phase]];
                if !clip.first.is_finite() {
                    return Err("invalid source flag bank coordinate".into());
                }
                for frame in &clip.frames {
                    if !frame
                        .body
                        .positions
                        .iter()
                        .chain(&frame.body.normals)
                        .flatten()
                        .all(|value| value.is_finite())
                    {
                        return Err("invalid prepared flag body pose".into());
                    }
                    rods.check_pose(group.role, frame.hand, Transform::IDENTITY)?;
                    slot.push(rods.pose(group.role, frame.hand)?);
                }
            }
            local_rod.push(phases);
            for point in &group.placements {
                let position = Vec3::from_array(point.position);
                let [x, z] = point.orientation_pair;
                let yaw = Quat::from_rotation_y((-z).atan2(x));
                let body = Transform::from_translation(position + yaw * bank.foot_offset)
                    .with_rotation(yaw);
                if !body.to_matrix().is_finite() {
                    return Err("invalid prepared standing flag placement".into());
                }
                rows.push(Placement {
                    group: index,
                    position,
                    body,
                });
            }
        }
        Ok(Self {
            bank,
            rods,
            local_rod,
            rows,
        })
    }

    #[cfg(test)]
    pub(super) fn bank(&self) -> &near::flag_bank::Prepared {
        &self.bank
    }

    pub(super) fn total_vertices(&self) -> usize {
        self.bank.total_vertices
    }

    pub(super) fn source_groups(&self) -> impl Iterator<Item = usize> + '_ {
        self.bank.groups.iter().map(|group| group.source_group)
    }

    fn role(&self, row: usize) -> Role {
        self.bank.groups[self.rows[row].group].role
    }

    pub(super) fn publish(
        &self,
        replay: &mut Replay,
        segment: u64,
        tick: u64,
        selected: impl Iterator<Item = usize>,
    ) {
        let mask = selected.fold(0, |mask, row| mask | 1 << (self.role(row) as u8 - 1));
        replay.publish_crowd_flag_demand(
            segment,
            tick,
            flag_demand::Demand::observed(mask).unwrap(),
        );
    }

    pub(super) fn install(
        &self,
        replay: &mut Replay,
        settings: &graphics::Settings,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
        materials: &mut Assets<CrowdMaterial>,
        rod_materials: &mut Assets<StandardMaterial>,
    ) -> AnyResult<Installed> {
        let profiles = self
            .bank
            .groups
            .iter()
            .map(|group| {
                self.bank
                    .phase_clips(group)
                    .map(|clips| (group.role, clips))
                    .ok_or("invalid installed marshal flag clock")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let diffuse = texture_upload::image(
            &self.bank.diffuse,
            settings.sampler(false),
            RenderAssetUsages::default(),
        )?;
        let tint = texture_upload::image_linear(
            &self.bank.tint,
            settings.sampler(false),
            RenderAssetUsages::default(),
        )?;
        let rods = self.rods.install(settings, meshes, images, rod_materials)?;
        let diffuse = images.add(diffuse);
        let tint = images.add(tint);
        let materials = self
            .bank
            .groups
            .iter()
            .map(|group| {
                materials.add(ExtendedMaterial {
                    base: StandardMaterial {
                        base_color_texture: Some(diffuse.clone()),
                        perceptual_roughness: 1.,
                        reflectance: 0.,
                        cull_mode: None,
                        double_sided: true,
                        ..default()
                    },
                    extension: Tint {
                        map: tint.clone(),
                        colour0: group.colours[0],
                        colour1: group.colours[1],
                        colour2: group.colours[2],
                        billboard_light: Default::default(),
                        source_sample_coverage: false,
                    },
                })
            })
            .collect();
        let meshes = self
            .bank
            .clips
            .iter()
            .map(|(&id, clip)| {
                let handles = near::upload_frames(
                    clip.frames.iter().map(|frame| &frame.body),
                    &self.bank.uv,
                    &self.bank.indices,
                    meshes,
                );
                (id, handles)
            })
            .collect();
        replay.install_crowd_flag_profiles(profiles);
        println!(
            "crowd_flag_bank groups={} placements={} modifier={:?} total_vertices={}",
            self.bank.groups.len(),
            self.rows.len(),
            String::from_utf8_lossy(&self.bank.modifier),
            self.bank.total_vertices,
        );
        Ok(Installed {
            meshes,
            materials,
            rods,
        })
    }

    pub(super) fn choice(&self, replay: &Replay, group: usize) -> Option<flag_phase::Frame> {
        replay.crowd_flag_frame(self.bank.groups[group].role)
    }

    pub(super) fn group(&self, row: usize) -> usize {
        self.rows[row].group
    }

    pub(super) fn pose<'a>(
        &'a self,
        installed: &'a Installed,
        row: usize,
        choice: flag_phase::Frame,
    ) -> Option<Presented<'a>> {
        let placement = self.rows.get(row)?;
        let group = self.bank.groups.get(placement.group)?;
        let phase = choice.phase as usize;
        let ordinal = group.clips[phase];
        Some(Presented {
            body: placement.body,
            mesh: installed.meshes.get(&ordinal)?.get(choice.sample)?,
            material: installed.materials.get(placement.group)?,
            hand: *self
                .local_rod
                .get(placement.group)?
                .get(phase)?
                .get(choice.sample)?,
            role: group.role,
            group: group.source_group,
            clip: self.bank.clips.get(&ordinal)?.id.as_str(),
        })
    }

    pub(super) fn spawn(
        &self,
        installed: &Installed,
        commands: &mut Commands,
        pose: &Presented<'_>,
        tick: u64,
    ) -> Live {
        let body = commands
            .spawn((
                Name::new("crowd/source-flag-body"),
                Mesh3d(pose.mesh.clone()),
                MeshMaterial3d(pose.material.clone()),
                pose.body,
                NoFrustumCulling,
                NotShadowCaster,
            ))
            .id();
        let rod = commands
            .spawn(self.rods.child(&installed.rods, pose.role, body, pose.hand))
            .id();
        println!(
            "crowd_flag_visible group={} role={:?} tick={tick} clip={}",
            pose.group, pose.role, pose.clip
        );
        Live { body, rod }
    }
}

#[derive(Resource)]
pub(super) struct Installed {
    meshes: BTreeMap<usize, Vec<Handle<Mesh>>>,
    materials: Vec<Handle<CrowdMaterial>>,
    rods: near::flag_rod::Draws,
}

pub(super) struct Presented<'a> {
    body: Transform,
    pub(super) mesh: &'a Handle<Mesh>,
    material: &'a Handle<CrowdMaterial>,
    pub(super) hand: Transform,
    role: Role,
    group: usize,
    clip: &'a str,
}

pub(super) struct Live {
    pub(super) body: Entity,
    pub(super) rod: Entity,
}
