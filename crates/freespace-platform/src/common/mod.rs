//! Types shared by every platform backend.
//!
//! The trait objects in [`crate::Platform`] must not expose operating-system
//! specific types, therefore their records live here and are re-exported by the
//! backend modules that produce them.

pub mod metadata;
pub mod monitor;
pub mod shred;
pub mod storage;
pub mod unlock;
pub mod volumes;
