//! Explorer context-menu integration (FR-080, FR-081).
//!
//! Entries are written to the per-user `HKCU\Software\Classes` tree so that no
//! elevation is required, or to `HKLM\Software\Classes` for a machine-wide
//! installation. Every target class (files, folders and drives) gets an
//! *Analyze* item; files and folders additionally get an *Unlock* item.
//!
//! The command line is quoted according to the rules used by
//! `CommandLineToArgvW`, so paths with spaces, Unicode characters and trailing
//! separators survive the Explorer round trip (FR-081).

use std::{
    io,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ},
};

/// Registry key suffix of the *Analyze* command.
pub const ANALYZE_KEY: &str = "FreeSpace.Analyze";
/// Registry key suffix of the *Unlock* command.
pub const UNLOCK_KEY: &str = "FreeSpace.Unlock";
/// Class tree used for per-user registration.
pub const PER_USER_CLASSES: &str = r"Software\Classes";
/// Class tree used for machine-wide registration.
pub const MACHINE_CLASSES: &str = r"Software\Classes";

/// Registry hive that receives the entries.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuScope {
    /// `HKCU\Software\Classes`; available without elevation.
    #[default]
    User,
    /// `HKLM\Software\Classes`; requires administrator rights.
    Machine,
}

impl MenuScope {
    fn root(self) -> RegKey {
        match self {
            Self::User => RegKey::predef(HKEY_CURRENT_USER),
            Self::Machine => RegKey::predef(HKEY_LOCAL_MACHINE),
        }
    }

    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Machine => "machine",
        }
    }

    /// Whether the scope needs administrator rights.
    #[must_use]
    pub const fn requires_elevation(self) -> bool {
        matches!(self, Self::Machine)
    }
}

/// Action offered in the context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuAction {
    /// Open FreeSpace with the selected path.
    Analyze,
    /// List and release the processes that hold the selected file.
    Unlock,
}

impl MenuAction {
    /// Every action in display order.
    pub const ALL: [Self; 2] = [Self::Analyze, Self::Unlock];

    /// Registry key suffix.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Analyze => ANALYZE_KEY,
            Self::Unlock => UNLOCK_KEY,
        }
    }

    /// Menu caption shown by Explorer.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Analyze => "Analyze with FreeSpace",
            Self::Unlock => "Unlock with FreeSpace",
        }
    }

    /// CLI arguments placed after the executable, with `%1` as the placeholder.
    #[must_use]
    pub const fn arguments(self) -> &'static [&'static str] {
        match self {
            Self::Analyze => &["%1"],
            Self::Unlock => &["unlock", "%1"],
        }
    }
}

/// Kind of object the entry is registered for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuTarget {
    /// Every file (`*`).
    File,
    /// Every directory (`Directory`).
    Directory,
    /// Drive roots (`Drive`).
    Drive,
}

impl MenuTarget {
    /// Every supported target in registration order.
    pub const ALL: [Self; 3] = [Self::File, Self::Directory, Self::Drive];

    /// Shell class name.
    #[must_use]
    pub const fn class(self) -> &'static str {
        match self {
            Self::File => "*",
            Self::Directory => "Directory",
            Self::Drive => "Drive",
        }
    }

    /// Actions registered for this target.
    #[must_use]
    pub const fn actions(self) -> &'static [MenuAction] {
        match self {
            Self::File | Self::Directory => &MenuAction::ALL,
            Self::Drive => &[MenuAction::Analyze],
        }
    }
}

/// One registered menu entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MenuEntry {
    /// Object class.
    pub target: MenuTarget,
    /// Action.
    pub action: MenuAction,
    /// Full registry path of the entry.
    pub key: String,
    /// Command line stored in the `command` subkey.
    pub command: String,
}

/// Installer for the Explorer integration.
#[derive(Debug, Clone)]
pub struct ContextMenu {
    scope: MenuScope,
    base: String,
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self::user()
    }
}

impl ContextMenu {
    /// Creates an installer for the current user.
    #[must_use]
    pub fn user() -> Self {
        Self {
            scope: MenuScope::User,
            base: PER_USER_CLASSES.to_owned(),
        }
    }

    /// Creates an installer for all users.
    #[must_use]
    pub fn machine() -> Self {
        Self {
            scope: MenuScope::Machine,
            base: MACHINE_CLASSES.to_owned(),
        }
    }

    /// Creates an installer for the given scope.
    #[must_use]
    pub fn for_scope(scope: MenuScope) -> Self {
        match scope {
            MenuScope::User => Self::user(),
            MenuScope::Machine => Self::machine(),
        }
    }

    /// Redirects the installer to another registry subtree.
    ///
    /// Tests use it to keep the real Explorer menu untouched.
    #[must_use]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();

