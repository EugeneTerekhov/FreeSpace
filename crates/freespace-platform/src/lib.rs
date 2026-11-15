//! Windows platform layer.
//!
//! Every call into the Windows API lives in this crate; the rest of the
//! workspace stays free of `unsafe` code and of the `windows` crate.
//!
//! Phase 2 provides the foundational pieces:
//!
//! * [`paths`] — verbatim path conversion, validation and scope checks;
//! * [`win`] — RAII handles and error conversion;
//! * [`volumes`] — logical-drive enumeration and volume queries.
//!
//! MFT/USN access and lock inspection arrive with the user stories that need
//! them.

pub mod paths;
pub mod volumes;
pub mod win;
