# Runtime limits audit

Status: research and documentation only. No runtime code was changed. The audit covers every place where the running prototype can stop, panic or silently misbehave because of a hard limit or an unexpected state. Each one is classified against the original GRID.

- **Revision audited:** `c8ea239` (main, 15 October 2026). F317 (`edfb99c`, the parallel world-bounds fix) landed while this audit ran; section 2.1 records what it changed. Line numbers refer to `c8ea239`; F317 shifts some lines in `multi.rs` and `dynamic_props.rs`.
- **Method:**
  - Five read-only static passes over `crates/grid-world`, the `crates/grid-tools` library modules that grid-app uses, and `crates/grid-app`.
  - Static analysis of the hash-pinned `local/compatibility/grid-unpacked.exe` (SHA-256 `aac46a64…4514`) through the private `local/tyre-smoke/scripts/pe.py` (capstone; Codex runtime Python 3.12.14).
  - Existing Ghidra 12.1.4 text outputs and the existing read-only live world snapshot `local/world-settings/paused01.json`.
  - A route-extent scan of all 92 shipped `progress_track.xml` files with the project's `grid-binxml` tool. It was read-only on the Steam install and left the install unchanged.
- **Exclusions:** test modules and the offline tool binaries in `grid-tools/src/bin`.
- **Evidence:** private, under `local/runtime-limits-audit/` (disassembly snippets for AD8480, AD8BA0, A74F00, A749A0, 5F5D00, B24EF0, B78530, AD9300, 7D4940, 7D6A30, 8A98B0, 890A10, 89F1B0, 8981B0, 892480 and B18E70; route extents in `route-extents.tsv`).

Tags: **[V]** verified statically (this audit or a cited doc), **[I]** inferred, **[Live]** existing read-only live capture, **AUTHORED** a port choice with no original counterpart, **port guard** a validation/harness bound chosen during porting research (not an original value). Likelihood H/M/L/None is for a 12-car race on Donington launched by `Play GRID Race.cmd` unless stated; "route" means it depends on the `--route` circuit.

## 1. How a failure reaches the player

- **Any `Err` from the per-tick call pauses the whole race.** The per-tick calls are `MultiRuntime::tick`, `Runtime::tick`, `tick_with_body_geometry` and `tick_with_deforming_body_geometry`. On an `Err`, `grid-app/src/main.rs:2220` sets `driving.failure` ("tick N: …; R resets"), logs `driving paused`, and stops FixedUpdate driving for every car.
  - The HUD shows only "Simulation paused after an error. R resets."
  - The real message is only in the window title and the launcher log.
- **Two more paths set the same failure:**
  - a time-trial bookkeeping error (`main.rs:2229`, "R restarts"; physics has already advanced one tick);
  - a player body-dent presentation error (`main.rs:2467`). The same error for an opponent is ignored (`main.rs:2511`).
- **No per-car isolation.** `MultiRuntime::tick` propagates every stage with `?`, so one car's or one prop piece's error stops all 12 cars.
  - The tick is staged and deterministic, so the failing tick reproduces on every retry.
- **R is a full restart.** All cars return to the grid; pieces, AI, standings and the trial are reset, and all race progress is lost. A capacity cause such as prop pieces returns as soon as the same state recurs.
- **Panics abort the app.** No reachable panic was found on the per-tick path. Every `unwrap`, `expect` or index site is preceded by a covering validation. The exceptions are two invariant `expect`s (`deformation_world.rs:141`, `deforming_group_frame.rs:82`) and the route-data panic `ai_driver.rs:1941` (R14).

Counts outside tests and offline binaries:

| Crate | `Err(` | `unwrap`/`expect` | `as` integer casts |
|---|---|---|---|
| grid-world | 451 | 30 | 270 |
| grid-tools | 199 | 43 | 192 |
| grid-app | 71 | 24 | 46 |

No reachable integer wrap was found:
- tick counters are `u64`;
- the lap clock is `f64` computed from ticks;
- f32 phases and angles wrap every tick;
- lap counters use `checked_add`;
- indices are validated before every cast.

## 2. What the original does at its limits

The verdicts are from static analysis of grid.exe. Only addresses and one-line interpretations are given here.

| Port limit | Port location | Original value | Original address | Original behaviour when full | Confidence |
|---|---|---|---|---|---|
| 16 bodies per world | `contact_groups.rs:75`; also per group in `impact_frame.rs:91`, `impact_step.rs:76`, `contact_rows.rs:164` | **No per-world solver cap.** Per island (contact group): **200 bodies, 1000 constraints, 3000 DOF rows**, set by the game; engine defaults 1000/1000/6000. Global rigid-body pool 1000. | config write 5F5F4B..5F5F57; world+788 → AD8480 → solve manager B7A9E0/B77360 (+68/+6C/+70); check B7B850 | The island is **skipped that step**: status 1 before preparation, no integration, each contact's virtual +0C called. If the manager focus flag +11C is set, B24EF0 first freezes the bodies farthest from the focus point in batches of 20 until the island fits. | [V] values (code + [Live] world+788 = 200/1000/3000); [I] whether +11C is on in a race |
| Island layout in a race | — | 12 islands, largest 1 body, at the start grid; island-assignment capacity 1000 | world+58, +5C, +7FC | Islands beyond 1000 are not assigned that step | [Live] snapshot; [V] AD6C40 |
| No 16-entry array or bitmask | — | Per-island arrays are sized from the manager limits (`cap<<5`, `cap<<6`, `cap*0xC0`) | B7A9E0 | — | [V] |
| `MAX_WORLD_CONTACTS` 256 | `lib.rs:421` | 1000 per island; global maxContacts 10000, maxBroadPhaseCollisions 10000 | 5F5E6D.., mgr+6C | Per island as above. Global pools: consumer not traced. | [V] values |
| `MAX_SOLVER_ROWS` 1024 | `lib.rs:423` | 3000 DOF rows per island | mgr+70, B7ACC0 | B7ACC0 returns false, B7B850 returns status 1, cleanup is skipped | [V] |
| Per-owner contact selection 80 | `impact_generation.rs:119`, `contact_reduction.rs:11` | 80 | push 0x50 at B787BE/B788D4, B775D0 | Contacts beyond 80 are dropped | [V] |
| Response pairs 240 | `response_consumption.rs:48` | 240 | RESPONSE_CONSUMPTION.md | A new contribution at capacity is discarded | [V] (doc) |
| Contacts per collision 16 | `contact_manifold.rs:391`, `packed_contacts.rs:209` | maxContactsPerCollision 20 | store 5F5EA3 | Contacts beyond the cap are dropped | [V] value, [I] consumer |
| Shapes 513 / 512 | `contact_groups.rs:79`, `packed_world.rs:31` | geoms pool 11500, fixed objects 10000 | A749A0, AD9220 | The allocation returns NULL (refused) | [V] |
| Global pools | — | rigid bodies 1000, geoms 11500, fixed 10000, constraints 1000 | AD9300/AD9220/AD9260 | Fixed free lists. They refuse (NULL) and never grow. | [V] |
| Fixed waitable lists | collision dispatch 32 | 32/64 | CF5CA8 "Waitable list is full" → A67190 → A94910 | The item is dropped and error 0x21 recorded; no crash | [V] |
| Callback owners 64 | `owner_collection.rs:17` | 32 entries on the caller's stack | B78530, B77770 | No capacity check, so a 33rd owner would overrun the stack (cars only). The port's 64 is looser. | [V] |
| Vehicle force solver 40 / 8 | `force_equations.rs:7-8` | 40 variables / 40 rows / 8 terms | 9DF330 family | not traced | [V] value |
| Candidate grid 25 | `candidate_grid.rs` | 25 | 96B7F0 | Candidates beyond 25 are dropped | [V] (doc) |
| Deformation | `deformation_impact.rs:78-79` etc. | Fixed 400-node pool. The 1200 edges, 512 records and 0x2000 settings are port guards; 1200 matches the descriptor layout. | 9A26F0 | — | [V] (doc) |
| Field size 12 | `multi.rs:116`, `main.rs:1186` | **20**: race manager 20 × 0xC08, car manager 20 job slots, 20 grid slots. The live race runs 12. | 911C90, A02B70 | — | [V] (doc) |
| Breakable props: unbounded pieces, no sleep | `dynamic_props.rs:858-931` | ENTITYMANAGER instance limits default **1000** each, with no installed plain-file override. Pieces use rbDefault turnOffTime 1, so they **sleep after 1 s**. No timer or distance despawn. | 7BA850; 7D4940; 7D6A30; 7B5FC0/7B6080; 7D5180 | When a break does not fit, the **farthest other broken entity** (from the focus point) is reset if that makes room; otherwise **the break is skipped and the prop stays intact**. | [V] mechanism, [I] that 1000 is not overridden in archives |
| Wheel debris 12 000 particles | `wheel_debris.rs:20` | 12000 is `effects/config.xml`, which the exe never reads. Real cap: **80 particle systems** per manager. | 8A98B0 (`cmp [esi+16C],0x50`, error 0x37); emitter buffers 88DC70 | A new system is refused (error 0x37); an emitter without a free buffer is not created. No per-emitter particle cap was found. | [V] 80/refuse; unknown buffer counts |
| Skid marks 240 × 32 | `skid_marks.rs:12-15` | 240 × 32 | 73E430 (0xF0) | Free list, then retiring strips, then **steal the active strip farthest from the camera** | [V] |
| Tyre smoke 80 systems | grid-tools `tyre_smoke.rs` | 80 system slots, 10 births per system | 8A98B0, 8A99C0 | refuse | [V] |
| Sector gates 16, lap table 396, grid population 512, track entries 512 | various | no original analogue | — | — | port-only |

