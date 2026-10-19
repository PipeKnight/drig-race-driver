# Lacetti clearcoat and road-mask follow-through

19 October 2026. Owner requests continued graphics work. Keep performance, physics, crowns and camera features outside this step.

## Candidate source and independent changes

1fb5db4 resolves an omitted surface-specular option to SourceBase only for enhanced, source-layer, non-collision track scenes. The explicit Off/WhiteControl controls survive flag order. Baseline, car-only and single-layer scenes retain their former behavior. This avoids editing a shared primary checkout or changing the baseline engine agent's settings; the dedicated updated executable will supply the normal graphics-launcher behavior after acceptance.

8949dd0 adds bounded player-paint coat controls: --paint-clearcoat0..1 and --paint-coat-roughness0.04..1. Reject nonfinite/out-of-range values and baseline/unlayered explicit overrides. Defaults remain0.20/0.18, preserving the prior coat. Source base mask/roughness, albedo, glass, tyres, lights, physics and opponent materials are independent. These are authored Bevy parameters, not recovered original GRID scalars.

## Planned comparisons

Native3840x2160, same source sun/sky, EV13/AgX, car pose, source masks and camera as G154. Capture legacy roadOff/coat0.20/0.18 for comparison to the installed3bfc image; capture implicit source-road/coat0.20/0.18 against the installed explicit-mask image. Then strength0.35/0.18, strength0.50/0.18 and0.35/0.12. Check sun-facing and shaded panels from front and rear. Inspect white decals, black pillars, hood/roof, angular highlight shape, and motion; avoid using global exposure or brightening all paint to hide missing reflected detail.

Existing3bfc masked-road full6500tick/virtual30FPS lap +30motionframes is running with exact established source inputs and1080p output. This is new default-promotion evidence, not original-game parity. Compare four physical exit records against the accepted F427 control (F438 core libraries unchanged). Shared heavy build load may exhaust the bounded900second capture deadline; preserve that failure as resource/timing evidence if it occurs, rather than call it a handling crash.

New source is building with Cargo2jobs. Full resource tests/Clippy queued; no new executable installed. Private local/graphics/lacquer01 contains command scripts/logs. A documentation mismatch was noticed in the candidate help string (still says surfaceOff default): fix that in the final release source before delivery; do not silently label the diagnostic candidate as finished. No final lacquer/default value accepted before GPU controls and movement checks.

## Acceptance remaining

Build exact candidate, actual CLI rejection checks, native legacy parity/source-default equivalence, inspect strength/roughness controls, then rear/motion and RGBA checks. Complete required source checks, source-identical core canonical/quick evidence, exact-path active-process guard, immutable-candidate delivery and rollback. Preserve the primary normal executable and owner's unrelated notes. Keep source/default status distinct from the currently installed3bfc build throughout.

## Installed-runtime road-mask lap acceptance

The existing3bfc runtime with explicitSourceBase completed6500ticks cleanly,1validlap91.208764765s. Four physical exit records match the accepted F427 fixed-clock control exactly after excluding wall_seconds. Source world/tools/formats remain unchanged across the current graphics commits. Thirty requested30FPS images are unique, ticks6202..6262, max frame-time error3.34ns; first/last inspected. This is virtual clock evidence, not monitor FPS or complete temporal-AA acceptance. Source asphalt stays readable; pre-existing raised/black road-edge strips remain outside this material fix. No timeout occurred despite the shared compile load; a speculative resource-deadline concern above is superseded by the clean actual result. Private asphalt-paint-check01/road-mask-full-validation01.json and road-mask-motion-audit01.json preserve exact scope.
