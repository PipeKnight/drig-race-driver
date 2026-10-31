# Sun shadows: the original's cascades and the port's

How the original renders its sun shadows (cascade count, map size, caster selection) and how the port's Bevy cascades differ, 30 October 2026. Static analysis of the unpacked executable (`grid_re.py lookup|xrefs|dis|decompile`, master Ghidra project read-only) and the shipped `system/hardware_settings_*.xml`; no runtime trace of the original. Labels: [O] observed in the original (data or code), [I] inferred, [?] open. Costs and the port's caster culling: docs/PERFORMANCE.md "Sun shadow cost (F652)".

## 1. Short answer

- [O] The original's CSM plugin (`CMR6PluginCSMShadows`, init 0x5CC6D0) renders **three depth maps** (`depthMap0..2`, CSM +0x638/+0x63C/+0x640), all of one size, then a screen-space mask pass and a smoothing pass. The world (track ornaments, trees, characters, opponents) casts only into **cascade passes 1 and 2**; cascade 0 takes no world caster (docs/CAR_LOD.md "Cascades (F616)").
- [O] Map size: `<shadows size=...>` from the hardware settings, clamped to 128..8192, 1024 when the key is missing (0x601B30 -> 0x5B43E0). The shipped options: off and low `enabled="false"`; **medium 1024**, `maskQuality="1"`; **high 2048**, `maskQuality="2"`; the shipped `hardware_settings_config.xml` has 1024.
- [O] The ranges come from a per-view-mode table, not from XML (2.5). Outside views: cascade 1 from 0.4 to 76 m, cascade 2 from 59.9 to 252 m fading out from 117.5 m, cascade 0 around a point near the camera (the player car [I]) while the camera is within 50 m of it.
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

### 2.5 Cascade ranges per view mode [O]

The split distances are not XML values: they come from a table of four 0x90-byte entries at CSM +0xB98, one per view mode, indexed by CSM +0xDD8.

- Defaults for all four entries: 0x5B4990 (called four times from the plugin init, loop at 0x5CC951). Then 0x5B4620 rewrites mode 1, 0x5B4720 mode 2, 0x5B4860 mode 3.
- The mode: `camera_model_visibility_policy` (0x607BC0) sets 1 while the cockpit interior is shown (`VehicleInteriorPlugin` +0x50) and 0 otherwise; 0x4CD100 sets 2 (front end [I]); 3 skips the cascade passes (0x5C02D0).
- An entry holds the near distance (+0x20, 0.4 m), cascade 0's far distance (+0x1C, 1600 m) and one 0x1C-byte block per cascade at +0x38, +0x54, +0x70: [0] range, [2] far extension, [5] fade fraction (other fields [?]: bias-like values).

| mode 0 (outside views) | block [0], [2], [5] | near | far | fade |
|---|---|---|---|---|
| cascade 0 | 250, 3.75, 0 | 0.4 | 1600 | none; switched by the camera's distance, below |
| cascade 1 | 75, 1.0, 0.2 | 0.4 | 76 | 60.9 to 76 m |
| cascade 2 | 250, 2.0, 0.7 | 59.9 | 252 | 117.5 to 252 m |

- Split setup 0x5DABC0 (loop at 0x5DB4A4, per-cascade structure CSM + 0xE0 + 0x160 i, near at +0xD0, far at +0xD8): cascade 0 from the entry's near to +0x1C; cascade `i > 0` far = range + extension; cascade 2's near = cascade 1's range minus cascade 1's fade share, 75 - 0.2 (76 - 0.4) = 59.88 m.
- Fade (0x5CD5F0, `shadowFade0..2`): with the flag CSM +0xE0C (set to 1 at init, no other writer found) a world cascade fades out in squared camera distance from `near + (1 - fraction)(far - near)` to `far`.
- Mode 1 (cockpit, 0x5B4620) keeps the ranges; cascade 1's fade fraction is 0.27, cascade 2's extension 3.0, cascade 0's extension 0.8.
- Cascade 0 is centred on a point the frame supplies (cascade structure +0x150, CSM +0x230) and serves receivers whose camera distance lies in `max(d - 10, 0)` to that plus 20 m, d being the camera's distance from that point; it is switched off when d exceeds 50 m (`2500 = 50^2`). The world casts only into cascades 1 and 2 (2.3); what draws into cascade 0 is not traced. 0x5CCDC0 draws the vehicles and characters of the cascade 1 and 2 passes' lists through the layers `car_layer1` and `car_layer2`, so it is not a cascade-0 map. The port takes cascade 0 as the player car's map [I].
- `(block[2] - block[1]) / block[0] / 2` per cascade is `discScales` (2.6), not `shadowRange`: the loop stores it at +0x80 of each cascade's 0x90-byte record (0x5CE9BB), and 0x5CF606 copies the three into `discScales` (CSM +0x750). Block [1] is 0.6, 0.5, 0.6 (0x5B4990).

