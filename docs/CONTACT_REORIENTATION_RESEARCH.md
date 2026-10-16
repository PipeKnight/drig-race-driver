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

## Degenerate displacement: port deviation (F342)

16 October 2026, Claude Code. Found by `grid-soak` (Nuerburgring quick seeds 6
and 3; F326 #3). Static: capstone on the hash-pinned image
(`local/soak-fixes/disasm/aee640.asm`).

- [D] Small branch of AEE640: when either displacement length is at most
  3727C5AC (1e-5), 0xAEEB40 `CMPLTPS` compares |a|^2 < |b|^2 and 0x4682A0 tests
  lane 0 of that mask; true selects a (stored at esp+0x40, `param_4 - param_1`),
  false b (esp+0x50, `param_2 - param_5`). The sign of `dot(normal, chosen)`
  decides the flip. This is what the port did (byte-exact in F232).
- [I] The chosen vector is the **smaller** one, so it is always the
  sub-threshold one: exactly zero when the triangle witness is the anchor vertex
  (triangle[0]), otherwise a few ulps of rounding (at |x| > ~80 m an ulp is
  already > 7.6e-6). Observed in the port:
  - noise: a 2e-6 (one-ulp) witness offset gave dot = -7e-8 and flipped an
    upward normal; the manifold then published four contacts at the car roof
    with 1.45 m depth and the car was pushed 1.14 m into the road in one tick
    (seed 6, car 9 at (-213.6, 607.2, -31.0));
  - exact zero: a margin contact (hull 2 mm above a kerb vertex, so the witness
    difference points down) kept its downward normal because a = 0; 1.3 m
    contacts, the same fall (seed 3, car 1 at (-224.7, 606.5, -80.7));
  - rounding just above the threshold: a triangle witness 1e-4 from the anchor
    at y = 612 (one ulp there is 6.1e-5) takes the normalised branch, and that
    rounding-sized vector's projection (-0.998) outvotes the hull side (+0.6);
    a resting Nuerburgring tyre was pushed 0.3 m into the ground (long seed
    1020). The same happens to brake-marker boards at Donington |x| 400-550.
- [D] AEA1F0's later backface correction (0.707 at D11548, B1AB30 extent test,
  B225E0 vertical ray) would catch both, but its extents at packed shape
  +0x294..+0x2A0 are written only by the constructor (FLT_MAX at
  0xB1F938..0xB1F976; a search for writes of all four offsets found no other
  writer for this class), so it never runs.
- **Port deviation (owner rule F320, recorded):** a displacement is
  degenerate when its length is at most 1e-5 *or* 32 ulps of its largest
  endpoint coordinate (32 x FLT_EPSILON x scale, AUTHORED; about 2.3 mm at
  600 m). If one is degenerate the port orients by the other, informative one;
  if both are, it keeps the normal; otherwise the original normalised branch
  runs unchanged. Grid.exe's result differs only in these degenerate cases:
  for noise it was arbitrary; for an exact zero the original kept the normal
  and the port now lets the mesh side decide. The
  F232 boundary sequences that place witnesses at or near the threshold are
  therefore no longer byte-identical by design; the unit test
  `orientation_swaps_witnesses_and_preserves_signed_normal_lanes` states the
  new rule. Whether grid.exe avoids these contacts somewhere downstream was not
  found: contact bias (B0CE10/B0CEC0) has a FLT_MAX upper bound.
- Unchanged: Donington solo AI laps 88.42029812185638 / 86.28228501935392,
  output byte-identical to main. Changed: the 12-car Donington race
  (`12 12000 ai` with DF01) no longer matches F324's reference (main: 11 800
  ticks, 827 car contacts; now 12 000 ticks, a different order from tick
  3000). Bisected: with only this file taken from main the race is identical to
  main again, so the per-pair budgets, lost-piece hold and 0/0 blend do not
  change it; the orientation deviation does, through degenerate contacts in the
  field's car-car and car-kerb contacts.

