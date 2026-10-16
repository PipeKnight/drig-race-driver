# Handling-damage source investigation

F204 supersedes the earlier source-reader exclusion: [complete cold configuration and car reset](HANDLING_CONFIGURATION.md) verify all360 settings bytes through original9C5580 and join the whole selected reset to source-constructed cars. Original text tokenizer/resource acquisition and outer factory remain excluded; the numeric CRT conversion is explicitly adapted.

F191 supersedes earlier isolated-update scope: [complete selected9F3E80 and notification-one publication](VEHICLE_HANDLING.md) include visual evolution, wear, rollover, gear history and full selected effects. Above-threshold collision distribution and live queries/notifications remain unimplemented.

Recorded 8 October 2026. F132 joins source limits to visual wheel-level evolution (F131). F130 confirms actual Standard source selection/live steering; F129 joins source to [deterministic reset F128](STEERING.md). The original XML loader is not isolated for comparison.

## Source visual limits (F132)

The bounded HDM reader now exposes ten fields: the earlier seven steering values plus `visual_damage` with angular limit280, displacement limit284 and terminal percentage288. Defaults are27, stored-f32 .45 (3ee66666),150. Numeric conversion remains decimal f64 then f32. `grid-wheel-damage --source HDM` overrides the supplied angular/displacement placeholders and emits the three source words before four result words. Terminal percentage is reported for provenance; its notification behavior is unimplemented.

**20 datasets /3,200 cases /22,400 fields exact debug/release**, comprising both hashed installed HDMs and18 authored default/zero/signed-zero/underflow/decimal/explicit settings controls. Each has160deformation/correction/prior/ordinal states. Whole original9f2f80 receives independently Python-decoded parameters; native reads HDM. Distinct999/998placeholders prove overrides are consumed. All36captured parameter words from twelve Standard cars match independent source and native outputs. Original XML loader/converter/constructor, deformation producers and notifications remain unexecuted; this is source joining to the numeric subset.

Evidence `local/behavior/source-wheel-damage-20261008-01/validation.json`,02/; source probe SHA c45b962f08dad43576157cf26a7ae7360dc3c7367ee109ce50bfb07372319724. Four malformed/range/nonfinite source guards reject before output. Prior2,426-case supplied/live wheel replay and2,560-case source reset regress exactly in both builds (`wheel-damage-source-regression-20261008-01/`,02/ and `source-reset-wheel-damage-regression-20261008-01/`,02/). Prior original probes remain unchanged.100workspace tests/fmt/warnings-deniedClippy pass `source-wheel-damage-checks/`; Python compilation passes. No numerical failures/new tools/game inputs/settings changes. Next upstream deformation/impact production and full update order; this checkpoint supersedes F131's pending source limit work.

## Visual wheel-level evolution (F131)

Production9f3e80 calls whole9f2f80 for each wheel before9c4b70 and the later9c5b90 steering producer. ECX is mutable+258; stack arguments are delta, wheel ordinal and mutable object, with12-byte callee cleanup. Delta is never read. Body identity follows mutable214 -> vehicle24. The selected prior level is mutable18+4*ordinal.

Either signed zero at settings28 (mutable280) skips the entire update. Otherwise divide absolute angular deformation at body1790+4*ordinal by settings28 times stored-f32 degree conversion3c8efa35. Displacement is body1650+16*ordinal's four-lane vector; when bodyC7AE is true, subtract bodyEEFC times bodyC740..C74C. Squared length is `(y*y+x*x)+(w*w+z*z)`. RSQRTPS and ordered refinement `(1-root*seed)*(root*.5)+root` produce length, with positive zero selected for zero squared length. Divide by settings2C (mutable284), choose the larger ratio, clamp[0,1], retain prior if strictly larger. Prior above1 is not reduced; equality selects the new word.

`wheel_damage::calculate` implements finite guarded arithmetic, nonnegative angular scale, positive displacement scale when enabled and x86 SSE seeds. Whole original probe executes intact RX9f2f80 with mutable442=false/status444=7: actual branch skips notifications, no patches/stubs/injection. Its mapped E58000 lazy-data page is writable then restored. Terminal events/status changes and deformation producers remain separate.