The headline result is that the port's 16-body and 256-contact world bounds are far below the original's. The original's bounds apply per island, not per world, and when one is exceeded the original keeps running: it skips or freezes the overflowing island, or refuses or recycles a prop break. It never stops the race.

### 2.1 Status after F317 (`edfb99c`)

F317 changed these limits:
- `grid_world::MAX_WORLD_BODIES` is 1000. It replaces the world 16-body bound and the per-group 16/64 guards in `contact_groups`, `contact_rows`, `impact_frame`, `impact_step`, `physical_tick`, `main_contact_response`, `physical_response` and `impact_correction`.
- `MAX_WORLD_SHAPES` is 513 + 1000.
- `MAX_WORLD_CONTACTS` is 10000, and `MAX_SOLVER_ROWS` is 4 × that.
- `PropWorld::apply` refuses a break that would exceed 1000 bodies.
- Car contact pools keep `clamp(24n, 64, 256)` as an explicit AUTHORED budget; pieces add 12 slots each beyond it.

Effect on this audit:
- **R01 and R04 are resolved for pause purposes.** 312 ported pieces plus 12 cars cannot reach 1000.
- **R05 remains:** 208 car geometry slots at 12 cars.
- **R06 remains:** pieces still never sleep or despawn. *F342:* a piece more than 100 m (AUTHORED) below the lowest road chunk is held (`PropWorld::hold_lost`), so a lost piece can no longer reach the 1e5 domain and pause the world; the tunnelling itself is open (DYNAMIC_PROPS_RESEARCH.md "Soak fixes (F342)").
- The refusal is a deviation from the original's farthest-first restore, which F317 recorded but did not port.

**Correction to F317's provenance comment.** The `lib.rs` comment says the engine defaults apply because no shipped file carries a PHYSICSENGINE node. That is true of the files, but the game code itself writes the per-island values:
- `0x5F5F4B mov [esp+20],0xC8` (maxBodiesInIsland 200);
- `0x5F5F53 mov [esp+24],ebx` (maxConstraintsInIsland; 1000 in the live snapshot);
- `0x5F5F57 mov [esp+28],0xBB8` (maxDOFConstraintsInIsland 3000);
- `0x5F5E28 mov [esp+F4],0x2CEC` (maxGeoms 11500).

The existing read-only snapshot `local/world-settings/paused01.json` reads world+788 = 200/1000/3000. [V] static, re-checked in this audit with `pe.py dis 0x5F5E10`; [Live] snapshot.

So the original's per-island limit is **200 bodies / 1000 contacts / 3000 rows**, and an island over it is skipped for that step (B7B850). The port's 1000-body per-group guard is looser than the original. That is safe for pausing, but a fidelity difference only for islands of more than 200 bodies, which needs about 190 touching pieces in one island.

### Route extents (observation)

The port has `abs > 1000` guards on contact points and car poses in `collision_damage.rs:36-57` and `body_impact.rs:47-81`. The racing line of **34 of the 92 shipped routes** leaves ±1000 m in x or z:

| Track | Max \|x\|,\|z\| on the line | Routes over 1000 m |
|---|---|---|
| Le Mans | 2928 | 2 / 2 |
| Haruna | 2416 | 10 / 10 |
| Spa-Francorchamps | 1463 | 4 / 4 |
| Washington | 1324 | 2 / 10 |
| Okutama | 1133 | 6 / 6 |
| San Francisco | 1037 | 6 / 6 |
| Detroit | 1035 | 4 / 10 |

Donington (795 m), Nürburgring (851 m), Istanbul, Jarama, Milan, Shibuya, Yokohama and Long Beach stay inside. All routes stay well inside the 10 km manifold bound (`contact_manifold.rs:404-423`) and the 100 km lap-timing bound.

## 3. Ranked risks

Score = likelihood (H 3, M 2, L 1) × impact. Impact: race freeze or crash 3, wrong physics with no stop 2, cosmetic or audio 1. Rows marked "12–20 cars" can be hit by a normal 12–20 car race, as the 16-body bound already was.

