"""Prepare a private pinned dlss_wgpu hint/frame-time/exposure probe; never edit Cargo's cache.

Build with the generated per-command --config, --locked and --offline. This is a
research copy, not a renderer default or proof of the active neural preset.
"""
import argparse
import difflib
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM_SHA256 = '339dead011e19775da9bd73d3228ca20a4cb3a55308ec2cedc254d802c0ef558'
INJECTION = '''        // GRID private diagnostic: supported native-DLAA hint only. Not active-model proof.
        if let Ok(name) = std::env::var("GRID_DLSS_PRESET_HINT") {
            assert_eq!(perf_quality_mode, DlssPerfQualityMode::Dlaa, "preset diagnostic requires native DLAA");
            let requested = match name.as_str() {
                "K" => NVSDK_NGX_DLSS_Hint_Render_Preset_NVSDK_NGX_DLSS_Hint_Render_Preset_K,
                "L" => NVSDK_NGX_DLSS_Hint_Render_Preset_NVSDK_NGX_DLSS_Hint_Render_Preset_L,
                _ => panic!("GRID_DLSS_PRESET_HINT must be K or L"),
            } as u32;
            unsafe {
                NVSDK_NGX_Parameter_SetUI(locked_sdk.parameters,
                    NVSDK_NGX_Parameter_DLSS_Hint_Render_Preset_DLAA.as_ptr().cast(), requested);
                let mut stored = u32::MAX;
                check_ngx_result(NVSDK_NGX_Parameter_GetUI(locked_sdk.parameters,
                    NVSDK_NGX_Parameter_DLSS_Hint_Render_Preset_DLAA.as_ptr().cast(), &mut stored))?;
                assert_eq!(stored, requested, "preset hint parameter did not round-trip");
                eprintln!("grid_dlss_preset_hint requested={name} stored={stored} active_model_verified=false scope=private-diagnostic");
            }
        }
'''


FRAME_TIME_EXPRESSION = """*{
                static VALUE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
                VALUE.get_or_init(|| {
                    match std::env::var("GRID_DLSS_FRAME_TIME_MS") {
                        Ok(raw) => {
                            let value: f32 = raw.parse().expect("GRID_DLSS_FRAME_TIME_MS must be a number");
                            assert!(value.is_finite() && (0.0..=250.0).contains(&value),
                                "GRID_DLSS_FRAME_TIME_MS must be finite in 0..=250");
                            eprintln!("grid_dlss_frame_time_ms requested={value} scope=private-diagnostic effect_verified=false");
                            value
                        }
                        Err(std::env::VarError::NotPresent) => 0.0,
                        Err(error) => panic!("invalid GRID_DLSS_FRAME_TIME_MS: {error}"),
                    }
                })
            }"""


UNITY_EXPOSURE_CREATE = """        // Private control: input HDR is already eye-adapted; compare unity with auto.
        let diagnostic_exposure = match std::env::var("GRID_DLSS_EXPOSURE") {
            Ok(name) if name == "unity" => {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("grid_private_unity_exposure"),
                    size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                    mip_level_count: 1, sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0,
                        origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    &1.0f32.to_le_bytes(),
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: None, rows_per_image: None },
                    wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 });
                eprintln!("grid_dlss_exposure requested=unity automatic_flag=false scope=private-diagnostic effect_verified=false");
                Some(texture.create_view(&wgpu::TextureViewDescriptor::default()))
            }
            Err(std::env::VarError::NotPresent) => None,
            _ => panic!("GRID_DLSS_EXPOSURE must be unity or absent"),
        };
"""


def replace_once(text, anchor, replacement):
    if text.count(anchor) != 1:
        raise ValueError("Expected exactly one private exposure anchor: " + anchor[:60])
    return text.replace(anchor, replacement)


