//! Rename preview, conflict detection, execution and rollback metadata
//! (FR-054…FR-056).

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result, Uuid};
use freespace_platform::paths;

use crate::{
    operation::{
        JournalEntry, Operation, OperationKind, OperationStatus, OperationTarget, TargetStatus,
    },
    rename::{RenameRule, is_reserved},
    store::Store,
};

/// One planned rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameItem {
    /// Current path.
    pub from: PathBuf,
    /// Planned path.
    pub to: PathBuf,
    /// Current name.
    pub old_name: String,
    /// Planned name.
    pub new_name: String,
    /// Object size in bytes.
    pub size: u64,
}

/// A reason why the plan cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameConflict {
    /// Object that caused the conflict.
    pub path: PathBuf,
    /// Planned target, when one could be built.
    pub target: Option<PathBuf>,
    /// Human-readable explanation.
    pub reason: String,
}

/// Preview of a bulk rename.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenamePlan {
    /// Planned renames, in input order.
    pub items: Vec<RenameItem>,
    /// Conflicts that block execution.
    pub conflicts: Vec<RenameConflict>,
}

impl RenamePlan {
    /// Returns `true` when the plan can be applied.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.conflicts.is_empty()
    }

    /// Number of objects that would change.
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.items.len()
    }
}

/// Builds a rename plan without touching the file system.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when a source path cannot be normalized and
/// [`Error::InvalidArgument`] for an empty input list.
pub fn build_plan(paths_to_rename: &[PathBuf], rule: &RenameRule) -> Result<RenamePlan> {
    if paths_to_rename.is_empty() {
        return Err(Error::InvalidArgument(
            "a rename needs at least one object".to_owned(),
        ));
    }

    let mut plan = RenamePlan::default();
    for path in paths_to_rename {
        let source = paths::normalize(path)?;
        let Some(old_name) = source.file_name().and_then(|name| name.to_str()) else {
            plan.conflicts.push(RenameConflict {
                path: path.clone(),
                target: None,
                reason: "the object has no usable file name".to_owned(),
            });
            continue;
        };
        let new_name = match rule.apply(old_name) {
            Ok(name) => name,
            Err(error) => {
                plan.conflicts.push(RenameConflict {
                    path: path.clone(),
                    target: None,
                    reason: error.to_string(),
                });
                continue;
            }
        };
        if is_reserved(&new_name) {
            plan.conflicts.push(RenameConflict {
                path: path.clone(),
                target: None,
                reason: format!("`{new_name}` is a reserved device name"),
            });
            continue;
        }
        if new_name == old_name {
            continue;
        }

        let parent = match source.parent() {
            Some(parent) => paths::strip_verbatim(parent),
            None => {
                plan.conflicts.push(RenameConflict {
                    path: path.clone(),
                    target: None,
                    reason: "the object has no parent directory".to_owned(),
                });
                continue;
            }
        };
        let target = match paths::join_within_limit(&parent, &new_name) {
            Ok(target) => target,
            Err(error) => {
                plan.conflicts.push(RenameConflict {
                    path: path.clone(),
                    target: None,
                    reason: error.to_string(),
                });
                continue;
            }
        };
        let metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) => {
                plan.conflicts.push(RenameConflict {
                    path: path.clone(),
                    target: Some(target),
                    reason: error.to_string(),
                });
                continue;
            }
        };
        plan.items.push(RenameItem {
            from: paths::strip_verbatim(&source),
            to: target,
            old_name: old_name.to_owned(),
            new_name,
            size: metadata.len(),
        });
    }

    detect_conflicts(&mut plan);

    Ok(plan)
}

fn detect_conflicts(plan: &mut RenamePlan) {
    let mut targets: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, item) in plan.items.iter().enumerate() {
        targets
            .entry(item.to.to_string_lossy().to_lowercase())
            .or_default()
            .push(index);
    }

    let mut conflicts = Vec::new();
    for indexes in targets.values() {
        if indexes.len() > 1 {
            for index in indexes {
                let item = &plan.items[*index];
                conflicts.push(RenameConflict {
                    path: item.from.clone(),
                    target: Some(item.to.clone()),
                    reason: "several objects would get the same name".to_owned(),
                });
            }
        }
    }

    for item in &plan.items {
        if same_path(&item.from, &item.to) {
            continue;
        }
        if paths::to_verbatim(&item.to).is_ok_and(|path| path.exists()) {
            conflicts.push(RenameConflict {
                path: item.from.clone(),
                target: Some(item.to.clone()),
                reason: "the target already exists".to_owned(),
            });
        }
    }

    conflicts.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.reason.cmp(&right.reason))
    });
    plan.conflicts.extend(conflicts);
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

