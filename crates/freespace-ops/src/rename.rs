//! Rename rules based on regular expressions and case transforms
//! (FR-054…FR-056).

use freespace_core::{Error, Result};
use freespace_platform::paths;
use regex::Regex;

/// Case transformation applied after the replacement.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaseTransform {
    /// Keep the case produced by the replacement.
    #[default]
    Preserve,
    /// Lower-case the result.
    Lower,
    /// Upper-case the result.
    Upper,
}

impl CaseTransform {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preserve => "preserve",
            Self::Lower => "lower",
            Self::Upper => "upper",
        }
    }

    /// Parses a command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "preserve" | "keep" => Some(Self::Preserve),
            "lower" | "lowercase" => Some(Self::Lower),
            "upper" | "uppercase" => Some(Self::Upper),
            _ => None,
        }
    }
}

/// A compiled rename rule.
#[derive(Debug, Clone)]
pub struct RenameRule {
    pattern: String,
    replacement: String,
    case: CaseTransform,
    regex: Regex,
}

impl RenameRule {
    /// Compiles a rule.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] for a malformed regular expression.
    pub fn new(pattern: &str, replacement: &str, case: CaseTransform) -> Result<Self> {
        let regex = Regex::new(pattern).map_err(|error| {
            Error::InvalidArgument(format!("invalid rename pattern `{pattern}`: {error}"))
        })?;

        Ok(Self {
            pattern: pattern.to_owned(),
            replacement: replacement.to_owned(),
            case,
            regex,
        })
    }

    /// Original regular expression.
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Replacement template.
    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// Case transform.
    #[must_use]
    pub const fn case(&self) -> CaseTransform {
        self.case
    }

    /// Applies the rule to a file name and validates the result.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] when the result is empty, contains a
    /// forbidden character or is not a valid Windows name.
    pub fn apply(&self, name: &str) -> Result<String> {
        let replaced = self.regex.replace_all(name, self.replacement.as_str());
        let transformed = match self.case {
            CaseTransform::Preserve => replaced.into_owned(),
            CaseTransform::Lower => replaced.to_lowercase(),
            CaseTransform::Upper => replaced.to_uppercase(),
        };
        validate_name(&transformed)?;

        Ok(transformed)
    }

    /// Checks a resulting name without keeping the rule.
    ///
    /// # Errors
    /// Returns an error when the name is not a valid Windows file name.
    pub fn validate(name: &str) -> Result<()> {
        validate_name(name)
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidArgument(
            "a rename rule produced an empty name".to_owned(),
        ));
    }
    if name == "." || name == ".." {
        return Err(Error::InvalidArgument(
            "a rename rule produced a relative directory name".to_owned(),
        ));
    }
    if name.chars().any(|character| {
        matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        ) || character.is_control()
    }) {
        return Err(Error::InvalidArgument(format!(
            "`{name}` contains a character that is not allowed in a Windows name"
        )));
    }
    if name.ends_with(['.', ' ']) {
        return Err(Error::InvalidArgument(format!(
            "`{name}` ends with a dot or a space"
        )));
    }
    paths::validate_length(std::path::Path::new(name))?;
    if name.len() > 255 {
        return Err(Error::InvalidArgument(format!(
            "`{name}` is longer than 255 bytes"
        )));
    }

    Ok(())
}

/// Reserved device names that cannot be used as ordinary file names.
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Returns `true` for a Windows reserved device name (including `CON.txt`).
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();

    RESERVED.contains(&stem.as_str())
}

#[cfg(test)]
mod tests {
    use super::{CaseTransform, RenameRule, is_reserved};
    use freespace_core::ErrorCode;

    #[test]
    fn applies_regex_replacement_and_case() {
        let rule =
            RenameRule::new(r"^IMG_(\d+)$", "holiday-${1}", CaseTransform::Upper).expect("rule");

        assert_eq!(rule.apply("IMG_0042").expect("apply"), "HOLIDAY-0042");
        assert_eq!(rule.pattern(), r"^IMG_(\d+)$");
        assert_eq!(rule.replacement(), "holiday-${1}");
        assert_eq!(rule.case(), CaseTransform::Upper);
    }

    #[test]
    fn changes_extensions_and_lowercases() {
        let rule = RenameRule::new(r"(?i)\.jpeg$", ".jpg", CaseTransform::Lower).expect("rule");

        assert_eq!(rule.apply("Photo.JPEG").expect("apply"), "photo.jpg");
    }

    #[test]
    fn rejects_invalid_results_and_patterns() {
        assert!(RenameRule::new("[", "x", CaseTransform::Preserve).is_err());
        let rule = RenameRule::new(r".*", "", CaseTransform::Preserve).expect("rule");
        assert_eq!(
            rule.apply("name.txt").expect_err("empty").code(),
            ErrorCode::InvalidArgument
        );

        let rule = RenameRule::new(r".*", "a:b", CaseTransform::Preserve).expect("rule");
        assert!(rule.apply("name.txt").is_err());
        assert!(RenameRule::validate("trailing.").is_err());
        assert!(is_reserved("CON"));
        assert!(is_reserved("con.txt"));
        assert!(!is_reserved("console.txt"));
    }

    #[test]
    fn parses_case_names() {
        assert_eq!(
            CaseTransform::parse_value("lower"),
            Some(CaseTransform::Lower)
        );
        assert_eq!(
            CaseTransform::parse_value("UPPER"),
            Some(CaseTransform::Upper)
        );
        assert_eq!(CaseTransform::parse_value("magic"), None);
    }
}
