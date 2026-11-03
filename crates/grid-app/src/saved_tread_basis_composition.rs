// CPU import/composition regression with actual embedded Bevy sources. No GPU or full Naga-validation claim.
#[cfg(test)]
mod portable_composition_tests {
    use bevy::{
        app::{Plugin, TaskPoolPlugin},
        asset::{AssetPlugin, AssetServer, Assets, io::ErasedAssetReader},
        core_pipeline::CorePipelinePlugin,
        image::ImagePlugin,
        mesh::MeshPlugin,
        pbr::{MeshRenderPlugin, PbrPlugin},
        prelude::*,
        render::{
            RenderApp, RenderPlugin,
            render_resource::{DownlevelFlags, WgpuFeatures},
            renderer::RenderDevice,
            settings::{RenderCreation, WgpuSettings},
        },
        shader::{
            Shader, ShaderCache, ShaderCacheError, ShaderCacheSource, ShaderDefVal, ShaderImport,
            ValidateShader,
        },
    };
    use std::{collections::HashSet, path::Path};
    // Named providers from the pinned 0.19.1 libraries. Resolved only when actual Shader.imports requests one.
    // The actual source is embedded by its owning public plugin, not copied from installed files.
    #[rustfmt::skip]
    const PROVIDERS: &[(&str,&str)] = &[
        ("bevy_pbr::atmosphere::bindings", "bevy_pbr/atmosphere/bindings.wgsl"),
        ("bevy_pbr::atmosphere::bruneton_functions", "bevy_pbr/atmosphere/bruneton_functions.wgsl"),
        ("bevy_pbr::atmosphere::functions", "bevy_pbr/atmosphere/functions.wgsl"),
        ("bevy_pbr::atmosphere::types", "bevy_pbr/atmosphere/types.wgsl"),
        ("bevy_pbr::cluster", "bevy_pbr/cluster/cluster.wgsl"),
        ("bevy_pbr::decal::clustered", "bevy_pbr/decal/clustered.wgsl"),
        ("bevy_pbr::decal::forward", "bevy_pbr/decal/forward_decal.wgsl"),
        ("bevy_pbr::pbr_deferred_functions", "bevy_pbr/deferred/pbr_deferred_functions.wgsl"),
        ("bevy_pbr::pbr_deferred_types", "bevy_pbr/deferred/pbr_deferred_types.wgsl"),
        ("bevy_pbr::environment_map", "bevy_pbr/light_probe/environment_map.wgsl"),
        ("bevy_pbr::irradiance_volume", "bevy_pbr/light_probe/irradiance_volume.wgsl"),
        ("bevy_pbr::light_probe", "bevy_pbr/light_probe/light_probe.wgsl"),
        ("bevy_pbr::lightmap", "bevy_pbr/lightmap/lightmap.wgsl"),
        ("bevy_pbr::meshlet_visibility_buffer_resolve", "bevy_pbr/meshlet/dummy_visibility_buffer_resolve.wgsl"),
        ("bevy_pbr::meshlet_bindings", "bevy_pbr/meshlet/meshlet_bindings.wgsl"),
        ("bevy_pbr::meshlet_cull_shared", "bevy_pbr/meshlet/meshlet_cull_shared.wgsl"),
        ("bevy_pbr::prepass_bindings", "bevy_pbr/prepass/prepass_bindings.wgsl"),
        ("bevy_pbr::prepass_io", "bevy_pbr/prepass/prepass_io.wgsl"),
        ("bevy_pbr::prepass_utils", "bevy_pbr/prepass/prepass_utils.wgsl"),
        ("bevy_pbr::clustered_forward", "bevy_pbr/render/clustered_forward.wgsl"),
        ("bevy_pbr::fog", "bevy_pbr/render/fog.wgsl"),
        ("bevy_pbr::forward_io", "bevy_pbr/render/forward_io.wgsl"),
        ("bevy_pbr::mesh_bindings", "bevy_pbr/render/mesh_bindings.wgsl"),
        ("bevy_pbr::mesh_functions", "bevy_pbr/render/mesh_functions.wgsl"),
        ("bevy_pbr::mesh_types", "bevy_pbr/render/mesh_types.wgsl"),
        ("bevy_pbr::mesh_view_bindings", "bevy_pbr/render/mesh_view_bindings.wgsl"),
        ("bevy_pbr::mesh_view_types", "bevy_pbr/render/mesh_view_types.wgsl"),
        ("bevy_pbr::morph", "bevy_pbr/render/morph.wgsl"),
        ("bevy_pbr::occlusion_culling", "bevy_pbr/render/occlusion_culling.wgsl"),
        ("bevy_pbr::parallax_mapping", "bevy_pbr/render/parallax_mapping.wgsl"),
        ("bevy_pbr::ambient", "bevy_pbr/render/pbr_ambient.wgsl"),
        ("bevy_pbr::pbr_bindings", "bevy_pbr/render/pbr_bindings.wgsl"),
        ("bevy_pbr::pbr_fragment", "bevy_pbr/render/pbr_fragment.wgsl"),
        ("bevy_pbr::pbr_functions", "bevy_pbr/render/pbr_functions.wgsl"),
        ("bevy_pbr::lighting", "bevy_pbr/render/pbr_lighting.wgsl"),
        ("bevy_pbr::pbr_prepass_functions", "bevy_pbr/render/pbr_prepass_functions.wgsl"),
        ("bevy_pbr::pbr_types", "bevy_pbr/render/pbr_types.wgsl"),
        ("bevy_pbr::rgb9e5", "bevy_pbr/render/rgb9e5.wgsl"),
        ("bevy_pbr::shadow_sampling", "bevy_pbr/render/shadow_sampling.wgsl"),
        ("bevy_pbr::shadows", "bevy_pbr/render/shadows.wgsl"),
        ("bevy_pbr::skinning", "bevy_pbr/render/skinning.wgsl"),
        ("bevy_pbr::utils", "bevy_pbr/render/utils.wgsl"),
        ("bevy_pbr::view_transformations", "bevy_pbr/render/view_transformations.wgsl"),
        ("bevy_pbr::ssao_utils", "bevy_pbr/ssao/ssao_utils.wgsl"),
        ("bevy_pbr::raymarch", "bevy_pbr/ssr/raymarch.wgsl"),
        ("bevy_pbr::ssr", "bevy_pbr/ssr/ssr.wgsl"),
        ("bevy_pbr::transmission", "bevy_pbr/transmission/transmission.wgsl"),
        ("bevy_render::bindless", "bevy_render/bindless.wgsl"),
        ("bevy_render::color_operations", "bevy_render/color_operations.wgsl"),
        ("bevy_render::globals", "bevy_render/globals.wgsl"),
        ("bevy_render::maths", "bevy_render/maths.wgsl"),
        ("bevy_pbr::mesh_preprocess_types", "bevy_render/occlusion_culling/mesh_preprocess_types.wgsl"),
        ("bevy_render::view", "bevy_render/view/view.wgsl"),
        ("bevy_core_pipeline::fullscreen_vertex_shader", "bevy_core_pipeline/fullscreen_vertex_shader/fullscreen.wgsl"),
        ("bevy_core_pipeline::oit", "bevy_core_pipeline/oit/oit_draw.wgsl"),
        ("bevy_core_pipeline::tonemapping_lut_bindings", "bevy_core_pipeline/tonemapping/lut_bindings.wgsl"),
        ("bevy_core_pipeline::tonemapping", "bevy_core_pipeline/tonemapping/tonemapping_shared.wgsl"),
    ];
    fn cpu_only(app: &App) {
        assert!(
            app.get_sub_app(RenderApp).is_none(),
            "CPU fixture must never own RenderApp"
        );
        assert!(
            !app.world().contains_resource::<RenderDevice>(),
            "CPU fixture must never own RenderDevice"
        );
    }
    fn providers_app() -> App {
        let mut app = App::new();
        cpu_only(&app);
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ImagePlugin::default(),
            MeshPlugin,
        ));
        // None returns before renderer initialization. Some(empty) does NOT provide this guarantee.
        app.add_plugins(RenderPlugin {
            render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                backends: None,
                ..default()
            })),
            ..default()
        });
        cpu_only(&app);
        app.add_plugins((
            CorePipelinePlugin,
            PbrPlugin {
                use_gpu_instance_buffer_builder: false,
                // This CPU shader fixture has no glTF asset conversion plugin.
                // The public option skips only its unrelated extension registration.
                gltf_enable_standard_materials: false,
                ..default()
            },
        ));
        cpu_only(&app);
        // Narrow authorized finish only: its GPU branch is guarded by RenderApp; this embeds mesh_bindings.
        MeshRenderPlugin {
            use_gpu_instance_buffer_builder: false,
            debug_flags: default(),
        }
        .finish(&mut app);
        cpu_only(&app);
        app
    }
    fn sha(bytes: &[u8]) -> String {
        grid_tools::private_parameters::sha256(bytes)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect()
    }
    fn accept(
        _: &(),
        source: ShaderCacheSource<'_>,
        _: &ValidateShader,
    ) -> Result<(), ShaderCacheError> {
        match source {
            ShaderCacheSource::Naga(module) if !module.entry_points.is_empty() => Ok(()),
            _ => Err(ShaderCacheError::CreateShaderModule(
                "expected composed Naga entrypoint; no GPU compilation performed".into(),
            )),
        }
    }
    #[allow(clippy::too_many_arguments)] // Bounded recursive cache/asset bookkeeping, not render systems.
    fn import_stock(
        shader: &Shader,
        reader: &dyn ErasedAssetReader,
        cache: &mut ShaderCache<(), ()>,
        assets: &mut Assets<Shader>,
        keep: &mut Vec<Handle<Shader>>,
        seen: &mut HashSet<ShaderImport>,
        bytes: &mut usize,
        depth: usize,
    ) {
        assert!(depth < 64);
        for import in &shader.imports {
            if !seen.insert(import.clone()) {
                continue;
            }
            assert!(seen.len() <= 128);
            let name = import.module_name();
            let path = PROVIDERS
                .iter()
                .find_map(|(n, p)| (*n == name.as_str()).then_some(*p))
                .unwrap_or_else(|| panic!("unknown actual import provider {name}"));
            // Read only the embedded MemoryAssetReader: immediate memory future, no filesystem,
            // no asynchronous Shader asset readiness assumption and no registry removal race.
            let source = bevy::tasks::block_on(async {
                let mut input = reader.read(Path::new(path)).await.unwrap_or_else(|error| {
                    panic!("owning plugin did not embed required {name} at {path}: {error}")
                });
                let mut source = Vec::new();
                input
                    .read_to_end(&mut source)
                    .await
                    .expect("embedded memory read");
                source
            });
            *bytes = bytes.checked_add(source.len()).unwrap();
            assert!(source.len() <= 1024 * 1024 && *bytes <= 4 * 1024 * 1024);
            eprintln!(
                "cpu_composition provider={name} embedded_path={path} sha={}",
                sha(&source)
            );
            let mut provider = Shader::from_wgsl(String::from_utf8(source).unwrap(), path);
            assert_eq!(&provider.import_path, import, "provider identity mismatch");
            if name.as_str() == "bevy_pbr::mesh_view_types" {
                // Exact MeshRenderPlugin build settings, public constants; not inferred from Device.
                provider.shader_defs = vec![
                    ShaderDefVal::UInt(
                        "MAX_DIRECTIONAL_LIGHTS".into(),
                        bevy::pbr::MAX_DIRECTIONAL_LIGHTS as u32,
                    ),
                    ShaderDefVal::UInt(
                        "MAX_CASCADES_PER_LIGHT".into(),
                        bevy::pbr::MAX_CASCADES_PER_LIGHT as u32,
                    ),
                    ShaderDefVal::UInt("MAX_RECT_LIGHTS".into(), bevy::pbr::MAX_RECT_LIGHTS as u32),
                ];
            }
            import_stock(
                &provider,
                reader,
                cache,
                assets,
                keep,
                seen,
                bytes,
                depth + 1,
            );
            let handle = assets.add(provider.clone());
            cache.set_shader(handle.id(), provider);
            keep.push(handle);
        }
    }
    #[test]
    fn actual_embedded_saved_basis_composes_dither_off_and_on() {
        let app = providers_app();
        cpu_only(&app);
        let server = app.world().resource::<AssetServer>();
        let source = server
            .get_source("embedded")
            .expect("public registry-backed memory source");
        let reader = source.reader();
        let sources = [
            ("vertex", include_str!("saved_tread_basis_vertex.wgsl")),
            ("fragment", include_str!("saved_tread_basis_fragment.wgsl")),
        ];
        for dither in [false, true] {
            let mut cache = ShaderCache::<(), ()>::new(
                (),
                WgpuFeatures::empty(),
                DownlevelFlags::all(),
                accept,
            );
            // Independently authored malformed syntax must fail in the real composer.
            let bad = Shader::from_wgsl(
                "@fragment fn deliberate_parse_failure( ",
                "authored_parse_negative.wgsl",
            );
            let mut assets = Assets::<Shader>::default();
            let negative = assets.add(bad.clone());
            cache.set_shader(negative.id(), bad);
            assert!(
                cache.get(0, negative.id(), &[]).is_err(),
                "real composer did not reject malformed syntax"
            );
            eprintln!("cpu_composition authored_parse_negative=PASS");
            let mut keep = vec![negative];
            let mut seen = HashSet::new();
            let mut bytes = 0;
            let mut defs = vec![
                ShaderDefVal::Bool("VERTEX_POSITIONS".into(), true),
                ShaderDefVal::Bool("VERTEX_NORMALS".into(), true),
                ShaderDefVal::Bool("VERTEX_UVS_A".into(), true),
                ShaderDefVal::Bool("VERTEX_OUTPUT_INSTANCE_INDEX".into(), true),
                ShaderDefVal::UInt("MATERIAL_BIND_GROUP".into(), 3),
                ShaderDefVal::UInt("AVAILABLE_STORAGE_BUFFER_BINDINGS".into(), 8),
            ];
            if dither {
                defs.push(ShaderDefVal::Bool("VISIBILITY_RANGE_DITHER".into(), true));
            }
            eprintln!(
                "cpu_composition declared_defs={defs:?} full_naga_validation_claim=false pipeline_validation_claim=false"
            );
            for (stage, source) in sources {
                let shader = Shader::from_wgsl(source, format!("saved_basis_cpu_{stage}.wgsl"));
                import_stock(
                    &shader,
                    reader,
                    &mut cache,
                    &mut assets,
                    &mut keep,
                    &mut seen,
                    &mut bytes,
                    0,
                );
                let handle = assets.add(shader.clone());
                cache.set_shader(handle.id(), shader);
                keep.push(handle.clone());
                cache.get(0, handle.id(), &defs).unwrap_or_else(|error| {
                    panic!("real composition failed stage={stage} dither={dither}: {error}")
                });
                eprintln!(
                    "cpu_composition stage={stage} dither={dither} source_sha={} result=COMPOSED_NOT_GPU_VALIDATED",
                    sha(source.as_bytes())
                );
            }
        }
        cpu_only(&app);
    }
}
