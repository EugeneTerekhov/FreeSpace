//! Bulk rename preview and application (US4, T100).
//!
//! A rename never starts from a request: [`RenameService::preview`] compiles the
//! regular expression, builds the plan with `freespace-ops`, reports every
//! conflict and hands out a one-time confirmation token with a ten-minute TTL.
//! [`RenameService::apply`] consumes that token exactly once, re-checks the
//! scope and the elevation requirement and only then lets `freespace-ops` touch
//! the file system.
//!
//! The plan is stored in verbatim form (`\\?\C:\...`), so a rename works for
//! paths longer than `MAX_PATH` without the system-wide `LongPathsEnabled`
//! policy. Protected system locations need an elevated process; without it the
//! apply call fails before anything is renamed.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex, MutexGuard},
};

use freespace_ops::{
    CaseTransform, RenamePlan, RenameRule, Store,
    confirm::is_protected_path,
    rename_plan::{RenameConflict, RenameItem, apply_plan, build_plan},
};
use freespace_platform::paths;
use uuid::Uuid;

use crate::{
    dto::{
        RenameApplied, RenameCase, RenameConflictView, RenamePreview, RenamePreviewRequest,
        RenamePreviewRow,
    },
    error::{UiError, UiErrorCode},
    operations::PREVIEW_TTL_MS,
    validation::validate_path,
};

/// Reason key of a target that already exists.
pub const CONFLICT_EXISTS: &str = "rename.conflict.exists";
/// Reason key of several targets that would receive the same name.
pub const CONFLICT_DUPLICATE: &str = "rename.conflict.duplicate";
/// Reason key of a reserved device name.
pub const CONFLICT_RESERVED: &str = "rename.conflict.reserved";
/// Reason key of a result beyond the supported path length.
pub const CONFLICT_PATH_LIMIT: &str = "rename.conflict.path_limit";
/// Reason key of a rule that produced an unusable name.
pub const CONFLICT_INVALID_NAME: &str = "rename.conflict.invalid_name";

/// A preview that has not been confirmed yet.
#[derive(Debug, Clone)]
pub struct PreparedRename {
    /// One-time preview identifier.
    pub preview_id: String,
    /// One-time confirmation token.
    pub token: String,
    /// Expiry, Unix milliseconds.
    pub expires_at: i64,
    /// Plan with verbatim paths, ready for `freespace-ops`.
    pub plan: RenamePlan,
    /// Directory that bounds every target, when one was requested.
    pub scope: Option<PathBuf>,
    /// True when a target lives in a protected system location.
    pub requires_elevation: bool,
    /// Identifier returned by the apply call.
    pub operation_id: Uuid,
}

#[derive(Debug, Default)]
struct ServiceInner {
    previews: HashMap<String, PreparedRename>,
}

/// Preview registry of the rename screen.
#[derive(Debug)]
pub struct RenameService {
    inner: Mutex<ServiceInner>,
    ttl_ms: i64,
}

impl Default for RenameService {
    fn default() -> Self {
        Self::new()
    }
}

impl RenameService {
    /// Creates a service with the documented ten-minute TTL.
    #[must_use]
    pub fn new() -> Self {
        Self::with_ttl(PREVIEW_TTL_MS)
    }

    /// Creates a service with an explicit TTL; used by tests.
    #[must_use]
    pub fn with_ttl(ttl_ms: i64) -> Self {
        Self {
            inner: Mutex::new(ServiceInner::default()),
            ttl_ms,
        }
    }

