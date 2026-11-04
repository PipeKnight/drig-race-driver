//! Default-off shared lamp signal adapter; source focus/sampler/photometry remain unresolved.
use crate::lamp_association::{BankKey, Package, Receipt};
use bevy::{asset::RenderAssetUsages, image::ImageSampler, prelude::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
pub(crate) mod runtime;
pub(crate) mod audit;
use runtime::State;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Live,
    Brake,
    Reverse,
    Headlights,
}
impl Mode {
    pub fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            "off" => Ok(Self::Off),
            "live" => Ok(Self::Live),
            "brake" => Ok(Self::Brake),
            "reverse" => Ok(Self::Reverse),
            "headlights" => Ok(Self::Headlights),
            _ => Err("--lamp-signals needs off, live, brake, reverse or headlights"),
        }
    }
}
#[derive(Resource, Default, Clone, bevy::render::extract_resource::ExtractResource)]
pub struct Atlas(pub(crate) BTreeMap<BankKey, Handle<Image>>);
/// Actual validated DDS retained by exact source bank, NOT a merged livery map.
#[derive(Resource)]
pub(crate) struct Prepared {
    banks: BTreeMap<BankKey, Vec<u8>>,
    slots: usize,
}
#[derive(Clone, Copy, Default, Debug)]
struct Fades([f32; 3]);
impl Fades {
    // Source constructor defaults; actual source setters can still change these.
    const CAPS: [f32; 3] = [0.7, 0.7, 1.];
    fn step(&mut self, dt: f32, on: [bool; 3], caps: [f32; 3]) -> [f32; 4] {
        let amount = dt.abs() * 20.;
        for i in 0..3 {
            self.0[i] = (self.0[i] + if on[i] { amount } else { -amount }).clamp(0., caps[i]);
        }
        [
            self.0[0],
            self.0[1],
            self.0[2],
            (self.0[0] + self.0[1]).min(2.),
        ]
    }
}

