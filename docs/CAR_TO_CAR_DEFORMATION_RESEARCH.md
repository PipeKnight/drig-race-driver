# Car-to-car deformation in the original physical frame

15 October 2026, Claude Code, static research only. No game process was touched,
no Steam file changed, no crate or project-log edit. This closes the research gap
recorded in [multi-car integration F289](MULTI_CAR_INTEGRATION.md): groups of several
touching cars currently solve without deformation phases.

Labels: **[V]** verified in aligned disassembly (or by an earlier whole-original
proof, cited); **[D]** data words read from the hash-pinned analysis image;
**[I]** inference. Analysis image `local/compatibility/grid-unpacked.exe`,
SHA-256 `aac46a64…f34514`, base 0x400000.

## Summary

The original has no separate car-to-car deformation path. The same two registered
callbacks run once per car **side**, and each car uses its own owner, grid
descriptor and physical body:

- **Preparation (`+418`, 9A0C90)** runs once per side. It writes a per-side
  coefficient: `+40` for the first body and `+44` for the second. Both sides lower the
  same contact impulse cap at `+3C` by taking the minimum.
- **Bulk (`+41C`, A064A0)** also runs once per side. Each side reverses the
  normal/direction when it is the contact's second body. It splits the available
  crush between the two cars by their coefficients and changes **only its own**
  body velocities, grid and pending shift.
- **Consumption (B77490)** waits until both sides have handled a shared contact. It
  stores the first side's response, adds the second side's, then adds the result to
  contact `+38` and clamps it to zero or above.

The only car-to-car-specific behaviour is in 9A0C90. Against another
deformation-registered body, the coefficient is multiplied by descriptor `A488`
(0.75) and the contact's `+0E` "disabled" byte is set to 1. That byte suppresses the
optional position-correction rows for the contact. Against a dynamic body without
that registration, the multiplier is `A48C` (0.75) and `+0E` is left unchanged.

## Contact record fields used

| Offset | Role in these routines |
| --- | --- |
| +04 | first body A (always present for pooled contacts) |
| +08 | second body B; only treated as a body when byte +0C == 0 |
| +0C (byte) | second-object kind; 0 = body |
| +0D (byte) | skip: contact not pooled by B7B850 [V] |
| +0E (byte) | "disabled": zeroes optional-correction rows ([contact bias](CONTACT_BIAS.md)); **set to 1 by 9A0C90 for deformable peers** [V] |
| +0F (byte) | impact-pool flag: nonzero → callback pools 98/A0, zero → 9C/A4 (no callbacks) [V, B7B850] |
| +10 | next contact in group list |
| +28 / +2C | shape tokens; B775D0 excludes the contact (for **both** owners) when either shape's byte +230 is set [V, [collection](CONTACT_COLLECTION.md)] |
| +38 | accumulated response scalar (penetration base; consumed result) |
| +3C | impulse cap before solve; solved impulse after solve; shared by both sides |
| +40 / +44 | side-A / side-B coefficient written by 9A0C90 |
| +50 / +60 | world point / normal (one orientation for both sides) |
| +71 (bit 8 of word +70) | prototype "flipped" byte. **Not read** by 9A0C90, B77850, B775D0, B77770, B77490, 9A14D0 or 9EA6E0: the decompilations of these routines read only +73 in that word [V]. Its meaning belongs to contact generation, not deformation |
| +73 (byte) | generation eligibility (bulk records only for +73 != 0 and +3C > 0) [V] |

**Side predicate** (identical in 9A0C90, 9A14D0, 9EA6E0, response helper 7D88F0 and
B775D0/B77490): the owner is side B if and only if `+0C == 0 && +08 == owner`;
otherwise it is side A (`+04 == owner`) [V]. The side is selected by the body
pointers, not by the shape tokens or `+71`.

## Q1 — dispatch per side and order

Pools, from B7B850 aligned instructions [V]: the group contact list is walked in
order, and contacts with +0D set are skipped. A contact with `+0C == 0 && +08 != 0`
is two-body: it goes to pool A0/B4 if +0F is set, otherwise to A4/B8. All other
contacts go to pool 98/AC if +0F is set, otherwise to 9C/B0. Prototype car-to-car
records (`+0C = 0x01000000`, +08 = other car) therefore land in the two-body
callback pool A0.

