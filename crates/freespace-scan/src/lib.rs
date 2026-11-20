#![forbid(unsafe_code)]

//! FreeSpace scanning engine.
//!
//! * [`session`] — method selection, progress, cancellation and session state;
//! * `mft_scan` — NTFS MFT scanner (Windows only, whole volumes, requires
//!   elevation);
//! * [`walk`] — parallel directory walk for non-NTFS, unprivileged and
//!   sub-directory scans; the only scanner on Linux;
//! * `usn` — incremental updates from the USN change journal (Windows only);
//! * [`monitor_fallback`] — process I/O counters and, on Windows, the USN tail
//!   for the monitor;
//! * [`access`] — protected system directories and degraded access;
//! * [`empty_dirs`] — empty-directory discovery over a finished tree.

pub mod access;
pub mod empty_dirs;
#[cfg(windows)]
pub mod mft_scan;
pub mod monitor_fallback;
pub mod session;
#[cfg(windows)]
pub mod usn;
pub mod walk;

pub use empty_dirs::{EmptyDir, find_empty_dirs};

#[cfg(windows)]
pub use session::update_incremental;
pub use session::{
    MethodChoice, NoopObserver, ScanObserver, ScanPhase, ScanProgress, ScanRequest, ScanResult,
    is_volume_root, scan,
};
pub use walk::Exclusions;