### 2.6 Mask filter [O]

The mask pass draws one screen-space visibility value per mask pixel (`csm_wedge_shadow`); `csm_smooth` then filters the mask, and materials read it as `TShadowMask` (2.4).

- `maskQuality` (hardware_settings_options.xml: medium 1, high 2; 0x72ECD0 reads it, default 2) adds the pass-config key `shadowMaskQuality0..2` (0xC798A0..0xC798E1). The shader group `csm_wedge_shadow` has three passes: `_1` with passConfigMask 8, `_0`, `_1` [I: key n selects pass n].
- `csm_wedge_shadow_1` (high): the car map (`depthMap0`) takes 8 taps, the centre and seven Poisson points (shader constants, radius up to 1.27); cascades 1 and 2 take 4 taps each (`shadowSamplePos0` and `1`, 0xDAFC38: radius up to 0.64). Each tap is one point sample compared with the receiver's depth (no hardware filter), weighted equally. Cascades blend by `shadowFade1`, the last fades by `shadowFade2`, and the result is the darker of car and world.
- `csm_wedge_shadow_0` (medium): the same 8 car taps; one centre tap per world cascade.
- Tap offsets: `discScales[i] * (sin a, cos a) * p` for the car, `discScales[i] * ((sin a + cos a) p.x, (cos a - sin a) p.y)` for the world, a per-axis scaling by the pixel's angle `a`, not a rotation. The vertex shader gives `a = rotationScale.x x + rotationScale.y y` (clip x, y); `rotationScale = (0.178 pi W, 0.3 pi H)` for a W x H mask (entry +0x28, +0x2C; 0x5CF4A1), so `a` steps 0.356 pi along x and 0.6 pi along y per mask pixel.
- `discScales` (2.5): `(block[2] - block[1]) / range / 2`: car 0.0063, cascade 1 0.0033, cascade 2 0.0028 in map coordinates. If a world cascade's map spans 2 x range [I: the decoded clip planes scale by 1 / range], the disc radius is `block[2] - block[1]` in metres: 0.5 m for cascade 1 and 1.4 m for cascade 2, at any map size. The car map's extent is not decoded; 0.0063 of it.
- `csm_smooth`: four equal taps at (+-k, +-k) mask pixels, `k` = entry +0x24 (0.5, `TexBias0/1`, 0x5CF8C3), each a bilinear read of the mask: a 1-2-1 tent over the 3 x 3 neighbours [I: bilinear mask sampling], whose angles differ, so the 4 or 8 taps become 36 or 72 per smoothed pixel.

The original therefore softens by a world-sized disc and a screen-space tent, not by the map's texel size: medium and high differ in the world taps, not in the blur.

## 3. Port

| | original | port (baseline preset) | port (enhanced desktop) |
|---|---|---|---|
| maps | 3, 1024 (medium) / 2048 (high) | 4, 2048 | 4, 4096 |
| world casters | cascades 1 and 2 | all four | all four |
| ranges | `shadowRange0..2` [?] | 0.1-10, 8-24.7, 19.7-61.2, 49-150 m (20 % overlap) | first 18 m to the profile's shadow distance |
| sampling | screen-space mask, 4-tap smooth (2.6) | per-fragment Gaussian PCF | per-fragment Gaussian PCF |

The port now uses the original's layout by default (3.2); the table's "port" columns describe `GRID_SHADOW_LAYOUT=bevy`.

### 3.1 Caster culling by receiver (F652)

`crates/grid-app/src/shadow_cull.rs`. Bevy fits cascade `i` to the camera slice from `(1 - overlap) * far(i-1)` (the first from `minimum_distance`) to `far(i)` and samples it only for fragments in that slice (cascade selection and blending by view depth). It keeps a caster whose box meets the cascade's light-space square; the square is fitted to the slice's diagonal, so it also takes casters beside and behind the camera. After Bevy's caster visibility, the port drops a caster from a cascade when one face of that slice, whose normal does not point along the light, has the caster's box wholly outside by more than 10 texels of the cascade. Such a caster's shadow, swept along the light, cannot reach a fragment that samples the cascade, so the picture is bit-identical. `GRID_SHADOW_CULL=0` keeps Bevy's lists.