Owner list, B77770 [V, [F149](OWNER_COLLECTION.md)]: walk pool 98 by +04, then pool
A0 by +04 then +08. Keep only owners whose `+418` is nonzero, first occurrence wins.
A car that also has static contacts can therefore precede the contact's own A body.

**Preparation, B78530 → B77850 → 9A0C90** [V]:
- B78530 calls AD7DC0 with ECX = manager+0x64 (aligned at B78577). AD7DC0 only copies
  the owner list into a 10-slot log ring, so there is **no reordering or alternation**
  in this phase.
- Serial branch: for each owner, B77850 runs with budget 10000, skip 0 and both
  cursors at 0. It visits pool 98 (`+04 == owner`), then pool A0 (`+04 == owner`, or
  `+0C == 0 && +08 == owner`), and calls owner `+418` with (owner, contact, scalar,
  owner+420).
- A shared car contact is therefore prepared **twice, once by each car**, in
  owner-list order. The first pool also writes `+44 = FLT_MAX` after each call. The
  second pool does not, because side B writes its own `+44`.
- Task branch (manager+134 nonzero): same per-owner jobs, run as tasks. [I] The
  result does not depend on order: each side writes a different slot (+40/+44), and
  `+3C = min(+3C, capA, capB)` gives the same result in either order.

**Bulk, B78710 → B775D0 → B784B0 → A064A0 → B77490** [V,
[F150](SERIAL_COLLISION_DISPATCH.md) plus fresh aligned evidence]:
- The owner list comes from B77770 again. When more than one owner is eligible,
  global byte F5E1A4 is logically negated, and the list is reversed when the result is
  nonzero. F5E1A4 has no other reader or writer (Ghidra refs), so the toggle is
  **process-global and alternates on every group invocation that has two or more
  owners**.
- Serial branch: for each owner, B775D0 selects pool 98 (`+04 == owner`) then pool A0
  (`+04 == owner` or `+0C == 0 && +08 == owner`). Shape-disabled contacts are
  excluded and the cap is 80. Non-empty selections go to B784B0, which zeroes the
  responses and calls `+41C` with descriptor `+420`. B77490 runs only when AL != 0.
- Task branch (observed live for the player at B787D6): one task per owner, scheduled
  even when its selection is empty. B77B90 zeroes the responses and calls `+41C`. The
  branch waits on A7D530, then calls B77490 for **every** owner in list order,
  regardless of AL.
- One pending-pair list (at most 240 pairs) is shared by all owners within one B78710
  invocation and starts empty. Pairs left over are dropped at the end of the
  invocation [V, F149].

## Q2 — two-body arithmetic per side

### Preparation 9A0C90 (owner = O, other = P)

All steps below are [V] from aligned 9A0C90 (B107A0 ECX operands checked).

1. Point velocities at the contact point use B107A0 = `linear220 + cross(angular210,
   point - *(body+334))`: vA for body A if present, vB for body B if present. The
   relative normal speed is `|n · (vB - vA)|`, where n is +60, A is always the first
   body and the lanes are grouped as in the static port. If **either** body has
   mass +3F0 < 100.0 [D CAFBDC], the speed is capped at 5.0 [D CB093C]. This speed is
   the same for both sides and raises the ring slot of O's own window (A4C4[A4EC]).
2. Disable gates (A4B0, A4AF, global E85FAD) and the corner gate (max window <
   A4B4[corner] × 25.0 [D E4FC2C], with the point transformed by **O's** pose
   1D0..20C) both write FLT_MAX into O's slot and return FLT_MAX.
3. Penetration term: `+38 + max(0, n_O · (O+360 − P+360)) × 1.0` [D E4FC3C], where
   `n_O = n` for side A and `n × −1.0` [D CB0948] for side B. P+360 is omitted when P
   is absent. The prototype clears +360 at every advance, so this term is currently
   just +38.
4. Stiffness comes from 989A00 using O's grid at the O-local point. If it is
   unlimited, it is written to O's slot and returned.
5. The damage-lookup speed is `|n · (vO(P) − vP(P))|`, computed as point velocity of
   O minus point velocity of the other body. It goes to 9A09A0, and A490/A494/A498 are
   updated only on a strict increase.
