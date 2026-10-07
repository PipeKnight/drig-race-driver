# Donington VCQTC collision geometry

Continuation F086/F089: all245loaded chunks now match complete packed geometry, and original quadrant/leaf traversal matches2,964supplied source/analytic casts. See [production grid](PRODUCTION_GRID.md) and [packed queries](PACKED_QUERIES.md). Earlier opaque-node and sampled-only statements below are historical; outer shape normals/materials and whole-world policy remain separate.

Recorded 6 October 2026. The original `route_0/track.jpk` contains 245 uncompressed `.vcqtc` chunks. Independent Rust decoding recovers 68,803 quantized vertex records, 119,784 triangles, 115,869 node words and 57,208 leaf triangle lists. Independent Python checks every decoded position bit, triangle/material/flag, node word and leaf list against original bytes. Corrected-layout GPU overview and start-grid views were inspected successfully.

This is collision-resource geometry, not an implemented original collision solver. Node words and leaf references are decoded and checked, but original child subdivision/traversal rules, triangle flags and contact handling remain unverified. The viewer's colors are diagnostic labels, not original surface textures.

## GRID variant and byte contract

All header fields are little endian. Six f32 bounds at offset 0 define min XYZ / max XYZ. Three signed i32 values at offsets 24/28/32 are negative in all selected chunks; their magnitudes count triangles, vertices and materials. The current reader explicitly accepts this old GRID variant only. Four u32 offsets at 36/40/44/48 identify vertex, node, triangle and leaf-reference sections. Four-byte material identifiers begin at 52. Section boundaries agree exactly for all 245 chunks:

- Vertex section starts at `52 + material_count*4`; each record has eight bytes.
- Node section follows `vertex_count*8`; two-byte words extend to the triangle offset.
- Triangle section has `triangle_count*4` bytes and ends at the reference-workspace offset.
- Leaf lists cover the remaining entry bytes.

Each vertex stores unsigned big-endian X24 / Y16 / Z24. Decode using `min + packed * ((max-min)/2^bits)` with f32 operations. This reconstruction is bit-exact between the native reader and independently rounded Python arithmetic; exact original CPU arithmetic remains untraced.

For triangle bytes `a,b,c,d`, the observed original packed decoder uses `v0 = (a<<4) | (b>>4)`, `v1=v0+c`, `v2=v0+d`, material `b & 15`. The supported <=1024-vertex subset requires `a<64`. All Donington entries meet this condition. Material codes observed: `GRAS`, `STDR`, `LGDR`, `MPDR`, `RMBL`, `ROCK`, `MUD\0`, `CONC`. These names suggest surface classes but do not establish friction coefficients.

**Later consumer correction:** the community template masked `a&63` and described the upper bits as uncertain sheet information. Earlier parser/tests retained these as flags. Original functions `0x00b19ee0` and `0x00b23450` instead use the full first byte for the index. No normalization/masking intervenes in the observed leaf path. Actual Donington geometry is unaffected because every upper-bit pair is zero. The reader now rejects nonzero upper bits in its supported subset; the old diagnostic flags column remains a reserved zero for report compatibility. The synthetic nonzero-flags fixture was corrected and an explicit rejection test added. Larger vertex-index variants remain unsupported rather than silently masked.

Node words are big endian. A high-bit word other than `0xffff` refers to a leaf list at `(word & 0x7fff)` relative to the reference section. Other words remain opaque. Leaf lists begin with a BE u16 triangle index, then cumulative unsigned byte deltas, ending with `255`. Delta `254` advances the index without emitting a triangle. Every retained index and leaf offset fits its table.

## Failed layout and correction

The primary template includes several packed-triangle variants. Initially selected the older-looking four-byte variant with `v0=(a<<2)|(b>>6)`, material `b&7`, flags `(b>>3)&7`. Both independent readers agreed on its bytes and every index fit, demonstrating that structural validity alone is insufficient. Its GPU result was fragmented, and a vertical start-grid probe missed all road triangles.

