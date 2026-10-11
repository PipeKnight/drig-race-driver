# Disconnected drivetrain force solve — F184

11 October 2026. This extends the prepared four-wheel force subsystem to mode
two, observed while the Lacetti idles on the Donington Grand Prix starting grid.
The complete idle driver and its transition to driving remain separate work.

## Observations and implementation

The private compatible GRID copy reached Race Day using the existing guarded
AutoHotkey macros. The selected car/circuit were Chevrolet Lacetti/Donington
Park, Grand Prix Circuit, three laps. The assists page showed traction control,
braking assist and stability control On, automatic transmission; no assist
settings were changed. Only menu/resume/pause controls were used during capture.
GRID was visually paused after the debugger detached.

At `9DF8B0` entry the idle car has mode two, gear one, clutch flag `1640` set,
zero throttle/brake and four material contacts. Its temporary view and solver
were captured synchronously, never read later through expired stack pointers.

Compared with mode one, the original allocates an engine acceleration variable
before the wheel variables. It fixes drivetrain torque to zero and adds a
separate equation using gear-scaled engine inertia. Engine speed advances from
that acceleration; driven wheel inertia stays unchanged. The mode-one
wheel-derived engine-speed clamp is skipped, and the final engagement flag
comes from temporary view offset four. Failed solves also fix engine
acceleration to zero. Rust now follows those branches and arithmetic order.

`9B4730` sets view byte `38` to one. This prevents the drivetrain engagement
event in `9DF540` throughout this selected whole-force call; wheel-stop events
remain active. Supporting mode two here does not establish the separate mode
transition helper `98E1B0` or whole idle `A03670`/`A06A90` execution.

## Validation and limits

Both debug and release match unchanged original code and a repeated original
execution across 288 varied calls plus 32 independently carried sequences of
12 calls: **672 calls / 10,338,048 complete record bytes per profile**. All
record bytes, including shared diagnostics and retained sparse storage, are
compared in these isolated replays. Coverage includes 216 engine-speed changes,
252 velocity changes, 116 wheel-stop markings, 180 sparse compactions and all
288 authored cases preserving wheel inertia. No singular solve occurred in
this set; singular equivalence is not newly established by this coverage.

The successful live call matches all body/view/solver/material/engine-limit
bytes. It differs in 27 of the 32 shared friction diagnostic bytes. All four
wheel spins are below the threshold for this car to write those diagnostics,
and isolated replay leaves them unchanged. Another updater writing the globals
between stops is the current interpretation; the exact writer was not traced.
The validator's explicit `--shared-diagnostics` option excludes only those 32
bytes from live expectations, requires idle mode two, and checks that isolated
replay leaves them unchanged. It still requires original/repeat/Rust equality
over every byte. The unscoped live test failed and remains recorded; there is
no claim that the entire live record matches.

Fresh mode-one regression passes six live calls plus the same 672 varied/carried
calls per profile, with every live byte matching. Mode-two checks pass 18 native
and 22 Rust rejection cases; mode-one passes 15 native/19 Rust checks. Zero,
negative and nonfinite engine inertia are rejected before mode-two original
execution. A late equation-overflow unit verifies complete Rust rollback after
temporary initialization. All 145 library tests, formatting and all-target
warnings-denied Clippy pass.

## Evidence, failures and tools

All captures and numeric reference data remain in ignored `local/`.

- Successful capture: `local/behavior/wheel-force-mode2-live-20261011-02/`;
  completed/code-exact/hardware-resume-before-detach true, debugger exit zero.
- Earlier six-call attempt `wheel-force-mode2-live-20261011-01/` retained four
  complete calls but stopped unexpectedly at return `A04182`; completed false.
  Cleanup detached, and the game remained usable. Stale queued hardware return
  notification is a hypothesis, not an established cause. The bounded single
  call retry succeeded; failed capture was not promoted to completed evidence.
- Final reports: `local/behavior/wheel-force-mode{1,2}-{debug,release}-20261011-final-03/validation.json`.
- Static entry-aligned evidence: `mode2-force-aligned-20261011-02.json`, plus
  `9DF330`/`9B5370` entry ranges in `mode2-force-aligned-20261011.json`.
  Preliminary midinstruction ranges in the latter are superseded, not used.
- Fresh session/macro identity: `local/reference-race/idle-20261011/`; private
  GRID PID5092, macro PID38112. No driving input or Steam modification.

Reused Python 3.12.14, Rust/Cargo 1.99.0, MSVC 14.44.35207/SDK 10.0.26100.0,
Ghidra 12.1.4/JDK 21.0.9, Capstone 5.0.6 and AutoHotkey 2.0.29. x32dbg SHA-256
`28e05e36462a74e5561c45377d417058c2c6a3cf0f27bb0bda1f47ee1763de65`;
analysis image remains `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
No tools were installed. Next: idle/clutch driver integration, then physical
contact/publication scheduling and the first Bevy driving loop.
