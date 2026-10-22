//! UI font (AUTHORED): Bevy's default font is an ASCII-only subset of Fira
//! Mono Medium, so "Nürburgring" showed a box for the "ü" on the phone, which
//! has no system font to fall back on (owner, 2026-10-22). The full Fira Mono
//! Medium (same face and version, SIL OFL 1.1, `assets/fonts/README.md`)
//! replaces it as the default font. ASCII glyphs come from the same font as
//! before.
//!
//! sha256 of `FiraMono-Medium.ttf`:
//! 5f9173ce3d05fadef74c7eed06570d54e4f75bd0cd9860726fb2987a7f848292
use bevy::prelude::*;

pub const FIRA_MONO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/FiraMono-Medium.ttf");

/// Puts the full font at the default font handle. Call after `DefaultPlugins`
/// (whose text plugin inserts the subset there); does nothing without text.
pub fn install(app: &mut App) {
    if let Some(mut fonts) = app.world_mut().get_resource_mut::<Assets<Font>>() {
        let font = Font::from_bytes(FIRA_MONO_MEDIUM.to_vec());
        if let Err(error) = fonts.insert(AssetId::default(), font) {
            warn!("ui font: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Format 4 cmap of the font has a segment covering `c`.
    fn covers(font: &[u8], c: u16) -> bool {
        let u16_at = |at: usize| u16::from_be_bytes([font[at], font[at + 1]]);
        let u32_at = |at: usize| u32::from_be_bytes([font[at], font[at + 1], font[at + 2], font[at + 3]]);
        let tables = u16_at(4) as usize;
        let Some(cmap) = (0..tables)
            .map(|i| 12 + 16 * i)
            .find(|&at| &font[at..at + 4] == b"cmap")
            .map(|at| u32_at(at + 8) as usize)
        else {
            return false;
        };
        (0..u16_at(cmap + 2) as usize).any(|i| {
            let table = cmap + u32_at(cmap + 4 + 8 * i + 4) as usize;
            if u16_at(table) != 4 {
                return false;
            }
            let segments = u16_at(table + 6) as usize / 2;
            (0..segments).any(|s| {
                let end = u16_at(table + 14 + 2 * s);
                let start = u16_at(table + 16 + 2 * segments + 2 * s);
                (start..=end).contains(&c)
            })
        })
    }

    #[test]
    fn install_replaces_the_default_font_after_the_text_plugin() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::text::TextPlugin,
        ));
        let size = |app: &App| {
            app.world().resource::<Assets<Font>>().get(AssetId::default()).map(|f| f.data.len())
        };
        assert_eq!(size(&app), Some(bevy::text::DEFAULT_FONT_DATA.len()));
        install(&mut app);
        assert_eq!(size(&app), Some(FIRA_MONO_MEDIUM.len()));
    }

    #[test]
    fn the_ui_font_has_the_accented_names() {
        for c in "Nürburgring Ö É ç ğ ı ş Ü é è à ñ".chars() {
            assert!(covers(FIRA_MONO_MEDIUM, c as u16), "{c:?}");
        }
        assert!(covers(FIRA_MONO_MEDIUM, 'A' as u16));
    }
}
