# Rigid-body sleeping: original and port — F541

21 October 2026. The original world puts a body island to sleep once every body in it has stayed
below its stationary thresholds for turnOffTime seconds; asleep bodies leave the active list and
cost nothing until something wakes them. Until this entry the port's grouped frame admitted only
never-sleeping bodies (RUNTIME_LIMITS_AUDIT.md A11/A12), so every broken prop piece was solved,
advanced and collided on every tick for the rest of the race. Static analysis of the unpacked image
(20 October 2026, private notes `local/handoff/sleeping-research.md`, decompiles under
`local/robust/decomp/sleep01..03/`); measurements in section 6. Labels: [D] read in disassembly or
decompile, [P] already verified by an earlier port comparison, [I] interpretation, AUTHORED port
choice.

## 1. Body fields and settings

| Body field | Meaning |
|---|---|
| +0x210 / +0x220 | angular / linear velocity [D, P `impact_frame::asleep`] |
| +0x3A0 | sleep timer in seconds [D] |
| +0x3A4 / +0x3A8 | linear / angular stationary threshold, squared [D] |
| +0x3AC / +0x3B0 | wake force / wake torque threshold, squared [D] |
| +0x3B4 | turnOffTime (plain); FLT_MAX = never sleeps [D] |
| +0x3FC | parking radius squared (FLT_MAX = never parked) [D] |
| +0x400 | list state: 0/1 = in active list 0/1; -1 = asleep, in no list [D] |
| +0x300 / +0x304 | active-list next / prev [D] |
| +0x2C, +0x18, +0x8, +0x2FC | island byte, root, root's first body, next body in island [D] |

The body constructor B11F40 squares the descriptor's thresholds and copies turnOffTime unchanged
[D; `physical_constructor.rs` already did, P]. Prop pieces (rbDefault): stationary 0.3 / 0.3,
turnOffTime 1 s, wake thresholds 0, parking radius 50 [D values, DYNAMIC_PROPS_RESEARCH.md]. Lost
wheels: stationary 0.1 / 0.1, turnOffTime 1, never parked [P WHEEL_DETACHMENT.md]. Cars: turnOffTime
FLT_MAX and parking radius FLT_MAX, so **cars never sleep** [D+P].

## 2. Sleep test and transition

- **B0FE60** (per body, inside the island frame B7B850, before gravity): the timer resets to 0 if
  turnOffTime is FLT_MAX, if |w|² ≥ the angular or |v|² ≥ the linear threshold, or if a pending
  force or torque exceeds its wake threshold; otherwise it grows by the step and, at turnOffTime,
  clamps there and reports asleep [D]. Already ported as `impact_frame::asleep` [P].
- If every body of the island reports asleep, the frame returns **status 1** before solving;
  capacity overflows return 1 as well [D]. Status 2 (every body outside its parking radius) is the
  park path.
- **B77310** writes the status to the root (+0x30) and, for a nonzero status, clears every island
  contact's written-back normal force (+0x3C = 0, B0F0A0) [D].
- **AD8130**, after all island frames: status 1 → **B24D70** sleeps the island: per body unlink from
  the active list (+0x400 = -1), zero linear and angular velocity, clear force, torque and impulse
  accumulators. The timer stays at turnOffTime and the island links are kept [D].

## 3. What an asleep body skips

Everything that walks the active list [D]: the pose advance (AD7040), the broad phase B18300 (pairs
come only from active bodies; asleep–asleep and asleep–static pairs are never made), grouping
AD6C40 and therefore the island frame (no sleep test, gravity, solve or damping). The narrow phase
uses an asleep body's current pose when an awake body touches it.

## 4. Wake paths

| Path | Original | Port |
|---|---|---|
| Contact with an awake body | B184F0 collects the islands of asleep bodies in pairs with contacts; their pairs are collected again; AD7110 wakes them with B24E70 (zero velocities, link into the current list, timer 0) before grouping, so they are solved this tick [D] | `grouped_physical_frame::tick`: after the provider, contacted sleepers' islands wake (`wake_island`) and the provider is called a second time; the continuous stage runs only in the first call |
| External contact pool | AD6C40 wakes the island of a referenced asleep body [D] | the external prefix wakes its islands before grouping |
| Joint partner | AD6C40 pulls an asleep A through B's joint list; jointed bodies share an island [D] | `contact_groups` AD6C40 port pushes B-side joints when A is asleep and links the pulled bodies |
| Joint created | B825E0 wakes the asleep side into the awake side's list [D] | `dynamic_props::make_joint` |
| Joint removed | B82430: asleep A wakes its island, else A's timer restarts; an awake dynamic B's timer restarts [D] | `dynamic_props::remove_joint` |
| Forces | B0FB10 / B0FA20: a force above the wake threshold wakes (pieces: any force) [D] | pieces receive no external forces in the port; not needed |
| Unpark | AD6480 wakes parked bodies back in range [D] | parking not ported |

Between ticks `grouped_physical_frame::wake` links a woken island into the carried world's heads
(world[7 + selector], world[9 + selector]).

## 5. Port (F541)

- `dynamic_props::body_settings` passes the piece's turnOffTime (was FLT_MAX); the grouped frame no
  longer rejects it (A11). Status 1 without a car clears the island's contact +0x3C and, after the
  root loop, sleeps every body (`sleep_body`, B24D70). Status 1 for an island that holds a car (a
  capacity overflow; cars cannot pass the sleep test) and status 2 still pause (A12, narrowed).
