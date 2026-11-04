//! Bounded default-off ECS registration / GPU-image preparation audit.
//! NOT rendered/drawn/current-LOD/material-ownership/source-focus proof.
use super::{Atlas, Prepared, runtime::Receiver};
use crate::lamp_association::{BankKey, Receipt};
use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems, extract_resource::ExtractResourcePlugin,
        render_asset::RenderAssets, texture::GpuImage,
    },
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
const MAX_RECEIVERS: usize = 4096;
const MAX_BANKS: usize = 256;
pub(crate) fn enabled() -> bool {
    std::env::var_os("GRID_LAMP_AUDIT").as_deref() == Some(std::ffi::OsStr::new("1"))
}
#[derive(Clone)]
struct Queued {
    entity: Entity,
    slot: usize,
    lod: u8,
    bank: BankKey,
}
#[derive(Resource)]
struct Queue {
    slots: usize,
    records: Vec<Queued>,
    error: Option<&'static str>,
    finished: bool,
}
/// Called on the SAME queued entity that receives Receiver, only when audit explicitly enabled.
pub(crate) fn spawned(entity: &mut EntityCommands, slot: usize, receipt: &Receipt) {
    if !enabled() {
        return;
    }
    let record = Queued {
        entity: entity.id(),
        slot,
        lod: receipt.section.lod,
        bank: receipt.bank.clone(),
    };
    entity.commands().queue(move |world: &mut World| {
        let mut queue = world.resource_mut::<Queue>();
        if queue.records.len() >= MAX_RECEIVERS {
            queue.error = Some("configured receiver bound")
        } else {
            queue.records.push(record)
        }
    });
}
pub(super) fn install(app: &mut App) -> Result<(), String> {
    if !enabled() {
        return Ok(());
    }
    if app.get_sub_app(RenderApp).is_none() {
        return Err("explicit lamp audit needs actual RenderApp".into());
    }
    let prepared = app
        .world()
        .get_resource::<Prepared>()
        .ok_or("explicit lamp audit needs prepared banks")?;
    if prepared.banks.len() > MAX_BANKS || prepared.slots == 0 || prepared.slots > MAX_RECEIVERS {
        return Err("lamp audit prepared bank/slot bound".into());
    }
    let slots = prepared.slots;
    app.insert_resource(Queue {
        slots,
        records: Vec::new(),
        error: None,
        finished: false,
    })
    .add_plugins(ExtractResourcePlugin::<Atlas>::default())
    .add_systems(PostStartup, configured.after(super::initialize));
    app.get_sub_app_mut(RenderApp)
        .expect("validated RenderApp")
        .add_systems(Render, gpu_ready.in_set(RenderSystems::PrepareBindGroups));
    Ok(())
}
struct Joined {
    groups: BTreeMap<(usize, u8, BankKey), usize>,
    covered: BTreeSet<usize>,
    lods: BTreeSet<u8>,
    banks: BTreeSet<BankKey>,
}
fn join(queue: &Queue, actual: &HashSet<Entity>, atlas: &Atlas) -> Result<Joined, &'static str> {
    if let Some(error) = queue.error {
        return Err(error);
    }
    if queue.records.len() > MAX_RECEIVERS
        || actual.len() > MAX_RECEIVERS
        || atlas.0.len() > MAX_BANKS
    {
        return Err("lamp audit observed bound");
    }
    let queued: HashSet<_> = queue.records.iter().map(|r| r.entity).collect();
    if queued.len() != queue.records.len() || queued != *actual {
        return Err("queued entity/actual Receiver ID join mismatch");
    }
    let mut joined = Joined {
        groups: BTreeMap::new(),
        covered: BTreeSet::new(),
        lods: BTreeSet::new(),
        banks: BTreeSet::new(),
    };
    for r in &queue.records {
        if r.slot >= queue.slots {
            return Err("queued actual slot outside configured field");
        }
        if !atlas.0.contains_key(&r.bank) {
            return Err("queued source bank not in uploaded Atlas");
        }
        *joined
            .groups
            .entry((r.slot, r.lod, r.bank.clone()))
            .or_default() += 1;
        joined.covered.insert(r.slot);
        joined.lods.insert(r.lod);
        joined.banks.insert(r.bank.clone());
    }
    Ok(joined)
}
fn configured(
    mut queue: ResMut<Queue>,
    actual: Query<Entity, With<Receiver>>,
    atlas: Res<Atlas>,
    mut exit: MessageWriter<AppExit>,
) {
    if queue.finished {
        return;
    }
    queue.finished = true;
    let actual: HashSet<_> = actual.iter().take(MAX_RECEIVERS + 1).collect();
    match join(&queue, &actual, &atlas) {
        Err(error) => {
            queue.error = Some(error);
            eprintln!("lamp_audit_refused version=1 reason={error:?}");
            exit.write(AppExit::error());
        }
        Ok(joined) => {
            println!(
                "lamp_audit_configured version=1 queued={} actual={} joined=true configured_slots={} covered_slots={} queued_banks={} atlas_banks={} lods={:?} ecs_registration_only=true active_lod=UNPROVEN material_ownership=UNPROVEN",
                queue.records.len(),
                actual.len(),
                queue.slots,
                joined.covered.len(),
                joined.banks.len(),
                atlas.0.len(),
                joined.lods
            );
            for ((slot, lod, bank), count) in joined.groups {
                println!(
                    "lamp_audit_group version=1 slot={slot} lod={lod} receivers={count} bank={:?} resource={:?} ecs_registration_only=true active_lod=UNPROVEN",
                    bank.source, bank.resource
                );
            }
        }
    }
}
fn gpu_ready(
    atlas: Option<Res<Atlas>>,
    images: Res<RenderAssets<GpuImage>>,
    mut reported: Local<bool>,
) {
    if *reported {
        return;
    }
    let Some(atlas) = atlas else { return };
    if atlas.0.len() > MAX_BANKS {
        if !*reported {
            eprintln!("lamp_audit_gpu_refused version=1 reason=bank_bound");
            *reported = true;
        }
        return;
    }
    if !atlas
        .0
        .values()
        .all(|handle| images.get(handle.id()).is_some())
    {
        return;
    }
    *reported = true;
    println!(
        "lamp_audit_gpu version=1 banks={} prepared=true source=RenderAssetsGpuImage draw_execution=UNPROVEN active_lod=UNPROVEN",
        atlas.0.len()
    );
    for (bank, handle) in &atlas.0 {
        println!(
            "lamp_audit_gpu_bank version=1 bank={:?} resource={:?} image={:?} prepared=true draw_execution=UNPROVEN",
            bank.source,
            bank.resource,
            handle.id()
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bank() -> BankKey {
        BankKey {
            source: "actual_general_high".into(),
            resource: "actual_lights_on.tga".into(),
        }
    }
    #[test]
    fn actual_poststartup_query_joins_queued_ids_and_refuses_unrecorded_receiver() {
        fn produce(mut commands: Commands) {
            let receipt = Receipt {
                bank: bank(),
                geometry_source: "actual_geometry".into(),
                namespaced_texture: "namespace/actual_lights_on.tga".into(),
                section: crate::lamp_sections::Section {
                    semantic: 121,
                    palette: 9,
                    role: crate::lamp_sections::Role::Brake,
                    cluster: 6,
                    lod: 2,
                    node: 3,
                    bundle: "LOD2_".into(),
                    draw: "draw".into(),
                    material: "Rear Lights Pod".into(),
                    joint: "x2_light_brake_pod_bl".into(),
                },
            };
            let entity = commands
                .spawn(Receiver::from_receipt(1, &receipt).unwrap())
                .id();
            commands.queue(move |world: &mut World| {
                world.resource_mut::<Queue>().records.push(Queued {
                    entity,
                    slot: 1,
                    lod: 2,
                    bank: receipt.bank,
                })
            });
        }
        let mut app = App::new();
        app.add_message::<AppExit>()
            .insert_resource(Queue {
                slots: 2,
                records: vec![],
                error: None,
                finished: false,
            })
            .insert_resource(Atlas([(bank(), Handle::default())].into_iter().collect()))
            .add_systems(Startup, produce)
            .add_systems(PostStartup, configured);
        app.update();
        let q = app.world().resource::<Queue>();
        assert!(q.finished);
        assert!(q.error.is_none());
        assert_eq!(q.records.len(), 1);
        let actual = HashSet::new();
        assert!(join(q, &actual, app.world().resource::<Atlas>()).is_err());
    }
    #[test]
    fn bank_slot_and_duplicate_entity_refusals_are_explicit() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut queue = Queue {
            slots: 1,
            records: vec![Queued {
                entity,
                slot: 0,
                lod: 0,
                bank: bank(),
            }],
            error: None,
            finished: false,
        };
        let actual = HashSet::from([entity]);
        let atlas = Atlas([(bank(), Handle::default())].into_iter().collect());
        assert!(join(&queue, &actual, &atlas).is_ok());
        assert!(join(&queue, &actual, &Atlas::default()).is_err());
        queue.records[0].slot = 1;
        assert!(join(&queue, &actual, &atlas).is_err());
        queue.records[0].slot = 0;
        queue.records.push(queue.records[0].clone());
        assert!(join(&queue, &actual, &atlas).is_err());
    }
}
