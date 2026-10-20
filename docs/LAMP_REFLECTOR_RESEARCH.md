# Lamp reflector material research

19 October2026; static purchased-resource observations. No lamp appearance change or runtime-variant acceptance yet.

## Separate reflectors, covers and matt parts

The current layered material classifier assigns Head/Rear Lights Glass roughness0.08. Head/Rear Lights Pod and Lights Pod Matt retain the generic roughness0.7 response. Thus a blanket glass-roughness fix addresses the wrong material. All five groups currently bind their diffuse to lights; original static general-bank evidence also supplies separate normal, dust/damage and emissive inputs.

Saved Lacetti constants distinguish the interiors: rear pod EnvironmentColour1 with FresnelMin/Max1; head pod EnvironmentColour1 with Min0.4/Max0.7; matt pod Min/Max0. These are original shader parameters, not metallicness or Bevy roughness. Do not copy their numbers directly into a PBR field. LOD0 vertex audit reports respectively6/2/2/2/2draws for rear pod/rear glass/head glass/matt pod/head pod; all saved words ff00ffff. Evidence remains private under reflection-owner01/lamp-vertex-mask-{source,summary}01.json.

## Linked static shader passes

The installed shaderpack contains group IDs car_light_pod and car_light_glass (without the .fx suffix used by mesh references). The initial exact .fx group search found no match; checking actual group IDs resolved this instead of guessing a pixel shader from a name substring.

- Pod passConfigMask1 links the part-state vertex program to the dust/specular/occlusion/reflective/normal/emissive fragment variant. Its saved pass uses alpha test GREATER0.5, depth test/write and opaque colour blending.
- Glass passConfigMask1 links its part-state vertex program to a separate headlights-glass fragment variant. It uses source-alpha blending and depth write. Reflection-mask32 variants have separate links/culling; these masks are saved configuration, not proof of runtime pass selection.
- Pod has a separate velocity/depth passConfigMask2. Any future visible-material change must preserve its temporal/depth presentation.

Selected pod and glass fragments both sample normal, environment, dust, diffuse crack, emissive and shadow inputs. Pod emission is gated by part state; glass also computes its own alpha response, so the cover must not inherit the opaque pod's authored environment blend. Both decode the original sampled environment via alpha; our direct-HDR local probes must not repeat that decode.

The selected pod vertex program inverts saved red into its normal-control output; its fragment uses that control when reconstructing the tangent-space normal. Under conventional savedARGB-to-RGBA decoding, saved red0 therefore enables this variant's normal response rather than disabling it. This narrows the earlier uncertainty for this statically linked pod variant, but backend channel conversion, runtime vertex changes and active intact/damaged selection remain unobserved. Do not generalize the result to the paint shader or blindly enable full damage normals everywhere.

## Evidence and reproduction

Purchased shaderpack SHA b6b29b80397d12def9c90f5afcff9ea8f335d07da3dc81e293273b468c9029a6. Private lamp-reflectors01/node-{2585,7842}.json preserves group descendants; programs.json preserves the initial inventory. Selected assembly SHA256 values:

| Selection | Assembly SHA256 |
| --- | --- |
| Pod fragment | 921bc327515fee31f70a49a4f69da4c35116b161229bc9da75bb5706c6e3e450 |
| Glass fragment | 41f93f1a3dd3265a86b75217a718f5a9cbd26b771d2d357e66bb7f5d7af623c6 |
| Pod vertex | 36cda6fbf1dd5ad476add322ed07dd0d3aa09a776d04e5e9f71d4e6b11b64ca7 |

Existing tools/disassemble_packed_grid_shader.py and installed Windows D3DDisassemble reused. First three commands used Python -I and failed importing the adjacent disassemble_grid_shader module before outputs; retry without -I succeeds. No tools installed, original resources/project modified or shader listings published. Python3.12.14; installed compiler disassembly header9.22.949.2248.

Next: isolated opt-in reflector-only environment/normal comparison, keeping cover/matt/opponent selection distinct and preserving lamp-state/cracked-glass handling. Keep class in material-cache key if the same base handle can receive different controls. The current delivered paint build remains unchanged.

