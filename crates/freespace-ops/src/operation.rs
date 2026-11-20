//! Operation model, journal entries and the operation state machine.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use freespace_core::{Error, Result, Timestamp, Uuid};

/// Kind of a destructive or mutating operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationKind {
    /// Delete to the Recycle Bin or permanently.
    Delete,
    /// Overwrite and remove.
    Shred,
    /// Move to another location.
    Move,
    /// Rename in place.
    Rename,
    /// Change NTFS permissions or ownership.
    Permissions,
    /// Change file attributes.
    Attributes,
    /// Restore an object to a previous location (legacy audit only).
    Restore,
    /// Extract an archive.
    ArchiveExtract,
    /// Schedule deletion for the next reboot.
    DeleteOnReboot,
    /// Release a file lock held by another process.
    Unlock,
}

impl OperationKind {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::Shred => "shred",
            Self::Move => "move",
            Self::Rename => "rename",
            Self::Permissions => "permissions",
            Self::Attributes => "attributes",
            Self::Restore => "restore",
            Self::ArchiveExtract => "archive-extract",
            Self::DeleteOnReboot => "delete-on-reboot",
            Self::Unlock => "unlock",
        }
    }
}

/// How deleted objects leave the volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteMode {
    /// Move to the Windows Recycle Bin.
    RecycleBin,
    /// Delete without any recovery path.
    Permanent,
}

impl DeleteMode {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RecycleBin => "recycle-bin",
            Self::Permanent => "permanent",
        }
    }

    /// Parses a command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "recycle" | "recycle-bin" => Some(Self::RecycleBin),
            "permanent" => Some(Self::Permanent),
            _ => None,
        }
    }
}

/// Overwrite pattern used by the shredder.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShredPattern {
    /// Overwrite with zero bytes.
    #[default]
    Zeros,
    /// Overwrite with cryptographically strong random data.
    Random,
}

impl ShredPattern {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zeros => "zeros",
            Self::Random => "random",
        }
    }

    /// Parses a command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "zeros" | "zero" => Some(Self::Zeros),
            "random" => Some(Self::Random),
            _ => None,
        }
    }
}

/// Per-object progress of an operation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetStatus {
    /// Not processed yet.
    #[default]
    Pending,
    /// Processed successfully.
    Succeeded,
    /// Processing failed.
    Failed,
    /// Deliberately skipped.
    Skipped,
}

/// Lifecycle state of an operation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationStatus {
    /// Previewed and waiting for the user.
    #[default]
    Planned,
    /// Confirmed by the user.
    Confirmed,
    /// Currently executing.
    Running,
    /// Every target was processed.
    Completed,
    /// Some targets were processed.
    PartiallyCompleted,
    /// The operation failed.
    Failed,
    /// The user cancelled it.
    Cancelled,
    /// Completed work was undone.
    RolledBack,
}

impl OperationStatus {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Confirmed => "confirmed",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::PartiallyCompleted => "partially-completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::RolledBack => "rolled-back",
        }
    }

    /// Returns `true` when the operation cannot change state any more.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::PartiallyCompleted
                | Self::Failed
                | Self::Cancelled
                | Self::RolledBack
        )
    }
}

/// One object affected by an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationTarget {
    /// Tree node, when the target came from an analysis.
    pub node_id: Option<u32>,
    /// Full path.
    pub path: PathBuf,
    /// Known size in bytes.
    pub size: u64,
    /// Per-object status.
    pub status: TargetStatus,
    /// Per-object error, when it failed.
    pub error: Option<String>,
}

impl OperationTarget {
    /// Creates a pending target.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, size: u64) -> Self {
        Self {
            node_id: None,
            path: path.into(),
            size,
            status: TargetStatus::Pending,
            error: None,
        }
    }

    /// Attaches a node id.
    #[must_use]
    pub fn with_node(mut self, node_id: u32) -> Self {
        self.node_id = Some(node_id);
        self
    }
}

/// Extra parameters that depend on the operation kind.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationParams {
    /// Shredder passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passes: Option<u32>,
    /// Shredder overwrite pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shred_pattern: Option<ShredPattern>,
    /// Rename rule or other free-form description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

/// Aggregate outcome of an operation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultSummary {
    /// Targets processed successfully.
    pub succeeded: u32,
    /// Targets that failed.
    pub failed: u32,
    /// Targets deliberately skipped.
    pub skipped: u32,
    /// Bytes affected.
    pub bytes: u64,
}

