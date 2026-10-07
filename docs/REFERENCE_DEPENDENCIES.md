# Observed Lacetti / Donington resource dependencies

Recorded: 6 October 2026, Europe/Kiev. A macro-driven fresh race load in the verified private GRID process accessed 130 installation resources. Successful ReadFile events confirm `cars/lac` and `tracks/europe/donington/route_0`. Owner confirmation supplies the human-readable Chevrolet Lacetti / Donington Park pairing.

## Capture and provenance

- GRID PID 19680, creation UTC `2026-10-06T01:04:50.0562006Z`, private `uk-UA` executable SHA-256 `b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6`.
- Procmon 4.11 requested 90 seconds from `01:28:45.9899092Z`; Go To Race triggered at `01:29:18Z`. Final PML last write `01:30:22.8622718Z`. Export completed with exit 0 at `01:31:27.9344626Z`.
- 2,878,420 all-process events; 107,756 selected GRID events; 2,605 ReadFile events across all paths. Every one of the 130 installation paths below has successful ReadFile events. Repeated reads/opens are counted, not unique loader calls.
- Streaming report: `local/reference-race/load-20261006-0128/summary/procmon-summary.json`; filtered events: `summary/grid-events.private.csv`. Raw PML/CSV, machine paths and full per-file hashes remain private.
- Read-only comparison at `01:35:14.981935Z`: all 130 private resource hashes match their original Steam counterparts. Original `grid.exe` still has SHA-256 `dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f`. Evidence: `steam-resource-verification.json` in the capture directory.

This is an observed dependency set from a warm process: a previous race had already loaded. Cached shared/startup resources can be missing. A read from a JPK does not establish which internal entries are used, and file names alone do not prove data semantics or a complete dependency graph. No collision/physics field meanings are promoted from these observations.

## Next decoding targets

1. Lacetti `lac_highLOD.pssg`, `lac_gen_high.pssg` and `livery_00/textures_high/lac_tex_high_00.pssg`: confirmed reads make them suitable mesh/material/texture targets for the first car viewer. High-LOD versus low-LOD vehicle assignments remain an interpretation, not a per-object runtime trace.
2. Donington `objects.pssg`, `tracksplit.pssg`, `land.pssg`, route_0 `routesplit.pssg` and `track.jpk`: recover geometry, instances/transforms and container payloads. JPK table framing already works; payload decompression/wrappers remain unresolved.
3. `raceload.jpk` plus route_0 `reducedmechanics.jpk`: connect container entries to the statically identified loader candidates using arguments and buffers. Existing entry names are clues; internal consumption is unverified.
4. Lacetti `lac.csv`, `lac.ned`, `lac.ppts`, and route_0 `boundarylines.cqtc`, `route.clm`, `progress_track.xml`: investigate tuning, collision and race metadata without guessing units or layouts. Some XML extensions contain binary wrappers; inspect actual bytes before selecting a parser.

Assisted Automatic driving is the current reference condition. The trace also contains BMW car/audio resources, consistent with opponents, without identifying every object. Keep future handling measurements separate from these qualitative macro acceptance tests. See [reference conditions](REFERENCE_RACE.md), [macro tests](MACRO_SETUP.md), [first format readers](RESOURCE_FORMATS.md) and [loader candidates](LOADER_RESEARCH.md).

## Resource group counts

| Top-level group | Files |
|---|---:|
| `ai` | 7 |
| `anims` | 11 |
| `audio` | 14 |
| `cars` | 27 |
| `characters` | 1 |
| `effects` | 2 |
| `gameconfig` | 1 |
| `interiors` | 9 |
| `postprocess` | 2 |
| `raceload.jpk` | 1 |
| `simplefonts` | 4 |
| `skidmarks` | 2 |
| `track_sponsors` | 2 |
| `tracks` | 47 |

## Complete observed installation-file list

Paths are relative to the GRID installation. Counts include successful events only.

