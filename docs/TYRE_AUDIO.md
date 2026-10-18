# Tyre and surface audio — F282

15 October 2026. Owner request: add tyre sounds. Implementation of the original chain
recovered in [TYRE_AUDIO_RESEARCH.md](TYRE_AUDIO_RESEARCH.md) (static analysis of
456640 -> 455410 roll -> 44F330 peel/slide -> 44FE60 skid/scrub). Spot checks before
implementation confirmed the peel routine's reads of wheel+F4/+104, body+15A0 and
wheel+138, the roll routine's wheel+40/+104, and the descriptor loop bit 0x100000 ->
AL_LOOPING (0x1007) at A2A14C.

## What plays

- Data come from the purchased installation: `audio/car_surfaces.WIP` (185 looped
  PCM16 samples at their 40 kHz descriptor rate, 4-byte prefix skipped like the engine
  banks), `CollisionInfos.bin` (curves; preset values then overrides), `audioVarieties.bin`
  (sample lists) and `audioMaterials.bin` (material -> preset; per-material ColInfo
  attributes ignored as in the original loader).
- Per wheel the surface comes from the contact token (wheel+C0) -> mechanics name in
  `surface_materials.xml` -> AudioMaterial of the same name (exact match).
- Player car, four wheels, five categories: roll (wheel angular speed), peel (wheelspin
  ratio, load-scaled, phase modulation), slide (locking ratio minus slip-angle term,
  jitter), skid (smoothed slip angle with speed boost, wobble/jitter), scrub (4-knot
  curve with random drop-outs). dB-linear curves, volume clamp [-48, 0] dB, pitch clamp
  [0.5, 2]. Surface changes restart voices on new samples (random, no immediate repeat).
- `grid_tools::tyre_audio::{Bank, Driver, Mixer}`; the app (`tyre_sound.rs`) advances the
  driver once per successful physics tick and mixes on the audio thread. M mutes engine
  and tyres together; pause, failure and the finished trial silence them.

## Authored choices (not recovered)

- F422 replaced the authored -12 dB bus (`BUS_GAIN`) with the original group mix:
  roll in Wheels, the rest in Skids (Normal/Chase -9/-8 dB), per-wheel distance law and
  the -48 dB floor gate ([AUDIO_RESEARCH.md](AUDIO_RESEARCH.md)).
- Driver runs at the 62.5 Hz physics tick; the original per-frame smoothers (0.2, 1/3,
  1/13) run at the display frame rate, so their time constants differ slightly.
- Mono mix without 3D panning/Doppler; kickup one-shots and carscrape are not implemented.
  AI-car tyres (wheel object 0, peel/slide/skid, AI group) were added in F422. Treble is unused by the original tyre code.
- Event-mode flags W+4D/W+4E assumed zero (ordinary race).

## Verification

- `grid-tyre-audio GAME_ROOT local/OUT.wav TICKS` renders an authored scenario (handbrake
  rev, launch, straight, 0.6 steer, braking): peel up to -2.8 dB at the clutch dump, roll
  rising to -12 dB on tarmac (louder on a rumble strip), slide/skid/scrub in the turn and
  braking; peak 0.3366; two runs byte-identical.
- Tests: `decibel_floor_and_dual_curve`, and asset test
  `source_launch_peels_then_rolls_reproducibly` (front peel > -10 dB after the
  handbrake launch, roll > -20 dB at speed, bounded/finite voices, reproducible trace,
  mixer peak in (0.001, 1)). Workspace: 255 default and 19 asset-dependent tests pass,
  all-target Clippy `-D warnings` passes.
- Not yet verified by listening against the original. Owner feedback needed.
