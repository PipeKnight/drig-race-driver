# GRID sectioned binary XML and physics-data linkage

Recorded 6 October 2026. Independent Rust decodes **33 binary XML documents** from the 130-file captured dependency set, including two raw `raceload.jpk` entries. Every element, value, parent/child range and attribute matches an independent Python decoder. The global `surface_materials.xml` and `car_materials.xml` also match, giving **35 verified documents**. These include AI, camera, damage, cloth, ornament, rules and progress metadata; decoding them does not implement their game behaviour.

## Byte contract

The selected format starts with LE u32 magic `0x7252221a`, followed by file length minus eight. A string-group tag `0x72522217` gives the combined size of the next two sections, including their sixteen header bytes. Each section is a tag / payload-size pair:

| Tag | Payload |
|---|---|
| `0x7252221d` | Contiguous NUL-terminated UTF-8 strings, then up to fifteen zero padding bytes |
| `0x7252221e` | LE u32 string offsets relative to the preceding string blob |
| `0x7252221b` | Element records, six LE u32 fields / 24 bytes each |
| `0x7252221c` | Attribute records, name-string ID and value-string ID / eight bytes each |

Element fields are name ID, value ID, attribute count, attribute start index, child count and child start index. Children form ranges in the flat element table. Element-value ID zero means absent text; string ID zero remains valid for element names and attribute values. The reader retains other values as strings, preserving numeric lexemes for separate typed consumers.

String offsets are contiguous and count real strings separately from alignment zeros. Initially counting every NUL as a string made AI data appear to have 879 strings; the offset table has the correct 875 strings plus four padding zeros. The reader validates offsets, UTF-8, section sizes, record divisibility, IDs, unique attribute ownership/names, unique child ownership, connectivity and depth. Root is element zero. Unknown format variants are errors, not silently interpreted as this format.

Primary references: Petar Tasev's MIT-licensed [pinned binXML template](https://github.com/EgoEngineModding/Ego-Engine-Modding/blob/f3fe9ee0f6e8379c64bc4a29e1d1b586bba5d1c1/src/010%20Templates/binXML.bt) and [upstream XML reader](https://github.com/EgoEngineModding/Ego-Engine-Modding/blob/f3fe9ee0f6e8379c64bc4a29e1d1b586bba5d1c1/src/EgoEngineLibrary/Xml/XmlFile.cs). Downloaded references/license are private; our Rust reader is independently authored and uses no XML library dependency.

## Newly readable data

`progress_track.xml` declares a circuit, total distance **4003.84**, one forward route, three splits (finish gate 0, time gates 22 and 45) and 71 progress gates. Gate endpoints and fine progress points are explicit world coordinates. Their field values are directly observed; original crossing/progress/lap algorithms remain untraced. Human-readable layout name remains unconfirmed.

The grid anchor still requires orientation investigation: car front-light geometry lies at local +Z, but composed raw `slot_00` +Z points opposite increasing early gate distance. Preview position/road alignment is verified, while faithful spawn heading is not. Keep the raw transform and flag this discrepancy rather than hiding it in an unexplained rotation.

`surface_materials.xml` contains contact friction, MECHANICS grip/slowdown/bump fields and tyre-table identifiers. For example, STDR explicitly names smooth dry tarmac and references TARM; grass references GRAS and has different grip/slowdown settings. These are distinct named fields, not a proof that one number alone is tyre friction or final grip. `raceload.jpk#materials.xml` instead contains car rendering controls. `raceload.jpk#physparams.xml` is plain XML, with shape/body/behaviour properties referenced by objecttypes. Special tokens such as `+inf` and `d+inf` must remain strings until their consumer semantics are recovered.

The plain `car_loader.xml` is a parameterized loader template, with `%...%` tokens embedded in element names. A standard XML parser correctly rejects the raw template before substitution; it is not evidence of corrupt game data. Record the template as such and trace substitution rather than silently rewriting it into ordinary XML.

## Implementation and evidence

`grid-formats/src/binxml.rs` is standard-library-only. `grid-binxml` writes create-new private field reports with UTF-8 values encoded as hex for exact comparison. `tools/validate_binxml.py` walks the captured dependency set and its JPK entries, compares all native rows, and reconstructs private readable XML. No selected compressed binary-XML entry was skipped.

```powershell
cargo run -p grid-probes --bin grid-binxml --locked -- 'C:\Program Files (x86)\Steam\steamapps\common\Grid\tracks\europe\donington\route_0\progress_track.xml' local/binxml/new-progress-report.tsv
```

Synthetic tests cover multilingual UTF-8, text-ID sentinel versus valid string zero, tree/attribute ownership, truncations, malformed sections/IDs and invalid byte sequences. One test initially treated a different start index on a zero-length child range as invalid; corrected it to a genuinely out-of-bounds range. **30 format tests pass**, and workspace Clippy passes; logs under `local/binxml/` retain failed and corrected results.

Private evidence: `local/binxml/captured-dependencies/validation.json`, all field reports/readable derivatives there, `surface-materials/validation.json`, `raceload-entry-survey.json`, `raceload-physparams.xml`, raw loader templates and material/property linkage reports. `local/scene/car-forward-light-geometry.json` records the geometry-side heading observation. Original files were read only.
