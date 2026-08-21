use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum L10nError {
    #[error("Unsupported locale: {0}")]
    UnsupportedLocale(String),
    #[error("Missing translation key: {0}")]
    MissingKey(String),
    #[error("Failed to load translations: {0}")]
    LoadError(String),
    #[error("Failed to export translations: {0}")]
    ExportError(String),
}

impl Serialize for L10nError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleInfo {
    pub code: String,
    pub name: String,
    pub native_name: String,
    pub rtl: bool,
    pub completeness: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationEntry {
    pub key: String,
    pub value: String,
    pub description: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationBundle {
    pub locale: String,
    pub entries: HashMap<String, String>,
    pub version: String,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct L10nState {
    current_locale: Mutex<String>,
    available_locales: Mutex<Vec<LocaleInfo>>,
    custom_translations: Mutex<HashMap<String, HashMap<String, String>>>,
}

impl L10nState {
    pub fn new() -> Self {
        let locales = vec![
            LocaleInfo {
                code: "en".to_string(),
                name: "English".to_string(),
                native_name: "English".to_string(),
                rtl: false,
                completeness: 1.0,
            },
            LocaleInfo {
                code: "fr".to_string(),
                name: "French".to_string(),
                native_name: "Français".to_string(),
                rtl: false,
                completeness: 0.85,
            },
            LocaleInfo {
                code: "de".to_string(),
                name: "German".to_string(),
                native_name: "Deutsch".to_string(),
                rtl: false,
                completeness: 0.80,
            },
            LocaleInfo {
                code: "ja".to_string(),
                name: "Japanese".to_string(),
                native_name: "日本語".to_string(),
                rtl: false,
                completeness: 0.70,
            },
            LocaleInfo {
                code: "zh".to_string(),
                name: "Chinese".to_string(),
                native_name: "中文".to_string(),
                rtl: false,
                completeness: 0.65,
            },
            LocaleInfo {
                code: "ar".to_string(),
                name: "Arabic".to_string(),
                native_name: "العربية".to_string(),
                rtl: true,
                completeness: 0.55,
            },
            LocaleInfo {
                code: "he".to_string(),
                name: "Hebrew".to_string(),
                native_name: "עברית".to_string(),
                rtl: true,
                completeness: 0.50,
            },
            LocaleInfo {
                code: "es".to_string(),
                name: "Spanish".to_string(),
                native_name: "Español".to_string(),
                rtl: false,
                completeness: 0.75,
            },
            LocaleInfo {
                code: "pt".to_string(),
                name: "Portuguese".to_string(),
                native_name: "Português".to_string(),
                rtl: false,
                completeness: 0.60,
            },
            LocaleInfo {
                code: "ko".to_string(),
                name: "Korean".to_string(),
                native_name: "한국어".to_string(),
                rtl: false,
                completeness: 0.55,
            },
        ];

        Self {
            current_locale: Mutex::new("en".to_string()),
            available_locales: Mutex::new(locales),
            custom_translations: Mutex::new(HashMap::new()),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn default_translations_for(locale: &str) -> HashMap<String, String> {
    let mut entries = HashMap::new();
    // Provide a minimal set of default translations for the locale
    match locale {
        "en" => {
            entries.insert("app.name".to_string(), "CrossTerm".to_string());
            entries.insert("app.tagline".to_string(), "Terminal Emulator & Remote Access Suite".to_string());
        }
        "fr" => {
            entries.insert("app.name".to_string(), "CrossTerm".to_string());
            entries.insert("app.tagline".to_string(), "Émulateur de terminal & suite d'accès distant".to_string());
        }
        "ar" => {
            entries.insert("app.name".to_string(), "CrossTerm".to_string());
            entries.insert("app.tagline".to_string(), "محاكي طرفية ومجموعة وصول عن بعد".to_string());
        }
        "he" => {
            entries.insert("app.name".to_string(), "CrossTerm".to_string());
            entries.insert("app.tagline".to_string(), "אמולטור מסוף וחבילת גישה מרחוק".to_string());
        }
        _ => {
            entries.insert("app.name".to_string(), "CrossTerm".to_string());
        }
    }
    entries
}

/// Returns `Ok(())` if `locale` is one of the codes in `available`, or a
/// descriptive `UnsupportedLocale` error otherwise. Split out from the
/// individual Tauri commands (which all perform this exact check) so the
/// validation logic is unit-testable without a live `L10nState`.
fn validate_locale(available: &[LocaleInfo], locale: &str) -> Result<(), L10nError> {
    if available.iter().any(|l| l.code == locale) {
        Ok(())
    } else {
        Err(L10nError::UnsupportedLocale(locale.to_string()))
    }
}

/// Layer any custom-translation overrides for a locale on top of its
/// built-in defaults. Overrides win on key collision.
fn merge_translations(
    mut entries: HashMap<String, String>,
    overrides: Option<&HashMap<String, String>>,
) -> HashMap<String, String> {
    if let Some(overrides) = overrides {
        for (k, v) in overrides {
            entries.insert(k.clone(), v.clone());
        }
    }
    entries
}

/// Extract the bare language code from a POSIX-style locale string, e.g.
/// `"en_US.UTF-8"` -> `"en"`, `"fr_FR"` -> `"fr"`. Falls back to `"en"` for
/// an empty input (mirrors the caller's env-var-missing fallback).
fn extract_language_code(locale_env: &str) -> String {
    locale_env
        .split('_')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("en")
        .to_string()
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn l10n_list_locales(
    state: tauri::State<'_, L10nState>,
) -> Result<Vec<LocaleInfo>, L10nError> {
    let locales = state.available_locales.lock().unwrap();
    Ok(locales.clone())
}

#[tauri::command]
pub fn l10n_get_locale(
    state: tauri::State<'_, L10nState>,
) -> Result<String, L10nError> {
    let locale = state.current_locale.lock().unwrap();
    Ok(locale.clone())
}

#[tauri::command]
pub fn l10n_set_locale(
    state: tauri::State<'_, L10nState>,
    locale: String,
) -> Result<(), L10nError> {
    let available = state.available_locales.lock().unwrap();
    validate_locale(&available, &locale)?;
    drop(available);

    let mut current = state.current_locale.lock().unwrap();
    *current = locale;
    Ok(())
}

#[tauri::command]
pub fn l10n_get_translations(
    state: tauri::State<'_, L10nState>,
    locale: String,
) -> Result<TranslationBundle, L10nError> {
    let available = state.available_locales.lock().unwrap();
    validate_locale(&available, &locale)?;
    drop(available);

    let defaults = default_translations_for(&locale);

    // Merge custom translations on top
    let custom = state.custom_translations.lock().unwrap();
    let entries = merge_translations(defaults, custom.get(&locale));

    Ok(TranslationBundle {
        locale,
        entries,
        version: "1.0.0".to_string(),
    })
}

#[tauri::command]
pub fn l10n_set_custom_translation(
    state: tauri::State<'_, L10nState>,
    locale: String,
    key: String,
    value: String,
) -> Result<(), L10nError> {
    let available = state.available_locales.lock().unwrap();
    validate_locale(&available, &locale)?;
    drop(available);

    let mut custom = state.custom_translations.lock().unwrap();
    custom
        .entry(locale)
        .or_default()
        .insert(key, value);
    Ok(())
}

#[tauri::command]
pub fn l10n_export_translations(
    state: tauri::State<'_, L10nState>,
    locale: String,
) -> Result<String, L10nError> {
    let available = state.available_locales.lock().unwrap();
    validate_locale(&available, &locale)?;
    drop(available);

    let defaults = default_translations_for(&locale);
    let custom = state.custom_translations.lock().unwrap();
    let entries = merge_translations(defaults, custom.get(&locale));

    serde_json::to_string_pretty(&entries)
        .map_err(|e| L10nError::ExportError(e.to_string()))
}

#[tauri::command]
pub fn l10n_import_translations(
    state: tauri::State<'_, L10nState>,
    locale: String,
    data: String,
) -> Result<u32, L10nError> {
    let available = state.available_locales.lock().unwrap();
    validate_locale(&available, &locale)?;
    drop(available);

    let imported: HashMap<String, String> = serde_json::from_str(&data)
        .map_err(|e| L10nError::LoadError(e.to_string()))?;

    let count = imported.len() as u32;
    let mut custom = state.custom_translations.lock().unwrap();
    let entry = custom.entry(locale).or_default();
    for (k, v) in imported {
        entry.insert(k, v);
    }

    Ok(count)
}

#[tauri::command]
pub fn l10n_get_completeness(
    state: tauri::State<'_, L10nState>,
    locale: String,
) -> Result<f64, L10nError> {
    let available = state.available_locales.lock().unwrap();
    let info = available
        .iter()
        .find(|l| l.code == locale)
        .ok_or_else(|| L10nError::UnsupportedLocale(locale.clone()))?;
    Ok(info.completeness)
}

#[tauri::command]
pub fn l10n_detect_system_locale() -> Result<String, L10nError> {
    // Use the LANG or LC_ALL environment variable, falling back to "en"
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_else(|_| "en_US.UTF-8".to_string());

    // Extract the language code (e.g., "en" from "en_US.UTF-8")
    let code = extract_language_code(&locale);

    Ok(code)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> L10nState {
        L10nState::new()
    }

    #[test]
    fn test_locale_crud() {
        let state = make_state();

        // List locales
        let locales = state.available_locales.lock().unwrap();
        assert!(locales.len() >= 7);
        assert!(locales.iter().any(|l| l.code == "en"));
        assert!(locales.iter().any(|l| l.code == "ar"));
        drop(locales);

        // Set locale
        {
            let mut current = state.current_locale.lock().unwrap();
            *current = "fr".to_string();
        }

        // Get locale
        {
            let current = state.current_locale.lock().unwrap();
            assert_eq!(*current, "fr");
        }

        // Setting unsupported locale should fail
        let available = state.available_locales.lock().unwrap();
        let has_klingon = available.iter().any(|l| l.code == "tlh");
        assert!(!has_klingon);
    }

    #[test]
    fn test_custom_translations() {
        let state = make_state();

        // Set a custom translation
        {
            let mut custom = state.custom_translations.lock().unwrap();
            custom
                .entry("en".to_string())
                .or_default()
                .insert("greeting".to_string(), "Hello, CrossTerm!".to_string());
        }

        // Get translation bundle and verify override
        let mut entries = default_translations_for("en");
        let custom = state.custom_translations.lock().unwrap();
        if let Some(overrides) = custom.get("en") {
            for (k, v) in overrides {
                entries.insert(k.clone(), v.clone());
            }
        }

        assert_eq!(entries.get("greeting").unwrap(), "Hello, CrossTerm!");
        assert_eq!(entries.get("app.name").unwrap(), "CrossTerm");
    }

    #[test]
    fn test_locale_info() {
        let state = make_state();
        let locales = state.available_locales.lock().unwrap();

        // Arabic is RTL
        let arabic = locales.iter().find(|l| l.code == "ar").unwrap();
        assert!(arabic.rtl);
        assert_eq!(arabic.native_name, "العربية");

        // Hebrew is RTL
        let hebrew = locales.iter().find(|l| l.code == "he").unwrap();
        assert!(hebrew.rtl);
        assert_eq!(hebrew.native_name, "עברית");

        // English is LTR
        let english = locales.iter().find(|l| l.code == "en").unwrap();
        assert!(!english.rtl);
        assert_eq!(english.completeness, 1.0);

        // French is LTR
        let french = locales.iter().find(|l| l.code == "fr").unwrap();
        assert!(!french.rtl);
    }

    // ── default_translations_for ─────────────────────────────────────

    #[test]
    fn test_default_translations_for_en() {
        let entries = default_translations_for("en");
        assert_eq!(entries.get("app.name").unwrap(), "CrossTerm");
        assert!(entries.contains_key("app.tagline"));
    }

    #[test]
    fn test_default_translations_for_fr_ar_he() {
        let fr = default_translations_for("fr");
        assert!(fr.get("app.tagline").unwrap().contains("distant"));

        let ar = default_translations_for("ar");
        assert_eq!(ar.get("app.name").unwrap(), "CrossTerm");
        assert!(ar.contains_key("app.tagline"));

        let he = default_translations_for("he");
        assert_eq!(he.get("app.name").unwrap(), "CrossTerm");
        assert!(he.contains_key("app.tagline"));
    }

    #[test]
    fn test_default_translations_for_unmapped_locale_falls_back() {
        // "de", "ja", etc. hit the catch-all `_` branch: only app.name, no tagline.
        let de = default_translations_for("de");
        assert_eq!(de.get("app.name").unwrap(), "CrossTerm");
        assert!(!de.contains_key("app.tagline"));

        let unknown = default_translations_for("xx");
        assert_eq!(unknown.get("app.name").unwrap(), "CrossTerm");
        assert_eq!(unknown.len(), 1);
    }

    // ── validate_locale ───────────────────────────────────────────────

    #[test]
    fn test_validate_locale_accepts_known_code() {
        let state = make_state();
        let available = state.available_locales.lock().unwrap();
        assert!(validate_locale(&available, "en").is_ok());
        assert!(validate_locale(&available, "ar").is_ok());
    }

    #[test]
    fn test_validate_locale_rejects_unknown_code() {
        let state = make_state();
        let available = state.available_locales.lock().unwrap();
        let err = validate_locale(&available, "tlh").unwrap_err();
        assert!(matches!(err, L10nError::UnsupportedLocale(ref c) if c == "tlh"));
    }

    // ── merge_translations ───────────────────────────────────────────

    #[test]
    fn test_merge_translations_overrides_win() {
        let mut defaults = HashMap::new();
        defaults.insert("app.name".to_string(), "CrossTerm".to_string());
        defaults.insert("app.tagline".to_string(), "Original".to_string());

        let mut overrides = HashMap::new();
        overrides.insert("app.tagline".to_string(), "Custom Tagline".to_string());
        overrides.insert("greeting".to_string(), "Hi!".to_string());

        let merged = merge_translations(defaults, Some(&overrides));
        assert_eq!(merged.get("app.name").unwrap(), "CrossTerm");
        assert_eq!(merged.get("app.tagline").unwrap(), "Custom Tagline");
        assert_eq!(merged.get("greeting").unwrap(), "Hi!");
    }

    #[test]
    fn test_merge_translations_no_overrides_returns_defaults_unchanged() {
        let mut defaults = HashMap::new();
        defaults.insert("app.name".to_string(), "CrossTerm".to_string());
        let merged = merge_translations(defaults.clone(), None);
        assert_eq!(merged, defaults);
    }

    // ── extract_language_code ────────────────────────────────────────

    #[test]
    fn test_extract_language_code_variants() {
        assert_eq!(extract_language_code("en_US.UTF-8"), "en");
        assert_eq!(extract_language_code("fr_FR"), "fr");
        assert_eq!(extract_language_code("de"), "de");
        assert_eq!(extract_language_code(""), "en");
    }

    // ── l10n_detect_system_locale ────────────────────────────────────

    #[test]
    fn test_l10n_detect_system_locale_returns_two_letter_code() {
        // Whatever the CI/dev machine's actual LANG is, the result should
        // always be a short language code, never the raw "xx_YY.ENCODING" string.
        let code = l10n_detect_system_locale().unwrap();
        assert!(!code.is_empty());
        assert!(!code.contains('.'), "should strip encoding suffix: {code}");
        assert!(!code.contains('_'), "should strip region suffix: {code}");
    }

    // ── L10nError ──────────────────────────────────────────────────────

    #[test]
    fn test_l10n_error_display() {
        assert_eq!(
            L10nError::UnsupportedLocale("xx".into()).to_string(),
            "Unsupported locale: xx"
        );
        assert_eq!(
            L10nError::MissingKey("app.name".into()).to_string(),
            "Missing translation key: app.name"
        );
        assert_eq!(
            L10nError::LoadError("bad json".into()).to_string(),
            "Failed to load translations: bad json"
        );
        assert_eq!(
            L10nError::ExportError("disk full".into()).to_string(),
            "Failed to export translations: disk full"
        );
    }

    #[test]
    fn test_l10n_error_serialize() {
        let err = L10nError::UnsupportedLocale("xx".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Unsupported locale: xx\"");
    }

    // ── TranslationBundle / LocaleInfo serde ─────────────────────────

    #[test]
    fn test_translation_bundle_serde_roundtrip() {
        let mut entries = HashMap::new();
        entries.insert("k".to_string(), "v".to_string());
        let bundle = TranslationBundle {
            locale: "en".to_string(),
            entries,
            version: "1.0.0".to_string(),
        };
        let json = serde_json::to_string(&bundle).unwrap();
        let de: TranslationBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(de.locale, "en");
        assert_eq!(de.entries.get("k").unwrap(), "v");
        assert_eq!(de.version, "1.0.0");
    }
}
