# Phase 0 — Windows Steam reference

Recorded: 6 October 2026, Europe/Kiev. Research only; no engine implementation.

**Result:** exact reference binary identified and startup crash reproduced. The immediate exception is an invalid read from `0x10` in a string-formatting routine. CPU-affinity, refresh-rate and windowed-mode probes all reproduce the same exception. The initiating startup failure remains unresolved; a usable original-game baseline has not been established.

**Later result:** [controlled memory/encoding experiments](COMPATIBILITY_MEMORY_PATCHES.md) identify a startup remedy in a private unpacked copy: Windows 11 per-process `activeCodePage=en-US`. Matched UTF-8 control reproduces the original exception; owner confirms the legacy copy reaches the main menu. The original Steam installation remains preserved. The internal failing API and race/driving compatibility remain unresolved; the initial findings below are retained as historical evidence.

## Installation and binary identity

| Field | Observed value |
|---|---|
| Installation | `C:\Program Files (x86)\Steam\steamapps\common\Grid` |
| Steam application | GRID, app ID `12750` |
| Installed build | `252066`, state flags `4` |
| Installed depot | `12751`, manifest `8821377416256322215` |
| Language | English |
| Patch/version evidence | PE FileVersion and ProductVersion `1.3.0.0`; installscript DisplayVersion `1.30.0000` |
| Executable | `grid.exe`, 14,476,288 bytes |
| SHA-256 | `dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f` |
| PE architecture | Machine `0x014c` (Intel x86), optional header `0x010b` (PE32): **32-bit**, running under WOW64 |
| PE timestamp | `0x4ace02bc`, 2009-10-08 15:18:20 UTC; compiler metadata, not installation date |
| Preferred image base | `0x00400000` |
| Entry RVA | `0x00eda2ed`, in `.bind` |
| Large Address Aware | **False**; recorded only, no patch applied |
| Inventory | 22,375 files; 10,820,269,761 bytes |

The inventory byte total exactly matches Steam's `SizeOnDisk`. This is not a content-integrity check against Steam's depot hashes. Version identifies the reported patch level; the executable hash identifies the actual bytes. A `.bind` section and runtime/on-disk code differences are consistent with a protected Steam executable. We did not remove protection or modify it.

Static imports include `d3d9.dll!Direct3DCreate9`, `d3dx9_37.dll`, `OpenAL32.dll`, `DSOUND.dll`, `binkw32.dll`, `DINPUT8.dll`, and `XINPUT1_3.dll`, plus Windows libraries. The historical WER report confirms those runtime modules loaded, along with NVIDIA D3D9 driver modules and Steam/NVIDIA overlays. This makes a simple missing startup DLL unlikely; it does not prove every asset, delayed dependency or API call succeeded. There is no local `d3d9.dll` graphics wrapper in the game directory.

Machine-readable evidence: `local/phase0/installation.json`, `installation-files.csv`, `exe-version.json`, `appmanifest_12750.redacted.acf`, `historical-wer-summary.txt`. Full static imports and selected resource SHA-256 values are in `installation.json`; the CSV records paths, sizes and timestamps rather than hashing all 10.8 GB.

## Reference PC

| Component | Observed value |
|---|---|
| OS | Windows 11 Pro Insider Preview, 64-bit, build `26220.9568`, display version `25H2` |
| CPU | Intel Core i7-12700, 12 physical cores, 20 logical processors |
| Active GPU | NVIDIA GeForce RTX 4070, driver `32.0.16.1692` |
| Other adapters | Intel UHD Graphics 770, driver `32.0.101.7088`; Meta Virtual Monitor, driver `5.3.57.114` |
| Current display | 2560 × 1440, 165 Hz as reported by Win32_VideoController |
| Inputs | Keyboard, mouse, Logitech virtual HID devices, one generic HID game controller |

The controller model, wheel/pedals and force-feedback capability are unverified. CIM's current display data does not enumerate every monitor or establish the active display topology.

GRID's existing `hardware_settings_info.xml` reports `cores="20"`, `hyperThreading="false"` and an obsolete Windows version. Its saved config selects `workerMap4Core.xml`, parallel update/render, software audio, 1280 × 720 fullscreen, 165 Hz and 2x MSAA. Those are recorded facts, not proof of the crash cause. Existing registry compatibility-layer evidence contains a Windows 7 setting for an **old E: drive GRID path**, not the current C: executable. Two Steam user-config GRID blocks had no `LaunchOptions` entry when inspected; the user's earlier options are not reconstructed from that.

## Failure reproduction and diagnosis

The owner reports: the first loading screen appears, then GRID crashes immediately; previous launch flags and compatibility options did not help. Menu and race loading are not reached.

