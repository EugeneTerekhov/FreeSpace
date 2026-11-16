//! Read-only projections of an immutable scan tree.
//!
//! Every page is bound to a generation: a tree never changes after a scan, so
//! cursors are cheap and a stale cursor is detected by the generation, metric and
//! ordering stored inside it. Sizes and totals are copied from `SizeMetrics` and
//! never recomputed in TypeScript.

use std::cmp::Ordering;

use freespace_core::{
    model::{ScanSession, Volume},
    tree::{FsTree, NodeId},
};

use crate::{
    category,
    dto::{AttributeFlags, FilterSet, Metric, NodeKind, NodeRow, SortKey, SortSpec, TreePage},
    error::{UiError, UiErrorCode},
    paging,
    validation::{validate_filter, validate_filter_text, validate_page_size},
};

/// An immutable tree plus the generation and volume it belongs to.
#[derive(Debug, Clone)]
pub struct TreeSnapshot {
    tree: FsTree,
    generation: u64,
    volume: Volume,
    session: Option<ScanSession>,
    roots: Vec<NodeId>,
}

impl TreeSnapshot {
    /// Wraps a finished scan.
    #[must_use]
    pub fn new(
        tree: FsTree,
        generation: u64,
        volume: Volume,
        session: Option<ScanSession>,
    ) -> Self {
        let roots = tree.roots().collect();
        Self {
            tree,
            generation,
            volume,
            session,
            roots,
        }
    }

    /// Generation of the snapshot.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Analysed volume.
    #[must_use]
    pub const fn volume(&self) -> &Volume {
        &self.volume
    }

    /// Scan session metadata, when the snapshot came from a scan.
    #[must_use]
    pub const fn session(&self) -> Option<&ScanSession> {
        self.session.as_ref()
    }

    /// The underlying tree.
    #[must_use]
    pub const fn tree(&self) -> &FsTree {
        &self.tree
    }

    /// First scan root.
    #[must_use]
    pub fn root_id(&self) -> Option<NodeId> {
        self.roots.first().copied()
    }

    /// Number of live nodes.
    #[must_use]
    pub fn node_count(&self) -> u64 {
        u64::try_from(self.tree.live_len()).unwrap_or(u64::MAX)
    }

    /// Projects one node into a row.
    ///
    /// # Errors
    /// Returns `E_STALE_NODE` when the identifier does not belong to this
    /// generation.
    pub fn node_row(&self, id: NodeId) -> Result<NodeRow, UiError> {
        if usize::try_from(id).map_or(true, |index| index >= self.tree.len())
            || self.tree.node(id).is_removed()
        {
            return Err(UiError::new(UiErrorCode::StaleNode).with_details(format!(
                "node {id} is not part of generation {}",
                self.generation
            )));
        }

        let node = self.tree.node(id);
        let child_count = u32::try_from(self.tree.children(id).count()).unwrap_or(u32::MAX);
        let modified = node.timestamps.modified.as_millis();
        let accessed = node.timestamps.accessed.as_millis();

        Ok(NodeRow {
            id,
            parent_id: node.parent(),
            name: self.tree.name(id).to_owned(),
            path: self.tree.path(id).to_string_lossy().into_owned(),
            kind: map_kind(node.kind),
            logical_size: node.size.logical,
            on_disk_size: node.size.on_disk,
            physical_size: node.size.physical,
            ads_size: node.size.ads,
            child_count,
            file_count: u64::from(node.file_count()),
            modified_at: (modified != 0).then_some(modified),
            accessed_at: (accessed != 0).then_some(accessed),
            attributes: AttributeFlags {
                read_only: node
                    .attributes
                    .contains(freespace_core::tree::FileAttributes::READ_ONLY),
                hidden: node
                    .attributes
                    .contains(freespace_core::tree::FileAttributes::HIDDEN),
                system: node
                    .attributes
                    .contains(freespace_core::tree::FileAttributes::SYSTEM),
                compressed: node
                    .attributes
                    .contains(freespace_core::tree::FileAttributes::COMPRESSED),
                sparse: node
                    .attributes
                    .contains(freespace_core::tree::FileAttributes::SPARSE_FILE),
                reparse: node
                    .attributes
                    .contains(freespace_core::tree::FileAttributes::REPARSE_POINT),
            },
            accessible: node.is_accessible(),
            link_target: self.tree.link_target(id).map(str::to_owned),
        })
    }

