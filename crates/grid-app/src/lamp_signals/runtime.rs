//! Consumed optional lamp state/material adapter. Native focus/ancestry/gamma remain unproved.
use super::{Atlas, Fades, Mode};
use crate::lamp_association::{BankKey, Receipt};
use crate::lamp_sections::Role;
use bevy::{asset::AssetId, prelude::*};
use std::collections::HashMap;

#[derive(Resource, Default)]
pub(crate) struct State {
    slots: Vec<Fades>,
    epoch: Option<(u64, u64)>,
}
impl State {
    pub(crate) fn constant(&mut self, mode: Mode, count: usize) {
        let values = match mode {
            Mode::Headlights => [0.7, 0., 0.],
            Mode::Brake => [0., 0.7, 0.],
            Mode::Reverse => [0., 0., 1.],
            _ => [0.; 3],
        };
        self.slots = vec![Fades(values); count];
        self.epoch = None;
    }
    pub(super) fn invalidate(&mut self) {
        self.slots.clear();
        self.epoch = None;
    }
    pub(super) fn advance(
        &mut self,
        segment: u64,
        tick: u64,
        inputs: &[[bool; 3]],
    ) -> Result<(), &'static str> {
        if self.epoch == Some((segment, tick)) {
            return Ok(());
        }
        let reset = self.epoch.is_none_or(|(s, t)| s != segment || tick < t);
        if !reset && self.slots.len() != inputs.len() {
            return Err("lamp field slot count changed within segment");
        }
        if reset {
            self.slots = vec![Fades::default(); inputs.len()];
        }
        let last = self
            .epoch
            .filter(|(s, t)| *s == segment && *t <= tick)
            .map(|x| x.1)
            .unwrap_or(tick.saturating_sub(1));
        let dt = tick.saturating_sub(last) as f32 * grid_tools::source_vehicle::DELTA;
        for (f, input) in self.slots.iter_mut().zip(inputs) {
            f.step(dt, *input, Fades::CAPS);
        }
        self.epoch = Some((segment, tick));
        Ok(())
    }
    fn value(&self, slot: usize, role: Role) -> Option<f32> {
        let f = self.slots.get(slot)?.0;
        Some(match role {
            Role::Front | Role::Rear => f[0],
            Role::Brake => f[1],
            Role::Reverse => f[2],
            Role::Combi => (f[0] + f[1]).min(2.),
        })
    }
    #[cfg(test)]
    pub(super) fn levels(&self, slot: usize) -> Option<[f32; 3]> {
        self.slots.get(slot).map(|x| x.0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Signed {
    Off,
    Positive(f32),
    Broken,
}
fn signed(level: f32, reachable: bool, broken: bool) -> Signed {
    if broken {
        Signed::Broken
    } else if !reachable || level == 0. {
        Signed::Off
    } else {
        Signed::Positive(level)
    }
}
impl Signed {
    fn emission(self) -> f32 {
        match self {
            Self::Positive(x) => x,
            Self::Off | Self::Broken => 0.,
        }
    }
}
/// Only remove an already authenticated exact LOD prefix; no suffix role inference.
pub(crate) fn member(full: &str, lod: u8) -> Result<String, &'static str> {
    full.strip_prefix(&format!("x{lod}_"))
        .map(str::to_owned)
        .ok_or("admitted source lamp LOD prefix differs")
}
#[derive(Component, Clone)]
pub(crate) struct Receiver {
    slot: usize,
    role: Role,
    member: String,
    bank: BankKey,
    // App rigid attachment admission, NOT native parent-chain parity.
    reachable: bool,
}
impl Receiver {
    pub(crate) fn from_receipt(slot: usize, receipt: &Receipt) -> Result<Self, &'static str> {
        Ok(Self {
            slot,
            role: receipt.section.role,
            member: member(&receipt.section.joint, receipt.section.lod)?,
            bank: receipt.bank.clone(),
            reachable: true,
        })
    }
}
#[derive(Component, Default)]
pub(crate) struct Families {
    owned: HashMap<(BankKey, AssetId<StandardMaterial>), Handle<StandardMaterial>>,
    canonical: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
}
#[derive(Resource, Default)]
pub(crate) struct Owners(HashMap<AssetId<StandardMaterial>, Entity>);
impl Families {
    fn resolve(
        &mut self,
        entity: Entity,
        current: &Handle<StandardMaterial>,
        bank: &BankKey,
        image: Option<&Handle<Image>>,
        materials: &mut Assets<StandardMaterial>,
        owners: &mut Owners,
    ) -> Result<Option<Handle<StandardMaterial>>, &'static str> {
        if owners
            .0
            .get(&current.id())
            .is_some_and(|owner| *owner != entity)
        {
            return Err("lamp received another entity's owned material");
        }
        let source = self.canonical.get(&current.id()).unwrap_or(current).clone();
        let key = (bank.clone(), source.id());
        if let Some(handle) = self.owned.get(&key) {
            let Some(mut material) = materials.get_mut(handle) else {
                return Ok(None);
            };
            if let Some(image) = image {
                material.emissive_texture = Some(image.clone());
            }
            material.emissive = LinearRgba::BLACK;
            return Ok(Some(handle.clone()));
        }
        let Some(mut material) = materials.get(&source).cloned() else {
            return Ok(None);
        };
        // A pending bank still needs a neutral OWNED family: damage may have
        // cloned a currently lit material. Preserve its optics while dark;
        // only an actually available exact bank may replace the sampler handle.
        if let Some(image) = image {
            material.emissive_texture = Some(image.clone());
        }
        material.emissive = LinearRgba::BLACK;
        // Existing pilot adaptation, not source photometry or sampler parity.
        material.emissive_exposure_weight = 0.;
        let handle = materials.add(material);
        self.canonical.insert(handle.id(), source);
        self.owned.insert(key, handle.clone());
        owners.0.insert(handle.id(), entity);
        Ok(Some(handle))
    }
}
pub(crate) type Receivers<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Receiver,
        &'static mut MeshMaterial3d<StandardMaterial>,
        Option<&'static mut Families>,
        Option<&'static crate::damage_parts::Original>,
    ),