1. Leave the installed executable and system files unchanged.
2. Launch with Steam: `steam.exe -applaunch 12750`.
3. Observe GRID's processes and the first loading screen, then process exit. Steam may create a transient launcher process as well as the failing game process; identify the failing PID from the event.
4. Read Application Error event 1000 and Windows Error Reporting event 1001, matching the installation path, version and timestamp.

| Run, local time on 6 October | Result |
|---|---|
| Owner's historical run, error at 00:06:55 | `0xc0000005`, `grid.exe + 0x00801796` |
| Fresh Steam baseline, launch 00:10:26 | Same exception/offset; error at 00:10:34 |
| ProcDump minidump, 00:11:46 | Same exception; read address `0x10` |
| Temporary CPU affinity, launch 00:14:58 | Mask `0x000f` successfully set on failing PID 32676 before the crash; same exception/offset |
| Fullscreen refresh only, launch 00:16:06 | 165 → 60 Hz; same exception/offset; original config restored |
| Full dump, 00:17:18 | Same exception and invalid pointer; runtime caller code captured |
| Windowed only, launch 00:19:27 | Fullscreen true → false; saved rate remained 165; same exception/offset; original config restored |

An earlier affinity-probe attempt failed to read affinity from a transient process. It is not counted as a successful diagnostic. The revised probe recorded the successful affinity setting on the PID identified in the corresponding crash event. Affinity does not necessarily change the processor count returned by every detection API, and does not validate or repair GRID's worker-map logic.

### Confirmed immediate failure

- Exception `0xc0000005`, parameter 0 = `0` (read), parameter 1 = `0x10`.
- Runtime instruction address `0x00c01796`, equivalent to image RVA `0x00801796`.
- Instruction bytes `8a 13`: `mov dl, byte ptr [ebx]`; `EBX = 0x10`.
- The routine at `0x00c01650` obtains this pointer from its argument stack and uses it as a formatting string. Its caller sets up a bounded output buffer of 32 bytes through the routines at `0x00bf6e1a` / `0x00bf6ecb` and `0x004b3960`. This identification is based on runtime disassembly and actual argument values, not recovered symbols.
- The runtime call site at `0x0049fd2d` follows `mov ecx, [esi + 0x28]` and `add ecx, 0x10`, then passes that value onward. The observed argument is `0x10`, consistent with a null object pointer plus an unchecked member offset.

The nearby function releases objects and clears pointers; an upstream routine at `0x004ff7b0` has a deleting-destructor pattern. **Interpretation, medium confidence:** this is unsafe object cleanup during startup, possibly masking an earlier initialization failure. The invalid read is established; the first failed initialization operation is not. The raw stack scan also contains stale code pointers and strings such as `BroadcastMessage_ProfileUpdated`; these are not a verified call stack or proof of the failed subsystem. The formatting routine uses an unusual EBP layout, so a conventional frame-pointer chain initially returns misleading results; its prologue and saved-frame/argument area were checked directly.

Do not attribute the crash to high refresh, CPU count, VRAM, OpenAL, Windows Insider, overlays or Steam protection solely from this dump. These remain candidate triggers or confounders. The three controlled probes disprove those particular settings changes as sufficient remedies on this installation.

Next targeted investigation: capture the original initialization failure before cleanup, using a debugger breakpoint at the observed call site and a filtered Process Monitor trace of the failed startup. Check failed file/registry operations and renderer/device creation results. Steam content verification and individual overlay/audio/display experiments can follow concrete evidence; do not replace a precise trace with an omnibus fix.

Evidence: `local/phase0/baseline-launch.json`, `crash-events.json`, `crash-context.json`, `verified-format-call.txt`, `runtime-disassembly.txt`, `upstream-disassembly.txt`, `affinity-test.json`, `refresh-test.json`, `windowed-test.json` and local dumps. Dumps contain original runtime code/data and stay excluded from Git.

## Tools checked before additions

Checked command resolution, HKLM/HKCU uninstall records, and executable filenames under Program Files, Program Files (x86), Downloads, Desktop, user/local Tools, `.cargo`, and AppData Local Programs. No exhaustive search of all drives was performed. Portable tools elsewhere remain possible.

