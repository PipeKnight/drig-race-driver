# Source-only serial driving prototype (F210, verified scope)

12 October playtest update: **the owner reports a completed manual Lacetti/Donington lap**. Both first-session logs show successful startup and no recorded simulation failure, but contain no trajectory telemetry. This is an owner-confirmed result, not an automatically measured lap. [Findings and control follow-up](PLAYTEST_2026_10_12.md); [launcher and controls](TRY_THE_PROTOTYPE.md). Automatic reverse is now enabled through the existing control helper; source runtime and actual Bevy keyboard tests verify backward movement and handbrake stopping. New logs capture controls and motion. Fresh 160-tick release rendering remains approximately 70.7 km/h and its chase view was visually inspected. Partial-throttle behavior remains uncalibrated; keyboard throttle is binary. The sections below preserve the original F210 automated evidence and scope.

Recorded 12 October 2026, Europe/Kiev. `grid_tools::source_vehicle::Runtime::lacetti` now loads source CSV, NED, Standard HDM from raceload.jpk, packed Donington collision world, surface mechanics and slot_00 directly from the purchased installation. No car capture, decompiled output or native executable supplies runtime startup.

The composition joins the selected source-owner prelude, Standard constructor, suspension context, HDM/reset, complete source physical factory, SSE ground placement, pose bridge and persistent registered serial world. Original outer resource/preferences code is not executed. Assisted automatic preferences, readiness, owned shape identity and serial-manager settings are supplied explicitly. A single shared authored physical material is retained from the validated world adapter; general shape/material assignment, full body collisions, deformation/event damage, AI, race rules and automatic lap validation remain absent.

The world attachment preserves the source body's self-rooted group, active flag, tensors and pose. Only known external pointer tokens are canonicalized. TF04 is a separate registered-source diagnostic; TF03 keeps its original cold contract. TF04 permits up to4096 retained updates, TF03 remains bounded to64.

## Startup correction and limits

An initially successful source-only prototype reached only about7km/h at full throttle. Static outer A0E880 sets both 2FC and188C to1 before the Standard constructor. The latter is the engine torque multiplier; leaving it zero permits idle/drag behavior while ordinary supplied-state comparisons still agree. The runtime now supplies both observed defaults. This demonstrates why exact updates alone do not prove complete startup.

With those defaults, powered source runs reach higher gears and normal driving speeds. Constant steering eventually leaves the road; a long straight run also passes beyond the supported road and falls because full body barriers/collisions are absent. Those are retained failure/limitation observations, not successful laps. Validation therefore checks grounded driving intervals and separately documents the longer unsupported trajectories.

The earlier source pose copied from an authored physical-contact fixture caused a damage rejection. Source ground placement fixes that startup placement error. Normal road support uses tyre/suspension contacts; absence of wheel-overtravel physical constraints does not mean absence of road contacts.

## Interfaces

- `grid-drive GAME_ROOT local/OUTPUT.tsv TICKS THROTTLE STEERING [slot_00]`: source-only headless telemetry and private initial/final state. Positive source steering turns toward local positiveX, which is left.
- `Runtime::tick(Controls)`: one fixed0.016s interval. Stages controls, complete car/world and query cache; failure preserves authoritative state and tick count.
- `Runtime::reset`: restores complete initial state and initial query cache.
- `grid-app --drive`: Bevy keyboard/chase-camera prototype. Arrows/WASD drive, Space handbrake, P pause, R reset, Escape exit. Runtime failure pauses with an error rather than dropping invalid state. Source model alignment and moving chase view have been inspected in two offscreen renders; wheel animation and original renderer equivalence remain separate.
- `--drive-ticks N --throttle T --steering S --render local/FILE.png`: bounded offscreen integration check.

Model placement applies the constructor's NED-to-local-wheel translation (1630 with the copied vertical bias) before the authoritative physical pose. This is a source-geometry alignment, not verified whole original rendering.

## Evidence and verification status

Context proof is complete in [F209](SUSPENSION_CONTEXT.md). Final debug and release each pass four grounded trajectories: rest1200, power600, left160 and right160, totalling2120 complete original world calls per profile. Original/repeat/Rust observable car/body/world wires agree exactly. Complete source-only Runtime car and body agree with the original, with only the checked center-identity token normalized. Runtime view/solver agree with the Rust diagnostic after the checked view-owner identity normalization. Final input wires are constructed fresh from source-only initial state and explicit serial settings; no capture/template supplies queries or startup.

Rest displacement is0.003916895m over19.2s; powered displacement is328.653535579m over9.6s, approximately214.9km/h at the end. The two160-tick steering intervals travel15.96748m and15.98822m with opposite heading signs. Four road wheel hits are required after startup settling. These are source/original consistency checks, not calibration against the owner's recording or a completed lap.

Each profile also passes16 paired malformed-input rejections and four overwritten-inertia-cache controls (old AC0..AC8 zero/NaN/+infinity/-infinity). Whole9F1E10 overwrites those old values from separately guarded car inertia before use. The native preflight was corrected accordingly; the numerical Rust implementation did not change. Current legacy cold-world regression passes300 cases /6150 updates /42,636,000 observable bytes and44 guards per profile, including661 produced contacts,637 repeated contacts and175 history pairs.

163 library tests pass, plus the explicitly enabled purchased-source reset/control/rollback integration test. That test checks acceleration, braking, handbrake input, deterministic reset and failure rollback including the query cache. A separate explicitly enabled Bevy test passes keyboard steering signs, throttle, pause, reset, safe failure pause and recovery. Formatting, all-target warnings-denied Clippy and validator Python syntax checks pass. These asset-dependent tests are ignored by default, enabled with GRID_GAME_ROOT.

