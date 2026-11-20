//! Implemented CLI commands.

use serde_json::Value;

pub mod archives;
pub mod attrs;
pub mod capabilities;
pub mod cleanup;
pub mod config;
pub mod context_menu;
pub mod delete;
pub mod drives;
pub mod duplicates;
pub mod export;
pub mod filter;
pub mod monitor;
pub mod owners;
pub mod rename;
pub mod scan;
pub mod schedule;
pub mod shred;
pub mod snapshot;
pub mod top;
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
    /// Pre-rendered streaming output emitted verbatim (JSON Lines).
    pub raw: Option<String>,
}

impl Outcome {
    /// Creates an outcome without warnings.
    #[must_use]
    pub fn new(data: Value, human: String) -> Self {
        Self {
            data,
            warnings: Vec::new(),
            human,
            raw: None,
        }
    }

    /// Attaches warnings.
    #[must_use]
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    /// Emits `output` verbatim instead of the JSON envelope.
    #[must_use]
    pub fn with_raw(mut self, output: impl Into<String>) -> Self {
        self.raw = Some(output.into());
        self
    }
}