    /// Returns the ancestors of a node from the root to its parent.
    ///
    /// # Errors
    /// Returns `E_STALE_NODE` for an identifier outside the generation.
    pub fn path_rows(&self, id: NodeId) -> Result<Vec<NodeRow>, UiError> {
        self.node_row(id)?;

        let mut ancestors = Vec::new();
        let mut current = self.tree.node(id).parent();
        while let Some(parent) = current {
            ancestors.push(self.node_row(parent)?);
            current = self.tree.node(parent).parent();
        }
        ancestors.reverse();

        Ok(ancestors)
    }

    /// First page of the scan roots.
    ///
    /// # Errors
    /// Returns `E_STALE_CURSOR` when the cursor belongs to another request and
    /// `E_VALIDATION` for an invalid page size.
    pub fn root_page(
        &self,
        metric: Metric,
        sort: SortSpec,
        page_size: u16,
        cursor: Option<&str>,
    ) -> Result<TreePage, UiError> {
        self.build_page(
            self.roots.clone(),
            PageSpec {
                parent_id: None,
                metric,
                sort,
                filter: &FilterSet::default(),
                cursor,
                page_size,
            },
        )
    }

    /// One page of the children of `parent`.
    ///
    /// # Errors
    /// Returns `E_STALE_NODE` for an unknown parent and `E_STALE_CURSOR` for a
    /// cursor from another request.
    pub fn children_page(
        &self,
        parent: NodeId,
        metric: Metric,
        sort: SortSpec,
        filter: &FilterSet,
        page_size: u16,
        cursor: Option<&str>,
    ) -> Result<TreePage, UiError> {
        self.node_row(parent)?;
        validate_filter(filter)?;

        let ids: Vec<NodeId> = self.tree.children(parent).collect();
        self.build_page(
            ids,
            PageSpec {
                parent_id: Some(parent),
                metric,
                sort,
                filter,
                cursor,
                page_size,
            },
        )
    }

    /// Searches every live node for a name fragment.
    ///
    /// The scan is linear: `10M` nodes take several seconds, which is why search
    /// is an explicit action and not a keystroke filter.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for a malformed query or filter and
    /// `E_STALE_CURSOR` for a cursor from another request.
    pub fn search_page(
        &self,
        query: &str,
        metric: Metric,
        sort: SortSpec,
        filter: &FilterSet,
        page_size: u16,
        cursor: Option<&str>,
    ) -> Result<TreePage, UiError> {
        validate_filter_text(query)?;
        validate_filter(filter)?;

        let needle = query.to_lowercase();
        let mut ids = Vec::new();
        for index in 0..self.tree.len() {
            let Ok(id) = NodeId::try_from(index) else {
                break;
            };
            if self.tree.node(id).is_removed() {
                continue;
            }
            if !self.matches_filter(id, filter) {
                continue;
            }
            if !needle.is_empty() && !self.tree.name(id).to_lowercase().contains(&needle) {
                continue;
            }
            ids.push(id);
        }

        self.build_page(
            ids,
            PageSpec {
                parent_id: None,
                metric,
                sort,
                filter: &FilterSet::default(),
                cursor,
                page_size,
            },
        )
    }

