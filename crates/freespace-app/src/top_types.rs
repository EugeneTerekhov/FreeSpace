//! Top-file and file-type projections used by the analysis screens.
//!
//! Both views are read-only projections of an immutable generation. Sizes and
//! totals always come from Rust; the interface never sums rows itself.

use freespace_analysis::{
    top::{TopFile, top_files},
    types::{FileCategory, group_by_extension, is_regular_file},
};
use std::collections::HashSet;

use freespace_core::{
    query::extension_of,
    tree::{FsTree, NodeId},
};

use crate::{
    category,
    dto::{FilterSet, Metric, SortKey, SortSpec, TreePage, TypeSummary},
    error::UiError,
    paging,
    tree::TreeSnapshot,
    validation::{validate_filter, validate_page_size},
};

/// Largest N accepted by `top_files_page`.
pub const MAX_TOP_LIMIT: usize = 10_000;

/// Returns one page of the largest files, filtered and sorted natively.
///
/// # Errors
/// Returns `E_VALIDATION` for a bad limit, page size or filter and
/// `E_STALE_CURSOR` for a cursor from another generation or ordering.
#[allow(clippy::too_many_arguments)]
pub fn top_files_page(
    snapshot: &TreeSnapshot,
    limit: usize,
    metric: Metric,
    sort: SortSpec,
    filter: &FilterSet,
    page_size: u16,
    cursor: Option<&str>,
) -> Result<TreePage, UiError> {
    validate_page_size(page_size)?;
    validate_filter(filter)?;
    if limit == 0 || limit > MAX_TOP_LIMIT {
        return Err(UiError::validation(format!(
            "limit must be between 1 and {MAX_TOP_LIMIT}"
        )));
    }

    let files: Vec<TopFile> = top_files(snapshot.tree(), limit)
        .into_iter()
        .filter(|file| snapshot.matches_filter(file.node_id, filter))
        .collect();

    let mut rows = files
        .iter()
        .map(|file| snapshot.node_row(file.node_id))
        .collect::<Result<Vec<_>, _>>()?;
    sort_rows(&mut rows, sort, metric);

    let window = paging::page_window(
        rows.len(),
        page_size,
        cursor,
        snapshot.generation(),
        metric,
        sort,
    )?;
    let rows = rows[window.start..window.end].to_vec();

    Ok(TreePage {
        generation: snapshot.generation(),
        parent_id: None,
        cursor: cursor.map(str::to_owned),
        next_cursor: window.next_cursor,
        total_estimate: Some(files.len() as u64),
        metric,
        sort,
        stale_cursor: false,
        rows,
    })
}

/// Aggregates the current generation by extension, heaviest first.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed filter.
pub fn types_summary(
    snapshot: &TreeSnapshot,
    filter: &FilterSet,
) -> Result<Vec<TypeSummary>, UiError> {
    validate_filter(filter)?;

    let tree = snapshot.tree();
    let mut rows: Vec<TypeSummary> = group_by_extension(tree)
        .into_iter()
        .filter(|group| group_matches(snapshot, group.extension.as_str(), filter))
        .map(|group| TypeSummary {
            extension: group.extension,
            category: group.category.as_str().to_owned(),
            count: group.count,
            logical_size: group.size.logical,
            on_disk_size: group.size.on_disk,
            physical_size: group.size.physical,
            share: group.share,
        })
        .collect();

    rows.sort_by(|left, right| {
        right
            .logical_size
            .cmp(&left.logical_size)
            .then_with(|| left.extension.cmp(&right.extension))
    });

    Ok(rows)
}

