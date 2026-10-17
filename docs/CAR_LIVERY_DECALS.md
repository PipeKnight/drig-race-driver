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