    /// Returns every child of `parent` sorted by size without pagination.
    ///
    /// Used by the Treemap aggregation, which applies its own tile budget.
    #[must_use]
    pub fn sorted_children(&self, parent: NodeId, metric: Metric) -> Vec<(NodeId, u64)> {
        let mut children: Vec<(NodeId, u64)> = self
            .tree
            .children(parent)
            .map(|id| (id, metric.of(self.tree.node(id).size)))
            .filter(|(_, size)| *size > 0)
            .collect();
        children.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
        children
    }

    fn build_page(&self, ids: Vec<NodeId>, spec: PageSpec<'_>) -> Result<TreePage, UiError> {
        validate_page_size(spec.page_size)?;

        let mut keyed: Vec<(SortValue, NodeId)> = ids
            .into_iter()
            .filter(|id| self.matches_filter(*id, spec.filter))
            .map(|id| (self.sort_value(id, spec.metric, spec.sort.key), id))
            .collect();

        keyed.sort_by(|(left_value, left_id), (right_value, right_id)| {
            let ordering = compare_values(left_value, right_value);
            let ordering = if spec.sort.descending {
                ordering.reverse()
            } else {
                ordering
            };
            ordering.then(left_id.cmp(right_id))
        });

        let window = paging::page_window(
            keyed.len(),
            spec.page_size,
            spec.cursor,
            self.generation,
            spec.metric,
            spec.sort,
        )?;

        let rows = keyed[window.start..window.end]
            .iter()
            .map(|(_, id)| self.node_row(*id))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TreePage {
            generation: self.generation,
            parent_id: spec.parent_id,
            cursor: spec.cursor.map(str::to_owned),
            next_cursor: window.next_cursor,
            rows,
            total_estimate: Some(u64::try_from(keyed.len()).unwrap_or(u64::MAX)),
            metric: spec.metric,
            sort: spec.sort,
            stale_cursor: false,
        })
    }

    fn sort_value(&self, id: NodeId, metric: Metric, key: SortKey) -> SortValue {
        let node = self.tree.node(id);
        match key {
            SortKey::Name => SortValue::Text(self.tree.name(id).to_lowercase()),
            SortKey::Size => SortValue::Number(metric.of(node.size)),
            SortKey::Modified => SortValue::Signed(node.timestamps.modified.as_millis()),
            SortKey::Items => SortValue::Number(
                u64::from(node.file_count()).saturating_add(u64::from(node.dir_count())),
            ),
        }
    }

    /// Returns `true` when the node passes every condition of `filter`.
    ///
    /// Shared by the tree and the analysis screens so a filter means the same
    /// thing everywhere.
    #[must_use]
    pub fn matches_filter(&self, id: NodeId, filter: &FilterSet) -> bool {
        let node = self.tree.node(id);
        if node.is_removed() {
            return false;
        }
        if filter.accessible_only && !node.is_accessible() {
            return false;
        }

        let name = self.tree.name(id);
        if !filter.text.is_empty() && !name.to_lowercase().contains(&filter.text.to_lowercase()) {
            return false;
        }

        if !filter.extensions.is_empty() {
            if node.is_directory() {
                return false;
            }
            let extension = name
                .rsplit_once('.')
                .map(|(_, extension)| extension)
                .unwrap_or_default();
            let matched = filter.extensions.iter().any(|candidate| {
                candidate
                    .strip_prefix('.')
                    .unwrap_or(candidate)
                    .eq_ignore_ascii_case(extension)
            });
            if !matched {
                return false;
            }
        }

        if !filter.categories.is_empty() {
            let key = category::node_category_key(name, node.is_directory());
            if !filter
                .categories
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(key))
            {
                return false;
            }
        }

        if filter
            .min_size
            .is_some_and(|minimum| node.size.logical < minimum)
        {
            return false;
        }
        if filter
            .max_size
            .is_some_and(|maximum| node.size.logical > maximum)
        {
            return false;
        }

        let modified = node.timestamps.modified.as_millis();
        if filter.modified_after.is_some_and(|after| modified < after) {
            return false;
        }
        if filter
            .modified_before
            .is_some_and(|before| modified > before)
        {
            return false;
        }

        if !filter.exclude_paths.is_empty() {
            let path = self.tree.path(id).to_string_lossy().to_lowercase();
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
}

