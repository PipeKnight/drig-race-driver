# Soak and stress testing (grid-soak)

`grid-soak` (crates/grid-tools/src/bin/grid-soak.rs, shared code in
`grid_tools::soak`) runs seeded headless scenarios through the same driving path
as `grid-app --drive` and reports every crash, physics pause and runaway state
it finds. It is the automated complement to owner playtests: every failure
comes with a seed, the exact per-tick inputs and a one-line replay command.

Everything here is independently authored diagnostics. The chaos driver is
adversarial test behaviour, not original GRID behaviour.

## Running it

Needs the purchased installation and the private machine-local inputs, so it
runs on the owner's PC, not in the cloud:

```
set GRID_GAME_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Grid
set GRID_CONTACT_DIRECTIONS=%CD%\local\body-contact-directions.bin
set GRID_BODY_EVENT_PARAMETERS=%CD%\local\body-impact-parameters.bin
set GRID_DEFORMATION_PARAMETERS=%CD%\local\deformation-parameters.bin
set GRID_AI_CHARACTERISTICS=%CD%\local\ai-driver-characteristics.tsv
cargo build --release -p grid-tools --bin grid-soak
```

Without the variables the tool looks for the same file names in `local/`.
GRID_GAME_ROOT defaults to the Steam path. CD01 is required (opponents need body
contacts); BP01, DF01 and the characteristics table are used when present.

| Command | What it does |
|---|---|
| `grid-soak quick` | Seeds 1..16 (fixed; 1..12 until 2026-10-17, 13..16 are Istanbul Park), 5000 ticks each, 2 jobs: about 1-3 minutes on the owner's PC (12-car Donington 5000 ticks about 30 s). Run before a push. Exit code 1 if anything failed. |
| `grid-soak long --hours 8 --jobs 2` | Seeds from 1001 upwards until the deadline (or `--seeds A..B`), 6000-30000 ticks each. Unattended; the summary is rewritten after every scenario. |
| `grid-soak run --seed N [--profile quick\|long]` | One scenario in-process (the orchestrator runs each scenario this way in a child process). |
| `grid-soak replay local/soak/.../seed-N.inputs.tsv` | Exact replay of recorded inputs, independent of later chaos-driver changes. |
| `grid-soak replay-log local/playtest/manual-*.log` | Headless replay of an owner playtest log, with pose verification. |

Options: `--out local/soak/DIR` (must contain a `local` component; default
`local/soak/<command>-<unix time>`), `--jobs J`, `--seeds A..B`, `--timeout S`
(per scenario; default 120 s + 25 ms per car-tick), `--max-cars N`, and scenario
overrides `--cars N`, `--route tracks/...`, `--ticks N`, `--grid-position P`,
`--assists SPEC`, `--chaos MODE`, `--props on|off`, `--car STEM` and
`--opponent-cars STEM[,STEM...]` (the app's car options, F345), `--keep` (keep
the inputs of passing runs). `GRID_SOAK_TRACE=N` prints one player sample every N ticks.
`GRID_SOAK_TRACE_BODY=I,J..[:FROM]` prints world bodies I, J.. (cars first, then
pieces) every tick from session tick FROM: position, velocity, angular velocity,
the up row and, for cars, the body-contact count and wheel displacements. A
one-tick jump in position and orientation with little velocity is the signature of
a contact with a wrong normal (F342).

Artifacts (all under the ignored `local/soak/`): `summary.md` (distinct failures
grouped by error text, then one row per scenario), `summary.tsv`, and for each
failure `seed-N.failure.md` (seed, scenario, failure tick, error text, every
world body at the failing tick, the last 64 player samples) plus
`seed-N.inputs.tsv`.

### Overflow and debug-assertion builds

The workspace has a `soak` profile: release optimisation with
`overflow-checks` and `debug-assertions` on. Arithmetic overflow then panics,
and the per-scenario `catch_unwind` reports it as a `panic` failure with its
source location:

```
cargo build --profile soak -p grid-tools --bin grid-soak
target\soak\grid-soak.exe quick
```

## Scenario space

A scenario is a pure function of (seed, profile, which private files exist,
`--max-cars`); its full description is the `# scenario` header of the inputs
file, so replays never depend on the generator.

