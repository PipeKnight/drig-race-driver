//! Static USA grandstand billboard diagnostic, NOT recovered crowd animation/LOD.
//! Bounded diagnostic. Source positions/sex/group are retained.
//! Source palette variant0/colourSet0 and fixed CRT-draw fixture5000 chosen explicitly.
//! Actual global RNG, timing and pose are not recovered.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use grid_formats::{binxml, crowd_placements, pssg, texture};
use grid_tools::route::Route;
use super::crowd_population;
use std::{collections::BTreeMap, error::Error, path::Path};
type AnyResult<T> = Result<T, Box<dyn Error>>;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Control,
    StaticBillboards,
    OneNeutralActor,
    OneArmPose,
    OneKeyHypothesis,
    OneKeyLeft,
    OneKeyMid,
    OneKeyRight,
    OneKeySweep,
    SelectedSourceControl,
    SelectedSourcePose,
}
impl Mode {
    fn selected_source(self)->bool {matches!(self,Self::SelectedSourceControl|Self::SelectedSourcePose)}
    pub fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "static-billboards" => Ok(Self::StaticBillboards),
            "one-neutral-actor" => Ok(Self::OneNeutralActor),
            "one-arm-pose" => Ok(Self::OneArmPose),
            "one-key-hypothesis" => Ok(Self::OneKeyHypothesis),
            "one-key-left" => Ok(Self::OneKeyLeft),
            "one-key-mid" => Ok(Self::OneKeyMid),
            "one-key-right" => Ok(Self::OneKeyRight),
            "one-key-sweep" => Ok(Self::OneKeySweep),
            "selected-source-control" => Ok(Self::SelectedSourceControl),
            "selected-source-pose" => Ok(Self::SelectedSourcePose),
            _ => Err(
                "--crowd-preview needs off, control, static-billboards, one-neutral-actor, one-arm-pose, one-key-hypothesis, one-key-left/mid/right/sweep or selected-source-control/pose",
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn validate(
        self,
        baseline: bool,
        static_reference: bool,
        detroit8: bool,
        lod0: bool,
        render: bool,
        frames: Option<u32>,
        no_perf: bool,
    ) -> Result<(), &'static str> {
        if self != Self::Off
            && !(baseline
                && static_reference
                && (self.selected_source() || detroit8)
                && lod0
                && render
                && frames.is_some_and(|n| (1..=600).contains(&n))
                && no_perf)
        {
            return Err(
                "--crowd-preview requires bounded baseline static LOD0 reference; legacy modes additionally require Detroit route8; no drive/perf/headless/collision",
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
struct Spec {
    uv: [f32; 4],
    dimensions: [f32; 2],
    colours: [Vec4; 3],
    instance_scale: [f32; 2],
}
#[derive(Clone, Debug)]
struct Row {
    position: Vec3,
    orientation_pair: [f32; 2],
    opaque_scalar: f32,
    cell: u32,
    sex: usize,
}
#[derive(Resource)]
pub struct Prepared {
    mode: Mode,
    actor: Option<super::crowd_rig::Actor>,
    selected_actor_row: Option<usize>,
    rows: Vec<Row>,
    specs: [Spec; 2],
    diffuse: Vec<u8>,
    tint: Vec<u8>,
    total: u32,
}
fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> AnyResult<&'a str> {
    e.attributes
        .iter()
        .find(|v| v.0 == name)
        .map(|v| v.1)
        .ok_or_else(|| format!("crowd missing {name}").into())
}
fn float(e: &binxml::Element<'_>, name: &str) -> AnyResult<f32> {
    let v: f32 = attr(e, name)?.parse()?;
    if !v.is_finite() {
        return Err("crowd nonfinite definition".into());
    }
    Ok(v)
}
fn one<'d, 'a>(
    d: &'d binxml::Document<'a>,
    parent: usize,
    tag: &str,
    name: Option<&str>,
) -> AnyResult<(usize, &'d binxml::Element<'a>)> {
    let mut it = d.elements[parent].children.clone().filter_map(|i| {
        let e = &d.elements[i];
        (e.name == tag && name.is_none_or(|n| attr(e, "name").ok() == Some(n))).then_some((i, e))
    });
    let e = it
        .next()
        .ok_or_else(|| format!("crowd {tag}/{name:?} missing"))?;
    if it.next().is_some() {
        return Err("duplicate crowd definition".into());
    }
    Ok(e)
}
// Explicit source-draw fixture, NOT replay of the original global CRT RNG.
// 0x986990 packs width in low half and height in high half; finite positive
// normal scales only. Its first branch uses x87, its second branch uses SSE.
fn source_scale_fixture(height_range: f32, width_range: f32) -> [f32; 2] {
    const DRAW: f32 = 5000.;
    const RATIO: f32 = f32::from_bits(0x38d1b717);
    fn truncate_half_normal(v: f32) -> f32 {
        // For the validated range (0.5,1.5), source half conversion discards
        // the low 13 mantissa bits. Return the exactly decoded scale as f32.
        f32::from_bits(v.to_bits() & !0x1fff)
    }
    let height =
        (1.0f64 - 2. * f64::from(DRAW) * f64::from(RATIO) * f64::from(height_range)) as f32;
    let r = DRAW * RATIO;
    let width = 1. + ((r * 2.) - 1.) * width_range;
    [truncate_half_normal(width), truncate_half_normal(height)]
}
fn spec(d: &binxml::Document<'_>, def: usize) -> AnyResult<Spec> {
    let (_, scale_stance) = one(d, def, "stance", Some("sitting"))?;
    let height_range = float(scale_stance, "randomHeightScale")?;
    let width_range = float(scale_stance, "randomWidthScale")?;
    if !(0. ..0.25).contains(&height_range) || !(0. ..0.25).contains(&width_range) {
        return Err("crowd diagnostic source scale range unsupported".into());
    }
    let instance_scale = source_scale_fixture(height_range, width_range);
    let (variant, _) = one(d, def, "textureVariant", Some("texVariant0"))?;
    let (bd, _) = one(d, variant, "billboardData", None)?;
    let (_, s) = one(d, bd, "stance", Some("sitting"))?;
    let uv = [
        float(s, "u0")?,
        float(s, "v0")?,
        float(s, "u1")?,
        float(s, "v1")?,
    ];
    let dimensions = [float(s, "width")?, float(s, "height")?];
    if !(uv.iter().all(|v| (0. ..=1.).contains(v))
        && uv[0] < uv[2]
        && uv[1] < uv[3]
        && dimensions.iter().all(|v| *v > 0. && *v < 5.))
    {
        return Err("crowd UV/dimensions invalid".into());
    }
    let (set, _) = one(d, variant, "colourSet", Some("colourSet0"))?;
    let cs: Vec<_> = d.elements[set]
        .children
        .clone()
        .map(|i| &d.elements[i])
        .collect();
    if cs.len() != 3 || cs.iter().any(|c| c.name != "colour") {
        return Err("crowd requires three source tint colours".into());
    }
    let mut colours = [Vec4::ZERO; 3];
    for (dst, c) in colours.iter_mut().zip(cs) {
        *dst = Vec4::new(float(c, "red")?, float(c, "green")?, float(c, "blue")?, 1.);
    }
    Ok(Spec {
        uv,
        dimensions,
        colours,
        instance_scale,
    })
}
fn fixture_group<'m, 'p, 'asset>(
    groups: &'m [crowd_population::PreparedGroup<'p, 'asset>], name: &[u8],
) -> AnyResult<&'m crowd_population::PreparedGroup<'p, 'asset>> {
    let mut matches = groups.iter().filter(|g| g.source.instance_set == name);
    let group = matches.next().ok_or("grandstand source group absent")?;
    if matches.next().is_some() { return Err("duplicate grandstand source group".into()); }
    Ok(group)
}
fn fixture_template<'a>(
    group: &'a crowd_population::PreparedGroup<'_, '_>,
    map_name: &str,
    definition: &str,
) -> AnyResult<&'a crowd_population::Template> {
    let crowd_population::Resolution::StaticTemplates(templates) = &group.resolution else {
        return Err("grandstand metadata unresolved or moving".into());
    };
    let [template] = templates.as_slice() else {
        return Err("grandstand diagnostic requires exactly one map".into());
    };
    if template.map_name != map_name
        || template.definition_name != definition
        || template.definition_set != "usa"
        || template.definition_origin != crowd_population::DefinitionOrigin::Primary
        || template.stance_name != "sitting"
    {
        return Err("grandstand instance map changed".into());
    }
    Ok(template)
}
fn preflight_selected_rows(current: usize, additional: usize) -> Result<(), &'static str> {
    if current.checked_add(additional).is_none_or(|n| n > 100_000) {
        return Err("crowd preview source rows exceed bound");
    }
    Ok(())
}

