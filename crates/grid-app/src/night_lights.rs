//! The original's night lights (NightLights pass, docs/NIGHT_LIGHTS.md): the
//! track's street lights (`street_lights.xml`, 0x72C460) and every car's
//! headlight beam (0x736020), drawn as one extra diffuse term in the lit
//! shader. Shared by every night route: the lights come from track and car
//! data, the timing from the route's time-of-day mode.
//!
//! Each light is a shadowless Bevy spot light used only as a carrier for
//! clustering; the patched spot loop skips Bevy's own lighting for it and
//! adds the original's term instead (night_lights.wgsl): a rectangle of
//! half-angle ratio 2, linear range fall-off, the `headlightdirectional`
//! facing bias and the projected cookie (left half of `headlight.dds` for
//! headlights, right half for street lights). The sum is clamped to 1 and
//! scaled by `lightColour * lightStrength`, as the original's shader does.
use crate::headlight_frame::Prepared;
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    pbr::decal::clustered::{self, RenderClusteredDecals},
    prelude::*,
    render::{
        Extract, ExtractSchedule, RenderApp,
        render_asset::RenderAssets,
        renderer::{RenderAdapter, RenderDevice},
        sync_world::RenderEntity,
        texture::GpuImage,
    },
    shader::{ShaderImport, Source as ShaderSource},
    transform::TransformSystems,
};
use grid_formats::{pssg, texture};
use grid_tools::{
    route::Route,
    street_lights::Light,
    time_of_day::{Headlights, Mode, Timeline},
};
use std::path::Path;

/// `shadow_depth_bias` of a night light (no shadow map ever reads it): the
/// headlight half of the cookie atlas, and the street-light half.
const HEADLIGHT_MARKER: f32 = -31415.;
const STREET_MARKER: f32 = -31416.;
/// Clustered-decal tag of a night light's cookie.
const NIGHT_TAG: u32 = 0x4E49_4748;
const MARKER: &str = "// GRID_NIGHT_LIGHTS_V1";
/// The Bevy 0.19.1 sources the patch was written against (source_projector.rs
/// pins the same pair).
const FUNCTIONS_SHA256: &str = "13a5344ef4e0fa967e05978489d0909dd0c1862cf7a989ce0882dbc8de9a55dd";
const LIGHTING_SHA256: &str = "ec609c97f380d1d18d180763ffffb253a1349a4ab4739435bccf7ca829f4c8b2";
/// Lights further than this from the camera in x or z are not drawn (0x72C460).
const CULL: f32 = 500.;
/// Headlight range of the focus car and of every other car (0xDB79D8, 0xDB79D4).
const FOCUS_RANGE: f32 = 110.;
const OTHER_RANGE: f32 = 60.;
/// `headlightdirectional` when the route has no timeline (0x72CF10).
const DEFAULT_DIRECTIONAL: f32 = 0.3;

#[derive(Component, Clone, Copy)]
enum Source {
    Street { brightness: f32 },
    Headlight { slot: usize },
}

#[derive(Resource)]
struct Night {
    mode: Mode,
    /// Bevy linear units of one original light unit (the emissive unit).
    unit: f32,
    headlights: Option<Prepared>,
    /// Static scenes: the player's grid pose, its own lamp fade and timeline.
    still: Option<Still>,
    cookie: Option<Handle<Image>>,
}

struct Still {
    pose: Mat4,
    fade: Headlights,
    timeline: Option<Timeline>,
    time: f64,
}

#[derive(Resource, Default)]
struct Admission {
    shader: bool,
    /// The installed Bevy shaders did not take the patch: the pass stays off.
    refused: bool,
    decals: bool,
}

/// What `install` needs from the scene setup.
pub(crate) struct Setup {
    pub mode: Mode,
    pub unit: f32,
    pub street: Vec<Light>,
    /// Car beams (none without a drawn car).
    pub headlights: Option<Prepared>,
    /// Static scenes: the player's pose (identity without a car); None when driving.
    pub still: Option<Mat4>,
    pub cookie: Option<Image>,
    pub timeline: Option<Timeline>,
}