| Rank | Risk | Inventory IDs | Likelihood | Impact | Score | Original | 12–20 cars |
|---|---|---|---|---|---|---|---|
| R01 | **[Resolved by F317 for pausing; see 2.1]** **Prop pieces exceed the 16-body world bound.** With 12 cars only 4 pieces fit, so the 3rd board, the 5th cushion or any tyre stack (12 tyres) pauses on the tick after the break. A solo run fails at 15 pieces. Pieces never sleep or despawn. | A01, C01, C02, D01, D07 | H (observed) | freeze, recurs after R | 9 | No such cap: 200 bodies per island, 1000 global; pieces sleep; break refused or farthest broken entity recycled | **yes** |
| R02 | **No failure containment.** One car's or piece's `Err` from any guard freezes all cars, and R discards the whole race. This multiplies every freeze row below. | B amplifier, D01, D07, D09 | — | multiplier | — | Island skipped for one step; the race continues | **yes** |
| R03 | **[Resolved by F322: domain removed; 9E6660/999C00/9EF360 have none, see §5 S4]** **Collision damage and body impact reject points or poses beyond ±1000 m.** The first impact past 1 km freezes the race. | B01, B02 | H on 34 routes; None on Donington and Nürburgring | freeze | 9 (route) | No evidence of such a limit (9E6660/999C00/9EF360 to confirm) | **yes** on those routes |
| R04 | **[Resolved by F317: all now 1000 / 1513 shapes]** **Caps queued behind R01.** Per-group 16 bodies (`impact_frame`, `impact_step`, `contact_rows`), 513 shapes, 64 bodies per group (`main_contact_response`, `physical_response`, `impact_correction`). A pile-up group with a tyre stack passes 16 at once. | A15, A17, A19, A21–A23, A01 (shapes) | H once R01 is raised | freeze | 9 | 200 bodies per island; refuse at allocation | **yes** |
| R05 | **Silent contact starvation at 12 cars.** The pool is clamped to 256 (24 × 12 = 288), leaving 208 geometry slots filled in car-index order: later cars lose wall contacts, then all car-car contacts, then piece ground contacts. Nothing is logged. | C04–C07, A50 | M (pile-ups) | wrong physics (tunnelling, sinking pieces) | 4 | 1000 contacts per island, 80 per owner, 20 per collision | **yes** |
| R06 | **Falling-piece time bomb.** A piece that sinks or tunnels free-falls forever. About 150 s later \|pose\| > 1e5 fails `Mesh::world_bounds` on every tick. | A25 (fed by R05) | M | freeze, recurs | 6 | Pieces sleep after 1 s; original bound unknown (B44E80/B444D0) | **yes** |
| R07 | **About 120 finite and ±1e6 guards** in the vehicle, suspension, wheel, solver and deformation code. Any numeric blow-up in a pile-up becomes a whole-race pause. | B03, A03, A16, A18, A20–A22, A26, A43 | M | freeze | 6 | The original propagates the value and keeps running | **yes** |
| R08 | **Stuck AI never leaves the standings.** An AI that latches "Retire" brakes forever, so the race never ends by itself and the player must press F. | C27 | M | wrong race result | 4 | 0x77E3B0 retire timer retires the car | **yes** |
| R09 | **Unbounded per-tick logging.** A report of about 1–1.5 KB is written on every tick with any contact anywhere in the field. That is hundreds of MB per hour into the OneDrive-synced `local/playtest` log, and a failing stdout write panics `println!`. | D29, D21 | M–H (long sessions) | disk growth; crash on write failure | 4–6 | n/a (diagnostics) | **yes** |
| R10 | **`asin` domain on a near-exactly-sideways slide.** An rsqrt-normalized dot product can reach 1.0000001, and the code errors before `asin`. | B04 (`vehicle_step.rs:87`) | L–M (spins are common; needs Lacetti 0x122c/0x1230 > 0) | freeze | 3–6 | x87 `asin` would give NaN and not abort (A06A90 to confirm) | **yes** |
| R11 | **A car (or AI car) out of the world.** \|position\| > 100 km in lap timing (12–17 min of free fall) pauses everyone. There is no kill plane or respawn. | B06, C21, D04 | L–M | freeze | 3–6 | The original has a respawn path (0xde4/0xdfc bytes, not ported) | **yes** |
| R12 | **Manual gearbox presses are lost or doubled.** `just_pressed` is read inside FixedUpdate (static finding; confirm in a windowed run). | D34 | H for manual users | control | 3 | n/a | yes |
| R13 | **A 20-car field cannot start.** `--opponents` is limited to 1..11 and `MultiRuntime` to 1..12, and a 20-car world would trip R01 on tick 1. Liveries also repeat after 10 (`k % 10`). | C03, B46, D10, D14 | None today | feature gap | — | 20 cars | 20 |
| R14 | **Route data traps:** `expect("brake lines")` panics if a route has no brake lines; unsupported breakables fail startup; a chunk with more than 8 materials fails every tick; a chunk with more than 1024 undecoded vertices or a leaf with more than 256 triangles fails deterministically; more than 512 gates, 16 splits or 512 population entries fail at load. | C24, C11, C14, C35, A30, A31, B42, B44 | None on Donington; L–M per route | panic / freeze / load fail | 3–6 (route) | — | route |
| R15 | **Wheel ray hits a material with no mechanics** (WARN, RESE, RESD, UNRE, FBND). The tick fails. Chunk usage on Donington is not yet scanned. | C13, B26 | L–M (unverified) | freeze | 3–6 | Mechanics are optional in the original [I] | yes |
| R16 | **Bookkeeping or presentation errors escalate to a pause:** a zero-length sector; the player pushed across the finish line while held on the grid; a player dent projection error. | B08, D02, D03 | L | freeze | 3 | n/a | yes |
| R17 | **Exact-zero degeneracies:** surface drag with exactly zero point velocity (stationary damaged car or gravel); singular finalizer, coupling or travel-stop projections (car on its side); a degenerate deformed-mesh triangle in a heavy crush. | A42, B24, B27, A33 | L (A33 L–M) | freeze | 3 | Original divides and propagates inf/NaN [I] | yes |
| R18 | **Pose correction above 64 rad or rotation above 4000 rad/s** with light pieces wedged under cars. | A23, A24, B40 | L | freeze | 3 | port diagnostic bound | yes |
| R19 | **Force-solver event or limit iterations exceed 64.** | B09 | L | freeze | 3 | authored, not measured | yes |
| R20 | **Frame-rate collapse** when a tick takes more than 16 ms (FixedUpdate catch-up). Pile-ups with pieces, dent reprojection and effect mesh rebuilds can cause it, and it looks like a freeze. | D32, D33 | M | perceived freeze | 2–4 | n/a | yes |
| R21 | **[Resolved by F434: engines and tyres stay audible until the race is frozen]** **All engine audio is muted after the player finishes,** including AI cars still racing. | D28 | H | audio | 3 | n/a | yes |
| R22 | **Debris pool full: the newest particles are dropped.** Debris and smoke keep two separate 12 000 pools, sized from a config value the exe never reads. | D23 | M | cosmetic | 2 | 80 systems, new ones refused | yes |
| R23 | **Tyre smoke uses 80 system slots:** exactly full at 20 cars × 4 wheels. Whether the original's debris systems share this pool is not established. | C42, D25 | L (20 cars) | cosmetic | 1 | 80, refuse [V] | 20 |
| R24 | **Race progress uses a global nearest-point projection,** so a car far off the line can jump in progress and an AI can be marked finished early. | C28 | L | wrong result | 2 | — | yes |
| R25 | **[Resolved by F435: 0x73F330 ported, see D24]** **Skid-strip stealing takes the farthest finished strip;** the original takes the farthest active one. | D24 | — | cosmetic divergence | 1 | 73E430 [V] | — |
| Latent | **Unsupported original states** that would freeze at once when ported: respawn, nitro, manual clutch, handling notification receivers, pending deformation rebuild or observers. | B10, B11, B13, B25, B29, B30 | None today; H when ported | freeze | — | supported by the original | — |

Checked and supported, with no guard firing: car on its roof or side, airborne, reverse gear, neutral, handbrake, stalled engine, terminal wheel damage (no longer a pause since the 14 October fix), deflated tyres, saturated body damage, reverse driving across the finish line, and long races (u64 ticks, f64 lap clock, wrapped phases).

## 4. Inventory

IDs match the per-area work tables. Rows were grouped when several guards share one condition. "Pause" means `driving.failure` (section 1).

### 4.1 Shared world: contacts, impact and solver (grid-world)

