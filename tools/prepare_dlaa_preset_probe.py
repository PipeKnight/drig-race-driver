"""Prepare a private pinned dlss_wgpu K/L hint probe; never edit Cargo's cache.

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


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path, help="installed dlss_wgpu-4.0.0 crate directory")
    parser.add_argument("output", type=Path, help="new directory under this checkout's ignored local/")
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
    anchor = "        let locked_sdk = sdk.lock().unwrap();\n"
    text = original.read_text(encoding="utf-8")
    if text.count(anchor) != 1:
        parser.error("Expected exactly one native context-creation anchor")
    patched = text.replace(anchor, anchor + INJECTION)
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
                      scope="K/L native-DLAA hint roundtrip only; active model not verified; no default change")
    (output / "provenance.json").write_text(json.dumps(provenance, indent=2), encoding="utf-8")
    print(json.dumps(provenance, indent=2))


if __name__ == "__main__":
    main()
