# First corner against a live 12-car field (Donington, F433)

Status: 2026-10-19, Claude Code. Comparison of the original's 12-car Donington field
(F417's capture `field-20261018/field02.tsv`, main checkout `local/driving/`) with the
port's headless field. **No driving change** in F433; the launch was ported in F444 (section 7).

Evidence (ignored, worktree fervent-napier-50814b `local/firstcorner/`):
- `decode_field.py` decodes the FD01 capture into `field02.dec.tsv`;
- `compare.py` aligns the two fields at the go and compares them (`cmp-*.txt`);
- port traces `port-{nopace,pace,hold,grip}.tsv`;
- `local/robust/decomp/g1..g3`: Ghidra 12.1.4 headless, read-only, of 0x775BF0, 0x783180 and
  0x404C90. Capstone 5.0.6 (`pe.py`) for 0x7978D0.

## 1. The capture [V]

- 12 vehicles. Manager slot 0 is the player in grid slot 11; it stays parked at (−94, 29) and
  never moves. Slots 1–11 are the AI in grid slots 0–10, all with the same constant control
  and gear blocks, so one car model.
- Rows are written once per physics step of the original: 2715 rows over 43 s.
  - Between the go and the first corner the step is 15 ms on average (about 67 Hz).
  - It varies from under 8 ms to over 30 ms: unlocked, as the coordinator noted (F419).
  - The capture has no world +0x7C8. Its QPC time agrees with the speeds: distance travelled
    against ∫v dt is 0.97–1.02 in every window from the go to 34 s. So simulation time is
    wall time, and the step deltas below are taken from QPC.
- Timeline: the field is a restart.
  - The cars are placed on the grid at 2.25 s and held to the go at 9.04 s.
  - They reach Redgate (x < −520) 8.6–9.9 s after the go and run on through the Craner
    Curves to 43 s.

Car+0xE1C is the throttle. The doc comment of `tools/capture_grid_field.py` calls +0xE3C a
steering request; in this capture it equals the throttle.

## 2. The grid hold [V live, D static]

Live:
- From placement to 6.0 s (3.7 s), every AI car's throttle is 0.
- From 6.0 s to the go (3.04 s), every car's throttle is exactly 1.0, without pulses.
- At the go the throttle stays 1.0 for about 0.1 s. It then drops to 0.50 and ramps back
  up: the traction-control limiter.

Static:
- The throttle 0x783180 first calls the grid rev 0x775BF0 with the request's start state
  (request +0xA8).
  - States 2 and 3 give the random pulses the port already has.
  - State 1, and any state above 3, gives 0.
  - Below 1, 0x775BF0 clears its "revving" flag (+0x17), and 0x783180 then returns 1.0
    if request flag +0xAC is set, otherwise the usual minimum of the limiters.
- The request setup 0x7978D0 (0x797960–0x7979AE) fills those fields every frame.
  - It reads S = [[ctx+0x38]+0x18] and a held-state object X = [[[ctx+0x30]+4]+0xC]:
    X+8 means held, X+0xC means no rev.
  - While held: S > 1 → +0xA7 = 1 and +0xA8 = S (pulses).
  - S = 1, or X+0xC set → throttle cap +0x7C = 0.
  - S ≤ 0 with X+0xC clear → +0xAC = 1, full throttle.
  - Not held: +0xA8 = S when S ≠ 0.

So this restart held the field in state 1, then for the last 3.04 s in state 0 with full
throttle. The port's app and soak hold use states 2 then 3 (AUTHORED [I], F298). Who writes
S, and whether a normal race start (not a restart) uses 2/3, are not found yet.

The grid hold does not explain the launch gap. The port's held engine sits at the limiter
(~900 rad/s, `vehicle+0x1894`) at the go, and its launch is the same with or without the
hold (§3).

## 3. The launch: the main difference [V]

Speeds after the go (m/s, slots 0–10):

| Time after the go | Original | Port, no hold | Port, hold 3.7 s + 3.04 s | Port, hold + grip 1.25 from the go |
|---|---|---|---|---|
| 1 s | 15.4–17.1 | 8.4–8.5 | 8.4–8.5 | 10.6–10.7 |
| 3 s | 36.1–37.0 | 30.0–30.4 | 30.0–30.5 | 32.2–32.9 |
| 5 s | 49.1–50.0 | 43.1–44.0 | 43.2–44.0 | 44.5–45.5 |
| 7 s | 57.1–59.0 | 52.1–53.1 | 52.1–53.1 | 53.3–54.2 |

- The original gains 20–26 m/s² in its first half second (for example car 1:
  1.06 → 3.13 m/s in 92 ms) and 15–18 m/s² up to 1 s.
- The port gains 8 m/s² until 10 m/s, traction-limited with the engine at the limiter.
  Above 10 m/s, where the race-start grip factor 1.25 (0x404C90) applies, it gains 17 m/s².
- From 10 m/s up, both accelerate alike: about 10, then 6–7, then 4.5 m/s².
- The original's field launches in two groups. Slots 0, 1, 5, 6 and 7 reach 17.1 m/s at 1 s;
  slots 2, 3, 4, 8, 9 and 10 reach 15.4–15.5. The cause is not known (characteristics?).
- The 10 m/s gate of 0x404C90 is static [V]: car-state +0xD0 (speed in m/s,
  AI_AVOID_RECOVERY_LIVE) compared with 10.0 (0xCAFA58).
- Giving the port 1.25 from the go still leaves 10.7 against 15.4–17.1 m/s. So the grip factor
  alone does not close the gap.
- Open [I]: the AI's car-state speed may keep its pre-restart value (the cars were doing
  47 m/s before the restart) while the field is held, so the 1.25 factor would apply from the
  go after a restart. The rest of the gap needs more than that. A capture of a normal start,
  with the wheel slip and the vehicle's grip scales (wheel +0x80), would settle it.

