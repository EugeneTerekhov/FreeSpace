//! Error envelope returned by every IPC command.
//!
//! A command never leaks a raw Rust error to the interface as the only
//! explanation: every failure carries a stable `E_*` code, a category, a
//! localisable message key and a retry hint. The envelope matches
//! `specs/002-native-ui-redesign/contracts/ipc.md`.

use std::fmt;

use serde::{Deserialize, Serialize};

use freespace_core::ErrorCode as CoreErrorCode;

/// Maximum length of the free-form `details` field, in bytes.
const MAX_DETAILS_BYTES: usize = 4 * 1024;

/// Stable interface error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UiErrorCode {
    /// Invalid input, enum value, size or path.
    #[serde(rename = "E_VALIDATION")]
    Validation,
    /// Missing permissions or an out-of-scope path.
    #[serde(rename = "E_ACCESS_DENIED")]
    AccessDenied,
    /// Object, volume or snapshot does not exist.
    #[serde(rename = "E_NOT_FOUND")]
    NotFound,
    /// The pagination cursor is stale; restart from the first page.
    #[serde(rename = "E_STALE_CURSOR")]
    StaleCursor,
    /// The node does not belong to the current generation.
    #[serde(rename = "E_STALE_NODE")]
    StaleNode,
    /// A conflicting operation or scan is already running.
    #[serde(rename = "E_BUSY")]
    Busy,
    /// The task was cancelled.
    #[serde(rename = "E_CANCELLED")]
    Cancelled,
    /// The preview expired or its confirmation token is invalid.
    #[serde(rename = "E_PREVIEW_EXPIRED")]
    PreviewExpired,
    /// Administrative rights are required.
    #[serde(rename = "E_ELEVATION_REQUIRED")]
    ElevationRequired,
    /// The user declined the elevation prompt.
    #[serde(rename = "E_ELEVATION_DENIED")]
    ElevationDenied,
    /// Only part of the targets were processed.
    #[serde(rename = "E_OPERATION_PARTIAL")]
    OperationPartial,
    /// File-system failure.
    #[serde(rename = "E_IO")]
    Io,
    /// Unsupported file system, format or operation.
    #[serde(rename = "E_UNSUPPORTED")]
    Unsupported,
    /// A network action was attempted; FreeSpace is fully offline.
    #[serde(rename = "E_OFFLINE")]
    Offline,
    /// Unexpected internal failure.
    #[serde(rename = "E_INTERNAL")]
    Internal,
}

impl UiErrorCode {
    /// Stable textual code used in JSON envelopes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "E_VALIDATION",
            Self::AccessDenied => "E_ACCESS_DENIED",
            Self::NotFound => "E_NOT_FOUND",
            Self::StaleCursor => "E_STALE_CURSOR",
            Self::StaleNode => "E_STALE_NODE",
            Self::Busy => "E_BUSY",
            Self::Cancelled => "E_CANCELLED",
            Self::PreviewExpired => "E_PREVIEW_EXPIRED",
            Self::ElevationRequired => "E_ELEVATION_REQUIRED",
            Self::ElevationDenied => "E_ELEVATION_DENIED",
            Self::OperationPartial => "E_OPERATION_PARTIAL",
            Self::Io => "E_IO",
            Self::Unsupported => "E_UNSUPPORTED",
            Self::Offline => "E_OFFLINE",
            Self::Internal => "E_INTERNAL",
        }
    }

    /// Category used by the interface to pick an icon and an action.
    #[must_use]
    pub const fn category(self) -> ErrorCategory {
        match self {
            Self::Validation | Self::PreviewExpired | Self::Unsupported => {
                ErrorCategory::Validation
            }
            Self::AccessDenied | Self::ElevationRequired | Self::ElevationDenied => {
                ErrorCategory::Access
            }
            Self::NotFound | Self::Io => ErrorCategory::Io,
            Self::StaleCursor | Self::StaleNode => ErrorCategory::Stale,
            Self::Busy | Self::OperationPartial => ErrorCategory::Operation,
            Self::Cancelled => ErrorCategory::Cancel,
            Self::Offline | Self::Internal => ErrorCategory::Internal,
        }
    }

    /// Localisation key; the catalogues carry every one of these.
    #[must_use]
    pub const fn message_key(self) -> &'static str {
        match self {
            Self::Validation => "error.validation",
            Self::AccessDenied => "error.access_denied",
            Self::NotFound => "error.not_found",
            Self::StaleCursor => "error.stale_cursor",
            Self::StaleNode => "error.stale_node",
            Self::Busy => "error.busy",
            Self::Cancelled => "error.cancelled",
            Self::PreviewExpired => "error.preview_expired",
            Self::ElevationRequired => "error.elevation_required",
            Self::ElevationDenied => "error.elevation_denied",
            Self::OperationPartial => "error.operation_partial",
            Self::Io => "error.io",
            Self::Unsupported => "error.unsupported",
            Self::Offline => "error.offline",
            Self::Internal => "error.internal",
        }
    }

    /// Whether the interface may offer a plain "retry" action.
    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::Io | Self::Busy | Self::OperationPartial | Self::Internal
        )
    }

    /// Maps a domain [`CoreErrorCode`] onto the interface surface.
    #[must_use]
    pub const fn from_core(code: CoreErrorCode) -> Self {
        match code {
            CoreErrorCode::InvalidArgument | CoreErrorCode::InvalidPath | CoreErrorCode::Config => {
                Self::Validation
            }
            CoreErrorCode::AccessDenied | CoreErrorCode::ProtectedPath => Self::AccessDenied,
            CoreErrorCode::NotFound => Self::NotFound,
            CoreErrorCode::ConfirmationRequired => Self::PreviewExpired,
            CoreErrorCode::Cancelled => Self::Cancelled,
            CoreErrorCode::Conflict
            | CoreErrorCode::Locked
            | CoreErrorCode::InsufficientSpace
            | CoreErrorCode::Operation => Self::Busy,
            CoreErrorCode::Partial => Self::OperationPartial,
            CoreErrorCode::Unsupported
            | CoreErrorCode::IncompatibleSnapshot
            | CoreErrorCode::CorruptedSnapshot => Self::Unsupported,
            CoreErrorCode::Io | CoreErrorCode::Unknown => Self::Io,
        }
    }
}