>;
fn dark_owned(
    entity: Entity,
    handle: &Handle<StandardMaterial>,
    owners: &Owners,
    materials: &mut Assets<StandardMaterial>,
) {
    if owners.0.get(&handle.id()) == Some(&entity)
        && let Some(mut m) = materials.get_mut(handle)
    {
        m.emissive = LinearRgba::BLACK;
    }
}
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Registries<'w> {
    owners: ResMut<'w, Owners>,
    reflective: Option<ResMut<'w, crate::car_reflection_overlay::Reflective>>,
}
pub(crate) fn project(
    mut commands: Commands,
    atlas: Res<Atlas>,
    state: Res<State>,
    damage: Option<Res<crate::damage_parts::DamageParts>>,
    mut parts: Receivers,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut registries: Registries,
) {
    for (entity, receiver, mut material, families, original) in &mut parts {
        dark_owned(entity, &material.0, &registries.owners, &mut materials);
        if original.is_some_and(|o| {
            registries
                .owners
                .0
                .get(&o.0.id())
                .is_some_and(|owner| *owner != entity)
        }) {
            continue;
        }
        // Preserve keyed optics from the exact intact source used by overlay follow.
        // A damage clone need not have its own response row; Original identifies it.
        let reflection_source = original.map_or(&material.0, |o| &o.0);
        let reflection_source = families
            .as_ref()
            .and_then(|f| f.canonical.get(&reflection_source.id()))
            .unwrap_or(reflection_source)
            .id();
        let response = registries
            .reflective
            .as_deref()
            .and_then(|r| r.0.get(&reflection_source))
            .cloned();
        let image = atlas.0.get(&receiver.bank);
        let mut fresh = Families::default();
        // ALWAYS adopt before readiness. An unowned cracked material can inherit
        // emission from the current lit owned material via DamageParts::present.
        // Mutating that shared cracked source to dark is forbidden.
        let fresh = match families {
            Some(mut f) => {
                let Ok(Some(handle)) = f.resolve(
                    entity,
                    &material.0,
                    &receiver.bank,
                    image,
                    &mut materials,
                    &mut registries.owners,
                ) else {
                    continue;
                };
                material.0 = handle;
                None
            }
            None => {
                let Ok(Some(handle)) = fresh.resolve(
                    entity,
                    &material.0,
                    &receiver.bank,
                    image,
                    &mut materials,
                    &mut registries.owners,
                ) else {
                    continue;
                };
                material.0 = handle;
                Some(fresh)
            }
        };
        // The clone's material fields alone do not preserve this ID-keyed registry.
        // Refresh cached aliases too, including pending image/activation states.
        if let Some(ref mut reflective) = registries.reflective {
            // follow() reads Original when damaged. Refresh that alias too if
            // it is OUR intact handle; never rewrite a shared/foreign source.
            let intact = original
                .filter(|o| registries.owners.0.get(&o.0.id()) == Some(&entity))
                .map(|o| o.0.id());
            for target in [Some(material.0.id()), intact].into_iter().flatten() {
                if let Some(response) = &response {
                    reflective.0.insert(target, response.clone());
                } else {
                    reflective.0.remove(&target);
                }
            }
        }
        // resolve() neutralizes only our owned handle even when activation or
        // atlas is missing. There is no source/shared/Original material write.
        if image.is_some()
            && let Some(level) = state.value(receiver.slot, receiver.role)
        {
            let broken = damage
                .as_ref()
                .is_some_and(|d| d.light_smashed(receiver.slot, &receiver.member));
            let e = signed(level, receiver.reachable, broken).emission();
            if let Some(mut m) = materials.get_mut(&material.0) {
                m.emissive = LinearRgba::rgb(e, e, e);
            }
        }
        if let Some(f) = fresh {
            commands.entity(entity).insert(f);
        }
    }
}