Consequence: the port reaches Redgate 0.9–1.2 s later than the original (8.6–9.9 s), with the
field in the same order and the same staggered spacing.

## 4. Lines, speeds and contacts through Redgate [V]

Neither field runs side by side on the run to Redgate. Both keep the grid's staggered file,
because the grid slots are offset by about 4.5 m. Side-by-side pairs (|longitudinal| < 5 m,
lateral < 4 m) form only in the braking zone and the corner.

| | Original | Port (hold) |
|---|---|---|
| Pairs side by side, per second from the go | 9 s: 1; 10 s: 3; 11 s: 3; 12 s: 2; 13 s: 2 | 11 s: 1; 12 s: 5; 13 s: 1 |
| Closest centre distances, first 20 s | slots 0–1 1.77 m (11.8–12.4 s), 8–10 2.0 m, 8–9 2.57 m | 0–1 2.42–2.56 m |
| Minimum speed in Redgate (x < −500) | 24.7–37.0 m/s (front 32–37, back 25–28) | 18–32 m/s (front 25–32, back 18–22) |
| Retirements in 2400–2800 ticks | none (all 11 run on to 43 s) | none |

- The original's field gets through Redgate without anyone dropping out. At 11–13 s some
  pairs run with 1.3–1.8 m lateral gaps, less than a car's width. So it probably touches too
  [I: no contact data in the capture].
- The port brakes deeper. The back of its field slows to 18–22 m/s against 25–28 in the
  original. AI braking belongs to the drift-battle-ai session (F417 follow-ups) and is not
  changed here.
- Pacing (`GRID_REFERENCE_CAR=11`, the parked car as the reference) changes none of this.
  The port's race-start grip only switches on at 10 m/s.

## 5. Harness options (grid-opponent-race, research only)

None of these changes the default output. Identities are unchanged.
- `GRID_PARKED_CAR=k`: car k holds the brake, as the idle player at the back of the capture.
- `GRID_GRID_HOLD=idle,full`: every car is held on the handbrake, first at throttle 0, then
  at full throttle (start states 1 and 0), before the AI drives.
- `GRID_FIELD_TRACE=local/X.tsv`: per tick every car's position, speed, controls and
  behaviour, and the contact pairs.

Port runs: `GRID_PARKED_CAR=11 [GRID_GRID_HOLD=3.7,3.04] [GRID_REFERENCE_CAR=11]
GRID_FIELD_TRACE=… grid-opponent-race ROOT CD01 BP01 12 2800 ai`, with DF01 and the
characteristics.

## 6. Next

1. Who writes the start state S ([[ctx+0x38]+0x18]) and the held object. Then port the hold
   (states 1 and 0, full throttle) in place of the authored 2/3 schedule.
2. A live capture of a normal race start (not a restart) at the 60 Hz lock, adding car-state
   +0xD0 and the wheels' grip scale (+0x80) and slip, to settle the launch.
3. The deeper braking in Redgate (AI braking session).

