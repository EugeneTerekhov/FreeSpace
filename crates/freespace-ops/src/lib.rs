#![forbid(unsafe_code)]

//! FreeSpace safe file operations.
//!
//! Destructive actions go through this crate only: preview, confirmation,
//! journal, Recycle Bin, undo and system-path protection.
//!
//! The foundational phase provides:
//!
//! * [`operation`] — operation model, state machine and journal entries;
//! * [`confirm`] — preview, confirmation, scope and system-path guards;
//! * [`store`] — SQLite schema and persistence for the journal and the hash
//!   cache;
//! * [`unlock`] — lock inspection, soft/force release and deletion on reboot;
//! * [`shred`] — irreversible overwrite and deletion of files;
//! * [`delete`] — Recycle Bin and permanent deletion;
//! * [`undo`] — reversing renames recorded in the journal;
//! * [`guarded`] — scope and system-path guards;
//! * [`long_path_ops`] — rename, move, copy and delete beyond `MAX_PATH`;
//! * [`rename`] — regular-expression rename rules;
//! * [`rename_plan`] — bulk rename preview, conflicts and rollback;
//! * [`attributes`] — batch attribute changes;
//! * [`acl`] — batch ownership and permission changes;
//! * [`automation`] — reusable automation profiles for the Scheduler.

mod path_compat;

pub mod acl;
pub mod attributes;
pub mod automation;
pub mod confirm;
pub mod delete;
pub mod guarded;
pub mod long_path_ops;
pub mod operation;
pub mod rename;
pub mod rename_plan;
#[cfg(windows)]
pub mod shred;
pub mod store;
pub mod undo;
pub mod unlock;

pub use acl::{AclAction, AclItemResult, AclReport};
pub use attributes::{
    ARCHIVE, AttributeChange, AttributeItemResult, AttributeReport, COMPRESSED, HIDDEN,
    NOT_CONTENT_INDEXED, READ_ONLY, SPARSE, SYSTEM, TEMPORARY, apply_attributes,
};
pub use automation::{AutomationProfile, ScheduleSpec, Weekday};
pub use confirm::{
    Preview, PreviewItem, PreviewItemKind, SpaceShortage, is_protected_path, preview_delete,
    require_confirmation,
};
pub use delete::{
    DeleteOptions, DeleteOutcome, DeletedItem, delete_paths, delete_paths_cancellable,
};
pub use guarded::{GuardReport, enforce as enforce_guard, inspect as inspect_guard};
pub use long_path_ops::{LongPathOperation, LongPathOutcome};
pub use operation::{
    DeleteMode, JournalEntry, JournalResult, Operation, OperationKind, OperationParams,
    OperationStatus, OperationTarget, ResultSummary, TargetStatus,
};
pub use rename::{CaseTransform, RenameRule};
pub use rename_plan::{
    RenameConflict, RenameItem, RenameOutcome, RenamePlan, apply_plan, build_plan,
};
#[cfg(windows)]
pub use shred::{ShredOptions, ShredOutcome};
pub use store::{HashKey, Store};
pub use undo::{UndoOutcome, undo_operation, undoable_entries};
pub use unlock::{UnlockMode, UnlockOutcome, UnlockPlan};