- Cars 1..12 including the player. The quick profile cycles 12, 1, 12, 6, 2, 12,
  9, 4; the long profile picks a full field 35 % of the time, otherwise uniform.
  13..20 cars are refused at set-up ("unsupported shared-world car count"):
  `MultiRuntime` and the app's `--opponents 1..11` cap the field at 12, matching
  the original's 12 driver objects. `--max-cars 20` exercises that refusal.
- Routes: Donington GP, Nürburgring GP and (since 2026-10-17) Istanbul Park GP
  (`route_0`). Quick seeds 1..12 keep their routes; 13..16 run Istanbul Park.
  Long runs move about a third of the seeds to Istanbul Park from a separate
  stream, so the other fields of a seed are unchanged. A race holds the grid for
  the track's start-light sequence (Donington 4.0 s, Nürburgring and Istanbul
  5.0 s; [ISTANBUL_PARK.md](ISTANBUL_PARK.md)).
- Long Beach (`tracks/usa/long_beach/route_0`, the fourth circuit): quick seeds
  17..20 (`--seeds 17..20`; not in the default 1..16 until its tyre-stack falls
  are fixed) and a quarter of the long seeds, from their own stream
  ([LONG_BEACH.md](LONG_BEACH.md) section 5).
- Car models (F345):
  - quick runs stay all-Lacetti, so their results stay comparable with
    earlier runs; `--car`/`--opponent-cars` mix them;
  - long runs draw the models from a separate seeded stream, so every other
    field of a seed is unchanged: half of the races are a mixed BMW 320si +
    Lacetti field (player model random, opponents `bmw,lac` or `lac,bmw`), and
    a quarter of the other runs put the player in a BMW;
  - opponents cycle the list as in the app; each car uses its own AI
    statistics;
  - a non-Lacetti field adds `car=... opponent_cars=...` to the scenario
    header.
- Player grid position 1..cars; opponents fill the other slots in the app's
  order and use the app's AI seeds (0x5eed + k) and characteristics rows.
- Driver aids: random traction control, stability control, braking assist,
  automatic/manual gearbox, keyboard/wheel input with a random wheel linearity.
- Body contacts with breakable route props always on (as the app with
  `--body-contacts`), body events and deformation on when BP01/DF01 exist.
- Events: every race runs the app's 4.0 s start-light countdown (cars held,
  opponents rev with the ported grid revving); solo runs get the 3 s time-trial
  countdown or free practice at random.

### What mirrors the app

`grid_tools::soak::Session` builds the player runtime exactly as `grid-app run`
does (`Runtime::load` with the player's model on the player's slot, assists, lap timing, body
events, CD01 geometry, DF01 deformation, dynamic props) and, with opponents,
the shared world (`MultiRuntime::load_deforming_on` with the same slot
order, dynamic props, AI drivers). `Session::step` is `drive_tick`: countdown
hold (player handbrake, opponents `grid_rev` state 2 then 3), AI controls from
`View::of` and the others' poses, the player runtime swapped into slot 0 for one
common `MultiRuntime::tick`, or the solo `tick_with_(deforming_)body_geometry`.
`Session::reset` is `drive_actions` R: the player runtime, the shared world, the
AI drivers and the countdown. Trial lap results, standings and the after-finish
autopilot are presentation and are not mirrored; the player's recorded controls
already contain whatever drove the car. Cars retired by their own failed stage
(F323) stay held inside `MultiRuntime`, as in the app. Keep `Session` in step with
`drive_tick` when the app's tick changes; `replay-log` pose verification shows
drift immediately.

### The chaos player

Seeded (SplitMix64) and driven only by its RNG and the session state, so a seed
reproduces a run bit for bit. It switches between weighted modes every 1-12 s:

| Mode | Behaviour |
|---|---|
| autopilot | the ported AI drives the player's car (as the app's GRID_AUTOPILOT), to cover the whole lap |
| random | held random inputs, often at the extremes, handbrake 20 % |
| full-lock | full throttle and lock, occasional handbrake |
| prop | follow the AI line until an intact breakable is within 45 m and 0.9 rad of ahead, then drive straight at it; give up after 3 s |
| wall | aim 40 m to one side of the centreline (barriers, walls) |
| ram | aim at the nearest car within 40 m ahead |
| off-track | part lock and throttle off the road |
| reverse | brake-to-reverse (automatic) or reverse gear and throttle (manual) |
| spin | GRID_SPIN_CAR0 style: 60 ticks of full lock, throttle and handbrake |
| stop | brake to a standstill on the line (GRID_STOP_CAR0 style) |

