#![forbid(unsafe_code)]

//! Application layer shared by the FreeSpace desktop client and its Tauri commands.
//!
//! The crate is deliberately thin and free of WinAPI and UI dependencies: it owns
//! the data transfer objects, validation, pagination and the persistence of
//! interface settings, while `freespace-core`, `freespace-ops` and
//! `freespace-platform` remain the owners of domain logic and native calls.
//!
//! * [`dto`] — serialisable page, node, operation and journal payloads;
//! * [`error`] — the `E_*` error envelope returned by every command;
//! * [`paging`] — opaque cursors bound to a generation;
//! * [`top_types`] — top-file and file-type projections;
//! * [`duplicates`] — duplicate search orchestration and guarded selections;
//! * [`cleanup`] — recommendations and empty directories;
//! * [`snapshots`] — stored `.fssnap` files and comparison pages;
//! * [`operations`] — preview tokens, operation progress, journal and undo;
//! * [`settings`] — the `[ui]` section, its migration and the pre-migration backup;
//! * [`validation`] — limits applied before any file-system access;
//! * [`window_state`] — window geometry in `data/ui-state.toml`.

pub mod category;
pub mod cleanup;
pub mod dto;
pub mod duplicates;
pub mod error;
pub mod operations;
pub mod paging;
pub mod scan;
pub mod settings;
pub mod snapshots;
pub mod top_types;
pub mod tree;
pub mod validation;
pub mod volumes;
pub mod window_state;

// US4 — filesystem and operations.
pub mod long_paths;
#[cfg_attr(not(windows), path = "permissions_unix.rs")]
pub mod permissions;
pub mod rename;
pub mod size_details;
// US5 — inspection and reports.
pub mod archives;
pub mod export;
pub mod metadata;
pub mod owners;
#[cfg_attr(not(windows), path = "unlock_unix.rs")]
pub mod unlock;
// US6 — operations and settings.
pub mod about;
pub mod monitor;

use std::path::Path;

/// Returns `true` when `target` is `root` or lies below it.
///
/// The comparison is lexical and follows the platform path rules: Windows
/// compares case-insensitively, Linux case-sensitively. The objects do not have
/// to exist, which is what output paths need.
pub(crate) fn paths_within(root: &Path, target: &Path) -> bool {
    fn normalize(path: &Path) -> String {
        let separator = std::path::MAIN_SEPARATOR;
        let mut text = freespace_platform::platform().paths.equivalent_key(path);
        while text.len() > 3 && text.ends_with(separator) {
            text.pop();
        }
        text
    }

    let root = normalize(root);
    let target = normalize(target);

    target == root
        || target
            .strip_prefix(&root)
            .is_some_and(|rest| rest.starts_with(std::path::MAIN_SEPARATOR))
}
