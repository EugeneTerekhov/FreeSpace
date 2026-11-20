//! Permission and attribute previews (US4, T101) — Windows implementation.
//!
//! This module is compiled on Windows only; on other platforms
//! `permissions_unix.rs` provides the same public API and returns
//! `E_UNSUPPORTED` because the `acl` and `attrs` capabilities are `false`.
//!
//! A permission change is read-only until its one-time token is consumed. The
//! preview resolves the current owner, the raw attribute mask and every planned
//! attribute bit into an exact before/after diff, marks the changes that need an
//! elevated process and stores the request; [`PermissionsService::apply`] then
//! hands the batch to `freespace-ops`.
//!
//! Owner and copied-ACL changes need `SeTakeOwnershipPrivilege` /
//! `SeRestorePrivilege`, so they always require elevation. Attribute and ACL
//! resets on ordinary user data do not, because the operating system already
//! grants the owner the right to change them.

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex, MutexGuard},
};

use freespace_core::tree::FileAttributes;
use freespace_ops::{
    AclAction, AttributeChange, Store, acl::apply_acl as apply_acl_batch,
    attributes::apply_attributes as apply_attributes_batch, confirm::is_protected_path,
};
use freespace_platform::{paths, security};
use uuid::Uuid;

use crate::{
    dto::{
        AttributeChangeRow, AttributeChangeSet, AttributesPreviewRequest, FileOperationApplied,
        NodeKind, OperationKind, OperationPreview, OperationTarget, PermissionsAclAction,
        PermissionsChanges, PermissionsPreview, PermissionsPreviewRequest, PermissionsView,
    },
    error::{UiError, UiErrorCode},
    operations::PREVIEW_TTL_MS,
    size_details::flags,
    validation::validate_path,
};

/// A permission change waiting for its one-time token.
#[derive(Debug, Clone)]
pub struct PreparedPermissions {
    /// One-time preview identifier.
    pub preview_id: String,
    /// One-time confirmation token.
    pub token: String,
    /// Expiry, Unix milliseconds.
    pub expires_at: i64,
    /// Object whose permissions change, in verbatim form.
    pub path: PathBuf,
    /// Requested changes.
    pub changes: PermissionsChanges,
    /// True when applying needs an elevated process.
    pub requires_elevation: bool,
    /// Identifier returned by the apply call.
    pub operation_id: Uuid,
}

/// An attribute change waiting for its one-time token.
#[derive(Debug, Clone)]
pub struct PreparedAttributes {
    /// One-time preview identifier.
    pub preview_id: String,
    /// One-time confirmation token.
    pub token: String,
    /// Expiry, Unix milliseconds.
    pub expires_at: i64,
    /// Targets in verbatim form.
    pub targets: Vec<PathBuf>,
    /// Bits to set and clear.
    pub change: (u32, u32),
    /// True when applying needs an elevated process.
    pub requires_elevation: bool,
    /// Identifier returned by the apply call.
    pub operation_id: Uuid,
}

#[derive(Debug, Clone)]
enum PreparedChange {
    Permissions(PreparedPermissions),
    Attributes(PreparedAttributes),
}

#[derive(Debug, Default)]
struct ServiceInner {
    previews: HashMap<String, PreparedChange>,
}

/// Preview registry of the permissions screen.
#[derive(Debug)]
pub struct PermissionsService {
    inner: Mutex<ServiceInner>,
    ttl_ms: i64,
}

impl Default for PermissionsService {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionsService {
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

    /// Reads the owner, the attribute mask and the protection state of `path`.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for a malformed path, `E_NOT_FOUND` for a missing
    /// object and `E_ACCESS_DENIED` when the owner cannot be read.
    pub fn view(&self, path: &str) -> Result<PermissionsView, UiError> {
        permissions_view(path)
    }