impl ResultSummary {
    /// Recomputes the summary from target statuses.
    #[must_use]
    pub fn from_targets(targets: &[OperationTarget]) -> Self {
        let mut summary = Self::default();

        for target in targets {
            match target.status {
                TargetStatus::Succeeded => {
                    summary.succeeded = summary.succeeded.saturating_add(1);
                    summary.bytes = summary.bytes.saturating_add(target.size);
                }
                TargetStatus::Failed => {
                    summary.failed = summary.failed.saturating_add(1);
                }
                TargetStatus::Skipped => {
                    summary.skipped = summary.skipped.saturating_add(1);
                }
                TargetStatus::Pending => {}
            }
        }

        summary
    }
}

/// A planned or executed operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    /// Unique identifier.
    pub id: Uuid,
    /// Operation kind.
    pub kind: OperationKind,
    /// Affected objects.
    pub targets: Vec<OperationTarget>,
    /// Deletion mode; required for [`OperationKind::Delete`].
    pub delete_mode: Option<DeleteMode>,
    /// Kind-specific parameters.
    pub params: OperationParams,
    /// Lifecycle state.
    pub status: OperationStatus,
    /// Whether elevation is required.
    pub requires_elevation: bool,
    /// Creation time.
    pub created_at: Timestamp,
    /// Finish time.
    pub finished_at: Option<Timestamp>,
    /// Aggregate result.
    pub result_summary: Option<ResultSummary>,
}

impl Operation {
    /// Creates a planned operation.
    #[must_use]
    pub fn new(kind: OperationKind, targets: Vec<OperationTarget>) -> Self {
        let mut operation = Self {
            id: Uuid::new_v4(),
            kind,
            targets,
            delete_mode: None,
            params: OperationParams::default(),
            status: OperationStatus::Planned,
            requires_elevation: false,
            created_at: Timestamp::now(),
            finished_at: None,
            result_summary: None,
        };

        if kind == OperationKind::Shred {
            operation.delete_mode = Some(DeleteMode::Permanent);
            operation.params.passes = Some(1);
            operation.params.shred_pattern = Some(ShredPattern::Zeros);
        }

        operation
    }

    /// Creates a planned operation with an explicit identifier.
    ///
    /// Used when the interface already assigned the operation identifier at
    /// confirmation time; the journal must keep that identifier.
    #[must_use]
    pub fn with_id(kind: OperationKind, targets: Vec<OperationTarget>, id: Uuid) -> Self {
        let mut operation = Self::new(kind, targets);
        operation.id = id;
        operation
    }

    /// Sets the deletion mode.
    ///
    /// # Errors
    /// Returns an error when a non-permanent mode is selected for the shredder.
    pub fn with_delete_mode(mut self, mode: DeleteMode) -> Result<Self> {
        if self.kind == OperationKind::Shred && mode != DeleteMode::Permanent {
            return Err(Error::InvalidArgument(
                "shredding always uses permanent deletion".to_owned(),
            ));
        }
        self.delete_mode = Some(mode);

        Ok(self)
    }

