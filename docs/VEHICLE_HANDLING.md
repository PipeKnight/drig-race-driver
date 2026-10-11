# Mutable handling and notification-one publication

11 October 2026, F191. Rust reproduces the complete selected `9F3E80` mutable
handling update and one unchanged `A03220(delta,1)` call. This closes the larger
handling stage required by the registered car job. Complete car-job execution,
component notifications, geometric collision production, persistent physical
world state and Bevy driving remain open.

## Order and ownership

The mutable object is embedded at car `CDF0`. Its `214` component links back to
the same car through `24`; car `DB0` links to the physical body. Both activation
bytes `B13/B14` must be set. The selected owned component has mode zero and an
absent query object at `10`; wire pointer tokens are zero and never followed.
The original probe supplies exact owned links before its sole whole-function
call and checks/restores them afterward.

The whole handling sequence clamps delta to stored f32 `3CA3D70A` (approximately
0.02), checks four supplied collision values through `9E6660`, evolves visual
wheel levels with `9F2F80`, applies wheel wear and probability normalization
through `9C4B70`, normalizes the remaining probability pairs, updates body wear
and rollover through `9C4EB0`, updates gear history/damage through `9C5250`,
publishes all selected `9C5B90` effects and checks terminal wheel state through
`9E65B0`.

Aligned instructions resolve an unreliable decompiler inference: the timestep
remains in the x87 stack across wheel helpers; the integer result of `9C4B70`
does not replace it. The same clamped delta reaches subsequent wheel/body wear.
The wear branch advances its unsigned LCG even at zero delta when its threshold
is exceeded. All six style selections, active/inactive wear, visual correction,
normalizations, rollover entry/recovery, gear changes and unchanged gear timers
are included in the comparison.

The collision guard compares `car1AC8[wheel] * clamped_delta` against the stored
f32 product of physical mass `3F0` and mutable `368`. Above-threshold damage
production is rejected before execution. Its unimplemented spatial distribution
and notification paths are not approximated by this API.

`A69630` reads the published matrix at physical `1D0`; its second-row Y is `1E4`.
`A69820` reads physical `210`. Earlier provisional descriptions of that getter
as linear velocity were misleading: the established publication maps angular
velocity to `210` and linear to `220`. The rollover recovery uses the four lanes
actually read at `210`. Preserve these observed offsets and avoid relabeling
them based on an expected physical interpretation.

`vehicle_handling::publish` joins existing outer decay/velocity publication to
the complete mutable update. Whole `A03220(delta,1)` publishes velocities first,
then calls handling, then stable `A03030` setup and inactive rendering. Outer
timers retain the caller's full delta; the handling producer clamps its own
copy. No final synchronous `A09A80` axis projection or pose setter is inserted.

## Validation

Each variant matches **396 calls / 53,947,872 complete record bytes** in both
debug and release: original, independent repeat and Rust are byte-exact. Each
record includes full `21000` car, `424` physical body and input delta. The 256
initial cases carry arbitrary unconsumed storage; 128 updates independently
carry each branch's own output. Twelve additional controls prove identical
handling, RNG and physical results at delta 0.02 and 0.1 in six cases.

Standalone coverage includes 84 wheel-level, 177 wear-level, 198 RNG, 298
body-wear, 30 rollover, 209 gear-damage, 273 gear-history and 300 effect changes.
Publication coverage includes 86 wheel-level, 311 body-wear, 199 gear-damage,
269 gear-history, 321 effect and 284 physical velocity changes. Both include
53 zero-delta and 110 above-cap calls. Complete physical storage remains
unchanged standalone; publication changes only its specified velocity lanes.
Owned component identity and inactive notification state are checked separately.

Standalone has 42 paired semantic-guard outcomes and 10 framing outcomes per
profile; publication has 52 and 10. Malformed, truncated, trailing, oversized,
foreign-owner, active collision, terminal/notification, style, zero divisor and
nonfinite inputs leave no accepted output. A late zero-length steering ramp
test fails after earlier wear/gear changes and proves complete rollback. The
same test covers publication after timer and linked velocity writes.

Fresh notification-zero `A03220` regression matches all 256 calls /
34,875,392 bytes and 30 guard outcomes per profile after rebuilding the shared
native adapter. All **153 library tests**, formatting and all-target Clippy
with warnings denied pass. First numerical comparisons passed; four Rust
borrow errors, test literals and exact-operand-order Clippy warnings were fixed
before final runs. No native fault or numerical mismatch occurred.

Native adapters execute intact RX reference code, without imports, TLS or entry
point execution. Five lazy-data pages are snapshotted/restored; all executable
sections are checked unchanged. Full owned linkage and retained physical/body
storage are checked; partial invalid output is removed. Proprietary executable,
numeric fixtures, captures and decompilation stay ignored under `local/`.

Final private evidence:

- `local/behavior/vehicle-handling-{debug,release}-20261011-final-01/validation.json`
- `local/behavior/vehicle-handling-publication-{debug,release}-20261011-final-01/validation.json`
- `local/behavior/vehicle-publication-{debug,release}-20261011-f191-regression-01/validation.json`
- `local/behavior/vehicle-handling-final-audit-20261011.json`
- `mutable-handling-{aligned,getters,constants,steering-aligned}-20261011.json`
- `vehicle-notify-publication-aligned-20261011.json`
- Ghidra survey `20261011T1000416928472Z/` and earlier `20261008T1328370810021Z/`

## Actual race observation and limits

Read-only, hash/path/PID/start-guarded snapshot at 10:07:20.367608 UTC from the
existing private GRID process covers all twelve registered cars and their exact
backlinks. All sampled collision products are below the mass threshold at the
maximum allowed handling delta; all wear inputs are below their 999999
threshold. Terminal wheel flags are zero and status is seven. However,
notification byte `442` is **on** and the component query pointer is **nonzero**
in all twelve. This comparison explicitly owns those branches as inactive and
does not claim to reproduce their live behavior. The non-atomic paused snapshot
does not prove tick cadence, callback outcomes or arbitrary racing states.

`tools/inspect_reference_outer.py --handling` records these bounded fields and
raw reads in `local/behavior/mutable-handling-live-20261011.json`. No game input,
settings, installation changes or new tools were required. Python3.12.14,
Capstone5.0.6, Ghidra12.1.4/JDK21.0.9, Rust/Cargo1.99.0,
MSVC14.44.35207/SDK10.0.26100.0 and pinned Bevy0.19.1 are reused.
Original Steam executable and all130 traced resources remain unchanged at
10:18:21.115636 UTC (`local/scene/preservation-20261011-vehicle-handling.json`).

Next join whole synchronization → complete driver → notification-one publication
inside the actual `A0A290` job, preserving owned job/completion identity. Then
close remaining query/notification contracts and the car-to-world geometry and
persistent-state boundaries. Supplied initialization and scoped whole calls do
not establish a playable lap; earlier completion estimates are unchanged.
