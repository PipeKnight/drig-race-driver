# Shared-world car integration: next implementation boundary

F277 continuation,15October2026. This is a code/evidence dependency map, not an
implemented opponent or a claim that two cars already drive in one world.
Current playable0cb4405 remains one car with static peer contacts.

## Reusable verified behavior

The solver itself is not limited to one car: contact_groups accepts1..16 owned
bodies and64 contacts, with explicit active permutations and links. The original
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