/// Aggregates the current generation by coarse category, heaviest first.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed filter.
pub fn types_category_summary(
    snapshot: &TreeSnapshot,
    filter: &FilterSet,
) -> Result<Vec<TypeSummary>, UiError> {
    validate_filter(filter)?;
    if !filter.categories.is_empty() {
        // Category rows are grouped by the category itself; the filter is
        // applied by `types_summary` and by the file pages.
        let wanted = filter.categories.clone();
        let mut rows: Vec<TypeSummary> =
            freespace_analysis::types::group_by_category(snapshot.tree())
                .into_iter()
                .filter(|group| {
                    wanted
                        .iter()
                        .any(|item| item.eq_ignore_ascii_case(group.category.as_str()))
                })
                .map(|group| TypeSummary {
                    extension: String::new(),
                    category: group.category.as_str().to_owned(),
                    count: group.count,
                    logical_size: group.size.logical,
                    on_disk_size: group.size.on_disk,
                    physical_size: group.size.physical,
                    share: group.share,
                })
                .collect();
        rows.sort_by_key(|left| std::cmp::Reverse(left.logical_size));
        return Ok(rows);
    }

    let mut rows: Vec<TypeSummary> = freespace_analysis::types::group_by_category(snapshot.tree())
        .into_iter()
        .map(|group| TypeSummary {
            extension: String::new(),
            category: group.category.as_str().to_owned(),
            count: group.count,
            logical_size: group.size.logical,
            on_disk_size: group.size.on_disk,
            physical_size: group.size.physical,
            share: group.share,
        })
        .collect();
    rows.sort_by(|left, right| {
        right
            .logical_size
            .cmp(&left.logical_size)
            .then_with(|| left.category.cmp(&right.category))
    });

    Ok(rows)
}

/// Returns one page of files of a selected extension or category.
///
/// `type_key` is either an extension (`pdf`), `category:<key>`
/// (`category:video`), an empty string for files without an extension, or
/// `category:folder` for directories. Volume roots are never folder rows.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed type key, page size or filter and
/// `E_STALE_CURSOR` for a cursor from another generation or ordering.
#[allow(clippy::too_many_arguments)]
pub fn types_files_page(
    snapshot: &TreeSnapshot,
    type_key: &str,
    metric: Metric,
    sort: SortSpec,
    filter: &FilterSet,
    page_size: u16,
    cursor: Option<&str>,
) -> Result<TreePage, UiError> {
    validate_filter(filter)?;
    validate_page_size(page_size)?;

    // "No extension" and "folders" cannot be expressed with a FilterSet: an
    // empty extension entry is invalid, and a `folder` category also matches
    // volume roots. Select their ids explicitly and page them directly.
    if type_key.trim().is_empty() || type_key == "category:folder" {
        let tree = snapshot.tree();
        let roots: HashSet<NodeId> = tree.roots().collect();
        let no_extension = type_key.trim().is_empty();
        let mut stack: Vec<NodeId> = tree.roots().collect();
        let mut ids = Vec::new();

        while let Some(id) = stack.pop() {
            if tree.node(id).is_removed() {
                continue;
            }
            let node = tree.node(id);
            let wanted = if node.kind.is_directory() {
                !no_extension && !roots.contains(&id)
            } else {
                no_extension && is_regular_file(node.kind) && extension_of(tree.name(id)).is_empty()
            };
            if wanted {
                ids.push(id);
            }
            stack.extend(tree.children(id));
        }

        return snapshot.ids_page(ids, metric, sort, filter, page_size, cursor);
    }

    let mut combined = filter.clone();
    if let Some(category_key) = type_key.strip_prefix("category:") {
        if category_key.trim().is_empty() {
            return Err(UiError::validation("category key is empty"));
        }
        combined.categories = vec![category_key.to_owned()];
    } else {
        combined.extensions = vec![type_key.trim().trim_start_matches('.').to_owned()];
    }

    snapshot.search_page("", metric, sort, &combined, page_size, cursor)
}