6. The coefficient is `stiffness / A490` (FLT_MAX when A490 == 0.0 [D D85A50]).
7. **Peer factor** (only when P exists and the coefficient is finite):
   - A14F10 is called on P. It returns `P+418` and stores `P+420` to a local.
   - If `P+418 == O.descriptor+A484`, the coefficient is multiplied by A488 and
     B0CD10(contact, 1) sets **contact +0E = 1**. A484 is written as 9A0C90 by
     9A2E30/A03220, and A06A10 is the only writer of 9A0C90 into body +418 (Ghidra
     refs). In other words, P is another deformable car.
   - Otherwise the coefficient is multiplied by A48C.
   - Static peers (P absent) get no factor.
8. The coefficient is stored to +44 if O is side B, else +40. The function returns
   `coefficient × penetration` (x87). B77850 then sets `+3C = min(+3C, (f32)cap)` when
   the cap is not FLT_MAX.

A488/A48C are loaded by the 9A2E30 constructor from E4FC40/E4FC44. Both are 0.75
[D 3F400000], and nothing writes either global (Ghidra refs: reads only, both in
9A2E30). The prototype source constructor already passes `[0.75; 2]`.

### Solve

B7ACC0 uses +3C as the upper impulse bound ([contact rows](CONTACT_ROWS.md)) and
writes back the solved impulse. This is one shared impulse for both cars, applied by
the ordinary two-body rows [V, earlier proofs].

### Bulk A064A0 (per owner O)

These are [V] by earlier whole-original proofs that already included reverse and
counterpart cases, plus the 9EA6E0 decompilation reread:
- **Reduction 9A14D0**: the direction is multiplied by −1 for side-B contacts, then
  weighted by +3C. The point is the mean of +50 ([F144](CONTACT_REDUCTION.md)).
- **Generation 9EA6E0**: for side B the point/direction and normal are multiplied by
  −1. The own coefficient is +44 and the other is +40. For side A the own coefficient
  is +40 and the other is +44, or FLT_MAX when static. The relative vector is
  `O+360 − P+360` ([F138](IMPACT_GENERATION.md) controls cover reverse and zero
  denominators).
  - `available = +38 + max(0, dir · rel)`.
  - `limit = +3C / own`, `total = +3C / other + limit`.
  - If `available × 1.0 [D E500C8] < total`, then `limit = available / total × limit`.
  - This **splits the available crush between the two cars in proportion to
    1/coefficient**. A static peer (other = FLT_MAX) leaves the whole share to the car.
- **Body adjustment 9D4210**: only O's angular +210 and linear +220 change. It uses
  one CRT `rand` on the active path ([F140](IMPACT_BODY.md)).
- **Response slice**: the normal is reversed for side B. Only O's +380/+390 (pending
  shift) and the per-contact response floats change ([F139](IMPACT_RESPONSE.md)).
- **The callbacks never write the counterpart body**. It is read only: +360 in the
  bulk phase, and 210/220/334 through B107A0 in preparation.

### Consumption B77490 [V, [F148](RESPONSE_CONSUMPTION.md)]

- When the contact's other side is a registered owner (`+41C != 0`), the first side's
  response is stored as a pending pair `(contact, r1)`.
- The second side computes `r = r1 + r2`, then `+38 = r + +38`, clamped to 0 or above
  only when strictly negative. The pair is removed by swap-with-last.
- Static contacts are updated directly (`+38 += r`) with no clamp.
- Edge case [V]: in the serial branch, a side whose callback declines (A4AF or
  E85FAD) is not consumed, so the other car's pending response is lost. In the task
  branch it is consumed with a zero response.

Result: each car is pushed and dented only by its own callback; the shared impulse
cap is the tighter of the two; and the summed response feeds the optional correction
through +38. Because each callback reads only the other body's +360, and float
addition is commutative, **owner order affects only the RNG sequence** [I from the
above].

## Q3 — CRT rand stream [V]

- `rand` at BF4DA7 (called from 9D46F8) uses `_getptd()` (BFDDA6 → BFDD2F, TLS) at
  field **+0x14**, with update `seed·214013+2531011`.
- BFDC70 initializes +0x14 to 1 for each new thread-data block. The stream is
  therefore **per OS thread**, not per car and not process-global.
- 9A0C90 calls no rand. Bulk uses at most one rand per handled callback (9D4210).
- Serial branch: the solver thread's stream, consumed in the (alternating) bulk owner
  order.
- Task branch, observed live: the callbacks run on whichever worker executes each
  task (A7D710/A7D530). [I] Which thread that is cannot be determined statically, so
  the exact cross-car sequence is not reproducible.

A deterministic port may keep one shared stream consumed in the bulk owner order,
which matches the serial branch for a single-threaded solver. That is a documented
choice, not verified equivalence.