/// Explicit bounded diagnostic selection. No new file/CLI parser or native clock.
pub(crate) struct SelectedActorRequest {
    pub context:crowd_population::SelectedContext,
    pub group:Vec<u8>,pub map_name:String,pub placement_ordinal:usize,
    pub skin_id:Vec<u8>,pub modifier_instance_id:Vec<u8>,
    pub variant_ordinal:usize,pub colour_set_ordinal:usize,
    pub bank_coordinate:f32,pub experimental_performance:Option<u32>,pub experimental_draw:u32,
}
/// Values come only from the existing main argument parser; no config file/default policy.
pub(crate) fn selected_request_from_cli(
    mode:Mode, context:Option<&str>, target:Option<&str>, coordinate:Option<&str>,
    performance:Option<&str>, draw:Option<&str>,
)->AnyResult<Option<SelectedActorRequest>> {
    let values=[context,target,coordinate,performance,draw];
    if !mode.selected_source() {
        if values.iter().any(Option::is_some){return Err("crowd actor selectors require selected-source-control/pose".into());}
        return Ok(None);
    }
    let context=context.ok_or("selected source requires --crowd-actor-context")?;
    let target=target.ok_or("selected source requires --crowd-actor-target")?;
    let coordinate=coordinate.ok_or("selected source requires --crowd-bank-coordinate")?;
    let performance=performance.ok_or("selected source requires --crowd-actor-performance default|0..15")?;
    let draw=draw.ok_or("selected source requires --crowd-actor-draw")?;
    fn fields<const N:usize>(raw:&str)->AnyResult<[&str;N]> {
        let parts=raw.split(',').collect::<Vec<_>>();
        if parts.len()!=N || parts.iter().any(|p|p.is_empty() || p.len()>256 || p.contains(['\0','\r','\n'])){return Err("crowd actor field count/empty/length/control failure".into());}
        parts.try_into().map_err(|_|"crowd actor field count failure".into())
    }
    let [primary,fallback,map_group,billboard_userdata]=fields::<4>(context)?;
    let optional=|s:&str|if s=="-"{None}else{Some(s.to_owned())};
    let [group,map_name,placement,skin,modifier,variant,colour_set]=fields::<7>(target)?;
    let bank_coordinate:f32=coordinate.parse()?;
    if !bank_coordinate.is_finite(){return Err("nonfinite explicit crowd bank coordinate".into());}
    let experimental_performance=if performance=="default"{None}else{
        let value:u32=performance.parse()?;
        if value>=16{return Err("crowd performance index outside source0..15".into());} Some(value)
    };
    Ok(Some(SelectedActorRequest{
        context:crowd_population::SelectedContext{primary_definition_set:optional(primary),fallback_definition_set:optional(fallback),map_group:map_group.to_owned(),billboard_userdata:billboard_userdata.to_owned()},
        group:group.as_bytes().to_vec(),map_name:map_name.to_owned(),placement_ordinal:placement.parse()?,
        skin_id:skin.as_bytes().to_vec(),modifier_instance_id:modifier.as_bytes().to_vec(),
        variant_ordinal:variant.parse()?,colour_set_ordinal:colour_set.parse()?,bank_coordinate,
        experimental_performance,experimental_draw:draw.parse()?,
    }))
}
pub fn prepare(root:&Path,route:&Route,mode:Mode)->AnyResult<Prepared>{prepare_with_request(root,route,mode,None)}
pub(crate) fn prepare_with_request(root:&Path,route:&Route,mode:Mode,request:Option<&SelectedActorRequest>)->AnyResult<Prepared>{
    if mode.selected_source(){return prepare_selected_actor(root,route,mode,request.ok_or("selected source actor requires explicit typed bounded request")?);}
    if request.is_some(){return Err("source actor request supplied to legacy preview".into());}
    prepare_legacy(root,route,mode)
}
fn source_field<'a>(fields:&'a crowd_population::Fields,key:&str)->AnyResult<Option<&'a str>>{
    let mut matches=fields.iter().filter(|(k,_)|k==key);let value=matches.next().map(|(_,v)|v.as_str());
    if matches.next().is_some(){return Err("duplicate selected source field".into());} Ok(value)
}
fn selected_tints(template:&crowd_population::Template,variant:usize,set:usize)->AnyResult<[Vec4;3]>{
    let variant=template.variants.get(variant).ok_or("selected variant ordinal absent")?;
    let shift=variant.skin_fields.as_ref().map(|f|source_field(f,"textureShift")).transpose()?.flatten().map(str::parse::<i32>).transpose()?.unwrap_or(0) as u8;
    if shift!=0{return Err("selected nonzero source textureShift mapping remains unresolved".into());}
    let set=variant.colour_sets.get(set).ok_or("selected colourSet ordinal absent")?;
    let [a,b,c]=set.colours.as_slice() else{return Err("existing crowd tint material requires three actual colours".into())};
    let colour=|fields:&crowd_population::Fields|->AnyResult<Vec4>{
        let mut rgb=[0.;3];for (v,key) in rgb.iter_mut().zip(["red","green","blue"]){*v=source_field(fields,key)?.ok_or("source tint component absent")?.parse()?;}
        let result=Vec4::new(rgb[0],rgb[1],rgb[2],1.);if !result.is_finite(){return Err("nonfinite selected tint".into());}Ok(result)
    };Ok([colour(a)?,colour(b)?,colour(c)?])
}
fn shader_texture<'a>(bindings:&[grid_formats::material::TextureBinding<'a>],shader:&[u8],sampler:&[u8])->AnyResult<&'a [u8]>{
    let mut entries=bindings.iter().filter(|b|b.shader==shader && b.sampler==sampler);
    let b=entries.next().ok_or("selected source shader texture absent")?;
    if entries.next().is_some() || b.container.is_some(){return Err("selected source shader texture ambiguous/external".into());}Ok(b.texture)
}
fn unique_texture<'t,'a>(textures:&'t [texture::Texture<'a>],id:&[u8])->AnyResult<&'t texture::Texture<'a>> {
    let mut matches=textures.iter().filter(|t|t.id==id);
    let result=matches.next().ok_or("selected texture object absent")?;
    if matches.next().is_some(){return Err("selected texture object ambiguous".into());}
    Ok(result)
}
fn bounded_source_bytes(path:&Path,limit:u64)->AnyResult<Vec<u8>>{
    if std::fs::metadata(path)?.len()>limit{return Err("selected source asset exceeds diagnostic byte cap".into());}
    let bytes=std::fs::read(path)?;if bytes.len() as u64>limit{return Err("selected source asset grew beyond byte cap".into());}Ok(bytes)
}
fn prepare_selected_actor(root:&Path,route:&Route,mode:Mode,request:&SelectedActorRequest)->AnyResult<Prepared>{
    if !request.bank_coordinate.is_finite(){return Err("nonfinite explicit bank coordinate".into());}
    let dataset_graph=grid_tools::tyre_smoke::read_xml(&root.join(route.track_file("organism_track_dataset.xml")))?;
    let bindings=crowd_population::dataset(&dataset_graph)?;
    let definition_bytes=bounded_source_bytes(&root.join("anims/crowdDefs.xml"),4*1024*1024)?;let definitions=binxml::read(&definition_bytes)?;
    let placement_bytes=bounded_source_bytes(&root.join(route.file("crowd_standing.bin")),32*1024*1024)?;let placements=crowd_placements::read(&placement_bytes)?;
    let c=&request.context;
    let metadata=crowd_population::prepare(&placements,&definitions,&bindings.pssg,crowd_population::DefinitionSelection::Exact{primary_definition_set:c.primary_definition_set.as_deref(),fallback_definition_set:c.fallback_definition_set.as_deref(),map_group:&c.map_group,billboard_userdata:&c.billboard_userdata})?;
    let group=fixture_group(&metadata.groups,&request.group)?;
    let crowd_population::Resolution::StaticTemplates(templates)=&group.resolution else{return Err("selected source group is unresolved/moving".into())};
    let mut found=templates.iter().filter(|t|t.map_name==request.map_name);
    let template=found.next().ok_or("selected source map absent")?;if found.next().is_some(){return Err("selected source map ambiguous".into());}
    let crowd_placements::Placements::Standing(points)=&group.source.placements else{return Err("selected source group is not static placement".into())};
    let point=points.get(request.placement_ordinal).ok_or("selected placement ordinal absent")?;
    if !point.position.iter().all(|v|v.is_finite()){return Err("selected source placement nonfinite".into());}
    let hla_bytes=bounded_source_bytes(&root.join("anims/crowd_hla.xml"),4*1024*1024)?;let hla_doc=binxml::read(&hla_bytes)?;
    let library=grid_tools::crowd_selection::Library::parse(&hla_doc).map_err(|e|format!("source HLA: {e:?}"))?;
    let mut draws=0u32;
    let selected=super::crowd_selected_clip::select_template(&metadata,template,&library,request.experimental_performance,&mut ||{draws+=1;request.experimental_draw})?.ok_or("selected source tree returned Empty")?;
    // AUTHORED direct exact-ID diagnostic lookup, source161. No native map claim.
    let bank_bytes=bounded_source_bytes(&root.join(&selected.animation_bank.relative_path),64*1024*1024)?;let bank=pssg::read_tree(&bank_bytes)?;
    let geometry_resource=template.mesh.as_ref().ok_or("selected template has no skinned source resource")?;
    let geometry_bytes=bounded_source_bytes(&root.join(&geometry_resource.relative_path),64*1024*1024)?;let geometry=pssg::read_tree(&geometry_bytes)?;
    let prior_source=super::crowd_pose::Source::read(&geometry,&request.skin_id,&request.modifier_instance_id)?;
    let prior=prior_source.saved_locals();
    let (rig,posed)=super::crowd_selected_clip::pose_bound_clip(&selected,&bank,&geometry,super::crowd_selected_clip::DiagnosticPoseRequest{clip:super::crowd_selected_clip::ExplicitClipBinding{raw_ordinal:selected.raw_ordinal,clip_id:selected.candidate.raw_name.as_bytes()},bank_coordinate:request.bank_coordinate,skin_id:&request.skin_id,modifier_instance_id:&request.modifier_instance_id,caller_prior_locals:&prior,policy:super::crowd_pose::PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta{mirrored:false}})?;
    let neutral=rig.sample(&prior,&[],super::crowd_pose::PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta{mirrored:false})?;
    let source_min=neutral.positions.iter().copied().map(Vec3::from_array).fold(Vec3::splat(f32::INFINITY),Vec3::min);
    let source_max=neutral.positions.iter().copied().map(Vec3::from_array).fold(Vec3::splat(f32::NEG_INFINITY),Vec3::max);
    let foot_offset=Vec3::new(-(source_min.x+source_max.x)*0.5,-source_min.y,-(source_min.z+source_max.z)*0.5);
    if !source_min.is_finite() || !source_max.is_finite() || !foot_offset.is_finite(){return Err("selected neutral bounds/anchor invalid".into());}
    let colours=selected_tints(template,request.variant_ordinal,request.colour_set_ordinal)?;
    // Same existing material; actor path consumes colours, not billboard dimensions.
    let spec=Spec{uv:[0.,0.,1.,1.],dimensions:[1.,1.],colours,instance_scale:[1.,1.]};
    let texture_bindings=grid_formats::material::read_texture_bindings(&geometry)?;
    let diffuse_id=shader_texture(&texture_bindings,&rig.material_shader,b"TDiffuseAlphaMap")?;
    let tint_id=shader_texture(&texture_bindings,&rig.material_shader,b"TTintMap")?;
    let textures=texture::read_named(&geometry,&[diffuse_id,tint_id])?;
    let diffuse=unique_texture(&textures,diffuse_id)?;
    let tint=unique_texture(&textures,tint_id)?;
    let (max_position_error,max_palette_error,max_inverse_affine_error)=rig.diagnostics()?;
    let vertices=if mode==Mode::SelectedSourceControl{neutral}else{posed};
    let (original_positions,original_normals)=rig.original_vertices();
    let actor=super::crowd_rig::Actor{sweep:None,pose:None,source_positions:original_positions.to_vec(),source_normals:original_normals.to_vec(),positions:vertices.positions,normals:vertices.normals,uv:rig.uv.clone(),indices:rig.indices.clone(),diffuse:diffuse.validated_dds().0,tint:tint.validated_dds().0,source_min,source_max,foot_offset,max_position_error,max_palette_error,max_inverse_affine_error};
    println!("crowd_selected_source_prepare mode={mode:?} group={:?} map={} source_ordinal={} cell={} mesh={} skin={:?} modifier={:?} raw_ordinal={} clip={} bank={} coordinate={} performance={:?} caller_draw={} draws={} variant={} colour_set={} shader={:?} diffuse={:?} tint={:?} source_name_lookup=AUTHORED-exact-selected-bank native_map_clock_delta_parity=false",request.group,template.map_name,request.placement_ordinal,point.cell,geometry_resource.relative_path,request.skin_id,request.modifier_instance_id,selected.raw_ordinal,selected.candidate.raw_name,selected.animation_bank.relative_path,request.bank_coordinate,request.experimental_performance,request.experimental_draw,draws,request.variant_ordinal,request.colour_set_ordinal,rig.material_shader,diffuse_id,tint_id);
    Ok(Prepared{mode,actor:Some(actor),selected_actor_row:Some(0),rows:vec![Row{position:Vec3::from_array(point.position),orientation_pair:point.orientation_pair,opaque_scalar:point.opaque_scalar,cell:point.cell,sex:0}],specs:[spec.clone(),spec],diffuse:Vec::new(),tint:Vec::new(),total:placements.total_instances})
}
fn prepare_legacy(root: &Path, route: &Route, mode: Mode) -> AnyResult<Prepared> {
    if route.path() != "tracks/usa/detroit/route_8" {
        return Err("crowd preview only Detroit route8".into());
    }
    // Explicit USA/generic context is this bounded diagnostic fixture, not an
    // automatic region producer. Normal setup remains untouched when Off.
    let dataset_graph = grid_tools::tyre_smoke::read_xml(
        &root.join(route.track_file("organism_track_dataset.xml")),
    )?;
    let bindings = crowd_population::dataset(&dataset_graph)?;
    let defs = std::fs::read(root.join("anims/crowdDefs.xml"))?;
    let doc = binxml::read(&defs)?;
    let placements = std::fs::read(root.join(route.file("crowd_standing.bin")))?;
    let catalogue = crowd_placements::read(&placements)?;
    let metadata = crowd_population::prepare(
        &catalogue, &doc, &bindings.pssg,
        crowd_population::DefinitionSelection::Exact {
            primary_definition_set: Some("usa"),
            fallback_definition_set: Some("generic"),
            map_group: "generic",
            billboard_userdata: "billboardusa",
        },
    )?;
    // Retain the diagnostic's exact expected resource contract, without a
    // text-layout-dependent substring check or general filename whitelist.
    for (file, user) in [
        ("cr_us_billboard.pssg", "billboardusa"),
        ("cr_male_usa.pssg", "maleVariant0"),
        ("cr_female_usa.pssg", "femaleVariant0"),
    ] {
        if metadata.dataset_resources.get(user).map(|r| r.relative_path.as_str())
            != Some(format!("anims/{file}").as_str())
        {
            return Err("Detroit USA dataset changed".into());
        }
    }
    let male = fixture_template(fixture_group(&metadata.groups, b"m_gr")?, "maleSit", "crowdMale")?;
    let female = fixture_template(fixture_group(&metadata.groups, b"f_gr")?, "femaleSit", "crowdFemale")?;
    if male.billboard != female.billboard {
        return Err("grandstand atlas bindings differ".into());
    }
    let specs = [
        spec(metadata.definitions, male.definition_element)?,
        spec(metadata.definitions, female.definition_element)?,
    ];
    println!(
        "crowd_metadata source={} resolved_static={} unresolved_static={} moving={} dataset_records={} primary={:?} fallback={:?} map_group={} billboard={} rendered_policy=diagnostic-grandstand-only",
        metadata.source.total_instances, metadata.resolved_static_records,
        metadata.unresolved_static_records, metadata.unsupported_moving_records,
        bindings.source.children.len(), metadata.context.primary_definition_set,
        metadata.context.fallback_definition_set, metadata.context.map_group,
        metadata.context.billboard_userdata,
    );
    let mut rows = Vec::new();
    for group in &metadata.groups {
        let g = group.source;
        let sex = match g.instance_set {
            b"m_gr" => 0,
            b"f_gr" => 1,
            _ => continue,
        };
        let crowd_placements::Placements::Standing(points) = &g.placements else {
            return Err("grandstand moving paths unsupported".into());
        };
        preflight_selected_rows(rows.len(), points.len())?;
        for p in points {
            rows.push(Row {
                position: Vec3::from_array(p.position),
                orientation_pair: p.orientation_pair,
                opaque_scalar: p.opaque_scalar,
                cell: p.cell,
                sex,
            });
        }
    }
    if rows.is_empty() || rows.len() > 100_000 {
        return Err("crowd preview count absent/exceeds bound".into());
    }
    if matches!(
        mode,
        Mode::OneNeutralActor | Mode::OneArmPose | Mode::OneKeyHypothesis | Mode::OneKeyLeft | Mode::OneKeyMid | Mode::OneKeyRight | Mode::OneKeySweep
    ) {
        let row = rows.get(4275).ok_or("neutral actor row4275 absent")?;
        if row.sex != 1 || row.cell != 89 || !row.position.is_finite() {
            return Err("neutral female row4275/cell89 differs".into());
        }
        let selector = match mode {
            Mode::OneKeyLeft => Some(super::crowd_rig::StoredCoordinate::Left),
            Mode::OneKeyMid => Some(super::crowd_rig::StoredCoordinate::Mid),
            Mode::OneKeyRight => Some(super::crowd_rig::StoredCoordinate::Right),
            _ => None,
        };
        let actor = if mode == Mode::OneKeySweep {
            super::crowd_rig::prepare_sweep(root)?
        } else if let Some(selector) = selector {
            super::crowd_rig::prepare_stored_coordinate(root, selector)?
        } else if mode == Mode::OneKeyHypothesis {
            super::crowd_rig::prepare_key_hypothesis(root)?
        } else if mode == Mode::OneArmPose {
            super::crowd_rig::prepare_arm_pose(root)?
        } else {
            super::crowd_rig::prepare(root)?
        };
        println!(
            "crowd_neutral_selected ordinal=4275 cell=89 sex=female source_position={:?} model_scale=1 yaw=authored-camera-facing-XZ foot_alignment=authored-neutral-bounds clocks=off",
            row.position
        );
        return Ok(Prepared {
            mode,
            actor: Some(actor),
            selected_actor_row: None,
            rows,
            specs,
            diffuse: Vec::new(),
            tint: Vec::new(),
            total: catalogue.total_instances,
        });
    }
    let atlas = std::fs::read(root.join(&male.billboard.relative_path))?;
    let tree = pssg::read_tree(&atlas)?;
    let named = texture::read_named(&tree, &[b"cr_US_billboard_d.tga", b"cr_US_billboard_m.tga"])?;
    let d = named
        .iter()
        .find(|t| t.id == b"cr_US_billboard_d.tga")
        .ok_or("crowd diffuse absent")?;
    let m = named
        .iter()
        .find(|t| t.id == b"cr_US_billboard_m.tga")
        .ok_or("crowd tint absent")?;
    if d.width != 512
        || d.height != 256
        || d.format != texture::Format::Bc3
        || m.width != 512
        || m.height != 256
        || m.format != texture::Format::Bc1
    {
        return Err("crowd atlas format changed".into());
    }
    Ok(Prepared {
        mode,
        actor: None,
        selected_actor_row: None,
        rows,
        specs,
        diffuse: d.validated_dds().0,
        tint: m.validated_dds().0,
        total: catalogue.total_instances,
    })
}

