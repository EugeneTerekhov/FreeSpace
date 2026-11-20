//! Duplicate detection: size → partial digest → full digest → byte comparison.
//!
//! The pipeline never trusts a hash alone (FR-026). Groups are created only for
//! files whose complete contents were compared byte by byte, at least one copy
//! always stays unselected (FR-029), and a group whose files changed while the
//! search was running is marked `stale` and is not offered for deletion.

use std::{collections::BTreeMap, fs, path::PathBuf};

use freespace_core::{
    Error, Result, Timestamp, Uuid,
    tree::{AccessState, FsTree, NodeId, NodeKind},
};
use freespace_platform::paths;
use rayon::prelude::*;

use crate::{
    hash::{self, HashAlgorithm},
    hash_cache::HashCache,
};

/// Rule that selects the copy to keep.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeepRule {
    /// Keep the oldest copy.
    #[default]
    Oldest,
    /// Keep the newest copy.
    Newest,
    /// Keep the copy with the shortest path.
    Shortest,
    /// Keep the copy that was created first (the original directory).
    OriginalDir,
}

impl KeepRule {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Oldest => "oldest",
            Self::Newest => "newest",
            Self::Shortest => "shortest",
            Self::OriginalDir => "original-dir",
        }
    }

    /// Parses a command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "oldest" => Some(Self::Oldest),
            "newest" => Some(Self::Newest),
            "shortest" => Some(Self::Shortest),
            "original-dir" | "original" => Some(Self::OriginalDir),
            _ => None,
        }
    }
}

/// Parameters of a duplicate search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateOptions {
    /// Ignore files smaller than this many bytes.
    pub min_size: u64,
    /// Hash algorithm used for the full digest.
    pub algorithm: HashAlgorithm,
}

impl Default for DuplicateOptions {
    fn default() -> Self {
        Self {
            min_size: 1,
            algorithm: HashAlgorithm::Sha256,
        }
    }
}

/// One file of a duplicate group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateMember {
    /// Tree node.
    pub node_id: NodeId,
    /// Full path.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
    /// Last modification time.
    pub modified: Timestamp,
    /// Creation time.
    pub created: Timestamp,
    /// Whether this copy is proposed for deletion.
    pub selected_for_deletion: bool,
    /// Whether a byte-by-byte comparison confirmed the match.
    pub verified: bool,
    /// Why the member is kept or selected.
    pub reason: String,
}

/// A group of byte-identical files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    /// Stable identifier of this search result.
    pub id: Uuid,
    /// Algorithm of `hash`.
    pub algorithm: HashAlgorithm,
    /// Full digest in hexadecimal form.
    pub hash: String,
    /// Size of every member in bytes.
    pub size: u64,
    /// Members, at least two.
    pub members: Vec<DuplicateMember>,
    /// Recommended copy to keep.
    pub keep_node_id: Option<NodeId>,
    /// Bytes that would be freed by deleting every selected copy.
    pub reclaimable_bytes: u64,
    /// Whether the files changed while the search was running.
    pub stale: bool,
}

impl DuplicateGroup {
    /// Number of copies proposed for deletion.
    #[must_use]
    pub fn selected_count(&self) -> usize {
        self.members
            .iter()
            .filter(|member| member.selected_for_deletion)
            .count()
    }

    /// Returns `true` when the group can be deleted safely.
    #[must_use]
    pub fn is_deletable(&self) -> bool {
        !self.stale && self.members.len() >= 2 && self.selected_count() < self.members.len()
    }
}

/// Result of a duplicate search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateSearch {
    /// Algorithm that produced the groups.
    pub algorithm: HashAlgorithm,
    /// Groups ordered by reclaimable space.
    pub groups: Vec<DuplicateGroup>,
    /// Files that shared a size with another file and entered the pipeline.
    pub candidates: u64,
    /// Files whose complete contents were hashed.
    pub hashed: u64,
    /// Files that could not be read.
    pub unreadable: u64,
    /// Total reclaimable bytes across groups.
    pub reclaimable_bytes: u64,
    /// Non-fatal notes.
    pub warnings: Vec<String>,
}

impl DuplicateSearch {
    /// Returns every deletion target, refusing unsafe selections.
    ///
    /// # Errors
    /// Returns [`Error::Operation`] when a group would lose all of its copies.
    pub fn deletion_targets(&self) -> Result<Vec<PathBuf>> {
        let mut targets = Vec::new();

        for group in &self.groups {
            if group.stale {
                continue;
            }
            let selected = group.selected_count();
            if selected == 0 {
                continue;
            }
            if selected >= group.members.len() {
                return Err(Error::Operation(format!(
                    "group {} would lose every copy; keep at least one file",
                    group.hash
                )));
            }
            targets.extend(
                group
                    .members
                    .iter()
                    .filter(|member| member.selected_for_deletion)
                    .map(|member| member.path.clone()),
            );
        }

        Ok(targets)
    }
}

