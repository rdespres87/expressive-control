//! Zero-allocation internationalization (i18n) translation engine.
//!
//! Provides compile-time static string tables for all supported UI languages
//! and converts them directly to Slint-compatible structures.

use crate::UiI18n;

/// Supported user interface languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    French,
    English,
}

/// Static UI strings mapped to the Slint `UiI18n` structure.
pub struct UiStrings {
    pub root_note: &'static str,
    pub scale: &'static str,
    pub scale_chrom: &'static str,
    pub scale_major: &'static str,
    pub scale_minor: &'static str,
    pub scale_penta: &'static str,
    pub tuner_btn: &'static str,
    pub live_view: &'static str,
    pub base_octave: &'static str,
    pub octave_range: &'static str,
    pub midi_mode: &'static str,
    pub pb_range: &'static str,
    pub pitch_correction: &'static str,
    pub response_filter: &'static str,
    pub filter_slow: &'static str,
    pub filter_med: &'static str,
    pub filter_fast: &'static str,
    pub settings_btn: &'static str,
    pub transpose: &'static str,
    pub is_french: bool,
}

/// French static translation table.
pub const STRINGS_FR: UiStrings = UiStrings {
    root_note: "TONIQUE",
    scale: "GAMME",
    scale_chrom: "Chrom.",
    scale_major: "Majeure",
    scale_minor: "Mineure",
    scale_penta: "Penta",
    tuner_btn: "◀ ACCORD.",
    live_view: "VUE LIVE",
    base_octave: "OCTAVE BASE",
    octave_range: "PLAGE",
    midi_mode: "MODE MIDI",
    pb_range: "PLAGE PB",
    pitch_correction: "CORRECTION DE PITCH",
    response_filter: "RÉPONSE",
    filter_slow: "Lent",
    filter_med: "Moyen",
    filter_fast: "Rapide",
    settings_btn: "RÉGLAGES",
    transpose: "TRANSPOSITION: ",
    is_french: true,
};

/// English static translation table.
pub const STRINGS_EN: UiStrings = UiStrings {
    root_note: "ROOT NOTE",
    scale: "SCALE",
    scale_chrom: "Chrom.",
    scale_major: "Major",
    scale_minor: "Minor",
    scale_penta: "Penta",
    tuner_btn: "◀ TUNER",
    live_view: "LIVE VIEW",
    base_octave: "BASE OCTAVE",
    octave_range: "OCTAVE RANGE",
    midi_mode: "MIDI MODE",
    pb_range: "PB RANGE",
    pitch_correction: "PITCH CORRECTION",
    response_filter: "RESPONSE",
    filter_slow: "Slow",
    filter_med: "Med",
    filter_fast: "Fast",
    settings_btn: "SETTINGS",
    transpose: "TRANSPOSE: ",
    is_french: false,
};

/// Retrieve static translation table for the specified language.
#[inline(always)]
pub fn get_ui_strings(lang: Language) -> &'static UiStrings {
    match lang {
        Language::French => &STRINGS_FR,
        Language::English => &STRINGS_EN,
    }
}

/// Convert static strings to the Slint `UiI18n` structure.
pub fn to_slint_i18n(strings: &'static UiStrings) -> UiI18n {
    UiI18n {
        root_note: strings.root_note.into(),
        scale: strings.scale.into(),
        scale_chrom: strings.scale_chrom.into(),
        scale_major: strings.scale_major.into(),
        scale_minor: strings.scale_minor.into(),
        scale_penta: strings.scale_penta.into(),
        tuner_btn: strings.tuner_btn.into(),
        live_view: strings.live_view.into(),
        base_octave: strings.base_octave.into(),
        octave_range: strings.octave_range.into(),
        midi_mode: strings.midi_mode.into(),
        pb_range: strings.pb_range.into(),
        pitch_correction: strings.pitch_correction.into(),
        response_filter: strings.response_filter.into(),
        filter_slow: strings.filter_slow.into(),
        filter_med: strings.filter_med.into(),
        filter_fast: strings.filter_fast.into(),
        settings_btn: strings.settings_btn.into(),
        transpose: strings.transpose.into(),
        is_french: strings.is_french,
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::{get_ui_strings, to_slint_i18n, Language};

    #[test]
    fn test_translations_non_empty() {
        for lang in [Language::French, Language::English] {
            let s = get_ui_strings(lang);
            assert!(!s.root_note.is_empty());
            assert!(!s.scale.is_empty());
            assert!(!s.scale_chrom.is_empty());
            assert!(!s.scale_major.is_empty());
            assert!(!s.scale_minor.is_empty());
            assert!(!s.scale_penta.is_empty());
            assert!(!s.tuner_btn.is_empty());
            assert!(!s.live_view.is_empty());
            assert!(!s.base_octave.is_empty());
            assert!(!s.octave_range.is_empty());
            assert!(!s.midi_mode.is_empty());
            assert!(!s.pb_range.is_empty());
            assert!(!s.pitch_correction.is_empty());
            assert!(!s.response_filter.is_empty());
            assert!(!s.filter_slow.is_empty());
            assert!(!s.filter_med.is_empty());
            assert!(!s.filter_fast.is_empty());
            assert!(!s.settings_btn.is_empty());
            assert!(!s.transpose.is_empty());
        }
    }

    #[test]
    fn test_language_is_french_flag() {
        assert!(get_ui_strings(Language::French).is_french);
        assert!(!get_ui_strings(Language::English).is_french);
    }

    #[test]
    fn test_to_slint_i18n_conversion() {
        let fr = to_slint_i18n(get_ui_strings(Language::French));
        assert_eq!(fr.root_note, "TONIQUE");
        assert_eq!(fr.response_filter, "RÉPONSE");
        assert_eq!(fr.settings_btn, "RÉGLAGES");
        assert!(fr.is_french);

        let en = to_slint_i18n(get_ui_strings(Language::English));
        assert_eq!(en.root_note, "ROOT NOTE");
        assert_eq!(en.response_filter, "RESPONSE");
        assert_eq!(en.settings_btn, "SETTINGS");
        assert!(!en.is_french);
    }
}
