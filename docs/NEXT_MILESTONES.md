# Next playable milestones — 14 October 2026

Current build: 0cb4405, Lacetti/Donington practice and optional finite time trial with keyboard controls,
reverse/handbrake, static body/barrier contacts, damage/handling effects,
deformation, practice timing/HUD and continuous engine sound. Owner has driven
laps and collision-heavy sessions. The first drivable/timed-lap milestone is
already achieved; older roadmap text describing it as future is historical.

The owner reports that the new audio is definitely better but still more
artificial/mechanical than original GRID, and explicitly defers further engine
sound work. Preserve the current audio and its remaining fidelity gaps.
This is perceived improvement, not a1:1sound pass.

## Near-term target: validate the complete single-car event

1. Validate a bounded set of original-versus-prototype maneuvers: straight
   acceleration/gearing, braking, steady turn/steering release, frontal/glancing
   barrier contact and damaged-car recovery. Match known settings first
   (reference keyboard, assists On, automatic transmission); measure actual
   pedal/steering state and game-time evidence before comparing trajectories.
   The original F13brake macro did not demonstrate braking and must not be used
   as calibrated input until verified during the hold. Existing videos/logs,
   autonomous laps and offline original-code probes provide starting evidence.
   Fix demonstrated differences; do not tune merely to an assumed feel.
2. Implemented and released: configurable finite event, countdown, finish/results
   and private PBs, exposed by Play GRID Time Trial.cmd. Three actual-physics
   diagnostic laps complete and captured debug/release traces match. Existing
   free practice is retained. See [rules and evidence](TIME_TRIAL_EVENT.md).
3. Owner completion and saved PB across launches remain to observe; scripted
   collision/deformation comparison, pause/reset/terminal recovery and finite
   event checks pass. Original reference input capture currently needs a manual
   Enter at the title screen after bounded guarded confirms did not advance it.
   Continue independent shared-world work while awaiting that optional help.

## Following target: first race against one opponent

Introduce multiple independently owned cars in the same world and actual
car-to-car contact/damage handling before claiming an opponent race. The current
playable collision integration is one car against static peers. Start one
opponent on the purchased route, validate stable following/overtaking/contact
and finishing, then expand the grid. The existing autonomous practice controller
is an authored diagnostic driver, not reconstructed GRID AI.

## Later expansion

Improve camera/material/lighting/damage presentation and return to audio
load/gear/wobble, postmix/spatial/reverb fidelity. Add a second car or circuit
to expose hard-coded Lacetti/Donington assumptions, then broaden content.
Career/progression, original replay/Flashback and portable releases remain
larger milestones. Do not claim completion from isolated routine counts.