## 7. The launch, ported (F444)

The launch gap of section 3 was the missing AI traction control 0x77AA10
(docs/AI_DRIVER_PORT.md "Traction control and ABS cap").
- The port's front wheels rolled at about 19.5 m/s while the car did 0.2–6 m/s.
- The original cuts the throttle on that spin. The live trace shows the cut: 1.0 → 0.50 at
  0.1 s, then a ramp.
- Ported with the ABS cap 0x77A2E0, which shares the same wheel inputs.

[V] Port, field held as in the capture (`GRID_GRID_HOLD=3.7,3.04`, `GRID_PARKED_CAR=11`):

| | Original | Port before | Port with TC + ABS |
|---|---|---|---|
| Speed 1 s after the go | 15.4–17.1 m/s | 8.4–8.5 | 13.8–14.0 |
| Speed 3 s after the go | 36.1–37.0 | 30.0–30.5 | 33.5–34.0 |
| Redgate reached (x < −520) | 8.60–9.93 s | 9.70–11.07 | 9.30–10.68 |
| Leader's minimum in Redgate | 37.0 m/s | 32.1 | 32.6 |

- The rest of the launch gap is open. Candidates [I]: the race-start grip factor (section 3,
  a stale pre-restart speed), and the original's two launch groups.
- Redgate: the leader alone is 4.4 m/s slower than the original's, so the port's deeper
  braking there is the base corner speed (brake lines and corner speeds), not traffic. The
  follow brake 0x77A1E0 matches the port 1:1 [D]. Left to the AI braking session.
- [V] Jarama route 1, 12 cars, DF01 and characteristics, 12,000 ticks: no retirement (the
  pile-up of ROBUSTNESS_RESEARCH §4 retired cars 5, 6, 7 and 10).

## 8. Milan and Spa captures (F450)

2026-10-19, 02:39–02:55 UTC. Research copy only (SHA-256 b1da1b2d…aece34a6), PID 37284.
- Memory reads only; Steam install untouched. 11 of the 19 user files changed and were restored;
  all 19 are hash-verified. The game closed on the close request.
- Evidence: main checkout `local/driving/field-20261019/` (`milan01.tsv`, `spa01.tsv`, manager
  scans, `world-*.json`, `shots/`, `user-before*`), decoded and compared in this worktree's
  `local/firstcorner/` (`launch.py`, `live2/`).
- Setup: Race Day, Touring Cars, Chevrolet Lacetti, Normal, grid Back, 3 laps; each 60 s from a
  restart. Spa replaces the planned Jarama route 1: **Touring Cars does not offer Jarama**. Its
  locations are Okutama, Detroit, Washington, Spa, Milan, Nürburgring, Donington, Le Mans,
  Yokohama and Shibuya.
- Frame rate: unlocked. World +0x7C8 was 6.1–7.4 ms; the median step after the go was 6.3 ms
  (Milan) and 5.7 ms (Spa), against 13.5 ms in field02.
- The manager scan ran from a scratch copy of `tools/find_reference_vehicle_manager.ps1`, with
  `[Convert]::ToHexString` swapped for `BitConverter` (no PowerShell 7 on the machine).

**Findings [V]:**
1. **The hold is the same on all three tracks.** Throttle 0 from the placement, then exactly 1.0
   for the last 3.02–3.06 s before the go. The whole hold lasts 6.3–6.8 s (§2's start states 1
   and 0).
2. **The AI field is mixed, by grid slot.** The race results list BMW 320si and Chevrolet
   Lacetti. On every track slots 0, 1, 5, 6 and 7 launch to 17.0–17.6 m/s at 1 s and slots 2, 3,
   4, 8, 9 and 10 to 15.0–15.7 m/s. The faster group is the BMW [I]: the port's BMW is the faster
   launcher of the two as well. This also explains §3's "two launch groups".
3. **The launch is barely step-sensitive.** Donington at 13.5 ms and Milan/Spa at 5.7–6.3 ms give
   the same speeds within about 3 %.
