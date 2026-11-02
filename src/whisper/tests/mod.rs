//! The settings the manager validates and the catalogues it offers.

use crate::whisper::*;

#[test]
fn test_get_local_model_categories_returns_expected_categories() {
    let categories = WhisperManager::get_local_model_categories();
    assert_eq!(categories.len(), 6);
    assert!(categories.contains(&"General".to_string()));
    assert!(categories.contains(&"Tiny".to_string()));
    assert!(categories.contains(&"Base".to_string()));
    assert!(categories.contains(&"Small".to_string()));
    assert!(categories.contains(&"Medium".to_string()));
    assert!(categories.contains(&"Large".to_string()));
}

#[test]
fn test_get_local_models_for_category_general_returns_all_models() {
    let models = WhisperManager::get_local_models_for_category("General");
    let all_models = WhisperManager::get_available_local_models();
    assert_eq!(models, all_models);
}

#[test]
fn test_get_local_models_for_category_tiny_filters_correctly() {
    let models = WhisperManager::get_local_models_for_category("Tiny");
    assert_eq!(models.len(), 2);
    assert!(models.contains(&"tiny".to_string()));
    assert!(models.contains(&"tiny.en".to_string()));
}

#[test]
fn test_get_local_models_for_category_base_filters_correctly() {
    let models = WhisperManager::get_local_models_for_category("Base");
    assert_eq!(models.len(), 2);
    assert!(models.contains(&"base".to_string()));
    assert!(models.contains(&"base.en".to_string()));
}

#[test]
fn test_get_local_models_for_category_small_filters_correctly() {
    let models = WhisperManager::get_local_models_for_category("Small");
    assert_eq!(models.len(), 2);
    assert!(models.contains(&"small".to_string()));
    assert!(models.contains(&"small.en".to_string()));
}

#[test]
fn test_get_local_models_for_category_medium_filters_correctly() {
    let models = WhisperManager::get_local_models_for_category("Medium");
    assert!(models.contains(&"medium".to_string()));
    assert!(models.contains(&"medium.en".to_string()));
    assert!(models.contains(&"distil-medium.en".to_string()));
}

#[test]
fn test_get_local_models_for_category_large_filters_correctly() {
    let models = WhisperManager::get_local_models_for_category("Large");
    assert!(models.contains(&"large-v1".to_string()));
    assert!(models.contains(&"large-v2".to_string()));
    assert!(models.contains(&"large-v3".to_string()));
    assert!(models.contains(&"turbo".to_string()));
    assert!(models.contains(&"distil-large-v2".to_string()));
    assert!(models.contains(&"distil-large-v3".to_string()));
}

#[test]
fn test_get_local_models_for_category_unknown_returns_all() {
    let models = WhisperManager::get_local_models_for_category("Unknown");
    let all_models = WhisperManager::get_available_local_models();
    assert_eq!(models, all_models);
}

#[test]
fn test_get_available_local_models_returns_expected_count() {
    let models = WhisperManager::get_available_local_models();
    assert_eq!(models.len(), 16);
}

#[test]
fn test_get_model_info_returns_info_for_tiny() {
    let info = WhisperManager::get_model_info("tiny");
    assert_eq!(info.size, "75 MB");
    assert_eq!(info.memory, "~390 MB");
    assert_eq!(info.speed, "~10x");
    assert_eq!(info.quality, "Low");
}

#[test]
fn test_get_model_info_returns_info_for_base() {
    let info = WhisperManager::get_model_info("base");
    assert_eq!(info.size, "142 MB");
    assert_eq!(info.memory, "~500 MB");
    assert_eq!(info.speed, "~7x");
    assert_eq!(info.quality, "Low-Medium");
}

#[test]
fn test_get_model_info_returns_info_for_unknown() {
    let info = WhisperManager::get_model_info("unknown");
    assert_eq!(info.size, "Unknown");
    assert_eq!(info.memory, "Unknown");
    assert_eq!(info.speed, "Unknown");
    assert_eq!(info.quality, "Unknown");
}

#[test]
fn test_get_available_api_models_for_provider_openai() {
    let models = WhisperManager::get_available_api_models_for_provider("openai");
    assert!(models.contains(&"gpt-4o-transcribe".to_string()));
    assert!(models.contains(&"gpt-4o-mini-transcribe".to_string()));
}

#[test]
fn test_get_available_api_models_for_provider_groq() {
    let models = WhisperManager::get_available_api_models_for_provider("groq");
    assert!(models.contains(&"whisper-large-v3-turbo".to_string()));
    assert!(models.contains(&"whisper-large-v3".to_string()));
}

#[test]
fn test_get_available_api_models_for_provider_gemini() {
    let models = WhisperManager::get_available_api_models_for_provider("gemini");
    assert!(models.contains(&"gemini-2.0-flash".to_string()));
    assert!(models.contains(&"gemini-1.5-flash".to_string()));
}

#[test]
fn test_get_available_api_models_for_provider_unknown_defaults_to_openai() {
    let models = WhisperManager::get_available_api_models_for_provider("unknown");
    let openai_models = WhisperManager::get_available_api_models_for_provider("openai");
    assert_eq!(models, openai_models);
}

#[test]
fn test_whisper_manager_new_has_default_values() {
    let mut manager = WhisperManager::new();
    let result = manager.set_mode("api");
    assert!(result.is_ok());
}

#[test]
fn test_set_mode_accepts_valid_modes() {
    let mut manager = WhisperManager::new();
    assert!(manager.set_mode("api").is_ok());
    assert!(manager.set_mode("local").is_ok());
}

#[test]
fn test_set_mode_rejects_invalid_mode() {
    let mut manager = WhisperManager::new();
    assert!(manager.set_mode("invalid").is_err());
}
