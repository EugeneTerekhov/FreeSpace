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
//! * [`trim`] — solid-state detection and TRIM requests.

pub mod locks;
pub mod mft;
pub mod paths;
pub mod trim;
pub mod volumes;
pub mod win;