/// Coarse error category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorCategory {
    /// Invalid input or unsupported request.
    Validation,
    /// Missing permissions, elevation or scope.
    Access,
    /// File-system or missing-object failure.
    Io,
    /// Cursor or node from an older generation.
    Stale,
    /// Conflicting operation or partial success.
    Operation,
    /// Cancelled task.
    Cancel,
    /// Unexpected failure.
    Internal,
}

impl ErrorCategory {
    /// Stable textual name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Access => "access",
            Self::Io => "io",
            Self::Stale => "stale",
            Self::Operation => "operation",
            Self::Cancel => "cancel",
            Self::Internal => "internal",
        }
    }
}

/// Error returned by every command; serialises to the documented envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiError {
    /// Stable `E_*` code.
    pub code: UiErrorCode,
    /// Category used for presentation.
    pub category: ErrorCategory,
    /// Localisable message key.
    pub message_key: String,
    /// Optional details without secrets; at most 4 KiB.
    pub details: Option<String>,
    /// Whether a plain retry makes sense.
    pub retryable: bool,
    /// Related background task, when any.
    pub task_id: Option<String>,
    /// Related journal entry, when any.
    pub journal_entry_id: Option<String>,
}

impl UiError {
    /// Creates an error from its code, filling category and message key.
    #[must_use]
    pub fn new(code: UiErrorCode) -> Self {
        Self {
            code,
            category: code.category(),
            message_key: code.message_key().to_owned(),
            details: None,
            retryable: code.retryable(),
            task_id: None,
            journal_entry_id: None,
        }
    }

    /// Creates a validation error with an explanation.
    #[must_use]
    pub fn validation(details: impl Into<String>) -> Self {
        Self::new(UiErrorCode::Validation).with_details(details)
    }

    /// Attaches truncated details.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        let mut details = details.into();
        if details.len() > MAX_DETAILS_BYTES {
            details.truncate(MAX_DETAILS_BYTES);
        }
        self.details = Some(details);
        self
    }

    /// Associates the error with a background task.
    #[must_use]
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// Associates the error with a journal entry.
    #[must_use]
    pub fn with_journal(mut self, entry_id: impl Into<String>) -> Self {
        self.journal_entry_id = Some(entry_id.into());
        self
    }
}

impl fmt::Display for UiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.code.as_str())?;
        if let Some(details) = &self.details {
            write!(formatter, ": {details}")?;
        }

        Ok(())
    }
}

impl std::error::Error for UiError {}

impl From<freespace_core::Error> for UiError {
    fn from(error: freespace_core::Error) -> Self {
        Self::new(UiErrorCode::from_core(error.code())).with_details(error.to_string())
    }
}

