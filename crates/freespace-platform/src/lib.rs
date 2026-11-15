//! Windows platform layer.
//!
//! Every call into the Windows API lives in this crate; the rest of the
//! workspace stays free of `unsafe` code and of the `windows` crate.

// Phase 2 adds path handling, volume enumeration, MFT/USN access and the
// remaining WinAPI wrappers.
