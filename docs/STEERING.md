# Steering input and wheel coefficients

Recorded 8 October 2026. This specification describes supplied state; source tuning, device input and live timing remain separate questions.

## Input conditioning (F116)

The aligned call at 9b3147 passes the body in EAX and a pointer to a private command copy in ESI. Original 98f790 changes that copy, leaving body+e30 intact. The command is bounded to [-1,1]. A positive body+2294 enables a conditional perturbation: multiply the supplied body+159c by stored-f32 constants 8 and 5.3, evaluate original double sine, convert each result to f32, multiply them, and use negative absolute product below the 0.5 threshold. Its gain is `(2294+1)*0.5`, scaled by `clamp(1648*0.2,0,1)` and stored-f32 0.14. Each arithmetic operation rounds to f32 before the next operation. Input is added before the final clamp.

`grid-world::steering_input::condition` reproduces this helper. Whole original 98f790 and its actual trigonometry match **1,351 cases /1,351 words exactly in debug and release**. Controls cover perturbation enable/0.5 boundaries, both clamps, signed zero, speed threshold, small/large signed phase values and 1,024 seeded authored cases. Phase values are bounded to +/-10000; this does not prove equivalence of arbitrary platform math libraries. Finite-input/result guards are authored diagnostic bounds.

Evidence stays private: `local/behavior/steering-input-20261008-01/validation.json` and `02/validation.json`; probe SHA `edc905aba2c5a0821ab263d0d2e8934ca3d368d4fea51209ea4a0acc3296fbe0`. Original analysis PE SHA `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. The mapper keeps code RX and mapped data read-only: no code patches, stubs, mapped data writes, game injection or original entry-point execution. CLI `grid-steering-input` accepts four f32 words and emits one to ignored local output.

Static inspection: `steering-instructions-20261008-04.json`, prior `steering-instructions-20261008.json`/`-full.json`/`-final.json`, and targeted Ghidra `20261008T0136288512690Z/`. The first inspection stopped at the helper's first return and omitted its upper-clamp arm; bounded full instruction inspection corrected that omission before final documentation. Original 9b3110's decompilation removes branches and hides register arguments, so its pseudocode alone is insufficient for assisted steering.

Existing Ghidra12.1.4/JDK21.0.9, Capstone5.0.6, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0 and Rust1.99.0 were reused. Initial Ghidra run failed writing its external Java-path preference under the sandbox; the authorized escalated retry completed. Initial unit-test compilation needed an explicit f32 expected value; corrected. Final **96 workspace tests**, fmt and warnings-denied Clippy pass, `local/behavior/steering-input-tests/tests-final.txt` and `clippy.txt`. No numerical failures, new tools or game/user-setting changes.

## Update order and remaining wheel work

On the inspected a03670 path, 9b2790 prepares incoming force vectors at a03d55, a00e10 runs at a03dca, 9dfc50 applies slowdown at a03dda, and 9b40d0 runs later at a03f3e. That wrapper calls 9b3110 at 9b4163 with body and delta stack arguments. Thus the later coefficient writer must not be moved before the already verified force preparation when composing this path. This is static order on this path, not a synchronized live capture or universal scheduling claim.

The input helper does not establish full steering. Next compare whole 9b3110 with body+e6c false: supplied geometry, paired wheel identity, coefficient caps, per-wheel rotations, smoothing and frozen flags. Its assisted branch, tuning initialization and the physical meaning of body+159c/+2294 remain open. Neutral reset 997690 initializes wheel+20=1/+24=0; it is not the later writer.