Resets (app R) happen at random after the first 300 ticks of a segment (about 2
per 10 000 ticks). With a manual gearbox the player shifts with speed and
sometimes requests random gears. `--chaos MODE` forces one mode.

### Failure checks

Each scenario runs inside `catch_unwind`, on a 64 MiB stack, in its own child
process under the orchestrator (so aborts, stack overflows and hangs are caught
too). A scenario fails on the first of:

| Kind | Condition |
|---|---|
| world-error | the tick returned `Err`: in the app this is "driving paused: ...; R resets" (since F323 only for shared stages) |
| car-retired | a car's own tick stage failed and `MultiRuntime` retired it (F323 S3): the race goes on in the app, but the stage failure is the same bug |
| panic | any panic, with message and source location (overflow panics in the `soak` profile) |
| non-finite | NaN/inf in the pose, velocity or angular velocity of any world body (cars and prop pieces) |
| out-of-bounds | a body outside the road-chunk bounds (`Track::bounds`) plus 150 m horizontally, 60 m below, 180 m above |
| implausible-speed | a car above 120 m/s or a prop piece above 300 m/s (AUTHORED thresholds for solver explosions) |
| setup | the scenario could not be built (the app would not start) |
| crash / hang | the child process ended without a result, or exceeded its timeout |

Failures are grouped by kind plus error text with numbers replaced by `#` and a
trailing `(detail)` removed.

## Replaying owner playtests

`grid-app --drive` now logs (since this change):

- `drive_replay version=1 route=... opponents=N grid_position=P` once at start;
- `drive_input version=1 segment=S tick=T throttle=... brake=... steering=...
  handbrake=... shift=-|up|down|gN hold=0|1` whenever the player's final controls
  (after the countdown hold, autopilot or script) change, on a gear request and
  at the start of each segment. Floats use Rust's round-trip formatting.

`grid-soak replay-log` rebuilds the scenario from `drive_assists`,
`drive_opponents`, `drive_body_*`, `drive_deformation`,
`drive_ai_characteristics` and the trial countdown lines, then replays each
segment (R between segments), continuing past a world error to the next segment
as the owner did with R. It compares every `drive_sample` pose bit for bit and
prints the first divergence. Older logs without `drive_input` replay from the
`drive_sample` lines, which the app already prints on every control change; the
grid position then defaults to the race launcher's back of the grid
(`--grid-position` overrides) and manual gear changes are not recoverable.

`replay-log` also replays mixed fields: the app names a non-Lacetti field in
`drive_replay` (`car=... opponent_cars=...`), and the session builds the same
models. Before F345 such a log was refused.

## Findings (2026-10-16)

Evidence: main checkout `local/soak/` (every directory has `summary.md`,
`summary.tsv`, and per failure `seed-N.failure.md` + `seed-N.inputs.tsv`).
The harness change (F326) only reports; the fixes are in "Fixes (F342)" below.

### The 16-body prop freeze (owner's 12-car race), reproduced

- Before F317 (c8ea239, the owner's playable 7f22c32 had the same world
  bounds): `grid-soak run --seed 1 --chaos prop --ticks 4000` (12 cars,
  Donington, player on slot 6 aiming at breakables) fails at tick 1645 with
  `invalid contact-group counts or selector` after 4 breaks pushed the world to
  17 bodies; `grid-soak replay` of its inputs fails at the same tick with the
  same state digest 2624bb7f959c98c6 (`local/soak/prefix-prop/`).
- The owner's log `local/playtest/manual-23606-10155.log` (pre-`drive_input`)
  replays headless to the same `tick 2188: invalid contact-group counts or
  selector`; all 268 logged player poses match bit for bit
  (`local/soak/prefix-owner-23606/`).
- On main after F317 both pass: the prop run reaches tick 4000 with 18 bodies,
  the owner log replays through tick 2188 with 18 bodies and the same 268
  poses (`local/soak/main-prop/`, `local/soak/main-owner-23606/`).

### Distinct failures on main

Runs: `quick-main/` and the long runs on 511ac8a (F318) plus this harness;
`quick-f323/`, `quick-f323-nurburgring/` and every replay below on f6087f8
(F323) plus this harness. Every recorded failure below replayed on F323 with the
same tick and state digest as on F318; `quick-f325/` on 6234931 (F325) shows the
same groups.

