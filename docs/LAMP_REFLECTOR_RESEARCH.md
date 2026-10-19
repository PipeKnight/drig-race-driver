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