| ID | File:line | Condition / message | On fire | Per tick | Reachability | Bound provenance |
|---|---|---|---|---|---|---|
| A01 | `contact_groups.rs:75-81` (checked 3× per tick via `grouped_physical_frame.rs:190,269`, `contact_groups.rs:377`) | bodies ∉ 1..=16, contacts > 256, selector > 1, active order, shapes > 513: "invalid contact-group counts or selector" | pause | yes | **H** (R01). Contacts: None (pool clamped). Shapes: ~247 + 12 + pieces, about 255 pieces needed | port contract of AD6C40 (CONTACT_GROUPS.md "accepts 1..16 owned bodies"); original 200/island (§2) |
| A02 | `contact_groups.rs:84-166` | permutation, ownership, carried-link and dirty-chain structure | pause | yes | L (chain built by `build` and `PropWorld::apply`) | port structural contract |
| A03 | `contact_groups.rs:168-188` | invalid contact record or non-finite point/normal | pause | yes | L (NaN normal from a zero-area sheet triangle, A29) | port guard |
| A04 | `contact_groups.rs:423` | non-finite relative point | pause | yes | None/L | port guard |
| A05 | `contact_groups.rs:132,401,463`; `grouped_physical_frame.rs:393` | unwrap/index | panic | yes | None (validated) | — |
| A06 | `physical_world.rs:115-145` | completed-world, carried chain and head checks; contacts > 256 | pause | yes | None/L | 256 port guard (`lib.rs:421`) |
| A07 | `physical_world.rs:197,216` | step count, reproduced world | Err | no (diagnostic) | None | diagnostic |
| A08 | `grouped_physical_frame.rs:102-239` | external prefix and provider capacity | pause | yes | None (providers truncate silently, see A50) | port guard |
| A09 | `grouped_physical_frame.rs:192`, `pose.rs:69`, `dynamics.rs:16` | delta ∉ (0, 0.25] | pause | yes | None (DELTA 0.016) | diagnostic precondition |
| A10 | `grouped_physical_frame.rs:270` | registrations ≠ bodies | pause | yes | None | port |
| A11 | `grouped_physical_frame.rs:282-314` | manager: rows > 1024, capacity < 4 × contacts, iterations, materials > 8, **sleep duration ≠ MAX**, flags | pause | yes | L. **Blocks original piece sleeping** (turnOffTime 1) | GROUPED_PHYSICAL_FRAME.md selected scope; 1024 port guard |
| A12 | `grouped_physical_frame.rs:483` | group status ≠ 0 (asleep or capacity) → "unsupported grouped physical sleep or capacity status" | pause | yes | None while A11 holds | the original returns status 1 and **skips the island** (B7B850); the port rejects it |
| A13 | `grouped_physical_frame.rs:443` | `expect("registered seed")` | panic | yes | None | invariant |
| A14 | `grouped_physical_frame.rs:364,520` | properties indexed by group-local index | silent | yes | None in effect (overwritten by `impact_frame`) | port detail |
| A15 | `impact_frame.rs:83-117` | group bodies 0 or > 16, iterations > 32, non-finite | pause | yes | masked by A01; **H once A01 is raised** | IMPACT_STEP.md harness limits (B7B850/B7ACC0 oracle), not grid.exe |
| A16 | `impact_frame.rs:218,423` | non-finite prepared body or output | pause | yes | L | port finite guard |
| A17 | `impact_step.rs:74-131` | contacts > 256, bodies > 16, materials > 8, rows > 1024 | pause | yes | as A15 | harness limits |
| A18 | `impact_step.rs:325` | non-finite output | pause | yes | L | port |
| A19 | `contact_rows.rs:162-200` | contacts > 256, bodies > 16, materials > 8 | pause | yes | as A15 | CONTACT_ROWS.md adapter bounds |
| A20 | `contact_rows.rs:78-427` | subnormal vector, material mixture, degenerate tangent, non-finite row | pause | yes | None/L | port |
| A21 | `main_contact_response.rs:71-151` | rows > 1024, **bodies > 64**, iterations > 32; singular normalization | pause | yes | masked by A15; next cap after it | MAIN_CONTACT_RESPONSE.md adapter bounds (original B791B0) |
| A22 | `physical_response.rs:65-241` | 64 bodies; singular or non-finite | pause | yes | L | PHYSICAL_RESPONSE.md port bounds |
| A23 | `impact_correction.rs:71`, `physical_pose.rs:78`, `contact_bias.rs:40,51` | poses > 64; angular correction > 64 rad | pause | yes | L (light piece wedged deep) | port diagnostic bound (original B7B5E0) |
| A24 | `pose.rs:36-109`, `physical_pose.rs:34-171` | rotation > 64 rad per step (ω > 4000 rad/s), ranges | pause | yes | L | port |
| A25 | `collision_mesh.rs:552-560,598` (`Mesh::world_bounds`) | **any pose element \|x\| > 1e5** or margin out of range: "invalid mesh bounds controls" | pause, every tick after | yes (every car and piece) | **M** (R06) | port domain guard (F322: documented in code). Original has none: B44E80 has no float compares, B444D0 none on coordinates, and the kind-7 bounds reduction B42050 seeds FLT_MAX accumulators with no domain check |
| A26 | `convex_distance.rs:408-840` | inputs \|x\| > 1e5, no convergence in 256 iterations, zero penetration normal | pause | yes | L | port; original AE8AD0/AE8DB0 |
| A27 | `contact_manifold.rs:404-423` and others | matrix or point \|x\| > 1e4 (10 km), existing > 16, clip > 20 | pause | yes | None (all routes < 3 km) | port guard. F322: matrix and fallback point raised from 1e4 to the file's 1e5 domain; AE2FA0/B45310 bound no coordinate (their compares are value/value and \|dot\| vs 0.99) |
| A28 | `packed_contacts.rs:210-218` | capacity > 16, candidates > 32768, material index ≥ 8 | pause | yes | None (callers clamp) | port scope |
| A29 | `packed_contacts.rs:250-266` | rsqrt of a zero-area sheet normal is not checked | silent NaN → A03/A27 | yes | L | port |
| A30 | `packed_candidates.rs:176-258` | undecoded chunk > 1024 vertices, depth > 64, visits > 1e6, structure | pause | yes | None on Donington; **route** | data sanity |
| A31 | `packed_world.rs`, `packed_query.rs`, `leaf_query.rs:37,80`, `packed_shape.rs`, `candidate_grid.rs`, `rectangle_cull.rs` | wheel-ray queries: shapes > 512, leaf > 256, zero-area hit normal, tiles ≠ 1024 | pause | yes (4 rays per car) | None on Donington; route | authored leaf and 25-shape bounds; data sanity |
| A32 | `candidate_grid.rs:78`, `packed_candidates.rs:235`, `packed_contacts.rs:288` | first 25 shapes per ray; 4096 candidates; 16 per pair | silent drop | yes | L | 25 [V] original (96B7F0); 16 port (original 20) |
| A33 | `collision_mesh.rs:68-802` (rebuild via `body_geometry.rs:147`) | deformed triangle area² ∉ [1e-20, 1e20], adjustment > 1e5 | pause | yes (deforming cars) | **L–M** (heavy crush or rollover) | port normal-range guard; original B44E80/9B8B70 unknown |
| A34 | `wheel_contacts.rs:24-166` | 4-slot pool, ownership, shapes 1..=512 | pause / silent drop | yes | None | WHEEL_PHYSICAL_HANDOFF.md 513-shape adapter |
| A35 | `vehicle_world.rs:35-384` | single-car world ownership; contacts > 4/64/256; shapes ≤ 513 | pause | yes (solo) | None/L (A01 fires first) | port |
| A36 | `vehicle_bridge.rs:77,101` | shapes > 64 | startup Err | no | None | port |
| A37 | `body_impact.rs:60-82` | impact ∉ [0, 1e9], parameter ∉ [0, 0.1], velocity > 1e4 | pause | yes | L | port domain (original A726B0) |
| A38 | `collision_damage.rs:19-139`, `damage_region.rs:47` | damage gains, thresholds, divisor | pause | yes | None for Lacetti data; other cars data-dependent | port (LOCALIZED_COLLISION_DAMAGE.md) |
| A39 | `serial_callback.rs`, `response_consumption.rs`, `contact_reduction.rs`, `contact_inverse.rs`, `contact_preparation.rs`, `impact_generation.rs`, `impact_body.rs`, `impact_response.rs` | deformation callback: generated or selected > 80, contacts > 512, pending > 240, i32 damage conversion, finiteness | pause | yes (deforming) | None (counts) / L (finite) | 80 and 240 original [V]; 512 port |
| A40 | `response_consumption.rs:118`, `impact_generation.rs:263-269` | pending pair dropped at 240 active; deformation records stop at 400 | silent | yes | L | 240 original; 400 = node pool |
| A41 | `force_equations.rs:37-250` | > 40 variables, > 8 terms, unclosed selection | pause | yes | None/L | original sizes [V] (doc) |
| A42 | `surface_drag.rs:39-118` | **coefficient > 0 with exactly zero point velocity**: "surface drag requires nonzero finite point velocity" | pause | yes | L (stationary damaged car on drag) | port guard; original 9DFC50 normalizes (inf/NaN) |
| A43 | `dynamics.rs`, `finalize.rs`, `wheel_*`, `grip_history.rs`, `tyre_grip.rs`, `contact_material.rs`, `contact_ray.rs`, `force_view.rs`, `steering_*`, `assisted_steering.rs`, `wheel_damage.rs`, `wheel_deformation*.rs`, `setup.rs` | singular projections, finite envelopes, contact normal alignment 1024 iterations (authored; the original loop has no bound) | pause | yes | L each | port finite guards |
| A44 | `physical_constructor.rs:41-207` | settings \|x\| > 1e6, tensor, determinant | pause | at a prop break | L (validated classes) | port (B11F40) |
| A45 | `spawn.rs:68,82` | "reference arithmetic requires SSE" | pause | yes | None | port |
| A46 | `lib.rs:108-189` (BVH, MAX_TRIANGLES 1e6) | — | Err/panic | no (bins) | None | diagnostic |
| A47 | `collision_dispatch.rs`, `owner_collection.rs`, `contact_collection.rs`, `physical_tick.rs`, `tyre_response`, `tyre_friction`, `wheel_rotation`, `steering_geometry.rs` | various | Err | no (diagnostic bins or startup) | None | diagnostic |
| A48 | casts in `contact_rows.rs:403`, `impact_generation.rs:280`, `contact_groups.rs:480` | wrap | silent | yes | None | — |
| A49 | `contact_material.rs:75`, `candidate_grid.rs:35` | float to i32 range | pause | yes | None | port |
| A50 | (grid-tools providers) | 208 geometry slots at 12 cars; car bodies and pairs are served before pieces | silent drop | yes | L–M | see C05–C07 |