## Default-off PBR/normal pilot (G185, validation in progress)

The first implemented pilot keeps StandardMaterial for damage compatibility. Four modes separate unchanged, authored PBR response, original normal atlas and combined. PBR head/rear values are respectively roughness0.18/0.24 and metallic0.8/0.55; these are authored trial parameters, not original parameter equivalence. No light emission, cover alpha, opaque/mask mode or matt trim change. Original normal atlas contains actual reflector/lens surface structure; linear upload uses texture_upload::image_linear and existing portable fallback. Frame reconstruction is explicitly approximate: saved orthogonalizable source frames where valid, MikkTSpace fallback only for requested player pod normals. Off/PBR-only removes added lamp tangents so disabled mesh pipeline layout is preserved.

PostStartup player-only material cloning avoids accidentally sharing new response with opponents using the same model/base handle. Cache key includes head/rear class; damage still sees a standard material, and its cracked diffuse swap clones all remaining properties/reset stores the actual original. Scope regression includes a marked opponent sharing the same source handle plus both head/rear classes and unchanged glass. First compile name-alias mistake and first missing-frame resource rejection retained; intermediate fallback test passes, final-source full validation and GPU inspection required before promotion. No original shader source or new custom shader included.

## Validated9e160be pilot and limits (20 October2026)

Frozen source9e160be7883dbf7e055c8947a334eb0ffe26502b, private executable SHA b1b4d3e0071069575e5d4779c992f37ed187dd4b14b663451d70ce5b1c4083ff. Release build6m56s with one Cargo job;773resource tests pass,0failed/ignored across12reported harness results. Feature/default workspace releaseClippy pass. CanonicalF455 matches6d7184ac5d5e416d8849a2c382f1042ef3c17719cedbbe2324f666d3416f0063 and quick24scenarios/120000ticks pass using archived tools with current core/Cargo.lock byte-unchanged proof, not newly compiled current tools.

Eight native1920x1080 SMAA static cases isolate off/PBR/normals/combined at front and rear views with a real moving1024HDRprobe. All pass; all eight pod materials attach, only normal/combined uploads the linear Bc1RgbaUnorm map through the shared helper. Full frames and native-pixel crops inspected: front reflector structure becomes more legible, rear lower lens has additional ribs, but rear clusters remain visibly flat and cover/alpha/source shader reconstruction is unfinished. This is an authored material improvement, not recovered1:1 optics or owner live acceptance. Enabled captures also differ outside the lamp crops; pixel-scope01.json records full-frame bounds, so no global outside-lamp pixel-invariance claim is made. ECS scope proves material selection separately.

Off static SMAA/moving-probe PNG is byte-identical to delivered9abb921 (mean/max0,0changedpixels), limited to this pose. Four600tick nativeDLAA cases off/combined at virtual30/300FPS have four equal physical exit records; sampled motion/depth/header arrays are byte-identical at both clocks (maximum0). VirtualFPS is not display performance and these samples do not settle all temporal/collision/damage behaviour. Native3840x2160/DLAA forcedRGBA8 succeeds with zeroBC assets; desktop fallback, not phone hardware or BC pixel equivalence. Antenna/asphalt temporal issues stay open.

All eight saved lamp frames fail the tread's strict orthogonality contract. Read-only LOD0 frame-audit01.json distinguishes12degenerate saved rear vectors and skew in valid vectors (minimum absolute binormal alignment roughly0.00155for head pods). This does not prove original rendering invalid: the original can use a non-orthogonal saved basis directly. Actual pilot uses MikkTSpace for all eight pods and logs each fallback. Original lens covers save NormalMapScale0 versus pod1; runtime parameter/vertex controls remain unresolved, reinforcing the decision to keep cover normals separate.

Private lamp-reflectors01/{checks01,pixel-scope01}.json, static01/controls.json, motion01/{controls,parity}.json, rgba4k01/validation.json, off-compat01/validation.json preserve evidence. Four negative CLI cases pass with valid argument names; first attempt used nonexistent graphics mode legacy, so it stopped at the graphics parser rather than testing the lamp guard. Failure retained; retry uses baseline, plus wrong model/LOD and invalid mode. Rust1.99.0/Bevy0.19.1/wgpu29.0.4; no new tools or original writes.

