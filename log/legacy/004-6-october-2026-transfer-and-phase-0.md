## Session history

### 6 October 2026 — transfer and Phase 0

- Read the supplied plan as background and preserved it at [planning reference](docs/reference/GRID_2008_OPEN_SOURCE_PLAN.md).
- Identified the Steam installation and hashed executable/system/representative resources. Recorded OS, CPU, GPU/driver/display and input enumeration.
- Read owner-reported symptoms and historical Windows crash events, then reproduced startup failure through Steam.
- Checked PATH, uninstall registrations and common portable-tool locations before additions. Working bundled Python 3.12.14, Git 2.53.0.windows.3, Node 24.19.0 and a game-bundled 64-bit JDK 21.0.9 were found. Rust, MSVC/SDK, Ghidra and native debuggers/tracers were not found in the inspected locations.
- Added official signed portable ProcDump 12.01 and project-local Capstone 5.0.6. Captured mini/full crash dumps and independently inspected PE and dump structures.
- Confirmed F001–F010. Ran the affinity, refresh and windowed probes separately; all failed at the same RVA. An early affinity attempt raced a transient process and is not counted as a successful test.
- Restored user settings byte-for-byte; no original executable, DLL, system configuration or asset was intentionally changed. Diagnostic scripts passed Python compilation and PowerShell parsing checks.
- Details, exact run times, evidence paths and limitations: [Phase 0 report](docs/PHASE0.md).
