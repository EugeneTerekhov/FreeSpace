//! Filesystem command payloads (US4).
//!
//! These are the wire shapes of the size-details, long-path, rename and
//! permission screens. Every request carries its own validation so a command can
//! reject malformed input before any file-system access; every response copies
//! numbers from Rust instead of inviting the interface to recompute them.
//!
//! Field names are `snake_case`; enum values use the documented lowercase or
//! kebab-case spellings. Sizes are bytes.

use serde::{Deserialize, Serialize};

use crate::{
    dto::{AttributeFlags, DEFAULT_PAGE_SIZE, FilterSet},
    error::UiError,
    validation::{validate_filter, validate_page_size, validate_path},
};

/// Maximum number of objects in one bulk rename.
pub const MAX_RENAME_TARGETS: usize = 5_000;
/// Maximum length of a rename pattern, in characters.
pub const MAX_RENAME_PATTERN_CHARS: usize = 512;
/// Maximum length of a rename replacement template, in characters.
pub const MAX_RENAME_REPLACEMENT_CHARS: usize = 1_024;
/// Maximum number of targets in one permission or attribute change.
pub const MAX_PERMISSION_TARGETS: usize = 5_000;
/// Maximum length of an owner account name, in characters.
pub const MAX_OWNER_CHARS: usize = 256;

fn default_page_size() -> u16 {
    DEFAULT_PAGE_SIZE
}

// ---------------------------------------------------------------------------
// Size details (T098)
// ---------------------------------------------------------------------------

/// Exact size breakdown of one object.
///
/// The interface renders these values verbatim: it never subtracts or rounds
/// them. `metric_label` is a localisation key naming the metric the object is
/// best described by, so the screen can caption the primary number without
/// guessing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeDetails {
    /// Full path of the inspected object.
    pub path: String,
    /// Display name of the object.
    pub name: String,
    /// Sum of the lengths of every stream, with every hard link counted.
    pub logical_size: u64,
    /// Space the scanner attributed to the object on the volume.
    pub on_disk_size: u64,
    /// Physical contribution with hard links counted once.
    pub physical_size: u64,
    /// Space actually allocated, measured with `GetCompressedFileSizeW`.
    pub allocated_size: u64,
    /// Part of `logical_size` already owned by other links to the same file.
    pub hardlink_contribution: u64,
    /// Total size of the named alternate data streams.
    pub ads_size: u64,
    /// Bytes saved by NTFS compression.
    pub compression_savings: u64,
    /// Bytes saved by sparse regions.
    pub sparse_savings: u64,
    /// Reparse-point target, when the object is a link or junction.
    pub link_target: Option<String>,
    /// Localisation key of the metric that best labels the object.
    pub metric_label: String,
    /// Attribute flags of the object.
    pub attributes: AttributeFlags,
    /// True when the numbers come from a scan and could not be re-measured.
    pub estimated: bool,
}

// ---------------------------------------------------------------------------
// Long paths (T099)
// ---------------------------------------------------------------------------

/// Request of `long_paths_page`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongPathPageRequest {
    /// Generation the page belongs to.
    pub generation: u64,
    /// Shared filter applied before pagination.
    #[serde(default)]
    pub filter: FilterSet,
    /// Opaque cursor from the previous page.
    #[serde(default)]
    pub cursor: Option<String>,
    /// Rows per page, 1..=2000.
    #[serde(default = "default_page_size")]
    pub page_size: u16,
}

impl LongPathPageRequest {
    /// Validates the filter, the cursor length and the page size.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for a malformed request.
    pub fn validate(&self) -> Result<(), UiError> {
        validate_filter(&self.filter)?;
        validate_page_size(self.page_size)?;
        if let Some(cursor) = &self.cursor {
            crate::validation::validate_cursor(cursor)?;
        }

        Ok(())
    }
}

/// Result of `long_paths_validate`.
///
/// The check never touches the object: it validates the shape of the path and
/// reports the canonical verbatim form used by every operation, so a long path
/// works without the system-wide `LongPathsEnabled` policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongPathValidation {
    /// True when the path can be used by an operation.
    pub valid: bool,
    /// Localisation key explaining a rejection.
    pub reason_key: Option<String>,
    /// Canonical verbatim form (`\\?\C:\...`) accepted by the operations layer.
    pub normalized: Option<String>,
    /// Same path without the verbatim prefix, for display.
    pub display: Option<String>,
}

// ---------------------------------------------------------------------------
// Rename (T100)
// ---------------------------------------------------------------------------