// Only the authored frozen-pulse diagnostic uses these selected-grandstand
// ordinals. Coordinates always come from current validated Prepared rows.
#[derive(Clone, Copy)]
struct FlashRequest {
    ordinal: usize,
    sex: usize,
    cell: u32,
}
const FLASH_REQUESTS: [FlashRequest; 3] = [
    FlashRequest {
        ordinal: 1907,
        sex: 0,
        cell: 88,
    },
    FlashRequest {
        ordinal: 1902,
        sex: 0,
        cell: 89,
    },
    FlashRequest {
        ordinal: 4275,
        sex: 1,
        cell: 89,
    },
];
fn select_flash_rows(
    rows: &[Row],
    has_camera: [bool; 2],
    requests: &[FlashRequest],
) -> AnyResult<Vec<super::flash_fixture::Anchor>> {
    if requests.len() != 3 {
        return Err("flash fixture requires exactly three requested source rows".into());
    }
    let mut anchors = Vec::with_capacity(3);
    for (i, request) in requests.iter().enumerate() {
        if requests[..i].iter().any(|r| r.ordinal == request.ordinal) {
            return Err("duplicate flash fixture source ordinal".into());
        }
        let row = rows
            .get(request.ordinal)
            .ok_or("flash fixture source ordinal missing")?;
        if request.sex > 1 || row.sex != request.sex || row.cell != request.cell {
            return Err("flash fixture source group/cell changed".into());
        }
        if !has_camera.get(request.sex).copied().unwrap_or(false) {
            return Err("flash fixture source definition hasCamera is false".into());
        }
        if !row.position.is_finite() {
            return Err("nonfinite flash fixture source origin".into());
        }
        anchors.push(super::flash_fixture::Anchor {
            source_index: u32::try_from(request.ordinal)?,
            source_cell: row.cell,
            position: row.position.to_array(),
        });
    }
    Ok(anchors)
}
pub(crate) fn flash_fixture_anchors(
    root: &Path,
    prepared: &Prepared,
) -> AnyResult<Vec<super::flash_fixture::Anchor>> {
    if prepared.mode != Mode::StaticBillboards
        || prepared.total != 6854
        || prepared.rows.len() != 4667
    {
        return Err("flash fixture needs exact admitted Detroit static crowd population".into());
    }
    let raw = std::fs::read(root.join("anims/crowdDefs.xml"))?;
    let doc = binxml::read(&raw)?;
    let (country, _) = one(&doc, 0, "crowdDefSet", Some("usa"))?;
    let mut has_camera = [false; 2];
    for (slot, name) in ["crowdMale", "crowdFemale"].into_iter().enumerate() {
        let (_, definition) = one(&doc, country, "crowdDef", Some(name))?;
        has_camera[slot] = match attr(definition, "hasCamera")? {
            "0" => false,
            "1" => true,
            _ => return Err("unsupported crowd hasCamera flag".into()),
        };
    }
    select_flash_rows(&prepared.rows, has_camera, &FLASH_REQUESTS)
}

