#![forbid(unsafe_code)]

//! FreeSpace analysis services.
//!
//! * [`top`] — flat top-N lists of the largest files;
//! * [`types`] — occupied space grouped by extension and category;
//! * [`hash`] — partial and complete digests of a file;
//! * [`hash_cache`] — persistent hash cache in SQLite;
//! * [`duplicates`] — byte-verified duplicate groups and keep rules.
//!
//! Cleanup recommendations, metadata extraction, owner aggregation and archive
//! inspection arrive with their user stories.

pub mod duplicates;
pub mod hash;
pub mod hash_cache;
pub mod top;
pub mod types;

pub use duplicates::{
    DuplicateGroup, DuplicateMember, DuplicateOptions, DuplicateSearch, KeepRule, apply_keep_rule,
    find_duplicates,
};
pub use hash::{FileHash, HashAlgorithm, files_equal, full_digest, hex, partial_digest};
pub use hash_cache::HashCache;
pub use top::{TopFile, top_files};
pub use types::{
    CategoryTotal, FileCategory, TypeGroup, category_for, category_for_extension,
    group_by_category, group_by_extension,
};