- `long-1/`: long profile, 91.6 min wall, 532 scenarios, 3.69 M ticks (984 min
  simulated). 284 were Nürburgring set-ups refused by #1 (1-3 s each); 248 ran
  on Donington.
- `long-nurburgring/`: `--route .../nurburgring/route_0 --props off`, 82.6 min
  wall, 234 scenarios, 3.57 M ticks (951 min simulated).

| # | Group (`summary.md` key) | What and where | Seeds (replay: `grid-soak replay local/soak/<dir>/seed-N.inputs.tsv`) |
|---|---|---|---|
| 1 | `setup: scaled prop piece placement` | Every Nürburgring scenario with body contacts. `dynamic_props::Catalog::read` rejects a breakable whose piece placement has a scale (dynamic_props.rs, the `(length - 1).abs() > 1e-3` row check). The app enables dynamic props whenever `--body-contacts` is given, so `grid-app --route tracks/europe/nurburgring/route_0 --body-contacts ...` (any Nürburgring race) cannot start. Still present on F323. | quick seeds 2, 3, 6, 7, 10, 11 (`quick-f323/`); 284 long-1 seeds |
| 2 | `world-error: nonfinite handling effects` (F318) / `car-retired: car retired: nonfinite handling effects` (F323) | Nürburgring, 12 cars: AI car 6, racing normally at about 58 m/s near (-7, 598, -528), produces non-finite handling effects (`vehicle_handling.rs`, "nonfinite handling effects") at segment tick 6559 after 6 chaos resets. On F318 this paused the whole race ("driving paused"); on F323 the car is retired and the field races on. A real numeric bug in the handling path. | long-nurburgring seed 2177 (`f323-nonfinite/` has the F323 report) |
| 3 | `out-of-bounds: car falling through the world` | Nürburgring: cars fall through the road at about x -210..-225, z -20..-45 (36 of 48 cases), some only 2 m from the centreline at height 606-607, so this is a hole or query gap in the road collision, not a missing run-off. Mostly AI cars on the racing line; also the player. Other single spots: (-220, -90), (-240, -230), (280, -500). 48 of 234 runs (20 %). | quick seed 6 with `--props off` (`quick-overflow-nurburgring/`); long-nurburgring 2003, 2004, 2007, 2008, 2009, 2025, 2027, 2029, 2038, 2049, 2070, 2079, 2082 and 35 more |
| 4 | `out-of-bounds: prop piece falling through the world` / `... fell below the world` | Donington: broken brake-marker pieces sink slowly through the road and then free-fall, in the area x -390..-570, z -8..-240 (always within about 10 m of the centreline). Quick seed 5: 7 breaks at ticks 1160-1210; one cardboard half starts sinking at about tick 1188 and passes the road at y -15.3. Before F317 these extra bodies also counted towards the 16-body bound. 62 of 248 Donington long runs. | quick seed 5 (`quick-main/`, also `quick-f323/`); long-1 1019, 1020, 1033, 1040, 1044, 1067, 1069, 1072, 1075, 1076, 1081, 1082, 1091 and more |
| 5 | `out-of-bounds: car fell below the world` | Donington: a car falls through the world, mostly the chaos player off the line. The player at (-460, -27), 9 m from the centreline (seed 1066, `donington-car-fall/`), the area of #4; also (-335..-351, -28..-31), (-485..-583, -38..-94), (-217, -8), (83, 134), (122, -191) and once AI car 6 at (-23, -300). 11 of 248 Donington long runs. | long-1 1066, 1097, 1111, 1202, 1209, 1250, 1259, 1345, 1387, 1415, 1494 |
| 6 | `setup: unsupported shared-world car count` | 13..20 cars are refused by `MultiRuntime` (1..=12) and the app (`--opponents 1..11`). Expected limit, matching the original's 12 driver objects; listed so a 20-car request is a known refusal, not a surprise. | `grid-soak run --seed 1 --cars 13` and `--cars 20` |

No panics, no non-finite body poses or velocities, no implausible speeds, no
hangs or crashed scenario processes, and no world error other than #2 occurred
in any run.

### Overflow checks and debug assertions

`target\soak\grid-soak.exe` (profile `soak`) ran the quick set on Donington
(`quick-overflow/`, 5000 ticks x 12 seeds, including a 12-car run with 14
breaks and 27 bodies) and on Nürburgring with props off
(`quick-overflow-nurburgring/`): no overflow panic and no debug assertion. The
only failures were the groups above. The code base has no `debug_assert!`, so
the profile currently adds only overflow checks. It ran only slightly slower than release in these runs (the machine load
differed), so it is worth using for long runs now and then.

