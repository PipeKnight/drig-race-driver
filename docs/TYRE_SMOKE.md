# Tyre smoke — port

15 October 2026, Claude Code. Implements the original kickup smoke recovered in
[TYRE_SMOKE_RESEARCH.md](TYRE_SMOKE_RESEARCH.md) for the player and every AI opponent.
Presentation only: it reads vehicle state and never feeds physics.

## Code

- `grid_tools::tyre_smoke`:
  - Shipped data: `read_xml` (binary or plain XML), `parse_system` (every
    `effects/particle_systems/*.xml`), `parse_kickups` (`particleMaterials.xml`
    `<kickup><dry>`), `parse_surface_particles` (`surface_materials.xml` token → PARTICLE),
    `load_textures` (`effects/textures/pfx_textures.pssg`).
  - `Library`: every K_* system named by a kickup material, keeping only the smoke-textured
    emitters (SM33, SM33_dark, SM38, Wisps08_32, Wheel_spin). Mud, grass, stone and sand
    bits are left to ground effects, as agreed with that session.
  - `CarInput::read`: per-wheel token, contact point, spin, load, wheel+AC, wheel+FC,
    wheel+148 and steering words, plus car pose, velocity, speed, mass, handbrake and
    body+1008.
  - `Smoke::tick`, the driver: slip, rear spread lift, system choice, reverse slot,
    intensity, budget and distance level of detail, material respawn and time reset.
  - `Pfx`, the particle runtime: held systems, births by age, rate/speed live control,
    shapes, divergence cone, inherit, gravity, linear drag, angular drag, age curves,
    frequency atlas frames, original LCG.
- `grid_formats::texture::read_named`: reads chosen PSSG textures and
  accepts uncompressed `ui8x4` (`Format::Bgra8`, written as an A8R8G8B8 DDS). SM33,
  SM33_dark and Wisps08_32 use it.
- `grid-app/src/tyre_smoke.rs`:
  - Loads textures at runtime from the install; nothing is copied into the repository.
  - Advances the simulation once per successful physics tick (player and opponents).
  - Draws one alpha-blended mesh per texture: camera-facing quads, or horizon-aligned for
    Burnout_circles, sorted back to front, without shadows.
- `main.rs`: module registration, plus the validation flag `--handbrake FROM_TICK`
  (scripted handbrake from that tick on).

## AUTHORED (not recovered)

- **Material:** scene-lit diffuse billboards (roughness 1, reflectance 0) instead of the
  original `pfx_half_smoke` shader. Not reproduced: its per-particle light sample, its
  half-resolution buffer, and the near-camera fade. An unlit trial with the data colours
  (≈0.5 grey) looked sooty (render `handbrake-lock-250.png`); the lit version reads as light
  smoke (`handbrake-lock-225-lit.png`).
- **Data colours** are treated as display-referred (sRGB) and converted to linear vertex
  colours.
- **Alpha scale:** particle alpha = age alpha × the driver's frac copied at birth. The
  renderer's use of frac is inferred.
- **Update rate:** the driver and particles run at the 62.5 Hz physics tick, not the display
  frame rate.
- **Particle pool cap** is 12000 (config.xml `totalParticles`). The executable does not read
  that name; its own pool limits are unknown.
- **RNG seed** for the original LCG.
- **Simplifications:**
  - The contact point uses the port's wheel centre (anchor + deformation + axis·travel)
    minus radius (+E0).
  - The hit-material height (+A4) is not added.
  - The car-local frame is not offset by body+1630.
- **Not modelled:**
  - Drift mode (`Drift_effect` never runs; its 2-particle burst does).
  - car+0x64 (LOD ×3) and car+0x65 (AI rate ×0.15).
  - Blown-tyre rim sparks.
  - Wet kickup (the original never parses it).
- **Stopped systems:** emitter-relative particles (Burnout_circles, 0.2 s life) vanish with
  their stopped system.

## Validation

All runs use renders under `local/tyre-smoke/renders/` (private), with chase camera and
body contacts/events/deformation enabled.

**Asset test `launch_and_lock_emit_smoke_reproducibly`** (headless Lacetti, wheel order RL,
RR, FL, FR):
- Full-throttle launch: only the front wheels smoke (slip to 7.8 m/s², rate to 4.1×,
  ~300 particles). Two runs are identical.
- Full lock plus handbrake at speed: the rear wheels smoke, with rates capped at 5×.

**Renders:**
- `launch-100.png`: faint front-wheel smoke at 60 km/h. The player's front-drive Lacetti
  uses `K_Tarmac_Front`, which has only two small Initial_cover emitters. The first run
  showed dark streaks on the road from quad shadows; particles no longer cast or receive
  shadows (`launch-100-noshadow.png`).
- `handbrake-lock-250.png` / `handbrake-lock-225-lit.png`: steering 1, throttle 1,
  handbrake from tick 190. The car spins with a rear smoke cloud (rear slip 8.7–13.6 m/s²,
  rate at the 5× cap, 329 particles).
- `race12-start-330.png`: 12-car start. The player shows faint front smoke; AI front wheels
  have no tarmac smoke, as in the original.

**Windowed 12-car race** (GRID_AUTOPILOT, 11 opponents with characteristics, 3600 ticks):
- 57.6 s of simulation in 58.6 s wall time (real time).
- Smoke from the player and AI cars, up to ~100 particles.
- No slab-allocator errors after the empty-mesh fix (the first renders logged
  `Use-after-free` from zero-vertex smoke meshes; each mesh now keeps one transparent
  degenerate quad).

**Checks:** full workspace tests including asset tests, 290 passed; Clippy `-D warnings`
clean.

Not compared side by side with the running original. Owner feedback on look and density is
needed.


## Bounded grouped-direct boundary control (G274)

`--smoke-boundary grouped-direct` is default-off and restricted to the existing
mapped Detroit stock-car GroundFirst120-frame fixture, with CT disabled. It isolates
five existing smoke groups before batching, validates prepared spans, suppresses
their ordinary main-phase ranges, and replays their original commands after normal
main transparency into the same scene colour/depth/MSAA attachments. It restores
all retained keys/ranges/draw IDs unconditionally. Materials, shaders, meshes,
source emission and simulation remain unchanged.

A success marker executes only after the real material draw succeeds; render Ok
without markers cannot certify coverage. Every expected screenshot is registered
before readiness checks and must carry matching Extract entity/frame/tick through
barrier, preparation, replay and restoration. All120 rows and original PNG joins
plus a flushed sidecar gate the final capture. Missing stages, skipped draws,
duplicates, mixed batches and partial errors reject while restoring surviving state.

357 tests and Clippy pass. A new baseline executable completes both same-build
GPU arms with matched source traces/camera/geometry/phase sort state. All120 mapped
rows restore exactly; one to four successful batch heads cover five logical groups.
This counts encoded commands, not positive smoke fragments or GPU completion.

51 of120 sequence frames match exactly. The largest difference is0065:9434pixels,
maximum channel82. Native paired review shows its conspicuous driver safety net
becoming naturally attenuated by the cloud.0074 and0090 remain pixel-exact; the dark
wheel rings at0090 remain unresolved. Final405pixels differ, channel maxima40/41/42,
alpha unchanged. No universal tolerance or explanation for every residual is assumed.

This admits the transaction and the local0065 occlusion improvement. It does not
implement colour/transmittance accumulation or the original reduced-resolution
compositor, fix BMW glass/lenses, change defaults/playable launchers, or establish a
performance gain. A separate exact BMW profile is needed. Private evidence and all
actual hashes are retained in `local/graphics/smoke-boundary01/` and G274.
