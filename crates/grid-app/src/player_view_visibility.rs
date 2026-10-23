//! Source bumper-view selected-model suppression, adapted only to the main view.
//! Original stable policy: 607BC0 -> 99C2F0 -> 9CB0B0 -> 8F7480 -> 8F4F30.
//! Shadows/probes retain their existing lists; their original pass policy is unresolved.
use bevy::{
    camera::visibility::{VisibilitySystems, VisibleEntities},
    prelude::*,
};
use std::{any::TypeId, collections::HashSet};

pub(crate) fn install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        filter_player_model.after(VisibilitySystems::CheckVisibility),
    );
}

fn filter_player_model(
    cameras: Option<Res<super::camera::Cameras>>,
    sources: Query<(Entity, &super::track_surface::SourcePart), Without<super::OpponentPart>>,
    parents: Query<&ChildOf>,
    hulls: Query<(Entity, &super::wheel_exposure::Hull)>,
    mut views: Query<&mut VisibleEntities, With<super::ViewerCamera>>,
    mut roots: Local<HashSet<Entity>>,
) {
    if !cameras.is_some_and(|c| c.hides_player_model()) {
        return;
    }
    // Reuse allocation but refresh ownership: added/removed/reparented proxies
    // cannot leave a stale entity cache. No mesh/material/visibility mutation.
    roots.clear();
    roots.extend(
        sources
            .iter()
            .filter_map(|(entity, part)| part.0.starts_with("car0/").then_some(entity)),
    );
    if roots.is_empty() {
        return;
    }
    // This renderer is world-space with an explicit owner, not a transform child.
    for (entity, hull) in &hulls {
        if roots.contains(&hull.source) {
            roots.insert(entity);
        }
    }
    for mut visible in &mut views {
        visible.get_mut(TypeId::of::<Mesh3d>()).retain(|entity| {
            !roots.contains(entity)
                && !parents
                    .iter_ancestors::<ChildOf>(*entity)
                    .any(|parent| roots.contains(&parent))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Component)]
    struct OtherClass;
    #[derive(Resource)]
    struct FreshVisible(Vec<Entity>);
    // Stand-in for the NORMAL per-view visibility refill, not a source draw proof.
    fn refill(source: Res<FreshVisible>, mut views: Query<&mut VisibleEntities>) {
        for mut view in &mut views {
            *view.get_mut(TypeId::of::<Mesh3d>()) = source.0.clone();
        }
    }
    fn show(app: &mut App, ident: &str, back: bool) {
        let mut camera = app
            .world_mut()
            .resource_mut::<super::super::camera::Cameras>();
        camera.current = camera
            .views
            .0
            .iter()
            .position(|v| v.ident == ident)
            .unwrap();
        camera.frame(
            &super::super::camera::Sample::moving(Mat4::IDENTITY, Vec3::ZERO, 1, 0),
            back,
            super::super::camera::Look::default(),
            |v| v,
        );
    }
    fn list(app: &App, entity: Entity) -> Vec<Entity> {
        app.world()
            .get::<VisibleEntities>(entity)
            .unwrap()
            .get(TypeId::of::<Mesh3d>())
            .to_vec()
    }
    #[test]
    fn selected_bumper_filters_only_main_mesh_class_and_rebuilds_on_return() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let mut views = super::super::camera::Views::fallback();
        let base = views.0[0].clone();
        views.0.clear();
        for ident in [
            "bumper",
            "bumper_reverse",
            "chase_close",
            "bonnet",
            "head-cam",
        ] {
            let mut v = base.clone();
            v.ident = ident.into();
            v.hidden = ident == "bumper_reverse";
            v.reverse = None;
            views.0.push(v);
        }
        views.0[0].reverse = Some(1);
        app.insert_resource(super::super::camera::Cameras::new(
            views,
            "chase_close",
            None,
        ));
        let player = app
            .world_mut()
            .spawn((
                Mesh3d::default(),
                super::super::track_surface::SourcePart("car0/Car Body Paint".into()),
                Visibility::Inherited,
            ))
            .id();
        let proxy = app
            .world_mut()
            .spawn((Mesh3d::default(), ChildOf(player)))
            .id();
        let child = app
            .world_mut()
            .spawn((Mesh3d::default(), ChildOf(proxy)))
            .id();
        let opponent = app
            .world_mut()
            .spawn((
                Mesh3d::default(),
                super::super::track_surface::SourcePart("car1/Car Body Paint".into()),
            ))
            .id();
        let track = app
            .world_mut()
            .spawn((
                Mesh3d::default(),
                super::super::track_surface::SourcePart("don_road".into()),
            ))
            .id();
        let hull_transform = Transform::from_xyz(20., 30., 40.);
        let hull = app
            .world_mut()
            .spawn((
                Mesh3d::default(),
                super::super::wheel_exposure::Hull::visibility_fixture(player),
                hull_transform,
            ))
            .id();
        let opponent_hull = app
            .world_mut()
            .spawn((
                Mesh3d::default(),
                super::super::wheel_exposure::Hull::visibility_fixture(opponent),
            ))
            .id();
        let all = vec![player, proxy, child, opponent, track, hull, opponent_hull];
        let mut main_visible = VisibleEntities::default();
        main_visible
            .get_mut(TypeId::of::<OtherClass>())
            .push(player);
        let main = app
            .world_mut()
            .spawn((super::super::ViewerCamera, main_visible))
            .id();
        let probe = app.world_mut().spawn(VisibleEntities::default()).id();
        let light = app
            .world_mut()
            .spawn(bevy::camera::visibility::VisibleMeshEntities {
                entities: all.clone(),
            })
            .id();
        app.insert_resource(FreshVisible(all.clone())).add_systems(
            PostUpdate,
            refill.in_set(VisibilitySystems::CheckVisibility),
        );
        install(&mut app);
        for (ident, back, suppress) in [
            ("chase_close", false, false),
            ("bumper", false, true),
            ("bumper", true, true),
            ("bonnet", false, false),
            ("head-cam", false, false),
            ("chase_close", false, false),
        ] {
            show(&mut app, ident, back);
            app.update();
            assert_eq!(
                list(&app, main),
                if suppress {
                    vec![opponent, track, opponent_hull]
                } else {
                    all.clone()
                }
            );
            assert_eq!(list(&app, probe), all);
            assert_eq!(
                app.world()
                    .get::<bevy::camera::visibility::VisibleMeshEntities>(light)
                    .unwrap()
                    .entities,
                all
            );
            assert_eq!(
                app.world()
                    .get::<VisibleEntities>(main)
                    .unwrap()
                    .get(TypeId::of::<OtherClass>()),
                [player]
            );
            assert_eq!(
                *app.world().get::<Visibility>(player).unwrap(),
                Visibility::Inherited
            );
            assert!(app.world().get::<ChildOf>(hull).is_none());
            assert_eq!(*app.world().get::<Transform>(hull).unwrap(), hull_transform);
        }
        // A proxy can acquire a different owner; the next bumper pass follows
        // current ChildOf data, not its name or a permanently cached car tag.
        app.world_mut().entity_mut(proxy).insert(ChildOf(opponent));
        show(&mut app, "bumper", false);
        app.update();
        assert_eq!(
            list(&app, main),
            vec![proxy, child, opponent, track, opponent_hull]
        );
    }
}
