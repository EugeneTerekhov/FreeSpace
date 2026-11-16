//! Base error type and domain error codes shared by all FreeSpace crates.

use std::{io, path::PathBuf};

use thiserror::Error;

/// Convenience alias used throughout the workspace.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Base error type for domain and infrastructure failures.
///
/// Every failure carries a stable [`ErrorCode`] (`E_*`) and an exit code from
/// the documented CLI contract (`contracts/cli.md`).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// An operating-system call failed.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// A command-line option or configuration value is invalid.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// A path is syntactically invalid or exceeds the supported limit.
    #[error("invalid path `{path}`: {reason}")]
    InvalidPath {
        /// Offending path.
        path: PathBuf,
        /// Human-readable explanation.
        reason: String,
    },

    /// Settings or configuration could not be read or validated.
    #[error("configuration error: {0}")]
    Config(String),

    /// A file-system operation failed or was refused by a safety check.
    #[error("operation failed: {0}")]
    Operation(String),

    /// The requested object does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// The current user is not allowed to perform the operation.
    #[error("access denied: {0}")]
    AccessDenied(String),

    /// A destructive operation was not confirmed by the user.
    #[error("confirmation required: {0}")]
    ConfirmationRequired(String),

    /// The operation was cancelled before it changed anything.
    #[error("operation cancelled: {0}")]
    Cancelled(String),

    /// Names, permissions or snapshots conflict with existing state.
    #[error("conflict: {0}")]
    Conflict(String),

    /// Only part of the requested objects could be processed.
    #[error("partial success: {0}")]
    Partial(String),

    /// The chosen deletion mode cannot run because of free-space limits.
    #[error("insufficient free space: {required} bytes required, {available} bytes available")]
    InsufficientSpace {
        /// Bytes required by the operation.
        required: u64,
        /// Bytes currently available on the target volume.
        available: u64,
    },

    /// The target is a protected system path and needs elevated, explicit action.
    #[error("protected path: `{0}`")]
    ProtectedPath(PathBuf),

    /// The object is locked by another process.
    #[error("object is locked: {0}")]
    Locked(String),

    /// The requested capability is unavailable on this system or volume.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// Snapshots come from incompatible volumes or format versions.
    #[error("incompatible snapshot: {0}")]
    IncompatibleSnapshot(String),

    /// A snapshot container or its checksum is corrupted.
    #[error("corrupted snapshot: {0}")]
    CorruptedSnapshot(String),

    /// Any other failure without a dedicated variant.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Stable domain error code.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Io(error) => match error.kind() {
                io::ErrorKind::NotFound => ErrorCode::NotFound,
                io::ErrorKind::PermissionDenied => ErrorCode::AccessDenied,
                _ => ErrorCode::Io,
            },
            Self::InvalidArgument(_) => ErrorCode::InvalidArgument,
            Self::InvalidPath { .. } => ErrorCode::InvalidPath,
            Self::Config(_) => ErrorCode::Config,
            Self::Operation(_) => ErrorCode::Operation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::AccessDenied(_) => ErrorCode::AccessDenied,
            Self::ConfirmationRequired(_) => ErrorCode::ConfirmationRequired,
            Self::Cancelled(_) => ErrorCode::Cancelled,
            Self::Conflict(_) => ErrorCode::Conflict,
            Self::Partial(_) => ErrorCode::Partial,
            Self::InsufficientSpace { .. } => ErrorCode::InsufficientSpace,
            Self::ProtectedPath(_) => ErrorCode::ProtectedPath,
            Self::Locked(_) => ErrorCode::Locked,
            Self::Unsupported(_) => ErrorCode::Unsupported,
            Self::IncompatibleSnapshot(_) => ErrorCode::IncompatibleSnapshot,
            Self::CorruptedSnapshot(_) => ErrorCode::CorruptedSnapshot,
            Self::Other(_) => ErrorCode::Unknown,
        }
    }

    /// Documented CLI exit code for this failure.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        self.code().exit_code()
    }
}

/// Stable, documented domain error codes (`E_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Generic I/O failure.
    Io,
    /// Invalid argument or option value.
    InvalidArgument,
    /// Invalid, too long or unsupported path.
    InvalidPath,
    /// Invalid configuration.
    Config,
    /// A file-system operation failed.
    Operation,
    /// The object does not exist.
    NotFound,
    /// Access was denied.
    AccessDenied,
    /// A destructive operation was not confirmed.
    ConfirmationRequired,
    /// The operation was cancelled.
    Cancelled,
    /// State conflict prevented the operation.
    Conflict,
    /// Only part of the objects were processed.
    Partial,
    /// Not enough free space.
    InsufficientSpace,
    /// Protected system path.
    ProtectedPath,
    /// The object is locked by another process.
    Locked,
    /// Unsupported format or capability.
    Unsupported,
    /// Snapshots refer to different volumes or format versions.
    IncompatibleSnapshot,
    /// A snapshot is damaged or fails its checksum.
    CorruptedSnapshot,
    /// Unclassified failure.
    Unknown,
}