/// Loads the route's street lights, cookie atlas and timeline. Every route
/// that is dark at some time of day gets the pass; a route without
/// street_lights.xml still gets the car beams.
pub(crate) fn load(root: &Path, route: &Route) -> (Vec<Light>, Option<Image>, Option<Timeline>) {
    let street = grid_tools::street_lights::load(root, route).unwrap_or_else(|error| {
        eprintln!("night_lights street lights skipped: {error}");
        Vec::new()
    });
    let cookie = load_cookie(root, route)
        .inspect_err(|error| eprintln!("night_lights cookie skipped: {error}"))
        .ok();
    let timeline = grid_tools::time_of_day::Timeline::load(root, route).unwrap_or_else(|error| {
        eprintln!("night_lights timeline skipped: {error}");
        None
    });
    (street, cookie, timeline)
}

/// The deterministic per-car lamp delays the driving path uses
/// (vehicle_lights::State).
pub(crate) fn still_headlights(mode: Mode, cars: usize) -> Headlights {
    crate::vehicle_lights::State::new(mode, cars, None).headlights
}

pub(crate) fn install(app: &mut App, setup: Setup) -> Result<(), String> {
    if setup.mode == Mode::None {
        return Err("night lights need a night route".into());
    }
    if !setup.unit.is_finite() || setup.unit <= 0. {
        return Err("night light unit must be finite positive".into());
    }
    let street = setup.street.iter().filter(|l| l.name.is_empty()).count();
    let cookie = setup
        .cookie
        .map(|image| app.world_mut().resource_mut::<Assets<Image>>().add(image));
    let world = app.world_mut();
    for light in setup.street.iter().filter(|l| l.name.is_empty()) {
        let position = Vec3::from_array(light.position);
        let direction = Vec3::from_array(light.direction);
        if !position.is_finite() || direction.length_squared() < 0.5 {
            continue;
        }
        let transform = Transform::from_translation(position).looking_to(direction, street_up(direction));
        world.spawn((
            carrier(light.range, STREET_MARKER),
            transform,
            Visibility::Hidden,
            Source::Street { brightness: light.brightness },
        ));
    }
    let slots = setup.headlights.as_ref().map_or(0, Prepared::len);
    for slot in 0..slots {
        world.spawn((
            carrier(if slot == 0 { FOCUS_RANGE } else { OTHER_RANGE }, HEADLIGHT_MARKER),
            Transform::IDENTITY,
            Visibility::Hidden,
            Source::Headlight { slot },
        ));
    }
    // A static scene holds the route's start time; its lamps are switched
    // as a race start would switch them (snapped, no fade-in).
    let still = setup.still.map(|pose| {
        let time = crate::sky::static_start();
        let wanted = setup.mode == Mode::ForcedNight
            || setup
                .timeline
                .as_ref()
                .and_then(|t| t.flag("enableHeadlights", time))
                .unwrap_or(false);
        let mut fade = still_headlights(setup.mode, slots.max(1));
        fade.step(wanted, 0., 0.);
        Still {
            pose,
            fade,
            timeline: setup.timeline,
            time,
        }
    });
    println!(
        "night_lights version=1 mode={:?} street={street} headlights={slots} cookie={} unit={:.1}",
        setup.mode,
        cookie.is_some(),
        setup.unit
    );
    app.insert_resource(Night {
        mode: setup.mode,
        unit: setup.unit,
        headlights: setup.headlights,
        still,
        cookie,
    })
    .init_resource::<Admission>()
    .add_systems(Startup, check_device)
    .add_systems(Update, patch_loaded_shader)
    .add_systems(PostUpdate, present.before(TransformSystems::Propagate));
    app.get_sub_app_mut(RenderApp)
        .ok_or("night lights need the render app")?
        .add_systems(ExtractSchedule, extract_cookies.after(clustered::extract_decals));
    Ok(())
}

fn carrier(range: f32, marker: f32) -> SpotLight {
    SpotLight {
        color: Color::BLACK,
        intensity: 0.,
        range,
        radius: DEFAULT_DIRECTIONAL,
        shadow_maps_enabled: false,
        contact_shadows_enabled: false,
        shadow_depth_bias: marker,
        shadow_normal_bias: 0.,
        shadow_map_near_z: 0.,
        // The cone encloses the source rectangle |x|, |y| < 2z, corners included.
        outer_angle: (2. * 2_f32.sqrt()).atan(),
        inner_angle: 0.,
        ..default()
    }
}

