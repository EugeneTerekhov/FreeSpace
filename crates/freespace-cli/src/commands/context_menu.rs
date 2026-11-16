//! `freespace context-menu install|uninstall` (FR-079…FR-081).
//!
//! Installing or removing the current-user Explorer integration also flips the
//! persistent `context_menu_enabled` setting, so the GUI settings screen and the
//! CLI stay in agreement. A machine-wide installation is a separate
//! administrator action. Removing the integration is idempotent and is used by
//! the uninstaller to clean up the registry.

use std::path::Path;

use freespace_core::{
    Result,
    settings::{self, Settings},
};
use freespace_platform::context_menu::{ContextMenu, MenuScope};
use serde_json::json;

use crate::cli::ContextMenuAction;

use super::Outcome;

/// Installs or removes the Explorer integration.
///
/// # Errors
/// Returns an error when the executable path cannot be resolved, the registry
/// cannot be written, or the machine-wide scope is not permitted.
pub fn run(
    action: &ContextMenuAction,
    executable: Option<&Path>,
    config_path: Option<&Path>,
) -> Result<Outcome> {
    match action {
        ContextMenuAction::Install {
            machine,
            executable: action_executable,
        } => {
            let executable = match action_executable.as_deref().or(executable) {
                Some(path) => path.to_path_buf(),
                None => ContextMenu::preferred_executable()?,
            };
            let scope = scope(*machine);
            let menu = ContextMenu::for_scope(scope);
            let entries = menu.install(&executable)?;
            // The persistent setting describes the current-user integration.
            // A machine-wide installation is a separate administrator action.
            let warning = if *machine {
                None
            } else {
                persist_setting(config_path, true).err().map(|error| {
                    format!("the integration is installed, but the setting was not saved: {error}")
                })
            };

            let data = json!({
                "scope": scope.as_str(),
                "executable": freespace_platform::paths::display(&executable),
                "installed": true,
                "entries": entries
                    .iter()
                    .map(|entry| json!({
                        "target": entry.target.class(),
                        "action": entry.action.key(),
                        "key": entry.key,
                        "command": entry.command,
                    }))
                    .collect::<Vec<_>>(),
            });
            let human = format!(
                "Installed {} context-menu entries for {}.\n",
                entries.len(),
                scope_description(scope)
            );

            Ok(Outcome::new(data, human).with_warnings(warning.into_iter().collect()))
        }
        ContextMenuAction::Uninstall { machine, all } => {
            let scopes: Vec<MenuScope> = if *all {
                vec![MenuScope::User, MenuScope::Machine]
            } else {
                vec![scope(*machine)]
            };
            let mut warnings = Vec::new();
            for scope in &scopes {
                match ContextMenu::for_scope(*scope).uninstall() {
                    Ok(()) => {}
                    Err(error) if scope.requires_elevation() => warnings.push(format!(
                        "cannot remove the machine-wide integration: {error}"
                    )),
                    Err(error) => return Err(error),
                }
            }
            if !*machine || *all {
                if let Err(error) = persist_setting(config_path, false) {
                    warnings.push(format!(
                        "the integration is removed, but the setting was not saved: {error}"
                    ));
                }
            }

            let data = json!({
                "scope": if *all { "all" } else { scope(*machine).as_str() },
                "installed": false,
            });
            let human = if *all {
                "Removed the context-menu entries for the current user and all users.\n".to_owned()
            } else {
                format!(
                    "Removed the context-menu entries for {}.\n",
                    scope_description(scope(*machine))
                )
            };

            Ok(Outcome::new(data, human).with_warnings(warnings))
        }
    }
}

/// Stores `context_menu_enabled` in the active settings file.
fn persist_setting(config_path: Option<&Path>, enabled: bool) -> Result<()> {
    let path = match config_path {
        Some(path) => path.to_path_buf(),
        None => settings::default_path()?,
    };
    let mut settings = Settings::load(&path)?;
    settings.context_menu_enabled = enabled;
    settings.save(&path)
}

const fn scope(machine: bool) -> MenuScope {
    if machine {
        MenuScope::Machine
    } else {
        MenuScope::User
    }
}

const fn scope_description(scope: MenuScope) -> &'static str {
    match scope {
        MenuScope::User => "the current user",
        MenuScope::Machine => "all users",
    }
}
