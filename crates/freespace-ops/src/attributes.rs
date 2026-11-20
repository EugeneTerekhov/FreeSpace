//! Batch attribute changes with a per-object report (FR-057, FR-059).
//!
//! Attribute names are mapped to the Windows bit mask once, then applied to each
//! object independently: one read-only file must not stop the rest of the batch.

#[cfg(windows)]
use std::fs;
use std::path::PathBuf;

use freespace_core::{Error, Result};
#[cfg(windows)]
use freespace_platform::security;

#[cfg(windows)]
use crate::operation::{
    JournalEntry, Operation, OperationKind, OperationStatus, OperationTarget, TargetStatus,
};
#[cfg(windows)]
use crate::path_compat;
use crate::store::Store;

/// `FILE_ATTRIBUTE_READONLY`.
pub const READ_ONLY: u32 = 0x0000_0001;
/// `FILE_ATTRIBUTE_HIDDEN`.
pub const HIDDEN: u32 = 0x0000_0002;
/// `FILE_ATTRIBUTE_SYSTEM`.
pub const SYSTEM: u32 = 0x0000_0004;
/// `FILE_ATTRIBUTE_ARCHIVE`.
pub const ARCHIVE: u32 = 0x0000_0020;
/// `FILE_ATTRIBUTE_TEMPORARY`.
pub const TEMPORARY: u32 = 0x0000_0100;
/// `FILE_ATTRIBUTE_SPARSE_FILE`.
pub const SPARSE: u32 = 0x0000_0200;
/// `FILE_ATTRIBUTE_COMPRESSED`.
pub const COMPRESSED: u32 = 0x0000_0800;
/// `FILE_ATTRIBUTE_NOT_CONTENT_INDEXED`.
pub const NOT_CONTENT_INDEXED: u32 = 0x0000_2000;

/// One set of attribute bits to apply.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AttributeChange {
    /// Bits to set.
    pub set: u32,
    /// Bits to clear.
    pub clear: u32,
}

impl AttributeChange {
    /// Returns `true` when nothing would change.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.set == 0 && self.clear == 0
    }

    /// Resolves a set and a clear list into a change.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] for an unknown attribute name or when
    /// the same attribute appears in both lists.
    pub fn parse(set: Option<&str>, clear: Option<&str>) -> Result<Self> {
        let set = parse_mask(set.unwrap_or_default())?;
        let clear = parse_mask(clear.unwrap_or_default())?;
        if set & clear != 0 {
            return Err(Error::InvalidArgument(
                "an attribute cannot be set and cleared at the same time".to_owned(),
            ));
        }
        if set == 0 && clear == 0 {
            return Err(Error::InvalidArgument(
                "provide at least one attribute to set or clear".to_owned(),
            ));
        }

        Ok(Self { set, clear })
    }
}

/// Parses a comma- or space-separated attribute list.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for an unknown name.
pub fn parse_mask(list: &str) -> Result<u32> {
    let mut mask = 0u32;
    for name in list
        .split([',', ';', ' ', '|'])
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        mask |= attribute_bit(name).ok_or_else(|| {
            Error::InvalidArgument(format!(
                "unknown attribute `{name}`; expected one of readonly, hidden, system, archive, \
                 temporary, sparse, compressed or not-indexed"
            ))
        })?;
    }

    Ok(mask)
}

fn attribute_bit(name: &str) -> Option<u32> {
    match name.to_ascii_lowercase().replace('_', "-").as_str() {
        "readonly" | "read-only" | "r" => Some(READ_ONLY),
        "hidden" | "h" => Some(HIDDEN),
        "system" | "s" => Some(SYSTEM),
        "archive" | "a" => Some(ARCHIVE),
        "temporary" | "temp" | "t" => Some(TEMPORARY),
        "sparse" | "sparse-file" => Some(SPARSE),
        "compressed" | "compression" => Some(COMPRESSED),
        "not-indexed" | "not-content-indexed" | "no-index" => Some(NOT_CONTENT_INDEXED),
        _ => None,
    }
}

/// Result for one object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeItemResult {
    /// Object path.
    pub path: PathBuf,
    /// Attribute mask after the change, or before it on failure.
    pub attributes: Option<u32>,
    /// Error message, when the change failed.
    pub error: Option<String>,
}

/// Batch result with a per-object report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeReport {
    /// Per-object results.
    pub items: Vec<AttributeItemResult>,
    /// Journal identifier, when the run was journalled.
    pub journal_id: Option<u64>,
}

