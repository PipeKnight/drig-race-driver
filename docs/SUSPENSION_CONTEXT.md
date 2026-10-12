# Source suspension context (F209)

Recorded 12 October 2026, Europe/Kiev. The source Standard constructor alone does not initialize suspension axes. The outer factory calls whole `9C10B0`, using raw NED wheel positions and copied caster/camber properties, before constructing the Standard car. This explains the zero-axis rejection encountered after F208.

`grid-world::suspension_context::initialize` independently implements the complete selected context initialization. It preserves the complete 9,728-byte input except the observed history resets, normalized axes, wheel links and initialized flag. Positive local Z selects front angles; positive X selects the opposite camber sign from nonpositive X. The original double tangent, SSE reciprocal-square-root seed and refinement order are retained. Links become explicit relative owned-wheel tokens; no address is followed.

`Car::configure_suspension` supplies raw NED positions to that initializer. This composition is applied after the independently verified Standard constructor; it is not execution or equivalence of the entire outer factory. The selected axes/reset inputs do not require resource acquisition. The runtime subsequently converts the known relative wheel links into its existing ordinal ownership boundary.

## Verification

The private native probe calls whole unchanged `9C10B0` and its original tangent/normalization helpers. One lazy data page at E58000 is snapshotted, made writable and restored. No original code or math helper is replaced. Owned self/wheel pointers are checked and normalized for comparison.

Original, original repeat and Rust match **139 complete contexts / 1,352,192 bytes per debug/release profile**: 43 installed CSV/NED owners and 96 authored angle/sign/history controls. Independent assertions check all unchanged bytes, history resets, link identities and normalized axes. **18 rejection outcomes per profile** cover framing, foreign self, wheel count, nonfinite/excess angles and nonfinite position. These are context proofs, not complete outer-factory or driving proofs.

Evidence, all ignored/private:

- `local/behavior/suspension-context-{debug,release}-20261011-final-01/validation.json`
- `local/behavior/suspension-context-20261011-draft-01/`
- `local/behavior/suspension-context-aligned-20261011.json`
- `local/behavior/source-context-math-aligned-20261011.json`
- Fresh dependency surveys `20261011T2310406251903Z` and `20261011T2311437603261Z`.

Pinned tools reused: Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207, SDK 10.0.26100.0, tools prompt 17.14.41, Ghidra 12.1.4, JDK 21.0.9 and Capstone 5.0.6. Native `/W4 /WX /fp:strict /arch:SSE2`.

SHA256:

| Artifact | SHA256 |
|---|---|
| Native context probe | `bc511fbae6eb3a5436970f1213098c624176ff87ec4654ae361cf912f9f5d7da` |
| Rust debug context CLI | `1fb5995adac0100ff3f3e15828ce9ece670ad7a330d73eb43a4ff6ef1cb7c10f` |
| Rust release context CLI | `f1013f9bd14c642cb33c3312da58deb654b2f06db7c2fcb66d32a105737e8036` |
| Private analysis PE | `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514` |

The first native build rejected a shadowed variable under warnings-as-errors; renamed before native execution. A scratch attempt then referenced the missing executable from that failed build; preserved privately. Several guessed source/doc paths and Windows wildcard searches failed read-only and were corrected from inventories. There was no native fault or numeric mismatch in the completed context suite.

## Joined runtime findings

Source context axes and observed recovery scale 3D0=1 allow the source-configured Lacetti through whole A02B70. The native preflight had incorrectly required the old AC0..AC8 inertia cache to be positive, although whole 9F1E10 overwrites it from already-guarded 15A8..15B0 before consumption. That diagnostic guard is corrected in the following runtime integration; Rust already has the correct write order.

A pose reused from an authored contact fixture rejected collision damage in both original preflight and Rust. Ground placement from the actual Lacetti wheel geometry avoids the rejection and matches repeated whole manager/world updates. This does not justify removing the collision-damage guard. Source-only persistent assembly, complete startup defaults, model placement and Bevy controls are tracked in [source driving](SOURCE_DRIVING.md).