/// Finds byte-identical files below the roots of `tree`.
///
/// When `cache` is provided, complete digests are reused across runs.
///
/// # Errors
/// Returns an error only for failures that invalidate the whole search; files
/// that cannot be read are counted and skipped.
pub fn find_duplicates(
    tree: &FsTree,
    options: DuplicateOptions,
    cache: Option<&HashCache>,
) -> Result<DuplicateSearch> {
    let mut warnings = Vec::new();
    let mut unreadable = 0u64;
    let mut hashed = 0u64;

    let mut by_size: BTreeMap<u64, Vec<Candidate>> = BTreeMap::new();
    for node_id in 0..tree.len() as u32 {
        let node = tree.node(node_id);
        if node.is_removed() || node.kind != NodeKind::File {
            continue;
        }
        if node.size.logical < options.min_size {
            continue;
        }
        if node.access != AccessState::Ok {
            unreadable = unreadable.saturating_add(1);
            continue;
        }
        by_size
            .entry(node.size.logical)
            .or_default()
            .push(Candidate {
                node_id,
                path: tree.path(node_id),
                size: node.size.logical,
                modified: node.timestamps.modified,
                created: node.timestamps.created,
            });
    }

    let candidates: u64 = by_size
        .values()
        .filter(|files| files.len() >= 2)
        .map(|files| u64::try_from(files.len()).unwrap_or(u64::MAX))
        .sum();

    // Stage 2: partial digest of the first and last 64 KiB.
    let mut by_partial: BTreeMap<Vec<u8>, Vec<Candidate>> = BTreeMap::new();
    for files in by_size.values().filter(|files| files.len() >= 2) {
        let digests: Vec<(Candidate, Vec<u8>)> = files
            .par_iter()
            .filter_map(|candidate| {
                hash::partial_digest(&candidate.path)
                    .ok()
                    .map(|digest| (candidate.clone(), digest))
            })
            .collect();
        if digests.len() < 2 {
            unreadable = unreadable.saturating_add(
                u64::try_from(files.len().saturating_sub(digests.len())).unwrap_or(u64::MAX),
            );
            continue;
        }
        for (candidate, digest) in digests {
            by_partial.entry(digest).or_default().push(candidate);
        }
    }

    // Stage 3: complete digests, reusing the cache when possible.
    let mut by_full: BTreeMap<Vec<u8>, Vec<Verified>> = BTreeMap::new();
    for files in by_partial.values().filter(|files| files.len() >= 2) {
        let results: Vec<Result<Verified>> = files
            .par_iter()
            .map(|candidate| {
                let (digest, stale) = complete_digest(candidate, options.algorithm, cache)?;
                Ok(Verified {
                    candidate: candidate.clone(),
                    digest,
                    stale,
                })
            })
            .collect();
        for result in results {
            match result {
                Ok(verified) => {
                    hashed = hashed.saturating_add(1);
                    by_full
                        .entry(verified.digest.clone())
                        .or_default()
                        .push(verified);
                }
                Err(_) => unreadable = unreadable.saturating_add(1),
            }
        }
    }

    // Stage 4: byte-by-byte confirmation.
    let mut groups = Vec::new();
    for (digest, files) in by_full.into_iter().filter(|(_, files)| files.len() >= 2) {
        let group = confirm_group(digest, files, options.algorithm, &mut unreadable);
        if let Some(group) = group {
            groups.push(group);
        }
    }

    groups.sort_by(|left, right| {
        right
            .reclaimable_bytes
            .cmp(&left.reclaimable_bytes)
            .then_with(|| left.hash.cmp(&right.hash))
    });

    let reclaimable_bytes = groups
        .iter()
        .map(|group| group.reclaimable_bytes)
        .fold(0u64, u64::saturating_add);
    if unreadable > 0 {
        warnings.push(format!(
            "{unreadable} files could not be read and were skipped"
        ));
    }
    if groups.iter().any(|group| group.stale) {
        warnings.push(
            "some groups changed during the search and must be re-scanned before deletion"
                .to_owned(),
        );
    }

    Ok(DuplicateSearch {
        algorithm: options.algorithm,
        groups,
        candidates,
        hashed,
        unreadable,
        reclaimable_bytes,
        warnings,
    })
}

