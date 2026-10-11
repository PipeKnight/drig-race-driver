# Idle and clutch transitions through the complete vehicle wrapper — F185

Follow-up: [F186](VEHICLE_PUBLICATION.md) verifies the selected complete `A03220`
publication stage and joins it to the carried vehicle loop.

11 October 2026. Rust now follows the selected assisted-automatic idle/clutch
path through complete `A06A90`, including source contacts, suspension, controls,
engine/wheel forces, assistance and chassis integration. This extends
[F183](VEHICLE_STEP.md) using the disconnected force solve from
[F184](VEHICLE_IDLE_FORCE.md). The Bevy application is still a static viewer;
physical-world advancement/contact scheduling and `A03220` remain open.

## Behavior and scope

`A06A90` derives byte `1640` from handbrake input or low-speed idle before
creating its temporary frame. Early `A03670`, immediately after `9B5E80` and
`9904C0`, changes mode one to mode two when that byte and clutch assistance are
set. When it clears, mode two calls `98E1B0` to engage mode one. Rust now runs
that transition at the same point, before speed history, fuel, aero and controls.

`98E1B0` derives engine inertia at the current gear ratio and sums the actual
driven wheels' inertias and speeds. It redistributes their speed difference
when the wheel/engine direction or engine-speed threshold requires it; otherwise
it changes only the mode. The threshold constant is exactly f32 `3F8CCCCD`.
Mode two is now accepted by the existing control wrapper and reaches the F184
engine acceleration equation instead of being rejected by an old mode guard.

This contract retains non-neutral gears 1..9, automatic transmission and clutch
assistance, inactive respawn/nitro and the previously bounded source/contact
configuration. Neutral/manual operation is still rejected. Ownership and
interframe construction/history are authored fixtures; this is not proof of the
complete original constructor or world scheduler.

## Verification

Both debug and release match complete unchanged original `A06A90` and a repeated
original execution across **428 calls**, including 96 zero-delta calls and 192
independently carried positive updates. Each profile compares **4,163,584 full
car bytes / 4,742,240 observable bytes**. The retained observable scope is the
full `2600` car, framing, materials and globals; expired/uninitialized native
temporary view/solver bytes remain excluded for the reasons recorded in F183.

Coverage includes 201 mode transitions, 213 mode-two outputs, 249 engine-speed
changes, 1,316 wheel-spin changes and 391 contact slots. Source setup reads all
44 supported car tables. Runtime mode inputs are authored after the verified
mode-one source initialization, not injected into its narrower adapter.

The same path passes **332 linked synchronization→whole wrapper→publication
updates per profile**: 18,221,488 synchronization bytes, 3,229,696 driver car
bytes and 3,678,560 driver observable bytes. Each branch carries its own car and
physical state. Publication still performs no advance/correction, with empty
shapes and zero deformation dimensions. Its 201 mode transitions/197 mode-two
outputs therefore establish composition, not a world tick or lap.

Fresh engaged-path regression passes another 428 whole-wrapper calls per
profile. All 36 paired native/Rust guard outcomes pass in both wrapper suites,
including manual-clutch rejection, zero engine inertia and engagement overflow.
A new late engagement-overflow test verifies complete preparation rollback.
All 146 library tests, formatting and all-target warnings-denied Clippy pass.

## Evidence and failures

- Final reports under `local/behavior/whole-vehicle-{idle,idle-linked,engaged-regression}-{debug,release}-20261011-final-04/validation.json`.
- Whole entry-aligned `98E1B0`: `local/behavior/idle-transition-aligned-20261011.json`;
  private constant evidence `idle-transition-constants-20261011.json`.
- Existing early caller/helper surveys were reused. The preliminary caller
  range starting `A0387F` was not used as proof of an instruction boundary;
  existing whole-caller aligned evidence establishes schedule order.
- First fixture failed safely at the source initializer's mode-one guard.
  Mode input moved to runtime after initialization. The next attempt correctly
  hit the Rust control wrapper's old mode-one guard; its selected scope was
  extended, then all final comparisons rerun. No numerical mismatch/native fault.
- A malformed Rust test literal (`1.e-30`) caused a compile error, fixed to
  `1.0e-30`. Clippy flagged operand-order-preserving sums; a local allowance
  documents the observed SIMD order. Both failures preceded final verification.
- Guessed A03670/shared-header paths failed harmlessly; actual files were found
  with `rg`. No desktop inputs, settings changes or tool installs after F184.

Reused Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207/SDK 10.0.26100.0 and
Capstone 5.0.6. Original code and seven protected driver-global pages are checked
unchanged/restored in every whole native run. Private capture and proprietary
source data remain ignored. Next join the remaining outer publication/world
schedule, then complete source construction and Bevy driving controls.
