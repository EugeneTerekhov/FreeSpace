//! Windows platform layer.
//!
//! Every call into the Windows API lives in this crate; the rest of the
//! workspace stays free of `unsafe` code and of the `windows` crate.
//!
//! * [`paths`] — verbatim path conversion, validation and scope checks;
//! * [`win`] — RAII handles and error conversion;
//! * [`volumes`] — logical-drive enumeration and volume queries;
//! * [`mft`] — NTFS MFT and USN journal access;
//! * [`locks`] — Restart Manager lock inspection and release;
//! * [`trim`] — solid-state detection and TRIM requests;
//! * [`recycle`] — Recycle Bin deletion through `IFileOperation`;
//! * [`reparse`] — reparse-point classification;
//! * [`allocated`] — exact allocated size and allocated ranges;
//! * [`ads`] — alternate data stream enumeration;
//! * [`security`] — file attributes, ownership and ACL operations;
//! * [`file_lock`] — cross-process advisory locks for CLI runs;
//! * [`command_line`] — Windows command-line quoting;
//! * [`context_menu`] — Explorer context-menu registration (optional feature);
//! * [`etw`] — real-time write attribution through ETW;
//! * [`process_io`] — per-process I/O counters for the monitor fallback;
//! * [`elevation`] — elevation state and on-demand UAC relaunch.

pub mod ads;
pub mod allocated;
pub mod command_line;
#[cfg(feature = "context-menu")]
pub mod context_menu;
pub mod elevation;
pub mod etw;
pub mod file_lock;
pub mod locks;
pub mod mft;
pub mod paths;
pub mod process_io;
pub mod recycle;
pub mod reparse;
pub mod security;
pub mod trim;
pub mod volumes;
pub mod win;
