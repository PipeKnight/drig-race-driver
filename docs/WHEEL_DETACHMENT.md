# Wheel detachment — F346

17 October 2026. Port of the plan in [MECHANICAL_DAMAGE.md](MECHANICAL_DAMAGE.md) section 5 (F339/F340).
Tags: [D] read from the code, [V] checked by a run or test, [I] interpretation, AUTHORED port choice.

Static evidence: hash-pinned `local/compatibility/grid-unpacked.exe` (SHA-256 `aac46a64…4514`). Tools: Ghidra
12.1.4 headless `-readOnly -noanalysis` on `local/phase3/ghidra-project` (JAVA_HOME = the Songs of Syx Demo JRE,
script `tools/ghidra/GridProbe.java`), capstone 5.0.6 sweeps with `tools/scan_code_displacements.py`, and a
call-target table (every `call rel32` in `.text`) to find the entry of an interior address. Private outputs:
`local/wheel-detach/` (scan-01 `+2278..+227B`/`+22E4..+22E7`, scan-02 `+2270`, ghidra-01..04, checks/),
plus the F339 decompilations in `local/detached-parts/`. No input was sent to the original game.

## 1. Summary

A wheel comes off when one tick's impacts on its zone exceed `m_wheel_detach_level` × mass ×
`m_overall_speed_required_scalar`. For both shipped cars that is 6 × 1140 × 4 = **27 360 per tick** of solved
contact impulse (+3C) [V data]. The original then:

- treats the car as wrecked on the next tick: status +444 goes from 7 to **4** with the wheel at +448
  (9E65B0 → 9E64E0(4, wheel)). Status ≠ 7 disables the controls (+226C), as for the existing terminal
  wheel damage (status 3, 9F2F80). In single player this ends the player's race;
- drops the suspension force, tyre grip and spin of that wheel; the bare corner rides on the suspension
  floor stop, so the car sits lower there and slides without grip;
- turns the wheel part (and the caliper when `m_calipers_come_off_with_wheels` is set) into a debris body
  with a random kick and the axle spin.

The port reproduces the trigger, the terminal status and every driving branch. The loose wheel is drawn
by grid-app with the original launch (kick and spin), but its flight is AUTHORED, not a rigid body.

## 2. Trigger [D]

- 98B430 (the loose-part user 9A7670 of each solved body contact) adds the impact to zone accumulators
  +7F0..+830 by car-local cell (VISUAL_DAMAGE_PARTS_RESEARCH.md). Wheel zones +824..+830 follow the wheel
  records RL, RR, FL, FR (record order verified by `multi::tests::wheel_records_are_…` [V]).
- 9A7670 forwards notifications only while car B13 and B14 are set; the damage option's Off mode
  (mechanical_damage `Damage::Off`) clears B14, so a car with damage off never loses a wheel [V test].
- 9EC9B0 runs once per tick (from 9FF4F0). First it adds the zones to the totals; if `m_disableLooseParts`
  (+1C24) is set it skips every rule. S = `FUN_00A69370()` (physical mass) × +1C44 (scalar), stored as f32.
  While parts +7BF1 is 0, for each wheel `+1C3C × S < zone` sets car byte +22E4 + i once (9ECBF1..9ECC96).
  The rule reads the tick's zone, not the total. The zones are cleared at the end.
- +1C3C is `m_wheel_detach_level`; the loader seeds it with 2 × `m_bonnet_detach_level` before reading the
  attribute. Lacetti and BMW: level 6, scalar 4, `m_disableLooseParts` 0, `m_calipers_come_off_with_wheels`
  0 [V `loose_parts::tests::lacetti_and_bmw_wheels_detach_at_level_six`].
- Not needed: 996F60 (a setter with no direct caller), parts +7BF1 (set only by game-mode code).

[I] 9EC9B0 is called from the car's scene update A18460 (no direct caller, a vtable slot). The port runs the
rule once per simulation tick, as the earlier research concluded, not per rendered frame.

## 3. Every reader of the flags [D]

The 9BD7F0 copier writes car +22E4..+22E7 to body +2278..+227B and sets +2270 = 1.0 if any is set
(already in `vehicle_sync`). Literal sweeps for all eight byte offsets and +2270 found these readers; the
entry of an interior hit is the nearest preceding call target, confirmed by decompiling it.

