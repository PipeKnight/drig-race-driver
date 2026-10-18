# Source car livery decals and missing race number

18 October 2026. Owner noticed the Lacetti roof differs from the original-game recording: the prototype has a conspicuous empty white rectangle, whereas the original has a dark race-number marking. This is a missing presentation layer, not evidence that the blank base texture was decoded incorrectly.

## Confirmed source evidence

- Purchased livery00 main diffuse contains blank white number backing panels on the roof and both doors; the number is absent from the base RGB itself. Source and enhanced body atlas both preserve those panels.
- Purchased `cars/lac/decals.xml` has a `number` decal type with three placements. Their saved UV positions coincide with those panels when converting source V into top-left image coordinates; the roof placement has a quarter-turn rotation. Exact coordinates and hashes remain private in `local/graphics/materials-livery01/missing-race-number01.json`.
- Purchased `cars/user_liveries/numbers/font0.pssg` contains ten named digit textures0.tga..9.tga, each128x128 BC3 with alpha and seven declared mips. Private number-font0-survey01.json records the resource inventory.
- Before this restoration, prototype car loading bound the base diffuse without reading the decal layout or composing race numbers. That omission produced unmarked panels on the roof and both doors. The implementation and acceptance history below distinguish the repaired layer from the original diagnosis.

The recorded roof marking is consistent with that number layer. The lossy rear-view frame alone does not establish the digits; the later source-database audit below identifies the saved livery number as `01`. Saved placement resources do not establish runtime number selection, per-digit composition/alignment, opacity treatment, or complete original rendering order. Other sponsor decal slots also exist; their source/profile selection needs a separate binding audit. Do not invent a fixed number or assume all slots must always be populated.

## Implementation continuation

Restore the missing presentation layer within Lacetti material/detail work: identify the profile/event race-number selection and original composition; load source digit alpha; validate rotation, spacing and both door orientations; compose without replacing or damaging packed material maps. Apply numbers separately per vehicle/livery and before final mip preparation, so the optional8K body atlas also receives a sharp number at the correct normalized location. Reuse shared `texture_upload::image`/linear helper after device selection, including the no-BC path. Keep original/derived images private and preserve Steam files.

Compare a close roof/side view, native4K and a moving sequence with unchanged physical state. Check all three panels and source versus enhanced atlas; a screenshot with arbitrary text painted on is not proof of the source compositor. G107 was a read-only finding; the subsequent implementation status is recorded below. Lighting/reflections can also change panel brightness, so restoring the number alone is not a promise of full original-image parity.


## Source selection and bounded readers — implementation draft

The source schema explicitly defines `vehicle_livery.number_on_livery` separately from `file_string`. Independent fixed-schema NDBS decoding checks all78tables, every row marker and exact EOF; selected343livery entries are stored privately. The saved player livery00 selects `01`, and Lacetti livery01 selects `25`. Preserve leading zeroes as text: integer conversion changes the visual result. Empty/`-1` entries disable the saved number; career/event overrides of these saved values remain untraced.

`grid_formats::car_decals` reads only number placements from bounded plain XML and preserves UV/rotation/flip. `livery_numbers` interprets only the two needed string columns while validating the rest of the DB framing against source schema. The observed string storage reserves2bytes thenaligns4; this is the tested GRID subset, not a universal EGO database-format claim. All67formats library tests pass, including purchased resources and synthetic corruption/leading-zero cases.

Pinned private executable read-only Ghidra/capstone evidence identifies original placement loader6c5780; the first query failed because that function had not been analyzed, then a precedingret/int3boundary and vtable entry independently establish its start. No original executable or project changes were saved. Render-with-max-alpha shader variants confirm a separate compositing path, but static variant extraction does not establish its complete runtime use.

CPU rendering draft loads source glyph alpha and uses source UV slots, linear-light alpha filtering and regenerated full mipmaps. Equal-width per-digit layout is explicitly authored pending closer original-layout proof. Default app visuals stay unchanged until GPU acceptance. Source-selected opponent numbers must use their own livery banks; player overrides apply only to the player. A manual decimal-text control remains useful for asymmetric orientation checks.


## CPU renderer connected — GPU acceptance pending

`--numbers source` now reads the saved number for player livery00 and each actual opponent livery bank; `--numbers off` is the unchanged default. `--race-number 01` preserves decimal text and overrides only the player, including when combined with source mode. The render profile must be enhanced. Number composition runs after optional diffuse overrides, so the same normalized slots work on the source2048atlas and an8192candidate. No sponsor/damage/normal/glass maps are changed.

Glyphs are sampled with bilinear premultiplied linearRGB/alpha, then applied over the opaque body diffuse. Full linear-light mip chains are regenerated into BGRA8 DDS and uploaded through the existing shared helper after device selection. Decoded base texels outside the three placement bounds remain unchanged; regenerated lower mips can differ from authored source mips. This is a source-data-driven adaptation, not original D3D compositor equivalence.

Player and opponent outputs are staged together before replacing any CPU texture. A late missing-opponent-base failure leaves both banks byte-identical; independent bank tests select01versus25without leaking player numbers to opponents. Five composition tests/seven graphics settings tests and67formats library tests pass; workspace all-target Clippy29.60s passes after fixing a redundant test-only vec allocation. A627test workspace baseline precedes the leading-zero/source-bank conversion and is not claimed as a full final-version test. GPU roof/both-door/4K/moving/fallback checks are next; existing playables remain untouched.