/// Case transformation applied after the replacement.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenameCase {
    /// Keep the case produced by the replacement.
    #[default]
    Preserve,
    /// Lower-case the result.
    Lower,
    /// Upper-case the result.
    Upper,
}

/// Request of `rename_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenamePreviewRequest {
    /// Full paths of the objects to rename.
    pub targets: Vec<String>,
    /// Regular expression matched against each name.
    pub regex: String,
    /// Replacement template; supports `$1`-style groups.
    #[serde(default)]
    pub replacement: String,
    /// Directory that bounds every target; when absent the targets are used as
    /// they are.
    #[serde(default)]
    pub scope: Option<String>,
    /// Optional case transform applied to the result.
    #[serde(default)]
    pub case: RenameCase,
}

impl RenamePreviewRequest {
    /// Validates the targets, the pattern, the replacement and the scope.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for an empty target list, a malformed path, a
    /// relative scope or an over-long pattern.
    pub fn validate(&self) -> Result<(), UiError> {
        if self.targets.is_empty() {
            return Err(UiError::validation("a rename needs at least one target"));
        }
        if self.targets.len() > MAX_RENAME_TARGETS {
            return Err(UiError::validation(format!(
                "a rename affects at most {MAX_RENAME_TARGETS} targets"
            )));
        }
        for target in &self.targets {
            validate_path(target)?;
        }

        if self.regex.trim().is_empty() {
            return Err(UiError::validation("the rename pattern is empty"));
        }
        if self.regex.chars().count() > MAX_RENAME_PATTERN_CHARS {
            return Err(UiError::validation(format!(
                "the rename pattern exceeds {MAX_RENAME_PATTERN_CHARS} characters"
            )));
        }
        if self.replacement.chars().count() > MAX_RENAME_REPLACEMENT_CHARS {
            return Err(UiError::validation(format!(
                "the rename replacement exceeds {MAX_RENAME_REPLACEMENT_CHARS} characters"
            )));
        }
        if self.replacement.contains('\0') {
            return Err(UiError::validation(
                "the rename replacement contains a NUL character",
            ));
        }

        if let Some(scope) = &self.scope {
            validate_path(scope)?;
        }

        Ok(())
    }
}

/// One planned rename.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenamePreviewRow {
    /// Current full path.
    pub old_path: String,
    /// Planned full path.
    pub new_path: String,
    /// Current name.
    pub old_name: String,
    /// Planned name.
    pub new_name: String,
    /// Object size in bytes.
    pub size: u64,
    /// True when this row is blocked by a conflict.
    pub conflict: bool,
}

/// A conflict that blocks at least one rename.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameConflictView {
    /// Object that caused the conflict.
    pub path: String,
    /// Planned target, when one could be built.
    pub target: Option<String>,
    /// Explanation shown to the user.
    pub reason: String,
    /// Localisation key of the reason class.
    pub reason_key: String,
}

/// Preview of a bulk rename with its one-time confirmation token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenamePreview {
    /// One-time preview identifier.
    pub preview_id: String,
    /// One-time confirmation token bound to the preview.
    pub confirmation_token: String,
    /// Expiry, Unix milliseconds.
    pub expires_at: i64,
    /// Planned renames, in input order.
    pub rows: Vec<RenamePreviewRow>,
    /// Conflicts that block execution.
    pub conflicts: Vec<RenameConflictView>,
    /// Number of objects that would be renamed.
    pub change_count: u32,
    /// Sum of the planned object sizes.
    pub total_size: u64,
    /// True when any target is a protected system path.
    pub requires_elevation: bool,
    /// Localisable warning keys.
    pub warnings: Vec<String>,
}

/// Result of `rename_apply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameApplied {
    /// Operation identifier.
    pub operation_id: String,
    /// Number of renamed objects.
    pub renamed: u32,
    /// Journal entry, when one was written.
    pub journal_entry_id: Option<u64>,
    /// True when the rename can be undone.
    pub undo_available: bool,
}

// ---------------------------------------------------------------------------
// Permissions and attributes (T101)
// ---------------------------------------------------------------------------

/// One Windows file attribute the interface can toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttributeName {
    /// `FILE_ATTRIBUTE_READONLY`.
    ReadOnly,
    /// `FILE_ATTRIBUTE_HIDDEN`.
    Hidden,
    /// `FILE_ATTRIBUTE_SYSTEM`.
    System,
    /// `FILE_ATTRIBUTE_ARCHIVE`.
    Archive,
    /// `FILE_ATTRIBUTE_TEMPORARY`.
    Temporary,
    /// `FILE_ATTRIBUTE_SPARSE_FILE`.
    Sparse,
    /// `FILE_ATTRIBUTE_COMPRESSED`.
    Compressed,
    /// `FILE_ATTRIBUTE_NOT_CONTENT_INDEXED`.
    NotContentIndexed,
}