/// The street light's up: `cross(dir, normalize(cross(Y, dir)))`, `(1, 0, 0)`
/// when the light points straight up or down (0x72C460).
fn street_up(direction: Vec3) -> Vec3 {
    match Vec3::Y.cross(direction).try_normalize() {
        Some(side) => direction.cross(side),
        None => Vec3::X,
    }
}

/// The shared light values at the current time: `lightColour * lightStrength`
/// and `headlightdirectional` (defaults (1, 1, 1), 1 and 0.3).
fn shared_values(timeline: Option<&Timeline>, t: f64) -> (Vec3, f32) {
    let value = |name: &str, default: f32| timeline.and_then(|tl| tl.value(name, t)).unwrap_or(default);
    let colour = Vec3::new(
        value("lightColour_R", 1.),
        value("lightColour_G", 1.),
        value("lightColour_B", 1.),
    ) * value("lightStrength", 1.);
    (colour.max(Vec3::ZERO), value("headlightdirectional", DEFAULT_DIRECTIONAL))
}

/// A light's shader weight `3 * min(w, 1)` (0x736D80): street lights
/// `brightness * sat(F / 9)`, headlights `1.5 * clamp((F - d) * 4, .001, 1)`.
fn street_weight(brightness: f32, fade: &Headlights) -> f32 {
    3. * (brightness * fade.street_light_fade() as f32).min(1.)
}
fn headlight_weight(fade: &Headlights, slot: usize) -> f32 {
    fade.car(slot).map_or(0., |f| 3. * (1.5 * f as f32).min(1.))
}

type Lights<'w, 's> = Query<
    'w,
    's,
    (&'static Source, &'static mut SpotLight, &'static mut Transform, &'static mut Visibility),
    Without<crate::ViewerCamera>,
>;

#[allow(clippy::too_many_arguments)]
fn present(
    night: Res<Night>,
    admission: Res<Admission>,
    driving: Option<Res<crate::Driving>>,
    lamps: Option<Res<crate::vehicle_lights::State>>,
    event: Option<Res<crate::le_mans_event::LeMansEvent>>,
    camera: Query<&GlobalTransform, With<crate::ViewerCamera>>,
    mut lights: Lights,
    mut reported: Local<bool>,
) {
    let (fade, timeline, time) = match (&lamps, &night.still) {
        (Some(lamps), _) => (
            &lamps.headlights,
            event.as_ref().and_then(|e| e.timeline.as_ref()),
            event.as_ref().map_or(0., |e| e.time_of_day),
        ),
        (None, Some(still)) => (&still.fade, still.timeline.as_ref(), still.time),
        (None, None) => return,
    };
    let (colour, directional) = shared_values(timeline, time);
    let colour = colour * night.unit;
    let eye = camera.iter().next().map(GlobalTransform::translation);
    let (mut street_shown, mut beams_shown) = (0, 0);
    for (source, mut light, mut transform, mut visibility) in &mut lights {
        let weight = match *source {
            Source::Street { brightness } => street_weight(brightness, fade),
            Source::Headlight { slot } => {
                let pose = match (&driving, &night.still) {
                    (Some(d), _) => d.shown_pose(slot),
                    (None, Some(s)) if slot == 0 => Some(s.pose),
                    _ => None,
                };
                let frame = pose.and_then(|pose| {
                    night.headlights.as_ref()?.frame(slot, pose, night.mode).ok().flatten()
                });
                match frame {
                    Some(f) => {
                        *transform = Transform::from_translation(f.origin).looking_to(f.direction, f.up);
                        headlight_weight(fade, slot)
                    }
                    None => 0.,
                }
            }
        };
        let near = eye.is_none_or(|eye| {
            let d = transform.translation - eye;
            d.x.abs() <= CULL && d.z.abs() <= CULL
        });
        let shown = admission.shader && weight > 0. && near && colour.max_element() > 0.;
        let wanted = if shown { Visibility::Inherited } else { Visibility::Hidden };
        visibility.set_if_neq(wanted);
        if shown {
            match source {
                Source::Street { .. } => street_shown += 1,
                Source::Headlight { .. } => beams_shown += 1,
            }
            light.color = Color::LinearRgba(LinearRgba::rgb(colour.x, colour.y, colour.z));
            // Bevy divides a spot light's intensity by 4 pi at extraction.
            light.intensity = 4. * std::f32::consts::PI;
            light.shadow_map_near_z = weight;
            light.radius = directional;
        }
    }
    // Once, for smoke runs: the first frame that draws any night light.
    if !*reported && street_shown + beams_shown > 0 {
        *reported = true;
        println!(
            "night_lights shown street={street_shown} headlights={beams_shown} driving={}",
            driving.is_some()
        );
    }
}