pub(crate) fn replay_guard(
    replay: Option<Res<crate::replay::Replay>>,
    parts: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<Receiver>>,
    owners: Res<Owners>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if replay.is_some_and(|r| r.session.is_some()) {
        for (entity, handle) in &parts {
            dark_owned(entity, &handle.0, &owners, &mut materials);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::primitives::MeshAabb;
    fn key(bank: &str) -> BankKey {
        BankKey {
            source: bank.into(),
            resource: "same_lights_on.tga".into(),
        }
    }
    fn receiver(slot: usize, bank: &str) -> Receiver {
        Receiver {
            slot,
            role: Role::Brake,
            member: "light_brake_pod_bl".into(),
            bank: key(bank),
            reachable: true,
        }
    }
    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<State>()
            .init_resource::<Owners>();
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.insert_resource(Atlas([(key("actual_high"), image)].into_iter().collect()));
        app.world_mut()
            .resource_mut::<State>()
            .constant(Mode::Brake, 2);
        app.add_systems(PostStartup, project)
            .add_systems(Update, project);
        app
    }
    fn source(app: &mut App) -> Handle<StandardMaterial> {
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgba(0.2, 0.4, 0.6, 0.5),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.37,
                ..default()
            })
    }
    fn current(app: &App, e: Entity) -> Handle<StandardMaterial> {
        app.world()
            .get::<MeshMaterial3d<StandardMaterial>>(e)
            .unwrap()
            .0
            .clone()
    }
    #[test]
    fn actual_ecs_family_swap_original_reset_and_missing_low_do_not_leak() {
        let mut app = app();
        let base = source(&mut app);
        let original = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&base)
            .unwrap()
            .clone();
        let a = app
            .world_mut()
            .spawn((receiver(0, "actual_high"), MeshMaterial3d(base.clone())))
            .id();
        let b = app
            .world_mut()
            .spawn((receiver(1, "actual_high"), MeshMaterial3d(base.clone())))
            .id();
        app.world_mut().resource_mut::<State>().slots[1] = Fades::default();
        app.update();
        let intact = current(&app, a);
        let peer = current(&app, b);
        assert_ne!(intact, peer);
        assert_ne!(intact, base);
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&base)
                .unwrap()
                .emissive,
            original.emissive
        );
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&peer)
                .unwrap()
                .emissive,
            LinearRgba::BLACK
        );
        let cracked = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgb(0.01, 0.02, 0.03),
                ..original.clone()
            });
        app.world_mut().entity_mut(a).insert((
            MeshMaterial3d(cracked.clone()),
            crate::damage_parts::Original(intact.clone()),
        ));
        app.update();
        let owned_cracked = current(&app, a);
        assert_ne!(owned_cracked, cracked);
        assert_ne!(owned_cracked, intact);
        assert_eq!(
            app.world()
                .get::<crate::damage_parts::Original>(a)
                .unwrap()
                .0,
            intact
        );
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&cracked)
                .unwrap()
                .emissive,
            original.emissive
        );
        // Actual reset contract: restore Original, then remove it. Projection reuses intact ownership.
        app.world_mut()
            .entity_mut(a)
            .insert(MeshMaterial3d(intact.clone()))
            .remove::<crate::damage_parts::Original>();
        app.update();
        assert_eq!(current(&app, a), intact);
        // Same raw resource name in a different bank is NOT permission to reuse high.
        app.world_mut()
            .entity_mut(a)
            .insert(receiver(0, "actual_low_missing"));
        let assets = app.world().resource::<Assets<StandardMaterial>>().len();
        app.update();
        let pending_low = current(&app, a);
        assert_ne!(pending_low, intact);
        assert_eq!(
            app.world().resource::<Assets<StandardMaterial>>().len(),
            assets + 1
        );
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&pending_low)
                .unwrap()
                .emissive,
            LinearRgba::BLACK
        );
        let low_image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.world_mut()
            .resource_mut::<Atlas>()
            .0
            .insert(key("actual_low_missing"), low_image.clone());
        app.update();
        let low = current(&app, a);
        assert_eq!(
            low, pending_low,
            "exact image readiness updates our pending owned family"
        );
        assert_ne!(low, intact);
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&low)
                .unwrap()
                .emissive_texture,
            Some(low_image)
        );
        app.world_mut()
            .entity_mut(a)
            .insert(receiver(0, "actual_high"));
        app.update();
        assert_eq!(current(&app, a), intact);
        assert_eq!(current(&app, b), peer);
        let m = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&intact)
            .unwrap();
        assert_eq!(m.alpha_mode, original.alpha_mode);
        assert_eq!(m.perceptual_roughness, original.perceptual_roughness);
    }
    #[test]
    fn lit_owned_damage_clone_is_adopted_dark_before_missing_state_and_bank() {
        let mut app = app();
        let base = source(&mut app);
        let entity = app
            .world_mut()
            .spawn((receiver(0, "actual_high"), MeshMaterial3d(base.clone())))
            .id();
        app.update();
        let intact = current(&app, entity);
        let lit = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&intact)
            .unwrap()
            .clone();
        assert_eq!(lit.emissive, LinearRgba::rgb(0.7, 0.7, 0.7));
        // Same operation as damage_parts.rs1256..1266: clone CURRENT lit
        // material, change damage base texture, cache it, retain Original(base).
        let cracked = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color_texture: Some(Handle::default()),
                ..lit.clone()
            });
        app.world_mut().entity_mut(entity).insert((
            MeshMaterial3d(cracked.clone()),
            crate::damage_parts::Original(intact.clone()),
        ));
        app.world_mut().resource_mut::<State>().invalidate();
        app.world_mut().resource_mut::<Atlas>().0.clear();
        app.update();
        let adopted = current(&app, entity);
        assert_ne!(adopted, cracked);
        assert_ne!(adopted, intact);
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        assert_eq!(materials.get(&adopted).unwrap().emissive, LinearRgba::BLACK);
        assert_eq!(
            materials.get(&cracked).unwrap().emissive,
            lit.emissive,
            "shared cracked cache must remain untouched"
        );
        assert_eq!(
            app.world()
                .get::<crate::damage_parts::Original>(entity)
                .unwrap()
                .0,
            intact
        );
        assert_eq!(materials.get(&base).unwrap().emissive, LinearRgba::BLACK);
        assert_eq!(
            materials.get(&intact).unwrap().emissive,
            lit.emissive,
            "Original family untouched by cracked adoption"
        );
        // Exact bank returns while activation is still absent: no stale glow.
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.world_mut()
            .resource_mut::<Atlas>()
            .0
            .insert(key("actual_high"), image.clone());
        app.update();
        assert_eq!(current(&app, entity), adopted);
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&adopted)
                .unwrap()
                .emissive,
            LinearRgba::BLACK
        );
        // Once both inputs exist, update the same OWNED family only.
        app.world_mut()
            .resource_mut::<State>()
            .constant(Mode::Brake, 2);
        app.update();
        assert_eq!(current(&app, entity), adopted);
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        assert_eq!(materials.get(&adopted).unwrap().emissive, lit.emissive);
        assert_eq!(
            materials.get(&adopted).unwrap().emissive_texture,
            Some(image)
        );
        assert_eq!(materials.get(&cracked).unwrap().emissive, lit.emissive);
    }
    #[test]
    fn absent_material_readiness_retries_and_foreign_owned_handle_is_not_adopted() {
        let mut app = app();
        let missing = Handle::<StandardMaterial>::default();
        let e = app
            .world_mut()
            .spawn((receiver(0, "actual_high"), MeshMaterial3d(missing.clone())))
            .id();
        app.update();
        assert!(app.world().get::<Families>(e).is_none());
        assert_eq!(current(&app, e), missing);
        let base = source(&mut app);
        app.world_mut()
            .entity_mut(e)
            .insert(MeshMaterial3d(base.clone()));
        app.update();
        let owned = current(&app, e);
        assert_ne!(owned, base);
        let other = app
            .world_mut()
            .spawn((receiver(1, "actual_high"), MeshMaterial3d(owned.clone())))
            .id();
        let count = app.world().resource::<Assets<StandardMaterial>>().len();
        app.update();
        assert!(app.world().get::<Families>(other).is_none());
        assert_eq!(
            app.world().resource::<Assets<StandardMaterial>>().len(),
            count
        );
    }
    #[test]
    fn per_slot_clock_and_signed_override_are_source_ordered() {
        let mut s = State::default();
        s.advance(0, 0, &[[false, false, false], [false, false, false]])
            .unwrap();
        s.advance(0, 1, &[[false, true, false], [true, false, true]])
            .unwrap();
        let before = s.levels(0);
        s.advance(0, 1, &[[false, false, true], [false, true, false]])
            .unwrap();
        assert_eq!(s.levels(0), before);
        assert_ne!(s.levels(0), s.levels(1));
        assert_eq!(signed(0.7, false, false), Signed::Off);
        assert_eq!(signed(0.7, false, true), Signed::Broken);
        assert_eq!(signed(0.7, true, false), Signed::Positive(0.7));
        s.advance(1, 0, &[[false, false, false], [false, false, false]])
            .unwrap();
        assert_eq!(s.levels(0), Some([0.; 3]));
        assert_eq!(s.levels(1), Some([0.; 3]));
        assert_eq!(
            member("x2_light_2_brake_pod_bl", 2).unwrap(),
            "light_2_brake_pod_bl"
        );
        assert!(member("x3_light_pod_bl", 2).is_err());
        assert_eq!(member("x2_light_pod_bl\t", 2).unwrap(), "light_pod_bl\t");
    }
    #[test]
    fn actual_batcher_first_registration_sees_owned_handles() {
        let mut app = app();
        assert!(
            crate::car_batch::enabled(),
            "CPU fixture requires production batch install enabled; do not mutate ENV"
        );
        app.init_resource::<Assets<Mesh>>()
            .add_message::<AssetEvent<Mesh>>();
        crate::car_batch::install(&mut app);
        let base = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let mesh = Mesh::from(Cuboid::new(1., 1., 1.));
        let bounds = mesh.compute_aabb().unwrap();
        let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
        let mut entities = Vec::new();
        for slot in [0, 1] {
            let e = app
                .world_mut()
                .spawn((
                    receiver(slot, "actual_high"),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(base.clone()),
                    Transform::IDENTITY,
                    Visibility::Inherited,
                    bounds,
                    crate::OpponentPart {
                        car: slot,
                        rest: Mat4::IDENTITY,
                        wheel: None,
                        lost_with_wheel: false,
                    },
                ))
                .id();
            entities.push(e);
        }
        app.update();
        let owned: Vec<_> = entities.iter().map(|e| current(&app, *e)).collect();
        assert_ne!(owned[0], owned[1]);
        assert!(owned.iter().all(|h| h != &base));
        for _ in 0..5 {
            app.update();
        }
        let world = app.world_mut();
        let mut q = world.query::<(&crate::car_batch::Batch, &MeshMaterial3d<StandardMaterial>)>();
        let batches: Vec<_> = q.iter(world).map(|(b, m)| (b.car, m.0.clone())).collect();
        assert_eq!(
            batches.len(),
            2,
            "actual production batches must exist; no vacuous pass"
        );
        assert!(batches.iter().all(|(_, m)| m != &base));
        for (slot, handle) in owned.iter().enumerate() {
            assert!(batches.iter().any(|(car, m)| *car == slot && m == handle));
        }
        // No substitute classifier: this App actually installs the production car_batch systems.
    }
}