### 4.2 Vehicle, damage, deformation and lap timing (grid-world)

| ID | File:line | Condition / message | On fire | Per tick | Reachability | Bound provenance |
|---|---|---|---|---|---|---|
| B01 | `collision_damage.rs:36-57` | contact point \|x\| > 1000: "invalid collision damage framing"; car pose \|x\| > 1000: "unsupported collision damage owner" | pause | yes (every body event and wheel-impact dispatch) | **H on 34 of 92 routes**; None on Donington and Nürburgring | port domain (BODY_IMPACT_RUNTIME.md "explicit limits"); original 9E6660/999C00. **F322: removed; finiteness only** |
| B02 | `body_impact.rs:47-81` | point or normal \|x\| > 1000; velocity > 1e4; impact ∉ [0, 1e9]: "unsupported body impact context" | pause | yes (with `--body-events`) | as B01 | port (original 9EF360). **F322: point bound removed (finiteness only); the normal, velocity and impact bounds stay** |
| B03 | about 120 sites (listed in the work table; see A43 and B-section files) | non-finite, \|x\| > 1e6 | pause | yes | **M** in pile-ups | port validation domain (VEHICLE_DRIVER_PREPARATION.md "explicit unsupported branches") |
| B04 | `vehicle_step.rs:84-88` | normalized `sideways` ∉ [-1, 1]: "invalid outer angle domain" | pause | yes | **L–M** | port; original A06A90 x87 `asin` |
| B05 | `vehicle_step.rs:79`, `vehicle_chassis.rs:24` | angle velocity, chassis normalization | pause | yes | None | port |
| B06 | `lap_timing.rs:229-240` | \|position\| > 100 000, duration ∉ (0, 0.25]: "invalid prototype lap interval" | pause | yes, every car | **L–M** (a fallen car after 12–17 min) | authored practice policy |
| B07 | `lap_timing.rs:297-355` | completed-lap checks; guarded unwraps | pause/panic | yes | None | port policy |
| B08 | `time_trial.rs:113-150` | consecutive ticks, lap start, finish without start, zero sector | pause ("R restarts") | yes (player) | L (pushed across the finish while held; zero-length sector) | authored |
| B09 | `vehicle_force.rs:509-522` | events > 64, limits > 64 | pause | yes | L | authored (VEHICLE_FORCE_PIPELINE.md); original 9DF540/9B5520 |
| B10 | `vehicle_driver_prepare.rs:55-80` | disabled-wheel bytes, nitro/respawn, manual clutch, gear > 9 | pause | yes | None today; **latent** | port scope (original A03670) |
| B11 | (removed in F346) | wheel detached (+22E4/+2278) | — | — | None: the detached-wheel branches are ported (WHEEL_DETACHMENT.md) | done |
| B12 | `vehicle_handling.rs:255-294` | composite handling branch, bounded fields | pause | yes | L | port scope (9F3E80) |
| B13 | `vehicle_handling.rs:207,312,365`, `vehicle_job.rs:22` | damage without RNG; notification receivers | pause | 312 yes (RNG always set) | None | port scope; the 14 October freeze path |
| B14 | `vehicle_handling.rs:316` | wear divisor ≤ 0 | pause | yes | None (data) | data |
| B15 | `vehicle_manager.rs:291-321` | ownership, activation, gear ≥ 10, speed > 1e10 | pause | yes | None | port scope (A02B70) |
| B16 | `vehicle_manager.rs:42-52` | wheel constraint | Err | no (diagnostic) | None | — |
| B17 | `vehicle_job.rs:113-130` | physical link words | pause | yes | L | port scope (A0A290) |
| B18 | `vehicle_sync.rs:29-121` | ring, dimensions, history | pause | yes | None | descriptor layout |
| B19 | `vehicle_sync.rs:44-63` | node \|x\| > 1e6 | pause | yes | None (B03 only) | port |
| B20 | `vehicle_controls.rs:199-492` | control input domain, gear table | pause | yes | None (reverse, neutral and handbrake supported) | port scope |
| B21 | `vehicle_propulsion.rs:191-444` | engine phase > 2e6, spin > 1e6 | pause | yes | None (phases wrap) | port |
| B22 | `vehicle_chassis.rs:79-86` | chassis phase table bound 60 | pause | yes | None for Lacetti; other car data | original table 60; rejection is a port guard |
| B23 | `vehicle_prepared_frame.rs:104`, `vehicle_contact_update.rs:24`, `vehicle_post_force.rs:29`, `vehicle_contact_force.rs:27` | roughness, 0x3d0 ≤ 0, structure | pause | yes | None (BP01 offset 0.125) | port |
| B24 | `surface_drag.rs:104-119` | same as A42 (damaged-wheel additive drag at rest) | pause | yes | L | port |
| B25 | `suspension_prepare.rs:89`, `suspension_vehicle.rs:12-77`, `vehicle_force.rs:598` | disabled suspension branches | pause | yes | None | port scope |
| B26 | `suspension_vehicle.rs:153`, `vehicle_step.rs:194` | contact token without mechanics | pause | yes | L; see C13 | invariant |
| B27 | `finalize.rs:56`, `wheel_coupling.rs:146`, `wheel_step.rs:110,154`, `wheel_deformation.rs:48` | exact-zero projection, degenerate deformation direction | pause | yes | L | port; original divides |
| B28 | `wheel_step.rs:58`, `finalize.rs:102` | travel bounds | pause | yes | None | port |
| B29 | `vehicle_publication.rs:27-43` | 0xc7a0/0xc7f2 deformation flags in non-deforming publication | pause | yes | None | port scope |
| B30 | `deformation_publication.rs:27-145` | pending rebuild or observers; view \|x\| bounds; i32 conversion | pause | yes (deforming) | None/L | port scope (A03220) |
| B31 | `wheel_deformation_state.rs:22`, `wheel_deformation.rs:84` | flags, dimensions, radii | pause | yes | None | port scope |
| B32 | `deformation_grid.rs:47-69` | cell outside grid | pause | yes | None (clamped) | port |
| B33 | `deformation_callback_state.rs:64-171` | contacts > 256, edges > 1200 | pause | yes (deforming) | None | 1200 = descriptor layout |
| B34 | `deformation_impact.rs:74-122`, `deformation_settle.rs:42` | records > 512, edges > 1200, iterations | pause | yes | None (generator stops at 400) | port domain |
| B35 | `deformation_preparation.rs:26`, `deformation_contact_limit.rs:95-328` | contacts > 256, budget > 10000, mass, \|x\| > 1e6 | pause | yes | None | port domain |
| B36 | `deformation_world.rs:26-115`, `deforming_group_frame.rs:306`, `deforming_physical_frame.rs:38,175` | group correspondence, incomplete preparation | pause | yes (car-car and car-piece groups) | L | port scope |
| B37 | `deformation_world.rs:141`, `deforming_group_frame.rs:82` | `expect("registered owner")`, `expect("two-body contact")` | **panic** | yes | None (invariants) | — |
| B38 | `dynamics.rs:14`, `tyre_friction.rs:65` | delta bounds | Err | dynamics yes | None | diagnostic |
| B39 | `spawn.rs:67,81` | SSE | pause | yes | None | port |
| B40 | `pose.rs:34` | rotation > 64 rad | pause | yes | None (B03 only) | port |
| B41 | `damage_region.rs:27-48`, `collision_damage.rs:69-124` | levels, gains, thresholds | pause | yes | None (levels clamp at 1) | port scope |
| B42 | `lap_timing.rs:42-186` | gates ∉ 3..=512, sector gates > 16 | startup Err | no | route | port policy |
| B43 | `time_trial.rs:41-48` | laps ∉ 1..=99 | startup Err | no | None | authored |
| B44 | `grid_population.rs:30-76` | entries > 512 | startup Err | no | route | port domain |
| B45 | `deformation_initialization.rs`, `deformation_layout.rs`, `deformation_volume.rs`, `vehicle_gearing.rs`, `steering_geometry.rs`, `suspension_context.rs`, `suspension.rs`, `spawn.rs` placement | data and range checks; steep placement | startup Err | no | L (other cars or steep grids) | port domains |
| B46 | grid-tools `multi.rs:116` | cars ∉ 1..=12 | startup Err | no | 20 cars impossible | port choice |

