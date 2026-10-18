# Donington curb/asphalt seam investigation

19 October 2026. Current graphics-roadmap surface task. Evidence is private under local/graphics/surface-textures01/; no purchased assets enter Git.

## Observations and scope

Owner reported the ragged, harsh curb/road boundary in curb03-neural-native4k01.png. Matched native3840x2160 purchased, conventional Lanczos4x and neural4x crops show that the original texture path already has a visible junction; neural replacement makes the change of grain/contrast more conspicuous. This is a visual judgement, not a universal defect attribution.

Six CPU rays across y1400/x1300..1550 each hit one selected triangle: road !MQ/track1/2!3 then curb !SQ/rumble3. There is no duplicate hit in that sampled strip. This is not proof that no other part of the track has overlapping geometry.

A read-only source audit compares453 road and56 curb triangles. Twelve edges share exactly the same transformed endpoint coordinates; shared vertex normals are equal. The sampled seam is therefore joined geometry, not an observed gap or normal discontinuity. The two sides sample distinct diffuse images: don_track1_d versus don_rumble_strip_03_d. Their common edge uses road U0.9116211 and curb U0.875 (one endpoint0.8754883); V matches in most cases, but differs up to about0.00708 in others. Source red layer controls are0/1 out of255; tiny second-layer contributions are retained. Both use the same macro/skid bindings and matching macro coordinates at the inspected pair.

These facts support a material/texture discontinuity for this seam. They do not establish that every visible irregularity is texture-only, nor original GRID visual parity. Filtered source-alpha specular response also remains a separate variable.

## Diagnostic extension

grid-surface-texture retains its original locator syntax and adds:
- GAME_ROOT --triangle DRAW INDEX
- GAME_ROOT --triangle DRAW all

It prints selected triangle indices, transformed positions, original UV streams, ARGB controls, saved normals and material sampler bindings. Full-draw output is bounded to50000 triangles. Output belongs under ignored local/ because it describes the purchased resources. It does not alter imports, physical geometry, render attributes or user cameras.

Validation: actual single triangles and both full draws work; original locator output is identical after normalizing line endings; invalid/out-of-range indices return1. Exact-source release probe Clippy passes. An initial exact-text comparison falsely failed on CRLF versus LF; logical-line comparison passes. The private Python edge audit initially shadowed its regex module with an edge dictionary; corrected without changing source data. Failed attempts are recorded here; no failed render result is relabelled a pass.

## Private RGB transition experiment

Prepared two optional1024x2048/full12mip RGB carriers, one from Lanczos and one from the neural candidate. Only the asphalt-side strip is treated:
- smoothstep linear-light RGB transition over curb U0.825..0.875;
- donor is the purchased road texture, sampled at U0.9116211 at the join;
- U density ratio0.7 is authored and approximate, not recovered engine data;
- V is periodic and matched directly, so differing saved V phases cannot all be exact;
- base columns below U0.825 remain byte-identical: paint and white-line regions are preserved;
- runtime independently restores purchased packed alpha mips through the existing surface-RGB contract.

No global colour rebalance, added sampler, geometry move, screen blur or shader modification. These are local aesthetic pilots, not default replacements. Private recipes and hashes: prepare-seam-pilot01.py and curb03-{lanczos,neural}-seam-pilot01.json. Neural carrier SHA701ec63ff523e089e0a9d6d535ec6780d1ebce013db9e528955cd45a113e8695.

Both native4K captures complete successfully on the unchanged2591f75 candidate/SHA67191990..., and the inspected comparison shows a less conspicuous join with continuous-looking grain near the edge. This is not a claim of a completely invisible seam. Motion validation is pending at this checkpoint. Stable installed b9 graphics is unchanged and no launcher selects these derivatives.

## Next acceptance

Check motion and a second viewpoint, forced RGBA upload, all source mip alpha preservation and unaffected physics endpoints. Keep the neural inferred-detail/grain decision separate from edge treatment. Before default adoption, audit adjacent material pairs elsewhere; a successful two-draw donor mapping is not a safe universal road/curb remap. A reusable preparation tool should require explicit audited mappings and provenance rather than assume every curb joins the same asphalt.

## Reproducible preparation and subsequent acceptance

tools/prepare_surface_edge_candidate.py turns the pilot into explicit, bounded preparation. Supply purchased DDS donor and an opaque complete BGRA8 carrier, pin both SHA256 values, and provide the audited transition/mapping. It refuses packed/cutout carrier alpha at every mip, malformed/truncated chains, nonfinite mappings, existing outputs and destinations outside this checkout's ignored local/. Generates DDS, PNG preview and provenance JSON; NumPy2.3.5/Pillow12.3.0 on bundled Python3.12.14, no additional installation. Source packed alpha is still restored by surface_rgb.rs, not derived or replaced by this RGB tool.

Example (paths/hashes refer to private inputs, substitute verified actual values):
```powershell
python tools/prepare_surface_edge_candidate.py local/CARRIER.dds local/DONOR.dds local/EDGE_CANDIDATE --carrier-sha256 CARRIER_SHA256 --donor-sha256 DONOR_SHA256 --transition 0.825 0.875 --donor-u-at-edge 0.9116211 --u-density 0.7
python tools/validate_surface_edge_candidate.py
```

Four synthetic tests check linear-light transition values/full opaque mip packing, periodic donor interpolation, packed alpha/truncated chain rejection, and invalid/nonfinite mapping rejection. Actual CLI reproduces the tested neural pilot DDS SHA701ec63f... byte-for-byte. Existing-output/public-destination/bad-input-hash guards reject without creating a replacement.

Additional evidence:
- Both1200tick/virtual30FPS sequences have30unique frames and4equal physical exit records excluding wallclock.
- Both6500tick/1080p/virtual30FPS runs complete1valid90.021579891second lap and4equal physical exits. This is current F415 control versus RGB pilot, not equality to older b9 driving code before peer F413 improvements.
- Additional curb-pass capture at ticks6131..6254 has60unique30FPS frames; first/last inspected,4physical exits match the full pilot run. This addresses the first sequence's weak actual-curb visibility.
- Reverse-view native4K source/pilot and forward forced-RGBA pilot are nonblank/error-free; reverse pilot inspected. RGBA logs report0BC assets,537images/527decoded. It is desktop fallback operation, not a phone test or BC pixel parity.
- Projected shared-edge display-RGB difference at434sample pairs,3pixels each side: neural23.3720 → matched15.4289code values without analysis filtering; after4pixel Gaussian analysis filter9.5353 →2.5842. Analysis filtering is only for measurement, never fed into the renderer. This local metric is not an overall quality score.
- Source81aff6e:704workspace tests/19result harnesses including empty/doctest harnesses,0failed0ignored; feature/default alltarget workspace Clippy clean11.78s/2.85s. Solo canonical raw stdout SHAaad9a07a...e41b unchanged. Quick soak still pending at this checkpoint.

Current result remains optional: neural inferred grain/cracks and all adjacent material pairs still need broader acceptance. No launcher/default or installed executable change is implied by these tests. New remote F416–F427 must be preserved in a later integration; these checks do not cover that future merge.
