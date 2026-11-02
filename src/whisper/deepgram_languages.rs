//! Every language Deepgram publishes for Nova-3.
//!
//! Taken from the official Models & Languages table rather than picked by hand.
//! `multi` comes first (55-language code-switching), then all monolingual codes
//! including regional variants, alphabetical by display name. Labels are unique
//! because the dropdown selects by label string.
//!
//! A code is only valid with a model that supports it: `enhanced` and `base`
//! support far fewer, and Deepgram rejects the connection if you pick a
//! combination it does not serve.
//!
//! The table is split into one constant per language so that no single item
//! holds the whole list; [`languages`] stitches them back together.

/// A code and the label the dropdown shows for it.
type Language = (&'static str, &'static str);

/// Multilingual.
const MULTILINGUAL: [Language; 1] = [("multi", "Multilingual (code-switching)")];

/// Afrikaans.
const AF: [Language; 2] = [("af", "Afrikaans"), ("af-ZA", "Afrikaans (South Africa)")];

/// Arabic.
const AR: [Language; 17] = [
    ("ar", "Arabic"),
    ("ar-DZ", "Arabic (Algeria)"),
    ("ar-TD", "Arabic (Chad)"),
    ("ar-EG", "Arabic (Egypt)"),
    ("ar-IR", "Arabic (Iran)"),
    ("ar-IQ", "Arabic (Iraq)"),
    ("ar-JO", "Arabic (Jordan)"),
    ("ar-KW", "Arabic (Kuwait)"),
    ("ar-LB", "Arabic (Lebanon)"),
    ("ar-MA", "Arabic (Morocco)"),
    ("ar-PS", "Arabic (Palestine)"),
    ("ar-QA", "Arabic (Qatar)"),
    ("ar-SA", "Arabic (Saudi Arabia)"),
    ("ar-SD", "Arabic (Sudan)"),
    ("ar-SY", "Arabic (Syria)"),
    ("ar-TN", "Arabic (Tunisia)"),
    ("ar-AE", "Arabic (UAE)"),
];

/// Armenian.
const HY: [Language; 1] = [("hy", "Armenian")];

/// Assamese.
const AS: [Language; 2] = [("as", "Assamese"), ("as-IN", "Assamese (India)")];

/// Belarusian.
const BE: [Language; 1] = [("be", "Belarusian")];

/// Bengali.
const BN: [Language; 1] = [("bn", "Bengali")];

/// Bosnian.
const BS: [Language; 1] = [("bs", "Bosnian")];

/// Bulgarian.
const BG: [Language; 1] = [("bg", "Bulgarian")];

/// Catalan.
const CA: [Language; 1] = [("ca", "Catalan")];

/// Chinese.
const ZH: [Language; 6] = [
    ("zh-HK", "Chinese (Cantonese, Traditional)"),
    ("zh", "Chinese (Mandarin, Simplified)"),
    ("zh-CN", "Chinese (Mandarin, Simplified) — zh-CN"),
    ("zh-Hans", "Chinese (Mandarin, Simplified) — zh-Hans"),
    ("zh-Hant", "Chinese (Mandarin, Traditional)"),
    ("zh-TW", "Chinese (Mandarin, Traditional) — zh-TW"),
];

/// Croatian.
const HR: [Language; 1] = [("hr", "Croatian")];

/// Czech.
const CS: [Language; 2] = [("cs", "Czech"), ("cs-CZ", "Czech (Czechia)")];

/// Danish.
const DA: [Language; 2] = [("da", "Danish"), ("da-DK", "Danish (Denmark)")];

/// Dutch.
const NL: [Language; 2] = [("nl", "Dutch"), ("nl-BE", "Dutch (Belgium)")];

/// English.
const EN: [Language; 7] = [
    ("en", "English"),
    ("en-AU", "English (Australia)"),
    ("en-CA", "English (Canada)"),
    ("en-IN", "English (India)"),
    ("en-NZ", "English (New Zealand)"),
    ("en-GB", "English (United Kingdom)"),
    ("en-US", "English (United States)"),
];

/// Estonian.
const ET: [Language; 1] = [("et", "Estonian")];

/// Finnish.
const FI: [Language; 1] = [("fi", "Finnish")];

/// French.
const FR: [Language; 2] = [("fr", "French"), ("fr-CA", "French (Canada)")];

/// Galician.
const GL: [Language; 1] = [("gl", "Galician")];

/// Georgian.
const KA: [Language; 2] = [("ka", "Georgian"), ("ka-GE", "Georgian (Georgia)")];

/// German.
const DE: [Language; 2] = [("de", "German"), ("de-CH", "German (Switzerland)")];

/// Greek.
const EL: [Language; 1] = [("el", "Greek")];

/// Gujarati.
const GU: [Language; 2] = [("gu", "Gujarati"), ("gu-IN", "Gujarati (India)")];

/// Hebrew.
const HE: [Language; 1] = [("he", "Hebrew")];

/// Hindi.
const HI: [Language; 1] = [("hi", "Hindi")];