    /// Builds the plan and stores its one-time token.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for a malformed request or pattern and
    /// `E_ACCESS_DENIED` when a target leaves the requested scope.
    pub fn preview(
        &self,
        request: &RenamePreviewRequest,
        now_ms: i64,
    ) -> Result<RenamePreview, UiError> {
        request.validate()?;

        let scope = request.scope.as_deref().map(validate_path).transpose()?;
        let targets = request
            .targets
            .iter()
            .map(|target| validate_path(target))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(scope) = &scope {
            for target in &targets {
                paths::ensure_within(scope, target).map_err(|error| {
                    UiError::new(UiErrorCode::AccessDenied).with_details(error.to_string())
                })?;
            }
        }

        let rule = RenameRule::new(
            &request.regex,
            &request.replacement,
            case_transform(request.case),
        )?;
        let plan = build_plan(&targets, &rule)?;

        let mut conflicts: Vec<RenameConflictView> =
            plan.conflicts.iter().map(conflict_view).collect();
        let mut verbatim_items = Vec::with_capacity(plan.items.len());
        for item in &plan.items {
            match (paths::to_verbatim(&item.from), paths::to_verbatim(&item.to)) {
                (Ok(from), Ok(to)) => verbatim_items.push(RenameItem {
                    from,
                    to,
                    old_name: item.old_name.clone(),
                    new_name: item.new_name.clone(),
                    size: item.size,
                }),
                (_, Err(error)) | (Err(error), _) => conflicts.push(RenameConflictView {
                    path: paths::display(&item.from),
                    target: Some(paths::display(&item.to)),
                    reason: error.to_string(),
                    reason_key: CONFLICT_PATH_LIMIT.to_owned(),
                }),
            }
        }

        let mut rows: Vec<RenamePreviewRow> = plan
            .items
            .iter()
            .map(|item| RenamePreviewRow {
                old_path: paths::display(&item.from),
                new_path: paths::display(&item.to),
                old_name: item.old_name.clone(),
                new_name: item.new_name.clone(),
                size: item.size,
                conflict: false,
            })
            .collect();
        for row in &mut rows {
            row.conflict = conflicts
                .iter()
                .any(|conflict| conflict.path.eq_ignore_ascii_case(&row.old_path));
        }

        let requires_elevation = targets.iter().any(|target| is_protected_path(target));
        let mut warnings = Vec::new();
        if requires_elevation {
            warnings.push("rename.warning.protected".to_owned());
        }
        if !conflicts.is_empty() {
            warnings.push("rename.warning.conflicts".to_owned());
        }
        if rows.is_empty() && conflicts.is_empty() {
            warnings.push("rename.warning.no_change".to_owned());
        }

        let preview_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let expires_at = now_ms.saturating_add(self.ttl_ms);
        let preview = RenamePreview {
            preview_id: preview_id.clone(),
            confirmation_token: token.clone(),
            expires_at,
            change_count: u32::try_from(rows.len()).unwrap_or(u32::MAX),
            total_size: rows.iter().map(|row| row.size).sum(),
            rows,
            conflicts: conflicts.clone(),
            requires_elevation,
            warnings,
        };

        let stored = RenamePlan {
            items: verbatim_items,
            conflicts: conflicts.iter().map(conflict_from_view).collect(),
        };

        self.lock().previews.insert(
            preview_id.clone(),
            PreparedRename {
                preview_id,
                token,
                expires_at,
                plan: stored,
                scope,
                requires_elevation,
                operation_id: Uuid::new_v4(),
            },
        );

        Ok(preview)
    }