| Resource | Bytes | Opens (CreateFile) | ReadFile events |
|---|---:|---:|---:|
| `ai/ai_behaviour_fire.xml` | 1,248 | 5 | 1 |
| `ai/ai_choreographer.xml` | 9,432 | 5 | 1 |
| `ai/ai_driver_config.xml` | 13,164 | 5 | 1 |
| `ai/ai_driver_parameter_mappings.xml` | 16,796 | 5 | 1 |
| `ai/ai_vehicle_config.xml` | 10,548 | 5 | 1 |
| `ai/ai_vehicle_statistics.xml` | 228,684 | 5 | 1 |
| `ai/catchup.xml` | 9,436 | 5 | 1 |
| `anims/cr_female.pssg` | 852,605 | 6 | 2 |
| `anims/cr_male_2.pssg` | 852,370 | 6 | 2 |
| `anims/crowd_anims.pssg` | 1,389,508 | 6 | 3 |
| `anims/crowd_hla.xml` | 84,772 | 5 | 1 |
| `anims/crowd_props.xml` | 2,420 | 5 | 1 |
| `anims/crowdDefs.xml` | 117,248 | 5 | 1 |
| `anims/marshall.pssg` | 514,315 | 6 | 1 |
| `anims/organism_static_dataset.xml` | 1,127 | 5 | 1 |
| `anims/props.pssg` | 76,105 | 6 | 1 |
| `anims/props_flag_large.clo` | 19,252 | 5 | 1 |
| `anims/props_flag_small.clo` | 14,284 | 5 | 1 |
| `audio/cars/bmwengine.ini` | 29,976 | 25 | 5 |
| `audio/cars/bmwgear.ini` | 5,070 | 25 | 5 |
| `audio/cars/lacengine.ini` | 29,975 | 35 | 7 |
| `audio/cars/lacgear.ini` | 5,070 | 35 | 7 |
| `audio/eng_bmw.WIP` | 7,933,364 | 4 | 16 |
| `audio/eng_lac.WIP` | 5,093,480 | 4 | 10 |
| `audio/loading.WIP` | 4,648,968 | 4 | 9 |
| `audio/loc_europe.WIP` | 9,129,996 | 4 | 18 |
| `audio/mec_bmw.WIP` | 981,360 | 4 | 2 |
| `audio/mec_lac.WIP` | 1,122,950 | 4 | 3 |
| `audio/speech/en/05_rules/satOnTrack_1.raw` | 127,424 | 4 | 1 |
| `audio/speech/en/Names_Player/pit/Chief_BL_M_01.raw` | 80,448 | 4 | 1 |
| `audio/speech/en/Names_Player/pit/Chief_OK_M_01.raw` | 66,612 | 4 | 1 |
| `audio/speech/en/Names_Player/pit/Chief_WD_M_01.raw` | 83,108 | 4 | 1 |
| `cars/bmw/bmw.csv` | 2,633 | 25 | 5 |
| `cars/bmw/bmw.ned` | 33,184 | 25 | 5 |
| `cars/bmw/bmw.ppts` | 3,108 | 25 | 5 |
| `cars/bmw/bmw_anim_dummies.pssg` | 18,088 | 6 | 1 |
| `cars/bmw/bmw_AO.pssg` | 26,835 | 6 | 1 |
| `cars/bmw/bmw_gen_low.pssg` | 1,214,568 | 6 | 3 |
| `cars/bmw/bmw_lowLOD.pssg` | 1,338,794 | 6 | 3 |
| `cars/bmw/decals.xml` | 6,094 | 25 | 5 |
| `cars/bmw/livery_00/textures_low/bmw_tex_low_00.pssg` | 2,597,055 | 6 | 5 |
| `cars/bmw/livery_02/textures_low/bmw_tex_low_02.pssg` | 2,597,055 | 6 | 5 |
| `cars/bmw/livery_03/textures_low/bmw_tex_low_03.pssg` | 2,597,055 | 6 | 5 |
| `cars/lac/cameras.xml` | 34,576 | 5 | 1 |
| `cars/lac/decals.xml` | 5,029 | 35 | 7 |
| `cars/lac/lac.csv` | 2,628 | 35 | 7 |
| `cars/lac/lac.ned` | 32,336 | 35 | 7 |
| `cars/lac/lac.ppts` | 3,588 | 35 | 7 |
| `cars/lac/lac_anim_dummies.pssg` | 18,088 | 6 | 1 |
| `cars/lac/lac_AO.pssg` | 26,835 | 6 | 1 |
| `cars/lac/lac_gen_high.pssg` | 4,068,200 | 6 | 8 |
| `cars/lac/lac_gen_low.pssg` | 1,214,568 | 6 | 3 |
| `cars/lac/lac_highLOD.pssg` | 3,366,516 | 6 | 7 |
| `cars/lac/lac_lowLOD.pssg` | 1,501,122 | 6 | 3 |
| `cars/lac/livery_00/textures_high/lac_tex_high_00.pssg` | 6,138,047 | 6 | 12 |
| `cars/lac/livery_00/textures_low/lac_tex_low_00.pssg` | 2,597,055 | 6 | 5 |
| `cars/lac/livery_01/textures_low/lac_tex_low_01.pssg` | 2,597,055 | 6 | 5 |
| `cars/lac/livery_02/textures_low/lac_tex_low_02.pssg` | 2,597,055 | 6 | 5 |
| `cars/user_liveries/numbers/font0.pssg` | 236,904 | 6 | 1 |
| `characters/anims/driver_anims.pssg` | 1,714,799 | 6 | 4 |
| `effects/tree_fx.xml` | 1,496 | 5 | 1 |
| `effects/tunePfx.xml` | 881 | 5 | 1 |
| `gameconfig/rules.xml` | 1,680 | 5 | 1 |
| `interiors/bmw/int_bmw_anim.xml` | 983 | 25 | 5 |
| `interiors/bmw/int_bmw_anims.pssg` | 55,273 | 6 | 1 |
| `interiors/bmw/int_bmw_driver_anims.pssg` | 34,859 | 6 | 1 |
| `interiors/lac/int_lac.pssg` | 8,469,873 | 7 | 17 |
| `interiors/lac/int_lac.xml` | 1,735 | 5 | 1 |
| `interiors/lac/int_lac_anim.xml` | 983 | 35 | 7 |
| `interiors/lac/int_lac_anims.pssg` | 55,759 | 6 | 1 |
| `interiors/lac/int_lac_driver_anims.pssg` | 34,859 | 6 | 1 |
| `interiors/lac/int_lac_environmentmap.pssg` | 65,164 | 6 | 1 |
| `postprocess/effects.xml` | 66,433 | 5 | 1 |
| `postprocess/effects_override.xml` | 757 | 5 | 1 |
| `raceload.jpk` | 80,880 | 5 | 1 |
| `simplefonts/int_digi_gear_font.xml` | 188 | 5 | 1 |
| `simplefonts/int_digi_speed_font.xml` | 190 | 5 | 1 |
| `simplefonts/int_gear_font.xml` | 178 | 5 | 1 |
| `simplefonts/simple_fonts.pssg` | 129,559 | 6 | 1 |
| `skidmarks/road_tm.pssg` | 719,187 | 6 | 2 |
| `skidmarks/road_tm.xml` | 26,200 | 15 | 3 |
| `track_sponsors/bridgestone/sponsor_event.pssg` | 714,787 | 6 | 2 |
| `track_sponsors/tag_heuer/sponsor_minor.pssg` | 714,787 | 6 | 2 |
| `tracks/entities_dataset.xml` | 627 | 5 | 1 |
| `tracks/europe/donington/ambience_triggers.xml` | 1,936 | 5 | 1 |
| `tracks/europe/donington/basewind.xml` | 1,072 | 5 | 1 |
| `tracks/europe/donington/cloth.xml` | 56,264 | 5 | 1 |
| `tracks/europe/donington/effects.xml` | 10,142 | 5 | 1 |
| `tracks/europe/donington/grass.grs` | 623,410 | 5 | 2 |
| `tracks/europe/donington/ground_cover.pssg` | 371,025 | 6 | 1 |
| `tracks/europe/donington/ground_cover.xml` | 656 | 5 | 1 |
| `tracks/europe/donington/intro_sequence.xml` | 4,340 | 5 | 1 |
| `tracks/europe/donington/land.pssg` | 2,011,696 | 6 | 4 |
| `tracks/europe/donington/lemans_sponsors.pssg` | 2,640,735 | 6 | 6 |
| `tracks/europe/donington/lighting.xml` | 2,614 | 5 | 1 |
| `tracks/europe/donington/lod_overrides.xml` | 292 | 5 | 1 |
| `tracks/europe/donington/mud.xml` | 180 | 5 | 1 |
| `tracks/europe/donington/objects.pssg` | 15,048,076 | 5 | 29 |
| `tracks/europe/donington/objectstextures.pssg` | 133,587 | 5 | 1 |
| `tracks/europe/donington/objecttypes.pssg` | 131,812 | 6 | 1 |
| `tracks/europe/donington/organism_track_dataset.xml` | 662 | 5 | 1 |
| `tracks/europe/donington/patchup_ot.pssg` | 23,057,047 | 7 | 44 |
| `tracks/europe/donington/route_0/ai_track.xml` | 78,668 | 5 | 1 |
| `tracks/europe/donington/route_0/ai_vehicle_track.xml` | 16,584 | 5 | 1 |
| `tracks/europe/donington/route_0/boundarylines.cqtc` | 51,631 | 5 | 1 |
| `tracks/europe/donington/route_0/clothFile.bin` | 7,204 | 5 | 1 |
| `tracks/europe/donington/route_0/crowd_standing.bin` | 474,134 | 5 | 1 |
| `tracks/europe/donington/route_0/grids.pssg` | 19,656 | 11 | 2 |
| `tracks/europe/donington/route_0/objects.ens` | 330,161 | 11 | 2 |
| `tracks/europe/donington/route_0/ornament_attributes.xml` | 15,752 | 5 | 1 |
| `tracks/europe/donington/route_0/ornaments.bin` | 120,649 | 5 | 1 |
| `tracks/europe/donington/route_0/progress_track.xml` | 58,496 | 5 | 1 |
| `tracks/europe/donington/route_0/reducedmechanics.jpk` | 197,568 | 5 | 1 |
| `tracks/europe/donington/route_0/replay_camera_config.xml` | 149,348 | 5 | 1 |
| `tracks/europe/donington/route_0/route.clm` | 350,816 | 5 | 1 |
| `tracks/europe/donington/route_0/route_overrides.xml` | 376 | 5 | 1 |
| `tracks/europe/donington/route_0/routesplit.pssg` | 55,358 | 6 | 1 |
| `tracks/europe/donington/route_0/track.jpk` | 1,842,752 | 5 | 4 |
| `tracks/europe/donington/route_0/track.vis` | 2,842,800 | 5 | 6 |
| `tracks/europe/donington/route_0/tree_attributes.xml` | 1,640 | 5 | 1 |
| `tracks/europe/donington/route_0/trees.bin` | 34,190 | 5 | 1 |
| `tracks/europe/donington/sky.pssg` | 1,074,101 | 6 | 3 |
| `tracks/europe/donington/start_lights.xml` | 732 | 5 | 1 |
| `tracks/europe/donington/tracksplit.pssg` | 20,781,247 | 5 | 40 |
| `tracks/europe/donington/trees.pssg` | 404,752 | 6 | 1 |
| `tracks/europe/donington/treestextures.pssg` | 4,786,385 | 6 | 10 |
| `tracks/ornament_mip_streaming.xml` | 902 | 5 | 1 |
| `tracks/tracklodsettings.xml` | 61 | 5 | 1 |
| `tracks/vegetationdeform.xml` | 420 | 5 | 1 |
| `tracks/windNoise.per` | 65,556 | 5 | 1 |