    /// Total size of all targets.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.targets.iter().map(|target| target.size).sum()
    }

    /// Validates kind-specific invariants.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] when the operation is malformed.
    pub fn validate(&self) -> Result<()> {
        if self.targets.is_empty() {
            return Err(Error::InvalidArgument(
                "an operation must affect at least one object".to_owned(),
            ));
        }

        if self.kind == OperationKind::Delete && self.delete_mode.is_none() {
            return Err(Error::InvalidArgument(
                "deletion mode is required for delete operations".to_owned(),
            ));
        }

        if self.kind == OperationKind::Shred {
            if self.delete_mode != Some(DeleteMode::Permanent) {
                return Err(Error::InvalidArgument(
                    "shredding always uses permanent deletion".to_owned(),
                ));
            }
            if self.params.passes.is_none_or(|passes| passes == 0) {
                return Err(Error::InvalidArgument(
                    "shredding requires at least one pass".to_owned(),
                ));
            }
        }

        Ok(())
    }

    /// Confirms a planned operation.
    ///
    /// # Errors
    /// Returns an error when the transition is not allowed or validation fails.
    pub fn confirm(&mut self) -> Result<()> {
        self.validate()?;
        self.transition(OperationStatus::Confirmed)
    }

    /// Starts execution of a confirmed operation.
    ///
    /// # Errors
    /// Returns an error when the operation was not confirmed.
    pub fn begin(&mut self) -> Result<()> {
        self.validate()?;
        self.transition(OperationStatus::Running)
    }

    /// Cancels the operation.
    ///
    /// # Errors
    /// Returns an error when the current state cannot be cancelled.
    pub fn cancel(&mut self) -> Result<()> {
        self.transition(OperationStatus::Cancelled)
    }

    /// Moves the operation to a terminal state and records the result summary.
    ///
    /// # Errors
    /// Returns an error when the transition is not allowed.
    pub fn finish(&mut self, status: OperationStatus) -> Result<()> {
        if !status.is_terminal() {
            return Err(Error::InvalidArgument(format!(
                "`{}` is not a terminal status",
                status.as_str()
            )));
        }
        self.transition(status)?;
        self.result_summary = Some(ResultSummary::from_targets(&self.targets));

        Ok(())
    }

    /// Applies a state transition using the documented state machine.
    ///
    /// `planned → confirmed → running → completed | partially-completed | failed`;
    /// `planned | confirmed | running → cancelled`; completed or
    /// partially-completed work may be `rolled-back`.
    ///
    /// # Errors
    /// Returns an error when the transition is not allowed.
    pub fn transition(&mut self, next: OperationStatus) -> Result<()> {
        if !allowed_transition(self.status, next) {
            return Err(Error::Operation(format!(
                "operation `{}` cannot move from `{}` to `{}`",
                self.id,
                self.status.as_str(),
                next.as_str()
            )));
        }

        self.status = next;
        if next.is_terminal() {
            self.finished_at = Some(Timestamp::now());
        }

        Ok(())
    }
}

fn allowed_transition(from: OperationStatus, to: OperationStatus) -> bool {
    use OperationStatus::{
        Cancelled, Completed, Confirmed, Failed, PartiallyCompleted, Planned, RolledBack, Running,
    };

    matches!(
        (from, to),
        (Planned, Confirmed)
            | (Planned, Cancelled)
            | (Confirmed, Running)
            | (Confirmed, Cancelled)
            | (Running, Completed)
            | (Running, PartiallyCompleted)
            | (Running, Failed)
            | (Running, Cancelled)
            | (Completed, RolledBack)
            | (PartiallyCompleted, RolledBack)
    )
}

/// Outcome recorded in the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JournalResult {
    /// Every target was processed.
    Success,
    /// Some targets were processed.
    Partial,
    /// The operation failed.
    Error,
}

impl JournalResult {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Partial => "partial",
            Self::Error => "error",
        }
    }

    /// Parses a stored value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value {
            "success" => Some(Self::Success),
            "partial" => Some(Self::Partial),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// One journal record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Autoincrement identifier; zero before insertion.
    pub id: u64,
    /// Operation this entry belongs to.
    pub operation_id: Uuid,
    /// When the operation finished.
    pub timestamp: Timestamp,
    /// Account that performed the operation.
    pub user: String,
    /// Short human-readable summary.
    pub summary: String,
    /// Number of affected objects.
    pub affected_count: u32,
    /// Affected bytes.
    pub affected_bytes: u64,
    /// Outcome.
    pub result: JournalResult,
    /// Whether the operation can be undone.
    pub undo_available: bool,
    /// Structured details (targets, errors, parameters).
    pub details: serde_json::Value,
}

impl JournalEntry {
    /// Creates an entry for a finished operation.
    #[must_use]
    pub fn from_operation(operation: &Operation, user: impl Into<String>) -> Self {
        let summary = operation.result_summary.unwrap_or_default();
        let result = if summary.failed == 0 {
            JournalResult::Success
        } else if summary.succeeded == 0 {
            JournalResult::Error
        } else {
            JournalResult::Partial
        };

        Self {
            id: 0,
            operation_id: operation.id,
            timestamp: operation.finished_at.unwrap_or_else(Timestamp::now),
            user: user.into(),
            summary: format!(
                "{}: {} succeeded, {} failed, {} skipped",
                operation.kind.as_str(),
                summary.succeeded,
                summary.failed,
                summary.skipped
            ),
            affected_count: summary.succeeded.saturating_add(summary.failed),
            affected_bytes: summary.bytes,
            result,
            undo_available: false,
            details: serde_json::to_value(operation).unwrap_or(serde_json::Value::Null),
        }
    }
}

