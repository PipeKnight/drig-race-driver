# UI font

`FiraMono-Medium.ttf` is Fira Mono Medium, version 3.206, by The Mozilla Foundation and Telefonica
S.A. It is licensed under the SIL Open Font License 1.1 (`FiraMono-LICENSE`, copied with it).

- Source: the copy shipped in the `bevy_feathers` 0.19.1 crate (`src/assets/fonts/`), which is
  the full face of Bevy's ASCII-only default font, `bevy_text` 0.19.1 `FiraMono-subset.ttf`
  (same name and version, 95 glyphs).
- sha256 `FiraMono-Medium.ttf`: see `crates/grid-app/src/ui_font.rs`.
- It has 1,350 glyphs, including all of Latin-1 and 127 of 128 Latin Extended-A characters.
- `ui_font.rs` makes it the default UI font, so names such as "Nürburgring" render on Android,
  which has no system font fallback.
