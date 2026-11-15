#![forbid(unsafe_code)]

//! FreeSpace scanning engine.
//!
//! * [`session`] — method selection, progress, cancellation and session state;
//! * [`mft_scan`] — NTFS MFT scanner (whole volumes, requires elevation);
//! * [`walk`] — parallel directory walk for non-NTFS, unprivileged and
//!   sub-directory scans;
//! * [`usn`] — incremental updates from the USN change journal.

pub mod mft_scan;
pub mod session;
pub mod usn;
pub mod walk;

pub use session::{
    MethodChoice, NoopObserver, ScanObserver, ScanPhase, ScanProgress, ScanRequest, ScanResult,
    is_volume_root, scan,
};
pub use walk::Exclusions;
