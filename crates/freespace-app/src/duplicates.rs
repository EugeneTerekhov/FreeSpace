//! Duplicate-search orchestration and projection.
//!
//! The byte-by-byte pipeline lives in `freespace-analysis`; this module parses
//! the interface options, applies the keep rule and converts groups into the
//! pageable wire shape. A search result is immutable, so its pages are stable.

use std::path::PathBuf;

use freespace_analysis::{
    duplicates::{DuplicateGroup, DuplicateOptions, KeepRule, apply_keep_rule, find_duplicates},
    hash::HashAlgorithm,
    hash_cache::HashCache,
};

pub use freespace_analysis::duplicates::DuplicateSearch;

use crate::{
    dto::{
        DuplicateGroupPage, DuplicateGroupView, DuplicateMemberView, Metric, SortSpec, TaskState,
    },
    error::UiError,
    paging,
    tree::TreeSnapshot,
    validation::validate_page_size,
};

/// Parses the algorithm name accepted by `duplicates_start`.
///
/// # Errors
/// Returns `E_VALIDATION` for an unknown algorithm.
pub fn parse_algorithm(value: &str) -> Result<HashAlgorithm, UiError> {
    HashAlgorithm::parse_value(value)
        .ok_or_else(|| UiError::validation(format!("unknown hash algorithm `{value}`")))
}

/// Parses the keep rule accepted by `duplicates_start`.
///
/// # Errors
/// Returns `E_VALIDATION` for an unknown rule.
pub fn parse_keep_rule(value: &str) -> Result<KeepRule, UiError> {
    KeepRule::parse_value(value)
        .ok_or_else(|| UiError::validation(format!("unknown keep rule `{value}`")))
}

/// Runs a duplicate search over a finished generation.
///
/// # Errors
/// Returns `E_VALIDATION` for invalid options and the mapped analysis error when
/// the search itself cannot run.
pub fn search(
    snapshot: &TreeSnapshot,
    min_size: u64,
    algorithm: &str,
    keep_rule: &str,
) -> Result<DuplicateSearch, UiError> {
    search_with_exclusions(snapshot, min_size, algorithm, keep_rule, &[])
}

/// Runs a duplicate search while excluding paths from the result.
///
/// Exclusions are applied to the finished pipeline, so a group whose copies
/// become a single file disappears instead of being offered for deletion.
///
/// # Errors
/// Returns `E_VALIDATION` for invalid options and the mapped analysis error when
/// the search itself cannot run.
pub fn search_with_exclusions(
    snapshot: &TreeSnapshot,
    min_size: u64,
    algorithm: &str,
    keep_rule: &str,
    exclusions: &[String],
) -> Result<DuplicateSearch, UiError> {
    if min_size > 1 << 40 {
        return Err(UiError::validation("min_size is unreasonably large"));
    }
    if exclusions.len() > crate::validation::MAX_EXCLUSIONS {
        return Err(UiError::validation("too many exclusions"));
    }
    let options = DuplicateOptions {
        min_size,
        algorithm: parse_algorithm(algorithm)?,
    };
    let mut result = find_duplicates(snapshot.tree(), options, None::<&HashCache>)?;
    if !exclusions.is_empty() {
        let exclusions: Vec<String> = exclusions
            .iter()
            .map(|value| value.trim_end_matches(['\\', '/']).to_lowercase())
            .filter(|value| !value.is_empty())
            .collect();
        result.groups.retain_mut(|group| {
            group.members.retain(|member| {
                let path = member.path.to_string_lossy().to_lowercase();
                !exclusions.iter().any(|excluded| {
                    path == *excluded
                        || path.starts_with(&format!("{excluded}\\"))
                        || path.starts_with(&format!("{excluded}/"))
                })
            });

            group.members.len() >= 2
        });
    }
    let rule = parse_keep_rule(keep_rule)?;
    for group in &mut result.groups {
        apply_keep_rule(group, rule);
    }
    recompute_reclaimable(&mut result);

    Ok(result)
}

