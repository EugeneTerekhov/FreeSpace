//! Permission and attribute previews (US4, T101) — portable stub.
//!
//! NTFS ACL and attribute editing lives in `permissions.rs` and is compiled on
//! Windows only. On every other platform the capability flags `acl` and `attrs`
//! are `false`; the same public API stays available so CLI and GUI call sites
//! keep compiling, and every operation returns `E_UNSUPPORTED` before touching
//! the file system.

use std::path::PathBuf;

use freespace_ops::Store;
use uuid::Uuid;

use crate::{
    dto::{
        AttributesPreviewRequest, FileOperationApplied, OperationPreview, PermissionsPreview,
        PermissionsPreviewRequest, PermissionsView,
    },
    error::{UiError, UiErrorCode},
};

/// A permission change waiting for its one-time token.
///
/// Present for API compatibility; a preview is never created on this platform.
#[derive(Debug, Clone)]
pub struct PreparedPermissions {
    /// One-time preview identifier.
    pub preview_id: String,
    /// One-time confirmation token.
    pub token: String,
    /// Expiry, Unix milliseconds.
    pub expires_at: i64,
    /// Object whose permissions change.
    pub path: PathBuf,
    /// Requested changes.
    pub changes: crate::dto::PermissionsChanges,
    /// True when applying needs an elevated process.
    pub requires_elevation: bool,
    /// Identifier returned by the apply call.
    pub operation_id: Uuid,
}

/// An attribute change waiting for its one-time token.
///
/// Present for API compatibility; a preview is never created on this platform.
#[derive(Debug, Clone)]
pub struct PreparedAttributes {
    /// One-time preview identifier.
    pub preview_id: String,
    /// One-time confirmation token.
    pub token: String,
    /// Expiry, Unix milliseconds.
    pub expires_at: i64,
    /// Targets of the change.
    pub targets: Vec<PathBuf>,
    /// Bits to set and clear.
    pub change: (u32, u32),
    /// True when applying needs an elevated process.
    pub requires_elevation: bool,
    /// Identifier returned by the apply call.
    pub operation_id: Uuid,
}

/// Preview registry of the permissions screen.
///
/// On this platform it only keeps the TTL for API compatibility and refuses
/// every request.
#[derive(Debug)]
pub struct PermissionsService {
    #[allow(dead_code)]
    ttl_ms: i64,
}

impl Default for PermissionsService {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionsService {
    /// Creates the service with the documented ten-minute TTL.
    #[must_use]
    pub fn new() -> Self {
        Self::with_ttl(crate::operations::PREVIEW_TTL_MS)
    }

    /// Creates a service with an explicit TTL; used by tests.
    #[must_use]
    pub fn with_ttl(ttl_ms: i64) -> Self {
        Self { ttl_ms }
    }

    /// Permission and attribute changes are unsupported on this platform.
    ///
    /// # Errors
    /// Always returns `E_UNSUPPORTED`.
    pub fn view(&self, path: &str) -> Result<PermissionsView, UiError> {
        let _ = path;
        unsupported()
    }

    /// Permission changes are unsupported on this platform.
    ///
    /// # Errors
    /// Always returns `E_UNSUPPORTED`.
    pub fn preview(
        &self,
        request: &PermissionsPreviewRequest,
        now_ms: i64,
    ) -> Result<PermissionsPreview, UiError> {
        let _ = (request, now_ms);
        unsupported()
    }

    /// Permission changes are unsupported on this platform.
    ///
    /// # Errors
    /// Always returns `E_UNSUPPORTED`.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &self,
        preview_id: &str,
        confirmation_token: &str,
        store: Option<&Store>,
        elevated: bool,
        now_ms: i64,
    ) -> Result<FileOperationApplied, UiError> {
        let _ = (preview_id, confirmation_token, store, elevated, now_ms);
        unsupported()
    }

    /// Attribute changes are unsupported on this platform.
    ///
    /// # Errors
    /// Always returns `E_UNSUPPORTED`.
    pub fn attributes_preview(
        &self,
        request: &AttributesPreviewRequest,
        now_ms: i64,
    ) -> Result<OperationPreview, UiError> {
        let _ = (request, now_ms);
        unsupported()
    }