fn check_device(
    device: Option<Res<RenderDevice>>,
    adapter: Option<Res<RenderAdapter>>,
    mut admission: ResMut<Admission>,
) {
    admission.decals = match (device, adapter) {
        (Some(d), Some(a)) => clustered::clustered_decals_are_usable(&d, &a),
        _ => false,
    };
    println!("night_lights cookie_decals={}", admission.decals);
}

type Carriers<'w, 's> = Query<
    'w,
    's,
    (RenderEntity, &'static GlobalTransform, &'static ViewVisibility, &'static SpotLight),
    With<Source>,
>;

/// Each drawn night light carries the cookie atlas as a clustered decal of
/// its own (render world only, so no surface draws it as a decal).
fn extract_cookies(
    admission: Extract<Res<Admission>>,
    night: Extract<Res<Night>>,
    lights: Extract<Carriers<'_, '_>>,
    images: Option<Res<RenderAssets<GpuImage>>>,
    mut decals: ResMut<RenderClusteredDecals>,
) {
    let Some(cookie) = night.cookie.as_ref().map(Handle::id) else {
        return;
    };
    if !(admission.shader && admission.decals) || !images.is_some_and(|i| i.get(cookie).is_some()) {
        return;
    }
    for (entity, transform, visible, light) in &lights {
        if !visible.get() || !transform.affine().is_finite() {
            continue;
        }
        decals.insert_decal(
            entity,
            [Some(cookie), None, None, None],
            transform.affine().inverse().into(),
            transform.translation(),
            light.range,
            NIGHT_TAG,
        );
    }
}

/// The full `headlight.dds` atlas of the track's headlights.pssg (left half
/// the headlight beam, right half the street light).
fn load_cookie(root: &Path, route: &Route) -> Result<Image, String> {
    let relative = route.track_file("headlights.pssg");
    let bytes = std::fs::read(root.join(&relative)).map_err(|e| format!("{relative}: {e}"))?;
    let tree = pssg::read_tree(&bytes).map_err(|e| e.to_string())?;
    let textures = texture::read_textures(&tree).map_err(|e| e.to_string())?;
    let source = ["headlight.dds", "headlight.tga"]
        .iter()
        .find_map(|name| textures.iter().find(|t| t.id.eq_ignore_ascii_case(name.as_bytes())))
        .ok_or("headlight.dds/tga absent")?;
    let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..ImageSamplerDescriptor::linear()
    });
    crate::texture_upload::image_linear(
        &source.base_dds(),
        sampler,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .map_err(|e| e.to_string())
}

fn sha(source: &str) -> String {
    grid_tools::private_parameters::hex(&grid_tools::private_parameters::sha256(source.as_bytes()))
}
fn unique_replace(source: &str, anchor: &str, replacement: &str) -> Result<String, String> {
    if source.matches(anchor).count() != 1 {
        return Err(format!("night lights: Bevy shader anchor {anchor:?} changed"));
    }
    Ok(source.replacen(anchor, replacement, 1))
}

/// The helper WGSL with its constants filled in.
fn helper() -> String {
    include_str!("night_lights.wgsl")
        .replace("GRID_NIGHT_HEADLIGHT", &format!("{HEADLIGHT_MARKER:.1}"))
        .replace("GRID_NIGHT_STREET", &format!("{STREET_MARKER:.1}"))
        .replace("GRID_NIGHT_TAG", &format!("{NIGHT_TAG}u"))
}