    /// Builds a permission preview without touching the object.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for a malformed request, `E_NOT_FOUND` for a
    /// missing object or copy source and `E_ACCESS_DENIED` when the current
    /// state cannot be read.
    pub fn preview(
        &self,
        request: &PermissionsPreviewRequest,
        now_ms: i64,
    ) -> Result<PermissionsPreview, UiError> {
        request.validate()?;
        let path = validate_path(&request.path)?;
        let current = permissions_view(&request.path)?;
        let verbatim = paths::to_verbatim(&path).map_err(|error| {
            UiError::validation(format!("`{}`: {error}", paths::display(&path)))
        })?;

        if let Some(source) = &request.changes.copy_from {
            let source = validate_path(source)?;
            let source_verbatim = paths::to_verbatim(&source).map_err(|error| {
                UiError::validation(format!("`{}`: {error}", paths::display(&source)))
            })?;
            fs::metadata(&source_verbatim).map_err(|error| {
                let code = match error.kind() {
                    io::ErrorKind::NotFound => UiErrorCode::NotFound,
                    io::ErrorKind::PermissionDenied => UiErrorCode::AccessDenied,
                    _ => UiErrorCode::Io,
                };
                UiError::new(code).with_details(paths::display(&source))
            })?;
        }

        let attribute_changes =
            attribute_diff(current.attribute_mask, request.changes.attributes.as_ref());
        let acl_action = acl_action(&request.changes)?;
        let protected = is_protected_path(&verbatim);
        let requires_elevation = protected
            || matches!(
                acl_action,
                Some(AclAction::SetOwner(_) | AclAction::CopyFrom(_))
            );
        let mut warnings = Vec::new();
        if protected {
            warnings.push("permissions.warning.protected".to_owned());
        }
        if requires_elevation {
            warnings.push("permissions.warning.elevation".to_owned());
        }

        let preview_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let expires_at = now_ms.saturating_add(self.ttl_ms);
        let preview = PermissionsPreview {
            preview_id: preview_id.clone(),
            confirmation_token: token.clone(),
            expires_at,
            path: current.path.clone(),
            current,
            owner: request.changes.owner.clone(),
            acl_action: acl_action.as_ref().map(acl_action_kind),
            copy_from: request.changes.copy_from.clone(),
            attribute_changes,
            requires_elevation,
            warnings,
        };

        self.lock().previews.insert(
            preview_id.clone(),
            PreparedChange::Permissions(PreparedPermissions {
                preview_id,
                token,
                expires_at,
                path: verbatim,
                changes: request.changes.clone(),
                requires_elevation,
                operation_id: Uuid::new_v4(),
            }),
        );

        Ok(preview)
    }

    /// Consumes a permission preview and applies its changes.
    ///
    /// # Errors
    /// Returns `E_PREVIEW_EXPIRED` for an unknown, used or expired preview,
    /// `E_ELEVATION_REQUIRED` when an elevated process is needed and no change
    /// was attempted, and the mapped `freespace-ops` error otherwise.
    pub fn apply(
        &self,
        preview_id: &str,
        confirmation_token: &str,
        store: Option<&Store>,
        elevated: bool,
        now_ms: i64,
    ) -> Result<FileOperationApplied, UiError> {
        let prepared = self.take(preview_id, confirmation_token, now_ms)?;
        let PreparedChange::Permissions(prepared) = prepared else {
            return Err(UiError::new(UiErrorCode::PreviewExpired));
        };
        if prepared.requires_elevation && !elevated {
            return Err(UiError::new(UiErrorCode::ElevationRequired)
                .with_details("the permission change needs an elevated process"));
        }

        let mut result = FileOperationApplied {
            operation_id: prepared.operation_id.to_string(),
            journal_entry_ids: Vec::new(),
            changed: 0,
            failed: 0,
            errors: Vec::new(),
        };

        if let Some(action) = acl_action(&prepared.changes)? {
            let report = apply_acl_batch(store, std::slice::from_ref(&prepared.path), &action)?;
            result.journal_entry_ids.extend(report.journal_id);
            for item in &report.items {
                collect_item(&mut result, &item.path, item.error.as_deref());
            }
        }

        if let Some(changes) = &prepared.changes.attributes {
            let report = apply_attributes_batch(
                store,
                std::slice::from_ref(&prepared.path),
                AttributeChange {
                    set: changes.set_mask(),
                    clear: changes.clear_mask(),
                },
            )?;
            result.journal_entry_ids.extend(report.journal_id);
            for item in &report.items {
                collect_item(&mut result, &item.path, item.error.as_deref());
            }
        }

        Ok(result)
    }

