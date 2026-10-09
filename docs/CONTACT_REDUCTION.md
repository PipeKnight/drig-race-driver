# Contact point and direction preparation

9 October 2026, **F144**. Independent Rust reproduces original9A14D0's ordered contact reduction, matches all48prepared operand words in six captured collision calls, and feeds its own results into the F142 collision pipeline without any output difference. This removes two captured intermediate vectors from that diagnostic pipeline. Intervening A06050 descriptor policy and complete callback execution remain separate work.

## Contract and provenance

The callback A064A0 calls9A14D0,9A15D0 andA06050, then supplies the resulting vectors to9EA6E0. Private decompilation `local/behavior/20261008T2042548580424Z/9a14d0.c.txt` and aligned `contact-reduction-instructions-20261009.json` establish the reduction's actual cdecl arguments: linked owner, ordered contact-pointer array, positive contact count, direction-output pointer, point-output pointer. Producer PreparedA is the point and PreparedB the direction; the helper's output-argument order is reversed.

Start both vectors at positive zero. For **every** supplied contact, reverse all four direction lanes by multiplying by-1 when contact0C==0 and contact08==owner. Add `contact3C * direction` to the weighted direction sum, with product as the first ADDPS operand. Add each contact50vec4 to the point sum, with contact as the first operand. Multiply the point sum by direct f32 `1/count`. The direction is **not divided by count or normalized**. Disabled/zero/nonpositive generation candidates still participate; the helper does not inspect the later generation eligibility byte73. All four lanes participate, including W. The production caller already excludes zero contact count; the independent bounded API rejects0and>80.

`tools/probe_contact_reduction.c` executes the whole original routine against owned aligned contacts and both output vectors. No original helper, import, TLS initialization, code patch or skipped instruction is involved; executable sections are checked unchanged. Reverse identity is normalized into the original owner's comparison predicate, and all numerical inputs/outputs are preserved. Probe SHA-256 `8ee5000505b2465989624dcf919c95d48bc0deac24f898159cc1b010e7fd8a30`. MSVC14.44.35207/SDK10.0.26100.0 `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`; pinned analysis SHA-256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

`grid_world::contact_reduction::reduce` consumes the existing typed generation contacts, using only reverse/point/direction/value3C. `grid-contact-reduction` adapts private ICR1: magic0x31524349/u32cases1..1024; each row count1..80 and40bytes/contact (reverse u32,weight f32,point vec4,direction vec4). Output is point vec4 followed by direction vec4,32bytes/case. Size limit4MiB, create-new output only under ignored `local/`. Original failure output may be partial; Rust rejects before creating output.

## Verification

Retained live capture10 SHA-256 `c34887e65bb5bc0f6f7e8274422f4c169b6eb9f7351713e50eb04cc1cf745519` supplies actual ordered contacts and prepared vectors. Original/repeat/Rust match **48/48live operand words** in debug/release. Independently produced Rust point/direction are then substituted into the supplied inputs of the whole Rust collision path; it still matches **65,057live output words+6controlled seed words**. The descriptor input remains captured after A06050, so this is not whole-callback verification. Reports: `local/behavior/contact-reduction-live-20261009-debug-01/validation.json`, `...release-final/validation.json`.

Seed20081009 generates **512authored cases/4,096output words**, exactly equal to original/repeat in both builds. Controls include counts1/2/3/7/16/79/80, reversal, signed zeros, negative/zero weights, cancellation, smallest positive subnormal weights, arbitrary W lanes, uniform points and varied magnitudes/order. Nine malformed requests reject. Reports `contact-reduction-controls-20261009-debug-01/validation.json`, `...release-final/validation.json`. No arithmetic mismatch observed.

Rust diagnostic SHA-256: debug `970d2050c4e1634fb156bce2ead313426e0aa8be26964aa72971e02ebc7d2519`, release `21e53b083f97dd1bce7fe7316baa0058420cbee55f46f584dc76ac61922f280f`. Rust/Cargo1.99.0,104workspace tests/formatting/warnings-deniedClippy/PythonAST pass. No new desktop input, reference capture, settings change or tool installation for F144. Last Steam preservation remains2026-10-09T09:03:41.528915UTC,130resources+executable unchanged. GRID remains explicitly paused and debugger detached after F143. F143 research7006c84/Rust7324f84 are pushed.

## Next work

Recover9A15D0's inverse pose and A06050's descriptor-side policy with directly observed callback inputs/returns. Collector output and reduction are implemented, but geometric contact production, resource-derived initialization, original random cadence and Bevy driving integration are still open.
