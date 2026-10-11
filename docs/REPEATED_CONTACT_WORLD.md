# Repeated contact production into the carried physical world

Checkpoint F199, 11 October 2026. The unchanged original contact producer now
appends into the same four-slot pool before successive complete serial world
steps; the original wrapper consumes that pool after every solve. Rust matches
this repeated sequence. This extends [carried physical state](CARRIED_PHYSICAL_WORLD.md)
from initial-only contacts to reused storage and varying active counts.

## Selected sequence

Fresh whole car-manager outputs supply each branch's complete physical owner.
Cold physical configuration is supplied before that manager call, as in the
previous handoff. Afterward only the center-backlink identity is normalized.
Four independently authored local contact points, world normals, penetrations,
source shape identities and a per-step count schedule drive this experiment.
These inputs are controlled tests, not observed geometric contacts.

Each step transforms local points using the current published physical matrix,
calls whole original `B15E70 → B0EE80`, then whole `AD8D70 → A727A0` on retained
world, body, producer, pool, solver and worker buffers. No intermediate
normalization, cold reset or manual pool-count rearming occurs. Counts vary from
zero through four; inactive slots retain their previous bytes. Original
construction overwrites its documented fields, leaving old group-link and other
template fields until their actual later writer. Friction-row pointers remain
owned native pointers across frames and normalize only at final output.

The owned shape metadata follows the verified car0/static11 contract, shared
material, MAX friction limits and absent callbacks. Physical geometric-shape
registration, general pose cursor, sleeping, joints, deferred publication and
async work remain excluded. Rust shares the selected constructor implementation
with wheel-overtravel production and accepts a new active subset against retained
pool capacity. Failure preserves input storage, including after an earlier
request has constructed a record.

## Validation and corrections

In each debug/release profile, original/repeated-original/Rust match 198 runs
from 66 fresh manager owners, 2,772 world calls, 2,574 carried updates and
970,200 output bytes. The controlled runs append 5,199 contacts, including 4,827
after the first step, with 694 empty intervals. Lengths reach 64 updates.
Fifteen independent checks include exact ownership/flags, real pool consumption,
143 available initial/consecutive history pairs, 1,100 point-component checks
and 297 complete inactive-slot retention comparisons. Seventeen malformed
inputs produce 34 paired safe rejections per profile, including a nonfinite
inactive request and unsupported count, shape, basis and publication state.

The first isolated native draft caught an access violation during the initial
four-request case. Serialized request rows have a 40-byte stride; the second
normal was therefore unaligned. The original constructor contains an aligned
SIMD load at `B0F039`, consistent with the observed fault address `FFFFFFFF`.
Copying each normal into an explicitly 16-byte-aligned temporary resolves the
failure. All original code remains unchanged; lazy pages/protections restore
and failed output is removed. The failed draft remains private. Fault reporting
now translates the relocated instruction address back to the original image.

The first expanded numerical comparison was exact, but the independent point
check failed: Python used unrounded decimal local coordinates and timestep,
whereas the input wire stores f32. The check now rounds input values before
performing the independently ordered float operations. Final comparisons and
independent checks pass. An initial patch spelling mismatch made no changes;
corrected from the actual source. A guessed manager-header read failed harmlessly;
the real header was found through its include chain.

Fresh carried/whole/shaped frame/tick and manager-to-whole-world regressions pass
both profiles. The complete wheel car-manager regression matches 428 calls and
24 guard outcomes per profile after the shared constructor refactor. The
expanded existing fourth-request rollback unit passes; all 160 library tests,
formatting and warnings-denied all-target Clippy pass. Twelve-report current
executable/provenance audit verifies 178 independent checks, 276 guard outcomes
and four Python ASTs; complete manager regressions are recorded separately.

## Reproduction and evidence

Build `tools/build_reproduced_physical_world_probe.cmd` and both Rust diagnostic
binaries (`grid-physical-tick`, `grid-vehicle-force`) in debug/release. Run
`tools/validate_reproduced_physical_world.py` with `--profile`. Private final
reports: `local/behavior/reproduced-physical-world-{debug,release}-20261011-final-01`,
`{carried-physical-world,whole-physical-world,shaped-physical-frame,shaped-physical-tick,wheel-whole-world,wheel-car-manager}-{debug,release}-20261011-f199-regression-01`
and `reproduced-physical-world-final-audit-20261011.json`.

Native SHA256 `7f6b549ddb74c8a3692c6dc8b32691514762b9d226d9defbb7edcec1f1e2ec18`;
Rust physical debug `ddba120faefa13016ec91a688c395c3cb7b57556d302198ea2ee3dda51ac87fe`,
release `8ac844aedcdd3759d173df7eff74f917974f23c98d79ee9d90281f6b51134cc4`.
Installed Python3.12.14, Rust/Cargo1.99.0 and MSVC14.44.35207/SDK10.0.26100.0
strict x86/SSE2 reused; no installation. Static constructor evidence remains in
the private F195 surveys and `wheel-producer-aligned-20261011.json`.
Steam's 130 reference resources and executable remain unchanged at
16:18:59.093748 UTC, `local/scene/preservation-20261011-reproduced-physical-world.json`.
No desktop/settings changes; expired automation remains paused.

Next repeat complete vehicle-manager jobs against the carried physical world,
with authoritative geometry queries and source initialization. This experiment
still supplies material, physical timestep and contact decisions; it does not
establish the complete live frame or a drivable Rust lap. No owner action needed.