    /// Consumes a preview and applies its plan.
    ///
    /// # Errors
    /// Returns `E_PREVIEW_EXPIRED` for an unknown, used or expired preview,
    /// `E_ELEVATION_REQUIRED` when a protected target needs an elevated process,
    /// `E_ACCESS_DENIED` when a target left the stored scope and the mapped
    /// `freespace-ops` error when the rename itself fails.
    pub fn apply(
        &self,
        preview_id: &str,
        confirmation_token: &str,
        store: Option<&Store>,
        elevated: bool,
        now_ms: i64,
    ) -> Result<RenameApplied, UiError> {
        let mut inner = self.lock();
        let Some(prepared) = inner.previews.get(preview_id).cloned() else {
            return Err(UiError::new(UiErrorCode::PreviewExpired));
        };
        if prepared.token != confirmation_token || now_ms > prepared.expires_at {
            inner.previews.remove(preview_id);
            return Err(UiError::new(UiErrorCode::PreviewExpired));
        }
        if prepared.requires_elevation && !elevated {
            return Err(UiError::new(UiErrorCode::ElevationRequired)
                .with_details("the rename touches a protected system location"));
        }
        if !prepared.plan.is_clear() {
            return Err(UiError::validation(
                "the rename plan still has unresolved conflicts",
            ));
        }
        if prepared.plan.items.is_empty() {
            inner.previews.remove(preview_id);
            return Err(UiError::validation("the rename plan is empty"));
        }
        if let Some(scope) = &prepared.scope {
            for item in &prepared.plan.items {
                paths::ensure_within(scope, &item.from).map_err(|error| {
                    UiError::new(UiErrorCode::AccessDenied).with_details(error.to_string())
                })?;
            }
        }
        inner.previews.remove(preview_id);
        drop(inner);

        let outcome = apply_plan(&prepared.plan, store)?;

        Ok(RenameApplied {
            operation_id: outcome.operation_id.to_string(),
            renamed: outcome.renamed,
            journal_entry_id: outcome.journal_id,
            undo_available: outcome.undoable,
        })
    }

    /// Number of live previews; used by tests.
    #[must_use]
    pub fn preview_count(&self) -> usize {
        self.lock().previews.len()
    }

    fn lock(&self) -> MutexGuard<'_, ServiceInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Process-wide registry used by the Tauri command.
static SERVICE: LazyLock<RenameService> = LazyLock::new(RenameService::new);

/// Builds a rename preview with the process-wide registry.
///
/// # Errors
/// See [`RenameService::preview`].
pub fn preview(request: &RenamePreviewRequest, now_ms: i64) -> Result<RenamePreview, UiError> {
    SERVICE.preview(request, now_ms)
}

/// Applies a preview from the process-wide registry.
///
/// # Errors
/// See [`RenameService::apply`].
pub fn apply(
    preview_id: &str,
    confirmation_token: &str,
    store: Option<&Store>,
    elevated: bool,
    now_ms: i64,
) -> Result<RenameApplied, UiError> {
    SERVICE.apply(preview_id, confirmation_token, store, elevated, now_ms)
}

/// Number of live previews in the process-wide registry; used by diagnostics.
#[must_use]
pub fn preview_count() -> usize {
    SERVICE.preview_count()
}

fn case_transform(case: RenameCase) -> CaseTransform {
    match case {
        RenameCase::Preserve => CaseTransform::Preserve,
        RenameCase::Lower => CaseTransform::Lower,
        RenameCase::Upper => CaseTransform::Upper,
    }
}

fn conflict_view(conflict: &RenameConflict) -> RenameConflictView {
    RenameConflictView {
        path: paths::display(&conflict.path),
        target: conflict.target.as_deref().map(paths::display),
        reason_key: conflict_reason_key(&conflict.reason).to_owned(),
        reason: conflict.reason.clone(),
    }
}

/// Returns the localisation key of a conflict reason.
#[must_use]
pub fn conflict_reason_key(reason: &str) -> &'static str {
    let lowered = reason.to_lowercase();
    if lowered.contains("reserved") {
        CONFLICT_RESERVED
    } else if lowered.contains("already exists") {
        CONFLICT_EXISTS
    } else if lowered.contains("same name") {
        CONFLICT_DUPLICATE
    } else if lowered.contains("longer") || lowered.contains("limit") {
        CONFLICT_PATH_LIMIT
    } else {
        CONFLICT_INVALID_NAME
    }
}