### 4.3 grid-tools runtime library

| ID | File:line | Condition / message | On fire | Per tick | Reachability | Bound provenance |
|---|---|---|---|---|---|---|
| C01 | `dynamic_props.rs:858-931` → `contact_groups.rs:75` | `PropWorld::apply` appends pieces with no body bound | pause on the next tick | yes | **H** (R01) | port; original refuses or recycles (§2) |
| C02 | `source_vehicle.rs:745-751` | solo apply, same as C01 | pause | yes | M (15 pieces) | as C01 |
| C03 | `multi.rs:116` | cars ∉ 1..=12 | startup Err | no | 20 cars impossible | authored (original 20 slots) |
| C04 | `multi.rs:69-71` | pool = clamp(24n, 64, 256); rows 4× | sizing | — | already 256 at 11 or more cars | authored (RACE_MODE.md "port guards") |
| C05 | `multi.rs:416-425` | provider stops at `remaining`; at most 32 per car | **silent drop** (later cars first) | yes | M | authored |
| C06 | `multi.rs:455-488` | car pairs at most 16 each; stop at `remaining` | silent drop | yes | M (start and first corner) | authored |
| C07 | `dynamic_props.rs:728-805` | piece budgets 8 track / 4 pair / 8 car; last in order | silent (pieces sink) | yes | L | authored ("not recovered") |
| C08 | `dynamic_props.rs:880-895` | chain selector, cyclic, tail mismatch | pause | break tick | L | data sanity |
| C09 | `dynamic_props.rs:454,877` | constructor Err; `try_into().unwrap()` | pause/panic | break tick | L | data sanity |
| C10 | `dynamic_props.rs:813-848` | recorded contact not found | silent (no break) | yes | L | authored |
| C11 | `dynamic_props.rs:245-373` | unsupported breakable kinds; 256 MiB | startup Err | no | None on Donington; **M for other routes** (props always on) | data/authored scope |
| C12 | `multi.rs:303` → `source_vehicle.rs:588-600` | non-finite or out-of-range controls | pause (whole race) | yes | L (`wheel_command` NaN when forward = lateral = 0 at speed) | data sanity |
| C13 | `multi.rs:312-318`, `contact_surfaces.rs:162` | ray hits WARN/RESE/RESD/UNRE/FBND (no mechanics) | pause | yes | **L–M, unverified** | data sanity |
| C14 | `track.rs:264-267` | chunk materials > 8 | pause | yes (on overlap) | route | port (8-bit mask) |
| C15 | `track.rs:176` | query capacity > 64 | pause | yes | None (callers clamp) | authored |
| C16 | `multi.rs:463` → `body_geometry.rs:203` | hull point order differs | pause | yes | L | data sanity |
| C17 | `body_geometry.rs:243,366` | hull count 0 or > 4096 | silent | yes | None | port |
| C18 | `body_geometry.rs:51` | deformation grid dimensions | pause | publication | L | port |
| C19 | `multi.rs:589` | missing damage RNG | pause | yes | None | invariant |
| C20 | `multi.rs:558` | body events read `contacts[..supplied]`; the tail may hold last tick's records if grid-world returns fewer | silent double damage | yes | **unverified** | authored |
| C21 | `multi.rs:615` → `lap_timing.rs:231` | car out of the world | pause | yes | L | authored |
| C22 | grid-world `time_trial.rs:111` | > 24 h | pause | yes | L | authored |
| C23 | `time_trial.rs:61-110` | record files: 396 rows, laps 1..=99 | records disabled | no | None | data sanity |
| C24 | `ai_driver.rs:1935-1941` | `expect("brake lines")` | **panic** | yes | None on Donington; **route** | data sanity |
| C25 | `ai_driver.rs:111-131` | 0 links → index panic; non-loop | panic / startup Err | no | point-to-point routes | data sanity |
| C26 | `ai_driver.rs:213-236` | hinted projection searches ±8 segments | silent | yes | L | authored |
| C27 | `ai_driver.rs:1389-1405,1880`, `main.rs:2257` | latched Retire only brakes; standings never retire it | silent; the race never ends by itself | yes | **M** | original retire timer 0x77E3B0; hook missing |
| C28 | `race.rs:87-89,175-188` | global nearest-point progress | silent wrong order or finish | yes | L | authored |
| C29–C30 | `race.rs:214-259` | index and `zip` invariants | panic/silent | — | None | invariant |
| C31–C34 | `source_vehicle.rs:444,843`, `multi.rs:276,329` | pool reservation, `checked_sub`, ignored reset Err, wheel pool exhausted | startup Err/pause | — | None | authored |
| C35 | `multi.rs`/`apply` shape pushes | shapes > 513 | pause | yes | None on Donington; route | port (513 adapter) |
| C36–C39 | `track.rs:60-80`, `entity_geometry.rs:137,206`, `cqtc_geometry.rs:20-52`, `lap_progress.rs:16-35` | archive, box, CQTC and progress bounds; one forward route | startup Err | no | route (point-to-point tracks) | authored/data |
| C40 | `engine_audio_continuous.rs:263` | engine audio domain Err latches mute | silent (audio) | per block | L–M | data sanity |
| C41, C43, C44 | `engine_audio_publication.rs:106`, `tyre_audio.rs:330`, parser unwraps | unwrap | panic | — | None (guarded) | — |
| C42 | `tyre_smoke.rs:979,1031,1165` | 80 system slots; particle pool | silent drop | per frame | None at 12 cars; exactly full at 20 | original 80 [V] |
| C45 | `assists.rs:315` | gear request domain | Err to the input handler | per key | None | authored |
| C46 | `body_deformation.rs:15-85` | presentation dent errors | Err to the app (see D03) | per frame | L | data sanity |
| C47 | `pursuit_driver.rs` | not used by the app | — | — | None | — |

### 4.4 grid-app