### 3.2 The original's layout (owner's decision, 2026-10-30)

`crates/grid-app/src/shadow_layout.rs`, on by default; `GRID_SHADOW_LAYOUT=bevy` keeps the four Bevy cascades above (comparisons).

- Three maps of one size from `[video] shadows`: off (no shadow maps), low 512, medium 1024, high 2048 (the desktop default), ultra 4096; the mobile preset (Android) defaults to low (owner, 2026-10-31: an exception to the preset's resolution-only cut), a saved option wins. Off, medium and high are the original's options; the original's low has no shadows, and low 512 and ultra 4096 are the owner's extensions.
- Cascades 1 and 2 are the world's, split by view depth at the mode-0 values of 2.5: 0.4 to 76 m and 60.8 to 252 m (Bevy's blend over the 20 % overlap stands for cascade 1's fade, 60.9 to 76 m in the original). The last cascade fades out from 30 % of its range (118 m) to 252 m in squared view depth, as `shadowFade` does in squared camera distance [I: depth for distance]. With the enhanced profile the world reach is its shadow distance plus 2 m (252 m by default).
- Cascade 0 is the player car's map [I, 2.5]: an orthographic box around the player car's parts (`car0/` source parts and their hulls, as the bumper view's filter finds them) in whole metres plus 0.5 m, texel-snapped, reaching 50 m below the car along the light; only those parts cast into it. It is off while the viewing camera is more than 50 m from the car (the original's switch).
- Shading: an in-memory patch of Bevy 0.19.1's `fetch_directional_shadow` (`bevy_pbr::shadows`, like `clearcoat_prepass`) takes the darker of the car map and the world cascades. The car map reads lit outside its box, so it only adds the car's sharp shadow and self-shadow near the car. The original's receiver shell (camera distance within 10 m before and 10 m beyond the point) is replaced by the box.
- Filter (2.6), per fragment instead of a screen mask: high, low and ultra take the `_1` kernel, medium the `_0` one. The world disc radius is 0.5 m (cascade 1) and 1.4 m (cascade 2), converted to each Bevy cascade's map coordinates by its texel size, so the softness does not change with the map size; the car disc is 0.0063 of the car map. The smooth is evaluated as nine kernels at the neighbours' angles with the tent's weights. Differences: each tap is Bevy's 2x2 hardware comparison instead of a point compare (smoother within a texel), and the smooth does not mix the neighbours' positions (under a pixel's footprint). `GRID_SHADOW_FILTER=bevy` keeps Bevy's Gaussian PCF and `GRID_SHADOW_SOFT` scales the radii (research switches).
- Not ported: the mode-1 (cockpit) values (fade 0.27, extensions), the bias-like block fields.
- The performance test's `light_shadows` switch rebuilds the cascade bounds with Bevy's builder (6 to 60 m); with this layout the world cascades then start at 4.8 m. It is a diagnostic switch only.

## 4. Open

- What draws into cascade 0, and the point it is centred on (CSM +0x230).
- The remaining fields of a cascade block, `shadowRange0..2`, the world cascades' map extent (2.6 assumes 2 x range), the car map's extent, the mask's size and sampler state, and which pass each `shadowMaskQuality` key selects.
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
| 0x5B4990 | cascade mode-table defaults (one 0x90-byte entry) |
| 0x5B4620 | cascade mode table: mode 1 (cockpit) values |
| 0x5B4720 | cascade mode table: mode 2 values |
| 0x5B4860 | cascade mode table: mode 3 values |
| 0x5DABC0 | per-frame cascade split setup (near +0xD0, far +0xD8 per cascade) |
| 0x607BC0 | sets the cascade mode CSM +0xDD8: 1 with the cockpit interior, else 0 |
| 0x72ECD0 | reads `maskQuality` (default 2) and adds pass-config key `shadowMaskQuality0..2` |
| 0x5CE9BB | per-cascade disc scale `(block[2] - block[1]) / range / 2` |
| 0x5CF606 | copies the three disc scales into `discScales` (CSM +0x750) |
| 0x5CF8C3 | `csm_smooth` tap offsets `TexBias0/1` = +-k / mask size |
| 0xDAFC38 | `shadowSamplePos0..3` (4 x float4 Poisson points) |