### Randomized (proptest-style) tests: proposal

A throwaway probe (kept privately at `local/soak/parser-probe/`, not in the
repository) fed 3000 seeded mutations (bit flips, 0x00/0xFF bytes, random
bytes, truncation) of each of five purchased Donington files through
`binxml::read` + `progress::read`, `cqtc::read`, `ornaments::read`,
`pssg::read_tree` and `binxml::read` (ai_track), in the overflow profile:
15 000 inputs, 0 panics, every malformed input returned `Err`. The readers'
explicit bounds checks already do the job, so broad parser fuzzing has a low
expected yield. Worth adding, in order:

1. World-step properties, using the soak `Session` (asset-dependent, ignored
   tests): "a tick that returns `Err` leaves every car, the world and the prop
   state unchanged" (MultiRuntime documents this; the harness can compare a
   state digest before and after a failed step), "N resets restore the initial
   digest", and "the same inputs give the same digest" (determinism, which
   every replay here already confirmed).
2. Pure validation functions with small generated inputs (no assets, runs in
   CI later): `contact_groups` count/selector validation, `lap_timing`
   ordering, `time_trial::State` and `race::Standings` invariants, and the
   `Assists::parse`/`Display` and `soak::Scenario` header round trips (the
   latter two already have deterministic loops). A dependency such as
   `proptest` is not needed: `soak::Rng` drives seeded loops with the seed in
   the assertion message.
3. Parser mutation tests only for formats that change: a seeded mutation loop
   over a purchased file per reader, `#[ignore]` (asset-dependent), as the
   probe above.

## Fixes (F342, 2026-10-16)

Every fix was first reproduced with the recorded replay (same tick and digest),
then the replay was rerun on the fixed build. Pre-fix builds came from a scratch
worktree; evidence is in the main checkout's `local/soak/fix-*` and
`local/soak-fixes/`.

| # | Cause | Fix | Evidence |
|---|---|---|---|
| 1 | Nürburgring tyre stacks have a uniform 0.9718 placement scale; the catalog refused it. 0x7D3C40 gives the body constructor the unnormalised entity matrix (only the render path splits off the scale). | Keep the scaled basis; reject only a degenerate one. | DYNAMIC_PROPS_RESEARCH.md "Soak fixes (F342)"; test `nurburgring_scaled_tyre_stacks_keep_their_placement_scale`; quick seeds 2, 3, 6, 7, 10, 11 start. |
| 2 | Standard.hdm thresholds are 1.0; a damage level saturating at 1.0 gives the effect ramp 0/0. grid.exe stores the NaN too, but its readers only compare it (JBE) and read it as zero blend; the port's finiteness guard retired the car. | 0/0 blend = 0 in the effects and the steering caps. | HANDLING_DAMAGE.md "Unit thresholds: 0/0 blend (F342)"; test `saturated_damage_at_unit_thresholds_gives_zero_blend`; seed 2177 on the pre-fix build plus this change alone passes all 15 606 ticks. |
| 3 | Not a mesh gap (ray and brute-force triangle search agree, no holes). The body/road contact orientation (AEE640 small branch) decides by the sub-threshold displacement: a one-ulp witness offset flips the normal by noise (quick seed 6), and an exact vertex-0 witness keeps a margin contact's inverted normal (quick seed 3). A rounding-sized offset just above 1e-5 (one ulp is 6.1e-5 at 600 m) does the same in the normalised branch. Each publishes contacts through the whole car (1.3-1.45 m) and the car is pushed through the kerb in one tick. | A displacement within 1e-5 or 32 ulps of its coordinates is degenerate; orient by the other, informative one (port deviation, F320; recorded). | CONTACT_REORIENTATION_RESEARCH.md "Degenerate displacement"; test `orientation_swaps_witnesses_and_preserves_signed_normal_lanes`; the replays pass. |
| 4 | Mostly the orientation flaw of #3 (boards at Donington \|x\| 400-550). Also (a) a piece leaning on a static prop box got all its track contacts from the box (props first, one shared budget), so the road chunk got none and it slid through the road (quick seed 9). (b) Lost pieces kept falling toward the 1e5 domain, a later world pause (R06). | (a) Per shape pair capacities, bounded only by the pool (as in the original). (b) Hold a piece 100 m below the road. | DYNAMIC_PROPS_RESEARCH.md; tests `a_hull_against_a_prop_box_keeps_its_road_contacts`, `a_piece_lost_below_the_world_is_held_until_reset`; quick seeds 5 and 9 pass; 12 of 13 board falls of a pre-fix Donington long run and long-1 1044, 1386 pass on replay. |
| 5 | The same orientation flip and contact budget. | As 3 and 4. | Replays of long-1 seeds 1066, 1097, 1209 and 1345 pass. |
| - | Nürburgring with props: up to 138 bodies (12 tyres per stack) gave 749 world contacts; the deformation callback chain still had the 512 port bound (F318 A39) and paused the world. | Raised to `MAX_WORLD_CONTACTS` (10 000) like the F317 bounds. | Replays of long Nürburgring seeds 1007 and 1018 pass. |
| 6 | Unchanged: 12 driver objects, as in the original. | - | - |