Switching to the template's earlier-game/D3 triangle mode makes the full circuit coherent and finds the road beneath `slot_00`. Both results are retained. No data was silently changed to make the picture fit; only the explicitly documented bit interpretation changed. Primary reference: Petar Tasev's MIT-licensed [pinned VCQTC template](https://github.com/EgoEngineModding/Ego-Engine-Modding/blob/f3fe9ee0f6e8379c64bc4a29e1d1b586bba5d1c1/src/010%20Templates/vcqtc.bt). Repository license and downloaded source stay under `local/references/ego-tools/`; Rust is independently authored.

## Validation and use

At X −183.11911 / Z 19.41256, below max Y −7.147942, an independent vertical triangle probe finds `qt_01_10_01_00.vcqtc`, triangle 801, material `STDR`, surface Y **−12.2545865747**. The grid anchor is Y −12.147942: approximately 0.107 above this surface. This checks coordinate alignment, not initial suspension compression, ride height or original collision-query equivalence.

`grid-formats/src/collision.rs` bounds sections/counts, validates finite quantization, triangle/material indices and leaf references, and preserves unknown flags/node words. Synthetic tests cover nonzero packed index high/low bits, material/flags, quantization, continuation deltas and malformed/truncated inputs. Two fixture mistakes (a mutation that changed no byte and an unrelated scene source ID) were corrected; subsequent **28 format tests**, workspace formatting and warnings-denied Clippy pass.

```powershell
cargo run -p grid-tools --bin grid-collision --locked -- 'C:\Program Files (x86)\Steam\steamapps\common\Grid' local/scene/new-collision.tsv
cargo run -p grid-app --locked -- --scene collision --render local/scene/new-collision.png
cargo run -p grid-app --locked -- --scene collision --view start --render local/scene/new-collision-start.png
```

Private evidence: `track-collision-jpk-survey.json`, `vcqtc-exploratory-validation.json`, `vcqtc-native.tsv` / `vcqtc-native-validation.json` (initial wrong layout), `vcqtc-triangle-layout-comparison.json`, `vcqtc-native-grid-layout.tsv` / `vcqtc-native-grid-layout-validation.json`, `donington-collision-layout-initial.png`, `donington-collision-grid-layout.png`, `donington-collision-start.png` and run/test/lint logs under `local/scene/`.

Next: relate entity body/shape frames, decode physics/material binary XML, identify executable query/vehicle consumers and compare actual contacts. Boundary CQTC and route CLM have separate layouts; this reader does not claim to support them.

## Targeted original query trace

The generic segment wrapper dispatches through a file-backed 16-entry function-pointer table at `0x00e51fd4`, indexed by a runtime shape type. Slot 11 points to `0x00b23800`, which consumes four-byte packed triangles and material low nibbles. This identifies a compatible packed-mesh path, not a captured runtime type selection for Donington.

`0x00b1abc0` decodes BE X24/Y16/Z24 using runtime scale and offset vectors, consistent with the native geometry reader; exact initialization of those vectors and CPU rounding still need checks. `0x00b23630` branches on the node high bit, calls `0x00b19ff0` to subdivide horizontal bounds into four quadrants and uses a BE child-word index into contiguous node words. Empty leaves are `0xffff`; `0x00b23450` expands leaf lists and invokes triangle intersection `0x00adc290`, shortening the current segment after a closer hit. Leaf expansion `0x00b1a370` and the low-level triangle test are next targets. Traversal order/ties/filtering remain unimplemented in our independent BVH.

Private evidence: `local/behavior/chunk-query-dispatch-table.json`, static references `20261006T1410394897210Z/address-refs.tsv`, the 16 actual dispatch-target decompilations under `20261006T1413027837263Z/`, decode/traversal outputs under `20261006T1414314231908Z/`, and leaf/subdivision outputs under `20261006T1416328260182Z/`. Read-only PE identity checks precede table extraction. All pseudocode remains private; public code is independently authored from documented observations.

## Read-only running-reference linkage

At 14:41–14:45 UTC, guarded inspection of the existing private GRID PID 19680 confirms module base `0x00400000`, original recorded creation time, executable hash and **byte-identical live triangle kernel**. Access uses only query/read rights; no input, thread suspension, injection or game-file write occurs. The chunk registry contains **245 entries**, matching the source archive.

Eight sampled collision wrappers point to shape **type 11**, confirming the packed-mesh dispatch family is actually loaded in this race. Their runtime headers match eight original source entries by bounds, absolute counts and 64-byte vertex/triangle prefixes. Runtime counts are positive after loading. All sampled scale and offset components match our original-source quantization calculations **bit for bit** at shape offsets 0x250 and 0x260.

The first header-only matching assertion failed for one sample because two different source entries share identical bounds and counts. This was an insufficient identity test, not a quantization mismatch. A second bounded snapshot added geometry prefixes and resolves all eight uniquely. Both snapshots and the failed assertion remain private. This validates loaded shape/header linkage and sampled quantization initialization; it is not a trace of a particular runtime query or exhaustive geometry comparison of all 245 loaded chunks.

Evidence: `runtime-collision-20261006T1441597176072Z.json` with its initial validation, `runtime-collision-20261006T1445138600409Z.json` and successful `-validation.json`; scripts `tools/read_reference_collision_state.ps1` and `tools/validate_runtime_collision.py`, all under the durable project conventions.
