#![forbid(unsafe_code)]

//! FreeSpace analysis services.
//!
//! * [`top`] — flat top-N lists of the largest files;
//! * [`types`] — occupied space grouped by extension and category;
//! * [`hash`] — partial and complete digests of a file;
//! * [`hash_cache`] — persistent hash cache in SQLite;
//! * [`duplicates`] — byte-verified duplicate groups and keep rules;
//! * [`cleanup`] — stale-object filters over access and modification dates;
//! * [`recommend`] — categorised cleanup recommendations with risk levels;
//! * [`accounting`] — exact logical, allocated and physical size accounting;
//! * [`metadata`] — EXIF, audio, video and PE version metadata;
//! * [`owners`] — SID interning and per-owner summaries;
//! * [`archives`] — archive contents without unpacking;
//! * [`monitor`] — write-monitor policy, history and notifications.

pub mod accounting;
pub mod archives;
pub mod cleanup;
pub mod duplicates;
pub mod hash;
pub mod hash_cache;
pub mod metadata;
pub mod monitor;
pub mod owners;
pub mod recommend;
pub mod top;
pub mod types;

pub use accounting::{
    Accounting, AdsReport, SizeMode, dedupe_physical, enrich_ads, refresh_allocated, summarize,
};
pub use archives::{
    ArchiveEntry, ArchiveFormat, ArchiveInspection, ArchiveLimits, find_archives, inspect,
};
pub use cleanup::{StaleEntry, StaleOptions, find_stale, newest_use};
pub use metadata::{
    MetadataCache, MetadataFilter, MetadataIndex, MetadataKind, MetadataRecord, MetadataStatus,
    extract, parse_resolution, supported_files,
};
pub use owners::{
    OwnerError, OwnerIdentity, OwnerInfo, OwnerScan, OwnerSummary, OwnerTable, UNKNOWN_OWNER,
    annotate, annotate_with, summarize as summarize_owners,
};
pub use recommend::{
    CleanupAction, CleanupCandidate, CleanupCategory, CleanupRisk, RecommendOptions, classify_path,
    recommend,
};

pub use duplicates::{
    DuplicateGroup, DuplicateMember, DuplicateOptions, DuplicateSearch, KeepRule, apply_keep_rule,
    find_duplicates,
};
pub use hash::{FileHash, HashAlgorithm, files_equal, full_digest, hex, partial_digest};
pub use hash_cache::HashCache;
pub use monitor::{
    DEFAULT_HISTORY_CAPACITY, MonitorConfig, MonitorEvent, MonitorHistory, Notification,
    ProcessSummary, wildcard_match,
};
pub use top::{TopFile, top_files};
pub use types::{
    CategoryTotal, FileCategory, TypeGroup, category_for, category_for_extension,
    group_by_category, group_by_extension,
};
