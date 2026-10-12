# Collision damage within the handling update

F244 joins the verified impact dispatcher to the complete selected original `9F3E80` handling producer. All four wheel-impact dispatches run before wheel deformation, wear, component queries, gear history and final effects. This closes the previously guarded branch for callers that explicitly supply an owned global damage RNG. Production startup still supplies `None` pending constructor and wheel-region recovery.

## Ownership and ordering

`vehicle_publication::State::damage_random_state` is independent of the car-local wear generator at `1644`. `None` preserves the old below-threshold contract; `Some(seed)` enables reached collision dispatches. The field travels with cloned manager/world state, so a failed frame cannot commit a partial RNG advance. Reset restores the initial owned state. No captured pointer or process-global Rust RNG is introduced.

Aligned original instructions `9F3F7D..9F3FD0` establish the wheel-center forward transform as `(W*T + Y*Ybasis) + (Z*Zbasis + X*Xbasis)`, component by component. Its homogeneous W can be zero or one in the selected owned fixtures. The joined dispatcher admits both; its local region point is explicitly W=1 after inversion. The previous standalone API retains its W=1 contract.

All reached dispatches share and advance the independent global RNG in wheel order. The original delta clamp, strict threshold, dispatcher gains/channel schedule and subsequent car-local wear updates remain intact. The dispatcher receives the same optional query list as the surrounding producer. `None`, present-empty lists, first-match duplicate IDs and statuses 4/5 retain the earlier query semantics. Notification events remain guarded.

## Complete original comparison

All **640 cases match original, repeat, Rust debug and Rust release exactly**, including every car byte, physical byte, query metadata and final global RNG: **87,524,000 bytes per run**. There are 1,280 reached collision calls, 363 cases with global RNG changes, 486 with car-local wear RNG changes and 348 with changed collision levels.

| Cases | Scope |
| ---: | --- |
| 128 | Current source-built Lacetti, five NED regions and current zero gains |
| 256 | Authored active regions/gains/probabilities, simultaneous wheel wear, healthy notifications and owned queries |
| 128 | Authored finite pose bases/translations and homogeneous wheel centers |
| 128 | f32 threshold-adjacent force controls and clamped timesteps |

The native probe executes whole unchanged `9F3E80` and its damage/wear/effect helpers. It owns and canaries the complete car, physical wrapper, component and immutable query receiver. Six mapped data pages, including the global RNG page, are reset per case and restored with protections; the entire mapped image is compared afterward. No code replacements are used. Unrelated car bytes, physical state/padding and query contents are checked unchanged.

An independent Python scheduling projection recomputes all four forward transforms and impact products, then composes the earlier dispatcher/region oracle to check global RNG and collision levels. Full remaining effects are checked against original/repeat/both Rust outputs. The read-only audit rebuilds every request, verifies 14 current pinned inputs and 100 primary files, and checks all 39 framing/value/existing-output guards. Its projection is shared with the validator, rather than a second independently derived oracle.

## Transaction and regression checks

A unit test reaches positive damage and changes both RNGs, then triggers an unsupported fourth-wheel notification after the preceding collision and wear work. Failure preserves the input car, physical body and global RNG. The purchased-source reset test opts in within its private test instance and verifies manager/world retention, invalid-control rollback, initial-seed reset and deterministic replay. Production defaults remain guarded.

All five purchased-source release driving tests pass, including the unchanged clear-straight path, handbrake/reverse, shallow geometry and CQTC boundary behavior. The 600-tick boundary comparison still first differs at tick165, produces 176 body records and ends at Y=-14.807243 versus packed-only Y=-147.81863. These runs do not establish heavy-crash driving with a fully initialized damage model or a lap. All 188 regular library tests, formatting and all-target warnings-denied Clippy pass. The rebuilt standalone dispatcher also reruns all 768 F243 cases: 104,629,440 complete bytes per run remain exact across original/repeat/both profiles, with all 60 guards and the read-only audit passing.

## Failure retained and next work

The first draft incorrectly linked the query receiver through mutable `210`. The first populated-query source case disagreed at the exhaust level and engine effect. The established original query graph uses component `10`; mutable `210` is a different optional owner. Correcting native wiring gives complete agreement with the unchanged Rust query semantics. Draft outputs and the mismatch record remain private; no numeric tolerance was changed. Harmless guessed-file and literal-wildcard reads were corrected through file inventory.

The playable app remains F242, SHA-256 `d167cbf0e719e22406d0d59d7032697d35641dc888d4cbe5510e1d164abb73dd`. It still lacks the outer mutable constructor defaults and four wheel-region setup. Next verify that prelude against original `9F3D30` plus Standard construction, recover wheel-region source stores, then enable the RNG in startup and validate controlled heavy contact. Notification production, full factory/material scheduling, warm caches and visual damage remain separate gaps.

## Private evidence and versions

- `local/behavior/handling-damage-20261013-final-01/validation.json`, SHA-256 `4eadc7c04337879b705e1e44169d8f714fe7a88ff330becd8bbb15adaa545181`.
- `local/behavior/handling-damage-audit-20261013-final-01.json`; draft-01 mismatch and complete draft-02 proof retained.
- Standalone regression `local/behavior/collision-damage-20261013-f244-regression-01/validation.json` and its corresponding audit.
- Native SHA-256 `f1874c814f5495f0ede834e5a449acd13baac5d5a025a59783e1c7c927e79c9f`; Rust debug/release `07a1b6f69f57caccb80a7f5c22aa17ea303353fd64a7758ebf3a62a100b55f81` / `f91614ead9746710cca74ae1ec920200cdd620394b948650ce99b3937ebb749a`.
- `local/behavior/handling-impact-transform-20261013-draft-01.json`; prior original update decompilation `20261008T1328370810021Z/9f3e80.c.txt` and [query receiver proof](HANDLING_QUERIES.md).
- `local/behavior/handling-damage-driving-20261013-final-01.txt`, SHA-256 `16aa72b7a1752a4ed057d0b812073ce338e246ae8bd270af203be1bc0ba66b23`; library/Clippy logs with the same prefix.
- `local/scene/preservation-20261013-handling-damage.json`: all 130 tracked Steam resources and original executable unchanged at 2026-10-12 21:58:02.564551 UTC.

Reused Python 3.12.14, Rust/Cargo 1.99.0, MSVC 14.44.35207/SDK 10.0.26100.0 and Capstone 5.0.6. No installations, desktop input, Steam/settings changes or owner assistance were needed.