Still open (not a harness artefact): a piece driven into the ground by a hard
hit in one tick (a brake-marker board hovering inside the contact margin hit at
16 m/s; a piece spawned inside a car at 51 m/s): long-1 1091 and 1304 and two
pre-fix long-run seeds, see DYNAMIC_PROPS_RESEARCH.md "Pieces pushed into the
ground". Such pieces are now held below the world instead of growing to the
1e5 domain, so they no longer threaten a world pause. The board case and the
two player falls below were fixed in F351 ("Residuals (F351)").

### Runs after the fixes

Build ffb71b8 (rebased on F340), grid-soak sha256 a488ae0d...c3e6, main checkout
`local/soak/fix-quick-final/` and `final-long-*/`:

| Run | Scenarios | Failed | Groups |
|---|---|---|---|
| quick (both routes) | 12 | 0 | - |
| long Donington, props on, 60.7 min wall, 990 min simulated | 227 | 25 (11 %) | 24 prop piece falls (11 at the 150 m brake marker, -406..-413, -16; 4 at -532..-551, -170), 1 car fall (player 24 m off the line at (762, 45), seed 1087) |
| long Nürburgring, props on, 61.3 min wall, 591 min simulated | 125 | 6 (5 %) | 5 prop piece falls, 1 car fall (player 26 m off the line at (-281, 26), seed 1087) |

For comparison, F326: Donington 73 of 248 long runs failed (62 piece and 11 car
falls), Nürburgring with props could not start at all, and without props 48 of
234 runs lost a car. No world error, retirement, panic, set-up failure, crash or
hang occurred after the fixes.

### Runs for per-car AI extents (F345)

These runs cover the F345 change: each car now has its own AI traffic extents
([CAR_GENERALIZATION.md](CAR_GENERALIZATION.md), "AI car extents"), and mixed
fields are supported. The baseline is origin/main c09ca82, built in a separate
worktree. The runs are in the main checkout's private `local/car-extents/` (soak
directories under `soak/`).

| Run | Scenarios | Failed | Groups |
|---|---|---|---|
| quick, baseline | 12 | 0 | - |
| quick, F345 | 12 | 1 | seed 12: Donington board fall at (-406, -51, -16), tick 3003 (F342 residual) |
| quick, F345 `--car bmw --opponent-cars lac,bmw` | 12 | 0 | - |
| quick, F345 `--opponent-cars bmw,lac` | 12 | 1 | the same seed-12 board fall |
| long 1001.., baseline (stopped) | 24 | 4 | prop piece falls (1004, 1013, 1016, 1026) |
| long 1001.., F345, 14 mixed fields (stopped) | 26 | 2 | 1004, identical to the baseline; 1001, a mixed 12-car Nürburgring tyre-stack piece at (-255, 571, -130) |

Both new failures were replayed exactly with `grid-soak replay`. They belong to
the open F342 prop-piece residuals, not to a new class. The only cause found is
that the races now take different paths.

## Residuals (F351)

17 October 2026. Both F342 residuals were port gaps against the original. Details:
[BOX_CONTACTS.md](BOX_CONTACTS.md). Evidence is in the main checkout's private
`local/soak-residuals/` (decompilations, `wip/` with the trace and tried patches, the
binaries compared below in `bin/`, and `runs/` with the soak runs and traces).