| Tool | Initial finding |
|---|---|
| Python | WindowsApps aliases in PATH; a real bundled Python **3.12.14** works under Codex's dependency runtime |
| Git | Bundled Git **2.53.0.windows.3** works |
| Node.js | Bundled Node.js **24.19.0** works; npm not resolved |
| Java / javac | Not in PATH. Songs of Syx Demo bundles a working full **OpenLogic JDK 21.0.9, 64-bit**, including javac. rFactor 2 also has a Java executable; its version was not tested |
| Rustup / Rust / Cargo | Not found in inspected locations or command resolution |
| MSVC compiler / SDK | No `cl.exe`, `dumpbin.exe`, Visual Studio or Windows Kits location found in checks; installed VC++ redistributables are runtime libraries, not a compiler toolchain |
| Ghidra / ImHex / REA | Not found in inspected locations |
| ProcMon / ProcDump / x32dbg / x64dbg / WinDbg / cdb | Not found initially |
| apitrace / RenderDoc / EGO tools / Blender | Not found in inspected locations |
| Python PE/disassembly modules | `pefile`, `capstone`, `lief`, `minidump` absent from bundled interpreter initially |
| Windows runtimes | Multiple x86/x64 Visual C++ redistributables installed; required OpenAL32 and D3D9 modules actually loaded in GRID |

After the inventory, two narrowly scoped tools were added **inside `local/tools/` only**:

1. **Microsoft ProcDump 12.01**, portable official ZIP; Authenticode status Valid, Microsoft Corporation. ZIP SHA-256 `68e057587b0fd654efa095f76d80d633c0e5c60ea26fd3e7c0011c076bb2d00c`. Used `-mm -e -n 1 -w grid.exe` and later `-ma -e -n 1 -w grid.exe`; the standard license was accepted with `-accepteula`. No postmortem-debugger registration (`-i`) was performed. [Microsoft documentation](https://learn.microsoft.com/en-us/sysinternals/downloads/procdump).
2. **Capstone 5.0.6**, Python library installed with `pip --target local/tools/python`; used only for x86 runtime disassembly. The bundled Python environment itself was not modified.

No Rust, Ghidra, ImHex, REA, DXVK or GridMTFix was installed. The currently published REA README says Windows Ghidra operations are unavailable, so REA should not be assumed to provide this PC's native-analysis bridge. [REA primary documentation](https://github.com/morluto/rea#current-status). Standalone Windows Ghidra remains a Phase 1 candidate; validate its chosen release's JDK requirements before reusing the game-bundled JDK or installing another.

GridMTFix's author documents worker/hardware changes and a high-refresh DXVK addon. Its Steam workflow also modifies executable protection, so the bundle was not executed. These external descriptions are context, not proof of this PC's cause. [GridMTFix primary documentation](https://github.com/xatornet/GridMTFix).

## Provisional car/track reference and recipe

Select the installed `cars/lac` car resource set and `tracks/europe/donington/route_0` track resource set. These are provisional first-scene candidates; the expected Lacetti / Donington UI names, route length, selectable race pairing and reverse/forward mapping still require validation in a running reference or the database. Presence on disk does not establish a complete dependency map.

Selected hashes include `cars/lac/lac_gen_high.pssg`, `cars/lac/lac_highLOD.pssg`, `cars/lac/lac.csv`, `tracks/europe/donington/route_0/track.jpk`, `route.clm`, `database/database.bin` and `raceload.jpk`. The tuning file is CSV in this car's directory; no `.ctf` files were found in the overall installation inventory. Track-directory XML extensions can contain binary data; extension alone must not select a parser.

After startup is usable: select the matching car and route in Race Day/test mode if available, use a dry daytime session, record the exact UI names, assists, damage setting, input device, camera and graphics settings, and perform a restart plus one timed lap. Do not call these conditions measured or claim the pair is playable before that run. Keyboard is the initial reproducible input; controller identity and wheel support remain open.

Phase 0 exit criteria of binary identity and failure reproduction are met. The research-only car/track recipe is provisional. Phase 1 toolchain proof and Phase 2 running-reference acceptance are not complete.

## Preservation and repeatability

The original installation was only read. No game executable, DLL, system worker map or asset was changed. The user config was snapshotted before runs; the refresh/windowed probes restored its bytes in `finally`, and its final SHA-256 matches the snapshot: `74f2470aad62dcd608ee37c9557061ea732d9e9ef6856be9bdb6a0287bdfce6f`. Normal game launches may update generated hardware-info files and Steam/WER state; that is separate from intentional configuration changes. No save files were found under the inspected Codemasters GRID Documents directory before testing; other save locations were not exhaustively searched.

The installation manifest and selected hashes were regenerated after diagnostics; the executable hash is unchanged. `local/` is excluded by `.gitignore`, with no remote publication or repository initialization performed.

Read-only recollection:

```powershell
$python = 'C:\Users\debros\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'
& $python tools\inspect_installation.py --game 'C:\Program Files (x86)\Steam\steamapps\common\Grid' --out local\phase0
.\tools\collect_machine.ps1
```

The machine collector requires access to Windows CIM and the Application event log. The probe scripts actively launch the game; `probe_refresh.ps1` temporarily edits the user config. Run probes separately with no existing GRID session. Static inspection and dump inspection do not launch GRID. Diagnostic tools are research utilities, not the proposed Rust engine or GRID format readers.
