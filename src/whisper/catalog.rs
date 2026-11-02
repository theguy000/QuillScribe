//! What the Settings dropdown offers: models, languages and their labels.
//!
//! Pure data and lookups, so a settings screen can be built without touching
//! the network or the disk.

use super::WhisperManager;

use super::{deepgram_languages, ModelInfo};

/// One row of the model catalogue: the names it covers, and what it costs.
struct ModelRow {
    names: &'static [&'static str],
    size: &'static str,
    memory: &'static str,
    speed: &'static str,
    quality: &'static str,
}

/// Build a row from its four cost fields, so the table stays aligned.
const fn row(
    names: &'static [&'static str],
    info: (&'static str, &'static str, &'static str, &'static str),
) -> ModelRow {
    ModelRow {
        names,
        size: info.0,
        memory: info.1,
        speed: info.2,
        quality: info.3,
    }
}

/// The model catalogue: every name the dropdown offers and what it costs.
const MODEL_ROWS: &[ModelRow] = &[
    row(&["tiny", "tiny.en"], ("75 MB", "~390 MB", "~10x", "Low")),
    row(
        &["base", "base.en"],
        ("142 MB", "~500 MB", "~7x", "Low-Medium"),
    ),
    row(&["small", "small.en"], ("466 MB", "~1 GB", "~4x", "Medium")),
    row(
        &["medium", "medium.en"],
        ("1.5 GB", "~2.6 GB", "~2x", "Medium-High"),
    ),
    row(
        &["large-v1", "large-v2", "large-v3"],
        ("2.9 GB", "~4.7 GB", "~1x", "High"),
    ),
    row(&["turbo"], ("1.5 GB", "~2.6 GB", "~8x", "High")),
    row(
        &["distil-large-v2", "distil-large-v3"],
        ("756 MB", "~1.5 GB", "~6x", "Medium-High"),
    ),
    row(
        &["distil-medium.en"],
        ("789 MB", "~1.5 GB", "~5x", "Medium"),
    ),
    row(
        &["distil-small.en"],
        ("332 MB", "~800 MB", "~6x", "Low-Medium"),
    ),
    row(
        &["gpt-4o-transcribe"],
        ("Cloud", "N/A", "Fast", "Very High"),
    ),
    row(
        &["gpt-4o-mini-transcribe"],
        ("Cloud", "N/A", "Very Fast", "High"),
    ),
    row(
        &["whisper-large-v3-turbo"],
        ("Cloud", "N/A", "Very Fast", "High"),
    ),
    row(&["whisper-large-v3"], ("Cloud", "N/A", "Fast", "Very High")),
];

impl WhisperManager {
    pub fn get_available_api_models() -> Vec<String> {
        vec![
            "gpt-4o-transcribe".to_string(),
            "gpt-4o-mini-transcribe".to_string(),
        ]
    }

    pub fn get_available_groq_models() -> Vec<String> {
        vec![
            "whisper-large-v3-turbo".to_string(),
            "whisper-large-v3".to_string(),
        ]
    }

    pub fn get_available_gemini_models() -> Vec<String> {
        vec![
            "gemini-2.0-flash".to_string(),
            "gemini-1.5-flash".to_string(),
            "gemini-1.5-pro".to_string(),
        ]
    }

    pub fn get_available_deepgram_models() -> Vec<String> {
        vec![
            "nova-3".to_string(),
            "nova-2".to_string(),
            "nova-2-general".to_string(),
            "nova-2-phonecall".to_string(),
            "enhanced".to_string(),
            "base".to_string(),
        ]
    }

    pub fn get_available_api_models_for_provider(provider: &str) -> Vec<String> {
        match provider {
            "gemini" => Self::get_available_gemini_models(),
            "groq" => Self::get_available_groq_models(),
            "deepgram" => Self::get_available_deepgram_models(),
            _ => Self::get_available_api_models(),
        }
    }