Separate Play GRID Graphics Lamps DLAA.cmd selects combined plus the already established paintgain0.5/moving1024probe. Immutable local/graphics/lamps-9e160be contains matching runtime/full NVIDIA license/notices/guide/build manifest. Previous normal/DLAA/reflection launchers and binaries, plus peer note, are hash-guarded unchanged. StandardMaterial remains visible to damage/reset, but no new destroyed-lamp visual acceptance is claimed from these undamaged movement cases. Next: investigate rear lens appearance and original alpha/part-state response without faking constant emission; preserve current accepted paint build independently.

Exact installed primary600tick nativeDLAA smoke passes with eight selected pods, linear normal upload, movingHDRprobe/GPU readback and temporal audit without SDK environment. Evidence C:/Dev/Reassembly/local/graphics/lamp-delivery-smoke01/validation.json.

## Rear cover follow-up: source alpha and control linkage (20 October2026)

Further read-only research keeps the delivered9e160be pilot unchanged. The glass part-state vertex program is byte-identical to the pod vertex program (binary SHA77d0aea13dda09bbf20eba8437651c484a426ad17ca9a9a9a3c8ce1969ceb246; assembly36cda6fb…b64ca7). This proves shared selected code, not active runtime variant/vertex state.

Actual Ghidra xrefs of NormalMapScale reach the generic car feature selector61b2e0, a second selector61b730 and material replacement96e430. In61b2e0, missing TNormalMap or scale<=approximately0.0001 clears feature bit0x2; its table chooses generic car_di_* names. Important boundary: this establishes a generic selector, not that the dedicated car_light_glass group goes through it. Therefore the saved lens scale0 alone is not proof that lens normals are inactive. Earlier commentary's selector observation must be read with this limitation. Do not apply that hypothesis as a new global material rule.

GhostCarValues has a static registration/initialization patha14540 supplying(1,1,1,0), plus a distinct all-ones registration. This is not a captured live draw uniform and must not be assumed universal. PartStateDefault writer901f10 forwards its caller's scalar along with the Parts/PartsState vectors; it is not a constant emission-on default. These findings explain why correct original cover colour/alpha requires actual state/control linkage, not only diffuse/roughness. CPU/fragment alpha formulas still cannot be transplanted with guessed controls. Private control-refs01/{address-refs,contexts}.tsv, control-pack01/three containing functions, selector-strings01.json preserve actual evidence. Pinned private PE remainsaac46…4514; Ghidra12.1.4 read-only/noanalysis,768MiB/oneCPU, existing project/runtime, no install.

Pixel-ray investigation of the rear comparison finds Rear Lights Glass first and a rear pod behind it at both white/red sample points. White cover distance2.57746m versus pod2.62895m; red2.61945m versus pod2.70694m. This is CPU geometry identification, not proof of GPU visibility, transparency or exact coverage. Interpolating the actual selected triangle's source UV and decoding the base BC3 atlas independently gives approximatelyRGBA(252,253,252,42) at white UV(0.92881,0.47862), and(236,26,35,112) at red UV(0.94413,0.63830). Texture size1024x512, general-bank SHA8bf605b1afbfcd8ca93581a7f5181c30086fe7a2f5016492af273c13453e277e. This is a base-level bilinear sample, not actual GPU mip/filter/lighting output.

Thus the white cover region is intentionally nearly plain white in the purchased diffuse atlas with partial alpha, rather than proven missing texture. Pod-only changes cannot fully settle its appearance. Do not arbitrarily multiply opacity or add constant glow: the original combines source alpha, reflection/specular and state controls. Next candidate is a separately isolated lens response comparison, using these source UV/alpha observations and explicit active-variant uncertainty; current opt-in pod improvement remains available. Private layer-rays01/rays.json, cover-alpha-audit01.json, source-lights-base01.png and reproducible private scripts retain evidence. Original shader listings and atlas remain ignored/private.