**2,426 cases /9,704 original-vs-Rust words exact debug/release**:2,048 authored states, signed-zero skip/prior/clamp/correction/four-lane/ordinal controls, seven identical-output delta controls[-1000,1000], and96 wheel replays from two paused captures. Five invalid fixtures fail both guards; native writes no output.384 additional native-vs-captured words match and60/60 relevant blocks are stable. This verifies settled non-atomic idempotence, not incoming tick evolution or causation of earlier impact damage.

Aligned9c48c0 setup maps WHEEL/damageScaleFromVisual ->280 (default27), damageScaleFromVisualDisplacement ->284 (stored-f32 .45), terminalDamagePercentage ->288 (default150). Current live Standard settings are25.6879/.45/105. Native source joining remains next; current parser exposes seven earlier steering fields.

Final evidence `local/behavior/wheel-damage-20261008-04/validation.json`,05/;01..03 earlier exact runs. Probe SHA6087bb502314bb9a20a4b866f776c621c1c918a935358969db1f33b47bb33aac. Captures `general-vehicle-manager-20261008T1432013171667Z.json`, `general-vehicle-manager-20261008T1432414420465Z.json` each have282/282successful reads; capture adds9f2f80 prefix identity and three correction fields. Static `wheel-level-{instructions,tail,caller,words,source-instructions,source-strings,defaults}-20261008.json`, Ghidra `20261008T1422257334826Z/`, `20261008T1427186138572Z/`.9c4b70/9c5250/999c00 remain unimplemented research.

**100 workspace tests**, fmt and warnings-denied Clippy pass (`wheel-damage-checks/tests-final.txt`, `clippy.txt`); Python compilation and PowerShell syntax checks pass. Initial Rust test literal syntax, two nonexistent CLI/build lookups and one rg Windows wildcard query corrected; no numerical failures. A multi-document patch failed on a partial-line anchor and was reapplied. Reused Python3.12.14/Capstone5.0.6/Ghidra12.1.4/JDK21.0.9/Rust1.99.0/MSVC14.44.35207/SDK10.0.26100.0; no installs/additional game input/settings changes. GRID paused.

## Live selection and mutable outputs (F130)

A fresh private uk-UA reference loads Touring Cars / Chevrolet Lacetti / Donington Park / Grand Prix Circuit / three laps / Nevada Autosport. Menus confirm Normal difficulty, Random grid, four flashbacks, traction/braking/stability assists On and automatic transmission. The existing profile loads. Explicit Computer Use `process:` launch works; plain-path launch resolves a Steam URI and fails to expose GRID. Session identity is retained in `local/reference-race/steering-20261008/session.json` (PID29904, start14:06:22.3866196UTC). No source game/settings writes or new tools.

`find_reference_vehicle_manager.ps1 -IncludeSteering` reads the embedded object at body+CDF0 (0x460bytes) and separate body2294..22CF copies. It compares32-byte live producer9c5b90/copier9bd7f0 entry prefixes to the pinned private analysis PE, alongside the existing count-code/process/path/hash checks. Initial64MiB search finds no candidate; the existing256MiB bound finds the manager after160,059,392bytes. All seven captures have245/245successful reads, twelve vehicles/four wheels each. Snapshots do not suspend the process or write/inject anything.

Every captured object names **cars/Standard.hdm**. All84 sets of seven fields match both independent Python conversion of the hashed installed archive and the native source reader, including the .95/.1 defaults. The source is now observed for this race; other disciplines and override policy remain untested. The live record contains maxMisalignAngle0, maxWheelLockScale0.8902, wobble gain5, start.65, end.95, low.1 and suspension threshold1 (all stored f32).

`tools/validate_live_steering.py` replays captured runtime levels/seeds/parameters through the unchanged whole original9c5b90 probe and the Rust producer. **84 cases /3,360 compared fields, exact debug and release**: per case11 original-vs-Rust,11 Rust-vs-live,11 producer-vs-body-copy and7 source-vs-live. Rotations include positive and negative zeros, compared bit-for-bit; flags compare byte values. Baseline, post-throttle and final paused pairs each have24/24identical complete mutable/copy blocks. These are successive read-only non-atomic observations, not a synchronized tick or proof of whole copier side effects. The isolated original disables component queries; selected steering fields match the observed active live receiver, without validating ancillary query outputs.