/// Sort key value kept at hand so comparisons do not allocate.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SortValue {
    Text(String),
    Number(u64),
    Signed(i64),
}

/// Parameters of one page request.
struct PageSpec<'a> {
    parent_id: Option<NodeId>,
    metric: Metric,
    sort: SortSpec,
    filter: &'a FilterSet,
    cursor: Option<&'a str>,
    page_size: u16,
}

fn compare_values(left: &SortValue, right: &SortValue) -> Ordering {
    match (left, right) {
        (SortValue::Text(left), SortValue::Text(right)) => left.cmp(right),
        (SortValue::Number(left), SortValue::Number(right)) => left.cmp(right),
        (SortValue::Signed(left), SortValue::Signed(right)) => left.cmp(right),
        _ => Ordering::Equal,
    }
}

fn map_kind(kind: freespace_core::tree::NodeKind) -> NodeKind {
    use freespace_core::tree::NodeKind as Core;

    match kind {
        Core::File | Core::Hardlink => NodeKind::File,
        Core::Directory => NodeKind::Directory,
        Core::Symlink | Core::Junction => NodeKind::Link,
    }
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        model::{FileSystem, ScanMethod, ScanSession, Volume, VolumeStatus},
        time::Timestamp,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::TreeSnapshot;
    use crate::dto::{FilterSet, Metric, NodeKind as DtoKind, SortKey, SortSpec};
    use crate::error::UiErrorCode;

    fn build_tree() -> FsTree {
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

        tree.add_node(
            Some(root),
            NodeSpec {
                name: "alpha.txt".to_owned(),
                kind: NodeKind::File,
                size: freespace_core::size::SizeMetrics::file(100),
                timestamps: freespace_core::tree::Timestamps {
                    modified: Timestamp::from_millis(1_000),
                    ..freespace_core::tree::Timestamps::default()
                },
                ..NodeSpec::default()
            },
        )
        .expect("alpha");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "Beta.bin".to_owned(),
                kind: NodeKind::File,
                size: freespace_core::size::SizeMetrics::file(300),
                timestamps: freespace_core::tree::Timestamps {
                    modified: Timestamp::from_millis(3_000),
                    ..freespace_core::tree::Timestamps::default()
                },
                ..NodeSpec::default()
            },
        )
        .expect("beta");
        let gamma = tree
            .add_node(
                Some(root),
                NodeSpec {
                    name: "gamma".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("gamma");
        tree.add_node(
            Some(gamma),
            NodeSpec {
                name: "inner.dat".to_owned(),
                kind: NodeKind::File,
                size: freespace_core::size::SizeMetrics::file(50),
                ..NodeSpec::default()
            },
        )
        .expect("inner");

        tree.recompute_totals();
        tree
    }

    fn snapshot() -> TreeSnapshot {
        let tree = build_tree();
        let volume = Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(r"C:\")),
            mount_point: r"C:\".into(),
            label: "Test".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000,
            free_bytes: 100,
            cluster_size: Some(4096),
            scan_method: Some(ScanMethod::DirectoryWalk),
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };
        let session = ScanSession::new(
            volume.id,
            vec![r"C:\".into()],
            ScanMethod::DirectoryWalk,
            Vec::new(),
        );

        TreeSnapshot::new(tree, 7, volume, Some(session))
    }

    fn root(snapshot: &TreeSnapshot) -> u32 {
        snapshot.root_id().expect("root")
    }

    #[test]
    fn pages_children_by_size_and_by_name() {
        let snapshot = snapshot();
        let size_order = snapshot
            .children_page(
                root(&snapshot),
                Metric::OnDisk,
                SortSpec {
                    key: SortKey::Size,
                    descending: true,
                },
                &FilterSet::default(),
                10,
                None,
            )
            .expect("page");
        let names: Vec<_> = size_order
            .rows
            .iter()
            .map(|row| row.name.as_str())
            .collect();
        assert_eq!(names, ["Beta.bin", "alpha.txt", "gamma"]);
        assert_eq!(size_order.rows[2].kind, DtoKind::Directory);
        assert_eq!(size_order.generation, 7);

        let name_order = snapshot
            .children_page(
                root(&snapshot),
                Metric::OnDisk,
                SortSpec {
                    key: SortKey::Name,
                    descending: false,
                },
                &FilterSet::default(),
                10,
                None,
            )
            .expect("page");
        let names: Vec<_> = name_order
            .rows
            .iter()
            .map(|row| row.name.as_str())
            .collect();
        assert_eq!(names, ["alpha.txt", "Beta.bin", "gamma"]);
    }

    #[test]
    fn paginates_with_a_cursor_bound_to_the_generation() {
        let snapshot = snapshot();
        let sort = SortSpec {
            key: SortKey::Size,
            descending: true,
        };
        let first = snapshot
            .children_page(
                root(&snapshot),
                Metric::OnDisk,
                sort,
                &FilterSet::default(),
                2,
                None,
            )
            .expect("first");
        assert_eq!(first.rows.len(), 2);
        assert!(first.next_cursor.is_some());

        let second = snapshot
            .children_page(
                root(&snapshot),
                Metric::OnDisk,
                sort,
                &FilterSet::default(),
                2,
                first.next_cursor.as_deref(),
            )
            .expect("second");
        assert_eq!(second.rows.len(), 1);
        assert_eq!(second.rows[0].name, "gamma");
        assert!(second.next_cursor.is_none());

        let error = TreeSnapshot::new(build_tree(), 8, snapshot.volume().clone(), None)
            .children_page(
                root(&snapshot),
                Metric::OnDisk,
                sort,
                &FilterSet::default(),
                2,
                first.next_cursor.as_deref(),
            )
            .expect_err("stale");
        assert_eq!(error.code, UiErrorCode::StaleCursor);
    }

    #[test]
    fn searches_case_insensitively() {
        let snapshot = snapshot();
        let page = snapshot
            .search_page(
                "BETA",
                Metric::OnDisk,
                SortSpec::default(),
                &FilterSet::default(),
                10,
                None,
            )
            .expect("search");

        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].name, "Beta.bin");
    }

    #[test]
    fn filters_by_extension_and_category() {
        let snapshot = snapshot();
        let filter = FilterSet {
            extensions: vec!["txt".to_owned()],
            ..FilterSet::default()
        };
        let page = snapshot
            .children_page(
                root(&snapshot),
                Metric::Logical,
                SortSpec::default(),
                &filter,
                10,
                None,
            )
            .expect("filter");
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].name, "alpha.txt");

        let filter = FilterSet {
            categories: vec!["folder".to_owned()],
            ..FilterSet::default()
        };
        let page = snapshot
            .children_page(
                root(&snapshot),
                Metric::Logical,
                SortSpec::default(),
                &filter,
                10,
                None,
            )
            .expect("category");
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].name, "gamma");
    }

    #[test]
    fn returns_ancestors_from_root_to_parent() {
        let snapshot = snapshot();
        let inner = snapshot
            .search_page(
                "inner",
                Metric::OnDisk,
                SortSpec::default(),
                &FilterSet::default(),
                10,
                None,
            )
            .expect("search")
            .rows[0]
            .id;
        let path = snapshot.path_rows(inner).expect("ancestors");
        let names: Vec<_> = path.iter().map(|row| row.name.as_str()).collect();

        assert_eq!(names, ["root", "gamma"]);
    }

    #[test]
    fn rejects_nodes_from_another_generation() {
        let snapshot = snapshot();
        let error = snapshot.node_row(999).expect_err("stale");
        assert_eq!(error.code, UiErrorCode::StaleNode);
    }
}
