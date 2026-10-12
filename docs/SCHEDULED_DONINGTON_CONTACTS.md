# Scheduled Donington contact validation

F248 adds reproducible staged controls to headless source driving and verifies six actual Donington approaches. All24 runs (six schedules, two profiles, two repeats) complete600 ticks each: **14,400 simulation ticks**, with exact agreement in per-tick TSV and complete selected initial/final buffers. This tests source geometry, vehicle dynamics, contact solving and controls together. It is not an original-game trajectory comparison or a lap.

## Controls and results

All runs start from source Lacetti/Donington slot00 at seed200679 with the experimental packed-track/CQTC body mode. Full throttle applies unless the final braking stage says otherwise. One tick is the existing0.016-second simulation step.

| Schedule | First body-contact tick | Speed immediately before contact, km/h | Total body contacts | Maximum sampled wheel-impact/threshold ratio |
| --- | ---: | ---: | ---: | ---: |
| Steering0.25 for600 ticks | 165 | 69.38 | 176 | 0.17324 |
| Steering1 for600 ticks | 122 | 34.08 | 46 | 0.21082 |
| Straight180, steering1 for420 | 231 | 96.89 | 467 | 0.18105 |
| Straight300, steering1 for300 | 351 | 147.21 | 350 | 0.18883 |
| Straight300, steering-1 for300 | 374 | 142.31 | 30 | 0.23552 |
| Straight240, steering0.5 for240, brake120 | 305 | 135.57 | 102 | 0.20583 |

All six finish with finite movement and final Y above-30. Speed is simulated velocity magnitude converted using the prototype's meter/second interpretation; original-game handling calibration remains open. Contacts are generated records across ticks, not distinct impacts. The baseline matches the600-tick rendered app result from [F246](SOURCE_WHEEL_DAMAGE_STARTUP.md).

No run advances global damage RNG or changes the nine sampled published handling-effect words. Post-tick wheel-force operands remain below the mass-scaled wheel-damage threshold: maximum ratio0.23553. This observation does not inspect every intermediate handling input. These are useful high-speed contact tests, but they do not demonstrate the reached body-damage path. [Separate callback research](BODY_IMPACT_CALLBACK_GAP.md) identifies the missing connection; F247's reached authored-wall wheel-damage proof remains distinct.

## Reproduction on the owner's PC

`grid-drive-plan GAME_ROOT PRIVATE_CD01 PLAN.txt local/NEW_OUTPUT_DIR` reads purchased source resources and uses `Runtime::prepare_body_geometry`. Plans contain one whitespace-separated row per stage:

```text
# ticks throttle brake steering handbrake(0/1)
300 1 0 0 0
300 1 0 1 0
```

Plans admit at most64KiB,1024 stages and36,000 total ticks; controls must be finite and within their ranges. Output must be a new directory under canonical ignored `local/`; existing directories/files/links are preserved. The command retains its plan, an initial state, every successful tick's telemetry and either final state or last committed state plus error. A failed run exits nonzero. It does not synthesize desktop input or run the original game.

DP01 snapshots have a36-byte header (tag, four word counts,64-bit tick, RNG-present word, global RNG), followed by complete car/physical/force-view/solver buffers. This excludes other world/manager/cache metadata. TSV records every tick, control stage, pose/velocity, body count, global/wear RNG and raw threshold operands. `damage_words` names nine published handling-effect words at mutable3A0..3C0; it is not a dump of all region levels. Wheel-force fields are raw f32 words. Neither TSV decimal coordinates nor these selected snapshots establish whole-world bit identity.

`tools/validate_source_boundary_plans.py` runs all six plans in debug/release twice. The current-hash audit reconstructs plans/control sequences, rereads all96 primary files, rechecks all profiles/repeats and14 rejection/preservation controls, and hashes130 Steam reference resources/executable. The audit shares the metrics decoder; it is not a second independent numerical oracle.

## Evidence, failures and tools

- `local/behavior/source-boundary-plans-20261013-final-02/validation.json`: SHA-256 `70f575212c436773e89403271415220870e62b27ff2a1a8a7aacca22400b2170`.
- `source-boundary-plans-audit-20261013-final-02.json`: SHA-256 `82cfafb96bedea4d119c9b6f6dc42ea3067dc9540e55a438f8b21d3b8277f55c`.
- Diagnostic debug/release SHA-256 `072e849ea4d6727f3c5ff8848eca4973c47024eeac890399a7625961bf4a640e` / `1a94688a511988bbf04e8764aa7c0820e5eaa1cc4c56cf273da339989e9a4b68`.
- First exploratory six release runs remain in `source-boundary-plans-20261013-draft-01/`. First full sweep stopped at the right-turn sample because the validator incorrectly assumed a16-contact body cap. The verified stable pool has20 total slots with a variable wheel prefix. Corrected validator bound20 and fresh final02 sweep pass; no physics or runtime guard changed.
- The first audit reconstructed LF plan bytes, while Windows text output used CRLF. Reconstructing native line endings fixes the audit; complete recorded outputs remain unchanged. A formatted-code patch context failed atomically and was corrected; harmless wildcard/guessed-file reads were retained as failed commands.
- Debug/release build and warnings-denied all-target Clippy logs use `source-boundary-plan[s]-*-20261013-final-01.txt` under `local/behavior/`; formatting, both Python syntax checks and whitespace checks pass. No production physics changed in F248, so previously passing F246 source/library/Bevy suites were not redundantly rerun.
- `local/scene/preservation-20261013-source-boundary-plans.json`: all130 tracked resources and original executable unchanged at2026-10-12 22:43:44.083084UTC.

Rust/Cargo1.99.0 and Python3.12.14 reused. No installations, UI input, game-settings or Steam changes. The playable F246 app remains SHA-256 `2dad35aca0379be2a48f55b9b5dbe99cbcd8916be4fd3b2636c14f1f0be53261`. Next recover and validate source body-impact event construction/dispatch and integrate it transactionally; arbitrary crashes, notifications, deformation and full lap validation remain unfinished.
