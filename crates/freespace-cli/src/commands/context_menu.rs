//! `freespace context-menu install|uninstall` (FR-080, FR-081).

use std::path::Path;

use freespace_core::Result;
use freespace_platform::context_menu::{ContextMenu, MenuScope};
use serde_json::json;

use crate::cli::ContextMenuAction;

use super::Outcome;

/// Installs or removes the Explorer integration.
///
/// # Errors
/// Returns an error when the executable path cannot be resolved, the registry
/// cannot be written, or the machine-wide scope is not permitted.
pub fn run(action: &ContextMenuAction, executable: Option<&Path>) -> Result<Outcome> {
    match action {
        ContextMenuAction::Install {
            machine,
            executable: action_executable,
        } => {
            let executable = match action_executable.as_deref().or(executable) {
                Some(path) => path.to_path_buf(),
                None => ContextMenu::current_executable()?,
            };
            let scope = scope(*machine);
            let menu = ContextMenu::for_scope(scope);
            let entries = menu.install(&executable)?;

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

            Ok(Outcome::new(data, human))
        }
        ContextMenuAction::Uninstall { machine } => {
            let scope = scope(*machine);
            ContextMenu::for_scope(scope).uninstall()?;

            let data = json!({
                "scope": scope.as_str(),
                "installed": false,
            });
            let human = format!(
                "Removed the context-menu entries for {}.\n",
                scope_description(scope)
            );

            Ok(Outcome::new(data, human))
        }
    }
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
