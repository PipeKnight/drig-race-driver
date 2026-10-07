# Untransformed packed shape outputs (F090)

`grid-world::packed_shape::query` builds on the verified packed traversal and returns the chosen triangle's fraction, winding-based normal and material code. It requires one supplied material per triangle. World dispatch, source grid construction and nonzero shape transforms remain separate.

The original wrapper `b23800` decodes the selected triangle again using the shape's XYZ scales/offsets, subtracts two edges and forms their XYZ cross product with W=0. Squared length groups `(Y²+X²)+(W²+Z²)`. SSE `rsqrtps` seeds a refinement `(1-(squared*seed)*seed)*(seed*.5)+seed`; multiplication normalizes each component. Source winding is retained, without forcing the normal upwards. The low triangle nibble selects header material slot `34+index*4`. The native API resolves that slot to the supplied source code, avoiding runtime addresses.

The original sets auxiliary output to zero on hit and miss. A supplied default shape material is written before testing and overwritten on hit; fraction and normal remain unchanged on miss. The native API returns `None` on miss; the diagnostic adapter checks these original defaults separately. Finite inputs, matching material counts and positive finite normal length are authored bounds, not claims about original validation.

## Comparison

**2,964/2,964 cases and 12,687/12,687 output words match exactly in debug and release:** 759 hits, 2,205 misses. Every one of 245 installed Donington chunks has twelve casts; 24 authored cases exercise repeated triangles, layers, continuation lists and zero-length hits. Material table entries in the isolated original fixture are source-code DWORDs, never dereferenced. XYZ decoding scales and offsets match the verified production subset; scale W=1/offset W=0 is a supplied fixture, irrelevant to XYZ normals because every vertex has equal W.

Evidence: `local/behavior/packed-shape-20261007-02/validation.json` (debug), `03/validation.json` (release). The probe executes whole original `b23800`, including all real traversal/leaf/primitive/decoder/cross helpers. No stubs, code patches, imports, TLS, entry point or live injection. Only isolated data pages ec2000 (cull cache) and e58000 (lazy normal constants) become writable, then are restored. Probe SHA-256 `fab37147fe33ad1a1d60f810450683d9f4bcdda1ff9e02670487d3f663b45e6d`. Installed track archive hash is guarded before and after comparisons.

Initial run `01/` misclassified 1,515 misses because the authored probe interpreted the full return register rather than its low byte. Instructions show `mov al,1`/`xor al,al`; corrected function-pointer return type is `unsigned char`. Actual hit outputs already matched. The failed run remains private. Initial Clippy rejected two unnecessary usize casts; removed without arithmetic changes, final warnings-denied workspace Clippy and formatting pass. Full workspace tests pass; logs `packed-shape-final-tests-20261007.txt`, `packed-shape-final-clippy-20261007.txt`. The winding/material tie test specifically checks downward normal and later equal-hit material selection. Existing pinned Rust/MSVC/Python tools reused; no installation, game run or user-file changes.

Tools: `grid-packed-shape`, `probe_packed_shape.c`, `build_packed_shape_probe.cmd`, `validate_packed_shape.py`. Private instruction evidence `packed-shape-query-instructions-20261007.json`; decoder/triangle address surveys `20261006T1414314231908Z/`; cross survey `20261007T1659177524167Z/67c200.c.txt`.

[Production linkage](PRODUCTION_GRID.md) confirms all 245 track shape transforms are zero in the retained run. This does not establish nonzero transform behavior or whole-world/contact fidelity.
