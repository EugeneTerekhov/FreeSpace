//! Base error type shared by all FreeSpace crates.

use std::{io, path::PathBuf};

use thiserror::Error;

/// Convenience alias used throughout the workspace.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Base error type for domain and infrastructure failures.
///
/// Domain error codes (`E_*`) and process exit codes are layered on top of this
/// type by the CLI crate; this enum only carries the failure itself.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// An operating-system call failed.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

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

    /// The requested capability is unavailable on this system or volume.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// Any other failure without a dedicated variant.
    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::Error;

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
    }
}
