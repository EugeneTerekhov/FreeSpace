#![forbid(unsafe_code)]

//! FreeSpace scanning engine.
//!
//! Implements the NTFS MFT scanner with USN-based incremental updates and the
//! `jwalk`-based fallback used for non-NTFS volumes.

// Phase 2 adds MFT parsing, USN replay and the fallback directory walk.