        self
    }

    /// Registry hive used by this installer.
    #[must_use]
    pub const fn scope(&self) -> MenuScope {
        self.scope
    }

    /// Registry subtree prefix.
    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Describes every entry that [`Self::install`] would write.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] when the executable path is relative.
    pub fn plan(&self, executable: &Path) -> Result<Vec<MenuEntry>> {
        if !executable.is_absolute() {
            return Err(Error::InvalidArgument(format!(
                "the executable path must be absolute: `{}`",
                executable.display()
            )));
        }

        let executable = quote_os(executable.as_os_str());
        let mut entries = Vec::new();
        for target in MenuTarget::ALL {
            for action in target.actions() {
                let mut parts = vec![executable.clone()];
                parts.extend(
                    action
                        .arguments()
                        .iter()
                        .map(|argument| quote_argument(argument)),
                );
                entries.push(MenuEntry {
                    target,
                    action: *action,
                    key: self.entry_key(target, *action),
                    command: parts.join(" "),
                });
            }
        }

        Ok(entries)
    }

    /// Writes every entry to the registry.
    ///
    /// Returns the registry paths that were created.
    ///
    /// # Errors
    /// Returns [`Error::AccessDenied`] for a machine-wide installation without
    /// elevation and [`Error::Io`] when the registry cannot be written.
    pub fn install(&self, executable: &Path) -> Result<Vec<MenuEntry>> {
        let entries = self.plan(executable)?;
        let root = self.scope.root();
        let icon = quote_os(executable.as_os_str());

        for entry in &entries {
            let (key, _) = root
                .create_subkey(&entry.key)
                .map_err(|error| registry_error(&entry.key, &error))?;
            key.set_value("MUIVerb", &entry.action.label())
                .map_err(|error| registry_error(&entry.key, &error))?;
            let icon_value = format!("{icon},0");
            key.set_value("Icon", &icon_value)
                .map_err(|error| registry_error(&entry.key, &error))?;
            let command_key = format!(r"{}\command", entry.key);
            let (command, _) = root
                .create_subkey(&command_key)
                .map_err(|error| registry_error(&command_key, &error))?;
            command
                .set_value("", &entry.command)
                .map_err(|error| registry_error(&command_key, &error))?;
        }

        Ok(entries)
    }

    /// Removes every entry written by [`Self::install`].
    ///
    /// # Errors
    /// Returns [`Error::AccessDenied`] for a machine-wide removal without
    /// elevation and [`Error::Io`] when the registry cannot be changed.
    pub fn uninstall(&self) -> Result<()> {
        let root = self.scope.root();

        for target in MenuTarget::ALL {
            for action in target.actions() {
                let key = self.entry_key(target, *action);
                match root.delete_subkey_all(&key) {
                    Ok(()) => {}
                    Err(error) if is_not_found(&error) => {}
                    Err(error) => return Err(registry_error(&key, &error)),
                }
            }
        }

        Ok(())
    }

    /// Reads the entries that are currently registered.
    ///
    /// # Errors
    /// Returns [`Error::Io`] when the registry cannot be read for a reason other
    /// than a missing key.
    pub fn installed_entries(&self) -> Result<Vec<MenuEntry>> {
        let root = self.scope.root();
        let mut installed = Vec::new();

        for target in MenuTarget::ALL {
            for action in target.actions() {
                let key = self.entry_key(target, *action);
                let command_key = format!(r"{key}\command");
                let command = match root.open_subkey_with_flags(&command_key, KEY_READ) {
                    Ok(command) => command,
                    Err(error) if is_not_found(&error) => continue,
                    Err(error) => return Err(registry_error(&command_key, &error)),
                };
                let value: String = match command.get_value("") {
                    Ok(value) => value,
                    Err(error) if is_not_found(&error) => continue,
                    Err(error) => return Err(registry_error(&command_key, &error)),
                };
                installed.push(MenuEntry {
                    target,
                    action: *action,
                    key,
                    command: value,
                });
            }
        }

        Ok(installed)
    }

    /// Returns `true` when every entry is present with a non-empty command.
    ///
    /// # Errors
    /// Returns [`Error::Io`] when the registry cannot be read for a reason other
    /// than a missing key.
    pub fn is_installed(&self) -> Result<bool> {
        let root = self.scope.root();
        let executable = quote_argument("%1");

        for target in MenuTarget::ALL {
            for action in target.actions() {
                let key = self.entry_key(target, *action);
                let command_key = format!(r"{key}\command");
                let command = match root.open_subkey_with_flags(&command_key, KEY_READ) {
                    Ok(command) => command,
                    Err(error) if is_not_found(&error) => return Ok(false),
                    Err(error) => return Err(registry_error(&command_key, &error)),
                };
                let value: String = match command.get_value("") {
                    Ok(value) => value,
                    Err(error) if is_not_found(&error) => return Ok(false),
                    Err(error) => return Err(registry_error(&command_key, &error)),
                };
                if value.trim().is_empty() || !value.ends_with(&executable) {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Full registry path of one entry.
    #[must_use]
    pub fn entry_key(&self, target: MenuTarget, action: MenuAction) -> String {
        format!(r"{}\{}\shell\{}", self.base, target.class(), action.key())
    }

    /// Resolves the executable that the CLI should register.
    ///
    /// # Errors
    /// Returns [`Error::Config`] when the current executable path is unknown.
    pub fn current_executable() -> Result<PathBuf> {
        std::env::current_exe().map_err(|error| {
            Error::Config(format!(
                "cannot resolve the FreeSpace executable path: {error}"
            ))
        })
    }
}

fn is_not_found(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound || error.raw_os_error() == Some(2)
}

fn registry_error(key: &str, error: &io::Error) -> Error {
    if error.kind() == io::ErrorKind::PermissionDenied || error.raw_os_error() == Some(5) {
        return Error::AccessDenied(format!(r"cannot write registry key `{key}`: {error}"));
    }

    Error::Io(io::Error::new(
        error.kind(),
        format!(r"registry key `{key}`: {error}"),
    ))
}

/// Quotes one argument for a Windows command line.
///
/// Follows the `CommandLineToArgvW` rules: an argument is quoted when it is
/// empty or contains whitespace or a double quote, and backslashes are doubled
/// in front of the closing quote.
#[must_use]
pub fn quote_argument(argument: &str) -> String {
    if !argument.is_empty()
        && !argument
            .chars()
            .any(|character| character == ' ' || character == '\t' || character == '"')
    {
        return argument.to_owned();
    }

    let mut quoted = String::with_capacity(argument.len() + 2);
    quoted.push('"');
    let mut backslashes = 0usize;
    for character in argument.chars() {
        match character {
            '\\' => {
                backslashes += 1;
                quoted.push('\\');
            }
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                quoted.push('\\');
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                backslashes = 0;
                quoted.push(character);
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes));
    quoted.push('"');

    quoted
}

/// Quotes a filesystem path without lossy Unicode conversion.
#[must_use]
pub fn quote_os(path: &std::ffi::OsStr) -> String {
    quote_argument(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, path::Path};

    use super::{ContextMenu, MenuAction, MenuScope, MenuTarget, quote_argument};

    #[test]
    fn quotes_only_when_required() {
        assert_eq!(
            quote_argument("C:\\Tools\\freespace.exe"),
            "C:\\Tools\\freespace.exe"
        );
        assert_eq!(
            quote_argument("C:\\Program Files\\FreeSpace\\freespace.exe"),
            "\"C:\\Program Files\\FreeSpace\\freespace.exe\""
        );
        assert_eq!(quote_argument(""), "\"\"");
        assert_eq!(quote_argument("%1"), "%1");
    }

    #[test]
    fn escapes_quotes_and_trailing_backslashes() {
        assert_eq!(quote_argument("say \"hi\""), r#""say \"hi\"""#);
        assert_eq!(
            quote_argument(r"C:\Program Files\FreeSpace\"),
            "\"C:\\Program Files\\FreeSpace\\\\\""
        );
        assert_eq!(quote_argument("a\"b"), r#""a\"b""#);
    }

    #[test]
    fn keeps_unicode_paths_intact() {
        let path = r"C:\Пользователи\测试 目录\freespace.exe";
        let quoted = quote_argument(path);

        assert!(quoted.starts_with('"'));
        assert!(quoted.ends_with('"'));
        assert!(quoted.contains("测试 目录"));
    }

    #[test]
    fn plans_entries_for_every_target() {
        let menu = ContextMenu::user().with_base(r"Software\FreeSpaceTests\plan");
        let entries = menu
            .plan(Path::new(r"C:\Program Files\FreeSpace\freespace.exe"))
            .expect("plan");

        assert_eq!(entries.len(), 5);
        assert!(entries.iter().any(|entry| {
            entry.target == MenuTarget::File && entry.action == MenuAction::Analyze
        }));
        assert!(entries.iter().any(|entry| {
            entry.target == MenuTarget::Drive && entry.action == MenuAction::Analyze
        }));
        assert!(!entries.iter().any(|entry| {
            entry.target == MenuTarget::Drive && entry.action == MenuAction::Unlock
        }));

        let analyze = entries
            .iter()
            .find(|entry| {
                entry.target == MenuTarget::Directory && entry.action == MenuAction::Analyze
            })
            .expect("directory analyze");
        assert_eq!(
            analyze.command,
            "\"C:\\Program Files\\FreeSpace\\freespace.exe\" %1"
        );
        assert!(analyze.key.ends_with(r"Directory\shell\FreeSpace.Analyze"));

        let unlock = entries
            .iter()
            .find(|entry| entry.target == MenuTarget::File && entry.action == MenuAction::Unlock)
            .expect("file unlock");
        assert!(unlock.command.ends_with("unlock %1"));
    }

    #[test]
    fn rejects_relative_executables() {
        let menu = ContextMenu::user();

        assert!(menu.plan(Path::new("freespace.exe")).is_err());
    }

    #[test]
    fn exposes_scope_properties() {
        assert_eq!(MenuScope::User.as_str(), "user");
        assert!(!MenuScope::User.requires_elevation());
        assert!(MenuScope::Machine.requires_elevation());
        assert_eq!(ContextMenu::machine().scope(), MenuScope::Machine);
        assert_eq!(
            ContextMenu::machine().entry_key(MenuTarget::Drive, MenuAction::Analyze),
            r"Software\Classes\Drive\shell\FreeSpace.Analyze"
        );
        assert_eq!(
            quote_argument(OsStr::new("a b").to_string_lossy().as_ref()),
            "\"a b\""
        );
    }
}
