//! Bounded screenshot-entity to sorted-phase diagnostic; independently authored.
//! No sorting changes; CPU pre-batching evidence, not a GPU fragment trace.
use bevy::{
    camera::{MainPassResolutionOverride, NormalizedRenderTarget, RenderTarget},
    core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d},
    diagnostic::FrameCount,
    prelude::*,
    render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        camera::ExtractedCamera, render_phase::{ViewSortedRenderPhases, sort_phase_system},
        render_resource::{PipelineCache,CompareFunction,TextureFormat}, view::{ExtractedView, ViewTarget, screenshot::Screenshot}},
};
use std::{collections::HashMap, fmt::Write as _, fs::{File, OpenOptions}, io::Write as _,
    path::Path, sync::{Arc, Mutex, MutexGuard}};
use super::smoke_pilot::Profile;
#[derive(Clone, Debug)]
struct Row {
    entity: u64, tick: u64, frame: Option<u32>, phase: Option<String>,
    readback: bool, png: bool,
}
type CoverIdentity = (u64,String,String,String,String);
#[derive(Debug)]
struct State {
    expected: usize, rows: Vec<Row>, last_frame: Option<u32>, last_phase: Option<usize>, error: Option<String>, published: bool,
    cover_identity: Option<Vec<CoverIdentity>>,
}
impl State {
    fn new(expected: usize) -> Self {
        Self { expected, rows: Vec::with_capacity(expected), last_frame: None, last_phase: None, error: None, published: false, cover_identity: None }
    }
    fn request(&mut self, index: usize, entity: u64, tick: u64) -> Result<(), String> {
        if index != self.rows.len() || index >= self.expected || self.rows.iter().any(|r| r.entity == entity)
            || self.rows.last().is_some_and(|r| tick < r.tick) { return Err("duplicate/out-of-order request or rollback".into()); }
        self.rows.push(Row { entity, tick, frame: None, phase: None, readback: false, png: false }); Ok(())
    }
    fn row(&mut self, index: usize, entity: u64) -> Result<&mut Row, String> {
        self.rows.get_mut(index).filter(|r| r.entity == entity).ok_or_else(|| "unknown index/entity association".into())
    }
    fn extract(&mut self, index: usize, entity: u64, frame: u32, tick: u64) -> Result<(), String> {
        if self.last_frame.is_some_and(|f| frame <= f)
            || self.rows.iter().take(index).any(|r| r.frame.is_none()) {
            return Err("nonmonotonic/out-of-order Extract association".into());
        }
        let r=self.row(index, entity)?;
        if r.tick != tick || r.frame.is_some() { return Err("reassigned Extract or request/extract tick mismatch".into()); }
        r.frame=Some(frame); self.last_frame=Some(frame); Ok(())
    }
    fn phase(&mut self, index: usize, entity: u64, frame: u32, text: String) -> Result<(), String> {
        if index != self.last_phase.map_or(0, |i| i+1) { return Err("out-of-order phase association".into()); }
        let r=self.row(index, entity)?;
        if r.frame != Some(frame) || r.phase.is_some() { return Err("missing/wrong/duplicate phase association".into()); }
        r.phase=Some(text); self.last_phase=Some(index); Ok(())
    }
    fn readback(&mut self, index: usize, entity: u64) -> Result<(), String> {
        let r=self.row(index, entity)?;
        if r.frame.is_none() || r.phase.is_none() || r.readback { return Err("readback without phase or duplicate readback".into()); }
        r.readback=true; Ok(())
    }
    fn png(&mut self, index: usize, entity: u64) -> Result<(), String> {
        let r=self.row(index, entity)?;
        if !r.readback || r.png { return Err("PNG without readback or duplicate PNG".into()); }
        r.png=true; Ok(())
    }
    fn attempt(&mut self, f: impl FnOnce(&mut Self)->Result<(),String>) -> bool {
        if self.error.is_some() { return false; }
        if let Err(e)=f(self) { self.error=Some(e);return false; }true
    }
    fn cover_identity(&mut self, mut values: Vec<CoverIdentity>) -> Result<(),String> {
        values.sort_unstable_by_key(|v|v.0);
        if values.len()!=17 || values.windows(2).any(|w|w[0].0==w[1].0) {return Err("invalid BMW cover identity set".into());}
        if self.cover_identity.as_ref().is_some_and(|old|old!=&values) {
            return Err("BMW source/mesh/material/properties changed during undamaged capture".into());
        }
        self.cover_identity=Some(values);Ok(())
    }
    fn complete(&self) -> bool {
        self.error.is_none() && self.rows.len()==self.expected
            && self.rows.iter().all(|r| r.frame.is_some() && r.phase.is_some() && r.readback && r.png)
    }
}
// Current Groups pilot admits at most six distinct source draws per captured phase.
#[derive(Default)]
struct RelevantItems { seen: Vec<u64>, skids: usize, smoke: usize }
impl RelevantItems {
    fn admit(&mut self, entity: u64, skid: bool) -> Result<(), &'static str> {
        if self.seen.contains(&entity) { return Err("duplicate relevant MainEntity"); }
        if self.seen.len()>=6 || (skid && self.skids>=1) || (!skid && self.smoke>=5) {
            return Err("relevant draws exceed one skid/five Groups smoke bound");
        }
        self.seen.push(entity);
        if skid { self.skids+=1; } else { self.smoke+=1; } Ok(())
    }
}
// This check proves only main-view CPU phase priority, not smoke fragment coverage.
fn expected_bias(mode: super::skid_marks::OrderMode) -> Result<f32, &'static str> {
    use super::skid_marks::OrderMode;
    match mode { OrderMode::Isolated => Ok(0.), OrderMode::GroundFirst => Ok(-1024.),
        _ => Err("mapped skid pilot accepts only Isolated or GroundFirst") }
}
fn profiled_bias(profile: Profile, mode: super::skid_marks::OrderMode) -> Result<f32, &'static str> {
    match profile {
        Profile::Derby120 => expected_bias(mode),
        Profile::BmwIstanbul90 if mode==super::skid_marks::OrderMode::Off => Ok(50.),
        _ => Err("BMW phase profile requires unchanged skid-order Off"),
    }
}
fn priority_margin(require_before: bool, skid: (usize,f32), smoke: &[(usize,f32)]) -> Result<Option<f32>, &'static str> {
    if !skid.1.is_finite() || smoke.iter().any(|(_,key)| !key.is_finite()) {
        return Err("nonfinite skid/smoke priority key");
    }
    let mut minimum: Option<f32>=None;
    for &(rank,key) in smoke {
        let margin=key-skid.1;
        if !margin.is_finite() {return Err("nonfinite skid/smoke priority margin");}
        if require_before && (rank<=skid.0 || margin<=0.) {
            return Err("GroundFirst does not strictly precede every queued smoke draw");
        }
        minimum=Some(minimum.map_or(margin,|m|m.min(margin)));
    }
    Ok(minimum) // None: no queued smoke, not eligible layering/fragment evidence.
}
#[derive(Resource, Clone)]
pub(crate) struct Audit { state: Arc<Mutex<State>>, output: Arc<Mutex<File>>, expected_bias: f32, require_before: bool, profile: Profile }
impl Audit {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| { let mut s=p.into_inner(); s.error=Some("ledger lock poisoned".into()); s })
    }
    fn attempt(&self, f: impl FnOnce(&mut State)->Result<(),String>) -> bool {
        let mut s=self.state();
        s.attempt(f)
    }
    fn fail(&self, text: &str) { let mut s=self.state(); s.error.get_or_insert_with(|| text.into()); }
    pub(crate) fn ready(&self) -> bool { let s=self.state(); s.published && s.complete() }
    pub(crate) fn stamp(&self, index: u32, entity: Entity, tick: u64, image: &Handle<Image>) -> Option<Stamp> {
        self.attempt(|s| s.request(index as usize,entity.to_bits(),tick)).then(|| Stamp { index: index as usize,
            target: Screenshot::image(image.clone()).0.normalize(None).expect("image target normalizes") })
    }
    pub(crate) fn readback(&self, index: u32, expected: Entity, actual: Entity) -> bool {
        if expected != actual { self.fail("ScreenshotCaptured entity differs from requested entity"); return false; }
        self.attempt(|s| s.readback(index as usize, actual.to_bits()))
    }
    pub(crate) fn png(&self, index: u32, entity: Entity) { self.attempt(|s| s.png(index as usize,entity.to_bits())); }
}
#[derive(Component, Clone)]
pub(crate) struct Stamp { index: usize, target: NormalizedRenderTarget }
#[derive(Clone)]
struct Selected { index: usize, entity: Entity, frame: u32, tick: u64, camera: Entity, target: NormalizedRenderTarget }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum CoverKind { Window, Head, Rear }
impl CoverKind {
    fn source(part: &str) -> Option<Self> {
        match part { "car0/Window Glass"=>Some(Self::Window),"car0/Head Lights Glass"=>Some(Self::Head),
            "car0/Rear Lights Glass"=>Some(Self::Rear),_=>None }
    }
}
#[derive(Default)]
struct CoverCounts { seen: Vec<u64>, window:usize, head:usize, rear:usize }
impl CoverCounts {
    fn admit(&mut self, entity:u64,kind:CoverKind)->Result<(), &'static str> {
        if self.seen.contains(&entity) {return Err("duplicate BMW cover entity");}
        let (count,maximum)=match kind {CoverKind::Window=>(&mut self.window,6),
            CoverKind::Head=>(&mut self.head,3),CoverKind::Rear=>(&mut self.rear,8)};
        if *count>=maximum {return Err("BMW cover family exceeds exact bound");}
        *count+=1;self.seen.push(entity);Ok(())
    }
    fn complete(&self)->bool {self.window==6 && self.head==3 && self.rear==8 && self.seen.len()==17}
}
struct Cover { kind:CoverKind, part:String, mesh:String, material:String, properties:String }
fn cover_metadata(kind:CoverKind,part:&super::track_surface::SourcePart,mesh:&Mesh3d,
    standard:Option<&MeshMaterial3d<StandardMaterial>>,assets:&Assets<StandardMaterial>)->Result<Cover,&'static str> {
    let handle=standard.ok_or("BMW cover lost StandardMaterial binding")?;
    let material=assets.get(&handle.0).ok_or("BMW cover material unavailable")?;
    let c=material.base_color.to_linear();let colour=[c.red,c.green,c.blue,c.alpha];
    let em=material.emissive;let emission=[em.red,em.green,em.blue,em.alpha];
    if material.alpha_mode!=AlphaMode::Blend || material.depth_bias!=0.
        || material.cull_mode.is_some() || !material.double_sided
        || ![material.perceptual_roughness,material.reflectance,material.ior,material.metallic].into_iter().all(f32::is_finite)
        || !colour.into_iter().all(f32::is_finite) || !emission.into_iter().all(f32::is_finite) {return Err("BMW cover original Blend/material contract differs");}
    Ok(Cover {kind,part:part.0.clone(),mesh:format!("{:?}",mesh.id()),material:format!("{:?}",handle.0.id()),
        properties:format!("alpha={:?} sort_bias={} cull={:?} double_sided={} roughness={} reflectance={} ior={} metallic={} colour={colour:?} emissive={:?} base_texture={:?} normal_texture={:?}",
            material.alpha_mode,material.depth_bias,material.cull_mode,material.double_sided,
            material.perceptual_roughness,material.reflectance,material.ior,material.metallic,material.emissive,
            material.base_color_texture.as_ref().map(|h|h.id()),material.normal_map_texture.as_ref().map(|h|h.id())) })
}
#[derive(Resource, Default)]
struct Snapshot { audit: Option<Audit>, shot: Option<Selected>, names: HashMap<Entity,(String,bool)>, covers:HashMap<Entity,Cover> }

