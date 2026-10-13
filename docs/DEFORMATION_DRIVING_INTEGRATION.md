# Collision-driven deformation in the source prototype

13 October2026, F267. The source single-car world now owns both verified physical deformation registrations: preparation before solving; bulk deformation/response after successful solving and before optional correction/final publication. The next car job consumes dirty geometry/wheels/views after the driver and before handling, then advanced-pose contact queries use the rebuilt actual collision shape.

`deformation_world::Registration` owns callback parameters, car state, a separate CRT seed and diagnostic counters. The source context retains C7F8=D240; the adapter checks that role, supplies the owned settings prefix, canonicalizes it only inside the verified callback boundary and restores it. One-based physical owner1 becomes the explicit canonical first role; absent static peer and original byteC are preserved. Only consumed contact38/3C/40/44 changes are merged before normal manager metadata publication. Existing grouping/raw pointer guards stay unchanged. Multiple dynamic cars remain unsupported.

The complete world, callback owner/seed, source query cache and external BodyGeometry are staged. A late callback or body-event failure commits none of them. The global body-event RNG and CRT seed are independent; reset restores both streams and requests source geometry reconstruction before the next query.

## Purchased inputs and verification

DF01 privately stores252 bytes of initial mapped parameter data extracted read-only from the purchased executable, SHA`dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f`. Each source address/file-byte/zero-fill extent is recorded in `local/deformation-parameters.json`. The reader checks framing, finite values, flags/iterations and aliased globals shared by both phases. File SHA`5d48e35d630315b50a7e07450501e6e90d43311a8552bc98367c3e69f9c93292`. These are initial defaults, not observed live globals; the prototype explicitly starts its owned CRT stream at1.

Both debug and release source tests pass **600 driving ticks +600 exact replay ticks**, with no authored grid changes:292 body contacts,124 bulk invocations and166 generated records.334 XYZ node positions move, maximum displacement.28555748 in game units. This demonstrates collision-driven grid production in the composed source loop, not exclusive attribution to particular crashes or a calibrated match to an original-game lap. The CRT seed remains1 in this particular run; whole original-frame tests separately exercise seed changes. Reset reproduces complete owner/physical/event RNG/world contacts/rows/workspace/heads, callback counters/seed and cache exactly.

A deliberately inconsistent aliased parameter fails inside the physical phase after driver/publication/query work. Actual geometry contacts, car/physical/world/rows/cache/tick and CRT seed remain unchanged. The independent supplied-grid test remains separate.200 ordinary libraries, formatting, all-target Clippy and Python syntax checks pass. Five Bevy tests pass: baseline wheel presentation; normal and body-contact keyboard recovery; deformation keyboard recovery; deformation wheel pivots/finite unit-determinant transforms/no feedback/reset after600 ticks. The earlier complete original static-role proof is in [DEFORMATION_STATIC_ROLES.md](DEFORMATION_STATIC_ROLES.md); complete source factory/live cadence equivalence is not claimed.

## Playable selection

`--deformation PRIVATE_DF01` requires driving and body-contact inputs. The Body Contacts launcher selects it when the local DF01/BP01 files exist; both launchers keep using the same current release executable. The base launcher preserves its plain driving mode. `DAMAGE` marks the selected window mode. Wheels use verified damage translation/orientation with an explicitly authored render composition around source pivots; visible **body panels remain rigid**. Lap timing, full graphical deformation, other cars, original allocations and asynchronous scheduling remain open.

`drive_deformation_sample` records phase counters, CRT seed, mode, pending geometry and moved-node/max-displacement observations. Log summaries retain these separately from body-event RNG and geometric contact counts; counters are not crash/RNG-draw counts. Periodic build validation now includes DF01 provenance and both new source/Bevy routes. A clean executable refresh and headless render/stress validation follow this source commit; the earlierdf108 build remains current until that refresh is verified.

## Private evidence and failures

| Evidence | SHA-256 |
| --- | --- |
| local/behavior/source-deformation-callbacks-debug-20261013-final-01.txt | 0e2b99f750ee32da17188e5e9cbf2fa9fa69806a64ebeb2f039e986736bdf113 |
| local/behavior/source-deformation-callbacks-release-20261013-final-01.txt | e8f71197089dc164338d8837cfa0ef2e917b4f433769c1991c2eb34af97072fa |
| local/behavior/source-deformation-app-release-20261013-final-01.txt | 6f22a90d02f50992c1ca2e7a1bf460176dbb2304e459cfee272c3e55d596f7ca |
| local/deformation-parameters.json | 4fcba5842825013cf81db504415a469b386787ca199b44118f18fe69722b75cf |

Rust/Cargo1.99.0, Bevy0.19.1, Python3.12.14 reused. The first extractor rejectedE85FAD because that global lies in loader zero-fill, outside raw file data. It now explicitly records and supplies zero-fill within validated mapped section bounds. No data was accepted before this correction. No original binary was modified, no tools installed, no desktop input used and no approval rejected. No new source-loop simulation or numerical failure occurred; no owner help is required.