const SHADER: Handle<Shader> = uuid_handle!("3f1983b1-35cd-4ea7-986e-22b81999f3a2");
type CrowdMaterial = ExtendedMaterial<StandardMaterial, Tint>;
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct Tint {
    #[texture(100)]
    #[sampler(101)]
    map: Handle<Image>,
    #[uniform(102)]
    colour0: Vec4,
    #[uniform(103)]
    colour1: Vec4,
    #[uniform(104)]
    colour2: Vec4,
}
impl MaterialExtension for Tint {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}
pub fn install(app: &mut App, prepared: Prepared) -> AnyResult<()> {
    if !prepared.mode.selected_source() && !matches!(
        prepared.mode,
        Mode::OneNeutralActor | Mode::OneArmPose | Mode::OneKeyHypothesis | Mode::OneKeyLeft | Mode::OneKeyMid | Mode::OneKeyRight | Mode::OneKeySweep
    ) {
        println!(
            "crowd_preview mode={:?} total_source={} selected_grandstand={} variant=0 colour_set=0 scale_fixture_draws=5000,5000 poses=unswayed-static-billboards alpha=Bevy-A2C lighting=PBR source_parity=false shadows=disabled unknown_scalar=opaque rotation_pair=retained-billboard-source-faces-camera",
            prepared.mode,
            prepared.total,
            prepared.rows.len()
        );
    }
    if !prepared.mode.selected_source() && !matches!(
        prepared.mode,
        Mode::StaticBillboards | Mode::OneNeutralActor | Mode::OneArmPose | Mode::OneKeyHypothesis | Mode::OneKeyLeft | Mode::OneKeyMid | Mode::OneKeyRight | Mode::OneKeySweep
    ) {
        return Ok(());
    }
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(
            include_str!("crowd_preview.wgsl"),
            "grid_crowd_preview.wgsl",
        ),
    )?;
    if prepared.mode == Mode::OneKeySweep {
        app.get_sub_app_mut(bevy::render::RenderApp).ok_or("sweep renderer absent")?
            .add_systems(bevy::render::ExtractSchedule, extract_sweep);
        app.add_systems(PostUpdate, advance_sweep.before(bevy::camera::visibility::VisibilitySystems::CheckVisibility))
            .add_systems(Last, request_sweep.before(bevy::diagnostic::update_frame_count));
    }
    app.insert_resource(prepared)
        .add_plugins(MaterialPlugin::<CrowdMaterial>::default())
        .add_systems(PostStartup, spawn);
    Ok(())
}
#[derive(Default)]
struct Batch {
    p: Vec<[f32; 3]>,
    n: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    indices: Vec<u32>,
}
fn card(batch: &mut Batch, row: &Row, s: &Spec, eye: Vec3) -> Result<(), &'static str> {
    let d = eye - row.position;
    let len = d.x.hypot(d.z);
    if !row.position.is_finite() || !eye.is_finite() || !d.is_finite() || !len.is_finite() {
        return Err("nonfinite static crowd card basis");
    }
    if len < 0.0001 {
        return Err("camera coincides with crowd XZ");
    }
    if !s
        .dimensions
        .into_iter()
        .chain(s.instance_scale)
        .all(|v| v.is_finite() && v > 0.)
        || !s.uv.into_iter().all(f32::is_finite)
    {
        return Err("nonfinite/invalid static crowd card specification");
    }
    let right = Vec3::new(-d.z / len, 0., d.x / len);
    let forward = Vec3::new(d.x / len, 0., d.z / len);
    if !right.is_finite() || !forward.is_finite() {
        return Err("nonfinite static crowd card axes");
    }
    // Source table: dimension-bearing unswayed corners, u1 at negative local X.
    // This fixture uses explicit draw5000 scales. f32 corners and PBR normals
    // remain diagnostic adaptations; actual actor RNG, sway/near LOD are open.
    let first = u32::try_from(batch.p.len()).map_err(|_| "crowd vertex index overflow")?;
    first.checked_add(3).ok_or("crowd vertex index overflow")?;
    let half_width = s.dimensions[0] * 0.5;
    let mut positions = [[0.; 3]; 4];
    let mut normals = [[0.; 3]; 4];
    let mut uvs = [[0.; 2]; 4];
    for (i, (x, y, u, v)) in [
        (-half_width, 0., s.uv[2], s.uv[3]),
        (-half_width, s.dimensions[1], s.uv[2], s.uv[1]),
        (half_width, 0., s.uv[0], s.uv[3]),
        (half_width, s.dimensions[1], s.uv[0], s.uv[1]),
    ]
    .into_iter()
    .enumerate()
    {
        let position =
            row.position + right * x * s.instance_scale[0] + Vec3::Y * y * s.instance_scale[1];
        // Authored curved-normal approximation, not source backlight/fog.
        let normal = (forward + right * (4. * x.signum())).normalize();
        // Finite basis alone does not prevent scaled corner/addition overflow.
        // Validate all outputs before appending any partial card to the batch.
        if !position.is_finite() || !normal.is_finite() {
            return Err("nonfinite static crowd card vertex");
        }
        positions[i] = position.to_array();
        normals[i] = normal.to_array();
        uvs[i] = [u, v];
    }
    batch.p.extend(positions);
    batch.n.extend(normals);
    batch.uv.extend(uvs);
    batch
        .indices
        .extend([first, first + 1, first + 2, first + 3, first + 2, first + 1]);
    Ok(())
}
fn spawn(
    mut commands: Commands,
    prepared: Res<Prepared>,
    view: Query<(&Transform, &Msaa), With<super::ViewerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CrowdMaterial>>,
    settings: Res<super::graphics::Settings>,
) {
    let (camera, msaa) = view.single().expect("one static preview camera");
    assert_eq!(
        *msaa,
        Msaa::Sample4,
        "bounded crowd preview requires actual MSAA4"
    );
    let (diffuse_bytes, tint_bytes) = if let Some(actor) = &prepared.actor {
        (&actor.diffuse, &actor.tint)
    } else {
        (&prepared.diffuse, &prepared.tint)
    };
    let diffuse = images.add(
        super::texture_upload::image(
            diffuse_bytes,
            settings.sampler(false),
            RenderAssetUsages::default(),
        )
        .expect("validated crowd DDS"),
    );
    let tint = images.add(
        super::texture_upload::image_linear(
            tint_bytes,
            settings.sampler(false),
            RenderAssetUsages::default(),
        )
        .expect("validated crowd tint DDS"),
    );
    let handles: Vec<_> = prepared
        .specs
        .iter()
        .map(|s| {
            materials.add(ExtendedMaterial {
                base: StandardMaterial {
                    base_color_texture: Some(diffuse.clone()),
                    alpha_mode: if prepared.actor.is_some() {
                        AlphaMode::Opaque
                    } else {
                        AlphaMode::AlphaToCoverage
                    },
                    perceptual_roughness: 1.,
                    reflectance: 0.,
                    cull_mode: None,
                    double_sided: true,
                    ..default()
                },
                extension: Tint {
                    map: tint.clone(),
                    colour0: s.colours[0],
                    colour1: s.colours[1],
                    colour2: s.colours[2],
                },
            })
        })
        .collect();
    if let Some(actor) = &prepared.actor {
        let actor_row=prepared.selected_actor_row.unwrap_or(4275);
        let row = &prepared.rows[actor_row];
        let d = camera.translation - row.position;
        let len = d.x.hypot(d.z);
        assert!(
            d.is_finite() && len.is_finite() && len > 0.0001,
            "invalid one-actor authored yaw"
        );
        // Camera-facing yaw is authored. Source orientation_pair remains opaque.
        let forward = Vec3::new(d.x / len, 0., d.z / len);
        let yaw = Quat::from_rotation_y(forward.x.atan2(forward.z));
        let translation = row.position + yaw * actor.foot_offset;
        assert!(translation.is_finite());
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, actor.positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, actor.normals.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, actor.uv.clone());
        mesh.insert_indices(Indices::U16(actor.indices.clone()));
        let sweep_bounds = actor.sweep.as_ref().map(|_| {
            use bevy::camera::primitives::MeshAabb;
            mesh.compute_aabb().expect("finite sweep initial bounds")
        });
        let actor_mesh = meshes.add(mesh);
        let actor_transform = Transform { translation, rotation: yaw, ..default() };
        let actor_entity = commands.spawn((
            Name::new(
                if prepared.mode.selected_source() { "crowd-preview/selected-source-actor-diagnostic" } else if actor.pose.as_ref().is_some_and(|p| p.stored_key_hypothesis) {
                    "crowd-preview/one-key-HYPOTHESIS-female4275"
                } else if actor.pose.is_some() {
                    "crowd-preview/one-authored-arm-female4275"
                } else {
                    "crowd-preview/one-neutral-female4275"
                },
            ),
            Mesh3d(actor_mesh.clone()),
            MeshMaterial3d(handles[1].clone()),
            actor_transform,
            NotShadowCaster,
        )).id();
        if let Some(data) = &actor.sweep {
            commands.entity(actor_entity).insert((SweepActor, sweep_bounds.unwrap(), bevy::render::batching::NoAutomaticBatching));
            commands.insert_resource(Sweep {
                data: data.clone(), entity: actor_entity, mesh: actor_mesh,
                material: handles[1].clone(), transform_bits: transform_bits(&actor_transform),
                base_fingerprint: format!("{:?}",materials.get(&handles[1]).unwrap().base),
                uv_bits: actor.uv.iter().map(|p|p.map(f32::to_bits)).collect(), indices: actor.indices.clone(),
                positions: actor.positions.clone(), normals: actor.normals.clone(), step: None, frame: 0,
                directory: None, ledger: std::sync::Arc::new(std::sync::Mutex::new(SweepLedger {
                    failed: false, extracted: 0, shots: std::array::from_fn(|_|None),
                })),
            });
        }
        if prepared.mode.selected_source() {
            println!("crowd_selected_source_spawn mode={:?} prepared_row={} vertices={} indices={} material=existing-near-PBR-tint-opaque-two-sided textures=actual-selected-shader shift=0 yaw=authored-camera-facing root_prior=explicit-source-control native_map_clock_delta_parity=false",prepared.mode,actor_row,actor.positions.len(),actor.indices.len());
        } else {
        let label = if actor.pose.as_ref().is_some_and(|p| p.stored_key_hypothesis) {
            "crowd_key_hypothesis_spawn"
        } else if actor.pose.is_some() {
            "crowd_authored_arm_spawn"
        } else {
            "crowd_neutral_spawn"
        };
        println!(
            "{label} actors=1 ordinal=4275 row_position={:?} neutral_bounds={:?}/{:?} foot_offset={:?} raw_position_count={} raw_normal_count={} position_error={} palette_error={} inverse_affine_error={} yaw={:?} material=authored-near-PBR-tint-opaque-two-sided texture=near1024x512 textureShift=0 UV=source normals=weighted-normalized raw_attributes=retained shadows=off original_pose_parity=false",
            row.position,
            actor.source_min,
            actor.source_max,
            actor.foot_offset,
            actor.source_positions.len(),
            actor.source_normals.len(),
            actor.max_position_error,
            actor.max_palette_error,
            actor.max_inverse_affine_error,
            yaw
        );
        }
        return;
    }
    let mut batches: BTreeMap<(usize, u32), Batch> = BTreeMap::new();
    for row in &prepared.rows {
        card(
            batches.entry((row.sex, row.cell)).or_default(),
            row,
            &prepared.specs[row.sex],
            camera.translation,
        )
        .expect("validated static billboard expansion");
    }
    for ((sex, cell), b) in batches {
        let mut m = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.p);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, b.n);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, b.uv);
        m.insert_indices(Indices::U32(b.indices));
        commands.spawn((
            Name::new(format!("crowd-preview/sex{sex}/cell{cell}")),
            Mesh3d(meshes.add(m)),
            MeshMaterial3d(handles[sex].clone()),
            Transform::IDENTITY,
            NotShadowCaster,
        ));
    }
    // Retain raw source metadata: neither field has been reinterpreted or used
    // to fabricate time/RNG. Reader tests check their preserved float bits.
    let _raw = prepared.rows.iter().fold(0u32, |v, r| {
        v ^ r.orientation_pair[0].to_bits()
            ^ r.orientation_pair[1].to_bits()
            ^ r.opaque_scalar.to_bits()
    });
}
// One finite Extract-frame diagnostic. No wall-clock or source playback timer.
#[derive(Component)]
struct SweepActor;
#[derive(Component)]
struct SweepShot(usize);
struct Shot {
    entity: Entity,
    target: Handle<Image>,
    frame: u32,
    step: u8,
    extracted: bool,
    saved: bool,
}
struct SweepLedger { failed: bool, extracted: u8, shots: [Option<Shot>; 3] }
#[derive(Resource)]
pub(crate) struct Sweep {
    data: std::sync::Arc<super::crowd_rig::SweepData>,
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<CrowdMaterial>,
    transform_bits: [u32;16],
    base_fingerprint: String,
    uv_bits: Vec<[u32;2]>,
    indices: Vec<u16>,
    positions: Vec<[f32;3]>,
    normals: Vec<[f32;3]>,
    step: Option<u8>,
    frame: u32,
    directory: Option<std::path::PathBuf>,
    ledger: std::sync::Arc<std::sync::Mutex<SweepLedger>>,
}
impl Sweep {
    pub(crate) fn failed(&self) -> bool { self.ledger.lock().unwrap().failed }
    pub(crate) fn finished(&self) -> bool {
        let l=self.ledger.lock().unwrap();
        !l.failed && l.extracted==31 && l.shots.iter().all(|s|s.as_ref().is_some_and(|s|s.extracted && s.saved))
    }
}
fn transform_bits(t: &Transform) -> [u32;16] { t.to_matrix().to_cols_array().map(f32::to_bits) }
fn next_sweep_step(frame: u32, previous: Option<u8>) -> Result<Option<u8>, &'static str> {
    if frame<180 { return Ok(None); }
    let step=(frame-180).min(30) as u8;
    if previous.is_none() && step!=0 || previous.is_some_and(|p| step>p+1 || step<p) {
        return Err("finite sweep missed or reversed Extract step");
    }
    Ok(Some(step))
}
fn fail_sweep(s: &Sweep, error: &str) {
    s.ledger.lock().unwrap().failed=true;
    error!("crowd_sweep failure={error} preserved=true no_retry=true");
}
#[allow(clippy::type_complexity)]
fn advance_sweep(
    sweep: Option<ResMut<Sweep>>, frame: Res<bevy::diagnostic::FrameCount>,
    mut actors: Query<(&Mesh3d,&MeshMaterial3d<CrowdMaterial>,&Transform,&mut bevy::camera::primitives::Aabb),With<SweepActor>>,
    mut meshes: ResMut<Assets<Mesh>>, materials: Res<Assets<CrowdMaterial>>, mut exit: MessageWriter<AppExit>,
) {
    use bevy::{camera::primitives::MeshAabb, mesh::VertexAttributeValues};
    let Some(mut s)=sweep else { return; };
    let result=(|| -> AnyResult<()> {
        if s.failed() { return Err("sweep previously failed".into()); }
        let expected=frame.0.checked_add(1).ok_or("sweep frame overflow")?;
        let Some(step)=next_sweep_step(expected,s.step)? else { return Ok(()); };
        if actors.iter().count()!=1 { return Err("sweep actor count differs".into()); }
        let (handle,material,transform,mut bounds)=actors.get_mut(s.entity)?;
        if handle.0.id()!=s.mesh.id() || material.0.id()!=s.material.id() || transform_bits(transform)!=s.transform_bits
            || format!("{:?}",materials.get(&s.material).ok_or("sweep material lost")?.base)!=s.base_fingerprint {
            return Err("sweep actor ownership/transform/base material changed".into());
        }
        let mesh=meshes.get(&s.mesh).ok_or("sweep mesh lost")?;
        let Some(VertexAttributeValues::Float32x2(uv))=mesh.attribute(Mesh::ATTRIBUTE_UV_0) else { return Err("sweep UV type changed".into()); };
        let Some(Indices::U16(indices))=mesh.indices() else { return Err("sweep indices type changed".into()); };
        let Some(VertexAttributeValues::Float32x3(positions))=mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return Err("sweep positions type changed".into()); };
        let Some(VertexAttributeValues::Float32x3(normals))=mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else { return Err("sweep normals type changed".into()); };
        if uv.iter().map(|p|p.map(f32::to_bits)).collect::<Vec<_>>()!=s.uv_bits || indices.as_slice()!=s.indices.as_slice()
            || positions.iter().map(|p|p.map(f32::to_bits)).collect::<Vec<_>>()!=s.positions.iter().map(|p|p.map(f32::to_bits)).collect::<Vec<_>>()
            || normals.iter().map(|p|p.map(f32::to_bits)).collect::<Vec<_>>()!=s.normals.iter().map(|p|p.map(f32::to_bits)).collect::<Vec<_>>() {
            return Err("sweep mesh attributes changed outside owned update".into());
        }
        if s.step!=Some(step) {
            let (positions,normals)=s.data.sample(step)?;
            let mut mesh=meshes.get_mut(&s.mesh).ok_or("sweep writable mesh lost")?;
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,positions.clone());
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,normals.clone());
            *bounds=mesh.compute_aabb().ok_or("sweep updated AABB absent")?;
            if !Vec3::from(bounds.center).is_finite() || !Vec3::from(bounds.half_extents).is_finite() { return Err("sweep AABB nonfinite".into()); }
            s.positions=positions; s.normals=normals; s.step=Some(step);
        }
        s.frame=expected;
        Ok(())
    })();
    if let Err(e)=result { fail_sweep(&s,&e.to_string()); exit.write(AppExit::error()); }
}
fn request_sweep(
    mut commands: Commands, sweep: Option<ResMut<Sweep>>, capture: Res<super::Capture>,
    frame: Res<bevy::diagnostic::FrameCount>, mut exit: MessageWriter<AppExit>,
) {
    let Some(mut s)=sweep else { return; };
    let Some(step)=s.step else { return; };
    if s.failed() { return; }
    let Some(slot)=[0u8,15,30].iter().position(|&n|n==step) else { return; };
    if s.ledger.lock().unwrap().shots[slot].is_some() { return; }
    let result=(|| -> AnyResult<()> {
        if frame.0.checked_add(1)!=Some(s.frame) || s.frame!=180+u32::from(step) { return Err("sweep screenshot phase differs".into()); }
        if s.directory.is_none() {
            let stem=capture.path.file_stem().ok_or("sweep capture stem absent")?.to_string_lossy();
            let directory=capture.path.with_file_name(format!("{stem}-sweep"));
            std::fs::create_dir(&directory)?; s.directory=Some(directory);
        }
        let path=s.directory.as_ref().unwrap().join(format!("{step:02}.png"));
        if path.exists() { return Err("sweep refuses existing snapshot".into()); }
        let target=capture.target.as_ref().ok_or("sweep image target not prepared")?.clone();
        let entity=commands.spawn((bevy::render::view::screenshot::Screenshot::image(target.clone()),SweepShot(slot))).id();
        s.ledger.lock().unwrap().shots[slot]=Some(Shot { entity,target,frame:s.frame,step,extracted:false,saved:false });
        println!("crowd_sweep_request step={step} extract_frame={} screenshot_entity={:?} actor_entity={:?} mesh={:?} scheduler=authored-Extract-frame native_clock=false",s.frame,entity,s.entity,s.mesh.id());
        let ledger=s.ledger.clone();
        commands.entity(entity).observe(move |event:On<bevy::render::view::screenshot::ScreenshotCaptured>,mut exit:MessageWriter<AppExit>| {
            let result=(|| -> AnyResult<()> {
                { let l=ledger.lock().unwrap();let shot=l.shots[slot].as_ref().ok_or("sweep shot record lost")?;
                  if l.failed || event.entity!=entity || shot.entity!=entity || !shot.extracted || shot.saved { return Err("sweep callback missing real Extract verification".into()); } }
                if path.exists() { return Err("sweep snapshot appeared before callback".into()); }
                event.image.clone().try_into_dynamic().map_err(|e|format!("{e:?}"))?.to_rgb8().save(&path)?;
                ledger.lock().unwrap().shots[slot].as_mut().unwrap().saved=true;
                println!("crowd_sweep_saved step={step} screenshot_entity={entity:?} path={} extract_verified=true native_pose_parity=false",path.display());
                Ok(())
            })();
            if let Err(e)=result { ledger.lock().unwrap().failed=true;error!("crowd_sweep callback_failure={e}");exit.write(AppExit::error()); }
        });
        Ok(())
    })();
    if let Err(e)=result { fail_sweep(&s,&e.to_string());exit.write(AppExit::error()); }
}
fn extract_sweep(
    sweep: bevy::render::Extract<Option<Res<Sweep>>>, frame: bevy::render::Extract<Res<bevy::diagnostic::FrameCount>>,
    actors: bevy::render::Extract<Query<(&Mesh3d,&Transform),With<SweepActor>>>,
    shots: bevy::render::Extract<Query<(Entity,&SweepShot,&bevy::render::view::screenshot::Screenshot)>>,
) {
    let Some(s)=sweep.as_ref() else { return; };
    let Some(step)=s.step else { return; };
    let result=(|| -> AnyResult<()> {
        if s.frame!=frame.0 || s.frame<180 { return Err("sweep actual Extract frame differs from updated mesh stamp".into()); }
        let (mesh,transform)=actors.get(s.entity)?;
        if actors.iter().count()!=1 || mesh.0.id()!=s.mesh.id() || transform_bits(transform)!=s.transform_bits { return Err("sweep extracted actor ownership differs".into()); }
        let mut l=s.ledger.lock().unwrap();
        if s.frame<=210 {
            if u32::from(step)!=s.frame-180 || l.extracted!=step { return Err("sweep actual Extract sequence skipped/repeated step".into()); }
            l.extracted+=1;
            println!("crowd_sweep_extract step={step} actual_extract_frame={} mesh={:?} actor={:?} observed31_steps_not_GPU_fragments=true",s.frame,s.mesh.id(),s.entity);
        } else if step!=30 || l.extracted!=31 { return Err("sweep did not hold finalRight after31steps".into()); }
        for (entity,stamp,screenshot) in &shots {
            let shot=l.shots.get_mut(stamp.0).and_then(Option::as_mut).ok_or("sweep unknown screenshot stamp")?;
            if !shot.extracted {
                let image=match &screenshot.0 { bevy::camera::RenderTarget::Image(image)=>image, _=>return Err("sweep screenshot target not Image".into()) };
                if shot.entity!=entity || shot.frame!=frame.0 || shot.step!=step || image.handle.id()!=shot.target.id() { return Err("sweep screenshot actual Extract stamp mismatch".into()); }
                shot.extracted=true;
                println!("crowd_sweep_shot_extract step={} actual_extract_frame={} screenshot_entity={:?} target_image={:?} actor_entity={:?} mesh={:?} source_extract_verified=true GPU_fragments_proved=false",shot.step,frame.0,entity,shot.target.id(),s.entity,s.mesh.id());
            }
        }
        Ok(())
    })();
    if let Err(e)=result { fail_sweep(s,&e.to_string()); }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn flash_rows_fixture() -> (Vec<Row>, [FlashRequest; 3]) {
        let rows = (0..3)
            .map(|i| Row {
                position: Vec3::new(i as f32, 2., 3.),
                orientation_pair: [0., 1.],
                opaque_scalar: 0.,
                cell: i as u32 + 7,
                sex: usize::from(i == 2),
            })
            .collect();
        let requests = [
            FlashRequest {
                ordinal: 0,
                sex: 0,
                cell: 7,
            },
            FlashRequest {
                ordinal: 1,
                sex: 0,
                cell: 8,
            },
            FlashRequest {
                ordinal: 2,
                sex: 1,
                cell: 9,
            },
        ];
        (rows, requests)
    }
    #[test]
    fn flash_row_bridge_retains_exact_source_origin_and_order() {
        let (rows, requests) = flash_rows_fixture();
        let anchors = select_flash_rows(&rows, [true, true], &requests).unwrap();
        assert_eq!(anchors.len(), 3);
        for (i, a) in anchors.iter().enumerate() {
            assert_eq!(a.source_index, i as u32);
            assert_eq!(a.source_cell, rows[i].cell);
            assert_eq!(
                a.position.map(f32::to_bits),
                rows[i].position.to_array().map(f32::to_bits)
            );
        }
    }
    #[test]
    fn flash_row_bridge_rejects_duplicate_missing_group_cell_camera_and_nonfinite() {
        let (mut rows, requests) = flash_rows_fixture();
        let mut bad = requests;
        bad[1] = bad[0];
        assert!(select_flash_rows(&rows, [true, true], &bad).is_err());
        let mut bad = requests;
        bad[2].ordinal = 3;
        assert!(select_flash_rows(&rows, [true, true], &bad).is_err());
        let mut bad = requests;
        bad[0].sex = 1;
        assert!(select_flash_rows(&rows, [true, true], &bad).is_err());
        let mut bad = requests;
        bad[0].cell = 99;
        assert!(select_flash_rows(&rows, [true, true], &bad).is_err());
        assert!(select_flash_rows(&rows, [true, false], &requests).is_err());
        assert!(select_flash_rows(&rows, [true, true], &requests[..2]).is_err());
        rows[0].position.x = f32::NAN;
        assert!(select_flash_rows(&rows, [true, true], &requests).is_err());
    }
    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT; CPU only, no GPU"]
    fn purchased_flash_bridge_resolves_exact_three_usa_camera_rows() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let route = Route::new("tracks/usa/detroit/route_8").unwrap();
        let prepared = prepare(&root, &route, Mode::StaticBillboards).unwrap();
        let anchors = flash_fixture_anchors(&root, &prepared).unwrap();
        assert_eq!(anchors.len(), 3);
        for (request, anchor) in FLASH_REQUESTS.iter().zip(anchors) {
            let row = &prepared.rows[request.ordinal];
            assert_eq!(anchor.source_index, request.ordinal as u32);
            assert_eq!(anchor.source_cell, request.cell);
            assert_eq!(row.sex, request.sex);
            assert_eq!(
                anchor.position.map(f32::to_bits),
                row.position.to_array().map(f32::to_bits)
            );
        }
    }

    #[test]
    fn static_billboard_has_source_anchor_dimensions_and_distinct_stance_uv() {
        let row = Row {
            position: Vec3::new(3., 4., 5.),
            orientation_pair: [0., 1.],
            opaque_scalar: 7.,
            cell: 0,
            sex: 0,
        };
        let s = Spec {
            uv: [0.1, 0.2, 0.3, 0.8],
            dimensions: [0.6, 1.5],
            colours: [Vec4::ONE; 3],
            instance_scale: [1., 1.],
        };
        let mut b = Batch::default();
        card(&mut b, &row, &s, Vec3::new(3., 4., 10.)).unwrap();
        assert_eq!(b.p.len(), 4);
        assert_eq!(b.indices.len(), 6);
        assert!((b.p[1][1] - b.p[0][1] - 1.5).abs() < 1e-6);
        assert_eq!(b.uv[0], [0.3, 0.8]);
        assert_eq!(b.uv[1], [0.3, 0.2]);
        assert_eq!(b.uv[2], [0.1, 0.8]);
        assert_eq!(b.uv[3], [0.1, 0.2]);
        assert_eq!(row.position, Vec3::new(3., 4., 5.));
        assert!(card(&mut Batch::default(), &row, &s, row.position).is_err());
    }
    #[test]
    fn invalid_preview_context_rejects_and_off_is_inert() {
        assert!(
            Mode::Off
                .validate(false, false, false, false, false, None, false)
                .is_ok()
        );
        assert!(
            Mode::Control
                .validate(true, true, true, true, true, Some(120), true)
                .is_ok()
        );
        for (baseline, reference, route, lod, render, frames, perf) in [
            (false, true, true, true, true, Some(120), true),
            (true, false, true, true, true, Some(120), true),
            (true, true, false, true, true, Some(120), true),
            (true, true, true, false, true, Some(120), true),
            (true, true, true, true, false, Some(120), true),
            (true, true, true, true, true, None, true),
            (true, true, true, true, true, Some(601), true),
            (true, true, true, true, true, Some(120), false),
        ] {
            assert!(
                Mode::StaticBillboards
                    .validate(baseline, reference, route, lod, render, frames, perf)
                    .is_err()
            );
        }
    }
    fn ordinary_card_fixture() -> (Row, Spec) {
        (
            Row {
                position: Vec3::ZERO,
                orientation_pair: [0., 1.],
                opaque_scalar: 7.5,
                cell: 0,
                sex: 0,
            },
            Spec {
                uv: [0.1, 0.2, 0.3, 0.8],
                dimensions: [0.6, 1.5],
                colours: [Vec4::ONE; 3],
                instance_scale: [1., 1.],
            },
        )
    }
    #[test]
    fn card_nonfinite_extreme_and_scaled_output_reject_without_partial_append() {
        let (ordinary, spec) = ordinary_card_fixture();
        for (position, eye) in [
            (Vec3::ZERO, Vec3::new(f32::NAN, 0., 1.)),
            (Vec3::new(f32::NAN, 0., 0.), Vec3::Z),
            (Vec3::new(-f32::MAX, 0., 0.), Vec3::new(f32::MAX, 0., 1.)),
        ] {
            let mut batch = Batch::default();
            let row = Row {
                position,
                ..ordinary.clone()
            };
            assert!(card(&mut batch, &row, &spec, eye).is_err());
            assert!(
                batch.p.is_empty()
                    && batch.n.is_empty()
                    && batch.uv.is_empty()
                    && batch.indices.is_empty()
            );
        }
        let mut batch = Batch::default();
        let huge = Spec {
            dimensions: [f32::MAX, f32::MAX],
            instance_scale: [4., 4.],
            ..spec
        };
        assert_eq!(
            card(&mut batch, &ordinary, &huge, Vec3::Z),
            Err("nonfinite static crowd card vertex")
        );
        assert!(batch.p.is_empty() && batch.indices.is_empty());
        // A finite basis and finite unscaled corner can also overflow only
        // when added to the origin, independently of multiplication overflow.
        let (mut origin_row, mut wide) = ordinary_card_fixture();
        origin_row.position = Vec3::new(f32::MAX, 0., 0.);
        wide.dimensions = [f32::MAX, 1.];
        let mut batch = Batch::default();
        assert_eq!(
            card(&mut batch, &origin_row, &wide, Vec3::new(f32::MAX, 0., 1.)),
            Err("nonfinite static crowd card vertex")
        );
        assert!(
            batch.p.is_empty()
                && batch.n.is_empty()
                && batch.uv.is_empty()
                && batch.indices.is_empty()
        );
    }
    #[test]
    fn source_scale_is_applied_once_and_uv_order_preserved() {
        let (row, spec) = ordinary_card_fixture();
        let s = Spec {
            instance_scale: [2., 0.5],
            ..spec
        };
        let mut b = Batch::default();
        card(&mut b, &row, &s, Vec3::new(0., 0., 10.)).unwrap();
        assert!((b.p[0][0] - b.p[2][0] - 1.2).abs() < 1e-6);
        assert!((b.p[1][1] - b.p[0][1] - 0.75).abs() < 1e-6);
        assert_eq!(b.uv, vec![[0.3, 0.8], [0.3, 0.2], [0.1, 0.8], [0.1, 0.2]]);
        assert!(
            b.p.iter()
                .flatten()
                .chain(b.n.iter().flatten())
                .all(|v| v.is_finite())
        );
    }
    #[test]
    fn selected_count_preflight_is_bounded_without_allocating_rows() {
        assert!(preflight_selected_rows(99_999, 1).is_ok());
        assert!(preflight_selected_rows(100_000, 0).is_ok());
        assert!(preflight_selected_rows(100_000, 1).is_err());
        assert!(preflight_selected_rows(usize::MAX, 1).is_err());
    }
    #[test]
    fn source_draw_5000_fixture_is_explicit() {
        assert_eq!(source_scale_fixture(0.05, 0.05), [1., f32::from_bits(0x3f732000)]);
        assert_eq!(source_scale_fixture(0., 0.), [1., 1.]);
    }
    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT; CPU only, no GPU"]
    fn purchased_detroit_usa_crowd_control() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let route = Route::new("tracks/usa/detroit/route_8").unwrap();
        let raw = std::fs::read(root.join(route.file("crowd_standing.bin"))).unwrap();
        let catalogue = crowd_placements::read(&raw).unwrap();
        assert_eq!(catalogue.cells.len(), 106);
        assert_eq!(catalogue.total_instances, 6854);
        assert!(
            catalogue
                .groups
                .iter()
                .all(|g| matches!(&g.placements, crowd_placements::Placements::Standing(_)))
        );
        let p = prepare(&root, &route, Mode::Control).unwrap();
        assert_eq!(p.total, 6854);
        assert_eq!(p.rows.len(), 4667);
        assert_eq!(p.rows.iter().filter(|r| r.sex == 0).count(), 2439);
        assert_eq!(p.rows.iter().filter(|r| r.sex == 1).count(), 2228);
        let mut selected = 0;
        for group in &catalogue.groups {
            let sex = match group.instance_set {
                b"m_gr" => 0,
                b"f_gr" => 1,
                _ => continue,
            };
            let crowd_placements::Placements::Standing(rows) = &group.placements else {
                panic!("unexpected moving grandstand")
            };
            for r in rows {
                let out = &p.rows[selected];
                assert_eq!(
                    out.position.to_array().map(f32::to_bits),
                    r.position.map(f32::to_bits)
                );
                assert_eq!(
                    out.orientation_pair.map(f32::to_bits),
                    r.orientation_pair.map(f32::to_bits)
                );
                assert_eq!(out.opaque_scalar.to_bits(), r.opaque_scalar.to_bits());
                assert_eq!(out.cell, r.cell);
                assert_eq!(out.sex, sex);
                selected += 1;
            }
        }
        assert_eq!(selected, 4667);
        for spec in &p.specs {
            assert_eq!(spec.instance_scale, [1., f32::from_bits(0x3f732000)]);
        }
        // prepare also resolves exact source USA dataset/atlas IDs and formats.
        assert!(!p.diffuse.is_empty() && !p.tint.is_empty());
    }
    #[test]
    fn finite_sweep_refuses_skip_and_holds_after31_extract_steps() {
        assert_eq!(next_sweep_step(179,None),Ok(None));
        assert_eq!(next_sweep_step(180,None),Ok(Some(0)));
        assert!(next_sweep_step(182,Some(0)).is_err());
        assert_eq!(next_sweep_step(210,Some(29)),Ok(Some(30)));
        assert_eq!(next_sweep_step(211,Some(30)),Ok(Some(30)));
    }
}