/// Applies `rule` to a group, marking every other copy for deletion.
///
/// Stale groups are left untouched: they must be re-scanned first.
pub fn apply_keep_rule(group: &mut DuplicateGroup, rule: KeepRule) {
    if group.stale || group.members.len() < 2 {
        group.keep_node_id = None;
        group.reclaimable_bytes = 0;
        return;
    }

    let keep = keep_index(&group.members, rule);
    let keep_id = group.members[keep].node_id;
    let keep_path = group.members[keep].path.clone();
    group.keep_node_id = Some(keep_id);

    for member in &mut group.members {
        if member.node_id == keep_id {
            member.selected_for_deletion = false;
            member.reason = format!("keep ({})", rule.as_str());
        } else {
            member.selected_for_deletion = true;
            member.reason = format!(
                "duplicate of {} ({})",
                paths::display(&keep_path),
                rule.as_str()
            );
        }
    }

    let copies = group.members.len().saturating_sub(1);
    group.reclaimable_bytes = group
        .size
        .saturating_mul(u64::try_from(copies).unwrap_or(u64::MAX));
}

fn keep_index(members: &[DuplicateMember], rule: KeepRule) -> usize {
    match rule {
        KeepRule::Oldest => min_index(members, |left, right| {
            left.modified
                .cmp(&right.modified)
                .then_with(|| left.path.cmp(&right.path))
        }),
        KeepRule::Newest => min_index(members, |left, right| {
            right
                .modified
                .cmp(&left.modified)
                .then_with(|| left.path.cmp(&right.path))
        }),
        KeepRule::Shortest => min_index(members, |left, right| {
            left.path
                .as_os_str()
                .len()
                .cmp(&right.path.as_os_str().len())
                .then_with(|| left.path.cmp(&right.path))
        }),
        KeepRule::OriginalDir => min_index(members, |left, right| {
            left.created
                .cmp(&right.created)
                .then_with(|| left.path.cmp(&right.path))
        }),
    }
}

fn min_index<F>(members: &[DuplicateMember], compare: F) -> usize
where
    F: Fn(&DuplicateMember, &DuplicateMember) -> std::cmp::Ordering,
{
    let mut best = 0;
    for index in 1..members.len() {
        if compare(&members[index], &members[best]) == std::cmp::Ordering::Less {
            best = index;
        }
    }

    best
}

fn complete_digest(
    candidate: &Candidate,
    algorithm: HashAlgorithm,
    cache: Option<&HashCache>,
) -> Result<(Vec<u8>, bool)> {
    let normalized = paths::normalize(&candidate.path)?;
    let metadata = fs::metadata(normalized).map_err(Error::Io)?;
    let size = metadata.len();
    let modified = metadata
        .modified()
        .map(Timestamp::from)
        .unwrap_or(Timestamp::UNIX_EPOCH);
    let stale = size != candidate.size || modified != candidate.modified;

    if let Some(cache) = cache {
        if let Some(digest) = cache.get(&candidate.path, size, modified, algorithm)? {
            return Ok((digest, stale));
        }
    }

    let digest = hash::full_digest(&candidate.path, algorithm)?;
    if let Some(cache) = cache {
        cache.put(&candidate.path, size, modified, algorithm, &digest)?;
    }

    Ok((digest, stale))
}

fn confirm_group(
    digest: Vec<u8>,
    files: Vec<Verified>,
    algorithm: HashAlgorithm,
    unreadable: &mut u64,
) -> Option<DuplicateGroup> {
    let mut files = files;
    files.sort_by(|left, right| left.candidate.path.cmp(&right.candidate.path));
    let representative = files[0].candidate.path.clone();
    let mut stale = false;
    let mut confirmed = Vec::with_capacity(files.len());

    for file in files {
        stale |= file.stale;
        if file.candidate.path == representative {
            confirmed.push(file);
            continue;
        }
        match hash::files_equal(&representative, &file.candidate.path) {
            Ok(true) => confirmed.push(file),
            Ok(false) => {}
            Err(_) => *unreadable = unreadable.saturating_add(1),
        }
    }

    if confirmed.len() < 2 {
        return None;
    }

    let members: Vec<DuplicateMember> = confirmed
        .into_iter()
        .map(|file| DuplicateMember {
            node_id: file.candidate.node_id,
            path: file.candidate.path,
            size: file.candidate.size,
            modified: file.candidate.modified,
            created: file.candidate.created,
            selected_for_deletion: false,
            verified: true,
            reason: "keep".to_owned(),
        })
        .collect();

    let size = members.first()?.size;
    let mut group = DuplicateGroup {
        id: Uuid::new_v4(),
        algorithm,
        hash: hash::hex(&digest),
        size,
        members,
        keep_node_id: None,
        reclaimable_bytes: 0,
        stale,
    };
    apply_keep_rule(&mut group, KeepRule::Oldest);

    Some(group)
}

#[derive(Debug, Clone)]
struct Candidate {
    node_id: NodeId,
    path: PathBuf,
    size: u64,
    modified: Timestamp,
    created: Timestamp,
}

