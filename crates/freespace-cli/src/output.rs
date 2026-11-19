//! JSON envelope and human-readable output helpers.
//!
//! The envelope is the stable CLI contract:
//!
//! ```json
//! {
//!   "version": "1",
//!   "command": "drives",
//!   "status": "ok",
//!   "data": {},
//!   "warnings": [],
//!   "errors": [],
//!   "duration_ms": 12
//! }
//! ```

use std::path::Path;

use freespace_core::Error;
use freespace_platform::paths;
use serde::Serialize;
use serde_json::{Value, json};

/// Version of the envelope schema.
pub const ENVELOPE_VERSION: &str = "1";

/// Overall command outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The command completed.
    Ok,
    /// The command failed.
    Error,
    /// Only part of the requested work was done.
    Partial,
    /// The user cancelled the command.
    Cancelled,
}

impl Status {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Partial => "partial",
            Self::Cancelled => "cancelled",
        }
    }
}

/// One entry in the `errors` array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorItem {
    /// Domain error code (`E_*`).
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Offending path, when the error has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ErrorItem {
    /// Builds an error item from a domain error.
    #[must_use]
    pub fn from_error(error: &Error) -> Self {
        Self {
            code: error.code().as_str().to_owned(),
            message: error.to_string(),
            path: error_path(error).map(paths::display),
        }
    }
}

fn error_path(error: &Error) -> Option<&Path> {
    match error {
        Error::InvalidPath { path, .. } | Error::ProtectedPath(path) => Some(path),
        _ => None,
    }
}

/// Builds the JSON envelope.
#[must_use]
pub fn envelope(
    command: &str,
    status: Status,
    data: Value,
    warnings: &[String],
    errors: &[ErrorItem],
    duration_ms: u64,
) -> Value {
    json!({
        "version": ENVELOPE_VERSION,
        "command": command,
        "status": status.as_str(),
        "data": data,
        "warnings": warnings,
        "errors": errors,
        "duration_ms": duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use freespace_core::Error;

    use super::{ErrorItem, Status, envelope};

    #[test]
    fn builds_a_stable_envelope() {
        let value = envelope(
            "drives",
            Status::Ok,
            serde_json::json!({"items": []}),
            &["careful".to_owned()],
            &[],
            42,
        );

        assert_eq!(value["version"], "1");
        assert_eq!(value["command"], "drives");
        assert_eq!(value["status"], "ok");
        assert_eq!(value["warnings"][0], "careful");
        assert_eq!(value["duration_ms"], 42);
        assert!(value["data"]["items"].is_array());
    }

    #[test]
    fn serialises_error_codes_and_paths() {
        let error = Error::InvalidPath {
            path: PathBuf::from(r"C:\very\long"),
            reason: "too long".to_owned(),
        };
        let item = ErrorItem::from_error(&error);
        let json = serde_json::to_value(&item).expect("serialise error");

        assert_eq!(json["code"], "E_INVALID_PATH");
        assert_eq!(json["path"], r"C:\very\long");
        assert!(json["message"].as_str().is_some());
    }

    #[test]
    fn omits_missing_paths() {
        let item = ErrorItem::from_error(&Error::Unsupported("later".to_owned()));
        let json = serde_json::to_value(&item).expect("serialise error");

        assert_eq!(json["code"], "E_UNSUPPORTED");
        assert!(json.get("path").is_none());
    }
}
