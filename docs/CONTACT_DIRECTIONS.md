# CD01: body-contact sampling directions from the Steam grid.exe (P6, F359)

CD01 is the private 712-byte table of 44 body-contact sampling directions. Before F359, the only way to produce it was `tools/extract_contact_directions.py`, which needs the Steamless-unpacked image. A phone (P9/P10) and any user other than the owner have no such image. This note records how the original builds the table and how the port now reads it from the purchased Steam `grid.exe` alone.

## 1. How the original builds the table [V]

- **Generated at startup, not stored.** A static initializer at `C7DAE0` writes 44 16-byte vectors to `.data` `F5E1B0..F5E470`. The scratch constructor `B7E440` later copies the 0x2C0-byte template (docs/MESH_CONTACT_GENERATION.md, F219). In the file `F5E1B0` lies in the zero-filled part of `.data` (file-backed `.data` ends at `0xE58C00`). The vectors exist only after `C7DAE0` runs.
- **The initializer only moves data.** It is 344 instructions through its first `ret` at `C7E307`:
  - frame setup;
  - `xorps` to zero a register;
  - `movss` scalar loads from absolute addresses;
  - `movss` stores to a 16-byte aligned stack slot;
  - `movaps` loads from that slot;
  - `movaps` stores to the table.
  
  There is no arithmetic, negation or shuffle. The existing extractor fails closed on anything else.
- **So each lane is one source word or zero.** Following the moves gives, for every direction, the absolute address of each x/y/z word:
  - 119 of 132 lanes are one 32-bit word copied from the file;
  - the other 13 come from the zeroed register;
  - w is always zero.
  
  There are 55 distinct source words:
  - `0xCAF888`, `0xCAFE50` and `0xCB0948`, shared constants for the axis directions (1.0, -0.0 and -1.0 respectively);
  - 52 words at `0xD426B4..0xD42780`, a block of per-direction components.
- **The sources are readable in the Steam file.** All 55 words lie in `.rdata` (`0xC89000..0xDA59A4`). SteamStub 2.1 encrypts `.text`, which holds `C7DAE0`, but leaves `.rdata` in plain form: the section table and `.rdata` placement are the same in the Steam and unpacked files. That is why the Python extractor already read its scalars from the Steam file and only the instruction stream from the unpacked image.
- **Earlier validation.** Earlier (F219/F224) the whole original `C7DAE0` was run in the native harness. All 704 table bytes matched the moves-only reading (docs/TRIANGLE_MESH_CONTACT_QUERY.md, `local/behavior/contact-directions-source-20261012-final-02/validation.json`).

Evidence for this note: `local/p6-cd01/` in the main checkout:
- `trace_recipe.py`, the read-only Capstone 5.0.6 trace of the unpacked image with bundled Python 3.12.14;
- `trace.txt`, with the 344 instructions, the per-lane source addresses and 55 distinct sources;
- `recipe.rs.txt`, the generated Rust recipe;
- `cd01.bin`, the Rust output;
- the check logs.

## 2. Decision

Question (b) from P6 has a yes answer. The 44 directions are reproduced from the Steam `grid.exe` without unpacking, decrypting or executing it.

- **(a) Computing from game data or formulas: rejected.** The directions are the 6 axes plus icosahedron-family directions (golden-ratio values 0.276388, 0.447216, 0.525731, 0.723607, 0.850651, 0.894427, 0.951057 and so on). They were typed as about 6-digit decimal constants with inconsistent rounding. For example, 0.309011 and 0.309013 both stand for sin 18°, and 0.447216, 0.447219 and 0.44722 for 1/√5. A geometric formula gives the ideal values, not these bits, so it cannot reproduce CD01. docs/TRIANGLE_MESH_CONTACT_QUERY.md already warns against idealized values. No game data file other than `grid.exe` is known to hold the table.
- **(b) Reading from the Steam file: implemented.** The instruction stream that the Steam file hides was traced once. It is recorded in public source as an address recipe, `CD01_SOURCES` in `crates/grid-tools/src/private_parameters.rs`: 44 × 3 virtual addresses, 0 meaning a zeroed lane. The recipe holds addresses only, like the BP01/DF01 address lists. It holds no game bytes and no instruction bytes. At run time the reader:
  - checks the Steam SHA-256 `dd7b0933…e48f`;
  - reads each word with the BP01 rule (the first section whose file-backed bytes hold all 4 bytes);
  - writes `CD01`, count 44, then the 44 × (x, y, z, 0) vectors;
  - checks that each vector has unit length within 0.001, as the Python tool does.
- **(c) Desktop preparation step: not needed.** Any user with the supported Steam `grid.exe` gets CD01 at start-up. On Android, P10 copies `grid.exe` with the other files and the app derives CD01 the same way as BP01/DF01. No desktop step and no import of a prepared file are needed. Other `grid.exe` builds are refused by the SHA-256 check, as with BP01/DF01. Supporting another build would need its own trace.

## 3. Port

- `grid_tools::private_parameters::contact_directions(exe) -> Vec<u8>`.
- `grid-private-parameters GRID_EXE BP01 DF01 [CD01]` writes the file. It refuses to overwrite.
- App: `body_contacts = derive` in `[private]`, or `--body-contacts derive`. A path still selects a prepared file, as before. `grid-opponent-race`, the probes and the `GRID_CONTACT_DIRECTIONS` tests still take a file path.
- The shipped profiles `settings/{body-contacts,time-trial,race,race-istanbul,race-long-beach}.ini` now say `body_contacts = derive`. The five launchers no longer require `local\body-contact-directions.bin`.
- Tests:
  - `contact_directions_equal_the_prepared_file` compares the derived bytes with `GRID_CONTACT_DIRECTIONS`, or with `local/body-contact-directions.bin`, byte for byte. It skips when either input is absent;
  - `contact_direction_sources_are_rdata_words` checks the recipe shape: 119 aligned `.rdata` addresses.

## 4. Verification

Steam `grid.exe` SHA-256 `dd7b0933…e48f`, unchanged and read only.

- The derived CD01 is 712 bytes, SHA-256 `ea8d8bff5270d5b75410f822c3e784a66399b7aae4e39365f8f54747639179c7`. `cmp` against `local/body-contact-directions.bin` (produced 2026-10-12 from the unpacked image) shows the files are identical.
- The pre-push checks are in PROJECT_LOG.md F359.

`tools/extract_contact_directions.py` remains as the research tool that produced the recipe and the first file. It is no longer needed to play.
