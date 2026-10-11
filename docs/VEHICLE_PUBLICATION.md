# Complete selected outer velocity publication — F186

F187 correction: the complete synchronous `A09A80` final local masks clear linear X, angular Y and position X/Z; the earlier W-only interpretation was incorrect. `A03220` itself still clears velocity W. See [whole wrapper evidence and live-job distinction](VEHICLE_OUTER_WRAPPER.md).

11 October 2026. Rust now matches one unchanged original `A03220` call and its
composition with synchronization, the complete idle-capable vehicle wrapper and
final pose publication. This closes the selected outer velocity-publication
stage. Complete `A09A80`, world advancement/contact scheduling and the drivable
Bevy application remain unfinished.

## Observed behavior and contract

The original `A09A80` invokes `9BD7F0`, `A06A90`, then `A03220(delta,0)` before
its final pose and velocity setters. The new stage owns the full `21000` car
buffer and a linked `424` physical body. The notification argument is zero.

Positive timer `C798` decreases without clamping. When it is nonpositive, `C790`
halves and clears below exactly f32 `3A83126F`; `C794` clears and `C780` becomes
minus one. This expired-state work also runs at zero delta. `C784` receives the
original callback identity. Linear/angular velocity W becomes positive zero
before both linked setters; an unused nonfinite W is therefore accepted and
cleared. XYZ must be finite.

The selected branch keeps wheel render links, body render link and visual
update inactive; UI mode is not one. `A03030` runs with an empty manager list and
stable setup mode/flag, so it does not rebuild setup. Both its independent and
dependent stable setup cases are tested. This is a complete original call under
explicit branch restrictions, not proof of the omitted rendering/UI/setup work.

The stored callback address is an opaque reference tag, never executed by Rust.
The native adapter checks the generated pointer against the loaded image's
`9A0C90` location before normalizing its relocation. Input wrapper relocation is
also checked and restored. No original code is modified or replaced.

## Validation

Both debug and release match original/repeat/Rust across **256 complete calls /
34,875,392 record bytes per profile**, including 128 independent carried calls
and 50 zero-delta calls. Full car and physical buffers are compared. Coverage
includes 78 timer changes, 116 decay changes, 119 linear and 119 angular W clears,
and 144 physical velocity changes. All 30 paired native/Rust guard outcomes pass.

The joined loop compares **332 updates per profile** through whole `9BD7F0` →
whole `A06A90` → whole `A03220` → final pose/velocity setters and readback. Each
branch carries its own full wide car and physical state, including decay timers.
It compares 45,229,024 outer-publication bytes, 18,221,488 synchronization bytes,
3,229,696 driver car bytes and 3,678,560 driver observable bytes per profile.
All 44 supported source tables initialize independently. Coverage includes 201
mode transitions, 213 mode-two outputs and 1,284 spin changes. The driver retains
its explicit exclusion of expired/uninitialized native temporary view/solver.

This remains a composition: final setter notification is disabled, shapes and
manager list are empty, and no advance/correction occurs. Construction, history
and control inputs are authored. The full `A09A80` call, its `9A0` reset and
notification-enabled final setter are the next stronger comparison boundary;
world collision scheduling and lap behavior are not established here.

All 147 library tests, formatting and all-target warnings-denied Clippy pass.
The zero-delta/unused-NaN-W unit confirms that the input remains unchanged.

## Evidence and failures

Private final reports:
`local/behavior/vehicle-{publication,outer-publication}-{debug,release}-20261011-final-03/validation.json`.
Aligned whole functions and constants:
`vehicle-publication-aligned-20261011.json` and
`vehicle-publication-constants-20261011.json` under `local/behavior/`.

The first comparison retained 32 callback-pointer word differences, one per
case; all other words matched. The loaded PE had been relocated. Explicit
generated-pointer checking/normalization fixed the comparison, and all final
tests were rerun. There was no numerical mismatch or native fault. No desktop
input, settings change or tool installation occurred during F186.

Reused Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207/SDK 10.0.26100.0 and
Capstone 5.0.6. Each native call checks executable sections unchanged and restores
five protected data pages, including the owned-empty manager count. All original
assets, captures, generated buffers and executable evidence remain ignored.
