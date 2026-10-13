# Active deformation through the vehicle update

13 October 2026, F253. The Rust vehicle synchronization and registered car job now accept bounded supplied deformation grids. The original settling branch computes four corner scores from current and rest node positions. This removes the earlier blanket rejection of active grids from the car update; it does not yet initialize the grid from purchased resources or deform the visible Lacetti.

## Recovered behavior

The complete original `9BD7F0` advances the ten-entry strain ring and clears its new entry before calling `9A0A70`. A maximum strictly above 3 sets body byte `C7B1`. A maximum strictly below 0.5 with that byte already nonzero clears the byte and recomputes body `C7B4..C7C0`. Equality at either threshold retains the relevant state. Clearing the next ring entry can remove the only above-threshold value.

The reached traversal visits Z, then X, then Y. Dimensions are body `C714/C718/C71C`; node index is `(z*ny+y)*nx+x`. Nodes begin at body `4880`, stride 80 bytes. The first two vec4s are current and rest positions. All four displacement lanes, including W, participate. SIMD grouping is `(w*w+z*z)+(y*y+x*x)`. Bucket selection is `floor(2*x/nx)+2*floor(2*z/nz)`, including odd dimensions. Each bucket retains its strict maximum squared length; original `SQRTSS` produces the four final scores. Nodes and every opaque trailing node byte remain unchanged.

The bounded Rust API admits either all-zero dimensions or dimensions 2..16 with product at most 400. When the settling branch consumes nodes, each current/rest component must be finite and have absolute value at most one million. These are explicit admission bounds, not a claim that the original rejects malformed dimensions or coordinates. Skipped node data remains opaque, including nonfinite words. A late invalid node rejects the staged update without modifying the input.

The source was checked against aligned instructions rather than accepting the earlier inferred pseudocode's SIMD expression. Private evidence is `local/behavior/active-grid-sync-aligned-20261013.json`; original inferred function evidence is `local/behavior/20261008T1311500495038Z/9a0a70.c.txt`.

## Complete original comparisons

Each Rust profile independently matches original and repeated original execution:

| Boundary | Calls per profile | Compared bytes per profile |
| --- | ---: | ---: |
| Whole `9BD7F0` synchronization | 616 | 33,808,544 |
| Whole registered `A0A290` car job | 436 | 59,395,408 owner bytes; 60,211,600 observable bytes |

The active suite contains 144 authored grids, four skipped-branch opaque-node controls, and 96 combinations of two retained paused snapshots, twelve cars and four authored strain gates. Both 360-node and 390-node retained grids are included. Retained node pools are observations from earlier reference sessions; their incoming ring histories here are authored. This is not a new live capture or a source-only grid constructor.

Both boundaries carry sixteen independently owned branch states through twelve subsequent calls. Synchronization also repeats 180 legacy zero-grid cases. An independent Python oracle decodes flat node indices, assigns the corner halves and checks the four score words, ring state and complete unchanged node pool.

Whole-job requests reuse 32 retained independently initialized source-driver fixtures from the earlier query-car-job proof. Only node storage, dimensions and authored strain state are replaced. The job executes original synchronization, complete selected driver, notified publication and owned completion; original helpers are intact. Comparison includes the complete car and physical owner, query records and other observable protocol fields. Expired temporary view/solver storage is excluded under the existing job contract.

Seventy-six rejection/preservation guards and fourteen job-framing guards pass per profile. New cases cover mixed/negative/oversized dimensions, excessive node count, nonfinite current/rest coordinates and out-of-bound components. Native failures create no surviving output; existing output files are preserved. The audit independently reconstructs initial requests and every carried input, rechecks complete results, and verifies all 529 evidence files and nine current source/executable identities per profile.

The opt-in probes share admission code with the legacy adapters but produce separately named binaries. The earlier zero-grid native binaries were preserved. Original code remains unchanged; the original game entry point is not run and no live process is modified. The synchronization adapter restores its writable lazy-data page; the car-job adapter restores its selected original data pages and owned fixture links under its existing contract. This is not a new full-image mutation audit or a full original world-scheduler comparison.

## Driving integration and recovery

An ignored purchased-source test runs two independently initialized Lacetti/Donington runtimes through the same 600-tick powered turn, with physical body contacts and registered damage events enabled. One runtime additionally receives an explicitly authored 2x2x2 grid, with W displacements selecting four distinct corner scores. Both generate **176 body contacts**. Every compared physical state, frame, solved contact record, damage RNG, query-cache flag and runtime counter agrees; the complete car owner differs only in the explicitly installed grid and expected score words. No new deformation force or visible mesh change is inferred from this equality.

A reached invalid rest component then rejects a tick while preserving the checked complete owner/frame/contact buffers, RNG and runtime metadata. Reset removes the supplied grid, returns to the source-created initial car and retains event configuration. The two-runtime test uses an explicit 16 MiB worker stack; this changes no game or app stack setting.

All **190 regular library tests**, **seven purchased-source release driving tests**, formatting and all-target warnings-denied Clippy pass. The active-grid source test also passes in debug. The clean-source prototype refresh passes the same seven source checks and all three Bevy checks; its build and render evidence follow.