| Failure | Cause | Fix |
|---|---|---|
| Player falls out of the world after a hard off-track hit (seed 1087, both routes) | Route prop boxes were queried as twelve separate triangles. A tyre wall the car drove 1 m into gave contacts with downward normals (up to 0.9 m deep), which pushed the car through the road. | Prop boxes are box primitives in one AE8AD0 query (`grid_world::box_contacts`), with their shape shell as margin, as in the original. Pieces against static boxes use it too. |
| Thin boards driven into the ground (seed 12 and the 150 m marker at (-406, -16): seeds 1005, 1012, 1029, 1074) | Road triangles had margin 0. The original gives each its chunk's +0x220 margin, 0.02 in the live game. The marker's lower board stands 2.14 cm above the grass, so it had no ground contact when it spawned inside the breaking car, and the car pushed it under. | `ROAD_MARGIN` 0.02 on road triangles (`packed_contacts::Settings::shape_margin`). The board then rests on the grass. |

Replays: `grid-soak replay local/soak/final-long-{donington,nurburgring}/seed-1087.inputs.tsv`
(main checkout) fails on the F342 build 4ffec8b and passes with the box change.
Current main no longer reaches this state: F345 changed the field's paths. Main's own
failing inputs from the long run below (`local/soak-residuals/runs/soak/long-main-307fe9b-*/`), replayed on
the fixed build: 15 of 16 pass.

| Route | Main seeds (failure tick's piece) | On the fix |
|---|---|---|
| Donington | 1005, 1012, 1074 (the 150 m board, -406/-16); 1042, 1067, 1072, 1078, 1081 (other pieces) | pass; 1005 and 1074 with the same breaks as main |
| Donington | 1062 | fails at (-386, -54, -12), on another path (6 breaks against 2): the board kicked and spun by the triangle-soup car contacts |
| Nürburgring | 1001, 1004, 1011, 1035, 1062, 1085, 1099 (pieces) | pass; 1011 with the same breaks as main |

Most inputs take another path on the fix (the break count differs), so a pass there
shows only that the run survives.

### Runs (F351)

Main 307fe9b and the fix ran side by side on the same seeds, props on, two jobs each
(`--seeds 1001..1100`). The fix build is grid-soak sha256 9a34103e…12f2. Its code equals the pushed commit before the rebase onto the Istanbul Park and camera work.

| Run | Donington | Nürburgring |
|---|---|---|
| main 307fe9b | 9/100: 1005, 1012, 1042, 1062, 1067, 1072, 1074, 1078, 1081 | 7/100: 1001, 1004, 1011, 1035, 1062, 1085, 1099 |
| this change | 2/100: 1056, 1099 | 1/100: 1085 (a tyre-stack piece at (-335, 569, 64), not traced) |

Every failure in both builds is `prop piece falling through the world`; no car fell,
and there was no world error, retirement, panic or hang. The fix's runs were made in
two parts: seeds 1001..1074 / 1001..1049, then the rest. In between, two builds that
were not adopted ran on the same seeds; see BOX_CONTACTS.md, "Tried and not
adopted". The fix's two Donington losses:
- seed 1056: a board of breakable 105 at (-11, -331), spun into the road by the
  triangle-soup car contacts;
- seed 1099: the board of breakable 88 settled after its break at tick 1866 and was
  lost at (-257, -282) at tick 10930, about 170 m away (not traced).

Quick on the rebased build: 16/16 (seeds 13-16 are Istanbul Park), also with
`--car bmw --opponent-cars lac,bmw` and `--opponent-cars bmw,lac`. Targeted `--cars 1 --chaos prop --ticks 8000`, seeds
1..80 on Donington: main 1 piece fall, fix 0.

## Follow-ups

- Car against piece still uses the car-to-car hull-triangle path, and piece against
  piece the authored separating-axis test with the larger shell. The original uses
  AE8AD0 for both and sums the margins. Both were tried; at the port's fixed 16 ms
  step they lose more boards (BOX_CONTACTS.md, "Tried and not adopted"). Revisit
  them together with the world step: the original steps once per rendered frame.
- Remaining piece losses on this build: long Donington seeds 1056 (a board spun into
  the road by the triangle-soup car contacts) and 1099, and Nürburgring 1085 (a
  tyre-stack piece). The last two are not traced.
- The boundary sheet's and the car hull's own shape margins are not established
  (both 0 in the port).
- `Session` mirrors `drive_tick` as of F323 (retired cars, the player's own
  contact count). Keep it in step when the app's tick changes.
