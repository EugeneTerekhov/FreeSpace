//! Locale selection and runtime language switching.
//!
//! Translation tables are embedded at compile time by the `rust_i18n::i18n!`
//! macro invoked in the crate root; this module exposes the supported locales
//! and switches the process-wide active language. Switching happens at runtime
//! and never resets application state, which satisfies FR-092.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Locale shipped with FreeSpace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    /// Russian.
    Ru,
    /// English.
    En,
}

impl Locale {
    /// Every locale supported in the first release.
    pub const ALL: [Self; 2] = [Self::Ru, Self::En];

    /// Locale used when no preference has been stored yet.
    pub const DEFAULT: Self = Self::En;

    /// Returns the BCP 47 code stored in settings and used by translation files.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
        }
    }

    /// Parses a locale code, ignoring region and case (`ru-RU` becomes [`Locale::Ru`]).
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        let primary = code.split(['-', '_']).next().unwrap_or_default();

        match primary.to_ascii_lowercase().as_str() {
            "ru" => Some(Self::Ru),
            "en" => Some(Self::En),
            _ => None,
        }
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Applies a locale to the process-wide translation backend.
pub fn set_locale(locale: Locale) {
    rust_i18n::set_locale(locale.code());
}

/// Applies a locale given by code.
///
/// Returns the applied locale, or `None` when the code is not supported; the
/// active language is left untouched in that case.
pub fn set_locale_from_code(code: &str) -> Option<Locale> {
    let locale = Locale::from_code(code)?;
    set_locale(locale);

    Some(locale)
}

/// Returns the currently active locale, falling back to [`Locale::DEFAULT`].
#[must_use]
pub fn locale() -> Locale {
    let active = rust_i18n::locale();

    Locale::from_code(&active).unwrap_or(Locale::DEFAULT)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::Locale;

    /// Locale switching is process-wide, so tests that change it must not run
    /// concurrently with each other.
    static LOCALE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parses_supported_codes() {
        assert_eq!(Locale::from_code("ru"), Some(Locale::Ru));
        assert_eq!(Locale::from_code("EN"), Some(Locale::En));
        assert_eq!(Locale::from_code("ru-RU"), Some(Locale::Ru));
    }

    #[test]
    fn rejects_unknown_codes() {
        assert_eq!(Locale::from_code("de"), None);
        assert_eq!(Locale::from_code(""), None);
    }

    #[test]
    fn loads_embedded_translations() {
        let _guard = LOCALE_LOCK.lock().expect("locale lock");
        rust_i18n::set_locale("en");
        assert_eq!(t!("app.name").to_string(), "FreeSpace");

        rust_i18n::set_locale("ru");
        assert_eq!(
            t!("app.tagline").to_string(),
            "Анализатор и менеджер дискового пространства для Windows"
        );

        rust_i18n::set_locale("en");
    }

    #[test]
    fn switches_language_at_runtime() {
        let _guard = LOCALE_LOCK.lock().expect("locale lock");
        super::set_locale(Locale::Ru);
        assert_eq!(super::locale(), Locale::Ru);
        assert_eq!(t!("common.cancel").to_string(), "Отмена");

        let applied = super::set_locale_from_code("en-GB");

        assert_eq!(applied, Some(Locale::En));
        assert_eq!(super::locale(), Locale::En);
        assert_eq!(t!("common.cancel").to_string(), "Cancel");
        assert_eq!(super::set_locale_from_code("de"), None);
    }

    #[test]
    fn round_trips_locale_through_serde() {
        let json = serde_json::to_string(&Locale::Ru).expect("serialise locale");

        assert_eq!(json, "\"ru\"");
        assert_eq!(
            serde_json::from_str::<Locale>(&json).expect("deserialise locale"),
            Locale::Ru
        );
    }
}