Release Bevy offscreen runs at0 and160 powered ticks saved valid images and exited normally. Both have been visually inspected: Lacetti at the source start line, then progressing down Donington's straight with the chase camera. The printed160-tick pose/velocity agrees with headless telemetry within decimal-print precision. Rendering used RTX4070/Vulkan, driver616.92, BC textures. Gilrs reported controller event error0x80070057 and skipped that gamepad's events; keyboard and scripted checks still passed. No human desktop input was needed for these checks. At the original F210 checkpoint, manual driving/full-lap validation was still outstanding; the later owner-confirmed lap is recorded above.

The independent final audit checks current executable hashes, both source/cold reports, all139 F209 original/repeat/Rust output files per profile, source/upstream provenance, Python syntax, renders/160-tick telemetry agreement and preservation. Original Steam130 reference resources plus executable remain unchanged at2026-10-12T02:36:23.127943UTC.

Final private evidence:

- `local/behavior/source-drive-{debug,release}-20261011-final-02/validation.json` and adjacent execution logs.
- `local/behavior/source-drive-cold-{debug,release}-20261011-final-01/validation.json`.
- `local/behavior/source-drive-libtests-20261012-final.txt`, `source-drive-integration-test-20261011-current.txt`, `source-drive-bevy-keyboard-test-20261011-final.txt`, `source-drive-clippy-20261012-final.txt`.
- `local/scene/source-drive-20261011/{start,power}.png` and render logs.
- `local/scene/preservation-20261012-source-drive.json`.
- `local/behavior/source-drive-audit-20261012.json`.

Current SHA256:

| Artifact | SHA256 |
| --- | --- |
| source-vehicle-world-probe | `06e9f63f01298fc726ddce9ed08721ec4ca3d33508b2c65e3aabcbc56f73a0af` |
| legacy vehicle-world-probe | `c8d4c866d3ed0b1293d54c20256b4aca0db375e88594a4ef31f36f988a1df4ba` |
| grid-vehicle-force debug | `0bafe598289decf9135115407fd95ec82e74c93244b417faacf43d1989651e92` |
| grid-vehicle-force release | `f08f7ca53ffc1912d11f5eb43141469bcbdb65eedefe51f2ed5e1338b8d8ab16` |
| grid-drive debug | `674562538793c0b1a01112795966fa591b9ab43a5602bab46a088d9a8fdc70ea` |
| grid-drive release | `c48cf53aa2431e15c02de1ba0fc6c2acbba26dc16f2db412f55eb9060736a81f` |
| grid-app release | `ce83071f916e80db78afe5fc4f3f225375b778aedc6d60ddb0d9fc6ef94fbee9` |
| start PNG | `b0b8774fe670e4736aa9268ca6b08942404c45e79d436ccedbaed9a904e0ce4f` |
| power PNG | `db1e16f009d72fba13de92c0af854550bc6e889618b44430c64b48da986bf7fc` |

Private evidence:

- `local/behavior/source-runtime-20261011-draft-01/`: whole manager/source attachment, valid placement, shifted-height trials and earlier damage rejection.
- `local/behavior/source-drive-20261011-draft-01..05/`: source-only startup and telemetry. Draft01 rejects uncanonicalized relative wheel links. Draft02..04 omit torque multiplier; draft05 includes the correction and preserves long off-road trajectories.
- `local/behavior/source-drive-original-20261011-draft-01/`: all four1200-step wire comparisons agree, but accidentally inherited two authored query records; not the runtime's absent-query contract.
- `local/behavior/source-drive-original-20261011-draft-02/`: cleared queries; original/repeat/Rust and complete runtime car agree for the old zero-torque startup. Superseded for driving startup.
- `source-drive-debug-20261011-final-01`: first final validator stopped at an opaque view-owner tag comparison; numeric car/body agreement was not the failure. Comparison now checks and normalizes that explicit identity.

One initial API compilation used invalid leading-dot Rust literals; fixed before execution. One automatic literal cleanup also changed a tuple accessor; fixed. Handbrake was initially mapped to clutch/derived brake state, corrected from actual E38 consumer. A first patch used a stale formatted anchor and changed nothing. Clippy found two iterator/cache idioms; a follow-up array conversion compile error was corrected, then all-target Clippy passed. These corrections are independent of original numerical comparisons.

The first keyboard test incorrectly retained the held reset key between simulated presses because ButtonInput::clear only clears edges. The harness now uses reset_all; no production behavior change was needed. Early steering labels were corrected before final validation: positive original command turns left. Reset initially rebuilt an empty cache; it now restores the actual initial source-placement cache. All failed/superseded artifacts remain private.

## Next integration work

Wheel steering/spin/suspension presentation and an owner-confirmed first lap now exist. Next integrate body/static barrier collision support and safe recovery. Retain guards on currently unsupported damage branches. Trajectory-based lap/checkpoint validation remains open. General collision material assignment, source preferences/resource factory coverage and timing/calibration against the owner's keyboard/all-assists/automatic recording remain open. The moving prototype does not imply a complete original engine.

No new tool installation, desktop input, original Steam edits or owner assistance was required. Existing Rust1.99.0/Bevy0.19.1/Python3.12.14/MSVC14.44.35207/SDK10.0.26100.0/Ghidra12.1.4/JDK21.0.9/Capstone5.0.6 were reused. The temporary overnight automation remains expired/paused; this is the owner's resumed work.