4. **Port against original, Spa, mixed field** (BMW / Lacetti means, the field held as in the
   captures):

   | After the go | Original | Port (F444 + F447), no race manager | Port with `GRID_REFERENCE_CAR=11` (F460) |
   |---|---|---|---|
   | 0.5 s | 9.3 / 6.9 m/s | 9.0 / 6.4 | |
   | 1.0 s | 17.0 / 15.0 | 15.7 / 13.1 | 16.8 / 14.7 |
   | 2.0 s | 27.7 / 26.6 | | 27.5 / 26.4 |
   | 3.0 s | 36.1 / 35.3 | 34.0 / 33.0 | 36.0 / 35.1 |

   **Correction (F460, drift-battle-ai):** the gap in the third column comes from the test setup,
   not from the port. `grid-opponent-race` applies the race manager (race-start power ×1.05,
   grip ×1.25) only with `GRID_REFERENCE_CAR`, and the original's Race Day always has it. With
   the parked car 11 as the reference, the port matches the original within 0.3 m/s; throttle and
   the 1→2 shift time already matched. Live comparisons need `GRID_REFERENCE_CAR` (and the mixed
   field).
5. **Milan launch steering.**
   - In the original, steering (car +0xE30) is 0 through the hold. At the go it reaches about
     ±0.32–0.40 within two frames and decays to 0 by 0.5 s. All cars launch alike (14.7–17.6 m/s
     at 1 s).
   - In the port several cars steer to full lock (±1.0), slots 7 and 9 to the opposite side.
     Slots 9 and 10 brake at 1.5 s, and the field is at 11.9–12.8 m/s at 1 s (without the race
     manager).
   - On main with F455 (the standing-start `race_start` behaviour and the lateral PID 0x7892E0)
     and the race manager: 14.0–17.1 m/s at 1 s, no car contacts (drift-battle-ai, F460).
   - Below 3 m/s both plan at the car's own lateral offset. The port's heading error to the
     look-ahead target at standstill is therefore at least 12° against about 4° in the
     original: the standstill look-ahead target differs (open).
6. Car +0xE3C equals the throttle in all captures. The steering is +0xE30. The decoder's
   `steer` column reads +0xE3C and is relabelled in the notes.

Next:
- the standstill look-ahead and target point (`update_look_ahead` below laSpeedMin) against the
  original at the Milan grid;
- ~~the 0.5–1 s launch gap~~: the race manager's race-start factors (F460);
- port comparisons need the mixed field and the race manager: `GRID_REFERENCE_CAR=11 … --cars bmw,bmw,lac,lac,lac,bmw,bmw,bmw,lac,lac,lac,lac`.

## 9. First-corner queues against the original; alongside parts, observer list and path scan ported

2026-10-26, Claude Code (drift-battle-ai). The owner approved a live x32dbg capture at Jarama
route 0 with touring cars; it was **not run**: Race Day Touring Cars does not offer Jarama
(§8). The three existing 12-car touring captures (Donington field02, Milan01, Spa01; one
car model per grid slot as in §8) already pass through corners where the port loses cars, so
they were compared with the port first. No game run, no user files changed.