| Reader | Function | Detached-wheel branch | Port |
| --- | --- | --- | --- |
| 9C2516 | 9C1F60 suspension substep | impulse 0; travel +64 and speed +74 reset to 0 before gravity and the response loops; the axle means were taken before; the floor-contact loop has no check | `wheel_step::step_detached` (new) |
| A0129B | A00E10 suspension/grip | skips the tyre history 9BA490 and every grip modifier; grip view +5C + 4i = 0 and wheel +78 = 0 | `vehicle_grip` (new) |
| A036A7..A036D7 | A03670 driver preparation | wheel spin +104 = 0 right after the +1568 clock | `vehicle_driver_prepare` (new) |
| 9B29CD..9B2B0C | 9B29A0 automatic gearbox | the wheel is left out of the slip-ratio minimum | `vehicle_controls` (already) |
| 9E65DD | 9E65B0, called at the end of 9F3E80 | while status is 7, the first flagged wheel calls 9E64E0(4, wheel); 9E64E0 needs +442 and A14E40 (B13/B14) | `vehicle_handling` (new) |
| 9BAB4F | 9BAB10 wheel constraint | no constraint for the wheel | `wheel_contacts` (already); `vehicle_manager` refusal now per wheel |
| 9BDC1E.. | 9BD7F0 copier | +2278..+227B, +2270 | `vehicle_sync` (already) |
| 9B9BF7 (+2270) | 9B9A90, called at 9BA2B6 | a "stuck" test returns false when any wheel is gone | not ported (no port caller) |
| 9F7C02 | 9F6BE0 (from A18460) | node matrices of the debris | presentation |
| 9FADFD | 9FA6E0 wheel node update (from A18460) | skips the wheel's node; sets part 0x2E + i (and 0x33 + i when car +EDEB) to state 4 | grid-app (section 5) |
| 7EA1F1 | 7E9D60 (called from 98D7E5, A00B39) | copies the flag into a per-wheel record +6C | not identified, not ported |

The terminal transition is the only change in 9F3E80 itself: the refusal of +22E4 in `vehicle_handling`
was a guard, not an original branch. 9E65B0's second loop (levels +28.. ≥ 1.0) only returns early.

## 4. The car without the wheel [V]

Asset test `source_vehicle::wheel_detach` (release, Donington slot 0 with track body contacts): full throttle
for 2 s (Lacetti 21.5 m/s, BMW 23.8 m/s), then one front-left corner notification.

- An impact equal to the threshold does nothing; 1.01 × threshold sets only +22E6 (the front-right zone gets
  0.2 of it). The same hit again changes nothing.
- On the next tick the status is 4 with wheel 2 (`terminal_wheel() = Some(2)`, `lost_wheel()`), the body
  copy +227A is 1, and the wheel's grip +78 is 0 on every later tick. +226C is 1 from the tick after.
- A driven wheel (Lacetti front) spins again later in the same tick: A03670 zeroes +104 before the
  drivetrain runs, and the port keeps that order.
- After 4 more seconds both cars have stopped (0.02 / 0.04 m/s). The bare front-left hub sits
  7.8 cm (Lacetti) / 5.2 cm (BMW) below the front-right one. Suspension loads RL, RR, FL, FR: Lacetti
  4030 / 1099 / 0 / 4430 N, BMW 3562 / 1691 / 0 / 3896 N; the rest of the weight is on the corner.
- In the shared world (two Lacettis) car 1 loses a wheel; neither car retires, car 1 stops and car 0
  drives on (44.7 m/s after 5 s).

[I] The corner does not fall to the ground: the floor stop of 9C1F60 still meets the road at zero
travel, so the car rests on a stiff point without grip there. This follows the decompiled code; it has not
been compared with the running game.

## 5. The detached wheel as a body

Original [D]:

- 9FA6E0, for a flagged wheel and while parts are enabled (its `param_6`), calls 9DC270(part, 4) for wheel
  part 0x2E + i (wheel_bl, _br, _fl, _fr for records 0..3) and, when car +EDEB is set, for 0x33 + i
  (hub_caliper). It no longer updates the wheel's node.
