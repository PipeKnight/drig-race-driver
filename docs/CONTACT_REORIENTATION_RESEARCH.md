# Contact reorientation integration research

Follow-up: [F232](ORIENTED_TRIANGLE_MANIFOLD.md) verifies the whole reorientation
helper composed with retained manifold assembly. Caller normal/depth selection
and backface correction remain separate; the research below records the
original contract that informed that implementation.

12 October 2026. This records observed original call contracts and the next
implementation boundary. No new Rust reorientation equivalence or fresh runtime
contact detection is claimed here.

## Observations

Original AEA1F0 invokes AEE640 after a successful triangle/mesh outer query and
before AE2FA0 publication. The helper receives a triangle vertex, the moving
shape transform's translation row, a normal and two mutable witnesses. Its only
explicit output writes are multiplying the normal by minus one and swapping
the two witnesses when its orientation decision is negative.

The second anchor is **shape transform origin**, not unconditionally physical
body center: B39500 supplies the moving shape's +170 current-transform pointer
as AEA1F0 argument 7; the caller passes its +30 translation row to AEE640.
An offset collision mesh can therefore make these anchors differ.

The helper compares the displacement from the triangle anchor to one witness
and from the other witness to the moving transform origin. For ordinary
non-small displacements it uses approximate SSE lengths and refined reciprocal
normalization before choosing the larger absolute normal projection. The
small-displacement branch uses squared displacement comparisons and a
broadcast of the first comparison lane before its mask test; it is deterministic. The apparent no-argument call in the
first pseudocode is a missed fastcall parameter, **not randomness**.

Fresh Ghidra 12.1.4 / JDK 21.0.9 survey:
`local/behavior/20261012T1814175441096Z/4682a0.c.txt` and `4b1a70.c.txt`.
The former is the broadcast comparison-mask test. The latter indexes a vector
through an owned expression/view object in the caller's later correction
branch; it is not a random-number generator either. These interpretations are
supported by the aligned callers, not inferred from empty pseudocode argument
lists alone.

Private aligned original entry walks:

- `local/behavior/manifold-reorientation-aligned-20261012-01.json`: initial
  600-byte-range request (hexadecimal length), stopped before the helper return.
- `local/behavior/manifold-reorientation-aligned-20261012-02.json`: extended
  700-byte-range request covers the actual return at AEEC7A. Instructions after
  that return belong to the next entry and are excluded from interpretation.
- `local/behavior/20261012T1126269008807Z/aee640.c.txt`
- `local/behavior/20261012T0923051014062Z/aea1f0.c.txt`
- `local/behavior/20261012T0424183117875Z/b39500.c.txt`

Hash-guarded scalar extraction is retained in
`local/behavior/manifold-reorientation-constants-20261012-01.json`. The original
small-length threshold at CB46B4 has bits 3727C5AC (about 1e-5), the caller's
witness-distance threshold at CB2294 has bits 3A83126F (about 1e-3), and the
later caller correction threshold at D11548 has bits 3F34FDF4 (about 0.707).
The analyzed image remains SHA-256
`aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

## Required next work and uncertainty

Port the whole reorientation helper with exact operation ordering and paired
source/control evidence for both signs, equality, zero displacement and small
threshold neighborhoods. Compose it with the existing outer query and
[retained manifold append](RETAINED_TRIANGLE_MANIFOLD.md). Preserve the actual
moving transform origin and witness order throughout.

This alone does not reproduce the whole packed generator. AEA1F0 first derives
normal/depth from witness separation, selects normal behavior through the
nonaliased +290 sheet mask, and has a later backface/correction path involving
source mesh support and geometric tests. Those caller branches still need
joint validation before claiming a complete fresh Rust generator. The source
policy and legacy descriptor distinction remain documented in
[F229](PACKED_GENERATOR_COMPLETE.md).

The research reused installed tools. Ghidra headless analysis successfully
saved its private project; no approval was rejected. No desktop input, original
Steam file/settings modification or new installation was needed.