fn receipt_in(packages: &[Package], part: &crate::Part) -> Result<Option<Receipt>, String> {
    let mut found = None;
    for package in packages {
        if let Some(receipt) = package.receipt(part)? {
            if found.is_some() {
                return Err("ambiguous lamp geometry package receipt".into());
            }
            runtime::Receiver::from_receipt(0, &receipt).map_err(str::to_owned)?;
            found = Some(receipt);
        }
    }
    Ok(found)
}
pub(crate) fn receipt_for(car: &crate::Car, part: &crate::Part) -> Result<Option<Receipt>, String> {
    receipt_in(&car.lamp_associations, part)
}
pub(crate) fn low_receipt(
    package: Option<&Package>,
    part: &crate::Part,
) -> Result<Option<Receipt>, String> {
    match package {
        Some(package) => receipt_in(std::slice::from_ref(package), part),
        None => Err("low lamp geometry package unavailable".into()),
    }
}
fn receipts(car: &crate::Car) -> Result<Vec<Receipt>, String> {
    let mut all = Vec::new();
    for part in &car.parts {
        if let Some(receipt) = receipt_for(car, part)? {
            all.push(receipt);
        }
    }
    for low in &car.low_lods {
        for (_, part) in &low.parts {
            if let Some(receipt) = low_receipt(low.lamp_association.as_ref(), part)? {
                all.push(receipt);
            }
        }
    }
    Ok(all)
}
fn read_bank_dds(root: &Path, key: &BankKey) -> Result<Vec<u8>, String> {
    let bytes = crate::read(&root.join(&key.source)).map_err(|e| e.to_string())?;
    let tree = grid_formats::pssg::read_tree(&bytes).map_err(|e| e.to_string())?;
    let textures = grid_formats::texture::read_textures(&tree).map_err(|e| e.to_string())?;
    let matching: Vec<_> = textures
        .iter()
        .filter(|t| t.id == key.resource.as_bytes())
        .collect();
    let [texture] = matching.as_slice() else {
        return Err(format!(
            "actual bank {:?} resource {:?} is absent/ambiguous",
            key.source, key.resource
        ));
    };
    Ok(texture.validated_dds().0)
}
pub(crate) fn prepare(root: &Path, car: &crate::Car, slots: usize) -> Result<Prepared, String> {
    if slots == 0 {
        return Err("lamp slots require actual nonempty vehicle field".into());
    }
    let receipts = receipts(car)?;
    for receipt in &receipts {
        println!(
            "lamp_signal_receipt geometry={:?} draw={:?} lod={} palette={} bank={:?} resource={:?} namespaced_key={:?}",
            receipt.geometry_source,
            receipt.section.draw,
            receipt.section.lod,
            receipt.section.palette,
            receipt.bank.source,
            receipt.bank.resource,
            receipt.namespaced_texture
        );
    }
    let keys: BTreeSet<_> = receipts.iter().map(|r| r.bank.clone()).collect();
    let mut banks = BTreeMap::new();
    for key in keys {
        banks.insert(key.clone(), read_bank_dds(root, &key)?);
    }
    println!(
        "lamp_signal_prepare scope=shared-source-receipts slots={slots} sections={} banks={} sampler_gamma_unverified=true native_focus_unjoined=true",
        receipts.len(),
        banks.len()
    );
    Ok(Prepared { banks, slots })
}
pub(crate) fn upload(
    prepared: &Prepared,
    images: &mut Assets<Image>,
    sampler: ImageSampler,
) -> Atlas {
    let mut atlas = BTreeMap::new();
    for (key, dds) in &prepared.banks {
        match crate::texture_upload::image_linear(
            dds,
            sampler.clone(),
            RenderAssetUsages::default(),
        ) {
            Ok(image) => {
                println!(
                    "lamp_signal_atlas bank={:?} resource={:?} raw_linear=true source_sampler_gamma_unverified=true",
                    key.source, key.resource
                );
                atlas.insert(key.clone(), images.add(image));
            }
            Err(error) => eprintln!(
                "lamp_signal_atlas_refused bank={:?} resource={:?} reason={error:?} owned_emission_dark=true",
                key.source, key.resource
            ),
        }
    }
    Atlas(atlas)
}
pub(crate) fn install(app: &mut App) -> Result<(),String> {
    audit::install(app)?;
    app.init_resource::<State>()
        .init_resource::<runtime::Owners>()
        .add_systems(
            PostStartup,
            (
                initialize.after(crate::lamp_materials::attach),
                runtime::project,
            )
                .chain(),
        )
        .add_systems(
            FixedUpdate,
            tick.after(crate::drive_tick)
                .after(crate::vehicle_lights::tick)
                .after(crate::le_mans_event::tick),
        )
        .add_systems(
            Update,
            runtime::project.after(crate::damage_parts::Presented),
        )
        .add_systems(
            PostUpdate,
            runtime::replay_guard
                .after(crate::replay::present)
                .before(bevy::transform::TransformSystems::Propagate),
        );
    Ok(())
}
fn initialize(
    settings: Res<crate::graphics::Settings>,
    prepared: Res<Prepared>,
    mut state: ResMut<State>,
) {
    state.constant(settings.lamp_signals, prepared.slots);
}
fn tick(
    driving: Option<Res<super::Driving>>,
    settings: Res<super::graphics::Settings>,
    lights: Option<Res<super::vehicle_lights::State>>,
    mut state: ResMut<State>,
) {
    if settings.lamp_signals != Mode::Live {
        return;
    }
    let Some(driving) = driving else {
        return;
    };
    let Some(lights) = lights else {
        state.invalidate();
        return;
    };
    if driving.paused || driving.failure.is_some() {
        return;
    }
    let runtimes = std::iter::once(&driving.runtime).chain(
        driving
            .opponents
            .iter()
            .flat_map(|o| o.multi.cars.iter().skip(1)),
    );
    let mut inputs = Vec::new();
    for (slot, runtime) in runtimes.enumerate() {
        let Ok(main) = lights.lamp_request(slot) else {
            state.invalidate();
            return;
        };
        inputs.push([
            main,
            f32::from_bits(runtime.state.car.owner.vehicle[0xe20 / 4]) > 0.,
            runtime.reverse_gear(),
        ]);
    }
    state
        .advance(driving.segment, driving.runtime.ticks, &inputs)
        .expect("stable actual lamp slots");
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_fades_match_576_original_controls() {
        // Fingerprint of2304float words executed by unchanged source9024C0.
        // Authored controls, empty draw graph; tools/validate_lamp_fades.py.
        let mut hash = 0xcbf29ce484222325u64;
        let mut count = 0;
        for dt in [0., 1. / 300., 1. / 60., 1. / 30., 0.1, -0.016] {
            for start in [
                [0., 0., 0.],
                [0.4, 0.6, 0.8],
                [1., 1., 1.],
                [0.99, 0.01, 0.25],
            ] {
                for caps in [[1., 1., 1.], [0.6, 1.5, 0.8], [0.7, 0.7, 1.]] {
                    for head in [false, true] {
                        for brake in [false, true] {
                            for reverse in [false, true] {
                                let words = Fades(start).step(dt, [head, brake, reverse], caps);
                                for value in words {
                                    for byte in value.to_le_bytes() {
                                        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                                    }
                                }
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(count, 576);
        assert_eq!(hash, 0xfc3eafff8611bdb9);
    }
    #[test]
    fn simulation_ticks_do_not_double_advance_and_reset_clears_history() {
        let mut state = State::default();
        state.advance(0, 0, &[[false, false, false]]).unwrap();
        state.advance(0, 1, &[[false, true, false]]).unwrap();
        assert!((state.levels(0).unwrap()[1] - 0.32).abs() < 1e-6);
        let before = state.levels(0);
        state.advance(0, 1, &[[false, false, true]]).unwrap();
        assert_eq!(state.levels(0), before);
        state.advance(0, 2, &[[false, true, false]]).unwrap();
        state.advance(0, 3, &[[false, true, false]]).unwrap();
        assert_eq!(state.levels(0), Some([0., 0.7, 0.]));
        state.advance(0, 0, &[[false, false, false]]).unwrap();
        assert_eq!(state.levels(0), Some([0.; 3]));
        state.advance(0, 1, &[[false, false, true]]).unwrap();
        assert!((state.levels(0).unwrap()[2] - 0.32).abs() < 1e-6);
    }
    #[test]
    fn normal_default_stays_off_and_slot_admission_is_explicit() {
        assert_eq!(Mode::default(), Mode::Off);
        assert!(Mode::parse("on").is_err());
    }
    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT; actual44models highLOD0+low2..5 source bank DDS admission"]
    fn all_purchased_models_admit_actual_selected_lod_banks() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let root = Path::new(&root);
        let (mut models, mut sections, mut banks) = (0, 0, 0);
        for entry in std::fs::read_dir(root.join("cars")).unwrap() {
            let entry = entry.unwrap();
            let stem = entry.file_name();
            let Some(stem) = stem.to_str() else { continue };
            let Ok(model) = grid_tools::car::Car::new(stem) else {
                continue;
            };
            if !root.join(model.file("_highLOD.pssg")).is_file() {
                continue;
            }
            let mut car = crate::load(
                root,
                &grid_tools::route::Route::default(),
                &model,
                "LOD0_",
                false,
                false,
            )
            .unwrap();
            let low = crate::low_lod(root, &model, Mat4::IDENTITY, "", &car.bindings);
            car.low_lods.push(low);
            let admitted = receipts(&car).unwrap_or_else(|e| panic!("{stem}: {e}"));
            let prepared = prepare(root, &car, 3).unwrap_or_else(|e| panic!("{stem}: {e}"));
            for receipt in &admitted {
                assert!(prepared.banks.contains_key(&receipt.bank));
                assert_eq!(
                    runtime::member(&receipt.section.joint, receipt.section.lod).unwrap(),
                    receipt.section.joint.split_once('_').unwrap().1
                );
            }
            sections += admitted.len();
            banks += prepared.banks.len();
            models += 1;
        }
        assert_eq!(
            models, 44,
            "purchased fixture drift, not production whitelist"
        );
        assert!(sections > 0 && banks > 0);
        println!(
            "lamp_signal_admission models={models} sections={sections} actual_banks={banks} high=LOD0 low=actual2to5 upload_render_readiness=UNVERIFIED"
        );
    }
}