/// Files of a category or extension across the tree; used by the type screen.
#[must_use]
pub fn matching_file_ids(
    tree: &FsTree,
    extension: Option<&str>,
    category_key: Option<&str>,
) -> Vec<NodeId> {
    let mut ids = Vec::new();
    let mut stack: Vec<NodeId> = tree.roots().collect();

    while let Some(id) = stack.pop() {
        let node = tree.node(id);
        if node.is_removed() {
            continue;
        }
        if node.kind.is_directory() {
            stack.extend(tree.children(id));
            continue;
        }
        if !is_regular_file(node.kind) {
            continue;
        }
        let name = tree.name(id);
        let matches = extension.is_some_and(|wanted| {
            name.rsplit_once('.')
                .map(|(_, found)| found.eq_ignore_ascii_case(wanted.trim_start_matches('.')))
                .unwrap_or(false)
        }) || category_key.is_some_and(|wanted| {
            category::node_category_key(name, false).eq_ignore_ascii_case(wanted)
        });
        if matches {
            ids.push(id);
        }
    }

    ids
}

fn group_matches(snapshot: &TreeSnapshot, extension: &str, filter: &FilterSet) -> bool {
    if !filter.extensions.is_empty()
        && !filter.extensions.iter().any(|wanted| {
            wanted
                .trim_start_matches('.')
                .eq_ignore_ascii_case(extension)
        })
    {
        return false;
    }
    if !filter.categories.is_empty() {
        let category = freespace_analysis::types::category_for_extension(extension);
        if !filter
            .categories
            .iter()
            .any(|wanted| wanted.eq_ignore_ascii_case(category.as_str()))
        {
            return false;
        }
    }
    if filter.min_size.is_some() || filter.max_size.is_some() {
        // The summary has no single node size; the group is kept when any of its
        // files survives the size window.
        let mut stack: Vec<NodeId> = snapshot.tree().roots().collect();
        while let Some(id) = stack.pop() {
            let node = snapshot.tree().node(id);
            if node.is_removed() {
                continue;
            }
            if node.kind.is_directory() {
                stack.extend(snapshot.tree().children(id));
                continue;
            }
            if !is_regular_file(node.kind) {
                continue;
            }
            let name = snapshot.tree().name(id);
            let node_extension = name
                .rsplit_once('.')
                .map(|(_, value)| value)
                .unwrap_or_default();
            if !node_extension.eq_ignore_ascii_case(extension) {
                continue;
            }
            let size = node.size.logical;
            let above = filter.min_size.is_none_or(|minimum| size >= minimum);
            let below = filter.max_size.is_none_or(|maximum| size <= maximum);
            if above && below {
                return true;
            }
        }
        return false;
    }

    true
}

fn sort_rows(rows: &mut [crate::dto::NodeRow], sort: SortSpec, metric: Metric) {
    rows.sort_by(|left, right| {
        let ordering = match sort.key {
            SortKey::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
            SortKey::Path => left.path.to_lowercase().cmp(&right.path.to_lowercase()),
            SortKey::Size => row_size(left, metric).cmp(&row_size(right, metric)),
            SortKey::Modified => left.modified_at.cmp(&right.modified_at),
            SortKey::Items => {
                let left_items = left.file_count.saturating_add(u64::from(left.child_count));
                let right_items = right
                    .file_count
                    .saturating_add(u64::from(right.child_count));
                left_items.cmp(&right_items)
            }
        };
        let ordering = if sort.descending {
            ordering.reverse()
        } else {
            ordering
        };

        ordering.then_with(|| left.id.cmp(&right.id))
    });
}

const fn row_size(row: &crate::dto::NodeRow, metric: Metric) -> u64 {
    match metric {
        Metric::Logical => row.logical_size,
        Metric::OnDisk => row.on_disk_size,
        Metric::Physical => row.physical_size,
    }
}

