//! NTFS MFT scanner: turns raw `FILE` records into an [`FsTree`].
//!
//! Records are read in table order but linked by parent reference, so the
//! construction sorts them by their depth in the directory hierarchy first.
//! Hard links share one file reference number and therefore one physical
//! contribution (FR-002, FR-005).

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use freespace_core::{
    Result,
    model::{ScanIssue, ScanIssueKind},
    size::SizeMetrics,
    tree::{AccessState, FsTree, NodeId, NodeKind, NodeSpec},
};
use freespace_platform::{mft, paths};

use crate::{
    session::{ScanObserver, ScanPhase, ScanProgress},
    walk::Exclusions,
};

/// File reference number of the NTFS root directory.
const ROOT_FILE_REF: u64 = 5;

/// Records below this number describe NTFS metadata files.
const FIRST_USER_FILE_REF: u64 = 16;

/// Refresh progress after this many records.
const PROGRESS_INTERVAL: u64 = 8_192;

/// Result of an MFT pass.
#[derive(Debug, Default)]
pub struct ScanOutcome {
    /// Excluded entries.
    pub excluded: u64,
    /// Non-fatal problems.
    pub issues: Vec<ScanIssue>,
    /// Whether the scan was cancelled.
    pub cancelled: bool,
    /// Records skipped because the fixup sequence did not match.
    pub corrupt_records: u64,
    /// Records read from the table.
    pub records: u64,
}

/// Reads the MFT of the volume that contains `root` and fills `tree`.
///
/// # Errors
/// Returns an error when the volume cannot be opened or the tree overflows.
pub fn scan(
    root: &Path,
    tree: &mut FsTree,
    exclusions: &Exclusions,
    observer: &mut dyn ScanObserver,
) -> Result<ScanOutcome> {
    let root = paths::normalize(root)?;
    let metadata = fs::metadata(&root)?;
    if !metadata.is_dir() {
        return Err(freespace_core::Error::InvalidArgument(format!(
            "`{}` is not a directory",
            paths::display(&root)
        )));
    }

    let mut outcome = ScanOutcome::default();
    let root_spec = NodeSpec {
        name: paths::display(&root),
        kind: NodeKind::Directory,
        file_ref: ROOT_FILE_REF,
        ..NodeSpec::default()
    };
    let root_id = tree.add_node(None, root_spec)?;
    let mut nodes: HashMap<u64, NodeId> = HashMap::new();
    nodes.insert(ROOT_FILE_REF, root_id);

    observer.progress(&ScanProgress::new(ScanPhase::Enumerating, 0.1));

    let cancelled = || observer.is_cancelled();
    let mut records = Vec::new();
    let stats = {
        let cancel = &cancelled;
        mft::enumerate_mft(&root, cancel, |record| records.push(record))?
    };
    outcome.records = stats.records;
    outcome.corrupt_records = stats.corrupt;

    if observer.is_cancelled() {
        outcome.cancelled = true;

        return Ok(outcome);
    }

    observer.progress(&ScanProgress::new(ScanPhase::Building, 0.6));

    let index: HashMap<u64, usize> = records
        .iter()
        .enumerate()
        .map(|(position, record)| (record.file_ref, position))
        .collect();
    let mut depths: HashMap<u64, Option<u32>> = HashMap::new();
    let mut ordered: Vec<(u32, usize)> = Vec::with_capacity(records.len());

    for (position, record) in records.iter().enumerate() {
        let depth = record_depth(record.file_ref, &records, &index, &mut depths, 0);
        if let Some(depth) = depth {
            ordered.push((depth, position));
        }
    }
    ordered.sort_unstable_by_key(|(depth, position)| (*depth, *position));

    let total = ordered.len().max(1) as u64;
    let mut excluded_directories: HashSet<PathBuf> = HashSet::new();
    let mut processed = 0u64;
    let mut last_report = 0u64;

    for (_, position) in ordered {
        if observer.is_cancelled() {
            outcome.cancelled = true;
            break;
        }

        let record = &records[position];
        if record.file_ref == ROOT_FILE_REF || record.file_ref < FIRST_USER_FILE_REF {
            continue;
        }
        processed += 1;

        let is_directory = record.is_directory;
        let kind = if is_directory {
            if record.is_reparse_point() {
                NodeKind::Junction
            } else {
                NodeKind::Directory
            }
        } else if record.is_reparse_point() {
            NodeKind::Symlink
        } else {
            NodeKind::File
        };

        let mut first_name_of_record = true;
        for name in &record.names {
            let Some(&parent_id) = nodes.get(&name.parent_ref) else {
                if name.parent_ref < FIRST_USER_FILE_REF {
                    // Children of $Extend and other metadata files are not
                    // part of the user-visible tree.
                    continue;
                }
                outcome.issues.push(ScanIssue {
                    path: PathBuf::from(&name.name),
                    kind: ScanIssueKind::Other,
                    message: format!(
                        "record {} refers to a parent that was not found",
                        record.file_ref
                    ),
                });
                continue;
            };

            let parent_path = tree.path(parent_id);
            let full_path = parent_path.join(&name.name);
            if excluded_directories
                .iter()
                .any(|excluded| full_path.starts_with(excluded))
            {
                outcome.excluded = outcome.excluded.saturating_add(1);
                continue;
            }
            if exclusions.is_excluded(&full_path, &name.name) {
                if is_directory {
                    excluded_directories.insert(full_path);
                }
                outcome.excluded = outcome.excluded.saturating_add(1);
                continue;
            }

            let hardlink = !is_directory && record.hard_link_count > 1 && !first_name_of_record;
            let kind = if hardlink { NodeKind::Hardlink } else { kind };
            let size = if is_directory || kind == NodeKind::Junction {
                SizeMetrics::ZERO
            } else if hardlink {
                SizeMetrics::exact(record.size, record.allocated, 0, 0)
            } else {
                SizeMetrics::exact(record.size, record.allocated, record.size, 0)
            };

            let spec = NodeSpec {
                name: name.name.clone(),
                kind,
                file_ref: record.file_ref,
                size,
                timestamps: record.timestamps,
                attributes: record.attributes,
                access: AccessState::Ok,
                ..NodeSpec::default()
            };
            let id = tree.add_node(Some(parent_id), spec)?;
            if is_directory {
                nodes.entry(record.file_ref).or_insert(id);
            }
            first_name_of_record = false;
        }

        if processed.saturating_sub(last_report) >= PROGRESS_INTERVAL {
            last_report = processed;
            observer.progress(&ScanProgress {
                phase: ScanPhase::Building,
                nodes: processed,
                bytes: 0,
                fraction: 0.6 + 0.3 * (processed as f32 / total as f32),
                current: record
                    .names
                    .first()
                    .map_or_else(PathBuf::new, |name| PathBuf::from(&name.name)),
            });
        }
    }

    Ok(outcome)
}