    pub fn get_available_local_models() -> Vec<String> {
        vec![
            "tiny".to_string(),
            "tiny.en".to_string(),
            "base".to_string(),
            "base.en".to_string(),
            "small".to_string(),
            "small.en".to_string(),
            "medium".to_string(),
            "medium.en".to_string(),
            "large-v1".to_string(),
            "large-v2".to_string(),
            "large-v3".to_string(),
            "turbo".to_string(),
            "distil-large-v2".to_string(),
            "distil-large-v3".to_string(),
            "distil-medium.en".to_string(),
            "distil-small.en".to_string(),
        ]
    }

    pub fn get_local_model_categories() -> Vec<String> {
        vec![
            "General".to_string(),
            "Tiny".to_string(),
            "Base".to_string(),
            "Small".to_string(),
            "Medium".to_string(),
            "Large".to_string(),
        ]
    }

    pub fn get_local_models_for_category(category: &str) -> Vec<String> {
        Self::get_available_local_models()
            .into_iter()
            .filter(|model| matches_category(category, model))
            .collect()
    }

    pub fn get_model_info(model_name: &str) -> ModelInfo {
        MODEL_ROWS
            .iter()
            .find(|row| row.names.contains(&model_name))
            .map(|row| ModelInfo::new(row.size, row.memory, row.speed, row.quality))
            .unwrap_or_else(|| ModelInfo::new("Unknown", "Unknown", "Unknown", "Unknown"))
    }

    /// Languages Deepgram accepts on a live session. Deepgram has its own
    /// catalogue, and `multi` is the closest thing it has to auto-detection
    /// (55 languages with per-word code-switching on Nova-3).
    /// Every language Deepgram publishes for Nova-3.
    ///
    /// The list is built once and shared; the dropdown asks for it on every
    /// settings screen and it never changes at runtime.
    pub fn get_available_deepgram_languages() -> Vec<(String, String)> {
        static LANGUAGES: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
        LANGUAGES.get_or_init(deepgram_languages::languages).clone()
    }

    /// The language catalogue for a provider. Deepgram does not share the
    /// OpenAI/Whisper code list, so it needs its own.
    pub fn get_available_languages_for_provider(provider: &str) -> Vec<(String, String)> {
        if provider == "deepgram" {
            Self::get_available_deepgram_languages()
        } else {
            Self::get_available_languages()
        }
    }

    /// Display labels for the language dropdown, in list order.
    pub fn language_labels_for_provider(provider: &str) -> Vec<String> {
        Self::get_available_languages_for_provider(provider)
            .into_iter()
            .map(|(_, label)| label)
            .collect()
    }

    /// The display label for a stored language code, so the dropdown can show
    /// the saved value after a restart.
    pub fn language_label_for(provider: &str, code: &str) -> String {
        let resolved = Self::resolve_language_or_default(provider, code);
        Self::get_available_languages_for_provider(provider)
            .into_iter()
            .find(|(c, _)| *c == resolved)
            .map(|(_, label)| label)
            .unwrap_or_else(|| "English".to_string())
    }