    /// Builds an attribute preview for a batch of targets.
    ///
    /// The response reuses the shared [`OperationPreview`] shape so the confirm
    /// dialog is the same one the destructive screens use.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for a malformed request and `E_ACCESS_DENIED` for
    /// a target whose attributes cannot be read.
    pub fn attributes_preview(
        &self,
        request: &AttributesPreviewRequest,
        now_ms: i64,
    ) -> Result<OperationPreview, UiError> {
        request.validate()?;

        let mut targets = Vec::with_capacity(request.targets.len());
        let mut verbatim_targets = Vec::with_capacity(request.targets.len());
        let mut total_size = 0u64;
        let mut requires_elevation = false;
        for raw in &request.targets {
            let path = validate_path(raw)?;
            let verbatim = paths::to_verbatim(&path).map_err(|error| {
                UiError::validation(format!("`{}`: {error}", paths::display(&path)))
            })?;
            let metadata = fs::symlink_metadata(&verbatim).map_err(|error| {
                let code = match error.kind() {
                    io::ErrorKind::NotFound => UiErrorCode::NotFound,
                    io::ErrorKind::PermissionDenied => UiErrorCode::AccessDenied,
                    _ => UiErrorCode::Io,
                };
                UiError::new(code).with_details(paths::display(&path))
            })?;
            let protected = is_protected_path(&verbatim);
            requires_elevation |= protected;
            total_size = total_size.saturating_add(metadata.len());
            targets.push(OperationTarget {
                path: paths::display(&verbatim),
                kind: object_kind(&metadata),
                size: metadata.len(),
                accessible: true,
                is_system: protected,
            });
            verbatim_targets.push(verbatim);
        }

        let mut warnings = Vec::new();
        if requires_elevation {
            warnings.push("permissions.warning.protected".to_owned());
            warnings.push("permissions.warning.elevation".to_owned());
        }

        let preview_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let expires_at = now_ms.saturating_add(self.ttl_ms);
        let preview = OperationPreview {
            preview_id: preview_id.clone(),
            kind: OperationKind::Permissions,
            targets,
            total_size,
            delete_mode: None,
            reversible: false,
            requires_elevation,
            warnings,
            expires_at,
            confirmation_token: token.clone(),
        };

        self.lock().previews.insert(
            preview_id.clone(),
            PreparedChange::Attributes(PreparedAttributes {
                preview_id,
                token,
                expires_at,
                targets: verbatim_targets,
                change: (request.changes.set_mask(), request.changes.clear_mask()),
                requires_elevation,
                operation_id: Uuid::new_v4(),
            }),
        );

        Ok(preview)
    }

    /// Consumes an attribute preview and applies it.
    ///
    /// # Errors
    /// Returns `E_PREVIEW_EXPIRED` for an unknown, used or expired preview,
    /// `E_ELEVATION_REQUIRED` when a target is protected and the process is not
    /// elevated, and the mapped `freespace-ops` error otherwise.
    pub fn attributes_apply(
        &self,
        preview_id: &str,
        confirmation_token: &str,
        store: Option<&Store>,
        elevated: bool,
        now_ms: i64,
    ) -> Result<FileOperationApplied, UiError> {
        let prepared = self.take(preview_id, confirmation_token, now_ms)?;
        let PreparedChange::Attributes(prepared) = prepared else {
            return Err(UiError::new(UiErrorCode::PreviewExpired));
        };
        if prepared.requires_elevation && !elevated {
            return Err(UiError::new(UiErrorCode::ElevationRequired)
                .with_details("the attribute change touches a protected location"));
        }

        let report = apply_attributes_batch(
            store,
            &prepared.targets,
            AttributeChange {
                set: prepared.change.0,
                clear: prepared.change.1,
            },
        )?;
        let mut result = FileOperationApplied {
            operation_id: prepared.operation_id.to_string(),
            journal_entry_ids: report.journal_id.into_iter().collect(),
            changed: 0,
            failed: 0,
            errors: Vec::new(),
        };
        for item in &report.items {
            collect_item(&mut result, &item.path, item.error.as_deref());
        }

        Ok(result)
    }

    /// Number of live previews; used by tests.
    #[must_use]
    pub fn preview_count(&self) -> usize {
        self.lock().previews.len()
    }