## Q4 — car-vs-car versus car-vs-static differences

- Only 9A0C90 branches on the peer type: static → none; deformable car → ×A488 and
  +0E = 1; other dynamic body → ×A48C [V].
- Low-mass capping looks at both bodies (threshold 100, cap 5).
- No other thresholds, corner scales, gains or table entries differ. 9A0C90 reads no
  settings word from owner +D240. The bulk settings words used (D240+1024, +1028,
  +1030, +1034, +1038, +1048) have no peer-type branch [V for the decompiled paths].
- Body-impact damage through the **shape +21C** route is separate from grid
  deformation. A726B0 builds one event (event0/4 = side registrations) and calls
  side +28's callback, then side +2C's; an equal function/user pair runs once. Each
  car's A01910 picks the side whose owner chain resolves to its car, and 9EF360 flips
  the direction sign for side 1 ([body-impact gap](BODY_IMPACT_CALLBACK_GAP.md)) [V].
  For two cars both callbacks run, side +28 first. The prototype's `other_registration`
  0 must become the other car's registration.

## Implementation plan for the Rust prototype

Keep the single-car/static path bit-identical; it is the regression oracle.

1. **`deformation_contact_limit`**: add a two-body entry. Inputs: side (A/B), own
   physical body, optional other physical body (210/220/1C0/360/3F0), and a
   `peer_deformable` flag (other has a deformation `Registration`). Then:
   - relative speed `|n·(vB−vA)|` with A always the first body, and the low-mass
     check on both bodies;
   - side-B normal ×−1 and `own.360 − other.360` in the penetration term;
   - lookup speed `|n·(v_own − v_other)|`;
   - peer factor A488/A48C from the descriptor, and output `set_disabled` when
     deformable;
   - write +44 for side B, +40 for side A.

   Unit-test the slot choice, sign, factor, `+0E` and low-mass cases. Extend
   `probe_deformation_contact_limit.c` with an owned counterpart body whose +418/+420
   are real registrations, and compare whole 9A0C90 output.
2. **`deformation_preparation`**: replace the 0/1 owner tokens with owner identities
   and the two pools (98 then A0 order). Apply B77850 per owner: second-pool match
   on `+04 == owner || (+0C == 0 && +08 == owner)`, `+3C = min(...)`, and `+44 =
   FLT_MAX` only for pool 98.
3. **`deformation_callback_state`**: drop the `c[1]==0 && c[2]==0` and empty-pending
   guards. Take per-contact `reverse`, `counterpart_velocity` (other +360), and
   consumer roles (first/second owner, registered, `second_is_body`), plus an in/out
   pending list. The kernels (`impact_generation`, `contact_reduction`,
   `impact_response`, `serial_callback`, `response_consumption`) already implement
   these branches.
4. **`deforming_physical_frame::Context`**: hold `Vec<OwnerState>` with local body
   index, owner words, a 0x424-word physical copy, settings, limit, bulk and
   registration index; the shared raw contacts with pool tags; shapes; scalar; one
   shared seed; the alternation byte; and pending pairs.
   - `prepare`:
     - build the owner list (B77770 order, registered owners only);
     - run step 2 for each owner;
     - copy +3C → `previous_impulse` and +0E → `disabled` into `step.contacts`.
   - `solved`:
     - copy +3C/+38 back from `step.contacts`;
     - rebuild the owner list; if it has two or more owners, flip the byte and
       reverse the list when the new value is nonzero;
     - for each owner: refresh its physical 210/220/380/390 from
       `step.bodies[local]`/`bodies[local]`, select contacts (98 then A0, shape230
       excluded, cap 80), call bulk, consume (serial semantics: skip when declined),
       and write back its own velocities/shift only;
     - write +38 into `step.contacts[i].accumulated`.
   - Allow `frame.bodies.len() >= 1` and two-body step contacts. Unregistered
     dynamic bodies are peers but not owners.
5. **`deformation_world::calculate_group`**:
   - Take the group's bodies, contacts and `&mut [Option<&mut Registration>]` per
     local body.
   - Accept `r[2]` = local body token for two-body records with byte C == 0, and keep
     tokens as local indices + 1.
   - Return `+38/+3C/+40/+44/+0E`, plus updated owners, seed and toggle.
   - Move the shared seed and the alternation byte into one carried
     `DeformationShared { random_seed, owner_toggle }` owned by the world or
     `MultiRuntime`, instead of copying the seed into every registration.
