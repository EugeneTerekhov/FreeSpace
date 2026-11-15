//! Implemented CLI commands.

use serde_json::Value;

pub mod config;
pub mod drives;
pub mod scan;
pub mod unlock;

/// Result of a successful command.
#[derive(Debug)]
pub struct Outcome {
    /// Machine-readable payload placed into the envelope `data` field.
    pub data: Value,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
    /// Human-readable rendering.
    pub human: String,
}

impl Outcome {
    /// Creates an outcome without warnings.
    #[must_use]
    pub fn new(data: Value, human: String) -> Self {
        Self {
            data,
            warnings: Vec::new(),
            human,
        }
    }

    /// Attaches warnings.
    #[must_use]
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }
}
