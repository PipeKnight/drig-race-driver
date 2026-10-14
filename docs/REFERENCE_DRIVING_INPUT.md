# Original Lacetti input and motion reference — F277

15 October2026 client date /14October22:05–22:43UTC. This closes the immediate
question of whether the existing guarded Up/Down macros produce real player
pedal commands. It does not establish quantitative prototype handling fidelity
or explain the earlier high-speed F13 experiment retrospectively.

## Identity, settings and control

The owner explicitly freed the desktop and manually cleared GRID's title screen.
Guarded AHK2.0.29 menu navigation then worked. The research race is Touring Cars,
Chevrolet Lacetti, Donington Park Grand Prix Circuit, three laps, Normal/random
grid. The assists page visibly confirms traction control On, braking assist On,
stability control On, transmission Automatic. No assist/binding/audio setting
was changed. Main-menu/start navigation is not proof of driving input.

Private process51496, creation2026-10-14T22:05:30.3394574Z, executable
C:\Users\debros\Tools\grid-research\grid-compat\grid.exe SHA256
b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6.
Fresh bounded discovery finds12 owned controllers, player36ed9f20, car34527200,
physical34d0c8c0. These addresses coincidentally match older sessions, but were
revalidated against this process and ownership graph. Never reuse them without
fresh checks. Current world4fedd30 and player jobe8ed98 were independently checked
through physical+32C and the registered20-slot job array/table/method.

The original installation was never launched for modification. All130 baseline
Steam resources plus its executable remain unchanged at22:43:41.055486UTC,
local/scene/preservation-20261015-driving-input.json. All19 snapshotted user
files matched after the brake run; backup/manifest/comparison remain under
local/driving/native-reference-20261015-01/. No settings restoration was needed.

## Read-only recorder and framing

[tools/capture_grid_driving.ps1](../tools/capture_grid_driving.ps1) uses existing
PowerShell7.6.5/.NET10.0.11, not a newly installed recorder. Exact path/SHA/base,
PID/creation time and handle identity are checked, followed by unchanged analysis
code prefixes, player Lacetti controller links, car/physical and (DT02) world/job
ownership. It opens only VM_READ +QUERY_LIMITED_INFORMATION; there is no process
write, injection, suspension, desktop input or microphone capture in this tool.
Input comes separately from Computer Use and the existing guarded macro server.

Duration is500..30000ms, with3100-row/2MiB bounds, new exclusive outputs only
under ignored local/. Existing outputs and redirected directory links reject.
Relative and absolute private paths are supported. DT01 stores QPC begin/end,
repeat-controls equality,40bytes from car+E1C,96bytes of physical+1D0 pose/motion,
and24selected engine/gear words. DT02 adds owned world+7C4/+7C8 and player-job+8
phase/delta observations. Raw IEEE bits and unknown words remain private.

The ten-millisecond sleep requests do not produce an exact100Hz clock: observed
Windows sampling is roughly65Hz. Reads are sequential, not atomic. Repeated
control blocks agree in all1947brake and1945return observations; this does not
make motion/engine/schedule simultaneously atomic. Read-duration maxima are
548.2us(brake) and4587.6us(return); maximum sample gaps29.776ms and17.061ms.

[validate_grid_driving_capture.py](../tools/validate_grid_driving_capture.py)
checks framing, sizes, monotonic QPC, finite values, counts and bounds. Its
>.5pedal span classification is authored presentation, with original bits retained.
[validate_grid_driving_guards.py](../tools/validate_grid_driving_guards.py)
verifies rejection reasons for11bad duration/controller/non-player/start/hash/
output cases. Eight malformed trace controls reject separately. Existing pilot
bytes remain unchanged.

## Accepted observations

| Capture | Records | Observation |
| --- | ---: | --- |
| paused-pilot01.tsv |33| All pedals zero; speed0.004955m/s; useful read operation verified |
| throttle01.tsv |1296| Up sets E1C/E3C to1 and E20/E40 stay0; only final0.324s of active input fits this first capture |
| brake01.tsv |1947| Complete two-second Down macro and return to pause; actual brake and reverse transition observed |
| throttle-return02.tsv |1945| Complete two-second Up macro and return to pause; reverse-to-forward direction observed |
| schedule-paused01.tsv |33| DT02 world/player-job ownership and delta reads verified |
| coast-schedule01.tsv |1949| Input was cancelled before invocation; paused-only DT02 evidence, **not a coast maneuver** |

