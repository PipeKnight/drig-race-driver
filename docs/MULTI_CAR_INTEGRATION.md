# Shared-world car integration: next implementation boundary

F277 continuation,15October2026. This is a code/evidence dependency map, not an
implemented opponent or a claim that two cars already drive in one world.
Current playable0cb4405 remains one car with static peer contacts.

## Reusable verified behavior

The solver itself is not limited to one car: contact_groups accepts1..16 owned
bodies and64 contacts (validated domain; F317 raised the port's world guards to the
original PHYSICSENGINE defaults, 1000 bodies and 10000 contacts, see
[DYNAMIC_PROPS_RESEARCH.md](DYNAMIC_PROPS_RESEARCH.md#pools-and-world-bounds-f317)), with explicit active permutations and links. The original
[grouped frame/tick comparisons](GROUPED_PHYSICAL_FRAME.md) include357 multi-body
groups per variant and2749 two-body contacts. [Carried world ownership](CARRIED_PHYSICAL_WORLD.md)
already preserves selectors, dirty lists, reusable contacts and shared manager
state across updates. These proofs use supplied bodies/contacts, not two complete
source-initialized car jobs with geometric car-to-car contact and damage.

## Restrictions that must be removed through verified integration

| Boundary | Present contract | Required next behavior |
| --- | --- | --- |
| grid-tools/source_vehicle::Runtime | One car, one initial/cache snapshot, one timing state, one deformation registration | A shared world owning multiple car states and explicit per-car inputs/query caches/timing |
| vehicle_world::from_registered / calculate_owned | Exactly one world body; all car shape references select token1; remaining shapes are static | Owned car-to-body/shape mapping with validated shared active/group/dirty chains |
| wheel_contacts | Four wheel-prefix slots, car identity1 and static peer tokens2..N | Batch each car job into one shared prefix, remapping only documented identity fields |
| physical_world::advance_with_pool | Can carry multiple bodies but called once by the one-car wrapper | One common advance/group/solve/consume for all car jobs per timestep |
| body_geometry | Source car against purchased static track/boundaries | Two moving car proxies at advanced poses, with correct two-body contact points/materials |
| vehicle_world event dispatch | Exactly one registered car side; other registration explicitly0 | Dispatch each registered side with real peer ownership and independently verified event order |
| deformation registration / RNG | One selected owner; empty external notifications | Explicit routing of both owners plus verified shared/per-owner RNG aliasing and call order |
| Bevy presentation/audio | One car/camera/HUD and player engine | Per-car transforms; keep player sound until opponent spatial/audio behavior is implemented |

Do not run two independent Runtime ticks and call that a shared collision world:
that would advance/solve separately and lose coupled contact impulses. Preserve
the existing one-car path and release while the new boundary is validated.

## Concrete sequence

1. Build a private native comparison around two source-initialized car managers,
   then one common existing physical world. Establish canonical identity mapping
   for both cars, registered shapes, active/dirty/group links and pool storage.
   Start with empty geometry/body events; retain both full car/physical states,
   shared manager/world/contact bytes and RNG ownership. Prove no-contact
   independence, order/permutation behavior and late second-car failure rollback.
2. Add both wheel-contact producers with separate caller query/cache ownership,
   one bounded shared contact pool and the same material rules. Reproduce paired
   original/repeated-original/Rust whole updates and carried sequences. Reuse
   source constructors and normal force/ground paths; no replayed poses.
3. Recover/query actual moving-car collision proxies. Validate generated paired
   body records and common solved impulses in frontal/glancing/separating cases.
   A momentum sanity check supplements native comparison; it does not replace it.
4. Recover both-side registered impact/deformation callback and RNG ordering.
   Do not insert fake static peers, zero peer registrations or duplicate global
   random streams to make existing guards pass. Preserve complete world and both
   owner states on failed updates; verify damaged-car continuation and reset.
5. Run two actual source-physics cars headlessly on Donington, then expose one
   authored route-following opponent in Bevy with finish results. Revalidate
   the owner's one-car event/contacts/damage/pause/reset/PB path and refresh the
   release only after those checks. Original GRID AI, overtaking behavior and
   a full grid remain separate research, not implied by a second mesh.

Fresh original12-car ownership evidence:
local/behavior/general-vehicle-manager-20261014T2238592589563Z.json,
plus the current player/physical/world/job checks in [reference input](REFERENCE_DRIVING_INPUT.md).
These are sequential live observations, not a native two-car execution test.
Existing Rust1.99, native MSVC/Ghidra tools and private reference are sufficient
to start; no new tool installation is required at this checkpoint.

## F287: shared world implemented (15 October 2026, Claude Code)

`grid_tools::source_vehicle::multi::MultiRuntime` owns N source cars (each a full
`Runtime` with its own state, wheel query cache and lap timing) and one shared physical
world. Per tick every car manager runs with its own controls and track queries
(`vehicle_manager::calculate_with_world_contacts`); its wheel contacts are re-identified
to its world body (`+4`) and car shape token (`+0x28`); one
`physical_world::advance_with_geometry_provider` then solves all bodies with track,
boundary and route-prop body contacts per car plus car-to-car contacts
(`BodyGeometry::car_contacts`: one hull against the other car's world hull triangles,
two-body records with `+8` = other body and `+0x2C` = other car token). Shapes are
`[car0, statics…, car1, …]`; the bodies form one active chain; manager properties and
workspace are per body; the pool is 64 slots.

Verified: with car 1 idle on slot_01 while car 0 drives 200–300 ticks, car 0's pose,
velocities and complete vehicle state are bit-identical to the single-car runtime.
Car 1 steering 0.6 into idle car 0 produces car-to-car contacts from tick 83 (413 in 200
ticks) and moves car 0 by 1.97 m. Asset test
`shared_world_is_independent_until_contact_and_pushes_on_contact`; tool `grid-multi-drive`.

Scope limits (not original-verified): body events dispatch per car with no peer
registration (car-to-car impacts use `other_registration` 0); deformation is not
enabled in the shared world (single-owner registration); slipstream stays 1.

## F288: opponents in the playable app (15 October 2026, Claude Code)

`grid-app --drive --body-contacts CD01 [--body-events BP01] --opponents N` (N 1..7, no
`--deformation`) builds a `MultiRuntime` over `slot_00..slot_N`; the player is car 0 and
the other cars are drawn as copies of the Lacetti parts (body, glass, wheels with spin
and steer). Each `FixedUpdate` tick swaps the player runtime into the shared world,
ticks all cars once (opponents on the handbrake during the time-trial countdown hold),
and swaps back, so the player's HUD, timing, tyre/engine sound and reset logic are
unchanged. R resets every car and driver. C holds a look-back camera
(`GRID_LOOK_BACK=1` forces it for scripted renders). `Play GRID Race.cmd` starts
practice with 3 opponents and the private CD01/BP01 files from `local\`
(`GRID_PLAY_OPPONENTS` overrides the count).

The opponent driver is `grid_tools::pursuit_driver::PursuitDriver`: an AUTHORED
placeholder (pure pursuit of the purchased progress centreline, curvature speed
limit, speed cap 18 + 0.25·i m/s), not GRID AI. It is replaced once the original
AI driver is recovered (docs/AI_DRIVER_RESEARCH.md).

Checks: `grid-opponent-race ROOT CD01 BP01 CARS TICKS` runs a headless race: 2 cars
finish a valid lap; 8 cars run 3000 ticks with no failure in 16.6 s (5.5 ms/tick)
and bunch up (39,136 car-contact records) because the placeholder speed caps are
close. Offscreen look-back render at tick 20 shows three opponents on the grid
behind the player. Full workspace tests including asset tests pass; Clippy -D warnings.

Limits: body events per car without peer registrations; no deformation with
opponents; no race start order, positions or finish results for opponents.

## F289: per-car deformation in the shared world (15 October 2026, Claude Code)

`MultiRuntime::lacetti_deforming(..., Some((limit, bulk)))` enables source deformation
and callbacks on every car. Each tick a deforming car's manager publishes through its
own staged `BodyGeometry` (`calculate_with_world_publication`) and its registration
owner is the manager's vehicle. `physical_world::advance_with_deformation` now takes one
optional registration per body: a registered car alone in its contact group (static
peers: track, boundary, props) runs both deformation phases exactly as the single-car
path. The registrations share one CRT stream, consumed in group order (the original
CRT `rand` is process-global; the order is the selected group order). A group of
several touching cars solves without deformation phases. That is a selected gap: the
original both-side callback behaviour for car-to-car impacts is not yet recovered.

Asset test `shared_world_deformation_matches_single_car_and_survives_contact`:
- One deforming car in the shared world equals the single deforming runtime over 600
  ticks (vehicle words incl. the deformation grid, physical body, seed, bulk calls and
  records), including a barrier impact with dents.
- Two deforming cars, one ramming the other: 843 car-contact records, no failure, bulk
  calls [81, 75], shared seed.

`grid-app --opponents N --deformation DF01` is now accepted, and `Play GRID Race.cmd`
adds the private DF01 file when it is present. Scripted 600-tick wall run with 3
opponents: 313 bulk calls, no failure. Opponent meshes do not show dents yet (their
physics geometry deforms).

## F292: car-to-car deformation (15 October 2026, Claude Code)

Implemented from [CAR_TO_CAR_DEFORMATION_RESEARCH.md](CAR_TO_CAR_DEFORMATION_RESEARCH.md).
A contact group with two or more bodies and at least one deformation registration
now runs both deformation phases per owner (`grid_world::deforming_group_frame`,
`deformation_world::calculate_multi_group`):

- owner order B77770 (static contacts by first body, then two-body contacts by first
  and second body; registered owners only, first occurrence);
- preparation per owner and side (`deformation_contact_limit::calculate_side`):
  relative point speed of both bodies, low-mass check on both, owner-oriented normal
  and `own.360 − peer.360`, lookup speed `|n·(v_own − v_peer)|`, coefficient ×A488
  (deformable peer, sets contact +0E) or ×A48C (other dynamic body), side-B slot +44,
  shared `+3C = min`; only static contacts reset +44;
- bulk per owner (`deformation_callback_state::calculate_shared`): side-B contacts
  reversed with the peer's +360 as counterpart velocity, one pending response list
  (first side deferred, second side sums and clamps), owner order reversed on
  alternate multi-owner invocations (F5E1A4 toggle, carried in every registration
  like the shared seed), one CRT stream consumed in bulk owner order (serial-branch
  choice; the live task branch's worker interleaving is unknown);
- each owner writes only its own body; returned contacts carry +38/+3C/+40/+44/+0E.

Groups with a single body keep the previous path, so the single-car shared-world
test is still bit-identical (600 ticks incl. barrier dents). Ramming test: the idle,
rammed car (touched only by the other car) is dented from tick 83, the first car
contact tick; bulk calls [346, 364] versus [81, 75] before; no failure. Unit test
covers side slots, the 0.75 peer factor, +0E and the low-mass speed cap. App smoke run
with deformation and 3 AI opponents: 900 ticks, no failure.

Not yet: body-impact events to both cars (`other_registration` still 0), a native
two-owner whole-frame comparison, mirrored head-on symmetry test.