Enabling source mode adds read-only dependencies on database/schema.bin, database/database.bin, cars/<model>/decals.xml and cars/user_liveries/numbers/font0.pssg. Phone copies need these resources too; no-BC upload/memory acceptance has not yet been established for this feature. Equal-width digit cells, linear-space composition and fixed font0 remain explicit adaptations; career/event number overrides and original per-digit spacing remain open.


## First GPU acceptance — sourcec0b30eb

Release3m11s/SHA256a1931b87... on the F380 integration base. Six native3840x2160captures inspected: matched empty/source-number roof, both door sides, source-atlas forcedRGBA and source-number on the8192body candidate. Both doors read01without mirroring; the roof quarter-turn stays in the saved backing panel. The8K image logs BGRA8sRGB/8192/full14mips through the shared helper. Source2048numbered body has12mips.

ForcedRGBA reports0BCassets/557images/547decoded; whole-frame max65/mean0.172745byte and roofROI(2040,800)-(2370,1060)max65/mean0.012735byte are measured differences, not byte parity or phone performance proof. No new visible number-layout issue in the inspected static views. Roof contact sheet contains native-pixel crops from the same prototype/camera/light, not an original-game matched shot.

Paired1200tick off/source captures have all4physical/handling/deformation/lap exit lines equal after walltime removal. Both30frame sequences are unique, tick1102..1162/maxrequestdt2.05ns; first/last source side frames inspected. GPU bank logs select player01/lac_00andopponent25/lac_01separately. Only known nilUUID gamepad warning. Private numbers-static-validation01.json, numbers-rgba-audit01.json, numbers-motion-validation01.json and numbers-roof-comparison01.png/.json. Full6500tick/native4K paired lap acceptance and newest main integration remain pending; no ordinary launcher default or primary playable promotion yet.


## Full-lap failure retained and launcher candidate

The c0b30eb control run completed6500ticks/native4K and one valid89.955257484s lap. The source-number run hit the configured600s render-capture timeout at4674ticks; it did not complete a lap. All four state lines at that common tick match the control after removing walltime. Private numbers-full-timeout01.json and the failed log are retained. This does not establish number-render performance or full-lap acceptance; repeat with a supported longer capture budget after the F381 rebuild, without changing physical timestep or capture resolution.

The graphics launcher candidate enables saved source numbers and accepts third argument no-numbers for comparison; global CLI default remains off. This launcher must be used with a verified executable supporting the new flags. Primary playable publication remains pending combined-build checks. CLI help now describes both controls and leading-zero preservation. Lighting/exposure defaults stay unchanged.


### Trying the graphics build

After executable acceptance/publication, run `Play GRID Graphics.cmd` from the graphics checkout (or the updated main checkout). It enables saved numbers for Lacetti without changing the global CLI default. `Play GRID Graphics Body.cmd` combines the optional private8Kbody candidate with the same number layer; it requires the existing private override files. For the empty-panel comparison use `Play GRID Graphics.cmd "" 3840x2160 no-numbers`. The third argument selects one diagnostic preset, so do not combine it with body/probe/wheels in that launcher; use CLI flags for combined research captures. Source database/schema, per-car decal XML and number-font PSSG are read from the purchased installation, never modified or published.


Primary executable preservation: an independently installed primary playable reports27665f2704d5-dirty/SHA6364b328... while primary source main remainsfde38ef. Its uncommitted build provenance cannot be reconstructed from main alone; do not overwrite it with the graphics candidate. The graphics launcher checks help capability before adding number flags, so source integration remains usable with that older playable. Verified numbers are available in the graphics checkout build; main's executable is a separate publication decision. This is a compatibility check, not a claim that the older executable has the new numbers.


## Accepted paired lap and graphics playable — 18 October

Source31e74b689b19/SHAbee2138a... passes the repeated full paired6500tick/native3840x2160run with supported1800s capture budget. Both complete one valid89.955257484s lap; all four final physical/handling/deformation/lap lines match after removing walltime. Final images inspected, original-font01roof quarter-turn visible and attached. No render errors; only known nilUUID gamepad warning. Wall completion449.49s versus672.89s under changing workstation load is not a controlled performance comparison. Initial600s timeout remains retained, not hidden.

632release tests/12executables and all-target Clippy pass on the F381 integration. Later F382 route-prop caching is merged in source with its history preserved; all-target Clippy passes again2m06s, but the published executable deliberately stays the GPU-validatedF381/31e74b. No F382 executable/full-lap claim. Static roof/bothdoors/8K/forcedRGBA and30frame checks remain the earlier c0b graphics-equivalent evidence. No phone performance claim or original compositor parity claim.

Graphics checkout target/release/grid-app.exe and local/graphics/verified-grid-app.exe now contain that verified build. Previous529dad80... executable and current-build manifest are archived privately. Primary C:/Dev/Reassembly executable6364b328.../27665f2704d5-dirty remains byte-identical. Graphics launchers enable source numbers where supported; capability guard is tested on both executables (new0/old1 findstr match). Private current-build.json records built/integrated revisions and unbuiltF382scope. Next actual material change awaits production-channel/uniform binding proof rather than treating the packed map as generic PBR roughness.