The existing2,000ms throttle macro completes and advances the car. An independently authored F2 uses the same bounded resume/throttle-right/pause sequence; its log shows a2,000ms hold and keys released. A direct paused view shows barrier contact, and player wheel levels increase to approximately0.02891/0.02905/0.12844/0.37713. A further bounded throttle sample leaves these levels unchanged. They remain below.65, so this capture does **not** exercise nonzero perturbation/frozen steering. Standard's zero misalignment amplitude produces only signed rotation zeros. The pre-reset RNG state was not observed; six captured seeds are replay inputs, not proof of the complete live reset sequence. Damage accumulation, input transfer, scheduling and full driving remain open.

Final evidence `local/behavior/live-steering-20261008-06/validation.json` (debug),07/ (release) contains all seven snapshot paths/hashes, observed source names/words, input hashes, executable hashes and comparisons. Earlier01..05 reports record incremental runs. Snapshot timestamps:14:12:01.3861018,14:12:25.3758070,14:15:10.0986454,14:15:42.5060650,14:16:51.2524521,14:17:42.6445053,14:18:49.3433545UTC. The64MiB no-candidate result is `general-vehicle-manager-20261008T1411455320750Z.json`; macro records are under the session's `macros/`. A scratch inspection command had a Python quoting syntax error and was corrected; no capture/replay numerical failures. Reused Python3.12.14, Rust1.99.0 binaries and AutoHotkey2.0.29; prior99workspace tests remain the latest engine checks because no Rust changed. Original executable and130traced resources match baseline at14:19:11UTC (`local/scene/preservation-20261008-live-steering.json`).

## Native reader and reset composition (F129)

`handling_damage::read` accepts the observed ASCII HDM subset, up to64KiB/128elements/16levels/64attributes per element. It strips legacy description declarations only outside markup/attribute values, requires one VEHICLEHANDLINGDAMAGE root and one direct SUSPENSION/WHEEL each, checks matching ends/duplicate attributes and rejects malformed, ambiguous, nonfinite or oversized inputs. DOCTYPE/entity/CDATA and non-whitespace body content are unsupported. It exposes only the seven steering fields in the mapping below. Unknown attributes remain unconsumed; this is a bounded subset, not a full handling-damage reader.

`grid-steering-reset INPUT OUTPUT --source HDM` replaces all seven supplied parameter placeholders with source-derived fields. **20 datasets /2,560 cases /64,000 fields exact debug/release**:two hashed installed archive entries plus18 independently authored decimal/signed-zero/underflow/default/explicit-endpoint controls,128 supplied RNG states each with both wheel-view modes. Twenty-five compared fields comprise7parameters, final RNG state,6seeds and11 steering values/flags. Original whole9e6070->9c5b90 gets independently Python-converted parameters; native reads the HDM. **Original XML loader/configuration reader/numeric converter/full constructor remain unexecuted.** Parameter linkage is aligned static evidence. Source-mode reset does not establish current live selection, RNG state, damage evolution or scheduling.

Evidence `local/behavior/source-steering-reset-20261008-01/validation.json`,02/; original reset probe retained unchanged with F128's pinned SHA. Seven native invalid-source/reset guards pass before writing. Prior1,392-case supplied reset regresses both profiles (`steering-reset-source-regression-20261008-01/`,02/). Two new parser tests cover declarations/defaults/distinct names, every truncated prefix and malformed/ambiguous source. **99 workspace tests**, fmt and warnings-denied Clippy pass (`source-steering-reset-checks/`). Initial compilation assumed string attributes, whereas pinned API exposes bytes; corrected. Initial test literals lacked leading zero; corrected. No numerical failures.