- 9DC270 state 4 for a wheel-like part (98C560: discs 0x12..0x15, tyres 0x2A..0x2D, wheels 0x2E..0x31,
  calipers 0x33..0x36) stores in the part record:
  - +40 = spin × (1, 0, 0, 0), the spin +104 of the wheel record that 98C5C0 gives:
    0x2E → 1, 0x2F → 0, 0x30 → 3, 0x31 → 2, i.e. **the other wheel of the same axle**;
  - only while a deformation impact is active (car +C7A0 = 1): +30 = d × m, with
    d = normalise(|r₁ − 127.5|, r₂ − 127.5, r₃ − 127.5) × 0.0015686 (CRT rand bytes) and
    m = (r₄ / 255 × 5 + 2.5) × clamp(+C754 − 0.25, 0.75, 4).
- 9DB0E0 → 9D9F80 builds the debris body: bounds of the tyre node (0x2A + i) while the tyre is not
  deflated (+22E0 + i), else the wheel node; mass = box volume × density, capped at car mass × 0.015;
  shape by 9A8BB0 with descriptor D17150 (D17158 when car +BA4); created by A742E0.

Port (grid-app `damage_parts`):

- The rim and tyre meshes of a lost wheel leave the car; the disc and caliper stay (both cars have
  `m_calipers_come_off_with_wheels` 0).
- Launch: the car's velocity plus the kick above, the spin about the car's X axis from the axle partner's
  record (both [D]). [I] the kick is taken as a car-frame velocity.
- AUTHORED: the random bytes (seeded LCG), and the flight (`Debris::step`: gravity, ground plane at the hub
  height at the moment of loss, damped bounce). The wheel does not collide with cars or the track.
- Log line `drive_wheel_detached car= tick= wheel= impact_strength=`; the HUD says
  "Lost the front-left wheel; controls disabled; R resets".

## 6. Port

- `grid_tools::loose_parts`: NED config (moved from grid-app), 98B430 `classify`, `Frame`, the 9EC9B0 wheel
  rule `wheels_to_detach`. grid-app `damage_parts` uses the same classifier for its body parts.
- `Runtime::detach_wheels` after each committed tick (solo `tick_owned`, every car still in the shared
  world in `MultiRuntime::tick`); it writes +22E4 + i to the owner and to the frame's body copy (the manager
  refuses the tick if they differ). `Runtime::detached_wheels`, `Runtime::lost_wheel`; `terminal_wheel`
  now covers status 3 and 4, so grid-app, `grid-opponent-race` and soak retire a car that lost a wheel.
- grid-world: `wheel_step::step_detached`, `suspension_vehicle` (refusal removed), `vehicle_grip`,
  `vehicle_driver_prepare` (refusal removed), `vehicle_handling` (refusal removed, status 4 accepted,
  9E65B0), `vehicle_manager` (the unsupported-constraint guard now skips each detached wheel, as 9BAB4F).
- Behind the damage option: Off clears B14, which stops the rule ([V]).

## 7. Validation [V]

- Unit: `wheel_step::tests::a_detached_wheel_has_no_suspension_impulse_and_restarts_from_zero_travel`,
  `vehicle_handling::tests::a_detached_wheel_is_terminal_damage_for_the_first_flagged_wheel` (status 4 once,
  the healthy-receiver error, +442 clear), `suspension_vehicle` rollback with a detached wheel,
  `loose_parts::tests` (threshold equality, once only, `m_disableLooseParts`, the frame transform),
  grid-app `a_lost_wheel_launches_with_the_axle_partner_spin_and_the_impact_kick`,
  `only_rims_and_tyres_leave_with_a_lost_wheel`.
- Asset: `source_vehicle::wheel_detach` (section 4) and `damage_off_keeps_every_wheel`.
- Checks before the push: see PROJECT_LOG F346.

## 8. Not done / open

- The loose wheel as a rigid body (9D9F80: tyre-node box, density, mass cap 0.015 × car mass, 9A8BB0 shape,
  descriptor D17150) in the shared world, colliding with the track and cars. The dynamic-props pieces
  (`grid_tools::dynamic_props`, cylinders as twelve-sided prisms) are the likely route.
- 7E9D60's per-wheel record (+6C) consumer, and 9B9A90's caller 9BA2B6.
- No comparison with the running game, and no scripted real crash that loses a wheel through actual
  contacts: the asset tests inject one notification through the real classifier and frame.
- Ride height of the bare corner (section 4 [I]) against the original.