| ID | File:line | Condition / message | On fire | Runtime | Reachability | Bound provenance |
|---|---|---|---|---|---|---|
| D01 | `main.rs:2155-2224` | any tick `Err` | **pause**, all cars | yes | H (R01) | — |
| D02 | `main.rs:2226-2234` | `trial.tick` Err | pause ("R restarts") | yes | L | authored |
| D03 | `main.rs:2424-2470` | player body-dent projection Err | pause (presentation stops physics) | yes | L | authored |
| D04 | via `lap_timing.rs:231` | player NaN or out-of-world pose | pause | yes | L | authored |
| D05 | `main.rs:2246-2263` | player terminal damage finalizes the race | intended | yes | M | player rule [V]; opponent retire authored |
| D06 | `main.rs:2059` | `standings.over()` | intended stop | yes | H | authored |
| D07 | `main.rs:2334-2353` | R = full restart | all progress lost | yes | H after D01 | authored |
| D08 | `main.rs:2357-2373` | after finishing, F finalizes even while paused; R discards the finish | — | yes | M | authored |
| D09 | `main.rs:2617,2700` | error text only in the title and log | UX | yes | H after D01 | authored |
| D10 | `main.rs:1186` | `--opponents` ∉ 1..=11 | startup exit 1 | no | 20 cars impossible | port (original 20) |
| D11–D13 | `main.rs:1661-1860`, `tyre_smoke.rs:76` | BC-texture assert; validated-asset `expect`s; texture map index | panic | startup | L | authored |
| D14 | `main.rs:1431` | `load_livery(k % 10)` | duplicate liveries for AI 10 and 11 | startup | H (cosmetic) | shipped package count |
| D15 | `main.rs:1569-1617`, `damage_parts.rs:770-822` | optional-feature load failure → warn | silent feature drop | startup | L | authored (64/4096 glass guards) |
| D16 | `main.rs:2272` | `lap.unwrap()` | panic | yes | None | — |
| D17 | `main.rs:204-235` | `Driving::report` deformation loop bound read unchecked from vehicle words | panic (index) | yes (also on the failure report) | L | authored |
| D18–D20 | `damage_parts.rs:971`, `skid_marks.rs:193`, `dynamic_props.rs:198` | map/expect/index invariants | panic | yes | None/L | — |
| D21 | all `println!`/`eprintln!` | stdout write failure panics | crash | yes | L–M (with D29) | std |
| D22 | `ground_detail.rs:354,365` | LOD and sprite index | panic | yes | None | original table 0xD11698 [V] |
| D23 | `wheel_debris.rs:20,347`, `particles.rs:96` | 12 000-particle pool full → newest dropped | silent | yes | **M** (several cars in gravel) | config value not read by the exe; original 80 systems, refuse [V] |
| D24 | `skid_marks.rs:12-276` | 240 × 32 strips; steals the farthest finished strip | recycle | yes | None (stops) | original 240 × 32 [V]; original steals the farthest **active** strip (allocator 0x73F330 [V]; its release 0x7289C0 detaches the wheel laying it; ported in F435) |
| D25 | via grid-tools `tyre_smoke.rs` | 80 systems | silent drop | yes | 20 cars exactly full | original [V] |
| D26 | `damage_parts.rs:1251-1314` | glass shard bursts | bounded | yes | M | authored |
| D27 | `time_trial.rs:176` | first 6 laps shown | display clamp | yes | None | authored |
| D28 | `engine_sound.rs:137-195` | `finished` mutes every engine, AI included | silent audio | yes | **H** | authored; F434 mutes only once the race is frozen |
| D29 | `main.rs:2310-2316` and others | report on every tick with any field contact (`last_body_contacts` = whole world) | **unbounded log** | yes | **M–H** | authored diagnostics |
| D30 | `main.rs:101,2221` etc. | counters u64; `segment as u32` | wrap | yes | None | — |
| D31 | grid-world `time_trial.rs:113` | 24 h | pause | yes | None | authored |
| D32 | `main.rs:1590` | FixedUpdate catch-up when a tick takes more than 16 ms | frame-rate collapse | yes | M in pile-ups | Bevy default |
| D33 | `wheel_debris.rs:484`, `particles.rs:125`, `ground_detail.rs:394`, `main.rs:2433-2518` | per-frame CPU mesh rebuilds | frame cost | yes | M | authored |
| D34 | `assists.rs:20-34` from `main.rs:2137` | `just_pressed` inside FixedUpdate | lost or doubled shifts | yes | **H** (manual, high refresh rate) | authored |
| D35 | `main.rs:2117` | prop script | — | validation only | None | — |
| D36 | `main.rs:2497-2517` | opponent dent errors skipped; no retry | silent | yes | L | authored |
| D37–D38 | `dynamic_props.rs:24-208` | presentation catalog missing; piece without a body is hidden | silent | yes | L | authored |
| D39 | `main.rs:2478`, `wheel_ground.rs:67`, `damage_parts.rs:869` | NaN opponent pose → NaN transforms (no finite check) | silent render garbage (Bevy effect not verified) | yes | L | — |
| D40 | `Single<>` / `.single()` camera | skipped system, no panic (Bevy 0.19) | — | yes | None | — |
| D41 | `main.rs:1991-1999` | render-capture exit on failure or timeout | exit | validation only | None | — |

One correction to a work note: armco and tyre walls are **not** breakable in the port (`dynamic_props.rs:11`, "unported" and kept static). Only brake-marker boards (2 pieces), tyre stacks (12) and cushions (1) add bodies.

### 4.5 Shared-world quantities (Donington, observation from code)

Pieces allowed before R01 trips = 16 − cars. The contact pool is already 256 at 11 or more cars, so props cannot grow it.

| Cars | Pieces that fit | First trip | Pool (wheel / provider) |
|---|---|---|---|
| 1 (solo) | 15 | 1 tyre stack + 2 boards | 36 + 12 per piece |
| 2 | 14 | 1 stack + 2 boards | 64 (8 / 56) |
| 5 | 11 | 1 tyre stack | 120 (20 / 100) |
| 12 | 4 | 3rd board, 5th cushion, any tyre stack | 256 (48 / 208) |
| 20 (hypothetical) | — | tick 1 with no props | 256 (80 / 176) |

## 5. Recommended fix plan

Every fix either reproduces the original's limit and full behaviour (§2), or, where the port's guard protects an unverified state, degrades the way the original does: skip, refuse, recycle or freeze one island or car, rather than pausing the race. The groups touch mostly separate files and can become separate sessions. The order is by risk.

### S1. Shared-world body capacity and prop pieces (R01, R04, R06, partly R05)

*Done in F317:* the body, group and shape bounds were raised to 1000 / 1513, and a break beyond the world bound is refused (2.1). What remains from this group:
- port the original farthest-first restore (7D4940 → 7D4770) in place of the refusal;
- piece sleeping (turnOffTime 1);
- the per-island 200/1000/3000 skip behaviour (B7B850) for fidelity;
- correct the `lib.rs` provenance comment (game-set island limits, not engine defaults);
- the falling-piece bound (R06): *F342 holds lost pieces; the original bound at B44E80/B444D0 is still unrecovered.*

The original design notes for this group follow.

- **Separate the two limits the port conflates.** The world body bound (`contact_groups.rs:75`) should become a pool size close to the original's global rigid-body pool (1000) or a generous port bound. The per-group bound in `impact_frame.rs:91`, `impact_step.rs:76`, `contact_rows.rs:164`, `main_contact_response.rs:72`, `physical_response.rs:60/148` and `impact_correction.rs:44` should follow the original island limits: 200 bodies, 1000 contacts, 3000 rows. Raise `contact_groups` shapes 513 and the `packed_world` 512 shapes together.
- **When a group still exceeds the island limit, do what B7B850 does:** return status 1 and leave that island unintegrated for the step (positions keep their pre-step values), with no `Err`. `grouped_physical_frame.rs:483` must then accept status 1. B24EF0 farthest-body freezing applies only if the live focus flag +11C is confirmed set.
- **Props, following the original's 7D4940/7D6A30/7D5180 order:**
  - When a break would exceed capacity, reset the farthest other broken entity from the focus point (the player car) if that frees room.
  - Otherwise skip the break, so the prop stays intact and the contact limit stays armed.
- **Piece sleeping (turnOffTime 1).** This removes resting pieces from the active set and keeps them from drifting. It needs sleep support in the grouped frame (A11 rejects sleep durations other than MAX) and the A12 all-asleep status.
- **Falling pieces:** with sleep and island skipping, \|pose\| > 1e5 (A25) should not be reachable. Recover the original bound at B44E80/B444D0 before changing the guard. Meanwhile, recycling a piece far below the track is an original-like degradation, because the original recycles broken entities on capacity.
- **Tests:** 12 cars plus every Donington breakable broken, a pile-up into a tyre stack, and a piece pushed under the track. The headless chaos soak session (build-a-headless-chaos-s) should run these.

### S2. Contact budget and ordering (R05)

- Size the contact pool from the original per-island and per-owner limits (1000 contacts per island, 80 per owner, 20 per collision) instead of `clamp(24n, 64, 256)`. Then raise `MAX_WORLD_CONTACTS` and `MAX_SOLVER_ROWS` consistently (A06, A17, A19, A21, B33, B35).
- Where a port budget remains, fill it fairly. Use round-robin over cars, then pairs, then pieces, instead of car-index order, and count the truncations in the report so a starved car is visible.
- Verify C20 (the body-event window versus the active contact count returned by grid-world) and use the returned count.

### S3. Failure containment and recovery (R02, R16, R09)

- **Per-car isolation in `MultiRuntime::tick`:** if one car's vehicle pipeline returns `Err`, keep that car's previous state and freeze or retire it (status `Out::Retired`, with a single log line), and let the rest of the field continue. This is the closest original-like degradation: the original skips one island for one step and never stops the race. Keep the full pause only for world-level invariant failures, and show the message in the HUD.
- **Time trial and presentation must not stop physics.** On a trial bookkeeping error, invalidate the lap and log it. On a player dent error, keep the previous mesh as the opponent path does, instead of pausing.
- **Rate-limit the reports.** Report only player-car contacts, or every 16th tick, plus failures. Write through a buffered, non-panicking writer (`writeln!` with the error ignored).