    /// The Whisper/OpenAI language list, as the dropdown shows it.
    const LANGUAGES: &[(&str, &str)] = &[
        ("af", "Afrikaans"),
        ("am", "Amharic"),
        ("ar", "Arabic"),
        ("as", "Assamese"),
        ("az", "Azerbaijani"),
        ("ba", "Bashkir"),
        ("be", "Belarusian"),
        ("bg", "Bulgarian"),
        ("bn", "Bengali"),
        ("bo", "Tibetan"),
        ("br", "Breton"),
        ("bs", "Bosnian"),
        ("ca", "Catalan"),
        ("cs", "Czech"),
        ("cy", "Welsh"),
        ("da", "Danish"),
        ("de", "German"),
        ("el", "Greek"),
        ("en", "English"),
        ("es", "Spanish"),
        ("et", "Estonian"),
        ("eu", "Basque"),
        ("fa", "Persian"),
        ("fi", "Finnish"),
        ("fo", "Faroese"),
        ("fr", "French"),
        ("gl", "Galician"),
        ("gu", "Gujarati"),
        ("ha", "Hausa"),
        ("haw", "Hawaiian"),
        ("he", "Hebrew"),
        ("hi", "Hindi"),
        ("hr", "Croatian"),
        ("ht", "Haitian Creole"),
        ("hu", "Hungarian"),
        ("hy", "Armenian"),
        ("id", "Indonesian"),
        ("is", "Icelandic"),
        ("it", "Italian"),
        ("ja", "Japanese"),
        ("jw", "Javanese"),
        ("ka", "Georgian"),
        ("kk", "Kazakh"),
        ("km", "Khmer"),
        ("kn", "Kannada"),
        ("ko", "Korean"),
        ("la", "Latin"),
        ("lb", "Luxembourgish"),
        ("ln", "Lingala"),
        ("lo", "Lao"),
        ("lt", "Lithuanian"),
        ("lv", "Latvian"),
        ("mg", "Malagasy"),
        ("mi", "Maori"),
        ("mk", "Macedonian"),
        ("ml", "Malayalam"),
        ("mn", "Mongolian"),
        ("mr", "Marathi"),
        ("ms", "Malay"),
        ("mt", "Maltese"),
        ("my", "Myanmar"),
        ("ne", "Nepali"),
        ("nl", "Dutch"),
        ("nn", "Nynorsk"),
        ("no", "Norwegian"),
        ("oc", "Occitan"),
        ("pa", "Panjabi"),
        ("pl", "Polish"),
        ("ps", "Pashto"),
        ("pt", "Portuguese"),
        ("ro", "Romanian"),
        ("ru", "Russian"),
        ("sa", "Sanskrit"),
        ("sd", "Sindhi"),
        ("si", "Sinhala"),
        ("sk", "Slovak"),
        ("sl", "Slovenian"),
        ("sn", "Shona"),
        ("so", "Somali"),
        ("sq", "Albanian"),
        ("sr", "Serbian"),
        ("su", "Sundanese"),
        ("sv", "Swedish"),
        ("sw", "Swahili"),
        ("ta", "Tamil"),
        ("te", "Telugu"),
        ("tg", "Tajik"),
        ("th", "Thai"),
        ("tk", "Turkmen"),
        ("tl", "Tagalog"),
        ("tr", "Turkish"),
        ("tt", "Tatar"),
        ("uk", "Ukrainian"),
        ("ur", "Urdu"),
        ("uz", "Uzbek"),
        ("vi", "Vietnamese"),
        ("yi", "Yiddish"),
        ("yo", "Yoruba"),
        ("yue", "Cantonese"),
        ("zh", "Chinese"),
    ];

    pub fn get_available_languages() -> Vec<(String, String)> {
        Self::LANGUAGES
            .iter()
            .map(|(code, name)| (code.to_string(), name.to_string()))
            .collect()
    }
}

/// Whether a model belongs to a dropdown category.
///
/// `General` is everything; the others are prefixes, with `turbo` standing alone
/// under Large because it is a large model without the name.
fn matches_category(category: &str, model: &str) -> bool {
    const PREFIXES: &[(&str, &[&str])] = &[
        ("Tiny", &["tiny"]),
        ("Base", &["base"]),
        ("Small", &["small"]),
        ("Medium", &["medium", "distil-medium"]),
        ("Large", &["large", "distil-large"]),
    ];
    if category == "Large" && model == "turbo" {
        return true;
    }
    if category == "General" {
        return true;
    }
    // An unknown category is treated like General, so a stale setting cannot
    // hide every model from the dropdown.
    match PREFIXES.iter().find(|(name, _)| *name == category) {
        Some((_, prefixes)) => prefixes.iter().any(|p| model.starts_with(p)),
        None => true,
    }
}
