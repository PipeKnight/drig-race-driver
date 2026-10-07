# Research toolchain

Updated: 6 October 2026 (Europe/Kiev).

## Initial inventory

See [Phase 0 report](PHASE0.md#tools-checked-before-additions) for search scope and limitations. Absence means not found in inspected locations, not an exhaustive scan of every drive.

| Component | Version / location | Validation |
|---|---|---|
| Python | Codex bundled 3.12.14 | PE inventory, dump parsing and scripts run successfully |
| Git | Codex bundled 2.53.0.windows.3 | `git --version` |
| Node | Codex bundled 24.19.0 | `node --version`; npm absent from PATH |
| Existing JDK | OpenLogic 21.0.9, 64-bit; `C:\Program Files (x86)\Steam\steamapps\common\Songs of Syx Demo\jre` | Both java and javac report 21.0.9; Ghidra compatibility pending |
| ProcDump | 12.01; `local/tools/procdump` | Mini/full dumps captured actual GRID startup exception |
| Capstone | 5.0.6; `local/tools/python` | Runtime x86 instruction and caller disassembly |

ProcDump official source, Microsoft signature and ZIP hash are recorded in `local/tools/procdump-source.json`. Capstone was installed only with `--target local/tools/python`, not into the bundled interpreter's environment.

## Phase 1 additions and validation

| Tool | Pinned release and location | Proven operation |
|---|---|---|
| Ghidra | 12.1.4, `C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC` | Imports the captured runtime image as x86 and decompiles four known functions |
| x64dbg / x32dbg | Release 2026.05.27, `local/tools/x64dbg/release`; x86 `headless.exe` used | Attaches to GRID and reads the first/last-chance access violation; early call-site breakpoint remains unproven |
| Process Monitor | Microsoft 4.11, `local/tools/procmon` | Captures startup and exports actual GRID resource/configuration operations |

Downloads came from the official projects. Ghidra ZIP SHA-256 `ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db` and x64dbg ZIP SHA-256 `d41966dfc5b435a372798245300ca0ab7bb8e48bdbf48512c6fb20fcca427697` match their GitHub release asset digests. Procmon ZIP SHA-256 `80a6442b46af762ed1432f6fec3f7e20366bed62a2522b3486503398a40a1128`; all extracted executables have Valid Microsoft Corporation Authenticode signatures. Exact source URLs and verification results are in `local/tools/*-source.json`.

### Ghidra

The pinned 12.1.4 release requires Java 21 and accepts Python 3.9–3.14. It successfully uses the existing game-bundled JDK 21.0.9. The development branch's Java 25 requirement does not apply to this release. Requirements were read from the [pinned release documentation](https://github.com/NationalSecurityAgency/ghidra/blob/Ghidra_12.1.4_build/GhidraDocs/GettingStarted.md), saved at `local/phase1/ghidra-release-requirements.md`.

First installation under the Cyrillic workspace path caused Log4j JAR-URL XML parsing errors. Decompilation still completed, but normal application logging did not. The installation was moved, without replacement or deletion of original game files, to the ASCII user Tools path above. Reopening the same project then produced normal logs and four `GRID_PROBE_OK` results. No duplicate JDK was installed.

`tools/export_runtime_image.py` exports a private **memory-layout snapshot**, not a playable/patched executable. Image size 15,925,248 bytes; captured bytes 15,917,056. An uncaptured 8,192-byte range at RVA `0xed8000` is zero-filled and listed in `runtime-image.json`. It is outside the selected functions and is not evidence of observed bytes. Snapshot SHA-256: `2269a72b142fb731d9251d4db26812d67e3106beb58d1b5cad68bbf33073d6cd`.

Ghidra raw importer configuration: base `0x400000`, processor `x86:LE:32:default`, compiler spec `windows`. Broad automatic analysis is disabled for this first probe; known entry points are disassembled and decompiled explicitly by `tools/ghidra/GridProbe.java`.

| Entry VA | Validation |
|---|---|
| `0x004b3960` | Pseudocode contains a bounded 32-byte format helper, explicit NUL at byte 31; agrees with the observed call and instructions |
| `0x0049d760` | Pseudocode calls that helper and traverses/removes a keyed object; generated types include artifacts and require review |
| `0x00bf6e1a` | Pseudocode shows buffer/format-argument checks and forwarding to the formatting routine |
| `0x00c01650` | Pseudocode reads a format-string argument; has type-propagation/overlapping-global warnings |

The outputs prove useful decompilation, not recovery of original source or correctness of every inferred type. Original symbols are unavailable. Derived pseudocode stays under `local/phase1/decompilation/`; the saved project is `local/phase1/ghidra-project/grid-runtime.gpr`.

Commands from the project root:

```powershell
$python = 'C:\Users\debros\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'
& $python tools\export_runtime_image.py local\phase0\dumps\grid.exe_261006_001718.dmp --out-dir local\phase1\runtime
.\tools\run_ghidra_probe.ps1
# Reopen an existing imported project:
.\tools\run_ghidra_probe.ps1 -Existing
```

The exporter refuses to overwrite an existing snapshot. The wrapper selects JDK only for its process and restores `JAVA_HOME` afterwards. The post-script has been compiled and run by Ghidra.

### Process Monitor

Capture command: `Procmon64.exe /AcceptEula /Quiet /Minimized /BackingFile <private startup.pml> /Runtime 20`, followed by a Steam launch. Export: `/OpenLog <startup.pml> /SaveAs <startup-all.private.csv>`. The capture and export completed successfully. The driver/tool worked even though the spawning shell reported a non-administrator token; no claim is made about the precise elevation path. The monitor was terminated after capture/export.

The raw capture includes other system activity and remains private. `tools/summarize_procmon.py` streams the exported CSV and writes only GRID rows to `startup-grid.csv` plus a structured summary. Recorded counts: **2,052,174 total events**, **1,006,217 GRID events**, PID **3444**. The capture covers approximately 00:31:41–00:31:59 on 6 October, local time, and ends while GRID is querying display configuration. No GRID Process Exit appears in this capture; it is not the complete crash timeline.

Confirmed successful opens/reads include:

- `win_000.nfs`
- `system/applicationMemoryMap.xml`, `system/updateMemoryMap.xml`, `system/renderMemoryMap.xml`
- the user's `hardwaresettings/hardware_settings_config.xml`
- `system/hardware_settings_restrictions.xml`, `system/hardware_settings_options.xml`
- original Bink, D3DX9 and XInput libraries

The run includes many repeated video-registry queries, 656 unsuccessful `d3d.dll` probes in SysWOW64, and local DLL probes that are followed by successful system-library loads. A missing user-level `hardware_settings_restrictions.xml` probe also has a successful installation-level restriction-file access. These are observations; none has been established as the cause. Access-denied and other result codes require context rather than automatic classification as fatal errors. Instrumented startup may be substantially slower than the uninstrumented baseline.

### Native debugger

The pinned package includes x86 and x64 GUI debuggers and headless launchers. The x86 launcher supports `-pid`, `-c`, `-cf`, `-userdir`, `-workingDir` and `-help`. Its pinned source was read to verify the stdin command interface and `exit` handling; saved at `local/phase1/x64dbg-headless-source.cpp`.

Two actual GRID probes were performed:

1. **Late attachment, PID 4300:** attach succeeded at `EIP=0x00c01796`; the debugger confirmed `0xc0000005`, read address `0x10`. A subsequently set cleanup breakpoint was not hit, because execution had already passed it. Evidence: `x32dbg-startup.log`, `x32dbg-probe.json`.
2. **Pre-initialized debugger, PID 34196:** attached earlier at `EIP=0x76fa8ae9`, then set a hardware execution breakpoint at `0x0049fd2d`. The target did not hit it within the probe's bounded wait; the debugger was detached and exited. Evidence: `x32dbg-startup-early.log`, `x32dbg-probe-early.json`. This proves earlier attachment, not successful observation of the call or the initiating failure.

`tools/probe_debugger.py` refuses an existing GRID session, launches the original through Steam, attaches only to that new process, records results and detaches/exits in cleanup. No on-disk executable patch was made. Hardware breakpoints are chosen because loader writes can overwrite a software breakpoint placed too early.

Next debugger work: give the instrumented process a sufficiently long monitored startup interval, or break closer to the suspect object's creation/failure, and record the first initialization failure before cleanup. Do not report the earlier call-site breakpoint as validated yet.

## Decisions

- Prefer the existing JDK if compatible with the selected Ghidra release. Do not install a duplicate merely because Java is missing from PATH.
- GRID is x86. Use an x86 debugging session even when the host or debugger launcher is 64-bit.
- Protected on-disk `.text` differs from runtime bytes. A static project on the original binary is useful for identity/imports but cannot be assumed to decompile the runtime crash code correctly.
- Keep original executables, loaded-memory samples and derived decompilation private under `local/`.
- Rust/MSVC setup is now validated; graphics/audio/physics libraries will be selected when their concrete workload is established.
- Immediate research tools and native compilation work. A dedicated unpacked-PE project/loader trace and reference driving remain pending, so full Phase 1/2 exit criteria are not marked complete.

Primary references: [Ghidra releases](https://github.com/NationalSecurityAgency/ghidra/releases), [x64dbg releases](https://github.com/x64dbg/x64dbg/releases), [Microsoft Process Monitor](https://learn.microsoft.com/en-us/sysinternals/downloads/procmon), [Rust installation](https://rust-lang.org/tools/install/).

## Later compatibility dependencies and private static reference

Installed official portable NTCore 4GB Patch 1.0.0.1, Steamless release 3.1.0.3 and GridMTFix v0.3 resources for owner-authorized isolated experiments. CLI file-version field for Steamless reports 3.1.0.0. Source URLs, local hashes, unsigned status and actual run outcomes are in [memory-patch research](COMPATIBILITY_MEMORY_PATCHES.md). These old downloads provide no upstream asset digests; do not claim publisher-hash verification.

Steamless identified SteamStub variant 2.1 in the purchased executable and produced a private unpacked copy. This is an additional static-analysis reference; the original Steam installation remains unchanged. Its SHA-256 and derived manifest variants are recorded in that report. A per-process legacy-codepage variant survives the bounded startup probes; menu/race verification remains pending.

Owner subsequently confirms that the legacy-codepage variant reaches the main menu. Race/driving remains untested; this improves the original-game reference without completing every Phase 1/2 capability.

Earlier debugger target PID 34196 was later found alive with `Steam Error`; it was matched to our saved PID/start time and closed before compatibility probes. The previous empty-process snapshot did not guarantee that it remained closed.

## Native build installation checkpoint

On 6 October 2026, compiler inventory again found no Rust/Cargo/MSVC. The Winget WindowsApps alias could not execute. Started official installers using `tools/install_native_build_tools.ps1`:

- VS 2022 Build Tools release-channel bootstrapper from `https://aka.ms/vs/17/release/vs_buildtools.exe`, valid Microsoft signature, with VCTools, x86/x64 tools and SDK component `Microsoft.VisualStudio.Component.Windows11SDK.26100`. Target `C:\BuildTools\GRID`; no automatic restart. Component IDs checked against [Microsoft's directory](https://learn.microsoft.com/en-us/visualstudio/install/workload-component-id-vs-build-tools?view=vs-2022).
- Rustup from its official x86_64 MSVC distribution, published SHA-256 verified. Requested minimal profile, rustfmt and Clippy, no persistent PATH modification. Current download reports Rust 1.99.0; pin actual version after installation/build validation.

Both installers are still running at this checkpoint. Source/provenance JSON, logs and eventual result JSON: `local/tools/native-installers/`. Do not mistake a partial installation tree or successful download for a tested native toolchain. Check exit codes, then run Cargo and a native smoke build.

Later cleanup check: Rust installation completed with exit code 0, no reboot requested (`rust-result.json`, 01:20:31 local). Cargo/native build validation has not run. MSVC result was still absent at that check; retain its in-progress status until inspected.

## Native build validation — resumed session

The preceding installation checkpoints are historical. Both installers completed with exit code 0: Rust at `2026-10-05T22:20:31Z`, MSVC at `2026-10-05T22:25:41Z`. VS reports a complete, launchable installation and no reboot required.

| Component | Actual validated version |
|---|---|
| Rust | `rustc 1.99.0 (b940084d7 2026-09-28)`; LLVM 23.1.1; x86_64-pc-windows-msvc |
| Cargo | `1.99.0 (5f94df478 2026-08-27)` |
| VS Build Tools 2022 | 17.14.41 (September 2026), installation version 17.14.37710.0; `C:\BuildTools\GRID` |
| MSVC tools | 14.44.35207 |
| Windows SDK | 10.0.26100.0 |

The private `local/native-smoke` Cargo program built, linked and printed `GRID_NATIVE_BUILD_OK`, exit 0. Evidence: `local/tools/native-installers/{build-validated,vswhere-validated}.json`. The first public resource workspace subsequently built and passed six synthetic tests, formatting and Clippy with warnings denied. Actual GRID integration: 222 inspected files accepted.

`rust-toolchain.toml` pins 1.99.0 with rustfmt/Clippy; the named version was installed and tested rather than relying on a moving stable alias. No persistent PATH change was made. For PowerShell commands add `C:\Users\debros\.cargo\bin` to the current process PATH. Cargo discovers the installed MSVC linker. Bevy is not selected/installed yet. OneDrive reported an incremental-cache hard-link warning and copied files instead; compilation succeeded.

The separate unpacked-PE project now imports/saves correctly and supports targeted decompilation of two car/track candidates. Its bounded broad analysis remains partial; exact project identity, candidate addresses, provenance and limitations are in [loader research](LOADER_RESEARCH.md).

## Verified macro runtime

Installed official portable AutoHotkey 2.0.29 after bounded inventory found no existing installation. Runtime reports 2.0.29; pinned ZIP SHA-256 `b2d0200724a6b6ad22c965c939c5e5a2c64a35d1ccb455a3ca3f8ce415c5a296` matches the [official release digest](https://github.com/AutoHotkey/AutoHotkey/releases/tag/v2.0.29). Private executable `local/tools/autohotkey/AutoHotkey64.exe` has no Authenticode signature; publisher-digest verification is the provenance check used here. No system installer, driver or file association was added. Source metadata: `local/tools/autohotkey-source.json`.

`tools/install_research_tool.ps1 -Tool autohotkey` supports the pinned portable package. Independently authored `tools/grid_macros.ahk` and guarded `tools/run_grid_macro.ps1` are operational: menu navigation, bound hotkey, throttle, combined throttle/right and pause demonstrated. PowerShell parsing and actual script execution pass; the Rust input helper remains separately untested. See [macro setup](MACRO_SETUP.md).

Procmon 4.11 also completed the bounded race-load capture and native CSV export (exit 0), followed by streaming analysis. Detailed counts, verified resource hashes and coverage limits: [reference dependencies](REFERENCE_DEPENDENCIES.md).

## Live debugger and controlled numerics — 7 October

Installed x32dbg headless release 2026.05.27 is validated with disposable x86 breakpoint/detach/survival tests and twelve actual GRID placement-boundary captures. Explicit pause left a verified thread suspend count of one in a failed trial; guarded recovery restored menus. Upstream [pause-race fix PR 3906](https://github.com/x64dbg/x64dbg/pull/3906) supports this interpretation. The official latest release API still returns the installed version. Capture now supports `--arm-running`; original GRID survival after the successful capture remains unverified. [Detailed trials and limits](LIVE_SPAWN_CAPTURE.md).

Existing MSVC/Rust/Ghidra/Python tools build and execute isolated marker and outer dynamics probes. Debug/release Rust outer arithmetic agrees in 139 controlled cases; [dynamics contract](VEHICLE_UPDATE.md). Nothing newly installed for these continuations. Earlier statements about Bevy not yet being selected are historical; current pinned viewer is Bevy 0.19.1.

The same strict MSVC/SSE2 mapper now runs supplied-impulse (205 cases), conditional suspension (225 cases), point-response (253 cases), conditional wheel-coupling (192 cases) and coupled substep/contact/travel-stop (212 cases) helpers, all exactly matching Rust debug/release. Guarded read-only live context inspection and a bounded 2,000 ms throttle sequence also completed; profile files unchanged and display config restored. Exact tool versions and numerical scope remain recorded in [vehicle update](VEHICLE_UPDATE.md). No additional installations.

Supplied-state pose prediction (226 cases) and conditional two-pass finalization (200 cases) now match original whole helpers in debug/release; 62 project tests and Clippy pass. Same tools/versions, no new installations. [Scope and evidence](VEHICLE_UPDATE.md).

The initial suspension reader and unstubbed wheel initializer now compare 44 sources /968 words exactly in both build profiles, while 24 guarded runtime observations explicitly retain three changed fields. Existing tools reused; 63 tests and Clippy pass. New bounded validator `tools/validate_suspension_initialization.py`.

Preload (216 cases), suspension setup subset (200), range/default helpers (88), and combined retained-runtime chain (24 observations) now pass exact debug/release comparisons. **66 tests** and Clippy pass. All existing tools reused. Windows installer-name heuristic blocked the initially named setup probe before execution; renamed `suspension-prepare-probe.exe` runs without elevation. Detailed provenance and limits in [vehicle update](VEHICLE_UPDATE.md).
