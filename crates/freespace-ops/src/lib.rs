#![forbid(unsafe_code)]

//! FreeSpace safe file operations.
//!
//! Destructive actions go through this crate only: preview, confirmation,
//! journal, quarantine/Recycle Bin, undo and system-path protection.
//!
//! The foundational phase provides:
//!
//! * [`operation`] — operation model, state machine and journal entries;
//! * [`confirm`] — preview, confirmation, scope and system-path guards;
//! * [`quarantine`] — reversible removal with checksums and restore;
//! * [`store`] — SQLite schema and persistence for journal, quarantine and the
//!   hash cache;
//! * [`unlock`] — lock inspection, soft/force release and deletion on reboot;
//! * [`shred`] — irreversible overwrite and deletion of files.

pub mod confirm;
pub mod operation;
pub mod quarantine;
pub mod shred;
pub mod store;
pub mod unlock;

pub use confirm::{Preview, PreviewItem, is_protected_path, preview_delete, require_confirmation};
pub use operation::{
    DeleteMode, JournalEntry, JournalResult, Operation, OperationKind, OperationParams,
    OperationStatus, OperationTarget, ResultSummary, TargetStatus,
};
pub use quarantine::{Quarantine, QuarantineItem, QuarantineStatus};
pub use shred::{ShredOptions, ShredOutcome};
pub use store::{HashKey, Store};
pub use unlock::{UnlockMode, UnlockOutcome, UnlockPlan};