impl AttributeReport {
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

/// Applies attributes to every object and journals the result.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for an empty target list.
pub fn apply_attributes(
    store: Option<&Store>,
    targets: &[PathBuf],
    change: AttributeChange,
) -> Result<AttributeReport> {
    if targets.is_empty() {
        return Err(Error::InvalidArgument(
            "an attribute change needs at least one object".to_owned(),
        ));
    }
    if change.is_empty() {
        return Err(Error::InvalidArgument(
            "the attribute change is empty".to_owned(),
        ));
    }

    #[cfg(not(windows))]
    {
        let _ = (store, targets, change);
        Err(Error::Unsupported(
            "Windows file attributes are available on Windows only".to_owned(),
        ))
    }

    #[cfg(windows)]
    {
        let mut items = Vec::with_capacity(targets.len());
        for path in targets {
            match security::set_attributes(path, change.set, change.clear) {
                Ok(attributes) => items.push(AttributeItemResult {
                    path: path.clone(),
                    attributes: Some(attributes),
                    error: None,
                }),
                Err(error) => items.push(AttributeItemResult {
                    path: path.clone(),
                    attributes: security::get_attributes(path).ok(),
                    error: Some(error.to_string()),
                }),
            }
        }

        let journal_id = store.and_then(|store| record(store, &items, change).ok());

        Ok(AttributeReport { items, journal_id })
    }
}

#[cfg(windows)]
fn record(store: &Store, items: &[AttributeItemResult], change: AttributeChange) -> Result<u64> {
    let targets: Vec<OperationTarget> = items
        .iter()
        .map(|item| {
            let size = fs::metadata(
                path_compat::to_verbatim(&item.path).unwrap_or_else(|_| item.path.clone()),
            )
            .map_or(0, |metadata| metadata.len());
            OperationTarget::new(item.path.clone(), size)
        })
        .collect();
    let mut operation = Operation::new(OperationKind::Attributes, targets);
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
            "attributes".to_owned(),
            serde_json::json!({
                "set": change.set,
                "clear": change.clear,
            }),
        );
    }

    store.journal_append(&entry)
}

#[cfg(windows)]
fn current_user() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use freespace_core::ErrorCode;

    use super::{ARCHIVE, AttributeChange, HIDDEN, READ_ONLY, apply_attributes, parse_mask};
    #[cfg(windows)]
    use crate::store::Store;

    #[test]
    fn parses_attribute_names() {
        assert_eq!(
            parse_mask("readonly, hidden").expect("mask"),
            READ_ONLY | HIDDEN
        );
        assert_eq!(
            parse_mask("read-only;archive").expect("mask"),
            READ_ONLY | ARCHIVE
        );
        assert_eq!(parse_mask("").expect("empty"), 0);
        assert_eq!(
            parse_mask("magic").expect_err("unknown").code(),
            ErrorCode::InvalidArgument
        );
        assert!(AttributeChange::parse(Some("hidden"), Some("hidden")).is_err());
        assert!(AttributeChange::parse(None, None).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn updates_fixture_attributes_and_journals() {
        let directory = tempfile::tempdir().expect("temp dir");
        let first = directory.path().join("first.txt");
        let second = directory.path().join("second.txt");
        std::fs::write(&first, b"one").expect("first");
        std::fs::write(&second, b"two").expect("second");
        let store = Store::open_in_memory().expect("store");

        let report = apply_attributes(
            Some(&store),
            &[first.clone(), second],
            AttributeChange {
                set: HIDDEN,
                clear: ARCHIVE,
            },
        )
        .expect("apply");

        assert_eq!(report.succeeded(), 2);
        assert_eq!(report.failed(), 0);
        assert!(report.journal_id.is_some());
        assert_eq!(
            freespace_platform::security::get_attributes(&first).expect("attributes") & HIDDEN,
            HIDDEN
        );
        assert_eq!(store.journal_list(10).expect("journal").len(), 1);

        apply_attributes(
            Some(&store),
            std::slice::from_ref(&first),
            AttributeChange {
                set: 0,
                clear: HIDDEN,
            },
        )
        .expect("clear");
    }

    #[cfg(windows)]
    #[test]
    fn reports_missing_objects_separately() {
        let directory = tempfile::tempdir().expect("temp dir");
        let missing = directory.path().join("missing.txt");

        let report = apply_attributes(
            None,
            &[missing],
            AttributeChange {
                set: HIDDEN,
                clear: 0,
            },
        )
        .expect("report");

        assert_eq!(report.failed(), 1);
        assert!(report.items[0].error.as_deref().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn attribute_changes_are_unsupported_on_unix() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("file.txt");
        std::fs::write(&path, b"x").expect("write");

        let error = apply_attributes(
            None,
            &[path],
            AttributeChange {
                set: HIDDEN,
                clear: 0,
            },
        )
        .expect_err("unsupported");

        assert_eq!(error.code(), ErrorCode::Unsupported);
    }
}
