# Source car livery decals and missing race number

18 October 2026. Owner noticed the Lacetti roof differs from the original-game recording: the prototype has a conspicuous empty white rectangle, whereas the original has a dark race-number marking. This is a missing presentation layer, not evidence that the blank base texture was decoded incorrectly.

## Confirmed source evidence

- Purchased livery00 main diffuse contains blank white number backing panels on the roof and both doors; the number is absent from the base RGB itself. Source and enhanced body atlas both preserve those panels.
- Purchased `cars/lac/decals.xml` has a `number` decal type with three placements. Their saved UV positions coincide with those panels when converting source V into top-left image coordinates; the roof placement has a quarter-turn rotation. Exact coordinates and hashes remain private in `local/graphics/materials-livery01/missing-race-number01.json`.
- Purchased `cars/user_liveries/numbers/font0.pssg` contains ten named digit textures0.tga..9.tga, each128x128 BC3 with alpha and seven declared mips. Private number-font0-survey01.json records the resource inventory.
- Current prototype car loading binds the base diffuse but does not read this decal layout or compose race numbers. Therefore the unmarked white panels are expected from the present incomplete loader. This omission also applies to the door panels, not only the roof.

The recorded roof marking is consistent with that number layer. Exact digits in this lossy, moving rear-view frame are not assigned here. Saved placement resources do not establish runtime number selection, per-digit composition/alignment, opacity treatment, or complete original rendering order. Other sponsor decal slots also exist; their source/profile selection needs a separate binding audit. Do not invent a fixed number or assume all slots must always be populated.

## Implementation continuation

Restore the missing presentation layer within Lacetti material/detail work: identify the profile/event race-number selection and original composition; load source digit alpha; validate rotation, spacing and both door orientations; compose without replacing or damaging packed material maps. Apply numbers separately per vehicle/livery and before final mip preparation, so the optional8K body atlas also receives a sharp number at the correct normalized location. Reuse shared `texture_upload::image`/linear helper after device selection, including the no-BC path. Keep original/derived images private and preserve Steam files.

Compare a close roof/side view, native4K and a moving sequence with unchanged physical state. Check all three panels and source versus enhanced atlas; a screenshot with arbitrary text painted on is not proof of the source compositor. No number rendering has been implemented or promoted in this investigation. Lighting/reflections can also change panel brightness, so restoring the number alone is not a promise of full original-image parity.


## Source selection and bounded readers — implementation draft

The source schema explicitly defines `vehicle_livery.number_on_livery` separately from `file_string`. Independent fixed-schema NDBS decoding checks all78tables, every row marker and exact EOF; selected343livery entries are stored privately. The saved player livery00 selects `01`, and Lacetti livery01 selects `25`. Preserve leading zeroes as text: integer conversion changes the visual result. Empty/`-1` entries disable the saved number; career/event overrides of these saved values remain untraced.

`grid_formats::car_decals` reads only number placements from bounded plain XML and preserves UV/rotation/flip. `livery_numbers` interprets only the two needed string columns while validating the rest of the DB framing against source schema. The observed string storage reserves2bytes thenaligns4; this is the tested GRID subset, not a universal EGO database-format claim. All67formats library tests pass, including purchased resources and synthetic corruption/leading-zero cases.

Pinned private executable read-only Ghidra/capstone evidence identifies original placement loader6c5780; the first query failed because that function had not been analyzed, then a precedingret/int3boundary and vtable entry independently establish its start. No original executable or project changes were saved. Render-with-max-alpha shader variants confirm a separate compositing path, but static variant extraction does not establish its complete runtime use.

CPU rendering draft loads source glyph alpha and uses source UV slots, linear-light alpha filtering and regenerated full mipmaps. Equal-width per-digit layout is explicitly authored pending closer original-layout proof. Default app visuals stay unchanged until GPU acceptance. Source-selected opponent numbers must use their own livery banks; player overrides apply only to the player. A manual decimal-text control remains useful for asymmetric orientation checks.
