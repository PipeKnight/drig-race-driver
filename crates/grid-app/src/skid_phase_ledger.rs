//! Bounded screenshot-entity to sorted-phase diagnostic; independently authored.
//! No sorting changes; CPU pre-batching evidence, not a GPU fragment trace.
use bevy::{
    camera::{NormalizedRenderTarget, RenderTarget},
    core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d},
    diagnostic::FrameCount,
    prelude::*,
    render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        camera::ExtractedCamera, render_phase::{ViewSortedRenderPhases, sort_phase_system},
        render_resource::PipelineCache, view::{ExtractedView, screenshot::Screenshot}},
};
use std::{collections::HashMap, fmt::Write as _, fs::{File, OpenOptions}, io::Write as _,
    path::Path, sync::{Arc, Mutex, MutexGuard}};
const COUNT: usize = 120;
#[derive(Clone, Debug)]
struct Row {
    entity: u64, tick: u64, frame: Option<u32>, phase: Option<String>,
    readback: bool, png: bool,
}
#[derive(Debug)]
struct State {
    expected: usize, rows: Vec<Row>, last_frame: Option<u32>, last_phase: Option<usize>, error: Option<String>, published: bool,
}
impl State {
    fn new(expected: usize) -> Self {
        Self { expected, rows: Vec::with_capacity(expected), last_frame: None, last_phase: None, error: None, published: false }
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
#[derive(Resource, Clone)]
pub(crate) struct Audit { state: Arc<Mutex<State>>, output: Arc<Mutex<File>> }
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
#[derive(Resource, Default)]
struct Snapshot { audit: Option<Audit>, shot: Option<Selected>, names: HashMap<Entity,(String,bool)> }

pub(crate) fn install(app: &mut App, capture: &Path) -> std::io::Result<()> {
    if app.get_sub_app(RenderApp).is_none() { return Err(std::io::Error::other("skid phase ledger requires renderer")); }
    let local=Path::new("local").canonicalize()?;
    let parent=capture.parent().ok_or_else(|| std::io::Error::other("capture parent absent"))?.canonicalize()?;
    if !parent.starts_with(local) { return Err(std::io::Error::other("skid phase ledger must remain under ignored local/")); }
    let output=capture.with_extension("skid-phase.txt");
    let mut file=OpenOptions::new().write(true).create_new(true).open(&output)?;
    writeln!(file,"skid_phase version=1 status=pending expected={COUNT} scope=sorted-CPU-phase-before-batching")?;
    file.flush()?;
    app.insert_resource(Audit { state: Arc::new(Mutex::new(State::new(COUNT))), output: Arc::new(Mutex::new(file)) })
        .add_systems(Update,publish);
    app.get_sub_app_mut(RenderApp).expect("renderer checked")
        .init_resource::<Snapshot>().add_systems(ExtractSchedule,extract)
        .add_systems(Render,record_phase.in_set(RenderSystems::PhaseSort).after(sort_phase_system::<Transparent3d>));
    println!("skid_phase enabled=true expected={COUNT} output={} default=false",output.display()); Ok(())
}
fn publish(audit: Res<Audit>, driving: Res<super::Driving>, mut exit: MessageWriter<AppExit>) {
    let mut s=audit.state();
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
    else { println!("skid_phase complete=true expected={COUNT} all_entity_extract_phase_png_links=true"); }
}
#[allow(clippy::too_many_arguments,clippy::type_complexity)] // Bounded ECS extraction.
fn extract(audit: Extract<Res<Audit>>, frame: Extract<Res<FrameCount>>,
    driving: Extract<Res<super::Driving>>,
    shots: Extract<Query<(Entity,&Screenshot,Option<&Stamp>)>>,
    cameras: Extract<Query<(Entity,&RenderTarget),With<super::ViewerCamera>>>,
    names: Extract<Query<(Entity,&Name),With<Mesh3d>>>, mut snapshot: ResMut<Snapshot>) {
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
    snapshot.names.clear();
    for (e,name) in &names {
        let n=name.as_str();
        if n=="skid marks" || (n.starts_with("Tyre smoke ") && !n.starts_with("Tyre smoke sprite ")) {
            snapshot.names.insert(e,(n.into(),n=="skid marks"));
        }
    }
    if snapshot.names.values().filter(|(_,skid)|*skid).count()!=1 || snapshot.names.len()!=6 {
        audit.fail("expected one skid draw and five Groups smoke entities");return;
    }
    if !audit.attempt(|s|s.extract(stamp.index,entity.to_bits(),frame.0,driving.runtime.ticks)) {return;}
    snapshot.shot=Some(Selected { index:stamp.index,entity,frame:frame.0,tick:driving.runtime.ticks,camera,target:stamp.target });
    snapshot.audit=Some(audit);
}
fn record_phase(snapshot: Res<Snapshot>, phases: Res<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(Entity,&ExtractedView,&ExtractedCamera)>, pipelines: Res<PipelineCache>) {
    let (Some(audit),Some(shot))=(&snapshot.audit,&snapshot.shot) else {return;};
    let mut matched=views.iter().filter(|(_,v,_)|v.retained_view_entity.main_entity.id()==shot.camera && v.retained_view_entity.subview_index==0);
    let Some((entity,view,camera))=matched.next() else {audit.fail("main render view missing");return;};
    if matched.next().is_some() || camera.target.as_ref()!=Some(&shot.target) {audit.fail("duplicate main view or render target mismatch");return;}
    let Some(phase)=phases.get(&view.retained_view_entity) else {audit.fail("main Transparent3d phase missing");return;};
    if phase.items.len()>50_000 {audit.fail("phase exceeds bounded item limit");return;}
    let mut text=String::new(); let mut relevant=RelevantItems::default();
    let _=writeln!(text,"phase index={} entity_bits={} frame={} tick={} main_camera={:?} render_view={entity:?} target={:?} items={}",shot.index,shot.entity.to_bits(),shot.frame,shot.tick,shot.camera,shot.target,phase.items.len());
    for (rank,(key,item)) in phase.items.iter().enumerate() {
        let main=item.entity.1.id();
        let Some((name,skid))=snapshot.names.get(&main) else {continue;};
        if let Err(reason)=relevant.admit(main.to_bits(),*skid) {audit.fail(reason);return;}
        let TransparentSortingInfo3d::Sorted {mesh_center,depth_bias}=item.sorting_info else {audit.fail("relevant AlwaysOnTop item");return;};
        let z=view.rangefinder3d().distance(&mesh_center);
        if !item.distance.is_finite() || !z.is_finite() || !depth_bias.is_finite() || !mesh_center.is_finite() {audit.fail("nonfinite relevant sort data");return;}
        let ready=pipelines.get_render_pipeline(item.pipeline).is_some();
        if !ready {audit.fail("relevant pipeline unready; descriptor not read");return;}
        let descriptor=pipelines.get_render_pipeline_descriptor(item.pipeline);
        let Some(depth)=descriptor.depth_stencil.as_ref() else {audit.fail("relevant depth state missing");return;};
        if *skid && (depth_bias!=0. || depth.bias.constant!=50 || depth.depth_write_enabled!=Some(false)) {
            audit.fail("Isolated skid sort/raster/write invariant differs");return;
        }
        let _=writeln!(text,"item index={} rank={rank} phase_entity_key={key:?} entity_bits={} name={name:?} centre={mesh_center:?} projected_z={z} sort_bias={depth_bias} sort_distance={} pipeline={:?} ready={ready} depth={depth:?} draw={:?}",shot.index,main.to_bits(),item.distance,item.pipeline,item.draw_function);
    }
    let RelevantItems {skids,smoke,..}=relevant;
    if skids!=1 {audit.fail("expected one queued skid draw in selected capture");return;}
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
}
