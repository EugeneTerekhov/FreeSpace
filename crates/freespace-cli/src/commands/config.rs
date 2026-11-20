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
            if key == "context_menu_enabled"
                && !freespace_platform::platform().capabilities.context_menu
            {
                return Err(Error::Unsupported(
                    "the Explorer context menu is available on Windows only".to_owned(),
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
    if key != "context_menu_enabled" {
        return Vec::new();
    }
    let platform = freespace_platform::platform();
    if !platform.capabilities.context_menu {
        return Vec::new();
    }
    match platform
        .context_menu
        .set_enabled(settings.context_menu_enabled)
    {
        Ok(()) => Vec::new(),
        Err(error) => vec![format!(
            "the setting was saved, but the context menu could not be updated: {error}"
        )],
    }
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

    #[cfg(unix)]
    #[test]
    fn rejects_the_context_menu_setting_on_unix() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("settings.toml");

        let error = run(
            Some(&path),
            &ConfigAction::Set {
                key: "context_menu_enabled".to_owned(),
                value: "true".to_owned(),
            },
        )
        .expect_err("unsupported");

        assert_eq!(error.code(), ErrorCode::Unsupported);
        assert!(!path.exists(), "nothing may be written");
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