fn record_depth(
    file_ref: u64,
    records: &[mft::MftRecord],
    index: &HashMap<u64, usize>,
    memo: &mut HashMap<u64, Option<u32>>,
    guard: u32,
) -> Option<u32> {
    if let Some(depth) = memo.get(&file_ref) {
        return *depth;
    }
    if file_ref == ROOT_FILE_REF {
        memo.insert(file_ref, Some(0));

        return Some(0);
    }
    if guard > 512 {
        memo.insert(file_ref, None);

        return None;
    }

    let position = *index.get(&file_ref)?;
    let parent_ref = records[position]
        .names
        .first()
        .map(|name| name.parent_ref)?;
    let depth = record_depth(parent_ref, records, index, memo, guard + 1)?.saturating_add(1);
    memo.insert(file_ref, Some(depth));

    Some(depth)
}

/// Applies exclusion patterns to a path without walking the volume.
#[must_use]
pub fn is_excluded(exclusions: &Exclusions, path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    exclusions.is_excluded(path, &name)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use freespace_core::tree::{FileAttributes, Timestamps};
    use freespace_platform::mft::{MftName, MftRecord};

    use super::{ROOT_FILE_REF, record_depth};

    fn record(file_ref: u64, parent_ref: u64, name: &str) -> MftRecord {
        MftRecord {
            file_ref,
            names: vec![MftName {
                parent_ref,
                name: name.to_owned(),
            }],
            attributes: FileAttributes::empty(),
            size: 0,
            allocated: 0,
            timestamps: Timestamps::default(),
            is_directory: false,
            hard_link_count: 1,
        }
    }

    #[test]
    fn computes_depths_and_stops_on_cycles() {
        let records = vec![
            record(5, 5, "."),
            record(30, 5, "Users"),
            record(31, 30, "Alice"),
            record(40, 41, "cycle-a"),
            record(41, 40, "cycle-b"),
        ];
        let index: HashMap<u64, usize> = records
            .iter()
            .enumerate()
            .map(|(position, record)| (record.file_ref, position))
            .collect();
        let mut memo = HashMap::new();

        assert_eq!(record_depth(31, &records, &index, &mut memo, 0), Some(2));
        assert_eq!(
            record_depth(ROOT_FILE_REF, &records, &index, &mut memo, 0),
            Some(0)
        );
        assert_eq!(record_depth(40, &records, &index, &mut memo, 0), None);
        assert_eq!(record_depth(99, &records, &index, &mut memo, 0), None);
    }
}
