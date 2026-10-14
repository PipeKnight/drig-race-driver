# Autonomous source-physics practice laps

F272 adds a headless diagnostic driver to validate the complete Lacetti/Donington driving loop without desktop input. It reads the purchased centerline and chooses ordinary throttle, brake and steering inputs. Every movement comes from the existing car, tyre, ground, physical solver, body-contact, registered-damage and timing paths. It never teleports the car or supplies replayed positions. This is an independently authored test controller, not original GRID AI or an opponent implementation.

## Results

| Maximum-speed setting | Successful ticks | Practice lap time | Runs checked | Maximum lateral distance from centerline | Minimum wheels with ground hits |
|---|---:|---:|---|---:|---:|
| 18 m/s | 14,632 | 3:51.887 | Debug, release and two retained release repeats | 3.629 m | 4 |
| 28 m/s | 12,080 | 3:11.056 | Two release runs | 3.629 m | 4 |
| 40 m/s | 11,612 | 3:03.558 | One release run | 9.286 m | 3 |

All seven runs complete one valid practice lap with no simulation failure, terminal wheel damage or body/barrier contact. The 18/28 settings retain all four wheel-ground hits at every tick. The 40 setting has a larger lateral excursion and briefly fewer ground hits; it is a stress observation, not a clean racing line or handling-fidelity claim. The speed settings are caps for the diagnostic controller, not average speeds. It slows for bends using an authored 3.5 m/s² lateral-acceleration target.

All these ordinary-road runs retain zero deformation bulk calls/records, zero moved grid positions, initial deformation CRT seed `00000001` and initial body-event damage RNG `ac7e1d7f`. That extends the previous short straight-road no-spurious-damage check to a complete circuit. It does not replace the separately verified collision-heavy owner replays and terminal-damage tests.

At 18 m/s, all formatted sampled controls/poses/events and all **136,228 final car/physical bytes** are exactly equal across debug, current release and the two retained initial repeats. The two 28 m/s runs are also equal. Only one 40 m/s run was checked; no repeat/profile equivalence is claimed for it. Equality covers the trace and complete final car/physical owners, not every intermediate world byte.

## Driver and reproduction

`grid-practice-drive` uses horizontal nearest-segment projection with a bounded local search after startup. It aims ahead on the purchased centerline, applies pure-pursuit steering and speed-error throttle/braking, and stops at a completed lap, terminal damage, a reported simulation failure or its tick budget. Controller gains, speed caps, lookahead and curve-speed policy are authored test choices. Physics and contact/damage parameters are unchanged.

From the workspace, after building the diagnostic binary:

```powershell
target/release/grid-practice-drive.exe `
  'C:\Program Files (x86)\Steam\steamapps\common\Grid' `
  local/body-contact-directions.bin local/body-impact-parameters.bin `
  local/deformation-parameters.bin local/playtest/NEW_TRACE.tsv 22000 18 1.5
```

Outputs must stay under ignored `local/` and must not already exist. They include complete private source-derived final state and must not be published. The driver uses a 16 MiB worker stack, matching existing source tests. `tools/analyze_practice_drive.py` validates trace framing, finite controls, timing completion, terminal state, complete owner extents, ground-hit/deformation counters and repeated output identity. It records current source, executable and private-parameter hashes.

Private evidence includes `local/playtest/autonomous-practice-20261014-{debug,release}-18-final-01.tsv`, two retained initial release repeats, `release-28-final-{01,02}.tsv`, `release-40-final-01.tsv`, and `autonomous-practice-20261014-final-validation-02.json`. The initial pilot omitted complete final words; later traces include them. Two single-character append Clippy findings were corrected cosmetically; fresh debug/release traces match the earlier repeats after that change.

Rust/Cargo 1.99.0 and the existing toolchain were reused. No game launch, desktop input, installation, original-file modification or user action was needed. At `2026-10-14T00:20:38.538185Z`, all 130 tracked Steam resources and the original executable still match their baseline (`local/scene/preservation-20261014-practice-laps.json`).

This milestone closes autonomous full-course driving validation at the selected conservative settings. Next improve usable audio and broader original-versus-prototype calibration; higher-speed controller accuracy, off-track race rules, opponents and progression remain open.