6. **`grouped_physical_frame::calculate_impl`**: replace the `[only]` match with
   "any registered body in the group". Pass all local registrations and the shared
   state, and keep the existing path for groups with none. Contacts keep +0F = 1, so
   car contacts stay in the A0 pool. Propagate the returned `+0E` into stored contacts.
7. **Body events**: dispatch car-to-car impacts to both cars, side +28 then side +2C,
   using the peer's registration and direction-flip side 1, as a separate change.
8. **Checks**:
   - the single-car shared-world asset test stays bit-identical;
   - a mirrored head-on test of two identical cars gives equal +40/+44, symmetric
     dents and opposite pending shifts;
   - a ramming test shows dents on both cars and no failure;
   - after the bulk phase, check total linear momentum against plain solving. The
     angular kick and the 9D4210 linear compensation change it per car, so this is a
     sanity bound, not equality.

   Native whole-B78710 serial comparison with two real owners: extend
   `probe_serial_collision_dispatch.c` with active descriptors and a shared contact.

## Open questions

- Worker-thread assignment of bulk tasks, and therefore the live RNG interleaving [I].
- Meaning of contact +71 ("flipped") upstream in generation. It is not consumed by
  deformation.
- No native two-owner whole-frame comparison yet. Points 1–2 of the plan specify one.
- Physical meaning of body +360 (cleared at advance); it is zero in the prototype.

## Evidence (private, `local/`)

| File | SHA-256 |
| --- | --- |
| car-to-car-deformation/helpers-a14f10-b0cd10-20261015.json (A14F10, B0CD10, B107A0) | 3058675e64d2c117a9c4e2328ca2fbaa18e3bf3b01acb57d09dad7282d74af15 |
| car-to-car-deformation/b78530-b78710-20261015.json (AD7DC0 ECX, F5E1A4 toggle, task loop) | f460a3bd868870e1420962ceaecc33e7adf53dbfba4922e1e091d6073d852431 |
| car-to-car-deformation/bulk-task-b77b00-20261015.json (B77B90 task callback) | 641f2142bc16015f7d7c6c276b7a7765e570d7c8cd989f4548d4bd921b0bf53b |
| car-to-car-deformation/crt-rand-20261015.json (BF4DA7) | 6264ff7b504660d8c0505551d66d8e9734563997087964aef49383faeeb34a38 |
| car-to-car-deformation/crt-initptd-20261015.json (BFDC70) | 12d71436a56edaaeb6fe33aa7bba22372e30085daa90f051b5f022c81b48e17a |
| car-to-car-deformation/constants-20261015.json | 1215a46819d6f634902d7a13b5b946b4b22c476fad7d95e4a73af77125b140de |
| behavior/20261015T1014006838149Z/address-refs.tsv (E4FC40/44, 9A0C90, F5E1A4, B0CD10, A14F10) | ef3d2f1147c71a0bbde6dd1182a05584443bcfbd96707ba1fe335313f48cf01f |
| behavior/deformation-contact-limit-aligned-20261013.txt (whole 9A0C90, reused) | 16029ac06b07ac5989a654e1de072e652db11f015c0b5edf20c37a7b63037157 |
| behavior/deformation-callback-group-order-20261013.txt (B7B850 pools, reused) | 127075726f7c5984aa91f5c6e4764e0c764e6aa78ca6f03c63a7a53aaa4c984d |

Earlier decompilations reused: `20261008T2218019439763Z/9a0c90`, `20261010T0536324061639Z/b77850`,
`20261010T0535523011863Z/b78530`, `20261013T1617514366319Z/ad7dc0`,
`20261008T2001082829510Z/9ea6e0`, `20261013T0953434555955Z/9a2e30`,
`20261011T1433384595310Z/a726b0`.

Tools: existing `tools/inspect_private_instructions.py` (Capstone 5.0.6), run through a
scratch `sys.path` wrapper because `python -I` excludes the script directory, and
`tools/run_ghidra_address_refs.ps1` (Ghidra 12.1.4, one process). A scratch f32
data-word reader (hash-guarded, output under `local/`) produced the constants file.
Python 3.12 (Codex runtime). Failed attempt: the first instruction read at
9D46E8 was misaligned (garbage) and was discarded; the CRT rand target was re-read at
the aligned call 9D46F8 → BF4DA7. No installation, no game process access.