Evidence (ignored, this worktree `local/alongside/`): `live/queue.py` (queue geometry from the
go: nearest car ahead in a 2.2 m corridor, bumper gap, overlaps; time-weighted), `live/stall.py`
(the matrix's "above 10 m/s, then below 3 m/s within 30 s" rule), `live/corner.py`,
`live/spread.py`, `live/offref.py` (lateral offset from the live leader's path), port runs
`live/{base,all,final}-s{0,1,2}/` with `GRID_PARKED_CAR=11 GRID_GRID_HOLD=3.7,3.04
GRID_REFERENCE_CAR=11 --cars bmw,bmw,lac,lac,lac,bmw,bmw,bmw,lac,lac,lac,lac`, the alongside
decision trace `live/al-don.err`, matrices `matrix3.log`, `matrix4.log`.

**Observations [V]** (25 s from the go, AI cars):

| | original Donington / Milan / Spa | port base, 3 seeds (range) |
|---|---|---|
| a car ahead within 3 m bumper gap | 7.3 / 14.0 / 9.3 % of the time | 3.7–6.0 / 0.9–1.7 / 6.3–8.1 % |
| bumper gap p5 | 2.25 / 0.48 / 1.35 m | 0.4–2.9 / 4.2–4.8 / 0.0–1.1 m |
| overlapped (along < 4.6 m and lateral < 4 m apart) | 8.6 / 19.6 / 8.5 % | 6–9 / 1–6 / 8–15 % |
| cars stalled within 30 s | 0 / 0 / 0 | 1–2 / 0–1 / 2–3 per seed |
| Spa leader's minimum at La Source | 20.9 m/s | 16.4–16.6 m/s |

- The original queues nose-to-tail at least as tightly as the port (Milan's tail runs under
  0.5 m bumper gap 5 % of the time). So the faithful gap-0 "alongside" of a car directly ahead
  (0x795C00) is a normal state in the original, not the cause of pile-ups.
- Lateral spread from the leader's path through La Source is the same (mean 2.9 m original,
  2.3–2.8 m port).
- Spa: the port's cars 7 and 9 hit the hairpin wall at about 10 m/s (no car contact), behind a
  leader 4–5 m/s slower than the original's. The corner-speed gap is the AI braking/corner-speed
  topic, not traffic.
- Donington: the port's grid slots 2/4/6 drift 1–2.6 m further left than the original's before
  Redgate (slot 4: −4.1 m against −1.5 m at 8 s). Slot 4 is in Alongside with a car on its right
  at 1.2 m gap; the min-space move takes its planned offset to −2.9 m while the racing limit at
  the car is −1.0 m (the clamp uses the look-ahead limits, as 0x7878F0 does). It leaves the
  road at 58 m/s, scores Recovery, holds full lock and hits the wall at 10 s, in every variant.

**Static fix [V]:** 0x798E90 fills view +0x218/+0x21C (track) and +0x220/+0x224 (racing
limits) from the +0x150 block **at the car**, and +0x228/+0x22C and **+0x230/+0x234 from the
+0x90 block at the look-ahead**; 0x797190 returns (block planned offset + line length) − left
limit and right limit − that. AI_OVERTAKING_RESEARCH §1 had the two the other way round, and
the port followed it in five readers. Now the room at the look-ahead is used by the race start
(0x78A740 room, 0x776740 side), the overtake side choice (0x77BD20) and the yield (0x789780),
and the alongside squeeze (0x799630) is `room_ahead − ((room_car + asTargetBuffer) − g) <
minSpace`. The room at the car uses the planned offset +0xE0 for the car link's own +0x1A0,
which the port does not keep [I].

**Measured** (24 first-corner starts: Jarama 0/1, Long Beach 0/1/3/4, Detroit 1, Nürburgring 0
at 10/11/12 cars, 2000 ticks; cars lost per seed offset):

| variant | s0 | s1 | s2 | s3 | s4 | s5 | sum | 12-car starts | contacts |
|---|---|---|---|---|---|---|---|---|---|
| old main | 40 | 53 | 43 | 39 | 45 | 66 | 286 | 103 | 196k |
| room fix only | 37 | 63 | 28 | 34 | 37 | 60 | 259 | 98 | 184k |
| room fix + alongside parts + observer list + path scan (adopted) | 39 | 38 | 63 | 29 | 38 | 33 | 240 | 83 | 171k |
| (seeds 0–2) fix + alongside parts only | 53 | 65 | 53 | | | | | | |
| (seeds 0–2) fix + parts + scan, no list | 46 | 38 | 55 | | | | | | |
| (seeds 0–2, before the fix) all three | 60 | 54 | 69 | | | | | | |

The seed spread (28–66) is larger than any one difference; over six seeds the adopted
variant loses 16 % fewer cars than old main. On the three live-capture tracks it is unchanged
(12 stalls over 3 seeds, as old main); the original loses none. Those stalls are the La
Source corner speed and the Donington run-off above, not the parts ported here.

**What a live capture would still settle:** the planned offsets +0xE0/+0x1A0 and the
alongside fields (+0x0C/+0x10 gaps, +0x24/+0x25) of the cars at Donington's run to Redgate,
grid slots 2–6, 5–9 s after the go (Race Day, Touring Cars, Donington GP, grid Back): whether
the original's min-space move pushes a car past the racing limit at the car as the port's
does. Breakpoint logging at 0x799630 entry (ECX test object, view = [arg+0x18]: +0xE0,
+0x1A0, +0x220..+0x234) and 0x795C00 return, read-only.

## 10. La Source: the corner lift works only above the line target

2026-10-27, Claude Code (drift-battle-ai). Static; the original was not run. Evidence (ignored,
this worktree `local/corners2/`): `live.sh` (the §9 12-car runs with the prepush
characteristics), `run.sh` and `matrix*.sh` (the §9 24-start matrix, with characteristics),
`lead2.py`, port runs `base-s*`, `gate*-s*`, `m-*-s*`, logs `matrix.log`, `matrix2.log`.

**Observation [V]:** Spa01 leader (live car 1) against the port's car 0, seed 0: both brake from
54–55 m/s at the same place, with the same pedal (0.58 → 0.91), to 23.5 / 22.6 m/s. After the
release the original's throttle is 0.6–1.0 through the hairpin (21–24.7 m/s). The port's sits
at 0.2, `thCornerLiftMin`, and the car slows to 16.9 m/s without braking.

**Static [V]:** 0x77A940 starts with `comiss [car state +0xD0], [ecx+8]; jbe → return 1`: the
corner lift applies only while the speed is above controller +0x8. The governor 0x77A4C0 reads
the same field as the active line's target (it subtracts `thBrakeLineSpeedDrop` from it), and
0x7886C0 writes it from the request line's target (`[ebp+0x20]`) whenever the class data is
valid. That is the port's `lines.target`. The port applied the lift at any speed; now only above
the target.

**Measured** (12-car Donington / Milan / Spa as §9, seeds 0–2, cars stalled within 30 s of the
go; and the §9 matrix, 24 starts × seeds 0–5, now with `GRID_AI_CHARACTERISTICS`):

| variant | live tracks, stalls in 3 seeds (Spa) | matrix lost cars | in 12-car starts | contacts |
|---|---|---|---|---|
| before (main 0947dd94) | 17 (13) | 220 | 83 | 178k |
| corner-lift gate | 4 (0) | 203 | 80 | 172k |

Spa leader at La Source: 22.0 m/s in seed 0 (original 20.9; before 16.4–16.6). The four stalls
left are Donington (slot 4 in seeds 0–2, see the next section) and Milan car 10 at 27.6 s.

## 11. Donington: the car-link planned offset and the limit clamp

2026-10-27, Claude Code (drift-battle-ai). Static; the original was not run, and §9's proposed
x32dbg capture is not needed for this. Evidence as §10, plus `off-s2/` (offset trace of seed 2).

**Observation [V port]:** in Alongside, with a car on its right inside minSpace, slot 4's request
is its own look-ahead offset minus minSpace (0x799630, ported faithfully in §9), so the planned
offset walks left every tick: −0.0 → −4.1 m in 1 s at 56–59 m/s before Redgate. The left racing
limit there is 0.5–1.0 m from the line (look-ahead and car), so limit + half width lies right of
the line, and the port's clamp let the offset pass. The car leaves the road and goes into
Recovery.

**Static [V]:**
- 0x782D20 (called by both 0x7878F0 and 0x788030) measures the signed lateral `s` of the limit
  point moved inward by the half width (0x782FE0 with ∓view+0x60). If `s` lies between the line
  and the offset, the offset becomes `s` (0x779CB0 writes block +0x50 and the planned point).
  Otherwise, with the offset on the left (right) and `s` on the far side, right (left) of the
  line, `s` is cleared and the offset becomes 0. AI_DRIVER_RESEARCH §3 item 8 had "does not move
  it" for the second case, and the port followed it.
- 0x788030 moves the car-link block's planned offset +0x1A0 toward request +0x98 at request
  +0x9C, like 0x7878F0 does +0xE0, and stops it at the limit links of the car's block
  (+0x210/+0x214, 0x782D20). The lateral PID 0x7892E0 measures its error from +0x190 (line point
  + lateral × +0x1A0), and 0x797190 computes the room at the car (+0x220/+0x224) from it. The port
  kept only +0xE0 and used it for both.

**Port:** `stop_at_limits` with the far-side case, for both offsets; a second planned offset
`offset_here` that moves with the same request and rate, is stopped at the limits at the car
(racing, or track when the request selects track limits), feeds the PID error and the
squeeze's room at the car, and is reset and shifted with `offset`.

**Measured** (as §10; the first row is §10's gate, now on main):

| variant | live tracks, stalls in 3 seeds | matrix lost cars | in 12-car starts | contacts |
|---|---|---|---|---|
| corner-lift gate (F601) | 4 | 203 | 80 | 172k |
| + car-link offset only | 2 | 243 | 83 | 153k |
| + clamp correction only | 1 | 137 | 50 | 168k |
| + both (adopted) | 1 | 109 | 47 | 146k |

Per seed (adopted): 13 8 21 30 30 7; gate only 19 37 57 26 27 37. Donington slot 4 no longer
runs off in seeds 0–2; the one stall left on the live tracks is Milan car 10 at 27.7 s, after
the first corner. Against old main (F585, characteristics set, 220) the matrix loses half as many
cars.