For brake01, the car begins at19.965503511m/s in gear2. Down makes E40=1,
E20=1 and E3C/E1C=0; speed falls to2.964320300m/s. Near3m/s the original automatic
logic selects gear9 and swaps publication: E1C=1/E20=0 while E40 remains1 and
E3C remains0. The physical velocity projected onto the car's third pose axis
changes from+19.963984634 to-3.080593825m/s. This establishes braking followed by
actual reverse motion, not merely a gear label. The car is paused afterward.

For throttle-return02, Up sets E3C/E1C=1 and E40/E20=0. Gear changes9->1;
axis-projected velocity changes-3.080593825 ->+15.558928938m/s, ending at speed
15.560398712m/s. All relevant changes and subsequent zero-input pause fit in
this capture. No original-versus-Rust trajectory equality is claimed.

In the paused DT02 captures, world and player-job delta are both
0.006061346735805273s; observed phase words are0/7. These are stored values from
sequential paused observations, not measured substep frequency, elapsed game
time or a proof of async scheduling. Do not sum repeated observer samples as
simulation ticks or tune handling against wall seconds. Original game-clock/
substep alignment remains the next prerequisite for quantitative comparisons.

## Failed/limited experiments

- Initial nested PowerShell/Python here-string creation failed to parse before
  writing a tool; apply_patch corrected it. Parser/C# compile and live pilots pass.
- The first20s throttle window started too early relative to the tool/UI boundary:
  it captures only the opening0.324s of Up. Retained as input evidence, not a full
  acceleration sample. Later captures use30s and a verified ready log.
- Background throttle-return01 failed before recording because absolute paths
  were incorrectly joined to the root. No game input was applied for that run.
  This also invalidated the initial guards01 interpretation: all cases rejected
  at the common path check. That report is retained with guards01-rejection.json.
  Corrected paths and guards02/03 check each intended reason; full live absolute-
  path throttle-return02 and DT02 captures succeed.
- Before the coast macro, Computer Use reported intervening user input; refreshed
  capture reported GRID minimized. No F14 input was retried, no coast was accepted,
  and desktop input was left alone. The existing original race remains paused;
  macro logs end with the successful Up batch/key releases.
- Earlier F13 acceleration81->106mph remains unexplained. Current owned in-hold
  measurements demonstrate braking, but do not repair that earlier experiment.

## Private provenance and next actions

All paths below are under local/driving/native-reference-20261015-01/:

- player-initial.json: SHA256 fb49fc43c760af8060b24c462cbcf7917a9a0315d635600fc6243a37a6c870cc.
- brake01.tsv: SHA256 a0982af8817abe9a823374427b58f67f0374640021f4fcf192411e2b33fc6aad.
- brake01-validation02.json: SHA256 af96e66cde186960a6b7ee44af60cd92a4d408f6022f71a85eb34fa9c089e76b.
- throttle-return02.tsv: SHA256 cc0a17038c78c03d19e76c19bf0548e7c6bea5bb731899e70b44d32de62d9a12.
- throttle-return02-validation.json: SHA256 0ff6ea916abaaecfdd2cdd2229d79366d26d9cb3938498548f3f8d08253e711e.
- schedule-paused01.tsv: SHA256 912714f302bdccf6d72658f6be65bbab71425c806dfd0afbe1599095c3050220.
- coast-schedule01.tsv: SHA256 33cc633d6098c3592b7780544d062db2333ff3793b0bed429ac554ee41f80c95.
- guards03/validation.json: SHA256 f34359708c6a2f778df24cca3df10d1a6b4714342dcbb9ad09e94692538d4d4f.
- parser-guards01/validation.json: SHA256 b9007bed462f2aec1930a0eb97b90c7261c64c572b54ab5056cff5d359dec6d7.

General-manager discovery also succeeds: local/behavior/general-vehicle-manager-
20261014T2238592589563Z.json (12cars/219successful bounded blocks). Reused tools;
no approval rejection or new installation. Current playable0cb4405 remains
unchanged: no source behavior change/rebuild was needed for this diagnostic work.

Next capture a real no-input coast after the desktop is available; establish
original elapsed game-time/substep alignment before numeric braking/acceleration
calibration. Then compare matched source settings/initial state with Rust and
fix demonstrated differences. Shared-world multi-car ownership/contact/damage
integration and an opponent follow. Owner event/PB usability observation remains
valuable independently of these diagnostic comparisons.
