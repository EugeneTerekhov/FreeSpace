//! Batch NTFS ownership and permission changes with a per-object report
//! (FR-058…FR-060).

#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::path::Path;
use std::path::PathBuf;

use freespace_core::{Error, Result};
use freespace_platform::paths;
#[cfg(windows)]
use freespace_platform::security;

#[cfg(windows)]
use crate::operation::{
    JournalEntry, Operation, OperationKind, OperationStatus, OperationTarget, TargetStatus,
};
use crate::{path_compat, store::Store};

/// Requested permission change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AclAction {
    /// Set a new owner account.
    SetOwner(String),
    /// Reset the DACL to inherited defaults.
    Reset,
    /// Copy the DACL from another object.
    CopyFrom(PathBuf),
}

impl AclAction {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SetOwner(_) => "owner",
            Self::Reset => "reset",
            Self::CopyFrom(_) => "copy",
        }
    }

    /// Validates the action before any object is touched.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] for an empty account or a missing
    /// source object.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::SetOwner(account) if account.trim().is_empty() => Err(Error::InvalidArgument(
                "the owner account cannot be empty".to_owned(),
            )),
            Self::CopyFrom(source)
                if !path_compat::to_verbatim(source).is_ok_and(|path| path.exists()) =>
            {
                Err(Error::NotFound(paths::display(source)))
            }
            Self::SetOwner(_) | Self::Reset | Self::CopyFrom(_) => Ok(()),
        }
    }
}

/// Result for one object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclItemResult {
    /// Object path.
    pub path: PathBuf,
    /// New owner, when it could be read after the change.
    pub owner: Option<String>,
    /// Error message, when the change failed.
    pub error: Option<String>,
}

/// Batch result with a per-object report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclReport {
    /// Per-object results.
    pub items: Vec<AclItemResult>,
    /// Journal identifier, when the run was journalled.
    pub journal_id: Option<u64>,
}

impl AclReport {
    /// Number of succeeded objects.
    #[must_use]
    pub fn succeeded(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.error.is_none())
            .count()
    }

    /// Number of failed objects.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.items.len().saturating_sub(self.succeeded())
    }
}

/// Applies `action` to every object and journals the result.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for an empty target list and the
/// validation errors of [`AclAction::validate`].
pub fn apply_acl(
    store: Option<&Store>,
    targets: &[PathBuf],
    action: &AclAction,
) -> Result<AclReport> {
    if targets.is_empty() {
        return Err(Error::InvalidArgument(
            "a permission change needs at least one object".to_owned(),
        ));
    }
    action.validate()?;

    #[cfg(not(windows))]
    {
        let _ = (store, targets, action);
        Err(Error::Unsupported(
            "NTFS ownership and permission changes are available on Windows only".to_owned(),
        ))
    }

    #[cfg(windows)]
    {
        let mut items = Vec::with_capacity(targets.len());
        for path in targets {
            let result = match action {
                AclAction::SetOwner(account) => security::set_owner(path, account),
                AclAction::Reset => security::reset_acl(path),
                AclAction::CopyFrom(source) => security::copy_acl(source, path),
            };
            match result {
                Ok(()) => items.push(AclItemResult {
                    path: path.clone(),
                    owner: security::owner(path).ok().flatten(),
                    error: None,
                }),
                Err(error) => items.push(AclItemResult {
                    path: path.clone(),
                    owner: security::owner(path).ok().flatten(),
                    error: Some(error.to_string()),
                }),
            }
        }

        let journal_id = store.and_then(|store| record(store, &items, action).ok());

        Ok(AclReport { items, journal_id })
    }
}

#[cfg(windows)]
fn record(store: &Store, items: &[AclItemResult], action: &AclAction) -> Result<u64> {
    let targets: Vec<OperationTarget> = items.iter().map(|item| target(&item.path)).collect();
    let mut operation = Operation::new(OperationKind::Permissions, targets);
    operation.requires_elevation = true;
    operation.confirm()?;
    operation.begin()?;
    for (target, item) in operation.targets.iter_mut().zip(items) {
        target.status = if item.error.is_none() {
            TargetStatus::Succeeded
        } else {
            TargetStatus::Failed
        };
        target.error.clone_from(&item.error);
    }
    let succeeded = items.iter().filter(|item| item.error.is_none()).count();
    operation.finish(if succeeded == items.len() {
        OperationStatus::Completed
    } else if succeeded == 0 {
        OperationStatus::Failed
    } else {
        OperationStatus::PartiallyCompleted
    })?;

    let mut entry = JournalEntry::from_operation(&operation, current_user());
    if let Some(details) = entry.details.as_object_mut() {
        details.insert(
            "acl_action".to_owned(),
            serde_json::Value::String(action.as_str().to_owned()),
        );
        if let AclAction::SetOwner(account) = action {
            details.insert(
                "owner".to_owned(),
                serde_json::Value::String(account.clone()),
            );
        }
        if let AclAction::CopyFrom(source) = action {
            details.insert(
                "copy_from".to_owned(),
                serde_json::Value::String(paths::display(source)),
            );
        }
    }

    store.journal_append(&entry)
}

#[cfg(windows)]
fn target(path: &Path) -> OperationTarget {
    let size = path_compat::to_verbatim(path)
        .ok()
        .and_then(|path| fs::metadata(path).ok())
        .map_or(0, |metadata| metadata.len());

    OperationTarget::new(path.to_path_buf(), size)
}

#[cfg(windows)]
fn current_user() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use freespace_core::ErrorCode;

    use super::AclAction;
    #[cfg(windows)]
    use super::apply_acl;
    #[cfg(windows)]
    use crate::store::Store;

    #[cfg(windows)]
    #[test]
    fn resets_permissions_on_a_fixture() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, b"payload").expect("write");
        let store = Store::open_in_memory().expect("store");

        let report = apply_acl(Some(&store), &[path], &AclAction::Reset).expect("apply");

        assert_eq!(report.items.len(), 1);
        assert!(report.journal_id.is_some());
        assert_eq!(store.journal_list(10).expect("journal").len(), 1);
    }

    #[test]
    fn validates_actions() {
        assert_eq!(
            AclAction::SetOwner(" ".to_owned())
                .validate()
                .expect_err("empty account")
                .code(),
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            AclAction::CopyFrom("C:\\missing\\object".into())
                .validate()
                .expect_err("missing")
                .code(),
            ErrorCode::NotFound
        );
        assert!(AclAction::Reset.validate().is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn reports_missing_objects() {
        let directory = tempfile::tempdir().expect("temp dir");

        let report = apply_acl(
            None,
            &[directory.path().join("missing.bin")],
            &AclAction::Reset,
        )
        .expect("report");

        assert_eq!(report.failed(), 1);
        assert!(report.succeeded() == 0);
        assert!(report.items[0].error.as_deref().is_some());
    }
}
