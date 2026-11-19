//! Cleanup recommendations and empty-directory projections.
//!
//! Recommendations come from `freespace-analysis`; this module applies the
//! shared filter and maps domain values onto the IPC contract. Nothing here
//! deletes data: every action still goes through the operation preview.

use freespace_analysis::recommend::{RecommendOptions, recommend};
use freespace_core::tree::{FsTree, NodeId};

use crate::{
    dto::{CleanupCandidateView, FilterSet, Metric, SortSpec, TreePage},
    error::UiError,
    paging,
    tree::TreeSnapshot,
    validation::{validate_filter, validate_page_size},
};

/// Builds cleanup recommendations for the current generation.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed filter or a threshold above ten years.
pub fn cleanup_candidates(
    snapshot: &TreeSnapshot,
    older_than_days: u32,
    filter: &FilterSet,
) -> Result<Vec<CleanupCandidateView>, UiError> {
    validate_filter(filter)?;
    if older_than_days > 3_650 {
        return Err(UiError::validation("older_than_days must not exceed 3650"));
    }

    let candidates = recommend(
        snapshot.tree(),
        RecommendOptions {
            now: freespace_core::Timestamp::now(),
            older_than_days,
        },
    );

    let mut views: Vec<CleanupCandidateView> = candidates
        .into_iter()
        .filter(|candidate| matches_cleanup_filter(candidate, filter))
        .map(|candidate| CleanupCandidateView {
            node_id: candidate.node_id,
            name: candidate
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: candidate.path.to_string_lossy().into_owned(),
            reason_key: format!("cleanup.reason.{}", candidate.category.as_str()),
            category: candidate.category.as_str().to_owned(),
            reclaimable_bytes: candidate.reclaimable_bytes,
            risk: candidate.risk.as_str().to_owned(),
            recommended_action: candidate.recommended_action.as_str().to_owned(),
        })
        .collect();

    views.sort_by(|left, right| {
        right
            .reclaimable_bytes
            .cmp(&left.reclaimable_bytes)
            .then_with(|| left.path.cmp(&right.path))
    });

    Ok(views)
}

/// Returns one page of directories that contain no files at all.
///
/// # Errors
/// Returns `E_VALIDATION` for a bad page size and `E_STALE_CURSOR` for a cursor
/// from another generation.
pub fn empty_dirs_page(
    snapshot: &TreeSnapshot,
    metric: Metric,
    sort: SortSpec,
    page_size: u16,
    cursor: Option<&str>,
) -> Result<TreePage, UiError> {
    validate_page_size(page_size)?;
    let ids = find_empty_dirs(snapshot.tree());
    let window = paging::page_window(
        ids.len(),
        page_size,
        cursor,
        snapshot.generation(),
        metric,
        sort,
    )?;
    let rows = ids[window.start..window.end]
        .iter()
        .map(|id| snapshot.node_row(*id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TreePage {
        generation: snapshot.generation(),
        parent_id: None,
        cursor: cursor.map(str::to_owned),
        next_cursor: window.next_cursor,
        rows,
        total_estimate: Some(ids.len() as u64),
        metric,
        sort,
        stale_cursor: false,
    })
}

fn matches_cleanup_filter(
    candidate: &freespace_analysis::recommend::CleanupCandidate,
    filter: &FilterSet,
) -> bool {
    let path = candidate.path.to_string_lossy().to_lowercase();
    if !filter.text.is_empty() && !path.contains(&filter.text.to_lowercase()) {
        return false;
    }
    if filter
        .min_size
        .is_some_and(|minimum| candidate.reclaimable_bytes < minimum)
    {
        return false;
    }
    if filter
        .max_size
        .is_some_and(|maximum| candidate.reclaimable_bytes > maximum)
    {
        return false;
    }
    if !filter.exclude_paths.is_empty() {
        let excluded = filter.exclude_paths.iter().any(|candidate| {
            let candidate = candidate.trim_end_matches(['\\', '/']).to_lowercase();
            !candidate.is_empty()
                && (path == candidate
                    || path.starts_with(&format!("{candidate}\\"))
                    || path.starts_with(&format!("{candidate}/")))
        });
        if excluded {
            return false;
        }
    }

    true
}

/// Directories whose whole subtree contains no regular file.#[must_use]
pub fn find_empty_dirs(tree: &FsTree) -> Vec<NodeId> {
    let mut empty = Vec::new();
    let mut stack: Vec<(NodeId, bool)> = tree.roots().map(|id| (id, true)).collect();

    while let Some((id, top_level)) = stack.pop() {
        let node = tree.node(id);
        if node.is_removed() {
            continue;
        }
        if node.kind.is_directory() {
            if node.file_count() == 0 && !top_level {
                empty.push(id);
                continue;
            }
            stack.extend(tree.children(id).map(|child| (child, false)));
        }
    }

    empty.sort_by_key(|left| tree.path(*left));
    empty
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        model::{FileSystem, Volume, VolumeStatus},
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::{cleanup_candidates, empty_dirs_page, find_empty_dirs};
    use crate::dto::{FilterSet, Metric, SortSpec};
    use crate::tree::TreeSnapshot;

    fn snapshot() -> TreeSnapshot {
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: "root".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        let cache = tree
            .add_node(
                Some(root),
                NodeSpec {
                    name: "Cache".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("cache");
        tree.add_node(
            Some(cache),
            NodeSpec {
                name: "blob.bin".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(64),
                ..NodeSpec::default()
            },
        )
        .expect("blob");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "report.bak".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(32),
                ..NodeSpec::default()
            },
        )
        .expect("backup");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "empty".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("empty");
        tree.recompute_totals();

        let volume = Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(r"C:\")),
            mount_point: r"C:\".into(),
            label: "Test".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000,
            free_bytes: 100,
            cluster_size: None,
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };

        TreeSnapshot::new(tree, 4, volume, None)
    }

    #[test]
    fn maps_recommendations_and_filters_them() {
        let snapshot = snapshot();
        let all = cleanup_candidates(&snapshot, 0, &FilterSet::default()).expect("all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].category, "cache");
        assert_eq!(all[0].reason_key, "cleanup.reason.cache");
        assert_eq!(all[0].recommended_action, "delete");

        let filter = FilterSet {
            text: "report".to_owned(),
            ..FilterSet::default()
        };
        let backups = cleanup_candidates(&snapshot, 0, &filter).expect("backups");
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].risk, "high");
    }

    #[test]
    fn finds_and_pages_empty_directories() {
        let snapshot = snapshot();
        let ids = find_empty_dirs(snapshot.tree());
        assert_eq!(ids.len(), 1);

        let page = empty_dirs_page(&snapshot, Metric::Logical, SortSpec::default(), 10, None)
            .expect("page");
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].name, "empty");
        assert_eq!(page.total_estimate, Some(1));
    }
}
