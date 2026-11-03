# Source road material response contracts

Static source and metadata findings for shared road response and lighting inputs; no appearance change or complete material port.

3 November 2026, G354. Original files, disassembly and numeric captures remain private under ignored `local/`.

## Why family or filenames cannot choose a terrain response

The current `track_surface` path admits six shared surface/infield families. Its dry-ground roughness/reflectance rule is authored; an infield family does not prove grass or soil. In retained Detroit metadata, concrete-, grass- and tarmac-named land materials share the same saved response tuple. Other infield presets overlap ordinary surface presets, and textures occur in both families.

These are representative counterexamples, not a new Detroit-specific rule. Neither shader family, a texture-name list nor the inspected scalar presets establish a general ground semantic. Physical `MECHANICS.roughness` belongs to contact simulation; no render/physical material join was recovered here.

## Separate filtered alpha from RGB and physical roughness

The existing host/shader already retains the filtered alpha of the two diffuse layers, mixed with the saved vertex-red layer weight. RGB filtering and skid colour construction do not replace that alpha. The selected original direct-specular programs use mixed diffuse alpha to weight a Phong term alongside runtime specular colour, Fresnel, macro AO and shadow.

That alpha is not opacity, roughness or a direct GGX conversion. The current SourceBase control adapts Bevy base reflectance using the mask; the default Off control omits that adaptation. Existing dry-ground and grazing controls remain authored choices, not recovered source response. Other reflective/normal-mapped variants require their own program/binder evidence.

| Input | Current shared path | Evidence boundary |
|---|---|---|
| Diffuse layers and alpha | Surface textures100/102 and mixed layer alpha | Actual filtered mask survives independently of RGB |
| Macro/skid inputs | Existing texture104/106 and shared controls | Source runtime AO scale remains open |
| PhongPower, SpecScale, PhongColour, Fresnel fields, EnvironmentColour | Constants decoded but response fields not retained/uploaded by Definition | Saved fields do not establish their runtime composition |
| Dry-ground response | Authored roughness1/reflectance.25 eligibility | Not a semantic classifier or legacy conversion |
| Optional grazing gain | Guarded authored F0 gain | Not the original Fresnel parameter contract |

## Concrete runtime specular-colour publisher

The selected named publishers at `0x00743220` (case3) and `0x00743750` copy RGB from a lighting object field+0x1E0, append W=1 and submit `specularColour`. The reader `0x0072BB40` targets the same field under lighting XML `specular_colour`; the existing colour pack/reconstruction path connects packed+0x310 back to that RGB field in `0x00730F90`.

This establishes a lighting-object input path. It does not prove an exclusive writer, the selected road pass's active GPU state, or a formula combining saved material SpecScale and PhongColour. Those emitter blocks do not perform that multiplication. An exact-name negative search is not proof no hashed/indirect binder exists. The remaining PhongColour property-lookup leads have no demonstrated road consumer and remain explicitly unproved.

EnvironmentColour is a different saved value. No EnvMapScale formula was recovered from the selected inputs/programs. Reflections, probes, solar shadows and paint response are outside this change.

## Next shared implementation boundary

Establish the actual binder, then retain saved response fields and exact filtered/mixed-alpha provenance alongside their first meaningful consumer. A metadata-only production patch would currently add unused state; the existing alpha already reaches its consumer. Keep unknowns visible and avoid a guessed global roughness, a longer per-track name list or a contact-roughness shortcut. No material, shader, default or optical-parity change is made by these findings.

The current style of authored PBR approximation and the original Phong response are distinct contracts. Rendered representative validation is still required for any future response change.

Private exact program/host-binding audit and cached-material counterexamples: `local/handoff/drafts/road-material-response50/`. Positive lighting publisher, reader and bounded negative binder evidence: `local/handoff/drafts/road-specular-binder51/`. Both reuse the pinned PE, shared master/cache and existing tools; no new original process, GPU run or decompiler project was needed.

## Baseline port of the direct specular (2026-11-03, session fervent-napier)

Asked for by the coordinator after F716: under a low sun (Haruna route 0, sun 22 degrees up) the
baseline road went white. Bevy's GGX (roughness 0.7, F0 4%) stood in for the original's lobe.

### The programs [V static]

`tools/disassemble_packed_grid_shader.py` on `shaderpack.pssg`, the six groups `track_surface.rs`
draws (private: `local/r41/track_surface_simple.cg.fp.asm`, `local/r47/*.cg.fp.asm` in the
fervent-napier worktree). With `I` the normalised eye-to-point direction and `N` the normal:

- `t = saturate(pow(1 + dot(I, N), FresnelPower))`, `fresnel = lerp(FresnelMin, FresnelMax, t)`;
- `R = I - 2 dot(I, N) N`, `lobe = pow(saturate(dot(R, specularDirection)), PhongPower)`;
- `specular = lobe * specularColour * a * fresnel * ao * 4 * shadow`, where `a` is the mixed
  diffuse alpha (the two `TDiffuseAlphaMap` layers by vertex red), `ao = 1 + aoScale.x (macro.a - 1)`
  and `shadow = 1 + shadowStrength (mask.r - 1)`, the same shadow factor the sun's diffuse takes;
- colour `= albedo * (sun sat(N.L) shadow + back (1 - N.L)/2 + ambient) * ao + specular`, then
  the screen AO mask and fog.

`track_surface_simple`, `track_surface`, `track_infield` and `track_infield_dxt5nm` share this
(the last two sample a normal map first; the port has none). `track_infield_wsm` uses `FresnelMax`
alone (no Fresnel curve). `track_surface_reflective` has no Phong lobe: Fresnel weights a
`ReflectionMap`, not ported; the port keeps Bevy's specular on it as the stand-in (59 of
Shibuya's 103 road materials).

Every material saves `PhongPower`, `FresnelMin`, `FresnelMax` and `FresnelPower` (Haruna's road:
5, 0.5, 1, 2; its walls 10, 0, 1, 1). `SpecScale`, `PhongColour` and `EnvironmentColour` are saved
too but these programs take none of them. `specularColour` is lighting.xml `specular_colour`
(the publisher above). Haruna's night lighting files (route 1 and others) save no
`specular_colour`: the lighting constructor's defaults (`0x007411E0`) set +0x1E0..+0x1E8 to
1, 1, 1, and the reader (`0x0072BB40`) writes the field only when the key is present, so the
port takes white [V static; a later writer is not excluded]. `specularDirection` is not traced; lighting.xml saves `spec_linked`
(1 on Haruna) with `spec_heightangle`/`spec_rotationangle`, read here as: linked means the sun's
direction [I].

### Port

`track_surface.wgsl` `source_specular`, on the baseline preset where the F716 fill applies
(`graphics::Phong`, `GRID_BASELINE_SPECULAR=source`, the default; `bevy` keeps Bevy's for review):

- the lobe above, added after Bevy's lighting; the material's reflectance is set to 0, which
  removes Bevy's direct, F90 and ambient specular;
- `specularColour` in the fill's units: one unit = `backlight_suncolour`'s peak = the baseline
  sun's 18,000 lux, divided by pi (Bevy's Lambert); linear like the fill;
- the shadow is the sun's shadow map (the brightest directional light); `shadow_strength` is not
  applied, as for the diffuse;
- `ao` is the port's macro AO at scale 1 (the AO scale uniform is not recovered).

AUTHORED / open: the enhanced preset keeps its GGX response; `min_specular_power`,
`max_specular_power` and `max_specular_scale` in lighting.xml have no traced consumer here; the
reflective group's reflection map and the normal-mapped groups' normals are not ported.
