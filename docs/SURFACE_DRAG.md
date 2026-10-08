# Four-wheel surface slowdown (F112)

Recorded 8 October 2026. `grid_world::surface_drag::apply` independently reproduces whole original `009dfc50` for four wheels, supplied contacts, relative levers, body state, speed and additive slowdown scalars. It returns updated linear/angular velocity and four wheel spins. This covers actual impulse application within this consumer; the complete vehicle update remains separate.

**620 cases / 7,440 output words exact in debug and release**: 24 analytic controls, all 84 installed MECHANICS slowdown scalar sets and 512 seeded states. Controls cover absent/mixed contacts, conditional flag+73, speed thresholds, positive/nonpositive wheel spins, zero delta, negative coefficients, varied four-lane levers and arbitrary finite body bases. The source mechanical values are independently decoded; the original source loader is not invoked in this comparison.

With contact, the coefficient is `slowdown * clamp((speed-minimum)/(maximum-minimum), 0, 1) + body[22b4+ordinal*4]`. Without contact it is zero. When every contact has material+73 set, the ramp instead uses `clamp((speed-3)*stored_f32(1/3), 0, 1)`. The source material initializer/loader explicitly clears +73; authored true cases test conditional arithmetic, not established live activation.

The consumer sums the four coefficients left-to-right, multiplies by 0.25 and stored f32 `0.949999988079071`. Each coefficient then becomes its own value times stored f32 `0.050000011920928955` plus that shared amount. This can apply slowdown at a wheel lacking its own contact when another wheel contributes a positive shared amount.

For each positive blended coefficient, in ordinal order:

1. Compute current point velocity as body linear velocity plus angular velocity crossed with the supplied relative lever. The lever comes from the body's pointed view at +250+ordinal*10; its source generation is still open.
2. Normalize its four-lane squared length using SSE reciprocal-square-root seed and the original refinement `(1-(length*seed)*seed)*(seed*0.5)+seed`.
3. Multiply the direction by `-0 - mass*coefficient*delta` and apply the original angular/linear impulse response.
4. If wheel spin is positive, subtract `(coefficient/radius)*delta`, clamping only the lower result to zero.

The original spin update precedes impulse application, and the implementation preserves that order. Each impulse changes the body state read by the next wheel. Parallel impulse evaluation against one starting velocity would produce different behavior.

Aligned original instructions at 9dfcfb..9dfd0f initialize AL to one, clear it on the first of seven iterations, and branch past a stabilization arm when it is zero. This arm is unreachable under these instructions and is not implemented as an active feature. This observation comes from the byte-level loop, not merely the decompiler's unreachable-block warnings.

Evidence under `local/behavior/`: `surface-drag-20261008-01/validation.json`, `02/validation.json`; `20261008T0017248161799Z/9dfc50.c.txt`; `surface-drag-instructions-20261008.json` (use 009dfc50 through return009e01d5); `surface-drag-constants-20261008.json`. Original material loader/initializer evidence: `20261007T1610340353294Z/`. Probe SHA **7a873b985de7d1b45f620037a5e991ad0a200682988c1999cb8849cee198f5ca**. The entire consumer and its original cross/impulse helpers execute without code patches or stubs. The private lazy-normalization data page E58000 is writable during execution, then its original protection is restored. No live injection or original-install modification.

**95 workspace tests**, formatting and warnings-denied Clippy pass: `surface-drag-tests-20261008.txt`, `surface-drag-clippy-20261008.txt`. No numerical failures. After separate line-ending normalization, restoring the module initially failed the final formatting check on module order only; Cargo fmt corrected it and the check passes. A failed shell wildcard search was corrected without changes. Existing MSVC 14.44.35207/SDK 10.0.26100.0, Rust 1.99.0, Python 3.12.14, Ghidra 12.1.4/JDK 21.0.9 and Capstone 5.0.6 reused. No tools installed or game/settings/save changes.

The public boundary requires four wheels, finite inputs, positive mass/inertias/radii, unequal material speed bounds, delta within zero-to-0.1 and nonzero finite point velocity when a positive slowdown is applied. Original zero-length normalization can produce nonfinite results; those states are rejected. SSE seed agreement is verified on this PC, not all architectures/processors. Body/speed/contact/additive initialization, live flags, main cadence and other production force stages remain open.