/// Patches bevy_pbr::lighting (the helper) and bevy_pbr::pbr_functions (the
/// spot loop skips night lights and sums their term; the clamped sum joins
/// the direct diffuse light).
fn patch_pair(lighting: &str, functions: &str) -> Result<(String, String), String> {
    if lighting.contains(MARKER)
        || functions.contains(MARKER)
        || sha(lighting) != LIGHTING_SHA256
        || sha(functions) != FUNCTIONS_SHA256
    {
        return Err("night lights: unsupported or already patched Bevy shaders".into());
    }
    let l = unique_replace(lighting, "fn point_light(\n", &(helper() + "\nfn point_light(\n"))?;
    let mut f = unique_replace(
        functions,
        "    // Spot lights (direct)\n",
        &format!(
            "{MARKER}\n    var grid_night_sum = vec3<f32>(0.0);\n    var grid_night_colour = vec3<f32>(0.0);\n    // Spot lights (direct)\n"
        ),
    )?;
    let start = f.find("    // Spot lights (direct)\n").ok_or("spot loop absent")?;
    let end = f[start..]
        .find("    // directional lights (direct)\n")
        .ok_or("directional boundary absent")?
        + start;
    let spot = unique_replace(
        &f[start..end],
        "        var shadow: f32 = 1.0;",
        r#"        if lighting::grid_night_light(light_id) {
            if enable_diffuse {
                grid_night_sum += lighting::grid_night_term(light_id, in.world_position.xyz, in.N);
                grid_night_colour = view_bindings::clustered_lights.data[light_id].color_inverse_square_range.rgb;
            }
            continue;
        }
        var shadow: f32 = 1.0;"#,
    )?;
    f.replace_range(start..end, &spot);
    f = unique_replace(
        &f,
        "    // directional lights (direct)\n",
        "    direct_light += clamp(grid_night_sum, vec3(0.0), vec3(1.0)) * grid_night_colour * diffuse_color * diffuse_occlusion;\n\n    // directional lights (direct)\n",
    )?;
    Ok((l, f))
}