**Status (F323, implemented):**
- `MultiRuntime::tick` contains every per-car stage: controls, vehicle manager, the car's own track contacts and car-pair hull contacts (inside the geometry provider), body events and lap timing.
  - A failing car keeps its previous state. Its world body is held at the last committed kinematics (words 0x1C0..0x230, velocities zeroed) on every later tick. It still collides with the others, and its clock (`Runtime::ticks`) keeps running.
  - The car is exposed through `retirement(i)` and `newly_retired()`.
  - The world advance and prop pieces still fail the whole tick atomically, as before.
  - With no failure the tick is byte-identical to before (12-car props test hash `6e19b665dbf510b1` on both builds).
- grid-app:
  - marks the car `Out::Retired` and logs one `drive_car_retired` line;
  - shows "NAME retired: reason" (or "Your car retired: reason. R restarts") in the HUD while the field races on;
  - shows the real message for a world-level pause ("Simulation paused: tick N: ...").
- A refused trial tick logs `type=refused`, and `State::skip_tick` counts the lap as an invalid attempt and keeps the trial clock in step. If no lap is still timed, the runtime's current lap is marked invalid.
- A refused player dent projection keeps the previous mesh and logs one line per refusal kind. The refused field is not projected again until it changes.
- Reports in a race count only the player's body records (`body_contacts_of(0)`). The after-finish AI's continuous controls no longer trigger a report, and a retired player's contacts do not either.
- `log_line!` replaces `println!` on the per-tick and per-event paths (reports, lap, trial, race-result, retirement, damage, prop-break and audio events). Startup prints are unchanged.
- Observed side effect: a car parked directly behind a held car stays blocked (car 7 behind car 5 in the test). F332 correction: that test drives every car with fixed throttle and no AI. With the ported AI the field avoids or recovers past a held car, and the original never respawns AI cars (docs/RESET_AND_RESPAWN_RESEARCH.md).
- Not done: single-car `Runtime::tick` failures (practice and time trial) still pause, with the message now in the HUD; the R restart is still the only recovery.

### S4. Coordinate domains for other routes (R03, R11, A27)

*Done in F322 (static check: private `local/runtime-limits-audit/s4/`):*
- 9E6660 compares only the impact with mass × threshold and clamps the ratio to 1. It subtracts the car translation from the point and transforms the difference with no check. 999C00 works on the car-local point (enable byte, half-extent, radius, attenuation, clamp to 1). 9EF360 checks only the FLT_MAX impact sentinel, the B13/B14 flags, the +15A0 impact gate, the +2E8 timer, the velocity² threshold and the suspension clamp. None of them bounds a coordinate.
- `collision_damage.rs` and `body_impact.rs` keep only finiteness for the point and the car translation. The settings-only `damage_region` pre-pass now uses the local origin instead of the world point, which removed a hidden 1e4 world domain. The rotation bound (1.1), the normal bound and the impact and velocity bounds remain port guards.
- `contact_manifold.rs`: the matrix and fallback point were raised from 1e4 to 1e5, the domain the same function already applies to triangles and existing points. AE2FA0/B45310 have no such bound.
- `Mesh::world_bounds` keeps its 1e5 port guard, now documented in code. B42050 (kind-7 bounds), B44E80 and B444D0 have no domain. Only a lost body can reach 1e5 (R06 and R11, handled by S1 and S3).
- **Remaining blockers for the large routes:** none of the 34 routes beyond 1 km loads in `grid-opponent-race` today. Each fails at startup, before any tick:
  - Spa (all 4 routes): "VCQTC packed count limit exceeded";
  - Le Mans (both routes): "prototype lap timing requires one forward route with finish gate zero";
  - Haruna (all 10 routes): "collision archive entry count unsupported";
  - Okutama (all 6 routes): "expected one typed entity child";
  - Detroit (4 routes): "unsupported collision primitive";
  - San Francisco (6 routes) and Washington (2 routes): "unsupported mesh stream kind/format".

  These belong with S8.

The original plan for this group follows.


- Remove the ±1000 m point and pose bounds in `collision_damage.rs` and `body_impact.rs`, keeping only the finiteness checks. Confirm in grid.exe (9E6660, 999C00, 9EF360) that the original has no such domain. 34 of the 92 shipped routes exceed 1 km, so this blocks most of the route list.
- Document or relax the 1e4 manifold bound and the 1e5 mesh-bounds guard against the original (AE2FA0, B45310, B44E80). They are safe for every shipped route today (all under 3 km), so this is low priority.
- For a car out of the world, retire the car (S3) instead of letting the 100 km lap bound pause the race. F332: the original's reset path (ResetLinesManager, docs/RESET_AND_RESPAWN_RESEARCH.md) serves only the local player: upside down and nearly still for 2 s, or the pause menu. It does not write +0xDE4/+0xDFC, whose meaning stays open. AI cars are never reset.

### S5. Numeric edge guards that the original does not have (R07, R10, R17, R18, R19)

- B04: clamp `sideways` to [-1, 1] before `asin`, after confirming the A06A90 tail.
- A42/B24: with exactly zero point velocity, skip the drag impulse for that wheel instead of failing. Confirm the original's 9DFC50 result first.
- B27 singular projections and A33 degenerate deformed triangles: match the original's arithmetic (B44E80/9B8B70) or skip the degenerate element.
- Route the remaining finite guards (B03) through S3 containment rather than relaxing each one. Measure the B09 iteration limits (64) in the chaos soak before changing them.

### S6. Race layer and field size (R08, R13, R23, R24)

- Retire latched-Retire AI cars in the standings (C27), using the original retire-timer semantics at 0x77E3B0.
- Compute opponent progress from the AI driver's hinted segment instead of a global nearest point (C28).
- Allow up to 20 cars (original race manager and car manager capacity): raise the `MultiRuntime` and `--opponents` caps only after S1 and S2. Then check the tyre smoke 80-system budget (exactly full at 20) and whether the original's debris systems share that pool, and fix livery cycling (D14).

### S7. App input, audio and effects (R12, R20, R21, R22, R25)

- Latch gear-shift key presses in `Update` and consume them in FixedUpdate (D34).
- Mute only the player engine, or none, after the player finishes (D28).
- Debris pool: follow the original system-level refusal (80 systems, error 0x37; emitters not created when no buffer is free) instead of a 12 000-particle drop-newest pool shared with nothing. Recover the per-material buffer counts first.
- Skid strips: steal the farthest active strip, as at 73E430 (D24).
- Profile tick cost in pile-ups and bound the per-frame mesh rebuilds (D32, D33).

### S8. Route generalization traps (R14, R15)

- Replace `expect("brake lines")` with a no-brake-line branch.
- Skip unsupported breakables instead of failing startup.
- Validate chunk material counts (more than 8) and undecoded vertex counts (more than 1024) at load.
- Scan every route's packed chunks for the materials without mechanics (C13) and decide the original fallback.
- Support point-to-point routes in lap progress and the AI line (C25, C39).

### S9. Latent unsupported states (Latent row)

When wheel detachment, respawn, nitro, manual clutch or handling notification receivers are ported, remove the matching rejections in the same change: `suspension_vehicle.rs:78`, `vehicle_driver_prepare.rs:55-80`, `vehicle_handling.rs:207/261/365` and `vehicle_job.rs:22`. Otherwise each becomes an immediate race freeze.

## 6. Open questions

- Is the solve-manager focus flag (+11C) enabled in a live race? It decides between skipping the whole island and freezing the farthest bodies.
- What happens when the global `maxContacts` (10000), `maxBroadPhaseCollisions` and `maxContinuousCollisions` pools are full? Their consumers are not traced (B18E70).
- What are the per-material particle buffer counts? Is there a per-emitter particle cap? Do kick-up debris systems share the 80-system manager with tyre smoke?
- Do the archives override the ENTITYMANAGER instance limits? Only grid.exe contains the key text among the uncompressed install files.
- Does Donington's packed collision use WARN/RESE/RESD/UNRE/FBND (C13)?
- Can grid-world return fewer active contacts than it was supplied (C20)?
- What are the original behaviours at A06A90 (`asin` domain), 9DFC50 (zero-velocity drag), B44E80/B444D0 (mesh-bounds domain; F322: none) and 9E6660/999C00/9EF360 (damage coordinate domain; F322: none)?