#[cfg(test)]
mod selected_source_tests {
    use super::*;
    #[test]
    fn generic_source_modes_share_static_admission_without_legacy_route_whitelist(){
        assert!(Mode::SelectedSourcePose.validate(true,true,false,true,true,Some(200),true).is_ok());
        assert!(Mode::SelectedSourceControl.validate(true,true,false,true,true,Some(200),true).is_ok());
        assert!(Mode::OneNeutralActor.validate(true,true,false,true,true,Some(200),true).is_err());
        assert!(Mode::SelectedSourcePose.validate(true,false,true,true,true,Some(200),true).is_err());
        assert!(Mode::SelectedSourcePose.validate(true,true,true,true,true,None,true).is_err());
    }
    fn template(shift:&str)->crowd_population::Template {
        let component=|v:&str|vec![("red".to_owned(),v.to_owned()),("green".to_owned(),"0.5".to_owned()),("blue".to_owned(),"0.75".to_owned())];
        crowd_population::Template{definition_set_element:0,definition_element:0,definition_origin:crowd_population::DefinitionOrigin::Primary,requested_definition_name:"source".into(),map_name:"actual".into(),map_fields:vec![],instance_set_fields:vec![],map_reference_fields:vec![],definition_set:"source".into(),definition_name:"source".into(),definition_fields:vec![],stance_name:"source".into(),stance_fields:vec![],mesh:None,billboard:crowd_population::ResourceRef{userdata:"fixture".into(),relative_path:"fixture.pssg".into()},variants:vec![crowd_population::Variant{fields:vec![],billboard_stance:None,skin_fields:Some(vec![("textureShift".into(),shift.into())]),colour_sets:vec![crowd_population::ColourSet{fields:vec![],colours:vec![component("0.25"),component("0.5"),component("0.75")]}]}]}
    }
    #[test]
    fn actor_tints_consume_selected_source_and_preserve_low_byte_shift_admission(){
        assert_eq!(selected_tints(&template("0"),0,0).unwrap(),[Vec4::new(0.25,0.5,0.75,1.),Vec4::new(0.5,0.5,0.75,1.),Vec4::new(0.75,0.5,0.75,1.)]);
        assert!(selected_tints(&template("1"),0,0).is_err());
        assert!(selected_tints(&template("256"),0,0).is_ok());
        assert!(selected_tints(&template("0"),1,0).is_err());
        assert!(selected_tints(&template("0"),0,1).is_err());
    }
    #[test]
    fn selected_shader_requires_exact_unique_local_sampler_binding(){
        let bindings=[grid_formats::material::TextureBinding{shader:b"actual-shader",sampler:b"TDiffuseAlphaMap",texture:b"actual-texture",container:None}];
        assert_eq!(shader_texture(&bindings,b"actual-shader",b"TDiffuseAlphaMap").unwrap(),b"actual-texture");
        assert!(shader_texture(&bindings,b"other-shader",b"TDiffuseAlphaMap").is_err());
        let duplicate=[grid_formats::material::TextureBinding{shader:b"s",sampler:b"m",texture:b"one",container:None},grid_formats::material::TextureBinding{shader:b"s",sampler:b"m",texture:b"two",container:None}];
        assert!(shader_texture(&duplicate,b"s",b"m").is_err());
        let external=[grid_formats::material::TextureBinding{shader:b"s",sampler:b"m",texture:b"one",container:Some(b"external")}];
        assert!(shader_texture(&external,b"s",b"m").is_err());
    }
}