/// Result of applying a rename plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameOutcome {
    /// Number of renamed objects.
    pub renamed: u32,
    /// Operation identifier used for undo.
    pub operation_id: Uuid,
    /// Journal identifier, when the run was journalled.
    pub journal_id: Option<u64>,
    /// Final mappings: original path → current path.
    pub renames: Vec<(PathBuf, PathBuf)>,
    /// Whether undo is available.
    pub undoable: bool,
}

/// Applies a plan after the conflicts were resolved.
///
/// Every rename is executed in two phases (source → temporary name → target) so
/// swaps and case-only renames cannot overwrite each other.
///
/// # Errors
/// Returns [`Error::Conflict`] when the plan has conflicts and rolls back on the
/// first file-system failure.
pub fn apply_plan(plan: &RenamePlan, store: Option<&Store>) -> Result<RenameOutcome> {
    if !plan.is_clear() {
        return Err(Error::Conflict(format!(
            "the rename plan has {} unresolved conflicts",
            plan.conflicts.len()
        )));
    }
    if plan.items.is_empty() {
        return Err(Error::InvalidArgument(
            "the rename plan is empty".to_owned(),
        ));
    }

    let mut temporary = Vec::with_capacity(plan.items.len());
    for item in &plan.items {
        let parent = item.from.parent().unwrap_or_else(|| Path::new("."));
        let temp = parent.join(format!(".freespace-rename-{}", Uuid::new_v4()));
        if let Err(error) = fs::rename(&item.from, &temp) {
            restore_temporaries(&temporary);
            return Err(error.into());
        }
        temporary.push((item.clone(), temp));
    }

    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (item, temp) in &temporary {
        if let Err(error) = fs::rename(temp, &item.to) {
            // Roll back the completed renames and leave the rest at their
            // original names.
            for (from, to) in done.iter().rev() {
                let _ = fs::rename(to, from);
            }
            for (pending, temp_path) in temporary.iter().rev() {
                if temp_path.exists() {
                    let _ = fs::rename(temp_path, &pending.from);
                }
            }
            return Err(error.into());
        }
        done.push((item.from.clone(), item.to.clone()));
    }

    let operation_id = Uuid::new_v4();
    let journal_id = if let Some(store) = store {
        record(store, operation_id, plan, &done).ok()
    } else {
        None
    };

    Ok(RenameOutcome {
        renamed: u32::try_from(done.len()).unwrap_or(u32::MAX),
        operation_id,
        journal_id,
        undoable: store.is_some(),
        renames: done,
    })
}

fn restore_temporaries(temporary: &[(RenameItem, PathBuf)]) {
    for (item, temp) in temporary.iter().rev() {
        if temp.exists() {
            let _ = fs::rename(temp, &item.from);
        }
    }
}

fn record(
    store: &Store,
    operation_id: Uuid,
    plan: &RenamePlan,
    renames: &[(PathBuf, PathBuf)],
) -> Result<u64> {
    let mut operation = Operation::new(
        OperationKind::Rename,
        plan.items
            .iter()
            .map(|item| OperationTarget::new(item.from.clone(), item.size))
            .collect(),
    );
    operation.id = operation_id;
    operation.confirm()?;
    operation.begin()?;
    for target in &mut operation.targets {
        target.status = TargetStatus::Succeeded;
    }
    operation.finish(OperationStatus::Completed)?;

    let mut entry = JournalEntry::from_operation(&operation, current_user());
    entry.undo_available = true;
    if entry.details.is_null() {
        entry.details = serde_json::json!({});
    }
    if let Some(details) = entry.details.as_object_mut() {
        details.insert(
            "renames".to_owned(),
            serde_json::Value::Array(
                renames
                    .iter()
                    .map(|(from, to)| {
                        serde_json::json!({
                            "from": from.to_string_lossy(),
                            "to": to.to_string_lossy(),
                        })
                    })
                    .collect(),
            ),
        );
    }

    store.journal_append(&entry)
}