impl AttributeName {
    /// Every attribute, in display order.
    pub const ALL: [Self; 8] = [
        Self::ReadOnly,
        Self::Hidden,
        Self::System,
        Self::Archive,
        Self::Temporary,
        Self::Sparse,
        Self::Compressed,
        Self::NotContentIndexed,
    ];

    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::Hidden => "hidden",
            Self::System => "system",
            Self::Archive => "archive",
            Self::Temporary => "temporary",
            Self::Sparse => "sparse",
            Self::Compressed => "compressed",
            Self::NotContentIndexed => "not-content-indexed",
        }
    }

    /// Windows attribute bit of this flag.
    #[must_use]
    pub const fn bit(self) -> u32 {
        match self {
            Self::ReadOnly => freespace_ops::attributes::READ_ONLY,
            Self::Hidden => freespace_ops::attributes::HIDDEN,
            Self::System => freespace_ops::attributes::SYSTEM,
            Self::Archive => freespace_ops::attributes::ARCHIVE,
            Self::Temporary => freespace_ops::attributes::TEMPORARY,
            Self::Sparse => freespace_ops::attributes::SPARSE,
            Self::Compressed => freespace_ops::attributes::COMPRESSED,
            Self::NotContentIndexed => freespace_ops::attributes::NOT_CONTENT_INDEXED,
        }
    }

    /// Returns `true` when `mask` carries this attribute.
    #[must_use]
    pub const fn is_set(self, mask: u32) -> bool {
        mask & self.bit() != 0
    }
}

/// Attributes to set and clear in one operation.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeChangeSet {
    /// Attributes to set.
    #[serde(default)]
    pub set: Vec<AttributeName>,
    /// Attributes to clear.
    #[serde(default)]
    pub clear: Vec<AttributeName>,
}

impl AttributeChangeSet {
    /// Validates that the change is not empty and has no contradictions.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` when nothing would change or an attribute appears
    /// in both lists.
    pub fn validate(&self) -> Result<(), UiError> {
        if self.set.is_empty() && self.clear.is_empty() {
            return Err(UiError::validation(
                "provide at least one attribute to set or clear",
            ));
        }
        for (index, name) in self.set.iter().enumerate() {
            if self.clear.contains(name) {
                return Err(UiError::validation(format!(
                    "the attribute `{}` cannot be set and cleared at the same time",
                    name.as_str()
                )));
            }
            if self.set[..index].contains(name) {
                return Err(UiError::validation(format!(
                    "the attribute `{}` is listed twice",
                    name.as_str()
                )));
            }
        }
        for (index, name) in self.clear.iter().enumerate() {
            if self.clear[..index].contains(name) {
                return Err(UiError::validation(format!(
                    "the attribute `{}` is listed twice",
                    name.as_str()
                )));
            }
        }

        Ok(())
    }

    /// Bits that would be set by this change.
    #[must_use]
    pub fn set_mask(&self) -> u32 {
        self.set.iter().fold(0, |mask, name| mask | name.bit())
    }

    /// Bits that would be cleared by this change.
    #[must_use]
    pub fn clear_mask(&self) -> u32 {
        self.clear.iter().fold(0, |mask, name| mask | name.bit())
    }
}

/// One attribute before and after the planned change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeChangeRow {
    /// Attribute being changed.
    pub attribute: AttributeName,
    /// State on disk now.
    pub before: bool,
    /// State after the change.
    pub after: bool,
}

/// ACL action requested by the permission screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionsAclAction {
    /// Set a new owner.
    Owner,
    /// Reset explicit permissions to inherited ones.
    Reset,
    /// Copy the permissions of another object.
    Copy,
}

impl PermissionsAclAction {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Reset => "reset",
            Self::Copy => "copy",
        }
    }
}

/// Requested permission changes for one object.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionsChanges {
    /// New owner account; empty strings are rejected.
    #[serde(default)]
    pub owner: Option<String>,
    /// Reset explicit permissions to the inherited set.
    #[serde(default)]
    pub reset: bool,
    /// Object whose permissions should be copied.
    #[serde(default)]
    pub copy_from: Option<String>,
    /// Attribute changes applied together with the ACL action.
    #[serde(default)]
    pub attributes: Option<AttributeChangeSet>,
}