Reused cached/pinned quick-xml0.41.0, already in Cargo.lock; builds completed offline without tool installation. Checked local API source and [official quick-xml reader documentation](https://docs.rs/quick-xml/0.41.0/quick_xml/reader/struct.Reader.html); newer documentation has different attribute types, so the local pinned API governs. Existing toolchain unchanged. Original reference preservation at13:46:41UTC confirms all130 traced resources and executable (`local/scene/preservation-20261008-handling-damage.json`). Next verify actual selected source/initialized mutable fields in a fresh reference race.

## Static source provenance

Constructor a0e880 passes body+CDF0 to whole9c6360 at a0f53e, then resets that same object via9e6070 at a0f54e. Named HANDLINGDAMAGE setup optionally chooses a custom damage filename; its fallback is cars/standard.hdm. The traced raceload.jpk contains raw standard.hdm/openwheeler.hdm and hdm_loader.xml, whose loader entries select both resources. All archive bytes match the earlier hashed race baseline. Individual Standard vehicle override selection is not yet established.

The HDM files are ASCII text with legacy `<!description>` declarations. A strict XML parser failed on these at line3; the bounded private adapter removes plain description declarations and parses the remaining XML. It rejects DOCTYPE/entity/CDATA declarations. Each HDM yields26 elements. The survey proves independent resource decoding, not original parser equivalence. `local/behavior/handling-damage-source-20261008-01/` retains the failed attempt;02/survey.json records success, entry/archive hashes and private decoded files.

9c6360 loads into object+218 via9c5750; that reader passes the same destination to9c5580. Aligned calls in9c5580 use destination+28 for suspension reader9c47a0 and destination+40 for wheel reader9c48c0. Float property reader a67b60 calls numeric converter bf3b60 and stores its x87 return to f32. Whole converter/source decoder are unexecuted here.

| Mutable object field | Node/attribute | Initial fallback |
|---|---|---|
| 250 | SUSPENSION/maxWheelLockScale | zeroed record |
| 254 | SUSPENSION/damageToStartEffect | zeroed record |
| 26C | WHEEL/maxMisalignAngle | zeroed record |
| 270 | WHEEL/maxSteerWobbleScale | zeroed record |
| 274 | WHEEL/damageToStartBaseEffect | zeroed record |
| 278 | WHEEL/damageToHaveFullEffect | stored-f32 constant0.95 |
| 27C | WHEEL/damageMinEffect | stored-f32 constant0.1 |

Wheel setup initializes its local20/24 defaults before optional property reads. Both shipped HDMs omit those two attributes. The similarly named damageToStartSteeringWobble source attribute is not read by the inspected9c48c0 path; do not substitute it for274. Outer constructor9f3d30 clears object218..37F before later setup. Fallback/override behavior beyond this inspected path remains open.

Private aligned evidence: `handling-damage-setup-instructions-20261008.json`, `handling-damage-load-instructions-20261008.json`, `handling-damage-default-constants-20261008.json`, three handling-damage string JSONs. Targeted Ghidra output `20261008T1331402853307Z/`, `20261008T1332309391001Z/`, `20261008T1333040046986Z/`; caller survey `20261008T1336570555821Z/`. Existing tools reused; no installation/game/settings modifications.

## Unit thresholds: 0/0 blend (F342)

16 October 2026, Claude Code. Found by `grid-soak` (long Nuerburgring seed 2177,
AI car 6 retired "nonfinite handling effects" at segment tick 6559). Static:
capstone (`local/soak-fixes/disasm/9c5b90.asm`).

- [V] Standard.hdm has suspension threshold +0x354 = 1 (and wheel +0x2F8 and cap
  +0x254 = 1). Car 6's suspension level +0x14 reached 1.0 (levels clamp to
  [0, 1]), so +0x3C0 = +0x35C x (1 - 1) / (1 - 1) = 0/0.
- [D] 9C5B90 computes it at 0x9C5D27..0x9C5D78: `comiss threshold, level; jbe`,
  `comiss threshold, 1; ja`, then `divss` and a `comiss`/`jbe` clamp. An
  unordered compare takes both `jbe` branches, so grid.exe stores the NaN too;
  the 9BD7F0 copier moves it with x87 `fld`/`fstp` (0x9BDC58: +0xD1B0 ->
  +0x2280). Its only readers, 0x9B2C5A and 0x9B42CD, do `comiss x, [0.8]; jbe
  skip`, i.e. they treat the NaN as "not above 0.8". The cap readers 0x9B37F7 and
  0x9B388B (`comiss 1.0, cap; jbe skip`) skip a NaN cap like an uncapped 1. The
  per-wheel effect (+0x2284.., reader loop 0x9B2F50) is used in arithmetic.
- Port: `vehicle_handling::fraction` and the steering cap blend return 0 for
  that 0/0. Every reader then behaves as grid.exe's compare-only readers do with
  the NaN (multiple-shift off, caps off); for the per-wheel effect the original
  would spread the NaN into arithmetic, and zero (no reduction) is the
  recorded deviation. Past a unit threshold x/0 = +inf clamps to 1, as in the
  original. The authored start == end ramp (not in the HDM) still fails as before.
  Test: `saturated_damage_at_unit_thresholds_gives_zero_blend`; the replay of
  seed 2177 on the pre-fix build plus only this change passes all 15 606 ticks.

