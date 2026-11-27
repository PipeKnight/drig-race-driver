# EGO engine modding tools (community index)

Recorded 27 November 2026 (F970) at the owner's request: a pointer to the community's list of EGO engine modding tools and how each relates to DRIG work. Nothing on this list is a DRIG dependency. Our readers are written independently and use these tools only as references, as the provenance notes in each format doc state.

## Where

- **[EGO Engine Modding Wiki: Utilities](https://ego.fandom.com/wiki/Utilities)** lists the maintained suite, other community tools, superseded tools and the historical tools of the GRID and DiRT 2 era. Read 27 November 2026; the page says its versions were last checked in October 2026. Community content is CC-BY-SA.
- Petar Tasev's (Ryder25) suite: sources on GitHub under MIT ([EgoEngineModding/Ego-Engine-Modding](https://github.com/EgoEngineModding/Ego-Engine-Modding)); downloads on his [itch.io page](https://p2t5r.itch.io/).

## Already used as references

- PSSG, JPK, binary XML and collision templates and readers from the suite repository, pinned at commit `f3fe9ee0f6e8379c64bc4a29e1d1b586bba5d1c1`: [RESOURCE_FORMATS.md](RESOURCE_FORMATS.md), [BINARY_XML.md](BINARY_XML.md), [TRACK_COLLISION.md](TRACK_COLLISION.md), [DONINGTON_BODY_CONTACTS.md](DONINGTON_BODY_CONTACTS.md).
- Ego CTF Editor 4.2.2016.0920 and its `EgoEngineLibrary` DLL, run read-only on the Lacetti tuning CSV: [VEHICLE_TUNING.md](VEHICLE_TUNING.md).

## Listed tools that may help later (not downloaded or run)

As the wiki lists them; GRID support here is the wiki's claim, not our check.
- **Ego Database Editor**: edits `database.bin` (events, AI skill, vehicles, teams); lists GRID, DiRT 2 and DiRT 3. A cross-check for career and event data (backlog row 60).
- **Ego File Converter**: binary XML, `lng`, `tpk` and `pkg` to common formats; a cross-check for our binary XML reader.
- **Ego Language Editor**: `lng` language files (front end text, original UI restoration).
- **EEnot Audio Extractor** (NC22): sound effects and music from `.dic` and `.raw` files of Race Driver: GRID and DiRT 3. A cross-check for the audio work (rows 142, 154).
- **EECMT** (zissakos): per-car camera values.
- Historical GRID-era tools (BinXML converter, PSSG Utility, Ryder's PSSG Editor 3.0, the 2010 database editor) are archived on the page.
- Newer-title tools (ERP Archiver, NeFS Edit, EasyERPMod) target later EGO games, not GRID; possibly relevant to the Dirt 3 study (backlog row 156) only if its files use them.

## Rules

- Download or run a tool only when a task needs it; record version and hash, keep binaries and outputs under `local/`, and never redistribute them or their schemas unless their licence allows it.
- Code from the MIT suite may be used only under its licence, with attribution; so far no code has been copied.