impl PermissionsChanges {
    /// Validates that at least one change is present and that the ACL choices
    /// are mutually exclusive.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for an empty or contradictory request.
    pub fn validate(&self) -> Result<(), UiError> {
        if let Some(owner) = &self.owner {
            if owner.trim().is_empty() {
                return Err(UiError::validation("the owner account is empty"));
            }
            if owner.chars().count() > MAX_OWNER_CHARS {
                return Err(UiError::validation(format!(
                    "the owner account exceeds {MAX_OWNER_CHARS} characters"
                )));
            }
        }
        if self.reset && self.copy_from.is_some() {
            return Err(UiError::validation(
                "reset and copy_from cannot be requested together",
            ));
        }
        let acl_actions = usize::from(self.owner.is_some())
            .saturating_add(usize::from(self.reset))
            .saturating_add(usize::from(self.copy_from.is_some()));
        if acl_actions > 1 {
            return Err(UiError::validation(
                "choose one ACL action: owner, reset or copy_from",
            ));
        }
        if let Some(source) = &self.copy_from {
            validate_path(source)?;
        }
        if let Some(attributes) = &self.attributes {
            attributes.validate()?;
        }
        if self.owner.is_none()
            && !self.reset
            && self.copy_from.is_none()
            && self.attributes.is_none()
        {
            return Err(UiError::validation(
                "a permission change needs at least one action",
            ));
        }

        Ok(())
    }
}

/// Request of `permissions_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionsPreviewRequest {
    /// Object whose permissions change.
    pub path: String,
    /// Requested changes.
    pub changes: PermissionsChanges,
}

impl PermissionsPreviewRequest {
    /// Validates the path and the requested changes.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for a malformed path or an empty change set.
    pub fn validate(&self) -> Result<(), UiError> {
        validate_path(&self.path)?;
        self.changes.validate()
    }
}

/// Current owner, attributes and flags of one object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionsView {
    /// Full path of the object.
    pub path: String,
    /// Resolved owner account, when it could be read.
    pub owner: Option<String>,
    /// Owner security identifier in `S-1-...` form.
    pub owner_sid: Option<String>,
    /// Raw Windows attribute mask.
    pub attribute_mask: u32,
    /// Attribute flags decoded from the mask.
    pub attributes: AttributeFlags,
    /// True when the object is a directory.
    pub is_directory: bool,
    /// True when the object is a reparse point.
    pub is_reparse_point: bool,
    /// True when the object lives in a protected system location.
    pub is_protected: bool,
    /// True when changing the permissions needs elevated rights.
    pub requires_elevation: bool,
}

/// Preview of a permission or attribute change.
///
/// Nothing is written until the paired confirmation token is consumed by
/// `permissions_apply`. `attribute_changes` carries the exact before/after state
/// of every attribute so the dialog can show a diff instead of a promise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionsPreview {
    /// One-time preview identifier.
    pub preview_id: String,
    /// One-time confirmation token.
    pub confirmation_token: String,
    /// Expiry, Unix milliseconds.
    pub expires_at: i64,
    /// Object whose permissions change.
    pub path: String,
    /// State before the change.
    pub current: PermissionsView,
    /// Requested owner, when any.
    pub owner: Option<String>,
    /// Requested ACL action, when any.
    pub acl_action: Option<PermissionsAclAction>,
    /// Source object of a copied ACL.
    pub copy_from: Option<String>,
    /// Attribute diff, empty when no attributes change.
    pub attribute_changes: Vec<AttributeChangeRow>,
    /// True when applying requires elevated rights.
    pub requires_elevation: bool,
    /// Localisable warning keys.
    pub warnings: Vec<String>,
}

/// Request of `attributes_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributesPreviewRequest {
    /// Full paths of the objects.
    pub targets: Vec<String>,
    /// Attributes to set and clear.
    pub changes: AttributeChangeSet,
}

impl AttributesPreviewRequest {
    /// Validates the targets and the attribute change.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for an empty target list, a malformed path or an
    /// empty change set.
    pub fn validate(&self) -> Result<(), UiError> {
        if self.targets.is_empty() {
            return Err(UiError::validation(
                "an attribute change needs at least one target",
            ));
        }
        if self.targets.len() > MAX_PERMISSION_TARGETS {
            return Err(UiError::validation(format!(
                "an attribute change affects at most {MAX_PERMISSION_TARGETS} targets"
            )));
        }
        for target in &self.targets {
            validate_path(target)?;
        }
        self.changes.validate()
    }
}

