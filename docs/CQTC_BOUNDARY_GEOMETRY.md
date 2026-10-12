# Donington CQTC boundary geometry (F239)

The purchased Donington routes contain a separate `boundarylines.cqtc`, which
the current packed-track driving adapter does not load. This is a candidate for
the missing wall layer. Finding and decoding it does not establish that a
particular visible barrier has working collision, or that a complete lap stays
inside the circuit.

## Verified decoder contract

The independently authored `grid-formats::cqtc` reader admits a bounded
positive-count variant: 8-byte quantized vertices, 7-byte triangles, 3-byte nodes
and base-relative leaf lists. It keeps exact four-byte material keys. The
negative-count VCQTC reader and existing wheel queries are unchanged.

All four installed Donington routes were tested with two constructor fill
patterns and two material sheet policies: 16 complete original/repeat cases,
2,062,832 complete context bytes per execution set. The private original probe
executes the whole constructor and every vertex, triangle and material decoder
on owned storage. It checks stack, output and object canaries, source mutations,
mapped-image identity, restored lazy globals and unchanged executable code.
No original function was replaced.

An independent Python reconstruction and current Rust debug/release decoders
match all four source projections exactly: 322,304 bytes per Rust profile.
Counting the repeated policy cases gives 1,289,216 decoded bytes; that larger
number is not four times as much distinct geometry. Twenty-seven recorded
guard checks cover malformed framing/counts/offsets/indices, invalid fill/sheet
controls, image identity and preservation of existing output. All 185 regular
workspace library tests, formatting and all-target warnings-denied Clippy pass;
four source driving tests remain default-ignored and were last explicitly run
in F238.

| Routes | Vertices | Triangles | Nodes | Leaf lists | References |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 and 2 | 2,783 | 2,782 | 829 | 350 | 3,500 |
| 1 and 3 | 2,255 | 2,252 | 693 | 292 | 2,848 |

Route 0/2 source SHA-256:
`14f99310ad89ac030b9b783f914deee8949389b0f12b380008924905b6fb87c1`.
Route 1/3:
`d2f210aa6871419575d2153e79848fdf11be298e69b55db2abe351b9bf12c729`.
The node/list framing is independently parsed and bounded, but this milestone
does **not** execute original quadtree traversal or contact generation.

## Provenance and private evidence

Installed tools were reused: Ghidra 12.1.4 / JDK 21.0.9, MSVC 14.44.35207
with SDK 10.0.26100.0 and strict SSE2 arithmetic, Python 3.12.14, and
Rust/Cargo 1.99.0. No installation or desktop input was needed.

- `local/behavior/cqtc-decode-20261012-final-02/validation.json`:
  SHA-256 `8ef1a8b83919fc2faaee573691fcca67c5f8699a08a6f554ff589850201c1bfe`;
  records current executable hashes, all source inputs, requests, complete
  outputs, repeats, independent projections and guards.
- Original probe SHA-256
  `3f01ce31ffeae4de568e96340e61e2992e0db4b078bf12a61bc5a0dd21a286e9`.
- Rust diagnostic debug SHA-256
  `c71ed361c9ae8e19cda4d098825a33a69623300efbc7c62185efbde672b212f9`;
  release `4278a3a9539a308c5c9ddc8dd4532d4323f867c1cd0551b0729fd6efd5e6316b`.
- `local/behavior/cqtc-decode-build-20261012-draft-02.txt`,
  `cqtc-libraries-20261012-draft-01.txt`, `cqtc-clippy-20261012-draft-01.txt`.
- `local/scene/preservation-20261012-cqtc.json`: all 130 baseline resources
  and the original executable unchanged at 20:23:47.637333 UTC, 12 October 2026.

The actual loader string was not referenced in Ghidra's previously analyzed
region. An identity-checked raw literal-reference scan and bounded instructions
identified the observed callback. Its factory leads to a distinct CQTC
constructor and kind-10 world dispatch, rather than the packed kind-11 handler.
Private surveys and instruction evidence are under `local/behavior/`:
`cqtc-string-refs-20261012-draft-01.json`,
`cqtc-raw-xrefs-20261012-draft-01.json`,
`cqtc-loader-instructions-20261012-draft-01.*`,
`cqtc-decode-instructions-20261012-draft-01.*`, and survey directories
`20261012T1956251165370Z`, `20261012T1957002008724Z`,
`20261012T1957241138800Z`, `20261012T1957511232027Z`,
`20261012T1959462460559Z`, `20261012T2000192977551Z`.
The analyzed image remains SHA-256
`aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

Failures retained: the containing-function lookup failed in the unanalyzed
loader region; an unnecessary guessed collector entry was rejected after the
observed collector output succeeded, so the later actual callee was surveyed
separately. Guessed runtime/driving source paths were corrected using inventory.
Before final validation, native section arithmetic and decoder stack/output
canaries were strengthened. An initially ambiguous byte-count label was
corrected and the full comparison rerun; no numerical tolerance was relaxed.

## Next work

Validate the original kind-10 candidate traversal and fresh contact generator,
then add this separate boundary layer while preserving the existing 245 wheel
shape identities. Heavy impacts still reach the explicit unported handling
damage guard described in `VEHICLE_GEOMETRY_INTEGRATION.md`. Neither that branch,
the complete material factory nor usable boundary driving is claimed here.