/// Returns one page of groups for a finished search.
///
/// # Errors
/// Returns `E_NOT_FOUND` when the task identifier is unknown and
/// `E_STALE_CURSOR` for a cursor from another search.
pub fn groups_page(
    task_id: &str,
    search: &DuplicateSearch,
    page_size: u16,
    cursor: Option<&str>,
) -> Result<DuplicateGroupPage, UiError> {
    validate_page_size(page_size)?;
    let generation = task_id
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    let window = paging::page_window(
        search.groups.len(),
        page_size,
        cursor,
        generation,
        Metric::OnDisk,
        SortSpec::default(),
    )?;
    let groups = search.groups[window.start..window.end]
        .iter()
        .map(|group| group_view(group, search.algorithm))
        .collect();

    Ok(DuplicateGroupPage {
        cursor: cursor.map(str::to_owned),
        next_cursor: window.next_cursor,
        groups,
        total_estimate: search.groups.len() as u64,
    })
}

/// Returns one group of a finished search.
///
/// # Errors
/// Returns `E_NOT_FOUND` when the group identifier is unknown.
pub fn group(search: &DuplicateSearch, group_id: &str) -> Result<DuplicateGroupView, UiError> {
    search
        .groups
        .iter()
        .find(|candidate| candidate.id.to_string() == group_id)
        .map(|group| group_view(group, search.algorithm))
        .ok_or_else(|| {
            UiError::new(crate::error::UiErrorCode::NotFound).with_details("unknown group")
        })
}

/// Validates and resolves a client selection inside a finished search.
///
/// # Errors
/// Returns `E_VALIDATION` when a path is not part of the search or a group would
/// lose every copy.
pub fn deletion_targets(
    search: &DuplicateSearch,
    selected: &[String],
) -> Result<Vec<PathBuf>, UiError> {
    if selected.is_empty() {
        return Err(UiError::validation("no duplicate copies are selected"));
    }

    let mut selected_paths: Vec<PathBuf> = selected.iter().map(PathBuf::from).collect();
    for group in &search.groups {
        if group.stale {
            if group
                .members
                .iter()
                .any(|member| selected_paths.iter().any(|path| path == &member.path))
            {
                return Err(UiError::validation(
                    "the group changed during the search; run it again",
                ));
            }
            continue;
        }
        let chosen = group
            .members
            .iter()
            .filter(|member| selected_paths.iter().any(|path| path == &member.path))
            .count();
        if chosen > 0 && chosen >= group.members.len() {
            return Err(UiError::validation(
                "at least one copy of every group must stay on disk",
            ));
        }
    }

    selected_paths.retain(|path| {
        search
            .groups
            .iter()
            .any(|group| group.members.iter().any(|member| &member.path == path))
    });
    if selected_paths.is_empty() {
        return Err(UiError::validation(
            "the selected paths do not belong to this search",
        ));
    }

    Ok(selected_paths)
}

/// Maps a finished search onto the task summary.
#[must_use]
pub fn task_state(search: &DuplicateSearch, state: TaskState) -> TaskState {
    if state != TaskState::Completed {
        return state;
    }

    if search.unreadable > 0 || search.groups.iter().any(|group| group.stale) {
        TaskState::Partial
    } else {
        TaskState::Completed
    }
}

fn group_view(group: &DuplicateGroup, algorithm: HashAlgorithm) -> DuplicateGroupView {
    DuplicateGroupView {
        id: group.id.to_string(),
        algorithm: algorithm.as_str().to_owned(),
        hash: group.hash.clone(),
        size: group.size,
        members: group
            .members
            .iter()
            .map(|member| DuplicateMemberView {
                node_id: member.node_id,
                path: member.path.to_string_lossy().into_owned(),
                name: member
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                size: member.size,
                modified_at: member.modified.as_millis(),
                selected_for_deletion: member.selected_for_deletion,
                verified: member.verified,
                reason: member.reason.clone(),
            })
            .collect(),
        keep_node_id: group.keep_node_id,
        reclaimable_bytes: group.reclaimable_bytes,
        stale: group.stale,
        deletable: group.is_deletable(),
    }
}

