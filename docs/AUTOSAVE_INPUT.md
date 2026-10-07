# Autosave path and menu input investigation

Recorded: 6 October 2026, Europe/Kiev. Autosave creation and restart persistence are confirmed; reliable automated keyboard navigation remains unresolved.

## Owner observations

The owner confirms that arrows and Enter navigate GRID, and the mouse does not appear to operate menu selections. The owner could not create an autosave profile in the `en-US` compatibility copy and reached the main menu without persistent progress. Therefore earlier automated-key failures are not evidence of broken physical keyboard input.

## Confirmed Windows path conversion

The startup workaround used per-process `activeCodePage=en-US`, which selects Windows-1252. The actual Documents folder is `C:\Users\debros\OneDrive\Документы`. Cyrillic letters cannot be represented by that codepage.

An independently authored, read-only native probe calls `GetACP`, `SHGetFolderPathA/W` and `GetFileAttributesA/W`. It neither creates files nor changes the folder, locale or registry. Three separately manifested executables were compiled with validated Rust 1.99.0/MSVC and run on this PC:

| Manifest | Actual ACP | ANSI Documents result | Unicode Documents result |
|---|---:|---|---|
| `en-US` | 1252 | Lookup HRESULT succeeds, but final folder name becomes nine `?` bytes; attributes fail with error 123 (`ERROR_INVALID_NAME`) | Correct folder; attributes succeed |
| `uk-UA` | 1251 | Correct Cyrillic path bytes; attributes succeed | Correct folder; attributes succeed |
| `UTF-8` | 65001 | Correct UTF-8 path bytes; attributes succeed | Correct folder; attributes succeed |

Private results, executable hashes and exact path bytes: `local/autosave/codepage-probes/results.json`. Reproduce with `tools/probe_document_codepages.ps1`; source is `tools/windows/document_codepages.rs`.

This **confirms the Western-codepage path defect**. Static investigation below identifies ANSI Documents lookup in GRID's save-directory setup, and later owner/file observations confirm actual save creation and restart persistence with `uk-UA`. UTF-8 resolves the path in the probe but still triggers GRID's previously confirmed startup failure, so it is not our current gameplay remedy.

### Static confirmation in GRID

The original PE inventory already records imports of `SHGetFolderPathA`, `CreateDirectoryA`, `GetFileAttributesA`, `CreateFileA` and `WideCharToMultiByte`. A new saved-project survey identifies 10 matching import/IAT symbols and 180 reference records (including duplicate views of the same import and IAT; these are not 180 unique callers). `SHGetFolderPathA` has six observed code callers; all six decompiled successfully into private output.

At `0x005b9680` and `0x005b9750`, disassembly confirms a call through IAT `0x00c8936c` with folder argument **5 (Documents)**. Referenced path components append `\Codemasters` and `\GRID`, then ANSI directory creation is checked. The second routine's failure path reports that the save directory could not be created and will not be mounted, then clears an object flag. This is concrete static evidence connecting the bad ANSI Documents path to GRID's save-directory setup. It is not a recovered original function name or a runtime breakpoint observation. Decompiled stack/types/parameter counts contain artifacts, so the call argument is checked against actual instructions.

Private evidence: `local/autosave/static-survey/save-path-imports.tsv`, six `function-candidates/*.c.txt`, and `path-code.json`. Reproduce using `tools/run_ghidra_loader_survey.ps1 -Existing -SaveSurvey`, `-Existing -DecompileSavePaths`, and `tools/inspect_save_path_code.py`. Addresses apply to the hash-identified unpacked PE documented in [loader research](LOADER_RESEARCH.md).