/// Category key of an extension; exposed for tests and the client.
#[must_use]
pub fn category_key(extension: &str) -> String {
    let category: FileCategory = freespace_analysis::types::category_for_extension(extension);
    category.as_str().to_owned()
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        model::{FileSystem, Volume, VolumeStatus},
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::{
        MAX_TOP_LIMIT, category_key, matching_file_ids, top_files_page, types_category_summary,
        types_files_page, types_summary,
    };
    use crate::dto::{FilterSet, Metric, NodeKind as RowKind, SortKey, SortSpec};
    use crate::error::UiErrorCode;
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
        for (name, size) in [
            ("huge.mkv", 900),
            ("report.pdf", 300),
            ("notes.txt", 100),
            ("copy.txt", 100),
        ] {
            tree.add_node(
                Some(root),
                NodeSpec {
                    name: name.to_owned(),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(size),
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        }
        tree.recompute_totals();
        let volume = Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(r"C:\")),
            mount_point: r"C:\".into(),
            label: "Test".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 10_000,
            free_bytes: 1_000,
            cluster_size: None,
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };

        TreeSnapshot::new(tree, 3, volume, None)
    }

    #[test]
    fn top_files_respect_the_limit_and_sorting() {
        let snapshot = snapshot();
        let page = top_files_page(
            &snapshot,
            2,
            Metric::Logical,
            SortSpec::default(),
            &FilterSet::default(),
            10,
            None,
        )
        .expect("page");

        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.rows[0].name, "huge.mkv");
        assert_eq!(page.rows[1].name, "report.pdf");
        assert_eq!(page.total_estimate, Some(2));

        let by_name = top_files_page(
            &snapshot,
            10,
            Metric::Logical,
            SortSpec {
                key: SortKey::Name,
                descending: false,
            },
            &FilterSet::default(),
            2,
            None,
        )
        .expect("name page");
        assert_eq!(by_name.rows[0].name, "copy.txt");
        assert!(by_name.next_cursor.is_some());
    }

    #[test]
    fn top_files_sorts_by_full_path_across_volumes() {
        let mut tree = FsTree::new();
        let c = tree
            .add_node(
                None,
                NodeSpec {
                    name: r"C:\".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("C");
        let d = tree
            .add_node(
                None,
                NodeSpec {
                    name: r"D:\".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("D");
        // Names deliberately oppose the path order so this cannot pass by
        // accidentally sorting by name or size.
        for (root, name, size) in [(c, "zzz.mkv", 10), (d, "aaa.mkv", 20), (d, "bbb.mkv", 5)] {
            tree.add_node(
                Some(root),
                NodeSpec {
                    name: name.to_owned(),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(size),
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        }
        tree.recompute_totals();

        let volume = |mount: &str| Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(mount)),
            mount_point: mount.into(),
            label: mount.to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000,
            free_bytes: 100,
            cluster_size: None,
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };
        let snapshot =
            TreeSnapshot::new_multi(tree, 7, vec![volume(r"C:\"), volume(r"D:\")], Vec::new());

        let asc = top_files_page(
            &snapshot,
            10,
            Metric::Logical,
            SortSpec {
                key: SortKey::Path,
                descending: false,
            },
            &FilterSet::default(),
            10,
            None,
        )
        .expect("ascending page");
        let names: Vec<&str> = asc.rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["zzz.mkv", "aaa.mkv", "bbb.mkv"]);

        let desc = top_files_page(
            &snapshot,
            10,
            Metric::Logical,
            SortSpec {
                key: SortKey::Path,
                descending: true,
            },
            &FilterSet::default(),
            10,
            None,
        )
        .expect("descending page");
        let names: Vec<&str> = desc.rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["bbb.mkv", "aaa.mkv", "zzz.mkv"]);
    }

    #[test]
    fn top_files_filters_and_validates_the_limit() {
        let snapshot = snapshot();
        let filter = FilterSet {
            extensions: vec!["txt".to_owned()],
            ..FilterSet::default()
        };
        let page = top_files_page(
            &snapshot,
            100,
            Metric::Logical,
            SortSpec::default(),
            &filter,
            10,
            None,
        )
        .expect("filtered");
        assert_eq!(page.rows.len(), 2);

        let error = top_files_page(
            &snapshot,
            MAX_TOP_LIMIT + 1,
            Metric::Logical,
            SortSpec::default(),
            &FilterSet::default(),
            10,
            None,
        )
        .expect_err("limit");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    #[test]
    fn types_summary_uses_native_sizes() {
        let snapshot = snapshot();
        let rows = types_summary(&snapshot, &FilterSet::default()).expect("summary");
        let txt = rows
            .iter()
            .find(|row| row.extension == "txt")
            .expect("txt row");

        assert_eq!(txt.count, 2);
        assert_eq!(txt.logical_size, 200);
        assert_eq!(txt.category, "document");
        assert_eq!(category_key("mkv"), "video");
        assert_eq!(rows[0].extension, "mkv");
    }

    #[test]
    fn types_files_page_selects_a_category() {
        let snapshot = snapshot();
        let page = types_files_page(
            &snapshot,
            "category:video",
            Metric::Logical,
            SortSpec::default(),
            &FilterSet::default(),
            10,
            None,
        )
        .expect("files");

        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].name, "huge.mkv");
    }

    #[test]
    fn matching_file_ids_handles_extensions_and_categories() {
        let snapshot = snapshot();
        let tree = snapshot.tree();
        assert_eq!(matching_file_ids(tree, Some("txt"), None).len(), 2);
        assert_eq!(matching_file_ids(tree, None, Some("video")).len(), 1);
    }

    /// Tree with two volume roots, nested directories and extension-less files.
    fn grouped_snapshot() -> TreeSnapshot {
        let mut tree = FsTree::new();
        let c = tree
            .add_node(
                None,
                NodeSpec {
                    name: r"C:\".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("C");
        let d = tree
            .add_node(
                None,
                NodeSpec {
                    name: r"D:\".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("D");
        let docs = tree
            .add_node(
                Some(c),
                NodeSpec {
                    name: "Docs".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("Docs");
        let nested = tree
            .add_node(
                Some(docs),
                NodeSpec {
                    name: "Nested".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("Nested");
        for (parent, name) in [
            (c, "README"),
            (c, "notes.txt"),
            (docs, "LICENSE"),
            (d, "NOTICE"),
            (nested, "movie.mkv"),
        ] {
            tree.add_node(
                Some(parent),
                NodeSpec {
                    name: name.to_owned(),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(10),
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        }
        tree.recompute_totals();

        let volume = |mount: &str| Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(mount)),
            mount_point: mount.into(),
            label: mount.to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000,
            free_bytes: 100,
            cluster_size: None,
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };
        TreeSnapshot::new_multi(tree, 7, vec![volume(r"C:\"), volume(r"D:\")], Vec::new())
    }

    #[test]
    fn no_extension_page_contains_only_extension_less_files() {
        let snapshot = grouped_snapshot();
        let page = types_files_page(
            &snapshot,
            "",
            Metric::Logical,
            SortSpec::default(),
            &FilterSet::default(),
            50,
            None,
        )
        .expect("files");

        let mut names: Vec<&str> = page.rows.iter().map(|row| row.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, ["LICENSE", "NOTICE", "README"]);
        assert!(page.rows.iter().all(|row| row.kind == RowKind::File));
    }

    #[test]
    fn folder_page_excludes_volume_roots() {
        let snapshot = grouped_snapshot();
        let page = types_files_page(
            &snapshot,
            "category:folder",
            Metric::Logical,
            SortSpec::default(),
            &FilterSet::default(),
            50,
            None,
        )
        .expect("folders");

        let mut names: Vec<&str> = page.rows.iter().map(|row| row.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, ["Docs", "Nested"]);
        assert!(page.rows.iter().all(|row| row.kind == RowKind::Directory));
    }

    #[test]
    fn category_summary_counts_folders_but_not_volume_roots() {
        let snapshot = grouped_snapshot();
        let rows = types_category_summary(&snapshot, &FilterSet::default()).expect("summary");
        let folders = rows
            .iter()
            .find(|row| row.category == "folder")
            .expect("folder row");

        assert_eq!(folders.count, 2);
        assert_eq!(folders.logical_size, 0);
        assert_eq!(folders.share, 0.0);
        assert_eq!(rows.iter().map(|row| row.count).sum::<u64>(), 7);
    }
}
