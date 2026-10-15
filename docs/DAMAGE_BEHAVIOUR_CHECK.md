# How damage changes driving in the prototype — F281

15 October 2026. Owner question: does car damage affect behaviour on the road, and how?
This is a measurement of the current Rust prototype, not an original-game comparison.

## What accumulates in real play

The owner's three-lap time trial (`local/playtest/manual-6944-21324.log`, 1550 handling
samples) reached handling-damage wheel levels (body CDF0+18, RL/RR/FL/FR) of
0.018 / 0.014 / **0.653** / 0.137 and effect word 3 (CDF0+3AC) of 0.549. Two earlier
contact-free logs stay at zero. So collisions in the playable build do raise wheel damage
through the deformation -> visual wheel level path (HANDLING_DAMAGE.md F131/F132).

## Controlled straight-line experiment

New authored diagnostic `grid-damage-straight GAME_ROOT local/OUT.tsv TICKS THROTTLE
RL,RR,FL,FR` presets the four wheel levels on a fresh source car, then drives with
constant controls and records speed, drift from the initial line, heading change and
yaw rate. Full throttle, no steering, 500 ticks (8 s):

| Levels RL,RR,FL,FR | Effect 3 | Speed at 8 s | Lateral drift | Heading over the run | Yaw rate |
| --- | ---: | ---: | ---: | --- | --- |
| clean 0,0,0,0 | 0 | 57.281 m/s | +2.89 m | steady -0.60° (grid slot alignment) | ~0 |
| owner-like 0,0,.653,.137 | .545 | 57.264 m/s | -0.12 m | oscillates +0.38..-0.35° | up to 0.038 rad/s |
| front-left 1 | 2.0 | 57.167 m/s | -2.01 m | oscillates +1.14..-0.18° | up to 0.070 rad/s |
| rear-left 1 | 0 | 57.203 m/s | +3.02 m | steady -0.65° | ~0 |

Interpretation:

- Front-wheel damage produces **steering wobble** (heading/yaw oscillation growing with
  damage). Rear-wheel levels have almost no effect in this path.
- No constant pull: the installed `cars/Standard.hdm` sets `maxMisalignAngle` 0 (live
  reference capture F130), so misalignment cannot steer the car aside.
- Power is essentially unchanged (≤0.2% speed difference at 8 s).

## Not yet implemented

`Standard.hdm` also configures ENGINE (power loss, cut-outs, temperature), GEARBOX,
DRIVESHAFT, RADIATOR, EXHAUST, TURBO, BRAKE (`maxBrakeLossPercent`), SUSPENSION
(`maxDamageSpringLoss`), FRONTSPOILER/REARSPOILER (`rearGripPercentLoss`) and impact
thresholds (`min/maxForceToCauseDamage`, `min/maxCollisionAccToCauseDamage`). The
original's above-threshold collision distribution into these components (research
candidates 9C4B70/9C5250/999C00) is not implemented, so a crash in the prototype cannot
yet cost engine power, brakes, gears or aero grip. Part detachment, broken glass and
damage materials are also absent from the visual side (BODY_DEFORMATION_PRESENTATION.md).
These are the next damage targets; their original behaviour must be recovered before
implementation.

Private evidence: `local/damage/straight-*.tsv`.

## Owner report 15 October 2026: rear-left crash, "terminal front-left wheel"

Log `local/playtest/manual-8982-16959.log` (build 44d2e88, solo, deformation on).
Replayed headless with `grid-wheel-damage-trace` (new private diagnostic; DR01 from the log,
all 167 logged pose/velocity samples bit-exact).

- **Labels are correct.** Wheel records are RL, RR, FL, FR: the Lacetti anchors are
  (+0.761, −1.365), (−0.761, −1.365), (+0.741, +1.210), (−0.741, +1.210), with +X the car's left
  and +Z forward. The asset test `wheel_records_are_rear_left_rear_right_front_left_front_right`
  locks this.
- **Ticks 619–622: hard front-left impact.** Barrier contacts at x +0.6…+1.5 m and z +0.7…+1.95 m,
  impulses up to about 6,500. Front-left grid nodes are pushed back 0.3–0.5 m, and the
  front-left wheel level goes from 0.07 to 0.668.
- **Ticks 1566–1567: rear-left impact, as reported.** All contacts are at x +0.6…+0.8 m and
  z −2.0…−2.35 m. The grid nodes that move are at the rear-left (rest z −2.32).
  - The rear-left wheel level stays at 0.03.
  - Because the front-left cell is already folded (node 305 sits behind node 275's rest
    position), the 2–4 cm of grid movement that the hit spreads through the grid shifts the
    sampled front-left wheel anchor from −0.123 to −0.220 m (angle 0.30 → 0.48).
  - That raises the front-left level from 0.668 to 1.0 and gives terminal status 3, ordinal 2
    (front-left).
- So the terminal wheel is the one damaged earlier, and the final push came from a rear
  impact through the shared deformation grid. The wheel sampling (9BB1E0) and the bulk
  phases are each verified against the original. Whether the original couples a rear hit to
  an already crushed front wheel this strongly is **not yet compared live**.
- Proposed live check with the original: crush the front-left moderately, then hit a
  barrier with the rear-left and watch whether the car becomes undriveable.