- `contact_groups`: `ASLEEP` (+0x400 = u32::MAX); validation accepts asleep bodies outside the active
  chain and refuses contacts that name them; `physical_world::advance_owned` accepts
  `active + asleep = bodies`.
- `PropWorld::contacts`: an asleep piece makes no static-geometry contact and no pair with another
  asleep piece (B18300). Car–piece and awake–asleep pairs are still made, which is what wakes them.
- **Parking (status 2, B24DF0 / AD6480) is not ported**: pieces never leave the active list by
  distance. AUTHORED for now; the park reference is a car's body (5C6D2B), its scale unverified.
- Identities: unchanged (`tools/identity.tsv`). The identity runs have no dynamic props, and cars
  never sleep, so their worlds never take a sleep path.
- Tests: `pieces_at_rest_sleep_and_a_car_wakes_them` (Le Mans tyre wall 969: 24 of 44 pieces asleep
  after 600 ticks at rest, a car driving at the pile wakes some), and the cost probe
  `sleeping_cost_on_a_broken_nurburgring_field` (section 6).

## 6. Measurements

All release builds of d186946d (before) and the sleeping commit, on the shared PC (other sessions
building; wall times vary up to 4× between runs, so only paired or same-run figures are compared).
Evidence: `local/r27/measure/`, `local/r27/ab/`.

**Paired, same pile, same process** (`sleeping_cost_on_a_broken_nurburgring_field`): Nürburgring
drift circuit (route_1), 2 cars at rest, breakables broken at rest with `queue_break`, settled, then
cloned; one copy keeps sleeping, the other has every piece woken with turnOffTime FLT_MAX; the
copies tick alternately (env `GRID_SLEEP_AB_KINDS`, `_PIECES`, `_SETTLE`, `_TICKS`).

| Pile | Settle ticks | Pieces | Asleep (mean) | Sleeping ms/tick | All awake ms/tick | Ratio |
|---|---:|---:|---:|---:|---:|---:|
| tyre stacks | 3000 | 228 | 161 | 2.87 | 5.53 | 1.92 |
| tyre stacks | 300 | 228 | 33 | 5.36 | 5.73 | 1.07 |
| tyre stacks and armco | 300 | 755 | 746 | 7.58 | 18.00 | 2.38 |
| tyre stacks and armco | 300 | 987 | 856 | 12.18 | 24.85 | 2.04 |

An armco break takes its linked run with it (hundreds of pieces in one break), so the armco piles
start at 755. Armco sections settle within seconds; tyres roll and take 30–50 s (39 of 228 asleep
after 600 ticks, 157 after 3000).

**What an asleep piece still costs.** A sample profile of the sleeping copy alone (987 pieces, 897
asleep; `tools/sample_profile.py`, `GRID_THREADS=1`) puts about half of the tick in memory copies
and frees of whole-world buffers: the 0x424-byte body array and the contact pool are cloned
several times per tick (`physical_world::advance_owned`, `contact_groups` build, the frame's output),
so every body, asleep or not, costs bandwidth. `PropWorld::contacts` still builds every piece's world
hull and tests every piece pair's distance (O(n²)) and every car against every piece. Removing
those copies is a separate grid-world change.

**Sessions** (before / after; the races diverge once the first piece sleeps):

- Milan seed 41, 12 cars, grid-soak 3000 ticks, 72 pieces: piece contacts 349 → 12; world tick
  6.87 → 6.26 ms (same run conditions; an earlier run on a quieter PC measured 16.5 → 5.4 ms).
- Le Mans career repro, headless, first 3000 ticks: identical breaks before tick 2494; at 26 pieces
  piece contacts 115 → 53 per tick.
- Le Mans full race: at about 400 pieces the main build had 2,100 piece contacts per tick, the
  sleeping build 300–500 at 500 pieces; the world advance per piece stayed about 45 µs (the copies
  above). The main build's player car was wrecked at tick 12,221; the sleeping build raced on to
  tick 45,313 with 641 pieces.
- Nürburgring drift battle (route_1, 8 BMWs, autopilot, headless, 20,000 ticks): main 126 breaks
  and 813 pieces at the end, sleeping 32 breaks and 345 pieces; tick costs not comparable (the
  runs had 4× different load at 0 pieces).
- grid-soak `--chaos prop` on Nürburgring route_1, 8 BMWs, seeds 1–3, 10,000 ticks (22–46 bodies):
  seeds 1 and 2 diverge (pieces slept), seed 3 keeps its digest (no piece slept); wall time per
  tick 0.997 → 0.825, 0.932 → 0.751 and 1.366 → 1.269 ms. Seed 3's 7 % is load noise, since its
  work is unchanged, so the small-pile gain is within noise.

## 7. Does the original cap ticks per frame?

There is no catch-up loop in the original's physics: the world scheduler A72930 takes **one step of
the real frame delta per rendered frame** (live maxfps 10000, minfps 0, maxTimesToRun 1;
WORLD_STEPPING_RESEARCH.md, F285). A slow frame makes a longer step rather than more steps. Whether
anything clamps the frame delta itself was not read.

## Open

- Parking (status 2, AD6480/B24DF0/AD7D60) and the park reference point and scale.
- The role of B15280 on shape +0x184 (pair cache removal) and B187D0.
- B692B0 creates bodies and sleeps them immediately; owner unidentified.
- The per-tick whole-world copies in grid-world (section 6).