#[cfg(test)]
mod selected_cli_tests {
    use super::*;
    fn request(mode:Mode,coordinate:&str)->AnyResult<Option<SelectedActorRequest>> {
        selected_request_from_cli(mode,Some("region,-,maps,billboard"),Some("actual-group,actual-map,7,source-skin,source-modifier,2,3"),Some(coordinate),Some("default"),Some("123"))
    }
    #[test]
    fn explicit_cli_fields_preserve_selection_without_scene_defaults() {
        let r=request(Mode::SelectedSourcePose,"2.5").unwrap().unwrap();
        assert_eq!(r.context.primary_definition_set.as_deref(),Some("region"));assert!(r.context.fallback_definition_set.is_none());
        assert_eq!(r.context.map_group,"maps");assert_eq!(r.context.billboard_userdata,"billboard");
        assert_eq!(r.group,b"actual-group");assert_eq!(r.map_name,"actual-map");assert_eq!(r.placement_ordinal,7);
        assert_eq!(r.skin_id,b"source-skin");assert_eq!(r.modifier_instance_id,b"source-modifier");assert_eq!(r.variant_ordinal,2);assert_eq!(r.colour_set_ordinal,3);
        assert_eq!(r.bank_coordinate,2.5);assert_eq!(r.experimental_performance,None);assert_eq!(r.experimental_draw,123);
        assert!(selected_request_from_cli(Mode::Off,None,None,None,None,None).unwrap().is_none());
        assert!(request(Mode::Off,"2.5").is_err());assert!(request(Mode::OneNeutralActor,"2.5").is_err());
    }
    #[test]
    fn selected_cli_refuses_partial_bad_ordinals_nonfinite_or_source_categories() {
        for v in ["NaN","inf","-inf"]{assert!(request(Mode::SelectedSourcePose,v).is_err());}
        assert!(selected_request_from_cli(Mode::SelectedSourceControl,None,None,None,None,None).is_err());
        for target in ["too,few","g,m,-1,s,k,0,0","g,m,0,s,k,0,0,extra","g,,0,s,k,0,0"] {
            assert!(selected_request_from_cli(Mode::SelectedSourcePose,Some("r,-,g,b"),Some(target),Some("0"),Some("0"),Some("0")).is_err());
        }
        assert!(selected_request_from_cli(Mode::SelectedSourcePose,Some("r,-,g,b"),Some("g,m,0,s,k,0,0"),Some("0"),Some("16"),Some("0")).is_err());
        assert!(selected_request_from_cli(Mode::SelectedSourcePose,Some("r,-,g,b"),Some("g,m,0,s,k,0,0"),Some("0"),Some("0"),Some("-1")).is_err());
    }
    #[test]
    fn native_dlaa_baseline_changes_msaa_and_crowd_admission_refuses_it() {
        let mut world=World::new();
        let camera=world.spawn((Camera3d::default(),Msaa::Sample4)).id();
        let mut media=Assets::<bevy::light::atmosphere::ScatteringMedium>::default();
        let settings=super::super::graphics::Settings{anti_aliasing:3,..default()};
        let mut queue=bevy::ecs::world::CommandQueue::default();
        super::super::graphics::configure(&mut Commands::new(&mut queue,&world),camera,&settings,&mut media);
        queue.apply(&mut world);
        assert_eq!(*world.get::<Msaa>(camera).unwrap(),Msaa::Off);
        let capability=!settings.enhanced() && settings.anti_aliasing!=3;
        assert!(Mode::SelectedSourceControl.validate(capability,true,false,true,true,Some(120),true).is_err());
        assert!(Mode::SelectedSourcePose.validate(capability,true,false,true,true,Some(120),true).is_err());
        assert!(Mode::Off.validate(capability,true,false,true,true,Some(120),true).is_ok());
    }
    #[test]
    fn selected_texture_object_requires_exact_cardinality() {
        let make=|id|texture::Texture{id,width:4,height:4,format:texture::Format::Bc1,declared_mip_levels:0,base_blocks:&[0;8],remaining_data:&[]};
        let one=[make(b"one".as_slice())];assert_eq!(unique_texture(&one,b"one").unwrap().id,b"one");assert!(unique_texture(&one,b"missing").is_err());
        let duplicate=[make(b"one".as_slice()),make(b"one".as_slice())];assert!(unique_texture(&duplicate,b"one").is_err());
    }
    #[test]
    #[ignore="requires absolute GRID_GAME_ROOT purchased assets; CPU source admission only"]
    fn purchased_six_bank_dynamic_skin_and_material_admission() {
        let root=std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("absolute GRID_GAME_ROOT required"));assert!(root.is_absolute());
        // Three representative datasets only; actual resource names come from source.
        // These cases are CPU fixtures, NOT production route/model/sex admission.
        let mut banks=std::collections::BTreeSet::new();
        for track in ["tracks/europe/donington","tracks/usa/detroit","tracks/japan/shibuya"] {
            let graph=grid_tools::tyre_smoke::read_xml(&root.join(track).join("organism_track_dataset.xml")).unwrap();
            let dataset=crowd_population::dataset(&graph).unwrap();
            for userdata in ["maleVariant0","femaleVariant0"] {
                let entries=dataset.pssg.iter().filter(|r|r.userdata==userdata).collect::<Vec<_>>();
                let [entry]=entries.as_slice() else{panic!("source fixture userdata absent/ambiguous")};
                banks.insert(entry.filename.strip_prefix("/data/").expect("source fixture data prefix").to_owned());
            }
        }
        assert_eq!(banks.len(),6,"three fixture datasets should reference six distinct banks");
        let mut total=0;
        for file in &banks {
            let bytes=std::fs::read(root.join(file)).unwrap();let tree=pssg::read_tree(&bytes).unwrap();
            let catalogue=grid_formats::mesh::read_object_meshes(&tree).unwrap();let bindings=grid_formats::material::read_texture_bindings(&tree).unwrap();let mut instances=0;
            for i in 0..tree.nodes.len(){if tree.name(i)!=b"SKINNODE"{continue;}
                let skin=tree.string(i,b"id").unwrap();let meta=grid_formats::scene::read_skin(&tree,&catalogue,skin).unwrap();
                for (modifier,_,_) in &meta.instances {
                    let source=super::super::crowd_pose::Source::read(&tree,skin,modifier).unwrap();
                    let neutral=source.sample(&source.saved_locals(),&[],super::super::crowd_pose::PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta{mirrored:false}).unwrap();assert!(!neutral.positions.is_empty());
                    let diffuse=shader_texture(&bindings,&source.material_shader,b"TDiffuseAlphaMap").unwrap();let tint=shader_texture(&bindings,&source.material_shader,b"TTintMap").unwrap();
                    let textures=texture::read_named(&tree,&[diffuse,tint]).unwrap();assert!(!unique_texture(&textures,diffuse).unwrap().validated_dds().0.is_empty());assert!(!unique_texture(&textures,tint).unwrap().validated_dds().0.is_empty());
                    println!("crowd_generic_cpu bank={file} skin={skin:?} modifier={modifier:?} vertices={} indices={} shader={:?} diffuse={diffuse:?} tint={tint:?}",neutral.positions.len(),source.indices.len(),source.material_shader);instances+=1;
                }
            }
            assert!(instances>0,"actual bank has no admitted source modifier");total+=instances;
        }
        assert!(total>=banks.len());
    }
    #[test]
    #[ignore="requires absolute GRID_GAME_ROOT purchased assets; CPU full selected consumer only"]
    fn purchased_selected_cli_reaches_same_prepared_consumer() {
        let root=std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("absolute GRID_GAME_ROOT required"));assert!(root.is_absolute());
        let route=Route::new("tracks/usa/detroit/route_8").unwrap();
        let req=selected_request_from_cli(Mode::SelectedSourcePose,Some("usa,generic,generic,billboardusa"),Some("f_gr,femaleSit,0,f_US_lod0,!J_f_US_lod0,0,0"),Some("2"),Some("4"),Some("0")).unwrap().unwrap();
        let off=prepare_with_request(&root,&route,Mode::SelectedSourceControl,Some(&req)).unwrap();let pose=prepare_with_request(&root,&route,Mode::SelectedSourcePose,Some(&req)).unwrap();
        let a=off.actor.as_ref().unwrap();let b=pose.actor.as_ref().unwrap();assert_eq!(a.indices,b.indices);assert_eq!(a.uv,b.uv);assert_eq!(a.diffuse,b.diffuse);assert_eq!(a.tint,b.tint);assert_eq!(a.positions.len(),b.positions.len());
        assert!(a.positions.iter().zip(&b.positions).any(|(a,b)|a!=b),"explicit coordinate produced no pose difference");
        println!("crowd_selected_fixture point={:?} control_vertices={} pose_vertices={} selected_row={:?} native_clock=false",pose.rows[0].position,a.positions.len(),b.positions.len(),pose.selected_actor_row);
    }
}
