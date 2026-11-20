//! `freespace shred` — overwrite and delete files for good.
//!
//! Irreversible overwriting is available on Windows only; on other platforms
//! the command reports `E_UNSUPPORTED` and the GUI/CLI advertise the capability
//! as unavailable.

use std::path::PathBuf;

use freespace_core::{Error, Result, size::Units};

use super::Outcome;

/// Runs `shred` over every path.
///
/// # Errors
/// Returns [`Error::Unsupported`] outside Windows. On Windows it returns
/// [`Error::ConfirmationRequired`] without `--yes`, [`Error::InvalidArgument`]
/// for an unknown pattern or zero passes, the first error when every file
/// failed, and [`Error::Partial`] when only some failed.
pub fn run(
    paths: &[PathBuf],
    passes: u32,
    pattern: &str,
    confirmed: bool,
    store: Option<&freespace_ops::Store>,
    units: Units,
) -> Result<Outcome> {
    if !freespace_platform::platform().capabilities.shred {
        return Err(Error::Unsupported(
            "shred is available on Windows only".to_owned(),
        ));
    }

    #[cfg(windows)]
    {
        supported::run(paths, passes, pattern, confirmed, store, units)
    }

    #[cfg(not(windows))]
    {
        let _ = (paths, passes, pattern, confirmed, store, units);
        Err(Error::Unsupported(
            "shred is available on Windows only".to_owned(),
        ))
    }
}

#[cfg(windows)]
mod supported {
    use std::path::PathBuf;

    use freespace_core::{
        Error, Result,
        size::{Units, format_bytes},
    };
    use freespace_ops::{
        ShredOptions,
        operation::ShredPattern,
        shred::{self, ShredOutcome},
    };
    use freespace_platform::paths;
    use serde_json::json;

    use super::Outcome;

    /// Runs `shred` over every path.
    pub(super) fn run(
        paths: &[PathBuf],
        passes: u32,
        pattern: &str,
        confirmed: bool,
        store: Option<&freespace_ops::Store>,
        units: Units,
    ) -> Result<Outcome> {
        let pattern = ShredPattern::parse_value(pattern).ok_or_else(|| {
            Error::InvalidArgument(format!(
                "unknown overwrite pattern `{pattern}`; expected `zeros` or `random`"
            ))
        })?;
        let options = ShredOptions { passes, pattern };
        options.validate()?;
        freespace_ops::confirm::require_confirmation(confirmed)?;

        let mut outcomes = Vec::new();
        let mut failures: Vec<(PathBuf, Error)> = Vec::new();
        for path in paths {
            match shred::shred(path, options, true) {
                Ok(outcome) => outcomes.push(outcome),
                Err(error) => failures.push((path.clone(), error)),
            }
        }

        let mut warnings = Vec::new();
        for outcome in &outcomes {
            warnings.extend(outcome.warnings.iter().cloned());
        }
        if !outcomes.is_empty() {
            if let Some(store) = store {
                let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned());
                if let Err(error) = shred::record(store, &outcomes, &user) {
                    warnings.push(format!("the run could not be journalled: {error}"));
                }
            }
        }

        if !failures.is_empty() {
            let (path, error) = failures.swap_remove(0);
            if outcomes.is_empty() {
                return Err(error);
            }

            return Err(Error::Partial(format!(
                "{} of {} files were shredded; first failure at `{}`: {error}",
                outcomes.len(),
                paths.len(),
                paths::display(&path)
            )));
        }

        Ok(Outcome::new(
            json!({
                "passes": passes,
                "pattern": pattern.as_str(),
                "items": outcomes.iter().map(item_json).collect::<Vec<_>>(),
            }),
            human(&outcomes, units),
        )
        .with_warnings(warnings))
    }

    fn item_json(outcome: &ShredOutcome) -> serde_json::Value {
        json!({
            "path": paths::display(&outcome.path),
            "size": outcome.size,
            "passes": outcome.passes,
            "pattern": outcome.pattern.as_str(),
            "storage": outcome.storage.as_str(),
            "trimmed": outcome.trimmed,
            "warnings": outcome.warnings,
        })
    }

    fn human(outcomes: &[ShredOutcome], units: Units) -> String {
        let mut text = format!("{} files destroyed\n", outcomes.len());
        for outcome in outcomes {
            text.push_str(&format!(
                "{:>12}  {}  {} passes, {}{}\n",
                format_bytes(outcome.size, units),
                paths::display(&outcome.path),
                outcome.passes,
                outcome.pattern.as_str(),
                if outcome.trimmed { ", trimmed" } else { "" }
            ));
        }

        text
    }
}

#[cfg(all(test, windows))]
mod tests {
    use freespace_core::size::Units;

    use super::run;

    fn fixture() -> (tempfile::TempDir, Vec<std::path::PathBuf>) {
        let directory = tempfile::tempdir().expect("temp dir");
        let first = directory.path().join("first.bin");
        let second = directory.path().join("second.bin");
        std::fs::write(&first, vec![1u8; 1024]).expect("first");
        std::fs::write(&second, vec![2u8; 2048]).expect("second");

        (directory, vec![first, second])
    }

    #[test]
    fn refuses_without_confirmation_and_keeps_the_files() {
        let (_directory, paths) = fixture();

        let error = run(&paths, 1, "zeros", false, None, Units::Binary).expect_err("refuse");

        assert_eq!(error.code().as_str(), "E_CONFIRMATION_REQUIRED");
        assert!(paths.iter().all(|path| path.exists()));
    }

    #[test]
    fn shreds_every_file() {
        let (_directory, paths) = fixture();

        let outcome = run(&paths, 2, "random", true, None, Units::Binary).expect("shred");

        assert!(paths.iter().all(|path| !path.exists()));
        assert_eq!(outcome.data["items"].as_array().expect("items").len(), 2);
        assert_eq!(outcome.data["passes"].as_u64(), Some(2));
        assert!(outcome.data["items"][0]["trimmed"].as_bool().is_some());
    }

    #[test]
    fn rejects_invalid_parameters() {
        let (_directory, paths) = fixture();

        let error = run(&paths, 1, "magic", true, None, Units::Binary).expect_err("pattern");
        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");

        let error = run(&paths, 0, "zeros", true, None, Units::Binary).expect_err("passes");
        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
        assert!(paths.iter().all(|path| path.exists()));
    }
}

#[cfg(all(test, unix))]
mod unix_tests {
    use freespace_core::{ErrorCode, size::Units};

    use super::run;

    #[test]
    fn shred_is_unsupported_on_unix() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("secret.bin");
        std::fs::write(&path, b"payload").expect("write");

        let error = run(
            std::slice::from_ref(&path),
            1,
            "zeros",
            true,
            None,
            Units::Binary,
        )
        .expect_err("unsupported");

        assert_eq!(error.code(), ErrorCode::Unsupported);
        assert!(path.exists());
    }
}
