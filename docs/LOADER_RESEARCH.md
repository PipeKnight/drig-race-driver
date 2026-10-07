# Private PE loader survey

Recorded: 6 October 2026, Europe/Kiev. This is candidate-location research; no loader breakpoint or race-loading behavior has been validated yet.

## Reference and project

Private unpacked control: `local/compatibility/grid-unpacked.exe`, SHA-256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. This is the purchased binary's isolated analysis copy, distinct from both the protected original and the legacy-manifest gameplay variant.

Ghidra 12.1.4 imported it with the Portable Executable loader as `x86:LE:32:default:windows`, base `0x00400000`. Project: `local/phase3/ghidra-project/grid-unpacked.gpr`. The earlier raw crash-memory project is retained separately.

Automatic analysis was bounded to 120 seconds, four CPUs. It reached the deadline during analysis; reported analyzer total time was 133 seconds. The project saved successfully with **27,871 inferred functions**. That count is not a count of recovered original functions or fully correct boundaries. No matching `rd_master.pdb` was found. External libraries supplied cached import/export information but were not saved into this project; missing Bink/OpenAL in the importer search path is not a gameplay missing-library diagnosis.

The first survey found zero defined resource strings because the broad analysis was incomplete. The script was extended to scan printable ASCII runs in initialized mapped sections (block cap 64 MiB, run cap 4096 bytes), then rerun without broad analysis. It found **107 keyword-matching candidate strings and 100 references**. These are static references in the partial project, not observed file accesses.

Private output: `local/phase3/loader-survey/loader-survey.tsv`; execution logs alongside it. No vendor pseudocode is placed in source directories.

## Concrete candidates

All values are virtual addresses in this hash-identified PE at its preferred base. If the runtime base differs, convert through RVA before setting a breakpoint.

| Candidate | Static evidence | Interpretation / next check |
|---|---|---|
| `0x005d8290` | Ref at `0x005d93f8` to initialization diagnostic; references to PSSG default/front-end/track/car context names | Likely initialization/setup area; follow context creation and link resolver calls |
| `0x0060bef0` | Ref at `0x0060cb89` to `tracks/track_loader.xml`, string at `0x00ce633c` | Track-loading/setup candidate; observe arguments and child calls while selecting/loading a track |
| `0x006138d0` | Ref at `0x00614c93` to `cars/car_loader.xml`, string at `0x00ce67a0` | Car-loading/setup candidate; observe car ID and resolved resource paths |

Decompiled the two car/track candidates successfully using the saved PE project. Private outputs: `function-candidates/60bef0.c.txt` (14,946 bytes) and `6138d0.c.txt` (26,552 bytes), under the loader-survey directory. They contain inferred types, calling conventions and names; successful decompilation does not confirm those inferences or prove the functions are the binary parsers.

Other mapped strings contain track-container and `raceload.jpk` path templates. Some templates occur within larger strings and have no reference to the substring start. A zero substring-reference count does not show the template is unused.

## Reproduce

```powershell
# First import into a new private project, with bounded auto-analysis:
.\tools\run_ghidra_loader_survey.ps1
# Reuse saved project; do not restart broad analysis:
.\tools\run_ghidra_loader_survey.ps1 -Existing
# Reuse and export the two statically identified function candidates:
.\tools\run_ghidra_loader_survey.ps1 -Existing -DecompileCandidates
```

The wrapper verifies the reference SHA-256 and selects the existing JDK for its process. Initial sandboxed launch could not write Ghidra's existing `java_home.save` preferences cache; running with authorized cache access succeeded. This was a filesystem permission limitation, not an incompatible/missing JDK. The wrapper does not overwrite the existing project on its normal import path; use `-Existing` for subsequent work.

Next: race-load trace and runtime breakpoints at the identified references/containing functions, followed by specification of resource lookup and buffers. PSSG geometry consumers still need to be located and verified. Full automatic analysis can be resumed separately when its cost is justified; no complete decompilation is claimed here.
