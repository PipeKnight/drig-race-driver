# GRID memory patches and startup compatibility

Research date: 6 October 2026, Europe/Kiev. See [project log](../PROJECT_LOG.md) and [reference crash](PHASE0.md).

## Online findings

The author's [NTCore 4GB Patch](https://ntcore.com/4gb-patch/) sets a PE flag that lets a 32-bit program address up to 4 GB of user virtual address space on 64-bit Windows. Microsoft's [memory-limit documentation](https://learn.microsoft.com/en-us/windows/win32/memory/memory-limits-for-windows-releases) confirms the 2 GB / 4 GB distinction for `IMAGE_FILE_LARGE_ADDRESS_AWARE`. This does not increase installed physical RAM or repair every invalid-pointer failure. Our reference executable is x86 with this flag clear. Its observed read at `0x10` does not itself establish memory exhaustion.

[GridMTFix's author](https://github.com/xatornet/GridMTFix) supplies worker maps and revised hardware restrictions/options/ratings. Its Steam procedure uses Steamless before NTCore's patch. Thus a full GridMTFix test changes multiple things; it cannot isolate the memory flag's effect. We will distinguish an unpacked control, LAA alone, and the full bundle. Source reviewed at commit `c4bbc1d63f4fb1ff4d2add637d0286084d7bbe4b`; no upstream installer was blindly executed.

The [HD-resolution guide by Noxbringer](https://steamcommunity.com/sharedfiles/filedetails/?id=1363383540) is a different workaround: change/remove the `res` entry in `system/hardware_settings_restrictions.xml`, addressing an alleged VRAM-detection resolution cap. Its example changes `mem="270" maxWidth="1280"` to `mem="4096" maxWidth="7000"`. This does not change the executable's virtual-address-space limit.

The owner reports trying both patches without success and authorizes repeating them. Which exact earlier packages, binaries and settings were used remains unknown; those earlier results are user-reported rather than newly controlled experiments.

## Separate UTF-8 lead

A [firsthand Steam startup-fix report dated 11 January 2025](https://steamcommunity.com/app/12750/discussions/0/592886010360179176/) combines GridMTFix with disabling Windows' beta UTF-8 system-locale setting and rebooting. This is anecdotal and confounded by multiple changes, not proof of causation on this PC. [Microsoft documents](https://learn.microsoft.com/en-us/windows/apps/design/globalizing/use-utf8-code-page) this system setting and its reboot requirement.

Read-only checks on this PC found registry `ACP`, `OEMCP`, `MACCP` all `65001`, system locale `en-US`, and live kernel32 `GetACP()` / `GetOEMCP()` both `65001`. This confirms active UTF-8 codepages, independently of PowerShell's output encoding. Evidence: `local/compatibility/codepages.json`. **H003: UTF-8 compatibility is a concrete but unconfirmed startup-failure hypothesis.** No system-locale change or reboot has been performed.

## Controlled experiments

In progress: original Steam installation is preserved. Full research copy at `C:\Users\debros\Tools\grid-research\grid-compat` avoids introducing a Cyrillic executable path and supports a direct-launch control. User settings will be snapshotted/restored. Tool downloads/provenance are in `local/tools/memory-fixes/sources.json`; old GitHub assets do not provide upstream digests, so downloaded hashes are recorded without claiming an independently verified publisher hash.

Results will be appended here after actual runs. A surviving process is not proof that the menu or a race works; a crash must be matched to the launched executable path/PID and event or dump.

- Copy completed: all 22,375 files, 10,820,269,761 bytes; no copy failures. Robocopy returned `1`, its success-with-files-copied status.
- Initial probe refused to run because earlier debugger target PID 34196 remained alive with window title `Steam Error`. Matched PID and start time to our saved early debugger record, then closed it. Earlier toolchain verification's empty running-process snapshot is superseded by this later observation.
- Direct copied protected-executable control, PID 31668: remained at `Steam Error`, no startup crash captured; **not a usable baseline**. The original Steam-launch baseline remains the Phase 0 reference.
- Unpacked control, PID 25860: same `0xc0000005`, RVA `0x801796`, path points to isolated copy. No LAA flag. This establishes a comparable private control for subsequent patch experiments. Steamless identified SteamStub variant 2.1 and produced the private unpacked executable.
- NTCore LAA-only, PID 33772: flag verified set; same `0xc0000005`, RVA `0x801796`, isolated executable path. This patch is insufficient for startup on this PC in the tested conditions. User XML settings restored after both runs; Steam executable hash unchanged.

Run records: `local/compatibility/baseline.json`, `unpacked.json`, `laa.json`; tool output `steamless-stdout.txt`. Subsequent VRAM-only experiment uses the unpacked, non-LAA control, since the copied protected executable stops at `Steam Error`.

- VRAM-resolution XML only, PID 29472: still `0xc0000005`, but RVA changes to `0x484714`. This change affects execution and does not fix startup. Record: `vram.json`; do not conflate this exception with the original formatter fault.
- First full-bundle setup failed before game launch: Start-Process for NTCore returned "The operation was canceled by the user." Saved `mtfix.json`; settings restored. No successful GridMTFix test is inferred from this failed setup.
- Attempt to reconstruct and reuse the first NTCore result by setting only the flag failed its SHA-256 equality assertion. No candidate executable was saved. NTCore also recomputes the PE checksum (`0` to `0xd8205a`); recording only the LAA bit is insufficient to recreate its exact output.
- Full-bundle retry ran successfully as an experiment, PID 3384: LAA true, 20-core worker map selected in the generated config, settings reset generated 2560x1440 at 165 Hz with 4x MSAA. Still `0xc0000005`, RVA `0x801796`. Record: `mtfix-retry.json`; settings restored. The actual NTCore result was saved after verifying SHA-256 equality with the first LAA run.

## Per-process encoding experiment

Windows 11's [application manifest documentation](https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests#activecodepage) allows `activeCodePage` to name a locale, including `en-US` (Windows-1252/437). We can test this in a private unpacked executable without changing the machine-wide UTF-8 setting. The existing manifest's privilege declaration is preserved; only a codepage request is added. Independently authored script: `tools/set_copy_codepage.py`.

- Legacy manifest (`en-US`), PID 34356: non-LAA unpacked executable, original system XML and original user settings. Stayed running through 40 seconds, final window title `GRID`, no recorded crash event. Main menu/race not visually verified. Record: `legacy.json`. All saved XML settings restored, original executable unchanged.
- Matching UTF-8 manifest, PID 16964: same original `0xc0000005`, RVA `0x801796`. Record: `utf8-manifest.json`.
- Binary comparison proves both manifest-test executables are 14,127,408 bytes and differ **only at five consecutive offsets 13,612,964–13,612,968**, the strings `en-US` versus `UTF-8`. Same non-LAA flag, original system files/settings and launch method. Evidence: `manifest-binary-comparison.json` and private paired executables.
- Legacy confirmation, PID 23632: same legacy-manifest SHA-256, survived 60 seconds with final window title `GRID`, no crash event. Record: `legacy-confirmation.json`; settings restored. Main menu/race still not visually verified at the end of the automated probe; owner feedback requested.
- This controlled result substantially strengthens H003: requested process codepage determines whether the original startup exception is reproduced in the isolated unpacked copy. It does not identify the failing conversion/API inside GRID, establish full race compatibility, or verify the original protected Steam binary with a manifest patch. Survival alone is insufficient to call Phase 2 complete.
- **Owner confirmation:** "Yes, now it reaches the main menu window." Main-menu access is therefore confirmed by owner observation during the legacy confirmation. Automated records remain honest about their lack of UI inspection; separate `owner-observation.json` records this evidence. Race/driving remains untested.

## Download provenance

Checked existing research tools before installing these portable dependencies. Official URLs were taken from the author's reviewed Steam script; no third-party repack downloaded. NTCore `4gb_patch.exe` version `1.0.0.1`; Steamless release `3.1.0.3` (CLI file-version field reports `3.1.0.0`); GridMTFix resource archive tag `v0.3`. Executables are unsigned; no upstream asset SHA-256 was provided for these older releases. These are recorded local hashes, not independent signature verification:

| Archive | SHA-256 |
|---|---|
| NTCore | `1e13c263dc55e85cf62e3e30c6b2ce936d4c7e16e3bba3e311a5a8bed4be16ca` |
| Steamless | `47e0f09235db0f34bff963c6b103012aea4c0b685a5a595c938fa8920962ed82` |
| GridMTFix resources | `e0c3a1bfe9def24d6c9e849becc06f6c26d5b63b45656acc4787aa1fd7174348` |

Private unpacked executable SHA-256: `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. NTCore output: `72f52024e504957ae7a5dfa3ec66c9ac08bedd2679a718ad5a21689a2688a4db`. Legacy-manifest copy: `3aac37346531229daaec760d00ed0b6189c935be2356b97f039a680a795b9cda`. UTF-8-manifest control: `511044f3ffe6b2a207617a2940cb4b6bddd7caa492f854667a2ac9de54ac7a97`.

Future reference work should use the private legacy-manifest copy with its identity recorded, retain the unmodified Steam installation, and validate menu/race behavior before declaring a running reference established. No machine-wide locale change is needed for this bounded workaround.

Final verification checks all 37 previously hashed original files unchanged, both original settings XML files restored byte-for-byte, and completed run restoration records consistent. Current private executable hash matches the legacy variant. No GRID/headless/NTCore process remained in the final snapshot. Evidence: `local/compatibility/verification.json`.