fn current_user() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use freespace_core::ErrorCode;

    use super::{apply_plan, build_plan};
    use crate::{
        rename::{CaseTransform, RenameRule},
        store::Store,
        undo::undo_operation,
    };

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        for name in ["IMG_0001.jpg", "IMG_0002.jpg", "other.txt"] {
            std::fs::write(directory.path().join(name), b"payload").expect("write");
        }

        directory
    }

    fn paths(directory: &std::path::Path) -> Vec<std::path::PathBuf> {
        ["IMG_0001.jpg", "IMG_0002.jpg", "other.txt"]
            .into_iter()
            .map(|name| directory.join(name))
            .collect()
    }

    #[test]
    fn previews_and_applies_a_rename_plan() {
        let directory = fixture();
        let rule = RenameRule::new(r"^IMG_(\d+)", "photo-$1", CaseTransform::Lower).expect("rule");
        let plan = build_plan(&paths(directory.path()), &rule).expect("plan");

        assert_eq!(plan.items.len(), 2);
        assert!(plan.is_clear());
        let store = Store::open_in_memory().expect("store");
        let outcome = apply_plan(&plan, Some(&store)).expect("apply");

        assert_eq!(outcome.renamed, 2);
        assert!(directory.path().join("photo-0001.jpg").exists());
        assert!(directory.path().join("photo-0002.jpg").exists());
        assert!(!directory.path().join("IMG_0001.jpg").exists());

        let journal = store.journal_list(10).expect("journal");
        assert_eq!(journal.len(), 1);
        assert_eq!(
            journal[0].details["renames"]
                .as_array()
                .expect("renames")
                .len(),
            2
        );
    }

    #[test]
    fn blocks_duplicate_and_existing_targets() {
        let directory = fixture();
        let rule = RenameRule::new(r"IMG_\d+", "same", CaseTransform::Preserve).expect("rule");
        let plan = build_plan(&paths(directory.path()), &rule).expect("plan");
        assert!(plan.conflicts.len() >= 2);

        let rule = RenameRule::new(r"^IMG_0001\.jpg$", "other.txt", CaseTransform::Preserve)
            .expect("rule");
        let plan = build_plan(&paths(directory.path()), &rule).expect("plan");
        assert!(
            plan.conflicts
                .iter()
                .any(|conflict| conflict.reason.contains("already exists"))
        );
        let error = apply_plan(&plan, None).expect_err("conflicts");
        assert_eq!(error.code(), ErrorCode::Conflict);
    }

    #[test]
    fn undo_restores_the_original_names() {
        let directory = fixture();
        let rule = RenameRule::new(r"^IMG_", "photo-", CaseTransform::Preserve).expect("rule");
        let plan = build_plan(&paths(directory.path()), &rule).expect("plan");
        let store = Store::open_in_memory().expect("store");
        let outcome = apply_plan(&plan, Some(&store)).expect("apply");

        let quarantine_root = tempfile::tempdir().expect("quarantine");
        let quarantine =
            crate::quarantine::Quarantine::new(quarantine_root.path().join("store")).expect("q");
        let undone = undo_operation(&store, &quarantine, outcome.operation_id).expect("undo");

        assert_eq!(undone.restored, 2);
        assert!(directory.path().join("IMG_0001.jpg").exists());
        assert!(!directory.path().join("photo-0001.jpg").exists());
    }

    #[test]
    fn handles_a_swap_through_temporary_names() {
        let directory = tempfile::tempdir().expect("temp dir");
        let a = directory.path().join("a.txt");
        let b = directory.path().join("b.txt");
        std::fs::write(&a, b"a").expect("a");
        std::fs::write(&b, b"b").expect("b");
        let rule_a = RenameRule::new(r"^a\.txt$", "b.txt", CaseTransform::Preserve).expect("rule");
        let rule_b = RenameRule::new(r"^b\.txt$", "a.txt", CaseTransform::Preserve).expect("rule");
        let mut plan = build_plan(std::slice::from_ref(&a), &rule_a).expect("plan a");
        plan.items.extend(
            build_plan(std::slice::from_ref(&b), &rule_b)
                .expect("plan b")
                .items,
        );
        // The standalone plans see the other target as existing; clear the
        // artificial conflicts and exercise the execution engine itself.
        plan.conflicts.clear();

        apply_plan(&plan, None).expect("swap");

        assert_eq!(std::fs::read(&a).expect("a"), b"b");
        assert_eq!(std::fs::read(&b).expect("a"), b"a");
    }
}