Microsoft primary references: [per-process locale codepages](https://devblogs.microsoft.com/oldnewthing/20220531-00/?p=106697), [manifest activeCodePage](https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests).

## Private Cyrillic-codepage reference

Closed only the identified prior reference PID 7700 after verifying executable path and saved start time. First guard attempt falsely rejected it because a JSON datetime was converted through an unqualified local-time string; no process or file was changed by that rejected attempt. Preserving `DateTimeOffset` UTC semantics fixed the guard.

Backed up the entire existing GRID user directory and the known `en-US` executable under `local/autosave/game-manifest/`. Modified only the private compatibility copy's embedded locale request, preserving all other manifest declarations and resources. Original Steam files and global codepage settings were not edited.

| Variant | SHA-256 |
|---|---|
| Prior `en-US` | `3aac37346531229daaec760d00ed0b6189c935be2356b97f039a680a795b9cda` |
| Current `uk-UA` | `b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6` |

Both files are 14,127,408 bytes and differ at only three offsets: 13,612,964; 13,612,965; 13,612,968. These change `en-US` to `uk-UA`, with no other byte differences. Comparison record: `local/autosave/game-manifest/binary-comparison.json`.

New owned reference PID 30524, started `2026-10-06T00:55:58.2577424Z` (03:55:58 local). It reaches the resolution prompt and plays the introduction, preserving startup compatibility beyond the first loading screen. No profile/save success is claimed yet. The resolution confirmation timed out with No selected; a later capture is 800x600. The game rewrote its hardware-settings XML (1835-byte config, 6501-byte information file); no manual XML edit was made. Earlier files remain backed up.

One screenshot refresh failed with `foreground window did not report a process id`; fresh window selection/activation recovered the capture. This is an automation observation, not an additional game crash diagnosis.

## Menu automation limit

Observed injection attempts with Down, Return, numpad 2 and Right have not produced a verified selection change. The pointer can move, but clicking is not a demonstrated GRID menu control. Enter also did not visibly skip the introduction. The API exposes key/chord presses but no key-hold duration or scan-code selector; physical and injected input are different experimental conditions. No claim about exact injection implementation or initiating cause is made.

The Computer Use skill's guidance requires using its JS APIs for Windows app automation. No alternate SendInput/PowerShell input helper was used. Windows supports scan-code injection as documented in [KEYBDINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput), but the available skill API does not expose that option. This is a potential future comparison, not a tested fix.

The owner was asked to create/confirm the default local profile with arrows/Enter, enable autosave and leave the game open. Next validation: inspect newly created save files and errors, then restart the same hash-identified private copy and verify the profile persists. Record race setup, loading, driving and restart separately. Preserve any new user profile; do not restore a whole older GRID directory over it.

## Actual save creation and persistence experiment

Owner reports: "Now it works! You can inspect." Inspection confirms **15 files, 345,648 bytes** under `savegame/Autosave0`, written around `2026-10-06T01:01:10Z` (04:01:10 local). Previously that directory was absent. Metadata/hashes: `local/autosave/profile-created-files.json`; complete private backup: `local/autosave/profile-created-backup/`. User data remains private. This verifies successful creation on disk as well as the owner's UI observation.

The game had spawned child PID **33592** at `2026-10-06T00:56:26.8219704Z`, with recorded parent PID **30524** and the same private executable. This occurred after the resolution prompt; the exact restart trigger was not observed. A restart guard correctly refused the stale parent PID. Child parent, path, creation interval and executable hash were checked before closing the child; no unrelated process was closed.

Restarted the same `uk-UA` executable, PID **19680**, at `2026-10-06T01:04:50.0562006Z` (04:04:50 local). Complete user backup before this restart and session metadata are under `local/autosave/persistence-restart/`. No save/user file was replaced or restored. The owner explicitly confirms: "Yes, the profile has loaded correctly." This verifies restart persistence by UI observation in addition to the earlier on-disk save evidence. The owner subsequently opened Race Day and launched the proposed race.

`tools/restart_grid_reference.ps1` validates PID, executable path, start time and hash, snapshots the user directory and restarts only that reference. Its optional child handling requires the original recorded parent, same executable and a child launch within 120 seconds; it does not choose an arbitrary later GRID process.