    fn take(
        &self,
        preview_id: &str,
        confirmation_token: &str,
        now_ms: i64,
    ) -> Result<PreparedChange, UiError> {
        let mut inner = self.lock();
        let Some(prepared) = inner.previews.get(preview_id).cloned() else {
            return Err(UiError::new(UiErrorCode::PreviewExpired));
        };
        let (token, expires_at) = match &prepared {
            PreparedChange::Permissions(prepared) => (&prepared.token, prepared.expires_at),
            PreparedChange::Attributes(prepared) => (&prepared.token, prepared.expires_at),
        };
        if token != confirmation_token || now_ms > expires_at {
            inner.previews.remove(preview_id);
            return Err(UiError::new(UiErrorCode::PreviewExpired));
        }
        inner.previews.remove(preview_id);

        Ok(prepared)
    }

    fn lock(&self) -> MutexGuard<'_, ServiceInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Process-wide registry used by the Tauri commands.
static SERVICE: LazyLock<PermissionsService> = LazyLock::new(PermissionsService::new);

/// Reads the owner, attributes and protection state of one path.
///
/// # Errors
/// See [`PermissionsService::view`].
pub fn permissions_view(path: &str) -> Result<PermissionsView, UiError> {
    let path = validate_path(path)?;
    let verbatim = paths::to_verbatim(&path)
        .map_err(|error| UiError::validation(format!("`{}`: {error}", paths::display(&path))))?;
    let metadata = fs::symlink_metadata(&verbatim).map_err(|error| {
        let code = match error.kind() {
            io::ErrorKind::NotFound => UiErrorCode::NotFound,
            io::ErrorKind::PermissionDenied => UiErrorCode::AccessDenied,
            _ => UiErrorCode::Io,
        };
        UiError::new(code).with_details(paths::display(&path))
    })?;
    let attribute_mask = security::get_attributes(&verbatim)?;
    let identity = security::owner_identity(&verbatim)?;
    let protected = is_protected_path(&verbatim);

    Ok(PermissionsView {
        path: paths::display(&verbatim),
        owner: identity
            .as_ref()
            .map(|identity| identity.display_name.clone()),
        owner_sid: identity.as_ref().map(|identity| identity.sid.clone()),
        attribute_mask,
        attributes: flags(attribute_mask),
        is_directory: metadata.is_dir(),
        is_reparse_point: attribute_mask & FileAttributes::REPARSE_POINT.bits() != 0,
        is_protected: protected,
        requires_elevation: protected,
    })
}

/// Builds a permission preview with the process-wide registry.
///
/// # Errors
/// See [`PermissionsService::preview`].
pub fn preview(
    request: &PermissionsPreviewRequest,
    now_ms: i64,
) -> Result<PermissionsPreview, UiError> {
    SERVICE.preview(request, now_ms)
}

/// Applies a permission preview from the process-wide registry.
///
/// # Errors
/// See [`PermissionsService::apply`].
pub fn apply(
    preview_id: &str,
    confirmation_token: &str,
    store: Option<&Store>,
    elevated: bool,
    now_ms: i64,
) -> Result<FileOperationApplied, UiError> {
    SERVICE.apply(preview_id, confirmation_token, store, elevated, now_ms)
}

/// Builds an attribute preview with the process-wide registry.
///
/// # Errors
/// See [`PermissionsService::attributes_preview`].
pub fn attributes_preview(
    request: &AttributesPreviewRequest,
    now_ms: i64,
) -> Result<OperationPreview, UiError> {
    SERVICE.attributes_preview(request, now_ms)
}

/// Applies an attribute preview from the process-wide registry.
///
/// # Errors
/// See [`PermissionsService::attributes_apply`].
pub fn attributes_apply(
    preview_id: &str,
    confirmation_token: &str,
    store: Option<&Store>,
    elevated: bool,
    now_ms: i64,
) -> Result<FileOperationApplied, UiError> {
    SERVICE.attributes_apply(preview_id, confirmation_token, store, elevated, now_ms)
}

fn acl_action(changes: &PermissionsChanges) -> Result<Option<AclAction>, UiError> {
    if let Some(owner) = &changes.owner {
        return Ok(Some(AclAction::SetOwner(owner.clone())));
    }
    if changes.reset {
        return Ok(Some(AclAction::Reset));
    }
    if let Some(source) = &changes.copy_from {
        return Ok(Some(AclAction::CopyFrom(validate_path(source)?)));
    }

    Ok(None)
}

fn acl_action_kind(action: &AclAction) -> PermissionsAclAction {
    match action {
        AclAction::SetOwner(_) => PermissionsAclAction::Owner,
        AclAction::Reset => PermissionsAclAction::Reset,
        AclAction::CopyFrom(_) => PermissionsAclAction::Copy,
    }
}

fn attribute_diff(mask: u32, changes: Option<&AttributeChangeSet>) -> Vec<AttributeChangeRow> {
    let Some(changes) = changes else {
        return Vec::new();
    };

    let after_mask = (mask | changes.set_mask()) & !changes.clear_mask();
    changes
        .set
        .iter()
        .chain(changes.clear.iter())
        .map(|name| AttributeChangeRow {
            attribute: *name,
            before: name.is_set(mask),
            after: name.is_set(after_mask),
        })
        .collect()
}

fn object_kind(metadata: &fs::Metadata) -> NodeKind {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.file_attributes() & FileAttributes::REPARSE_POINT.bits() != 0 {
            return NodeKind::Link;
        }
    }
    if metadata.is_dir() {
        NodeKind::Directory
    } else {
        NodeKind::File
    }
}

fn collect_item(result: &mut FileOperationApplied, path: &Path, error: Option<&str>) {
    match error {
        Some(error) => {
            result.failed = result.failed.saturating_add(1);
            let lowered = error.to_lowercase();
            let code = if lowered.contains("denied") || lowered.contains("privilege") {
                UiErrorCode::AccessDenied
            } else {
                UiErrorCode::Io
            };
            result.errors.push(
                UiError::new(code).with_details(format!("{}: {error}", paths::display(path))),
            );
        }
        None => result.changed = result.changed.saturating_add(1),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use freespace_ops::{Store, attributes};

    use super::{PermissionsService, permissions_view};
    use crate::dto::{
        AttributeChangeSet, AttributeName, AttributesPreviewRequest, PermissionsChanges,
        PermissionsPreviewRequest,
    };
    use crate::error::UiErrorCode;

    fn temp_file() -> (tempfile::TempDir, String) {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        fs::write(&path, b"payload").expect("write");

        (directory, path.to_string_lossy().into_owned())
    }

    fn attribute_request(path: &str) -> PermissionsPreviewRequest {
        PermissionsPreviewRequest {
            path: path.to_owned(),
            changes: PermissionsChanges {
                attributes: Some(AttributeChangeSet {
                    set: vec![AttributeName::Hidden],
                    clear: vec![AttributeName::Archive],
                }),
                ..PermissionsChanges::default()
            },
        }
    }

    #[test]
    fn reads_owner_and_attributes() {
        let (_directory, path) = temp_file();
        let view = permissions_view(&path).expect("view");

        assert!(view.owner.is_some());
        assert!(
            view.owner_sid
                .as_deref()
                .is_some_and(|sid| sid.starts_with("S-1-"))
        );
        assert!(AttributeName::Archive.is_set(view.attribute_mask));
        assert!(!view.is_directory);
        assert!(!view.is_protected);
    }

    #[test]
    fn previews_an_attribute_diff_without_changing_anything() {
        let (_directory, path) = temp_file();
        let service = PermissionsService::new();
        let before = permissions_view(&path).expect("view");
        let preview = service
            .preview(&attribute_request(&path), 0)
            .expect("preview");

        assert_eq!(preview.attribute_changes.len(), 2);
        let hidden = preview
            .attribute_changes
            .iter()
            .find(|row| row.attribute == AttributeName::Hidden)
            .expect("hidden row");
        assert!(!hidden.before);
        assert!(hidden.after);
        let archive = preview
            .attribute_changes
            .iter()
            .find(|row| row.attribute == AttributeName::Archive)
            .expect("archive row");
        assert!(archive.before);
        assert!(!archive.after);
        assert!(!preview.requires_elevation);
        assert_eq!(service.preview_count(), 1);

        let after = permissions_view(&path).expect("view");
        assert_eq!(before.attribute_mask, after.attribute_mask);
    }

    #[test]
    fn applies_an_attribute_change_and_journals_it() {
        let (_directory, path) = temp_file();
        let service = PermissionsService::new();
        let preview = service
            .preview(&attribute_request(&path), 0)
            .expect("preview");
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

        assert_eq!(applied.changed, 1);
        assert_eq!(applied.failed, 0);
        assert_eq!(applied.journal_entry_ids.len(), 1);
        let mask = freespace_platform::security::get_attributes(std::path::Path::new(&path))
            .expect("attributes");
        assert_eq!(mask & attributes::HIDDEN, attributes::HIDDEN);
        assert_eq!(mask & attributes::ARCHIVE, 0);
        assert_eq!(service.preview_count(), 0);
    }

    #[test]
    fn a_token_is_one_time_and_expires() {
        let (_directory, path) = temp_file();
        let service = PermissionsService::with_ttl(1_000);
        let preview = service
            .preview(&attribute_request(&path), 0)
            .expect("preview");

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
            .preview(&attribute_request(&path), 0)
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
    fn an_owner_change_needs_elevation_and_changes_nothing_without_it() {
        let (_directory, path) = temp_file();
        let owner = permissions_view(&path).expect("view").owner;
        let service = PermissionsService::new();
        let request = PermissionsPreviewRequest {
            path: path.clone(),
            changes: PermissionsChanges {
                owner: Some(owner.clone().unwrap_or_else(|| "Users".to_owned())),
                ..PermissionsChanges::default()
            },
        };
        let preview = service.preview(&request, 0).expect("preview");
        assert!(preview.requires_elevation);
        assert!(
            preview
                .warnings
                .contains(&"permissions.warning.elevation".to_owned())
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
        assert_eq!(permissions_view(&path).expect("view").owner, owner);
    }

    #[test]
    fn previews_and_applies_a_whole_batch_of_attributes() {
        let directory = tempfile::tempdir().expect("temp dir");
        let first = directory.path().join("first.txt");
        let second = directory.path().join("second.txt");
        fs::write(&first, b"one").expect("first");
        fs::write(&second, b"two").expect("second");
        let service = PermissionsService::new();
        let request = AttributesPreviewRequest {
            targets: vec![
                first.to_string_lossy().into_owned(),
                second.to_string_lossy().into_owned(),
            ],
            changes: AttributeChangeSet {
                set: vec![AttributeName::ReadOnly],
                clear: Vec::new(),
            },
        };
        let preview = service.attributes_preview(&request, 0).expect("preview");
        assert_eq!(preview.kind, crate::dto::OperationKind::Permissions);
        assert_eq!(preview.targets.len(), 2);
        assert_eq!(preview.total_size, 6);
        assert!(!preview.requires_elevation);

        let store = Store::open_in_memory().expect("store");
        let applied = service
            .attributes_apply(
                &preview.preview_id,
                &preview.confirmation_token,
                Some(&store),
                false,
                1,
            )
            .expect("apply");
        assert_eq!(applied.changed, 2);
        assert_eq!(applied.journal_entry_ids.len(), 1);

        let mask = freespace_platform::security::get_attributes(&first).expect("attributes");
        assert_eq!(mask & attributes::READ_ONLY, attributes::READ_ONLY);
        // Restore the fixture so the temporary directory can be removed.
        freespace_platform::security::set_attributes(&first, 0, attributes::READ_ONLY)
            .expect("clear read-only");
    }

    #[test]
    fn rejects_an_empty_change_set() {
        let (_directory, path) = temp_file();
        let service = PermissionsService::new();
        let request = PermissionsPreviewRequest {
            path,
            changes: PermissionsChanges::default(),
        };

        let error = service.preview(&request, 0).expect_err("empty");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    #[test]
    fn reports_a_missing_object() {
        let directory = tempfile::tempdir().expect("temp dir");
        let missing = directory.path().join("missing.txt");
        let error = permissions_view(&missing.to_string_lossy()).expect_err("missing");

        assert_eq!(error.code, UiErrorCode::NotFound);
    }

    #[test]
    fn owner_change_applies_when_elevated_and_enabled() {
        if std::env::var("FREESPACE_ADMIN_TESTS").as_deref() != Ok("1") {
            return;
        }

        let (_directory, path) = temp_file();
        let owner = permissions_view(&path).expect("view").owner;
        let Some(owner) = owner else {
            return;
        };
        let service = PermissionsService::new();
        let preview = service
            .preview(
                &PermissionsPreviewRequest {
                    path: path.clone(),
                    changes: PermissionsChanges {
                        owner: Some(owner.clone()),
                        ..PermissionsChanges::default()
                    },
                },
                0,
            )
            .expect("preview");
        if !freespace_platform::elevation::is_elevated() {
            return;
        }

        let applied = service
            .apply(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                true,
                1,
            )
            .expect("apply");
        assert_eq!(applied.changed, 1);
        assert_eq!(permissions_view(&path).expect("view").owner, Some(owner));
    }
}
