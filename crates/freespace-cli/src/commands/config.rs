//! `freespace config get|set` — read and write configuration values.

use std::path::{Path, PathBuf};

use freespace_core::{
    Error, Result,
    settings::{self, Settings},
};
use freespace_platform::paths;
use serde_json::json;

use crate::cli::ConfigAction;

use super::Outcome;

/// Runs a configuration action.
///
/// # Errors
/// Returns an error for unknown keys, malformed values or I/O failures.
pub fn run(config_path: Option<&Path>, action: &ConfigAction) -> Result<Outcome> {
    let path = resolve_path(config_path)?;
    let display_path = paths::display(&path);

    match action {
        ConfigAction::Get { key } => {
            let settings = Settings::load(&path)?;
            let value = settings.get_value(key).ok_or_else(|| unknown_key(key))?;

            Ok(Outcome::new(
                json!({ "key": key, "value": value, "path": display_path }),
                format!("{key} = {value}\n"),
            ))
        }
        ConfigAction::Set { key, value } => {
            if key == "context_menu_enabled" && !cfg!(feature = "context-menu") {
                return Err(Error::Unsupported(
                    "the context menu is not available in this portable build".to_owned(),
                ));
            }

            let mut settings = Settings::load(&path)?;
            settings.set_value(key, value)?;
            settings.save(&path)?;
            let stored = settings.get_value(key).unwrap_or_else(|| value.clone());
            let warnings = apply_context_menu_setting(key, &settings);

            Ok(Outcome::new(
                json!({ "key": key, "value": stored, "path": display_path }),
                format!("{key} = {stored}\n"),
            )
            .with_warnings(warnings))
        }
    }
}

fn apply_context_menu_setting(key: &str, settings: &Settings) -> Vec<String> {
    #[cfg(feature = "context-menu")]
    {
        if key == "context_menu_enabled" {
            if let Err(error) = freespace_platform::context_menu::ContextMenu::set_enabled(
                settings.context_menu_enabled,
            ) {
                return vec![format!(
                    "the setting was saved, but the context menu could not be updated: {error}"
                )];
            }
        }
    }

    #[cfg(not(feature = "context-menu"))]
    {
        let _ = (key, settings);
    }

    Vec::new()
}

fn resolve_path(config_path: Option<&Path>) -> Result<PathBuf> {
    match config_path {
        Some(path) => Ok(path.to_path_buf()),
        None => settings::default_path(),
    }
}

fn unknown_key(key: &str) -> Error {
    Error::InvalidArgument(format!(
        "unknown configuration key `{key}`; expected one of {}",
        settings::CONFIG_KEYS.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use freespace_core::ErrorCode;

    use super::run;
    use crate::cli::ConfigAction;

    #[test]
    fn reads_and_writes_values() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("settings.toml");

        let set = run(
            Some(&path),
            &ConfigAction::Set {
                key: "language".to_owned(),
                value: "ru".to_owned(),
            },
        )
        .expect("set");
        assert_eq!(set.data["value"], "ru");

        let get = run(
            Some(&path),
            &ConfigAction::Get {
                key: "language".to_owned(),
            },
        )
        .expect("get");
        assert_eq!(get.data["value"], "ru");
        assert!(get.human.contains("language = ru"));
    }

    #[test]
    fn rejects_unknown_keys() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("settings.toml");
        let error = run(
            Some(&path),
            &ConfigAction::Get {
                key: "nope".to_owned(),
            },
        )
        .expect_err("unknown key");

        assert_eq!(error.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn rejects_invalid_values() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("settings.toml");
        let error = run(
            Some(&path),
            &ConfigAction::Set {
                key: "theme".to_owned(),
                value: "neon".to_owned(),
            },
        )
        .expect_err("bad theme");

        assert_eq!(error.code(), ErrorCode::InvalidArgument);
    }
}
