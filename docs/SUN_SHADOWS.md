# Sun shadows: the original's cascades and the port's

How the original renders its sun shadows (cascade count, map size, caster selection) and how the port's Bevy cascades differ, 30 October 2026. Static analysis of the unpacked executable (`grid_re.py lookup|xrefs|dis|decompile`, master Ghidra project read-only) and the shipped `system/hardware_settings_*.xml`; no runtime trace of the original. Labels: [O] observed in the original (data or code), [I] inferred, [?] open. Costs and the port's caster culling: docs/PERFORMANCE.md "Sun shadow cost (F652)".

## 1. Short answer

- [O] The original's CSM plugin (`CMR6PluginCSMShadows`, init 0x5CC6D0) renders **three depth maps** (`depthMap0..2`, CSM +0x638/+0x63C/+0x640), all of one size, then a screen-space mask pass and a smoothing pass. The world (track ornaments, trees, characters, opponents) casts only into **cascade passes 1 and 2**; cascade 0 takes no world caster (docs/CAR_LOD.md "Cascades (F616)").
- [O] Map size: `<shadows size=...>` from the hardware settings, clamped to 128..8192, 1024 when the key is missing (0x601B30 -> 0x5B43E0). The shipped options: off and low `enabled="false"`; **medium 1024**, `maskQuality="1"`; **high 2048**, `maskQuality="2"`; the shipped `hardware_settings_config.xml` has 1024.
- [?] The cascade ranges are shader parameters (`shadowRange0..2`, `shadowFade0..2`); their values are not traced.
- The port draws **four world cascades** with Bevy's default bounds (10, 24.7, 61.2, 150 m) at 2048 (the enhanced desktop profile: first bound 18 m, 4096). Every cascade takes the world's casters.

## 2. Original

### 2.1 Plugin init 0x5CC6D0 [O]

- Called once from `hardware_settings_read_graphics` (0x601B30). Sets the enable byte (plugin +0x62C) from `<shadows enabled>`; CSM +0x664/+0x668 (map width, height) = 0x400.
- Creates two render passes named "Shadow Cascade" with cascade numbers 1 and 2 (loop `local_24 = 1..2`), callback 0x5C02D0 per pass.
- Looks up the caster plugins: `OrnamentsPlugin`, `TreesPlugin`, `InCarCharactersPlugin`, `VehicleManagerPlugin`, `OrganismPlugin`, `FERenderPlugin`, `ParticleManagerPlugin`, `TrackManagerPlugin`.
- Three caster lists of 0x800 (2,048) entries each (plugin +0x554..+0x55C).

### 2.2 Map size 0x601B30 -> 0x5B43E0 [O]

`hardware_settings_read_graphics` reads `shadows size` into renderer +0xF48 and clamps it to 128..8192 (0x602137..0x602154); a missing key gives 0x400. (The next key, `reflections size` at +0xF3C, is clamped to 32..8192 with 256 as its default.) It then calls 0x5B43E0 on renderer +0xC0 (the CSM owner) with the size as width and height: `[ecx+0x664] = w; [ecx+0x668] = h`. 0x5BA1D0 (`csm_bind_depth_and_mask_resources`) creates the three depth maps from +0x664/+0x668 (0x5BA280..0x5BA322).

### 2.3 Cascade pass callback 0x5C02D0 [O]

For cascade `n > 0` with a pass object at CSM +0x38 + 4n, and unless CSM +0xDD8 == 3, it submits with pass code 0x401:
- the track/world list through 0x95FE90;
- vehicles (0x9CB5B0): opponents with their coarsest level (docs/CAR_LOD.md "Shadows");
- characters (0x85CC80);
- ornaments (0x979430, plus a second list for cascade 1 only; 0x979590): level 1 (docs/TRACK_LOD.md section 4);
- trees (0x97A160, 0x97A250).

Each list comes from that cascade pass's own visibility result (pass +0x170), so the original culls casters per cascade by the pass's own view [I: the cull volume of the pass is not decoded].

### 2.4 Mask and smoothing 0x5C04C0 [O]

Binds the shader parameters: `screenDepthMap`, `depthMap0..2` and their inverse sizes, `shadowSamplePos0..3`, `clip2depth0..2`, `shadowRange0..2`, `shadowFade0..2`, `softShadowTechnique`, `discScales`, `pointOfInterest`, the mask shader `csm_wedge_shadow` and the smoothing shader `csm_smooth` (four equal taps, docs/SOURCE_SKY_LIGHTING.md). The final mask is bound to materials as `TShadowMask`.

## 3. Port

| | original | port (baseline preset) | port (enhanced desktop) |
|---|---|---|---|
| maps | 3, 1024 (medium) / 2048 (high) | 4, 2048 | 4, 4096 |
| world casters | cascades 1 and 2 | all four | all four |
| ranges | `shadowRange0..2` [?] | 0.1-10, 8-24.7, 19.7-61.2, 49-150 m (20 % overlap) | first 18 m to the profile's shadow distance |
| sampling | screen-space mask, 4-tap smooth | per-fragment Gaussian PCF | per-fragment Gaussian PCF |

A faithful layout (two world cascades and one map for the player's car, at the option's size) would cut the shadow passes by about half on every device, but it changes the picture: it needs the owner's decision and the cascade ranges traced first. Not changed.

### 3.1 Caster culling by receiver (F652)

`crates/grid-app/src/shadow_cull.rs`. Bevy fits cascade `i` to the camera slice from `(1 - overlap) * far(i-1)` (the first from `minimum_distance`) to `far(i)` and samples it only for fragments in that slice (cascade selection and blending by view depth). It keeps a caster whose box meets the cascade's light-space square; the square is fitted to the slice's diagonal, so it also takes casters beside and behind the camera. After Bevy's caster visibility, the port drops a caster from a cascade when one face of that slice, whose normal does not point along the light, has the caster's box wholly outside by more than 10 texels of the cascade. Such a caster's shadow, swept along the light, cannot reach a fragment that samples the cascade, so the picture is bit-identical. `GRID_SHADOW_CULL=0` keeps Bevy's lists.

## 4. Open

- `shadowRange0..2` / `shadowFade0..2` values and who sets them per frame.
- What cascade 0 draws (0x5CCDC0, the focus car's `car_layer` maps [I]).
- The cull volume of a cascade pass's visibility query (0x5C02D0 reads pass +0x170).

## Addresses

| address | role |
|---|---|
| 0x5CC6D0 | CSM plugin init: two "Shadow Cascade" passes, caster plugins, map size 0x400 |
| 0x5B43E0 | CSM map size setter (+0x664 width, +0x668 height) |
| 0x601B30 | `hardware_settings_read_graphics`: `shadows size` (128..8192, default 1024) -> 0x5B43E0 |
| 0x5BA1D0 | creates `depthMap0..2` at +0x664 x +0x668 and binds the mask |
| 0x5C02D0 | cascade pass callback: world, vehicles, characters, ornaments, trees with code 0x401 |
| 0x5C04C0 | shader parameter binding (`depthMap0..2`, `shadowRange0..2`, `csm_wedge_shadow`, `csm_smooth`) |
