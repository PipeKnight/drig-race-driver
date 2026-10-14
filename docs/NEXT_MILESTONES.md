# Next playable milestones — 14 October 2026

Current build: bc90482, Lacetti/Donington practice with keyboard controls,
reverse/handbrake, static body/barrier contacts, damage/handling effects,
deformation, practice timing/HUD and continuous engine sound. Owner has driven
laps and collision-heavy sessions. The first drivable/timed-lap milestone is
already achieved; older roadmap text describing it as future is historical.

The owner reports that the new audio is definitely better but still more
artificial/mechanical than original GRID, and explicitly defers further engine
sound work. Preserve the current audio and its remaining fidelity gaps.
This is perceived improvement, not a1:1sound pass.

## Near-term target: a complete single-car three-lap event

1. Validate a bounded set of original-versus-prototype maneuvers: straight
   acceleration/gearing, braking, steady turn/steering release, frontal/glancing
   barrier contact and damaged-car recovery. Match known settings first
   (reference keyboard, assists On, automatic transmission); measure actual
   pedal/steering state and game-time evidence before comparing trajectories.
   The original F13brake macro did not demonstrate braking and must not be used
   as calibrated input until verified during the hold. Existing videos/logs,
   autonomous laps and offline original-code probes provide starting evidence.
   Fix demonstrated differences; do not tune merely to an assumed feel.
2. Add a clear start/countdown, a configurable three-lap practice event, finish
   state/results and persistent local personal-best records. Keep timing tied
   to successful simulation ticks; pause, reset and invalid laps must retain
   explicit rules. Save our settings/results outside Steam. This is an
   independently authored prototype event, not proof of original race rules.
3. Release and validate the complete event through the existing launchers,
   including a full clean run and collision/damage/reset cases. Preserve owner
   sessions and record revision/hash/checks. No immediate owner capture needed;
   ask for a specific maneuver only after recording is ready.

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
