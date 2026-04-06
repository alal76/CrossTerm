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
    if !available.iter().any(|l| l.code == locale) {
        return Err(L10nError::UnsupportedLocale(locale));
    }
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
    if !available.iter().any(|l| l.code == locale) {
        return Err(L10nError::UnsupportedLocale(locale.clone()));
    }
    drop(available);

    let mut entries = default_translations_for(&locale);

    // Merge custom translations on top
    let custom = state.custom_translations.lock().unwrap();
    if let Some(overrides) = custom.get(&locale) {
        for (k, v) in overrides {
            entries.insert(k.clone(), v.clone());
        }
    }

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
    if !available.iter().any(|l| l.code == locale) {
        return Err(L10nError::UnsupportedLocale(locale.clone()));
    }
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
    if !available.iter().any(|l| l.code == locale) {
        return Err(L10nError::UnsupportedLocale(locale.clone()));
    }
    drop(available);

    let mut entries = default_translations_for(&locale);
    let custom = state.custom_translations.lock().unwrap();
    if let Some(overrides) = custom.get(&locale) {
        for (k, v) in overrides {
            entries.insert(k.clone(), v.clone());
        }
    }

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
    if !available.iter().any(|l| l.code == locale) {
        return Err(L10nError::UnsupportedLocale(locale.clone()));
    }
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
    let code = locale
        .split('_')
        .next()
        .unwrap_or("en")
        .to_string();

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
}
