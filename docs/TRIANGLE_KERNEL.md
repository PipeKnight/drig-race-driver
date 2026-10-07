# Original triangle primitive comparison

Recorded 6 October 2026. Independently implemented f32 triangle intersection now matches the isolated original GRID numeric routine **bit for bit in all 568 tested cases**. This is our first executable-level behavior comparison beyond parsing. It does not establish equivalence for the full collision/contact solver, the running game's floating-point configuration or vehicle handling.

## Reference identification and execution

Target `0x00adc290` takes a start vector, finite-segment displacement, three triangle vertices and outputs fraction plus two barycentric coordinates. Its only observed calls are to pure cross-product helper `0x0067c200`. Targeted Ghidra results were checked against bounded original x86 instruction inspection before execution. Original code and disassembly remain private under `local/behavior/`.

`tools/probe_triangle_kernel.c` is an independently authored 32-bit research harness, built with the already installed MSVC x64-to-x86 compiler, `/W4 /WX /O2 /MT /std:c17`. It validates the private unpacked image SHA-256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`, PE architecture, section ranges and image size. It maps that image in its own process, makes code executable/read-only and data read-only, and calls the inspected arithmetic routine only. It does **not** run the game entry point, import initialization, TLS callbacks or constructors, and does not open or modify the running game. Input/output files and compiled artifacts remain private; output creation refuses replacement.

The first preferred-address attempt failed before any original instruction executed: requested mapping `0x00400000` conflicted with existing private-process allocations (`VirtualAlloc` error 487). A diagnostic retry confirmed the mapping stage. The harness now applies bounded PE HIGHLOW relocations when it must allocate elsewhere. The subsequent relocated eight-case smoke run succeeds, followed by the full comparison. This adjusts mapped address operands in private memory; source files remain unchanged. Exception handling records failures and prevents crash dialogs. No machine security/locale setting was changed.

Each run records MXCSR, CRT floating-point control and mapped image base. The tested default environment was MXCSR **0x00001f80**, control **0x0009001f**; this is an isolated-process measurement, not a capture from the running game.

## Observed primitive contract

The original routine is two-sided Moller–Trumbore with f32 arithmetic. Stored constants establish absolute determinant thresholds **−0.0000009999999974752427 / +0.0000009999999974752427**, barycentric/fraction lower bound of the same negative value, and upper bound **1.0000009536743164** (`0x3f800008`). Accepted fractions slightly below zero or above one are returned without clamping. A direction is a displacement for one segment, not a unit ray. Normal computation and shape/leaf selection happen outside this primitive.

Independent `grid-world/src/triangle.rs` implements the conventional algorithm using these observed bounds. It does not contain game-derived pseudocode or machine code. The existing f64 BVH query retains its separately documented robust geometric contract; this f32 primitive is a distinct comparison tool until whole-query semantics are resolved.

## Evidence

**568 / 568 classifications and output bits match**, with maximum numeric error zero. Cases comprise eight analytic smoke cases, **543 actual Donington triangles selected by the validated ground queries**, and 17 edge/fraction/scale cases. The analytic cases include both segment directions, misses, parallel casts, start/end contact, a vertex and a sloped plane. Boundary and small-scale cases exercise the recovered absolute tolerances. Track inputs are independently decoded from original JPK/VCQTC bytes; starts/displacements are explicitly rounded to f32 before both implementations receive identical decimal input.

Private evidence: `local/behavior/triangle-kernel-validation/validation.json` with executable/source hashes, floating-point environment, complete original/native bit rows and successful process results. Script: `tools/validate_triangle_kernel.py`; CLI: `grid-triangle`. Earlier mapping failures, instruction survey, constants and eight-case smoke outputs are retained separately. Two synthetic Rust tests check analytic intersections, observed edge/fraction tolerance, small determinant rejection and invalid inputs.

Next: inspect the running reference's floating-point state and actual shape/query selection, then connect this primitive to independently indexed geometry with clearly documented traversal/tie differences. Wheel clearance and contact-force integration still require separate evidence.
