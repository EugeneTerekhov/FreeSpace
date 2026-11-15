#![forbid(unsafe_code)]

//! FreeSpace safe file operations.
//!
//! Destructive actions go through this crate only: preview, confirmation,
//! journal, quarantine/Recycle Bin, undo and system-path protection.

// Phase 2 adds the operation state machine, journal and quarantine.