impl ErrorCode {
    /// Stable textual code used in JSON error envelopes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Io => "E_IO",
            Self::InvalidArgument => "E_INVALID_ARGUMENT",
            Self::InvalidPath => "E_INVALID_PATH",
            Self::Config => "E_CONFIG",
            Self::Operation => "E_OPERATION",
            Self::NotFound => "E_NOT_FOUND",
            Self::AccessDenied => "E_ACCESS_DENIED",
            Self::ConfirmationRequired => "E_CONFIRMATION_REQUIRED",
            Self::Cancelled => "E_CANCELLED",
            Self::Conflict => "E_CONFLICT",
            Self::Partial => "E_PARTIAL",
            Self::InsufficientSpace => "E_INSUFFICIENT_SPACE",
            Self::ProtectedPath => "E_PROTECTED_PATH",
            Self::Locked => "E_LOCKED",
            Self::Unsupported => "E_UNSUPPORTED",
            Self::IncompatibleSnapshot => "E_INCOMPATIBLE_SNAPSHOT",
            Self::CorruptedSnapshot => "E_CORRUPTED_SNAPSHOT",
            Self::Unknown => "E_UNKNOWN",
        }
    }

    /// Documented process exit code (`contracts/cli.md`).
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::InvalidArgument | Self::InvalidPath => 2,
            Self::AccessDenied | Self::ProtectedPath => 3,
            Self::NotFound => 4,
            Self::ConfirmationRequired | Self::Cancelled => 5,
            Self::Conflict => 6,
            Self::Partial => 7,
            Self::Unsupported => 8,
            Self::InsufficientSpace => 9,
            Self::Locked => 10,
            Self::IncompatibleSnapshot => 6,
            Self::CorruptedSnapshot => 1,
            Self::Io | Self::Config | Self::Operation | Self::Unknown => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Error, ErrorCode};

    #[test]
    fn formats_invalid_path_error() {
        let error = Error::InvalidPath {
            path: "C:\\very long".into(),
            reason: "exceeds the supported limit".to_owned(),
        };

        assert_eq!(
            error.to_string(),
            "invalid path `C:\\very long`: exceeds the supported limit"
        );
    }

    #[test]
    fn converts_io_errors() {
        let error: Error = std::io::Error::other("disk gone").into();

        assert!(error.to_string().contains("disk gone"));
        assert_eq!(error.code(), ErrorCode::Io);
    }

    #[test]
    fn classifies_io_error_kinds() {
        let missing: Error = std::io::Error::from(std::io::ErrorKind::NotFound).into();
        let denied: Error = std::io::Error::from(std::io::ErrorKind::PermissionDenied).into();

        assert_eq!(missing.code(), ErrorCode::NotFound);
        assert_eq!(denied.code(), ErrorCode::AccessDenied);
    }

    #[test]
    fn maps_every_code_to_documented_exit_code() {
        let cases = [
            (ErrorCode::Io, 1),
            (ErrorCode::InvalidArgument, 2),
            (ErrorCode::InvalidPath, 2),
            (ErrorCode::AccessDenied, 3),
            (ErrorCode::ProtectedPath, 3),
            (ErrorCode::NotFound, 4),
            (ErrorCode::ConfirmationRequired, 5),
            (ErrorCode::Cancelled, 5),
            (ErrorCode::Conflict, 6),
            (ErrorCode::Partial, 7),
            (ErrorCode::Unsupported, 8),
            (ErrorCode::InsufficientSpace, 9),
            (ErrorCode::Locked, 10),
            (ErrorCode::Config, 1),
            (ErrorCode::Operation, 1),
            (ErrorCode::IncompatibleSnapshot, 6),
            (ErrorCode::CorruptedSnapshot, 1),
            (ErrorCode::Unknown, 1),
        ];

        for (code, exit) in cases {
            assert_eq!(code.exit_code(), exit, "exit code for {code:?}");
        }
    }

    #[test]
    fn exposes_stable_error_code_strings() {
        assert_eq!(ErrorCode::AccessDenied.as_str(), "E_ACCESS_DENIED");
        assert_eq!(
            ErrorCode::InsufficientSpace.as_str(),
            "E_INSUFFICIENT_SPACE"
        );
        assert_eq!(
            Error::ProtectedPath(PathBuf::from(r"C:\Windows"))
                .code()
                .as_str(),
            "E_PROTECTED_PATH"
        );
        assert_eq!(
            Error::ConfirmationRequired("delete".to_owned()).exit_code(),
            5
        );
    }
}