impl From<std::io::Error> for UiError {
    fn from(error: std::io::Error) -> Self {
        let code = match error.kind() {
            std::io::ErrorKind::NotFound => UiErrorCode::NotFound,
            std::io::ErrorKind::PermissionDenied => UiErrorCode::AccessDenied,
            _ => UiErrorCode::Io,
        };

        Self::new(code).with_details(error.to_string())
    }
}

impl From<toml::de::Error> for UiError {
    fn from(error: toml::de::Error) -> Self {
        Self::new(UiErrorCode::Internal).with_details(error.to_string())
    }
}

impl From<toml::ser::Error> for UiError {
    fn from(error: toml::ser::Error) -> Self {
        Self::new(UiErrorCode::Internal).with_details(error.to_string())
    }
}

impl From<serde_json::Error> for UiError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(UiErrorCode::Internal).with_details(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorCategory, UiError, UiErrorCode};

    #[test]
    fn exposes_stable_codes_and_categories() {
        let cases = [
            (UiErrorCode::Validation, "E_VALIDATION", "validation"),
            (UiErrorCode::AccessDenied, "E_ACCESS_DENIED", "access"),
            (UiErrorCode::NotFound, "E_NOT_FOUND", "io"),
            (UiErrorCode::StaleCursor, "E_STALE_CURSOR", "stale"),
            (UiErrorCode::StaleNode, "E_STALE_NODE", "stale"),
            (UiErrorCode::Busy, "E_BUSY", "operation"),
            (UiErrorCode::Cancelled, "E_CANCELLED", "cancel"),
            (
                UiErrorCode::PreviewExpired,
                "E_PREVIEW_EXPIRED",
                "validation",
            ),
            (
                UiErrorCode::ElevationRequired,
                "E_ELEVATION_REQUIRED",
                "access",
            ),
            (
                UiErrorCode::OperationPartial,
                "E_OPERATION_PARTIAL",
                "operation",
            ),
            (UiErrorCode::Io, "E_IO", "io"),
            (UiErrorCode::Unsupported, "E_UNSUPPORTED", "validation"),
            (UiErrorCode::Offline, "E_OFFLINE", "internal"),
            (UiErrorCode::Internal, "E_INTERNAL", "internal"),
        ];

        for (code, text, category) in cases {
            assert_eq!(code.as_str(), text);
            assert_eq!(code.category().as_str(), category);
            assert!(code.message_key().starts_with("error."), "{code:?}");
        }
    }

    #[test]
    fn serialises_the_documented_envelope() {
        let error = UiError::new(UiErrorCode::AccessDenied)
            .with_details(r"C:\System Volume Information")
            .with_task("task-1");
        let value = serde_json::to_value(&error).expect("serialise");

        assert_eq!(value["code"], "E_ACCESS_DENIED");
        assert_eq!(value["category"], "access");
        assert_eq!(value["message_key"], "error.access_denied");
        assert_eq!(value["retryable"], false);
        assert_eq!(value["task_id"], "task-1");
        assert!(value["journal_entry_id"].is_null());
    }

    #[test]
    fn truncates_details_to_the_documented_limit() {
        let details = "x".repeat(5000);
        let error = UiError::validation(details);

        assert_eq!(
            error.details.as_ref().map(String::len),
            Some(super::MAX_DETAILS_BYTES)
        );
    }

    #[test]
    fn maps_core_errors_to_interface_codes() {
        use freespace_core::ErrorCode as Core;

        assert_eq!(
            UiErrorCode::from_core(Core::InvalidPath),
            UiErrorCode::Validation
        );
        assert_eq!(
            UiErrorCode::from_core(Core::AccessDenied),
            UiErrorCode::AccessDenied
        );
        assert_eq!(
            UiErrorCode::from_core(Core::ConfirmationRequired),
            UiErrorCode::PreviewExpired
        );
        assert_eq!(UiErrorCode::from_core(Core::Conflict), UiErrorCode::Busy);
        assert_eq!(
            UiErrorCode::from_core(Core::Partial),
            UiErrorCode::OperationPartial
        );
        assert_eq!(
            UiErrorCode::from_core(Core::Unsupported),
            UiErrorCode::Unsupported
        );
        assert_eq!(UiErrorCode::from_core(Core::Io), UiErrorCode::Io);

        let error: UiError = freespace_core::Error::AccessDenied("locked".to_owned()).into();
        assert_eq!(error.code, UiErrorCode::AccessDenied);
        assert_eq!(error.category, ErrorCategory::Access);
    }
}
