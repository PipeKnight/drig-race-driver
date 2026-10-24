//! Bounded current-frame foliage visibility observer; never skips presentation.
use super::Facing;
use bevy::{
    asset::{AssetEvent, AssetEventSystems, AssetId},
    camera::visibility::{
        CascadesVisibleEntities, CubemapVisibleEntities, NoCpuCulling, ViewVisibility,
        VisibilitySystems, VisibleEntities, VisibleMeshEntities,
    },
    diagnostic::FrameCount,
    ecs::message::MessageCursor,
    prelude::*,
};
use std::{any::TypeId, collections::BTreeMap};

const CAP: usize = 16_384;
const EVENT_CAP: usize = 65_536;
const MAX_CALLS: u32 = 360;

fn budget(value: &str) -> Option<u32> {
    let n: u32 = value.parse().ok()?;
    ((1..=MAX_CALLS).contains(&n) && n.to_string() == value).then_some(n)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Unknown,
    Contradiction,
    Hidden,
    Eligible,
    Unattributed,
}
#[derive(Clone, Copy, Default, Debug)]
struct Weight {
    rows: u64,
    vertices: u64,
    bytes: u64,
}
impl Weight {
    fn add(&mut self, vertices: u64, bytes: u64) {
        self.rows += 1;
        self.vertices += vertices;
        self.bytes += bytes;
    }
}
#[derive(Default, Debug)]
struct Counts {
    classes: [Weight; 5],
    membership: [Weight; 4],
    joined_masks: [Weight; 16],
    flag_true: u64,
    flag_false: u64,
    flag_missing: u64,
    first: u64,
    replaced: u64,
    changed: u64,
    reentry: u64,
    gap_visible: u64,
    no_cpu_culling: u64,
    no_position: u64,
    late_unjoined: u64,
    components_changed: u64,
    asset_epoch_unknown: u64,
    history_untracked: u64,
}
#[derive(Clone, Copy)]
struct Produced {
    mesh: AssetId<Mesh>,
    ticks: [u32; 2],
    epoch: u64,
    epoch_known: bool,
    family: usize,
    vertices: u64,
    bytes: u64,
    membership: u8,
}
struct Epoch {
    value: u64,
    known: bool,
}
#[derive(Clone, Copy)]
struct Seen {
    frame: u32,
    mesh: AssetId<Mesh>,
    epoch: u64,
    class: Class,
}
#[derive(Resource)]
pub(super) struct Census {
    budget: u32,
    observed: u32,
    call: u32,
    frame: Option<u32>,
    camera: Option<[u32; 10]>,
    finished: bool,
    duplicate: bool,
    rows: BTreeMap<Entity, Produced>,
    epochs: BTreeMap<AssetId<Mesh>, Epoch>,
    seen: BTreeMap<Entity, Seen>,
    events: MessageCursor<AssetEvent<Mesh>>,
    event_loss: bool,
    overflow_rows: u64,
    no_producer: u32,
    bad_join: u32,
    incomplete_calls: u32,
    invalid_main_calls: u32,
    counts: [Counts; 2],
}
fn camera_bits(t: &Transform) -> [u32; 10] {
    let p = t.translation.to_array();
    let q = t.rotation.to_array();
    let s = t.scale.to_array();
    [
        p[0].to_bits(),
        p[1].to_bits(),
        p[2].to_bits(),
        q[0].to_bits(),
        q[1].to_bits(),
        q[2].to_bits(),
        q[3].to_bits(),
        s[0].to_bits(),
        s[1].to_bits(),
        s[2].to_bits(),
    ]
}
fn component_matches(row: &Produced, facing: &Ref<Facing>, mesh: &Ref<Mesh3d>) -> bool {
    mesh.id() == row.mesh && [facing.last_changed().get(), mesh.last_changed().get()] == row.ticks
}
fn classify(identity: bool, complete: bool, visible: Option<bool>, mask: u8) -> Class {
    if !identity || !complete {
        return Class::Unknown;
    }
    match (visible, mask == 0) {
        (None, _) => Class::Unknown,
        (Some(false), false) => Class::Contradiction,
        (Some(false), true) => Class::Hidden,
        (Some(true), false) => Class::Eligible,
        (Some(true), true) => Class::Unattributed,
    }
}
fn reentry(
    previous: Option<Seen>,
    frame: u32,
    mesh: AssetId<Mesh>,
    epoch: u64,
    class: Class,
) -> bool {
    previous.is_some_and(|p| {
        p.frame.wrapping_add(1) == frame
            && p.mesh == mesh
            && p.epoch == epoch
            && p.class == Class::Hidden
            && matches!(class, Class::Eligible | Class::Unattributed)
    })
}
impl Census {
    fn new(budget: u32) -> Self {
        Self {
            budget,
            observed: 0,
            call: 0,
            frame: None,
            camera: None,
            finished: false,
            duplicate: false,
            rows: BTreeMap::new(),
            epochs: BTreeMap::new(),
            seen: BTreeMap::new(),
            events: MessageCursor::default(),
            event_loss: false,
            overflow_rows: 0,
            no_producer: 0,
            bad_join: 0,
            incomplete_calls: 0,
            invalid_main_calls: 0,
            counts: Default::default(),
        }
    }
    fn active(&self) -> bool {
        self.observed < self.budget
    }
    fn drain(&mut self, messages: Option<&Messages<AssetEvent<Mesh>>>) {
        let Some(messages) = messages else {
            self.event_loss = true;
            return;
        };
        if self.events.missed_messages(messages) != 0 {
            self.event_loss = true;
        }
        for (index, event) in self.events.read(messages).enumerate() {
            if index == EVENT_CAP {
                self.event_loss = true;
                break;
            }
            let id = match *event {
                AssetEvent::Added { id }
                | AssetEvent::Modified { id }
                | AssetEvent::Removed { id }
                | AssetEvent::Unused { id }
                | AssetEvent::LoadedWithDependencies { id } => id,
            };
            if let Some(epoch) = self.epochs.get_mut(&id) {
                epoch.value += 1;
            }
        }
        // An overflow permanently invalidates epochs; never accept a partial event history.
        if self.event_loss {
            self.events.clear(messages);
        }
    }
    pub(super) fn begin(
        &mut self,
        frame: Option<u32>,
        messages: Option<&Messages<AssetEvent<Mesh>>>,
    ) {
        if !self.active() {
            return;
        }
        self.duplicate = self.finished || !self.rows.is_empty();
        self.rows.clear();
        self.call += 1;
        self.frame = frame;
        self.camera = None;
        self.finished = false;
        for epoch in self.epochs.values_mut() {
            epoch.known = true;
        }
        self.drain(messages);
    }
    pub(super) fn camera(&mut self, camera: &Transform) {
        if self.active() {
            let bits = camera_bits(camera);
            self.camera = bits
                .iter()
                .all(|b| b & 0x7f80_0000 != 0x7f80_0000)
                .then_some(bits);
        }
    }
    pub(super) fn produced(&mut self, entity: Entity, facing: &Ref<Facing>, mesh: &Ref<Mesh3d>) {
        if !self.active() {
            return;
        }
        let family = usize::from(facing.source.cylindrical);
        let vertices = facing.source.centers.len().min(facing.source.offsets.len()) as u64;
        let bytes = facing.vertices.len() as u64;
        let id = mesh.id();
        if self.rows.len() >= CAP || (!self.epochs.contains_key(&id) && self.epochs.len() >= CAP) {
            self.overflow_rows += 1;
            self.counts[family].classes[Class::Unknown as usize].add(vertices, bytes);
            return;
        }
        let epoch = self.epochs.entry(id).or_insert(Epoch {
            value: 0,
            known: false,
        });
        let row = Produced {
            mesh: id,
            ticks: [facing.last_changed().get(), mesh.last_changed().get()],
            epoch: epoch.value,
            epoch_known: epoch.known,
            family,
            vertices,
            bytes,
            membership: 0,
        };
        if self.rows.insert(entity, row).is_some() {
            self.duplicate = true;
        }
    }
    pub(super) fn no_position(&mut self, cylindrical: bool) {
        if self.active() {
            self.counts[usize::from(cylindrical)].no_position += 1;
        }
    }
    pub(super) fn finish(&mut self) {
        if self.active() {
            self.finished = true;
        }
    }
    fn mark(&mut self, entities: impl IntoIterator<Item = Entity>, bit: u8) {
        for entity in entities {
            if let Some(row) = self.rows.get_mut(&entity) {
                row.membership |= bit;
            }
        }
    }
}
fn active(run: Option<Res<Census>>) -> bool {
    run.is_some_and(|r| r.active())
}
pub(super) fn install(app: &mut App) {
    let Ok(value) = std::env::var("GRID_FACING_VISIBILITY") else {
        return;
    };
    let Some(n) = budget(&value) else {
        eprintln!("facing_visibility disabled=invalid_config expected=N budget=1..360");
        return;
    };
    println!(
        "facing_visibility version=1 budget={n} cap={CAP} allocation_generation=unobserved upload_success=unobserved"
    );
    app.insert_resource(Census::new(n)).add_systems(
        PostUpdate,
        observe
            .after(VisibilitySystems::MarkNewlyHiddenEntitiesInvisible)
            .after(AssetEventSystems)
            .run_if(active),
    );
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn observe(
    mut commands: Commands,
    mut run: ResMut<Census>,
    frame: Option<Res<FrameCount>>,
    messages: Option<Res<Messages<AssetEvent<Mesh>>>>,
    viewer: Query<&Transform, With<crate::ViewerCamera>>,
    foliage: Query<(
        Entity,
        Ref<Facing>,
        Ref<Mesh3d>,
        Option<&ViewVisibility>,
        Has<NoCpuCulling>,
    )>,
    cameras: Query<(
        &Camera,
        Option<&VisibleEntities>,
        Has<crate::ViewerCamera>,
        Has<crate::scene_probe::ProbeCamera>,
        Has<NoCpuCulling>,
    )>,
    lights: Query<
        (
            Option<&DirectionalLight>,
            Option<&PointLight>,
            Option<&SpotLight>,
            Option<&CascadesVisibleEntities>,
            Option<&CubemapVisibleEntities>,
            Option<&VisibleMeshEntities>,
        ),
        Or<(With<DirectionalLight>, With<PointLight>, With<SpotLight>)>,
    >,
) {
    run.observed += 1;
    run.drain(messages.as_deref());
    let current_frame = frame.as_deref().map(|f| f.0);
    let joined = run.finished
        && !run.duplicate
        && run.frame.is_some()
        && run.frame == current_frame
        && run.camera.is_some()
        && viewer.single().ok().map(camera_bits) == run.camera;
    if !run.finished {
        run.no_producer += 1;
    }
    if !joined {
        run.bad_join += 1;
    }
    let mut complete = true;
    let mut camera_counts = [0u32; 4]; // main, probe, other, NoCpuCulling cameras
    for (camera, visible, main, probe, no_cpu) in &cameras {
        if !camera.is_active {
            continue;
        }
        let bit = if main {
            camera_counts[0] += 1;
            1
        } else if probe {
            camera_counts[1] += 1;
            2
        } else {
            camera_counts[2] += 1;
            4
        };
        camera_counts[3] += u32::from(no_cpu);
        let Some(visible) = visible else {
            complete = false;
            continue;
        };
        run.mark(visible.get(TypeId::of::<Mesh3d>()).iter().copied(), bit);
    }
    let single_active_main = camera_counts[0] == 1;
    complete &= single_active_main;
    run.invalid_main_calls += u32::from(!single_active_main);
    let mut shadow_lights = 0u32;
    for (directional, point, spot, cascades, cube, visible) in &lights {
        if directional.is_some_and(|l| l.shadow_maps_enabled) {
            shadow_lights += 1;
            if let Some(cascades) = cascades {
                for lists in cascades.entities.values() {
                    for list in lists {
                        run.mark(list.entities.iter().copied(), 8);
                    }
                }
            } else {
                complete = false;
            }
        }
        if point.is_some_and(|l| l.shadow_maps_enabled) {
            shadow_lights += 1;
            if let Some(cube) = cube {
                for list in cube.iter() {
                    run.mark(list.entities.iter().copied(), 8);
                }
            } else {
                complete = false;
            }
        }
        if spot.is_some_and(|l| l.shadow_maps_enabled) {
            shadow_lights += 1;
            if let Some(visible) = visible {
                run.mark(visible.entities.iter().copied(), 8);
            } else {
                complete = false;
            }
        }
    }
    run.incomplete_calls += u32::from(!complete);
    // Move bounded metadata ownership, never geometry. BTreeMap::clear below frees
    // nodes: this observer does not preserve node allocations or claim neutral cost.
    let mut rows = std::mem::take(&mut run.rows);
    for (&entity, row) in &rows {
        let late = foliage.get(entity).ok();
        let flag = late.as_ref().and_then(|(_, _, _, v, _)| v.map(|v| v.get()));
        let components_match = late
            .as_ref()
            .is_some_and(|(_, f, m, _, _)| component_matches(row, f, m));
        let epoch_match = row.epoch_known
            && !run.event_loss
            && run
                .epochs
                .get(&row.mesh)
                .is_some_and(|e| e.value == row.epoch);
        let class = classify(
            joined && components_match && epoch_match,
            complete,
            flag,
            row.membership,
        );
        let previous = run.seen.get(&entity).copied();
        let is_reentry =
            current_frame.is_some_and(|f| reentry(previous, f, row.mesh, row.epoch, class));
        let c = &mut run.counts[row.family];
        match flag {
            Some(true) => c.flag_true += 1,
            Some(false) => c.flag_false += 1,
            None => c.flag_missing += 1,
        }
        c.classes[class as usize].add(row.vertices, row.bytes);
        if matches!(class, Class::Hidden | Class::Eligible | Class::Unattributed) {
            c.joined_masks[row.membership as usize].add(row.vertices, row.bytes);
        }
        for bit in 0..4 {
            if row.membership & (1 << bit) != 0 {
                c.membership[bit].add(row.vertices, row.bytes);
            }
        }
        c.first += u64::from(previous.is_none());
        c.replaced += u64::from(previous.is_some_and(|p| p.mesh != row.mesh));
        c.changed += u64::from(!components_match || !epoch_match);
        c.components_changed += u64::from(!components_match);
        c.asset_epoch_unknown += u64::from(!epoch_match);
        c.reentry += u64::from(is_reentry);
        c.no_cpu_culling += u64::from(late.as_ref().is_some_and(|(_, _, _, _, no_cpu)| *no_cpu));
        c.gap_visible += u64::from(
            previous.is_some_and(|p| current_frame.is_some_and(|f| p.frame.wrapping_add(1) != f))
                && matches!(class, Class::Eligible | Class::Unattributed),
        );
        if let Some(frame) = current_frame
            && (run.seen.len() < CAP || run.seen.contains_key(&entity))
        {
            run.seen.insert(
                entity,
                Seen {
                    frame,
                    mesh: row.mesh,
                    epoch: row.epoch,
                    class,
                },
            );
        } else {
            run.counts[row.family].history_untracked += 1;
        }
    }
    // Late-only entities (or cap-untracked rows) have no same-call producer join.
    // Count separately; do not double-weight producer overflow already counted Unknown.
    for (entity, facing, _, _, _) in &foliage {
        if !rows.contains_key(&entity) {
            run.counts[usize::from(facing.source.cylindrical)].late_unjoined += 1;
        }
    }
    rows.clear();
    run.rows = rows;
    run.finished = false;
    if run.observed == 240 || run.observed == run.budget {
        println!(
            "facing_visibility version=1 observed={} producer_call={} frame={current_frame:?} budget_exhausted={} no_producer={} bad_join={} incomplete_calls={} invalid_main_calls={} event_loss={} overflow_rows={} attribution_complete={complete} active_main_probe_other_nocpu={camera_counts:?} shadow_lights={shadow_lights} allocation_generation=unobserved upload_success=unobserved classes=unknown,contradiction,hidden,eligible,unattributed membership=main,probe,other,shadow sphere={:?} cylinder={:?}",
            run.observed,
            run.call,
            run.observed == run.budget,
            run.no_producer,
            run.bad_join,
            run.incomplete_calls,
            run.invalid_main_calls,
            run.event_loss,
            run.overflow_rows,
            run.counts[0],
            run.counts[1]
        );
    }
    if !run.active() {
        commands.remove_resource::<Census>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_bounded_config() {
        for s in ["", "0", "361", "3600", "+1", "01", " 1", "1 ", "keys:360"] {
            assert_eq!(budget(s), None);
        }
        assert_eq!(budget("1"), Some(1));
        assert_eq!(budget("360"), Some(360));
    }
    #[test]
    fn unknown_and_shadow_only_cannot_become_hidden() {
        assert_eq!(classify(false, true, Some(false), 0), Class::Unknown);
        assert_eq!(classify(true, false, Some(false), 0), Class::Unknown);
        assert_eq!(classify(true, true, None, 0), Class::Unknown);
        assert_eq!(classify(true, true, Some(true), 8), Class::Eligible);
        assert_eq!(classify(true, true, Some(false), 1), Class::Contradiction);
        assert_eq!(classify(true, true, Some(true), 0), Class::Unattributed);
    }
    fn mesh() -> Mesh {
        let mut m = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0., 1., 0.]]);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_1, vec![[1., 2.]]);
        m
    }
    #[test]
    fn event_epoch_changes_for_same_handle_without_changing_component() {
        let mut assets = Assets::<Mesh>::default();
        let h = assets.add(mesh());
        let mut messages = Messages::<AssetEvent<Mesh>>::default();
        let mut run = Census::new(2);
        run.epochs.insert(
            h.id(),
            Epoch {
                value: 0,
                known: true,
            },
        );
        messages.write(AssetEvent::Modified { id: h.id() });
        run.drain(Some(&messages));
        assert_eq!(run.epochs[&h.id()].value, 1);
        assert!(!run.event_loss);
        run.drain(Some(&messages));
        assert_eq!(run.epochs[&h.id()].value, 1);
    }
    #[test]
    fn replacement_gap_and_unknown_are_not_ordinary_reentry() {
        let mut assets = Assets::<Mesh>::default();
        let a = assets.add(mesh());
        let b = assets.add(mesh());
        let prev = Seen {
            frame: 10,
            mesh: a.id(),
            epoch: 2,
            class: Class::Hidden,
        };
        assert!(reentry(Some(prev), 11, a.id(), 2, Class::Eligible));
        assert!(!reentry(Some(prev), 11, b.id(), 2, Class::Eligible));
        assert!(!reentry(Some(prev), 11, a.id(), 3, Class::Eligible));
        assert!(!reentry(Some(prev), 12, a.id(), 2, Class::Eligible));
        assert!(!reentry(Some(prev), 11, a.id(), 2, Class::Unknown));
    }
    #[test]
    fn same_handle_component_replacement_breaks_producer_join() {
        fn producer(q: Query<(Entity, Ref<Facing>, Ref<Mesh3d>)>, mut run: ResMut<Census>) {
            let messages = Messages::<AssetEvent<Mesh>>::default();
            run.begin(Some(0), Some(&messages));
            let (e, facing, mesh) = q.single().unwrap();
            run.produced(e, &facing, &mesh);
            run.finish();
        }
        fn replace(mut q: Query<&mut Mesh3d>) {
            let mut mesh = q.single_mut().unwrap();
            let handle = mesh.0.clone();
            *mesh = Mesh3d(handle);
        }
        fn check(
            q: Query<(Entity, Ref<Facing>, Ref<Mesh3d>)>,
            run: Res<Census>,
            mut seen: ResMut<Observed>,
        ) {
            let (e, facing, mesh) = q.single().unwrap();
            seen.0
                .push(component_matches(&run.rows[&e], &facing, &mesh));
        }
        let source_mesh = mesh();
        let mut assets = Assets::<Mesh>::default();
        let handle = assets.add(mesh());
        let facing = super::super::FacingSource::from_mesh("trees/facing/test", &source_mesh)
            .unwrap()
            .placed(Mat4::IDENTITY);
        let mut app = App::new();
        app.init_resource::<Observed>()
            .insert_resource(Census::new(1));
        app.world_mut().spawn((facing, Mesh3d(handle)));
        app.add_systems(Update, (producer, replace).chain());
        app.add_systems(PostUpdate, check);
        app.update();
        assert_eq!(app.world().resource::<Observed>().0, [false]);
    }
    #[test]
    fn final_budget_removes_observer_state_even_without_a_producer() {
        let mut app = App::new();
        app.insert_resource(Census::new(2));
        app.add_systems(PostUpdate, observe.run_if(active));
        app.update();
        assert_eq!(app.world().resource::<Census>().observed, 1);
        app.update();
        assert!(!app.world().contains_resource::<Census>());
        app.update(); // Optional run condition remains valid after resource removal.
    }
    #[test]
    fn observer_requires_exactly_one_active_main_camera() {
        #[derive(Resource)]
        struct RequestedMainCount(u32);
        #[derive(Component)]
        struct ExtraMain;
        fn producer(
            frame: Res<FrameCount>,
            viewer: Query<&Transform, With<crate::ViewerCamera>>,
            foliage: Query<(Entity, Ref<Facing>, Ref<Mesh3d>)>,
            messages: Res<Messages<AssetEvent<Mesh>>>,
            mut run: ResMut<Census>,
        ) {
            run.begin(Some(frame.0), Some(&messages));
            run.camera(viewer.single().unwrap());
            for (e, facing, mesh) in &foliage {
                run.produced(e, &facing, &mesh);
            }
            run.finish();
        }
        fn late_extra(mut commands: Commands, count: Res<RequestedMainCount>) {
            if count.0 == 2 {
                commands.spawn((
                    ExtraMain,
                    crate::ViewerCamera,
                    Camera::default(),
                    Transform::IDENTITY,
                    VisibleEntities::default(),
                ));
            }
        }
        fn clear_extra(mut commands: Commands, q: Query<Entity, With<ExtraMain>>) {
            for e in &q {
                commands.entity(e).despawn();
            }
        }
        for active_main in [0, 1, 2] {
            let source_mesh = mesh();
            let mut assets = Assets::<Mesh>::default();
            let handle = assets.add(mesh());
            let facing = super::super::FacingSource::from_mesh("trees/facing/test", &source_mesh)
                .unwrap()
                .placed(Mat4::IDENTITY);
            let mut app = App::new();
            app.add_plugins(bevy::diagnostic::FrameCountPlugin);
            app.insert_resource(Census::new(3))
                .insert_resource(RequestedMainCount(active_main))
                .init_resource::<Messages<AssetEvent<Mesh>>>();
            app.world_mut().spawn((
                crate::ViewerCamera,
                Camera {
                    is_active: active_main != 0,
                    ..default()
                },
                Transform::IDENTITY,
                VisibleEntities::default(),
            ));
            app.world_mut()
                .spawn((facing, Mesh3d(handle), ViewVisibility::HIDDEN));
            app.add_systems(Update, producer);
            app.add_systems(PostUpdate, (late_extra, observe).chain());
            app.add_systems(Last, clear_extra);
            app.update();
            app.update(); // The first mesh epoch is explicitly unknown.
            let run = app.world().resource::<Census>();
            if active_main == 1 {
                assert_eq!(run.invalid_main_calls, 0);
                assert_eq!(run.counts[0].classes[Class::Hidden as usize].rows, 1);
            } else {
                assert_eq!(run.invalid_main_calls, 2);
                assert_eq!(run.incomplete_calls, 2);
                assert_eq!(run.counts[0].classes[Class::Hidden as usize].rows, 0);
                assert_eq!(run.counts[0].classes[Class::Unknown as usize].rows, 2);
                if active_main == 0 {
                    assert_eq!(run.bad_join, 0);
                }
            }
        }
    }
    #[derive(Resource, Default)]
    struct Observed(Vec<bool>);
    #[test]
    fn deferred_light_mark_precedes_late_observer() {
        use bevy::camera::visibility::SetViewVisibility;
        fn mark(mut commands: Commands, q: Query<Entity, With<ViewVisibility>>) {
            let e = q.single().unwrap();
            commands.queue(move |world: &mut World| {
                world.get_mut::<ViewVisibility>(e).unwrap().set_visible();
            });
        }
        fn check(q: Query<&ViewVisibility>, mut seen: ResMut<Observed>) {
            seen.0.push(q.single().unwrap().get());
        }
        let mut app = App::new();
        app.init_resource::<Observed>();
        app.configure_sets(
            PostUpdate,
            (
                VisibilitySystems::CheckVisibility,
                VisibilitySystems::MarkNewlyHiddenEntitiesInvisible,
            )
                .chain(),
        );
        app.add_systems(Update, check);
        app.add_systems(PostUpdate, mark.in_set(VisibilitySystems::CheckVisibility));
        app.add_systems(
            PostUpdate,
            check.after(VisibilitySystems::MarkNewlyHiddenEntitiesInvisible),
        );
        app.world_mut().spawn(ViewVisibility::HIDDEN);
        app.update();
        assert_eq!(app.world().resource::<Observed>().0, [false, true]);
    }
}
