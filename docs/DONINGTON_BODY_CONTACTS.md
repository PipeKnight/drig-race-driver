# Experimental packed-track body contacts

F237/F238, 12 October 2026. `BodyGeometry` now builds the moving Lacetti mesh
from the purchased NED and verified owner offset. It composes its selected local
Y center with the advanced physical pose, queries actual Donington packed chunks
and submits fresh world-space contacts through the vehicle wrapper before solving.

`Track::body_contacts` preserves archive order and source shape identities.
Inclusive root AABB culling precedes cold per-pair queries. Each pair has at
most 16 contacts; the caller's total may be at most 64. The driving mode reserves
20 stable slots for four wheel contacts plus sixteen body contacts. Per-triangle
local material indices are retained. Sheet selection uses exact four-byte keys
and first-record fallback; no padding normalization occurs. This extends the
[verified selected packed generator](FRESH_PACKED_CONTACTS.md) and
[vehicle integration](VEHICLE_GEOMETRY_INTEGRATION.md).

The scheduler remains authored archive order. Solver material IDs remain zero,
selecting the prototype's explicit shared physical material. Cold pair state,
four MAXFLOAT correction extents, zero margin, selected local placement and
existing shape flags remain explicit. This does not establish the complete
original broadphase, shape factory, material registration, contact callbacks,
damage events or populated backface correction.

## Verification

Current debug and release purchased-source tests pass:

- The lowered source mesh (0.5 m authored displacement) returns 16 contacts
  belonging to owned static shape 103. Capacity one preserves the first record;
  zero returns none, and capacity 65 or missing body ownership rejects.
- A 240-tick full-throttle starting-straight run completes and matches the old
  wheel-only physical state exactly, ending at approximately 31.98 m/s. No body
  collision is established by this clear-road comparison.
- Starting 0.12 m lower, 20 repeated vehicle ticks generate 15 body records and
  differ from the matching no-body-contact control at every tick. This is a
  controlled source-road intersection, not a natural barrier collision.
- All four opted-in source driving tests pass in release, including earlier
  handbrake/reverse, reset/rollback and authored wall integration. All 184 regular
  library tests pass. Formatting and warnings-denied all-target Clippy pass.
- Three opted-in Bevy application tests pass: wheel presentation and keyboard
  recovery in ordinary and body-enabled modes. They cover throttle, steering,
  pause, reset, deliberate failure/recovery, Space handbrake and reverse.
- Vulkan offscreen rendering with body mode enabled completes 240 full-throttle
  ticks and saves the inspected Lacetti/Donington image. Telemetry records
  `body_geometry=true` and `body_contacts=0`; no body hit is claimed by that run.

The preceding F236 fresh original/repeat comparisons verify the unchanged wheel
wrapper. F234 verifies the packed kernel. No new whole-original comparison of
this authored multi-chunk scheduler or complete body-enabled vehicle frame is
claimed.

Private evidence: `local/behavior/donington-body-driving-20261012-final-02.txt`,
`donington-body-driving-debug-20261012-final-02.txt`,
`donington-body-app-tests-20261012-final-02.txt`, library/Clippy/build logs;
`local/playtest/donington-body-20261012-final-02.{log,png,json}`.
Current release executable SHA-256:
`bb131302cd8c2f3fe6b5c79959144bc4aa9f79d2452d6aeba2bd092bb0289d7e`.
Existing Bevy 0.19.1 / Rust-Cargo 1.99.0 / Python 3.12.14 / RTX 4070 Vulkan
driver 616.92 were reused. No new tool was installed.

## Try this mode on the owner's PC

Double-click **Play GRID Body Contacts.cmd** in the project root. It uses the
already built executable and the private prepared table at
`local/body-contact-directions.bin`. **BODY** in the window title confirms this
mode; its number is the fresh body-contact count from the last successful tick.
Body-hit ticks are also logged. The usual arrows/WASD, Space, P, R and Escape
controls apply. R restores the initial car/world and the reserved pool. A
simulation failure pauses and retains the last successful state; R recovers.

The ordinary **Play GRID Prototype.cmd** still launches the wheel-only mode.
Its optional second argument selects a private CD01 table. Both launchers write
private playtest logs. The new launcher has been reviewed but not double-click
tested; the equivalent executable path and application recovery behavior were
tested without desktop input.

Other checkouts must prepare their own private sampling data. The file has
712 bytes and header CD01/count44. Preparation currently depends on the locally
analyzed initializer plus unchanged Steam scalar data (see F225/F236), so this
is not an installation-only distribution path. No proprietary table or game
asset is committed. Current private table SHA-256:
`ea8d8bff5270d5b75410f822c3e784a66399b7aae4e39365f8f54747639179c7`.

## Failures and next collision layer

The original heavy-impact handling damage guard remains. F236's deep authored
wall reached that unsupported branch. It was not disabled for this mode.

The first 240-tick steering=1 render completes without body contacts. A longer
600-tick steering=0.25 run leaves the supported surface and falls to Y=-147.82,
with zero body contacts throughout. Exit success means the bounded script and
capture completed, not that off-track compatibility succeeded. Private
`donington-body-turn-20261012-draft-01.{log,png,json}` retains this failed driving
experiment. A connected-gamepad polling error (0x80070057) appears in that log;
scripted keyboard controls continued. No controller-fix claim is made.

`boundarylines.cqtc` is a separate, currently unloaded layer. Read-only inventory
finds 51,631 bytes, 2,782 triangles, 2,783 vertices and one material, SHTC. Its
positive-count CQTC layout differs from the negative-count VCQTC chunks. The
installed surface policy marks SHTC as sheet=true. These observations make it
a candidate for missing boundary collision, not proof of a particular wall or
original runtime dispatch. Source SHA-256:
`14f99310ad89ac030b9b783f914deee8949389b0f12b380008924905b6fb87c1`.
Inventory: `local/behavior/body-world-resource-inventory-20261012-draft-01.json`.

The existing pinned Petar Tasev CQTC template was read as research background;
it proposes 24-bit nodes, seven-byte triangles and a different reference stream.
Those layouts and original dispatch need independent validation before loading
this layer. Reducedmechanics.jpk has 14 entries, mostly per-class reduced
mechanics and track.rm; names alone do not establish collision geometry.

An initial Clippy check required moving the new test module below implementation;
corrected without suppressing the lint. One atomic patch missed a formatted
context and applied no edits; the corrected edit was checked and rebuilt.
Harmless guessed path/wildcard reads failed without modifying files. The earlier
low-throttle test barely moved; the final straight test uses full throttle and
requires displacement. No numerical threshold was widened. Steam preservation
at 19:52:39.581596 UTC confirms all 130 tracked resources and executable match baseline: `local/scene/preservation-20261012-donington-body.json`.

Next identify and validate CQTC construction/traversal and mesh dispatch, then
join the source boundary layer. Heavy-impact damage remains a separate required
step toward general barrier driving. No owner input is currently blocking work.