fn patch_loaded_shader(mut shaders: ResMut<Assets<Shader>>, mut admission: ResMut<Admission>) {
    if admission.shader || admission.refused {
        return;
    }
    let mut selected = Vec::new();
    for path in ["bevy_pbr::lighting", "bevy_pbr::pbr_functions"] {
        let ids: Vec<_> = shaders
            .iter()
            .filter(|(_, s)| s.import_path == ShaderImport::Custom(path.into()))
            .map(|(id, _)| id)
            .collect();
        let [id] = ids[..] else {
            return;
        };
        selected.push((id, shaders.get(id).expect("listed shader").clone()));
    }
    fn wgsl(s: &Shader) -> Option<&str> {
        match &s.source {
            ShaderSource::Wgsl(x) => Some(x.as_ref()),
            _ => None,
        }
    }
    let (Some(l), Some(f)) = (wgsl(&selected[0].1), wgsl(&selected[1].1)) else {
        eprintln!("night_lights refused: Bevy lighting modules are not WGSL");
        admission.refused = true;
        return;
    };
    match patch_pair(l, f) {
        Ok((l, f)) => {
            for ((id, mut shader), source) in selected.into_iter().zip([l, f]) {
                shader.source = ShaderSource::Wgsl(source.into());
                shaders.insert(id, shader).expect("same-id shader replacement");
            }
            admission.shader = true;
            println!("night_lights shader=patched");
        }
        Err(error) => {
            eprintln!("night_lights refused: {error}");
            admission.refused = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CPU reference of the shader term for one light (night_lights.wgsl).
    fn term(light: &Transform, range: f32, d: f32, weight: f32, p: Vec3, n: Vec3) -> f32 {
        let local = light.compute_affine().inverse().transform_point3(p);
        let q = -local;
        if q.z <= 0. || 0.5 * q.x.abs() >= q.z || 0.5 * q.y.abs() >= q.z {
            return 0.;
        }
        let to = p - light.translation;
        let facing = ((1. - d) * n.dot(-to.normalize()) + d).clamp(0., 1.);
        (1. - q.length() / range).clamp(0., 1.) * facing * weight
    }

    #[test]
    fn street_frame_matches_the_original_and_the_shader_fallback() {
        let dir = Vec3::new(0.3, -0.9, 0.2).normalize();
        let up = street_up(dir);
        assert!(up.dot(dir).abs() < 1e-6 && (up.length() - 1.).abs() < 1e-5);
        assert_eq!(street_up(Vec3::NEG_Y), Vec3::X);
        // Bevy's light space gives q = (cross(up, dir), -up, dir) . v.
        let t = Transform::IDENTITY.looking_to(dir, up);
        let v = Vec3::new(0.4, -3., 1.);
        let local = t.compute_affine().inverse().transform_point3(v);
        let q = Vec3::new(up.cross(dir).dot(v), -up.dot(v), dir.dot(v));
        assert!((-local - q).length() < 1e-5, "{local} {q}");
    }

    #[test]
    fn term_shape_is_a_ratio_two_rectangle_with_linear_falloff() {
        let light = Transform::IDENTITY.looking_to(Vec3::NEG_Y, Vec3::X);
        let floor = |x: f32, z: f32| term(&light, 10., 0.3, 3., Vec3::new(x, -5., z), Vec3::Y);
        assert!((floor(0., 0.) - 1.5).abs() < 1e-5, "{}", floor(0., 0.));
        // The rectangle's corner is lit when the range reaches it.
        assert!(term(&light, 15., 0.3, 3., Vec3::new(9.9, -5., 9.9), Vec3::Y) > 0., "corner inside");
        assert_eq!(floor(10.1, 0.), 0.);
        assert_eq!(term(&light, 10., 0.3, 3., Vec3::new(0., 5., 0.), Vec3::Y), 0., "behind");
        // A surface edge-on to the light keeps the directional bias d; one
        // facing away gets sat(2d - 1), dark for d < 0.5.
        let edge_on = term(&light, 10., 0.3, 3., Vec3::new(0., -5., 0.), Vec3::X);
        assert!((edge_on - 0.5 * 0.3 * 3.).abs() < 1e-5, "{edge_on}");
        assert_eq!(term(&light, 10., 0.3, 3., Vec3::new(0., -5., 0.), Vec3::NEG_Y), 0.);
    }

    #[test]
    fn weights_follow_the_lamp_fade() {
        // Forced night starts with the lamps on; a timeline route starts off.
        assert_eq!(street_weight(1., &still_headlights(Mode::ForcedNight, 3)), 3.);
        let mut fade = still_headlights(Mode::Timeline, 3);
        assert_eq!(street_weight(1., &fade), 0.);
        assert_eq!(headlight_weight(&fade, 0), 0.);
        fade.step(true, 0., 0.);
        assert_eq!(street_weight(1., &fade), 3.);
        assert_eq!(street_weight(0.25, &fade), 0.75);
        assert_eq!(headlight_weight(&fade, 1), 3.);
        assert_eq!(shared_values(None, 0.), (Vec3::ONE, 0.3));
    }

    /// The per-frame system as the interactive window runs it: street lights
    /// culled to +-500 x/z around the camera, weights from the lamp fade, the
    /// beam placed at the car's headlight anchor.
    #[test]
    fn present_places_weights_and_culls_lights() {
        use bevy::ecs::system::RunSystemOnce;
        let prepared = crate::headlight_frame::Preparation::fixture(
            vec![(Vec3::new(0.7, 0.6, 2.), "fixture")],
            vec![Some(Mat4::IDENTITY)],
            vec![0],
        )
        .finish()
        .unwrap();
        let pose = Mat4::from_translation(Vec3::new(100., 0., 50.));
        let mut fade = still_headlights(Mode::ForcedNight, 1);
        fade.step(true, 0., 0.);
        let mut w = World::new();
        w.insert_resource(Night {
            mode: Mode::ForcedNight,
            unit: 2.,
            headlights: Some(prepared),
            still: Some(Still { pose, fade, timeline: None, time: 0. }),
            cookie: None,
        });
        w.insert_resource(Admission { shader: true, refused: false, decals: false });
        w.spawn((crate::ViewerCamera, GlobalTransform::from_translation(Vec3::new(0., 5., 0.))));
        let street = |w: &mut World, x: f32, b: f32| {
            w.spawn((
                carrier(10., STREET_MARKER),
                Transform::from_xyz(x, 6., 0.).looking_to(Vec3::NEG_Y, street_up(Vec3::NEG_Y)),
                Visibility::Hidden,
                Source::Street { brightness: b },
            ))
            .id()
        };
        let near = street(&mut w, 450., 0.25);
        let far = street(&mut w, 501., 1.);
        let beam = w
            .spawn((carrier(FOCUS_RANGE, HEADLIGHT_MARKER), Transform::IDENTITY, Visibility::Hidden, Source::Headlight { slot: 0 }))
            .id();
        w.run_system_once(present).unwrap();
        let get = |w: &World, e| (*w.get::<Visibility>(e).unwrap(), *w.get::<SpotLight>(e).unwrap(), *w.get::<Transform>(e).unwrap());
        let (v, l, _) = get(&w, near);
        assert_eq!(v, Visibility::Inherited);
        assert_eq!(l.shadow_map_near_z, 0.75);
        assert_eq!(l.radius, DEFAULT_DIRECTIONAL);
        assert_eq!(l.color, Color::LinearRgba(LinearRgba::rgb(2., 2., 2.)));
        assert_eq!(get(&w, far).0, Visibility::Hidden);
        let (v, l, t) = get(&w, beam);
        assert_eq!(v, Visibility::Inherited);
        assert_eq!(l.shadow_map_near_z, 3.);
        assert!((t.translation - Vec3::new(100.7, 0.6, 52.)).length() < 1e-4, "{}", t.translation);
        // Forced-night beam: normalize(0, -0.8, 0.9962) in car space (0x736020).
        let beam_dir = Vec3::new(0., -0.8, 0.9962).normalize();
        assert!((*t.forward() - beam_dir).length() < 1e-4, "{:?}", t.forward());
        // Lamps off: everything hidden.
        w.resource_mut::<Night>().still.as_mut().unwrap().fade = still_headlights(Mode::Timeline, 1);
        w.run_system_once(present).unwrap();
        assert_eq!(get(&w, near).0, Visibility::Hidden);
        assert_eq!(get(&w, beam).0, Visibility::Hidden);
    }

    #[test]
    fn helper_constants_are_filled() {
        let h = helper();
        for placeholder in ["GRID_NIGHT_HEADLIGHT", "GRID_NIGHT_STREET", "GRID_NIGHT_TAG"] {
            assert!(!h.contains(placeholder), "{placeholder} left");
        }
        assert!(h.contains("-31415.0") && h.contains("-31416.0") && h.contains("1313425224u"));
    }

    #[test]
    fn unknown_shaders_are_refused() {
        assert!(patch_pair("fn point_light(\n", "x").is_err());
    }

    #[test]
    #[ignore = "reads the Bevy 0.19.1 shader sources (GRID_BEVY_SHADER_DIR or the cargo registry)"]
    fn installed_bevy_shaders_take_the_patch_once() {
        // GRID_BEVY_SHADER_DIR, else bevy_pbr 0.19.1 as cargo unpacked it.
        let dir = std::env::var_os("GRID_BEVY_SHADER_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let home = std::env::var_os("CARGO_HOME").map(std::path::PathBuf::from).or_else(|| {
                    std::env::var_os("USERPROFILE")
                        .or_else(|| std::env::var_os("HOME"))
                        .map(|h| std::path::PathBuf::from(h).join(".cargo"))
                })?;
                std::fs::read_dir(home.join("registry/src"))
                    .ok()?
                    .flatten()
                    .map(|e| e.path().join("bevy_pbr-0.19.1/src/render"))
                    .find(|d| d.join("pbr_functions.wgsl").is_file())
            })
            .expect("set GRID_BEVY_SHADER_DIR: bevy_pbr 0.19.1 not found in the cargo registry");
        let l = std::fs::read_to_string(dir.join("pbr_lighting.wgsl")).unwrap();
        let f = std::fs::read_to_string(dir.join("pbr_functions.wgsl")).unwrap();
        let (pl, pf) = patch_pair(&l, &f).unwrap();
        assert!(pl.contains("fn grid_night_term(") && pl.contains("fn spot_light("));
        assert_eq!(pf.matches(MARKER).count(), 1);
        assert!(pf.contains("continue;\n        }\n        var shadow: f32 = 1.0;"));
        assert!(pf.contains("grid_night_colour * diffuse_color * diffuse_occlusion"));
        assert!(patch_pair(&pl, &pf).is_err());
    }
}