#[derive(Debug, Clone)]
struct Verified {
    candidate: Candidate,
    digest: Vec<u8>,
    stale: bool,
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use freespace_core::{
        size::SizeMetrics,
        time::Timestamp,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::{DuplicateOptions, KeepRule, apply_keep_rule, find_duplicates};
    use crate::hash::HashAlgorithm;

    fn tree_for(root: &Path, files: &[(&str, &[u8])]) -> FsTree {
        let mut tree = FsTree::new();
        let root_id = tree
            .add_node(
                None,
                NodeSpec {
                    name: root.to_string_lossy().into_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        for (name, bytes) in files {
            let path = root.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(&path, bytes).expect("write fixture");
            let metadata = fs::metadata(&path).expect("metadata");
            tree.add_node(
                Some(root_id),
                NodeSpec {
                    name: (*name).to_owned(),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(metadata.len()),
                    timestamps: freespace_core::tree::Timestamps {
                        modified: metadata
                            .modified()
                            .map(Timestamp::from)
                            .unwrap_or(Timestamp::UNIX_EPOCH),
                        created: metadata
                            .created()
                            .map(Timestamp::from)
                            .unwrap_or(Timestamp::UNIX_EPOCH),
                        ..freespace_core::tree::Timestamps::default()
                    },
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        }
        tree.recompute_totals();

        tree
    }

    #[test]
    fn finds_only_byte_identical_files() {
        let directory = tempfile::tempdir().expect("temp dir");
        let payload = vec![7u8; 512];
        let tree = tree_for(
            directory.path(),
            &[
                ("a.bin", &payload),
                ("nested/b.bin", &payload),
                ("same-size.bin", &vec![9u8; 512]),
            ],
        );

        let search = find_duplicates(&tree, DuplicateOptions::default(), None).expect("search");

        assert_eq!(search.groups.len(), 1, "only the real pair is reported");
        assert_eq!(search.groups[0].members.len(), 2);
        assert_eq!(search.groups[0].size, 512);
        assert_eq!(search.groups[0].reclaimable_bytes, 512);
        assert_eq!(search.groups[0].selected_count(), 1);
        assert!(
            search.groups[0]
                .members
                .iter()
                .all(|member| member.verified)
        );
        assert_eq!(search.reclaimable_bytes, 512);
    }

    #[test]
    fn applies_every_keep_rule_and_keeps_one_copy() {
        let directory = tempfile::tempdir().expect("temp dir");
        let payload = vec![3u8; 100];
        let tree = tree_for(
            directory.path(),
            &[("a.bin", &payload), ("deeper/nested/b.bin", &payload)],
        );
        let search = find_duplicates(&tree, DuplicateOptions::default(), None).expect("search");

        for rule in [
            KeepRule::Oldest,
            KeepRule::Newest,
            KeepRule::Shortest,
            KeepRule::OriginalDir,
        ] {
            let mut group = search.groups[0].clone();
            apply_keep_rule(&mut group, rule);
            assert_eq!(group.selected_count(), group.members.len() - 1);
            assert!(group.keep_node_id.is_some());
            assert_eq!(group.reclaimable_bytes, 100);
        }

        let targets = search.deletion_targets().expect("targets");
        assert_eq!(targets.len(), 1);
    }

    #[test]
    fn rejects_removing_every_copy() {
        let directory = tempfile::tempdir().expect("temp dir");
        let payload = vec![3u8; 100];
        let tree = tree_for(
            directory.path(),
            &[("a.bin", &payload), ("b.bin", &payload)],
        );
        let mut search = find_duplicates(&tree, DuplicateOptions::default(), None).expect("search");
        for member in &mut search.groups[0].members {
            member.selected_for_deletion = true;
        }

        let error = search.deletion_targets().expect_err("must refuse");

        assert_eq!(error.code().as_str(), "E_OPERATION");
    }

    #[test]
    fn honours_the_minimum_size() {
        let directory = tempfile::tempdir().expect("temp dir");
        let payload = [1u8; 10];
        let tree = tree_for(
            directory.path(),
            &[("a.bin", &payload), ("b.bin", &payload)],
        );

        let search = find_duplicates(
            &tree,
            DuplicateOptions {
                min_size: 11,
                ..DuplicateOptions::default()
            },
            None,
        )
        .expect("search");

        assert!(search.groups.is_empty());
    }

    #[test]
    fn parses_keep_rules() {
        assert_eq!(KeepRule::parse_value("newest"), Some(KeepRule::Newest));
        assert_eq!(
            KeepRule::parse_value("original-dir"),
            Some(KeepRule::OriginalDir)
        );
        assert_eq!(KeepRule::parse_value("magic"), None);
        assert_eq!(KeepRule::Oldest.as_str(), "oldest");
        assert_eq!(HashAlgorithm::default().as_str(), "sha256");
    }
}