pub(crate) fn install(app: &mut App, capture: &Path, mode: super::skid_marks::OrderMode, profile: Profile) -> std::io::Result<()> {
    let count=profile.shots();
    let bias=profiled_bias(profile,mode).map_err(std::io::Error::other)?;
    let require_before=mode==super::skid_marks::OrderMode::GroundFirst;
    if app.get_sub_app(RenderApp).is_none() { return Err(std::io::Error::other("skid phase ledger requires renderer")); }
    let local=Path::new("local").canonicalize()?;
    let parent=capture.parent().ok_or_else(|| std::io::Error::other("capture parent absent"))?.canonicalize()?;
    if !parent.starts_with(local) { return Err(std::io::Error::other("skid phase ledger must remain under ignored local/")); }
    let output=capture.with_extension("skid-phase.txt");
    let mut file=OpenOptions::new().write(true).create_new(true).open(&output)?;
    writeln!(file,"skid_phase version=1 status=pending expected={count} scope=sorted-CPU-phase-before-batching")?;
    file.flush()?;
    app.insert_resource(Audit { state: Arc::new(Mutex::new(State::new(count))), output: Arc::new(Mutex::new(file)), expected_bias: bias, require_before, profile })
        .add_systems(Update,publish);
    app.get_sub_app_mut(RenderApp).expect("renderer checked")
        .init_resource::<Snapshot>().add_systems(ExtractSchedule,extract)
        .add_systems(Render,record_phase.in_set(RenderSystems::PhaseSort).after(sort_phase_system::<Transparent3d>));
    if profile==Profile::BmwIstanbul90 { println!("smoke_phase_profile name={} expected={count} cover_draws=17 unchanged_skid_order=Off",profile.name()); }
    println!("skid_phase enabled=true expected={count} output={} default=false expected_sort_bias={bias} require_ground_first={require_before}",output.display()); Ok(())
}
fn publish(audit: Res<Audit>, driving: Res<super::Driving>, mut exit: MessageWriter<AppExit>) {
    let mut s=audit.state();
    let count=s.expected;
    if !s.published && !s.complete() && (driving.failure.is_some() || driving.paused
        || driving.scripted_ticks.is_some_and(|end| driving.runtime.ticks>=end)) {
        s.error.get_or_insert_with(|| "incomplete screenshot ledger at stopped/failed/script-limit checkpoint".into());
    }
    if s.published || (s.error.is_none() && !s.complete()) { return; }
    let mut text=String::new();
    for (i,r) in s.rows.iter().enumerate() {
        let _=writeln!(text,"shot index={i} png={i:04}.png entity_bits={} request_tick={} extract_frame={:?} readback={} png_saved={}",r.entity,r.tick,r.frame,r.readback,r.png);
        if let Some(p)=&r.phase { text.push_str(p); }
    }
    let _=writeln!(text,"skid_phase status={} expected={} requests={} error={:?}",if s.complete(){"complete"}else{"failed"},s.expected,s.rows.len(),s.error);
    let result=audit.output.lock().map_err(|_| std::io::Error::other("output lock poisoned"))
        .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|_| f.flush()));
    if let Err(e)=result { s.error=Some(format!("sidecar write: {e}")); }
    s.published=true;
    if let Some(e)=&s.error { error!("skid_phase failed: {e}"); exit.write(AppExit::error()); }
    else { println!("skid_phase complete=true expected={count} all_entity_extract_phase_png_links=true"); }
}
#[allow(clippy::too_many_arguments,clippy::type_complexity)] // Bounded ECS extraction.
fn extract(audit: Extract<Res<Audit>>, frame: Extract<Res<FrameCount>>,
    driving: Extract<Res<super::Driving>>,
    shots: Extract<Query<(Entity,&Screenshot,Option<&Stamp>)>>,
    cameras: Extract<Query<(Entity,&RenderTarget),With<super::ViewerCamera>>>,
    names: Extract<Query<(Entity,&Name,&Mesh3d,Option<&super::track_surface::SourcePart>,Option<&MeshMaterial3d<StandardMaterial>>)>>,
    materials: Extract<Res<Assets<StandardMaterial>>>,
    smoke_owner: Extract<Option<Res<super::tyre_smoke::TyreSmoke>>>, mut snapshot: ResMut<Snapshot>) {
    snapshot.shot=None; snapshot.audit=None;
    let audit=(**audit).clone();
    let mut new=Vec::new();
    {
        let s=audit.state();
        if s.error.is_some() || s.published { return; }
        for (entity,shot,stamp) in &shots {
            let Some(stamp)=stamp else { continue; };
            let Some(row)=s.rows.get(stamp.index).filter(|r|r.entity==entity.to_bits()) else {
                drop(s); audit.fail("stamp has no requested row"); return;
            };
            if shot.0.normalize(None).as_ref()!=Some(&stamp.target) { drop(s);audit.fail("screenshot target differs from stamp");return; }
            if row.frame.is_none() { new.push((entity,stamp.clone())); }
        }
    }
    if new.is_empty() { return; }
    if new.len()!=1 { audit.fail("more than one new stamped request per Extract");return; }
    let (entity,stamp)=new.pop().expect("one request checked");
    // Scan ALL screenshot entities, independent of Bevy's private Capturing mutation order.
    if shots.iter().any(|(e,shot,_)|e!=entity && shot.0.normalize(None).as_ref()==Some(&stamp.target)
        // Older Capturing requests normally persist during readback and are not competitors.
        && !audit.state().rows.iter().any(|r|r.entity==e.to_bits() && r.frame.is_some())) {
        audit.fail("competing unassociated screenshot on capture target"); return;
    }
    let Ok((camera,target))=cameras.single() else { audit.fail("expected one ViewerCamera");return; };
    if target.normalize(None).as_ref()!=Some(&stamp.target) || driving.paused || driving.failure.is_some() {
        audit.fail("main camera target differs or driving stopped");return;
    }
    if audit.profile==Profile::BmwIstanbul90 && (driving.geometry.is_some() || driving.opponents.is_some()
        || driving.runtime.deformation_enabled || driving.runtime.body_events.is_some()) {
        audit.fail("BMW runtime damage/contact/opponent activation differs from guarded solo fixture");return;
    }
    let handles=smoke_owner.as_ref().map(|owner|owner.mesh_handles());
    if audit.profile==Profile::BmwIstanbul90 && handles.is_none_or(|handles|handles.len()!=5
        || handles.iter().enumerate().any(|(i,h)|handles[..i].iter().any(|old|old.id()==h.id()))) {
        audit.fail("BMW smoke owner must expose five unique Groups mesh handles");return;
    }
    let mut owned_smoke=Vec::new();
    snapshot.names.clear();snapshot.covers.clear();
    let mut cover_counts=CoverCounts::default();
    for (e,name,mesh,part,standard) in &names {
        let n=name.as_str();
        if n=="skid marks" || (n.starts_with("Tyre smoke ") && !n.starts_with("Tyre smoke sprite ")) {
            if audit.profile==Profile::BmwIstanbul90 && n!="skid marks" {
                if handles.is_none_or(|handles|!handles.iter().any(|h|h.id()==mesh.id())) || owned_smoke.contains(&mesh.id()) {
                    audit.fail("BMW named smoke entity is not a distinct typed owner mesh");return;
                }
                owned_smoke.push(mesh.id());
            }
            snapshot.names.insert(e,(n.into(),n=="skid marks"));
        }
        if audit.profile==Profile::BmwIstanbul90 && let Some(part)=part && let Some(kind)=CoverKind::source(&part.0) {
            let meta=match cover_metadata(kind,part,mesh,standard,&materials) {
                Ok(meta)=>meta,Err(e)=>{audit.fail(e);return;}
            };
            if let Err(e)=cover_counts.admit(e.to_bits(),kind) {audit.fail(e);return;}
            snapshot.covers.insert(e,meta);
        }
    }
    if audit.profile==Profile::BmwIstanbul90 && (owned_smoke.len()!=5 || !cover_counts.complete()) {
        audit.fail("BMW source metadata must contain exactly6windows/3head/8rear cover draws");return;
    }
    if audit.profile==Profile::BmwIstanbul90 {
        let identity=snapshot.covers.iter().map(|(e,c)|(e.to_bits(),c.part.clone(),c.mesh.clone(),c.material.clone(),c.properties.clone())).collect();
        if !audit.attempt(|s|s.cover_identity(identity)) {return;}
    }
    if snapshot.names.values().filter(|(_,skid)|*skid).count()!=1 || snapshot.names.len()!=6 {
        audit.fail("expected one skid draw and five Groups smoke entities");return;
    }
    if !audit.attempt(|s|s.extract(stamp.index,entity.to_bits(),frame.0,driving.runtime.ticks)) {return;}
    snapshot.shot=Some(Selected { index:stamp.index,entity,frame:frame.0,tick:driving.runtime.ticks,camera,target:stamp.target });
    snapshot.audit=Some(audit);
}
// Same native attachment predicates as smoke_boundary::valid_main_view.
// Format is read directly from the actual ViewTarget by record_phase; the
// optional scalar permits CPU tests without constructing a GPU-backed target.
fn bmw_main_view(
    extracted: &ExtractedView,
    camera: &ExtractedCamera,
    format: Option<TextureFormat>,
    msaa: Option<&Msaa>,
    resolution_override: Option<&MainPassResolutionOverride>,
    expected: &NormalizedRenderTarget,
) -> bool {
    camera.target.as_ref() == Some(expected)
        && matches!(
            camera.target.as_ref(),
            Some(NormalizedRenderTarget::Image(_))
        )
        && camera.physical_target_size == Some(UVec2::new(1920, 1080))
        && camera.viewport.is_none()
        && resolution_override.is_none()
        && extracted.viewport == UVec4::new(0, 0, 1920, 1080)
        && msaa.is_some_and(|m| m.samples() == 4)
        && format == Some(TextureFormat::Rgba8UnormSrgb)
}
#[allow(clippy::type_complexity)] // Optional attachments preserve the existing Derby query membership.
fn record_phase(snapshot: Res<Snapshot>, phases: Res<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(Entity,&ExtractedView,&ExtractedCamera,Option<&ViewTarget>,Option<&Msaa>,Option<&MainPassResolutionOverride>)>, pipelines: Res<PipelineCache>) {
    let (Some(audit),Some(shot))=(&snapshot.audit,&snapshot.shot) else {return;};
    let mut matched=views.iter().filter(|(_,v,..)|v.retained_view_entity.main_entity.id()==shot.camera && v.retained_view_entity.subview_index==0);
    let Some((entity,view,camera,target,msaa,resolution_override))=matched.next() else {audit.fail("main render view missing");return;};
    if matched.next().is_some() || camera.target.as_ref()!=Some(&shot.target) {audit.fail("duplicate main view or render target mismatch");return;}
    if audit.profile==Profile::BmwIstanbul90 && !bmw_main_view(
        view,camera,target.map(ViewTarget::main_texture_format),msaa,resolution_override,&shot.target,
    ) {
        audit.fail("BMW selected main view is not native1920/MSAA4/sRGB/fullviewport/nooverride");return;
    }
    let Some(phase)=phases.get(&view.retained_view_entity) else {audit.fail("main Transparent3d phase missing");return;};
    if phase.items.len()>50_000 {audit.fail("phase exceeds bounded item limit");return;}
    let mut text=String::new(); let mut relevant=RelevantItems::default();
    let mut skid_key=None; let mut smoke_keys=Vec::with_capacity(5);
    let mut cover_counts=CoverCounts::default();
    let _=writeln!(text,"phase index={} entity_bits={} frame={} tick={} main_camera={:?} render_view={entity:?} target={:?} items={}",shot.index,shot.entity.to_bits(),shot.frame,shot.tick,shot.camera,shot.target,phase.items.len());
    if audit.profile==Profile::BmwIstanbul90 {
        let _=writeln!(text,"bmw_view index={} physical_target_size={:?} camera_viewport={:?} extracted_viewport={:?} samples={} format={:?} resolution_override=false",shot.index,
            camera.physical_target_size,camera.viewport,view.viewport,msaa.expect("BMW attachment guard").samples(),target.expect("BMW attachment guard").main_texture_format());
    }
    for (rank,(key,item)) in phase.items.iter().enumerate() {
        let main=item.entity.1.id();
        if let Some(cover)=snapshot.covers.get(&main) {
            if let Err(e)=cover_counts.admit(main.to_bits(),cover.kind) {audit.fail(e);return;}
            let TransparentSortingInfo3d::Sorted {mesh_center,depth_bias}=item.sorting_info else {
                audit.fail("BMW cover unexpectedly AlwaysOnTop");return;
            };
            let z=view.rangefinder3d().distance(&mesh_center);
            if !mesh_center.is_finite() || !z.is_finite() || !item.distance.is_finite() || depth_bias!=0.
                || item.distance.to_bits()!=(z+depth_bias).to_bits() {audit.fail("BMW cover sort/centre data differs");return;}
            if pipelines.get_render_pipeline(item.pipeline).is_none() {audit.fail("BMW cover pipeline unready");return;}
            let descriptor=pipelines.get_render_pipeline_descriptor(item.pipeline);
            let Some(depth)=descriptor.depth_stencil.as_ref() else {audit.fail("BMW cover depth state missing");return;};
            if depth.format!=TextureFormat::Depth32Float || depth.depth_write_enabled!=Some(false)
                || depth.depth_compare!=Some(CompareFunction::GreaterEqual) || depth.bias.constant!=0
                || depth.bias.slope_scale!=0. || depth.bias.clamp!=0. || descriptor.multisample.count!=4
                || descriptor.fragment.as_ref().is_none_or(|f|f.targets.len()!=1 || f.targets[0].as_ref().is_none_or(|t|t.format!=TextureFormat::Rgba8UnormSrgb)) {
                audit.fail("BMW cover baseline pipeline/depth/MSAA contract differs");return;
            }
            let _=writeln!(text,"cover index={} rank={rank} phase_entity_key={key:?} entity_bits={} kind={:?} source_part={:?} mesh={} material={} properties={:?} centre={mesh_center:?} projected_z={z} sort_bias={depth_bias} sort_distance={} pipeline={:?} ready=true depth={depth:?} draw={:?}",
                shot.index,main.to_bits(),cover.kind,cover.part,cover.mesh,cover.material,cover.properties,item.distance,item.pipeline,item.draw_function);
        }
        let Some((name,skid))=snapshot.names.get(&main) else {continue;};
        if let Err(reason)=relevant.admit(main.to_bits(),*skid) {audit.fail(reason);return;}
        let TransparentSortingInfo3d::Sorted {mesh_center,depth_bias}=item.sorting_info else {audit.fail("relevant AlwaysOnTop item");return;};
        let z=view.rangefinder3d().distance(&mesh_center);
        if !item.distance.is_finite() || !z.is_finite() || !depth_bias.is_finite() || !mesh_center.is_finite() {audit.fail("nonfinite relevant sort data");return;}
        let ready=pipelines.get_render_pipeline(item.pipeline).is_some();
        if !ready {audit.fail("relevant pipeline unready; descriptor not read");return;}
        let descriptor=pipelines.get_render_pipeline_descriptor(item.pipeline);
        let Some(depth)=descriptor.depth_stencil.as_ref() else {audit.fail("relevant depth state missing");return;};
        if *skid && (depth_bias!=audit.expected_bias || depth.bias.constant!=50 || depth.depth_write_enabled!=Some(false)) {
            audit.fail("expected mapped skid sort/raster/write invariant differs");return;
        }
        if *skid {skid_key=Some((rank,item.distance));} else {smoke_keys.push((rank,item.distance));}
        let _=writeln!(text,"item index={} rank={rank} phase_entity_key={key:?} entity_bits={} name={name:?} centre={mesh_center:?} projected_z={z} sort_bias={depth_bias} sort_distance={} pipeline={:?} ready={ready} depth={depth:?} draw={:?}",shot.index,main.to_bits(),item.distance,item.pipeline,item.draw_function);
    }
    if audit.profile==Profile::BmwIstanbul90 {
        if !cover_counts.complete() {audit.fail("BMW captured phase lost/aliased one of17cover draws");return;}
        let _=writeln!(text,"cover_counts index={} window={} head={} rear={} total={} profile={}",shot.index,
            cover_counts.window,cover_counts.head,cover_counts.rear,cover_counts.seen.len(),audit.profile.name());
    }
    let RelevantItems {skids,smoke,..}=relevant;
    if skids!=1 {audit.fail("expected one queued skid draw in selected capture");return;}
    let Some(skid_key)=skid_key else {audit.fail("queued skid priority key absent");return;};
    let margin=match priority_margin(audit.require_before,skid_key,&smoke_keys) {
        Ok(margin)=>margin,
        Err(reason)=>{
            let _=writeln!(text,"phase_priority index={} status=failed expected_sort_bias={} require_ground_first={} reason={reason:?}",shot.index,audit.expected_bias,audit.require_before);
            // Preserve this captured frame's actual item ranks/keys even on failed admission.
            audit.attempt(|s|s.phase(shot.index,shot.entity.to_bits(),shot.frame,text));
            audit.fail(reason);return;
        }
    };
    let _=writeln!(text,"phase_priority index={} expected_sort_bias={} require_ground_first={} queued_smoke={} min_key_margin={margin:?} fragment_visibility_unproven=true",shot.index,audit.expected_bias,audit.require_before,smoke_keys.len());
    let _=writeln!(text,"phase_counts index={} skid={skids} smoke={smoke} smoke_queued={} fragment_visibility_unproven=true",shot.index,smoke>0);
    audit.attempt(|s|s.phase(shot.index,shot.entity.to_bits(),shot.frame,text));
}
#[cfg(test)]
mod tests {
    use super::*;
    fn extracted() -> State {
        let mut s=State::new(2);
        s.request(0,100,350).unwrap(); s.extract(0,100,10,350).unwrap();
        s.request(1,101,352).unwrap(); s.extract(1,101,11,352).unwrap();s
    }
    #[test]
    fn ground_first_requires_strict_rank_and_key_margin_not_only_sorted_distance() {
        assert_eq!(priority_margin(true,(2,-100.),&[(4,-99.),(6,-90.)]),Ok(Some(1.)));
        assert!(priority_margin(true,(5,-100.),&[(4,-99.)]).is_err()); // Key alone is insufficient.
        assert!(priority_margin(true,(2,-100.),&[(4,-100.)]).is_err()); // Tie is not strict.
        assert!(priority_margin(true,(2,-100.),&[(4,-101.)]).is_err());
        assert_eq!(priority_margin(false,(5,-100.),&[(4,-101.)]),Ok(Some(-1.))); // Isolated observation remains legal.
        assert_eq!(priority_margin(true,(2,-100.),&[]),Ok(None)); // No queued smoke is not fragment evidence.
    }
    #[test]
    fn priority_proof_rejects_nonfinite_keys_margins_and_unsupported_install_modes() {
        for bad in [f32::NAN,f32::INFINITY,f32::NEG_INFINITY] {
            assert!(priority_margin(true,(0,bad),&[(1,1.)]).is_err());
            assert!(priority_margin(false,(0,0.),&[(1,bad)]).is_err());
        }
        assert!(priority_margin(true,(0,-f32::MAX),&[(1,f32::MAX)]).is_err());
        for mode in [super::super::skid_marks::OrderMode::Off,super::super::skid_marks::OrderMode::Control] {
            assert!(expected_bias(mode).is_err()); // Ordinary/default cannot instantiate mapped priority audit.
        }
    }
    #[test]
    fn six_distinct_source_draws_fit_but_duplicate_and_sixth_smoke_do_not() {
        let mut r=RelevantItems::default();
        for entity in 10..15 {r.admit(entity,false).unwrap();}
        assert_eq!((r.smoke,r.skids,r.seen.len()),(5,0,5));
        assert!(r.admit(15,false).is_err()); // Sixth distinct smoke rejected before serialization.
        assert_eq!(r.seen.len(),5);
        assert!(r.admit(10,true).is_err()); // Same MainEntity cannot be reclassified/repeated.
        r.admit(20,true).unwrap();assert_eq!((r.smoke,r.skids,r.seen.len()),(5,1,6));
        assert!(r.admit(20,true).is_err());assert!(r.admit(21,true).is_err());
        assert_eq!(r.seen.len(),6);
    }
    #[test]
    fn async_completion_can_reverse_but_each_entity_phase_and_png_must_match() {
        let mut s=extracted();
        s.phase(0,100,10,"p0".into()).unwrap(); s.phase(1,101,11,"p1".into()).unwrap();
        s.readback(1,101).unwrap();s.png(1,101).unwrap();assert!(!s.complete());
        s.readback(0,100).unwrap();assert!(!s.complete());s.png(0,100).unwrap();assert!(s.complete());
        assert!(s.readback(0,100).is_err());assert!(s.png(1,101).is_err());
    }
    #[test]
    fn entirely_missing_phase_and_readback_are_in_expected_denominator() {
        let mut s=extracted();s.phase(0,100,10,"p0".into()).unwrap();s.readback(0,100).unwrap();s.png(0,100).unwrap();
        assert!(s.readback(1,101).is_err());assert!(!s.complete());
        s.phase(1,101,11,"p1".into()).unwrap();assert!(!s.complete());
        s.readback(1,101).unwrap();assert!(!s.complete());s.png(1,101).unwrap();assert!(s.complete());
    }
    #[test]
    fn wrong_entity_frame_duplicate_and_nonmonotonic_associations_fail() {
        let mut s=extracted();
        assert!(s.phase(1,101,11,"bad-order".into()).is_err());
        assert!(s.phase(0,101,10,"bad".into()).is_err());assert!(s.phase(0,100,11,"bad".into()).is_err());
        s.phase(0,100,10,"ok".into()).unwrap();assert!(s.phase(0,100,10,"dup".into()).is_err());
        assert!(s.extract(1,101,12,352).is_err());
        s.request(2,102,354).unwrap_err();
        let mut s=State::new(3);s.request(0,100,350).unwrap();s.extract(0,100,10,350).unwrap();
        s.request(1,101,352).unwrap();assert!(s.extract(1,101,10,352).is_err());
        s.extract(1,101,11,352).unwrap();s.request(2,102,354).unwrap();assert!(s.extract(2,102,10,354).is_err());
    }
    #[test]
    fn production_error_latch_refuses_repair_and_cannot_publish_complete() {
        let mut s=extracted();
        assert!(!s.attempt(|s|s.phase(1,101,11,"wrong-order".into())));
        assert!(s.error.is_some());
        assert!(!s.attempt(|s|s.phase(0,100,10,"later-correct".into())));
        assert!(s.rows[0].phase.is_none());assert!(!s.complete());
    }
    #[test]
    fn missing_request_reordered_extract_tick_rollback_and_error_never_complete() {
        let mut s=State::new(2);assert!(s.request(1,101,350).is_err());s.request(0,100,350).unwrap();
        assert!(s.request(1,100,352).is_err());assert!(s.request(1,101,349).is_err());
        s.request(1,101,352).unwrap();assert!(s.extract(1,101,10,352).is_err());
        assert!(s.extract(0,100,10,351).is_err());assert!(!s.complete());
        let mut s=State::new(0);assert!(s.complete());s.error=Some("failed".into());assert!(!s.complete());
    }
    #[test]
    fn typed_profile_count_does_not_complete_120_from_90_or_admit_crossed_order() {
        let mut bmw=State::new(Profile::BmwIstanbul90.shots());
        let mut derby=State::new(Profile::Derby120.shots());
        for i in 0..90 {
            for state in [&mut bmw,&mut derby] {
                let entity=1000+i as u64;let tick=190+i as u64;let frame=200+i as u32;
                state.request(i,entity,tick).unwrap();state.extract(i,entity,frame,tick).unwrap();
                state.phase(i,entity,frame,"synthetic CPU phase".into()).unwrap();
                state.readback(i,entity).unwrap();state.png(i,entity).unwrap();
            }
        }
        assert!(bmw.complete());assert!(!derby.complete());
        assert!(bmw.request(90,9999,300).is_err());
        bmw.rows[89].png=false;assert!(!bmw.complete());
        assert_eq!(profiled_bias(Profile::BmwIstanbul90,super::super::skid_marks::OrderMode::Off),Ok(50.));
        for mode in [super::super::skid_marks::OrderMode::Control,super::super::skid_marks::OrderMode::Isolated,
            super::super::skid_marks::OrderMode::GroundFirst] {assert!(profiled_bias(Profile::BmwIstanbul90,mode).is_err());}
        assert!(profiled_bias(Profile::Derby120,super::super::skid_marks::OrderMode::Off).is_err());
    }
    #[test]
    fn bmw_cover_families_are_exact_player_source_parts_and_6_3_8_unique() {
        assert_eq!(CoverKind::source("car0/Window Glass"),Some(CoverKind::Window));
        for part in ["car1/Window Glass","Window Glass","car0/Window Glass damaged","car0/GrillsGlass"] {
            assert_eq!(CoverKind::source(part),None);
        }
        let mut counts=CoverCounts::default();
        for i in 0..6 {counts.admit(i,CoverKind::Window).unwrap();}
        for i in 6..9 {counts.admit(i,CoverKind::Head).unwrap();}
        for i in 9..17 {counts.admit(i,CoverKind::Rear).unwrap();}
        assert!(counts.complete());assert!(counts.admit(17,CoverKind::Window).is_err());
        let mut alias=CoverCounts::default();alias.admit(3,CoverKind::Window).unwrap();
        assert!(alias.admit(3,CoverKind::Rear).is_err());assert!(!alias.complete());
    }
    #[test]
    fn cover_metadata_refuses_missing_opaque_culled_or_nonfinite_material_instead_of_fallback() {
        let mut assets=Assets::<StandardMaterial>::default();
        let part=super::super::track_surface::SourcePart("car0/Window Glass".into());
        let mesh=Mesh3d(Handle::default());
        let good=StandardMaterial {alpha_mode:AlphaMode::Blend,cull_mode:None,double_sided:true,..Default::default()};
        let handle=MeshMaterial3d(assets.add(good.clone()));
        assert!(cover_metadata(CoverKind::Window,&part,&mesh,Some(&handle),&assets).is_ok());
        assert!(cover_metadata(CoverKind::Window,&part,&mesh,None,&assets).is_err());
        for material in [StandardMaterial {alpha_mode:AlphaMode::Opaque,..good.clone()},
            StandardMaterial {double_sided:false,..good.clone()},
            StandardMaterial {reflectance:f32::NAN,..good}] {
            let handle=MeshMaterial3d(assets.add(material));
            assert!(cover_metadata(CoverKind::Window,&part,&mesh,Some(&handle),&assets).is_err());
        }
    }