## Evidence, versions and failures

All binary/node/capture evidence remains ignored and private:

- `local/behavior/active-grid-sync-debug-20261013-final-02/validation.json`, SHA-256 `9744572e7f2100b0f5666a290380df3439462a9ef6307f875c13c3a685166409`.
- `local/behavior/active-grid-sync-release-20261013-final-01/validation.json`, SHA-256 `6b96b44fa8b144663d14e08387519f22740715c760c678480a9b2fe2dfca7903`.
- `local/behavior/active-grid-sync-audit-20261013-final-01.json`, SHA-256 `6de64f5dcbb7d8aad2a1ab63fbd2099aba02f296ebf097beef676b677ab0a963`.
- `local/behavior/active-grid-library-tests-20261013-final-01.txt`, `active-grid-source-debug-20261013-draft-02.txt`, `active-grid-source-release-20261013-final-01.txt`, `active-grid-clippy-20261013-final-01.txt`.

Python 3.12.14, Rust/Cargo 1.99.0, MSVC 14.44.35207, Developer Prompt 17.14.41, Windows SDK 10.0.26100.0 and Capstone 5.0.6 were reused. Native compilation remains strict SSE2, x87 53-bit nearest, with warnings treated as errors. No installation was needed. The app remains Bevy 0.19.1.

Native sync probe SHA-256 `9665ac6f9efcb1c8e866c54a3b02d6f4de1d0f018dee0f2b8e2b4a0cc88846ac`; native job probe `1c02d5d1eac210501f0750aa1c2d9e6646453d776ae1e73e2b6d6cbd8c4b13d8`. Rust sync debug/release are `affbf51fea77fb8dd88c0f0f911e35628294689dcbb5d3dfebcef27867bcebfc` / `074def84f45a50ca9082320be2ad2cfd5ec5c4e87162517e458215a304c55a56`; job diagnostic debug/release are `108a62189a3cb62e0797e07f3d660a6eb9ff7b3369cc8e934a50f2590fe275c7` / `38322d0b0c7178f4bc29c398e1f661f903dc10323d8b3d853dee1450f6ddfb75`.

The first whole-job draft omitted physical storage when packing the new request. Original query guard 35 rejected it before any job completion. The corrected request includes the physical suffix and asserts its exact length; final comparisons pass. The first source regression prepared body geometry only for the baseline runtime, giving it twenty pool slots while the second runtime still had four. Preparing both identically fixed this fixture mismatch. No physics admission rule was relaxed. Failed reports/logs remain under the draft paths; no native execution fault or numerical mismatch occurred.

## Playable refresh

Clean build **f428d0a23ce8** (full revision `f428d0a23ce860734dba2bfebf22f6fbb35f6f67`) was rebuilt at `2026-10-13T09:51:37.7607818Z`, release SHA-256 `e1e50a3aa28fd82890d50ca8ba33aea269cca93426c342223618d066aecb26be`. Both existing launchers use this executable. No active owner session was replaced. The manifest, seven source tests, three Bevy tests and build log are private under `local/playtest/build-20261013T0950549337998Z/`.

The headless 600-tick powered turn with body contacts and events completes normally: 132 sampled records, no recorded simulation failure, 176 total body contacts in the source check, final damage RNG `2a73550a`. The new render was inspected and shows the Lacetti beside the Donington barrier. This run uses normal source-created zero-grid startup; the separately supplied-grid regression above covers the new active branch. No visible deformation or new manual lap is claimed. The known gamepad `80070057` warning recurs without preventing completion.

Private render/telemetry: `local/playtest/active-grid-f428d0a23ce8-20261013.{png,txt,json}`. Log SHA-256 `dd0c75199946b3a48784f2fa73e0dea16ada242a3c2d382e0c992e7fbf38901c`; image SHA-256 `c0dd570f22b35fec81188222e35cc0cfc54570ace0646d96ab4c621403e09981`. Original Steam executable and all 130 traced resources remain unchanged at `2026-10-13T09:54:21.095505Z`, recorded in `local/scene/preservation-20261013-active-grid-sync.json`. The final current-file audit also passes again after rebuilding the app (`active-grid-sync-audit-20261013-final-02.json`).

Several guessed filenames failed to read during initial navigation and were replaced with inventory-based reads. A sandbox CIM process query was unavailable; the existing ordinary process inventory subsequently worked. This was not an automatic approval rejection. Mixed header line endings triggered whitespace checks and were normalized without changing C semantics. No desktop input, original Steam change or user settings/save change was needed.

## Next integration boundary

Active-grid synchronization is now available through the registered job and selected physical driving world. The next work is source-derived grid construction (`9A2A70`/`98A7D0`, bounds, edges and settings), followed by the registered contact-to-deformation callback and dirty wheel/setup rebuild. The existing verified `9EA6E0` supplied-contact pipeline has a separate CRT random seed and must not reuse the body-damage RNG by assumption. Visible mesh mapping follows authoritative initialized grid state. Dirty `C7F2`, terminal notifications and their dependent component ownership remain guarded. No owner capture or input is required for the next offline investigation.