fn conflict_from_view(conflict: &RenameConflictView) -> RenameConflict {
    RenameConflict {
        path: paths::to_verbatim(Path::new(&conflict.path))
            .unwrap_or_else(|_| PathBuf::from(&conflict.path)),
        target: conflict.target.as_deref().map(|target| {
            paths::to_verbatim(Path::new(target)).unwrap_or_else(|_| PathBuf::from(target))
        }),
        reason: conflict.reason.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use freespace_ops::Store;

    use super::{CONFLICT_DUPLICATE, CONFLICT_EXISTS, RenameService, conflict_reason_key};
    use crate::dto::{RenameCase, RenamePreviewRequest};
    use crate::error::UiErrorCode;

    fn request(targets: &[PathBuf]) -> RenamePreviewRequest {
        RenamePreviewRequest {
            targets: targets
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            regex: r"^IMG_(\d+)".to_owned(),
            replacement: "photo-$1".to_owned(),
            scope: None,
            case: RenameCase::Preserve,
        }
    }

    #[test]
    fn previews_and_applies_a_rename() {
        let directory = tempfile::tempdir().expect("temp dir");
        let first = directory.path().join("IMG_0001.jpg");
        let second = directory.path().join("IMG_0002.jpg");
        fs::write(&first, b"one").expect("first");
        fs::write(&second, b"two").expect("second");
        let service = RenameService::new();
        let preview = service
            .preview(
                &RenamePreviewRequest {
                    scope: Some(directory.path().to_string_lossy().into_owned()),
                    ..request(&[first.clone(), second.clone()])
                },
                0,
            )
            .expect("preview");

        assert_eq!(preview.change_count, 2);
        assert_eq!(preview.total_size, 6);
        assert!(preview.conflicts.is_empty());
        assert!(!preview.requires_elevation);
        assert_eq!(preview.rows[0].new_name, "photo-0001.jpg");
        assert!(!preview.rows[0].conflict);

        let store = Store::open_in_memory().expect("store");
        let applied = service
            .apply(
                &preview.preview_id,
                &preview.confirmation_token,
                Some(&store),
                false,
                1,
            )
            .expect("apply");

        assert_eq!(applied.renamed, 2);
        assert!(applied.journal_entry_id.is_some());
        assert!(applied.undo_available);
        assert!(directory.path().join("photo-0001.jpg").exists());
        assert!(!first.exists());
        assert_eq!(service.preview_count(), 0);
    }

    #[test]
    fn reports_existing_and_duplicate_targets() {
        let directory = tempfile::tempdir().expect("temp dir");
        let first = directory.path().join("IMG_0001.jpg");
        let second = directory.path().join("IMG_0002.jpg");
        fs::write(&first, b"one").expect("first");
        fs::write(&second, b"two").expect("second");
        fs::write(directory.path().join("photo-0001.jpg"), b"three").expect("existing");
        let service = RenameService::new();

        let preview = service
            .preview(&request(&[first.clone(), second.clone()]), 0)
            .expect("preview");
        assert!(preview.rows[0].conflict);
        assert!(!preview.rows[1].conflict);
        assert!(
            preview
                .conflicts
                .iter()
                .any(|conflict| conflict.reason_key == CONFLICT_EXISTS)
        );
        assert!(
            preview
                .warnings
                .contains(&"rename.warning.conflicts".to_owned())
        );

        let error = service
            .apply(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                1,
            )
            .expect_err("conflicts");
        assert_eq!(error.code, UiErrorCode::Validation);
        assert!(first.exists());

        let duplicate = RenamePreviewRequest {
            regex: r"IMG_\d+".to_owned(),
            replacement: "same".to_owned(),
            ..request(&[first, second])
        };
        let preview = service.preview(&duplicate, 0).expect("duplicate preview");
        assert!(
            preview
                .conflicts
                .iter()
                .any(|conflict| conflict.reason_key == CONFLICT_DUPLICATE)
        );
    }

    #[test]
    fn a_token_is_one_time_and_expires() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("IMG_0001.jpg");
        fs::write(&path, b"payload").expect("write");
        let service = RenameService::with_ttl(1_000);
        let preview = service.preview(&request(&[path]), 0).expect("preview");
        assert_eq!(preview.expires_at, 1_000);

        let expired = service
            .apply(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                1_001,
            )
            .expect_err("expired");
        assert_eq!(expired.code, UiErrorCode::PreviewExpired);

        let preview = service
            .preview(&request(&[directory.path().join("IMG_0001.jpg")]), 0)
            .expect("second preview");
        service
            .apply(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                1,
            )
            .expect("first take");
        let reused = service
            .apply(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                2,
            )
            .expect_err("reuse");
        assert_eq!(reused.code, UiErrorCode::PreviewExpired);
    }

    #[test]
    fn rejects_a_target_outside_the_scope() {
        let directory = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside");
        let path = outside.path().join("IMG_0001.jpg");
        fs::write(&path, b"payload").expect("write");
        let service = RenameService::new();

        let error = service
            .preview(
                &RenamePreviewRequest {
                    scope: Some(directory.path().to_string_lossy().into_owned()),
                    ..request(&[path])
                },
                0,
            )
            .expect_err("outside");
        assert_eq!(error.code, UiErrorCode::AccessDenied);
    }

    #[test]
    fn rejects_an_invalid_pattern() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("IMG_0001.jpg");
        fs::write(&path, b"payload").expect("write");
        let service = RenameService::new();

        let error = service
            .preview(
                &RenamePreviewRequest {
                    regex: "[".to_owned(),
                    ..request(&[path])
                },
                0,
            )
            .expect_err("invalid regex");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    #[test]
    fn applies_a_rename_beyond_max_path() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut current = directory.path().to_path_buf();
        for index in 0..8 {
            current.push(format!("segment-{index}-{}", "x".repeat(40)));
            fs::create_dir_all(freespace_platform::paths::to_verbatim(&current).expect("verbatim"))
                .expect("mkdir");
        }
        let path = current.join("IMG_9999.bin");
        fs::write(
            freespace_platform::paths::to_verbatim(&path).expect("verbatim"),
            b"payload",
        )
        .expect("write");
        assert!(path.as_os_str().len() > 270);

        let service = RenameService::new();
        let preview = service
            .preview(&request(std::slice::from_ref(&path)), 0)
            .expect("preview");
        assert_eq!(preview.change_count, 1);
        assert!(preview.conflicts.is_empty());

        let applied = service
            .apply(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                1,
            )
            .expect("apply");
        assert_eq!(applied.renamed, 1);
        assert!(
            freespace_platform::paths::to_verbatim(&current.join("photo-9999.bin"))
                .expect("verbatim")
                .exists()
        );
        assert!(
            !freespace_platform::paths::to_verbatim(&path)
                .expect("verbatim")
                .exists()
        );
    }

    #[test]
    fn protected_targets_need_elevation_before_anything_changes() {
        let service = RenameService::new();
        let protected =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()))
                .join("System32")
                .join("kernel32.dll");
        let preview = service
            .preview(
                &RenamePreviewRequest {
                    regex: r"^kernel32".to_owned(),
                    replacement: "kernel32-backup".to_owned(),
                    ..request(&[protected])
                },
                0,
            )
            .expect("preview");
        assert!(preview.requires_elevation);
        assert!(
            preview
                .warnings
                .contains(&"rename.warning.protected".to_owned())
        );

        let refused = service
            .apply(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                1,
            )
            .expect_err("elevation");
        assert_eq!(refused.code, UiErrorCode::ElevationRequired);
    }

    #[test]
    fn classifies_conflict_reasons() {
        assert_eq!(
            conflict_reason_key("the target already exists"),
            CONFLICT_EXISTS
        );
        assert_eq!(
            conflict_reason_key("several objects would get the same name"),
            CONFLICT_DUPLICATE
        );
        assert_eq!(
            conflict_reason_key("`CON` is a reserved device name"),
            super::CONFLICT_RESERVED
        );
        assert_eq!(
            conflict_reason_key("path has 40000 characters, limit is 32000"),
            super::CONFLICT_PATH_LIMIT
        );
        assert_eq!(
            conflict_reason_key("a rename rule produced an empty name"),
            super::CONFLICT_INVALID_NAME
        );
    }
}