def patch_source(text, frame_time=False, unity_exposure=False):
    anchor = "        let locked_sdk = sdk.lock().unwrap();\n"
    if text.count(anchor) != 1:
        raise ValueError("Expected exactly one native context-creation anchor")
    patched = text.replace(anchor, anchor + INJECTION)
    if frame_time:
        field = "InFrameTimeDeltaInMsec: 0.0,"
        if patched.count(field) != 1:
            raise ValueError("Expected exactly one frame-time field")
        patched = patched.replace(field, "InFrameTimeDeltaInMsec: " + FRAME_TIME_EXPRESSION + ",")
    if unity_exposure:
        patched = replace_once(patched, "    feature: *mut NVSDK_NGX_Handle,", "    feature: *mut NVSDK_NGX_Handle,\n    diagnostic_exposure: Option<TextureView>,")
        patched = replace_once(patched, "        let perf_quality_value =", UNITY_EXPOSURE_CREATE + "\n        let perf_quality_value =")
        patched = replace_once(patched, "InFeatureCreateFlags: feature_flags.as_flags(),", "InFeatureCreateFlags: if diagnostic_exposure.is_some() { feature_flags.as_flags() & !DlssFeatureFlags::AutoExposure.as_flags() } else { feature_flags.as_flags() },")
        patched = replace_once(patched, "            feature,\n        })", "            feature,\n            diagnostic_exposure,\n        })")
        patched = replace_once(patched, "            pInExposureTexture: exposure,", "            pInExposureTexture: match &self.diagnostic_exposure { Some(view) => &mut texture_to_ngx(view, adapter) as *mut _, None => exposure },")
        patched = replace_once(patched, "command_encoder.transition_resources(iter::empty(), render_parameters.barrier_list());", "command_encoder.transition_resources(iter::empty(), render_parameters.barrier_list().chain(self.diagnostic_exposure.iter().map(|view| TextureTransition { texture: view.texture(), selector: None, state: TextureUses::RESOURCE })));")
    return patched


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path, help="installed dlss_wgpu-4.0.0 crate directory")
    parser.add_argument("output", type=Path, help="new directory under this checkout's ignored local/")
    parser.add_argument("--frame-time", action="store_true", help="optional process-start GRID_DLSS_FRAME_TIME_MS diagnostic; absent keeps upstream zero")
    parser.add_argument("--unity-exposure", action="store_true", help="optional GRID_DLSS_EXPOSURE=unity 1x1 exposure control; absent retains upstream automatic path")
    args = parser.parse_args()
    source = args.upstream.resolve(strict=True)
    output = args.output.resolve()
    private = (ROOT / "local").resolve(strict=True)
    if not output.is_relative_to(private) or output.exists():
        parser.error("Output must be a new directory under ignored local/")
    output.parent.resolve(strict=True)
    original = source / "src/super_resolution.rs"
    if sha(original) != UPSTREAM_SHA256:
        parser.error("Pinned upstream super_resolution.rs SHA256 mismatch")
    for license_name in ("LICENSE-MIT", "LICENSE-APACHE"):
        if not (source / license_name).is_file():
            parser.error("Upstream licenses must be preserved")
    text = original.read_text(encoding="utf-8")
    try:
        patched = patch_source(text, args.frame_time, args.unity_exposure)
    except ValueError as error:
        parser.error(str(error))
    output.mkdir()
    target = output / "dlss_wgpu"
    shutil.copytree(source, target)
    (target / "src/super_resolution.rs").write_text(patched, encoding="utf-8")
    (output / "wrapper.patch").write_text("".join(difflib.unified_diff(
        text.splitlines(True), patched.splitlines(True),
        fromfile="dlss_wgpu-4.0.0/src/super_resolution.rs",
        tofile="private-copy/src/super_resolution.rs")), encoding="utf-8")
    (output / "cargo-config.toml").write_text(
        "paths = [" + json.dumps(target.as_posix(), ensure_ascii=False) + "]\n", encoding="utf-8")
    provenance = dict(upstream=str(source), upstream_sha256=sha(original),
                      patched_sha256=sha(target / "src/super_resolution.rs"),
                      patch_sha256=sha(output / "wrapper.patch"),
                      tool_sha256=sha(Path(__file__)),
                      frame_time_probe=args.frame_time,
                      unity_exposure_probe=args.unity_exposure,
                      scope="Private hint/frame-time inputs; active model/effect not verified; absent environment preserves upstream values")
    (output / "provenance.json").write_text(json.dumps(provenance, indent=2), encoding="utf-8")
    print(json.dumps(provenance, indent=2))


if __name__ == "__main__":
    main()