/// Hungarian.
const HU: [Language; 1] = [("hu", "Hungarian")];

/// Indonesian.
const ID: [Language; 1] = [("id", "Indonesian")];

/// Italian.
const IT: [Language; 1] = [("it", "Italian")];

/// Japanese.
const JA: [Language; 1] = [("ja", "Japanese")];

/// Kannada.
const KN: [Language; 1] = [("kn", "Kannada")];

/// Kazakh.
const KK: [Language; 2] = [("kk", "Kazakh"), ("kk-KZ", "Kazakh (Kazakhstan)")];

/// Korean.
const KO: [Language; 2] = [("ko", "Korean"), ("ko-KR", "Korean (Korea)")];

/// Latvian.
const LV: [Language; 1] = [("lv", "Latvian")];

/// Lithuanian.
const LT: [Language; 1] = [("lt", "Lithuanian")];

/// Macedonian.
const MK: [Language; 1] = [("mk", "Macedonian")];

/// Malay.
const MS: [Language; 1] = [("ms", "Malay")];

/// Marathi.
const MR: [Language; 1] = [("mr", "Marathi")];

/// Mongolian.
const MN: [Language; 1] = [("mn", "Mongolian")];

/// Nepali.
const NE: [Language; 1] = [("ne", "Nepali")];

/// Norwegian.
const NO: [Language; 1] = [("no", "Norwegian")];

/// Pashto.
const PS: [Language; 2] = [("ps", "Pashto"), ("ps-AF", "Pashto (Afghanistan)")];

/// Persian.
const FA: [Language; 1] = [("fa", "Persian")];

/// Polish.
const PL: [Language; 1] = [("pl", "Polish")];

/// Portuguese.
const PT: [Language; 3] = [
    ("pt", "Portuguese"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("pt-PT", "Portuguese (Portugal)"),
];

/// Punjabi.
const PA: [Language; 2] = [("pa", "Punjabi"), ("pa-IN", "Punjabi (India)")];

/// Romanian.
const RO: [Language; 1] = [("ro", "Romanian")];

/// Russian.
const RU: [Language; 1] = [("ru", "Russian")];

/// Serbian.
const SR: [Language; 1] = [("sr", "Serbian")];

/// Slovak.
const SK: [Language; 1] = [("sk", "Slovak")];

/// Slovenian.
const SL: [Language; 1] = [("sl", "Slovenian")];

/// Spanish.
const ES: [Language; 2] = [("es", "Spanish"), ("es-419", "Spanish (Latin America)")];

/// Swedish.
const SV: [Language; 2] = [("sv", "Swedish"), ("sv-SE", "Swedish (Sweden)")];

/// Tagalog.
const TL: [Language; 1] = [("tl", "Tagalog")];

/// Tamil.
const TA: [Language; 1] = [("ta", "Tamil")];

/// Telugu.
const TE: [Language; 1] = [("te", "Telugu")];

/// Thai.
const TH: [Language; 2] = [("th", "Thai"), ("th-TH", "Thai (Thailand)")];

/// Turkish.
const TR: [Language; 2] = [("tr", "Turkish"), ("tr-TR", "Turkish (TR)")];

/// Ukrainian.
const UK: [Language; 1] = [("uk", "Ukrainian")];

/// Urdu.
const UR: [Language; 1] = [("ur", "Urdu")];

/// Vietnamese.
const VI: [Language; 1] = [("vi", "Vietnamese")];

/// Xhosa.
const XH: [Language; 1] = [("xh", "Xhosa")];

/// Zulu.
const ZU: [Language; 1] = [("zu", "Zulu")];

/// The list, as the dropdown shows it.
pub(super) fn languages() -> Vec<(String, String)> {
    CHUNKS
        .iter()
        .flat_map(|chunk| chunk.iter())
        .map(|(code, label)| (code.to_string(), label.to_string()))
        .collect()
}

/// Every language block, in the order the dropdown shows them.
const CHUNKS: &[&[Language]] = &[
    &MULTILINGUAL,
    &AF,
    &AR,
    &HY,
    &AS,
    &BE,
    &BN,
    &BS,
    &BG,
    &CA,
    &ZH,
    &HR,
    &CS,
    &DA,
    &NL,
    &EN,
    &ET,
    &FI,
    &FR,
    &GL,
    &KA,
    &DE,
    &EL,
    &GU,
    &HE,
    &HI,
    &HU,
    &ID,
    &IT,
    &JA,
    &KN,
    &KK,
    &KO,
    &LV,
    &LT,
    &MK,
    &MS,
    &MR,
    &MN,
    &NE,
    &NO,
    &PS,
    &FA,
    &PL,
    &PT,
    &PA,
    &RO,
    &RU,
    &SR,
    &SK,
    &SL,
    &ES,
    &SV,
    &TL,
    &TA,
    &TE,
    &TH,
    &TR,
    &UK,
    &UR,
    &VI,
    &XH,
    &ZU,
];