    /// Attribute changes are unsupported on this platform.
    ///
    /// # Errors
    /// Always returns `E_UNSUPPORTED`.
    #[allow(clippy::too_many_arguments)]
    pub fn attributes_apply(
        &self,
        preview_id: &str,
        confirmation_token: &str,
        store: Option<&Store>,
        elevated: bool,
        now_ms: i64,
    ) -> Result<FileOperationApplied, UiError> {
        let _ = (preview_id, confirmation_token, store, elevated, now_ms);
        unsupported()
    }

    /// Number of live previews; always zero on this platform.
    #[must_use]
    pub fn preview_count(&self) -> usize {
        0
    }
}

/// Reads the owner, attributes and protection state of one path.
///
/// # Errors
/// Always returns `E_UNSUPPORTED` on this platform.
pub fn permissions_view(path: &str) -> Result<PermissionsView, UiError> {
    let _ = path;
    unsupported()
}

/// Builds a permission preview with the process-wide registry.
///
/// # Errors
/// Always returns `E_UNSUPPORTED` on this platform.
pub fn preview(
    request: &PermissionsPreviewRequest,
    now_ms: i64,
) -> Result<PermissionsPreview, UiError> {
    let _ = (request, now_ms);
    unsupported()
}

/// Applies a permission preview from the process-wide registry.
///
/// # Errors
/// Always returns `E_UNSUPPORTED` on this platform.
#[allow(clippy::too_many_arguments)]
pub fn apply(
    preview_id: &str,
    confirmation_token: &str,
    store: Option<&Store>,
    elevated: bool,
    now_ms: i64,
) -> Result<FileOperationApplied, UiError> {
    let _ = (preview_id, confirmation_token, store, elevated, now_ms);
    unsupported()
}

/// Builds an attribute preview with the process-wide registry.
///
/// # Errors
/// Always returns `E_UNSUPPORTED` on this platform.
pub fn attributes_preview(
    request: &AttributesPreviewRequest,
    now_ms: i64,
) -> Result<OperationPreview, UiError> {
    let _ = (request, now_ms);
    unsupported()
}

/// Applies an attribute preview from the process-wide registry.
///
/// # Errors
/// Always returns `E_UNSUPPORTED` on this platform.
#[allow(clippy::too_many_arguments)]
pub fn attributes_apply(
    preview_id: &str,
    confirmation_token: &str,
    store: Option<&Store>,
    elevated: bool,
    now_ms: i64,
) -> Result<FileOperationApplied, UiError> {
    let _ = (preview_id, confirmation_token, store, elevated, now_ms);
    unsupported()
}

fn unsupported<T>() -> Result<T, UiError> {
    Err(UiError::new(UiErrorCode::Unsupported)
        .with_details("permission and attribute changes are available on Windows only"))
}

#[cfg(test)]
mod tests {
    use freespace_ops::Store;

    use super::{apply, attributes_apply, permissions_view, preview};
    use crate::dto::{
        AttributeChangeSet, AttributesPreviewRequest, PermissionsChanges, PermissionsPreviewRequest,
    };
    use crate::error::UiErrorCode;

    #[test]
    fn every_permission_operation_is_unsupported() {
        assert_eq!(
            permissions_view("/tmp").expect_err("view").code,
            UiErrorCode::Unsupported
        );

        let preview_request = PermissionsPreviewRequest {
            path: "/tmp".to_owned(),
            changes: PermissionsChanges {
                attributes: Some(AttributeChangeSet::default()),
                ..PermissionsChanges::default()
            },
        };
        assert_eq!(
            preview(&preview_request, 0).expect_err("preview").code,
            UiErrorCode::Unsupported
        );

        let store = Store::open_in_memory().expect("store");
        assert_eq!(
            apply("id", "token", Some(&store), true, 0)
                .expect_err("apply")
                .code,
            UiErrorCode::Unsupported
        );

        let attributes = AttributesPreviewRequest {
            targets: vec!["/tmp".to_owned()],
            changes: AttributeChangeSet::default(),
        };
        assert_eq!(
            crate::permissions::attributes_preview(&attributes, 0)
                .expect_err("attributes preview")
                .code,
            UiErrorCode::Unsupported
        );
        assert_eq!(
            attributes_apply("id", "token", None, true, 0)
                .expect_err("attributes apply")
                .code,
            UiErrorCode::Unsupported
        );
    }
}