/// Outcome of a permission or attribute change already applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOperationApplied {
    /// Operation identifier used by the journal and the progress events.
    pub operation_id: String,
    /// Journal entries written by the operation.
    pub journal_entry_ids: Vec<u64>,
    /// Objects changed successfully.
    pub changed: u32,
    /// Objects that failed.
    pub failed: u32,
    /// Per-object failures.
    pub errors: Vec<UiError>,
}

#[cfg(test)]
mod tests {
    use super::{
        AttributeChangeSet, AttributeName, PermissionsChanges, PermissionsPreviewRequest,
        RenameCase, RenamePreviewRequest,
    };
    use crate::error::UiErrorCode;

    #[test]
    fn rejects_malformed_rename_requests() {
        let mut request = RenamePreviewRequest {
            targets: vec![r"C:\data\a.txt".to_owned()],
            regex: r"^a".to_owned(),
            replacement: "b".to_owned(),
            scope: None,
            case: RenameCase::Preserve,
        };
        assert!(request.validate().is_ok());

        request.targets.clear();
        assert_eq!(
            request.validate().expect_err("empty").code,
            UiErrorCode::Validation
        );

        let invalid = RenamePreviewRequest {
            targets: vec!["relative.txt".to_owned()],
            regex: "^a".to_owned(),
            replacement: "b".to_owned(),
            scope: None,
            case: RenameCase::Preserve,
        };
        assert_eq!(
            invalid.validate().expect_err("relative").code,
            UiErrorCode::Validation
        );

        let no_pattern = RenamePreviewRequest {
            targets: vec![r"C:\data\a.txt".to_owned()],
            regex: "  ".to_owned(),
            replacement: String::new(),
            scope: None,
            case: RenameCase::Preserve,
        };
        assert!(no_pattern.validate().is_err());
    }

    #[test]
    fn validates_attribute_changes() {
        let valid = AttributeChangeSet {
            set: vec![AttributeName::Hidden],
            clear: vec![AttributeName::Archive],
        };
        assert!(valid.validate().is_ok());
        assert_eq!(valid.set_mask(), freespace_ops::attributes::HIDDEN);
        assert_eq!(valid.clear_mask(), freespace_ops::attributes::ARCHIVE);

        let contradictory = AttributeChangeSet {
            set: vec![AttributeName::Hidden],
            clear: vec![AttributeName::Hidden],
        };
        assert!(contradictory.validate().is_err());

        let duplicate = AttributeChangeSet {
            set: vec![AttributeName::Hidden, AttributeName::Hidden],
            clear: Vec::new(),
        };
        assert!(duplicate.validate().is_err());

        assert!(AttributeChangeSet::default().validate().is_err());
    }

    #[test]
    fn validates_permission_requests() {
        let mut request = PermissionsPreviewRequest {
            path: r"C:\data\file.bin".to_owned(),
            changes: PermissionsChanges {
                owner: Some("BUILTIN\\Administrators".to_owned()),
                ..PermissionsChanges::default()
            },
        };
        assert!(request.validate().is_ok());

        request.changes.owner = Some("   ".to_owned());
        assert!(request.validate().is_err());

        let contradictory = PermissionsPreviewRequest {
            path: r"C:\data\file.bin".to_owned(),
            changes: PermissionsChanges {
                reset: true,
                copy_from: Some(r"C:\data\other.bin".to_owned()),
                ..PermissionsChanges::default()
            },
        };
        assert!(contradictory.validate().is_err());

        let empty = PermissionsPreviewRequest {
            path: r"C:\data\file.bin".to_owned(),
            changes: PermissionsChanges::default(),
        };
        assert_eq!(
            empty.validate().expect_err("empty").code,
            UiErrorCode::Validation
        );

        let two_actions = PermissionsPreviewRequest {
            path: r"C:\data\file.bin".to_owned(),
            changes: PermissionsChanges {
                owner: Some("BUILTIN\\Administrators".to_owned()),
                reset: true,
                ..PermissionsChanges::default()
            },
        };
        assert!(two_actions.validate().is_err());
    }

    #[test]
    fn attribute_names_use_kebab_case_on_the_wire() {
        let value = serde_json::to_value(AttributeName::NotContentIndexed).expect("serialise");
        assert_eq!(value, "not-content-indexed");
        assert!(AttributeName::Hidden.is_set(freespace_ops::attributes::HIDDEN));
        assert!(!AttributeName::Hidden.is_set(freespace_ops::attributes::ARCHIVE));
    }
}