fn recompute_reclaimable(search: &mut DuplicateSearch) {
    search.reclaimable_bytes = search
        .groups
        .iter()
        .map(|group| group.reclaimable_bytes)
        .fold(0u64, u64::saturating_add);
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use freespace_core::{
        model::{FileSystem, Volume, VolumeStatus},
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::{deletion_targets, groups_page, parse_algorithm, search};
    use crate::dto::Metric;
    use crate::error::UiErrorCode;
    use crate::tree::TreeSnapshot;

    fn snapshot(root: &Path, files: &[(&str, &[u8])]) -> TreeSnapshot {
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
            fs::write(&path, bytes).expect("write");
            let metadata = fs::metadata(&path).expect("metadata");
            tree.add_node(
                Some(root_id),
                NodeSpec {
                    name: (*name).to_owned(),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(bytes.len() as u64),
                    timestamps: freespace_core::tree::Timestamps {
                        modified: metadata
                            .modified()
                            .map(freespace_core::Timestamp::from)
                            .unwrap_or(freespace_core::Timestamp::UNIX_EPOCH),
                        created: metadata
                            .created()
                            .map(freespace_core::Timestamp::from)
                            .unwrap_or(freespace_core::Timestamp::UNIX_EPOCH),
                        ..freespace_core::tree::Timestamps::default()
                    },
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        }
        tree.recompute_totals();
        let volume = Volume {
            id: Volume::id_for_mount_point(root),
            mount_point: root.to_string_lossy().into_owned().into(),
            label: "Test".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000_000,
            free_bytes: 1_000,
            cluster_size: None,
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };

        TreeSnapshot::new(tree, 9, volume, None)
    }

    #[test]
    fn maps_groups_and_pages_them() {
        let directory = tempfile::tempdir().expect("temp dir");
        let payload = [7u8; 512];
        let snapshot = snapshot(
            directory.path(),
            &[("a.bin", &payload), ("nested/b.bin", &payload)],
        );

        let search = search(&snapshot, 1, "sha256", "oldest").expect("search");
        assert_eq!(search.groups.len(), 1);

        let page = groups_page("task-1", &search, 10, None).expect("page");
        assert_eq!(page.groups.len(), 1);
        assert_eq!(page.groups[0].members.len(), 2);
        assert!(page.groups[0].deletable);
        assert_eq!(page.groups[0].algorithm, "sha256");
        assert_eq!(page.total_estimate, 1);
    }

    #[test]
    fn refuses_to_delete_every_copy() {
        let directory = tempfile::tempdir().expect("temp dir");
        let payload = [1u8; 128];
        let snapshot = snapshot(
            directory.path(),
            &[("a.bin", &payload), ("b.bin", &payload)],
        );
        let search = search(&snapshot, 1, "sha256", "oldest").expect("search");
        let paths: Vec<String> = search.groups[0]
            .members
            .iter()
            .map(|member| member.path.to_string_lossy().into_owned())
            .collect();

        let error = deletion_targets(&search, &paths).expect_err("must refuse");
        assert_eq!(error.code, UiErrorCode::Validation);

        let targets = deletion_targets(&search, &paths[..1]).expect("one");
        assert_eq!(targets.len(), 1);
    }

    #[test]
    fn validates_options() {
        assert_eq!(parse_algorithm("sha256").expect("sha").as_str(), "sha256");
        assert_eq!(parse_algorithm("md5").expect("md5").as_str(), "md5");
        assert_eq!(
            parse_algorithm("crc").expect_err("unknown").code,
            UiErrorCode::Validation
        );
        let _ = Metric::OnDisk;
    }
}