    #[test]
    fn undamaged_bmw_cover_identity_refuses_handle_mesh_property_or_owner_switch() {
        let original:Vec<_>=(0..17).map(|i|(i,"part".into(),"mesh".into(),"material".into(),"properties".into())).collect();
        let mut state=State::new(90);state.cover_identity(original.clone()).unwrap();
        let mut reordered=original.clone();reordered.reverse();state.cover_identity(reordered).unwrap();
        for field in 0..5 {
            let mut changed=original.clone();
            match field {0=>changed[0].0=999,1=>changed[0].1="other".into(),2=>changed[0].2="other".into(),
                3=>changed[0].3="cracked".into(),_=>changed[0].4="different opacity".into()}
            assert!(state.cover_identity(changed).is_err());
        }
        assert!(state.cover_identity(original[..16].to_vec()).is_err());
    }

    fn bmw_view_fixture() -> (ExtractedView, ExtractedCamera, NormalizedRenderTarget) {
        use bevy::ecs::schedule::ScheduleLabel;
        let target = NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget {
            handle: Handle::default(),
            scale_factor: 1.,
        });
        let defaults = Camera::default();
        let camera = ExtractedCamera {
            target: Some(target.clone()),
            physical_viewport_size: Some(UVec2::new(1920, 1080)),
            physical_target_size: Some(UVec2::new(1920, 1080)),
            viewport: None,
            schedule: bevy::app::Main.intern(),
            order: 0,
            output_mode: defaults.output_mode,
            msaa_writeback: defaults.msaa_writeback,
            clear_color: defaults.clear_color,
            sorted_camera_index_for_target: 0,
            exposure: 1.,
            hdr: false,
            compositing_space: None,
        };
        let view = ExtractedView {
            retained_view_entity: bevy::render::view::RetainedViewEntity::new(
                Entity::PLACEHOLDER.into(),
                None,
                0,
            ),
            clip_from_view: Mat4::IDENTITY,
            world_from_view: GlobalTransform::IDENTITY,
            clip_from_world: None,
            target_format: TextureFormat::Rgba8UnormSrgb,
            viewport: UVec4::new(0, 0, 1920, 1080),
            color_grading: default(),
            invert_culling: false,
        };
        (view, camera, target)
    }
    #[test]
    fn bmw_control_view_rejects_wrong_actual_attachment_contract() {
        let (mut view, mut camera, target) = bmw_view_fixture();
        let format = Some(TextureFormat::Rgba8UnormSrgb);
        let msaa = Some(&Msaa::Sample4);
        assert!(bmw_main_view(&view, &camera, format, msaa, None, &target));
        for size in [
            None,
            Some(UVec2::new(1280, 720)),
            Some(UVec2::new(1920, 1079)),
        ] {
            camera.physical_target_size = size;
            assert!(!bmw_main_view(&view, &camera, format, msaa, None, &target));
        }
        camera.physical_target_size = Some(UVec2::new(1920, 1080));
        camera.viewport = Some(bevy::camera::Viewport::default());
        assert!(!bmw_main_view(&view, &camera, format, msaa, None, &target));
        camera.viewport = None;
        for viewport in [UVec4::new(1, 0, 1920, 1080), UVec4::new(0, 0, 960, 540)] {
            view.viewport = viewport;
            assert!(!bmw_main_view(&view, &camera, format, msaa, None, &target));
        }
        view.viewport = UVec4::new(0, 0, 1920, 1080);
        let resolution = MainPassResolutionOverride(UVec2::new(960, 540));
        assert!(!bmw_main_view(
            &view,
            &camera,
            format,
            msaa,
            Some(&resolution),
            &target
        ));
        for samples in [
            None,
            Some(&Msaa::Off),
            Some(&Msaa::Sample2),
            Some(&Msaa::Sample8),
        ] {
            assert!(!bmw_main_view(
                &view, &camera, format, samples, None, &target
            ));
        }
        for format in [
            None,
            Some(TextureFormat::Rgba8Unorm),
            Some(TextureFormat::Rgba16Float),
        ] {
            assert!(!bmw_main_view(&view, &camera, format, msaa, None, &target));
        }
        camera.target = None;
        assert!(!bmw_main_view(&view, &camera, format, msaa, None, &target));
        let nonimage = NormalizedRenderTarget::None {
            width: 1920,
            height: 1080,
        };
        camera.target = Some(nonimage.clone());
        assert!(!bmw_main_view(
            &view, &camera, format, msaa, None, &nonimage
        ));
        camera.target = Some(target.clone());
        assert!(!bmw_main_view(
            &view, &camera, format, msaa, None, &nonimage
        ));
        assert!(bmw_main_view(&view, &camera, format, msaa, None, &target));
    }
}