#[cfg(test)]
mod tests {
    use freespace_core::ErrorCode;

    use super::{
        DeleteMode, JournalEntry, JournalResult, Operation, OperationKind, OperationStatus,
        OperationTarget, TargetStatus,
    };

    fn delete_operation() -> Operation {
        Operation::new(
            OperationKind::Delete,
            vec![
                OperationTarget::new(r"C:\temp\a.txt", 10),
                OperationTarget::new(r"C:\temp\b.txt", 20),
            ],
        )
    }

    #[test]
    fn requires_a_delete_mode() {
        let operation = delete_operation();

        let error = operation.validate().expect_err("mode is required");

        assert_eq!(error.code(), ErrorCode::InvalidArgument);

        let operation = operation
            .with_delete_mode(DeleteMode::RecycleBin)
            .expect("set mode");
        assert!(operation.validate().is_ok());
    }

    #[test]
    fn shredding_is_always_permanent() {
        let operation = Operation::new(
            OperationKind::Shred,
            vec![OperationTarget::new(r"C:\temp\secret.txt", 5)],
        );

        assert_eq!(operation.delete_mode, Some(DeleteMode::Permanent));
        assert!(operation.validate().is_ok());
        assert!(
            operation
                .clone()
                .with_delete_mode(DeleteMode::RecycleBin)
                .is_err()
        );
    }

    #[test]
    fn requires_targets() {
        let operation = Operation::new(OperationKind::Rename, Vec::new());

        assert!(operation.validate().is_err());
    }

    #[test]
    fn runs_the_documented_state_machine() {
        let mut operation = delete_operation()
            .with_delete_mode(DeleteMode::RecycleBin)
            .expect("mode");

        assert_eq!(operation.status, OperationStatus::Planned);
        operation.confirm().expect("confirm");
        assert!(!operation.status.is_terminal());
        operation.begin().expect("begin");
        assert_eq!(operation.status, OperationStatus::Running);

        operation.targets[0].status = TargetStatus::Succeeded;
        operation.targets[1].status = TargetStatus::Failed;
        operation
            .finish(OperationStatus::PartiallyCompleted)
            .expect("finish");

        assert!(operation.status.is_terminal());
        assert!(operation.finished_at.is_some());
        let summary = operation.result_summary.expect("summary");
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.bytes, 10);
    }

    #[test]
    fn refuses_invalid_transitions() {
        let mut operation = delete_operation()
            .with_delete_mode(DeleteMode::Permanent)
            .expect("mode");

        assert!(operation.begin().is_err(), "cannot run unconfirmed work");
        operation.cancel().expect("cancel");
        assert!(operation.confirm().is_err(), "cancelled is terminal");
        assert!(operation.finish(OperationStatus::Completed).is_err());
    }

    #[test]
    fn records_journal_entries() {
        let mut operation = delete_operation()
            .with_delete_mode(DeleteMode::Permanent)
            .expect("mode");
        operation.confirm().expect("confirm");
        operation.begin().expect("begin");
        operation.targets[0].status = TargetStatus::Succeeded;
        operation.targets[1].status = TargetStatus::Succeeded;
        operation
            .finish(OperationStatus::Completed)
            .expect("finish");

        let entry = JournalEntry::from_operation(&operation, "tester");

        assert_eq!(entry.operation_id, operation.id);
        assert_eq!(entry.result, JournalResult::Success);
        assert_eq!(entry.affected_count, 2);
        assert_eq!(entry.affected_bytes, 30);
        assert!(!entry.summary.is_empty());
        assert_eq!(
            JournalResult::parse_value("partial"),
            Some(JournalResult::Partial)
        );
        assert_eq!(JournalResult::parse_value("nope"), None);
        assert!(entry.details.is_object());
    }

    #[test]
    fn reports_failed_journal_results() {
        let mut operation = delete_operation()
            .with_delete_mode(DeleteMode::Permanent)
            .expect("mode");
        operation.confirm().expect("confirm");
        operation.begin().expect("begin");
        operation.targets[0].status = TargetStatus::Failed;
        operation.targets[1].status = TargetStatus::Failed;
        operation.finish(OperationStatus::Failed).expect("finish");

        let entry = JournalEntry::from_operation(&operation, "tester");

        assert_eq!(entry.result, JournalResult::Error);
        assert_eq!(entry.affected_count, 2);
        assert_eq!(entry.affected_bytes, 0);
    }
}
