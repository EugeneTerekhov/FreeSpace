//! Owner summaries and per-owner file pages (US5, T112).
//!
//! Reading ownership costs one security query per file, so the annotated result
//! is cached per generation. The cache keeps the dense `owner_id -> option`
//! table instead of a second tree clone: pages are then cheap and the immutable
//! scan snapshot is never mutated.
//!
//! Objects whose owner cannot be read land in the documented `unknown` category
//! and stay visible; the report never hides them and never fails because of a
//! single denied descriptor.

use std::{
    path::Path,
    sync::{Arc, LazyLock, Mutex, MutexGuard},
};

use freespace_analysis::owners::{self as analysis_owners, OwnerTable, UNKNOWN_OWNER};
use freespace_core::tree::{FsTree, NodeId};
use freespace_platform::security::{self, OwnerIdentity};

use crate::{
    dto::{
        FilterSet, Metric, OwnerSummaryPage, OwnerSummaryView, SortKey, SortSpec, TreePage,
        inspection::scoped_generation,
    },
    error::{UiError, UiErrorCode},
    paging,
    tree::TreeSnapshot,
    validation::{validate_filter, validate_page_size},
};

/// Resolver used to read the owner of one path.
pub type OwnerResolver =
    Arc<dyn Fn(&Path) -> freespace_core::Result<Option<OwnerIdentity>> + Send + Sync>;

/// Annotated ownership of one generation.
#[derive(Debug, Clone)]
struct OwnerIndex {
    generation: u64,
    node_count: u64,
    volume_id: String,
    root_path: String,
    table: OwnerTable,
    assignments: Vec<Option<u32>>,
    summaries: Vec<freespace_analysis::OwnerSummary>,
    total_bytes: u64,
    unknown_bytes: u64,
    error_count: u64,
}

impl OwnerIndex {
    fn matches(&self, snapshot: &TreeSnapshot) -> bool {
        self.generation == snapshot.generation()
            && self.node_count == snapshot.node_count()
            && self.volume_id == snapshot.volume().id.to_string()
            && self.root_path == root_path(snapshot)
    }
}

/// Reusable owner report with a per-generation cache.
#[derive(Clone)]
pub struct OwnerService {
    index: Arc<Mutex<Option<Arc<OwnerIndex>>>>,
    resolver: OwnerResolver,
}

impl std::fmt::Debug for OwnerService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cached = self.index.lock().ok().and_then(|guard| {
            guard
                .as_ref()
                .map(|index| (index.generation, index.assignments.len()))
        });

        formatter
            .debug_struct("OwnerService")
            .field("cached", &cached)
            .finish_non_exhaustive()
    }
}

impl Default for OwnerService {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnerService {
    /// Creates a service that reads real security descriptors.
    #[must_use]
    pub fn new() -> Self {
        Self::with_resolver(Arc::new(security::owner_identity))
    }

    /// Creates a service with an injectable resolver; used by tests.
    #[must_use]
    pub fn with_resolver(resolver: OwnerResolver) -> Self {
        Self {
            index: Arc::new(Mutex::new(None)),
            resolver,
        }
    }

    /// Builds or reuses the annotated index of `snapshot`.
    fn ensure(&self, snapshot: &TreeSnapshot) -> Result<Arc<OwnerIndex>, UiError> {
        if let Some(cached) = self
            .lock()
            .as_ref()
            .filter(|index| index.matches(snapshot))
            .map(Arc::clone)
        {
            return Ok(cached);
        }

        let built = Arc::new(self.build(snapshot)?);
        *self.lock() = Some(Arc::clone(&built));

        Ok(built)
    }

    fn build(&self, snapshot: &TreeSnapshot) -> Result<OwnerIndex, UiError> {
        let mut tree: FsTree = snapshot.tree().clone();
        let resolver = Arc::clone(&self.resolver);
        let scan = analysis_owners::annotate_with(&mut tree, move |path| resolver(path))?;

        let mut assignments = Vec::with_capacity(tree.len());
        for index in 0..tree.len() {
            let owner = NodeId::try_from(index)
                .ok()
                .map(|id| tree.node(id).owner_id)
                .unwrap_or(None);
            assignments.push(owner);
        }

        let unknown_bytes = scan
            .summaries
            .iter()
            .filter(|summary| summary.owner_id == u32::MAX)
            .map(|summary| summary.total_bytes)
            .fold(0, u64::saturating_add);

        let total_bytes = scan.total_bytes();

        Ok(OwnerIndex {
            generation: snapshot.generation(),
            node_count: snapshot.node_count(),
            volume_id: snapshot.volume().id.to_string(),
            root_path: root_path(snapshot),
            table: scan.table,
            assignments,
            summaries: scan.summaries,
            total_bytes,
            unknown_bytes,
            error_count: scan.error_count,
        })
    }

    /// One page of owner summaries.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for a malformed filter or page size,
    /// `E_STALE_CURSOR` for a cursor from another request, and `E_IO` when the
    /// tree cannot be enumerated.
    pub fn owners_page(
        &self,
        snapshot: &TreeSnapshot,
        filter: &FilterSet,
        page_size: u16,
        cursor: Option<&str>,
    ) -> Result<OwnerSummaryPage, UiError> {
        validate_filter(filter)?;
        validate_page_size(page_size)?;
        let index = self.ensure(snapshot)?;

        let rows: Vec<OwnerSummaryView> = index
            .summaries
            .iter()
            .filter(|summary| matches_owner_filter(summary, filter))
            .map(|summary| {
                let unknown = summary.owner_id == u32::MAX;
                OwnerSummaryView {
                    owner_id: (!unknown).then_some(summary.owner_id),
                    sid: summary.sid.clone(),
                    display_name: summary.display_name.clone(),
                    total_bytes: summary.total_bytes,
                    file_count: summary.file_count,
                    share: if index.total_bytes == 0 {
                        0.0
                    } else {
                        summary.total_bytes as f64 / index.total_bytes as f64
                    },
                    accessible: !unknown,
                    label_key: if unknown {
                        "owners.label.unknown".to_owned()
                    } else {
                        "owners.label.account".to_owned()
                    },
                }
            })
            .collect();

        let sort = SortSpec {
            key: SortKey::Size,
            descending: true,
        };
        let scope = format!("owners|{}", filter_scope(filter));
        let window = paging::page_window(
            rows.len(),
            page_size,
            cursor,
            scoped_generation(snapshot.generation(), &scope),
            Metric::Logical,
            sort,
        )?;

        let mut warnings = Vec::new();
        if index.error_count > 0 {
            warnings.push("owners.warning.unreadable".to_owned());
        }
        if index.unknown_bytes > 0 {
            warnings.push("owners.warning.unknown".to_owned());
        }

        Ok(OwnerSummaryPage {
            generation: snapshot.generation(),
            cursor: cursor.map(str::to_owned),
            next_cursor: window.next_cursor.clone(),
            rows: rows[window.start..window.end].to_vec(),
            total_estimate: u64::try_from(rows.len()).unwrap_or(u64::MAX),
            total_bytes: index.total_bytes,
            unknown_bytes: index.unknown_bytes,
            error_count: index.error_count,
            warnings,
            stale_cursor: false,
        })
    }

    /// One page of the files of `owner`.
    ///
    /// `owner` accepts an account name, a SID, a numeric owner index or the
    /// documented `unknown` label.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an owner that is not part of the generation,
    /// `E_VALIDATION` for a malformed request and `E_STALE_CURSOR` for a cursor
    /// from another request.
    #[allow(clippy::too_many_arguments)]
    pub fn owners_files_page(
        &self,
        snapshot: &TreeSnapshot,
        owner: &str,
        filter: &FilterSet,
        metric: Metric,
        sort: SortSpec,
        page_size: u16,
        cursor: Option<&str>,
    ) -> Result<TreePage, UiError> {
        validate_filter(filter)?;
        validate_page_size(page_size)?;
        let index = self.ensure(snapshot)?;
        let selection = resolve_owner(&index, owner)?;

        let mut keyed: Vec<(u64, NodeId)> = Vec::new();
        let nodes = index.assignments.len().min(snapshot.tree().len());
        for node_index in 0..nodes {
            let Ok(node_id) = NodeId::try_from(node_index) else {
                break;
            };
            let node = snapshot.tree().node(node_id);
            if node.is_removed()
                || node.kind != freespace_core::tree::NodeKind::File
                || !node.is_accessible()
            {
                continue;
            }
            if !snapshot.matches_filter(node_id, filter) {
                continue;
            }
            if !selection.matches(index.assignments.get(node_index).copied().flatten()) {
                continue;
            }
            keyed.push((metric.of(node.size), node_id));
        }

        keyed.sort_by(|(left_size, left_id), (right_size, right_id)| {
            let ordering = left_size.cmp(right_size);
            let ordering = if sort.descending {
                ordering.reverse()
            } else {
                ordering
            };
            ordering.then(left_id.cmp(right_id))
        });

        let scope = format!(
            "owner-files|{}|{}|{metric:?}|{}|{}",
            owner.trim().to_lowercase(),
            filter_scope(filter),
            sort.key.as_str(),
            sort.descending
        );
        let window = paging::page_window(
            keyed.len(),
            page_size,
            cursor,
            scoped_generation(snapshot.generation(), &scope),
            metric,
            sort,
        )?;

        let rows = keyed[window.start..window.end]
            .iter()
            .map(|(_, id)| snapshot.node_row(*id))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TreePage {
            generation: snapshot.generation(),
            parent_id: None,
            cursor: cursor.map(str::to_owned),
            next_cursor: window.next_cursor,
            rows,
            total_estimate: Some(u64::try_from(keyed.len()).unwrap_or(u64::MAX)),
            metric,
            sort,
            stale_cursor: false,
        })
    }

    fn lock(&self) -> MutexGuard<'_, Option<Arc<OwnerIndex>>> {
        self.index
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Global service used by the Tauri command layer.
static OWNERS: LazyLock<OwnerService> = LazyLock::new(OwnerService::new);

/// One page of owner summaries using the global cache.
///
/// # Errors
/// See [`OwnerService::owners_page`].
pub fn owners_page(
    snapshot: &TreeSnapshot,
    filter: &FilterSet,
    page_size: u16,
    cursor: Option<&str>,
) -> Result<OwnerSummaryPage, UiError> {
    OWNERS.owners_page(snapshot, filter, page_size, cursor)
}

/// One page of files of `owner` using the global cache.
///
/// # Errors
/// See [`OwnerService::owners_files_page`].
#[allow(clippy::too_many_arguments)]
pub fn owners_files_page(
    snapshot: &TreeSnapshot,
    owner: &str,
    filter: &FilterSet,
    metric: Metric,
    sort: SortSpec,
    page_size: u16,
    cursor: Option<&str>,
) -> Result<TreePage, UiError> {
    OWNERS.owners_files_page(snapshot, owner, filter, metric, sort, page_size, cursor)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerSelection {
    Known(u32),
    Unknown,
}

impl OwnerSelection {
    fn matches(self, assignment: Option<u32>) -> bool {
        match self {
            Self::Known(owner_id) => assignment == Some(owner_id),
            Self::Unknown => assignment.is_none(),
        }
    }
}

fn resolve_owner(index: &OwnerIndex, owner: &str) -> Result<OwnerSelection, UiError> {
    let owner = owner.trim();
    if owner.is_empty() {
        return Err(UiError::validation("owner is empty"));
    }
    if owner.eq_ignore_ascii_case(UNKNOWN_OWNER) {
        return Ok(OwnerSelection::Unknown);
    }
    if let Some(owner_id) = index.table.find(owner) {
        return Ok(OwnerSelection::Known(owner_id));
    }
    if let Ok(owner_id) = owner.parse::<u32>() {
        if index.table.get(owner_id).is_some() {
            return Ok(OwnerSelection::Known(owner_id));
        }
    }

    Err(UiError::new(UiErrorCode::NotFound)
        .with_details(format!("owner `{owner}` is not part of this generation")))
}

fn matches_owner_filter(summary: &freespace_analysis::OwnerSummary, filter: &FilterSet) -> bool {
    if filter.accessible_only && summary.owner_id == u32::MAX {
        return false;
    }
    let text = filter.text.trim().to_lowercase();
    if !text.is_empty()
        && !summary.display_name.to_lowercase().contains(&text)
        && !summary.sid.to_lowercase().contains(&text)
    {
        return false;
    }
    if filter
        .min_size
        .is_some_and(|minimum| summary.total_bytes < minimum)
    {
        return false;
    }
    !filter
        .max_size
        .is_some_and(|maximum| summary.total_bytes > maximum)
}

fn filter_scope(filter: &FilterSet) -> String {
    format!(
        "text={};ext={:?};cat={:?};min={:?};max={:?};after={:?};before={:?};accessible={};exclude={:?}",
        filter.text,
        filter.extensions,
        filter.categories,
        filter.min_size,
        filter.max_size,
        filter.modified_after,
        filter.modified_before,
        filter.accessible_only,
        filter.exclude_paths
    )
}

fn root_path(snapshot: &TreeSnapshot) -> String {
    snapshot
        .root_id()
        .map(|id| {
            snapshot
                .tree()
                .path(id)
                .to_string_lossy()
                .to_ascii_lowercase()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use freespace_core::{
        model::{FileSystem, ScanMethod, Volume, VolumeStatus},
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };
    use freespace_platform::security::OwnerIdentity;

    use super::{OwnerResolver, OwnerService};
    use crate::dto::{FilterSet, Metric, SortSpec};
    use crate::error::UiErrorCode;
    use crate::tree::TreeSnapshot;

    fn fixture() -> (tempfile::TempDir, TreeSnapshot) {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: directory.path().to_string_lossy().into_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        for (name, size) in [("alice.bin", 100u64), ("bob.bin", 250), ("shared.bin", 50)] {
            std::fs::write(directory.path().join(name), vec![0u8; size as usize]).expect("file");
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
            total_bytes: 1_000,
            free_bytes: 100,
            cluster_size: None,
            scan_method: Some(ScanMethod::DirectoryWalk),
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };

        (directory, TreeSnapshot::new(tree, 12, volume, None))
    }

    fn resolver() -> OwnerResolver {
        Arc::new(|path: &std::path::Path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let identity = match name {
                "alice.bin" => Some(("S-1-5-21-1", r"PC\Alice")),
                "bob.bin" => Some(("S-1-5-21-2", r"PC\Bob")),
                _ => None,
            };
            Ok(identity.map(|(sid, display_name)| OwnerIdentity {
                sid: sid.to_owned(),
                display_name: display_name.to_owned(),
            }))
        })
    }

    #[test]
    fn pages_owners_and_reports_unknown_files() {
        let (_directory, snapshot) = fixture();
        let service = OwnerService::with_resolver(resolver());

        let first = service
            .owners_page(&snapshot, &FilterSet::default(), 2, None)
            .expect("first page");
        assert_eq!(first.generation, 12);
        assert_eq!(first.rows.len(), 2);
        assert_eq!(first.rows[0].display_name, r"PC\Bob");
        assert_eq!(first.total_bytes, 400);
        assert_eq!(first.unknown_bytes, 50);
        assert!(first.next_cursor.is_some());

        let second = service
            .owners_page(
                &snapshot,
                &FilterSet::default(),
                2,
                first.next_cursor.as_deref(),
            )
            .expect("second page");
        assert_eq!(second.rows.len(), 1);
        assert_eq!(second.rows[0].display_name, "unknown");
        assert!(second.rows[0].owner_id.is_none());
        assert_eq!(second.rows[0].label_key, "owners.label.unknown");

        let filtered = FilterSet {
            text: "alice".to_owned(),
            ..FilterSet::default()
        };
        let page = service
            .owners_page(&snapshot, &filtered, 10, None)
            .expect("filtered");
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].file_count, 1);

        let stale = service
            .owners_page(&snapshot, &filtered, 10, first.next_cursor.as_deref())
            .expect_err("cursor from another filter");
        assert_eq!(stale.code, UiErrorCode::StaleCursor);
    }

    #[test]
    fn pages_files_of_one_owner() {
        let (_directory, snapshot) = fixture();
        let service = OwnerService::with_resolver(resolver());

        let page = service
            .owners_files_page(
                &snapshot,
                r"pc\alice",
                &FilterSet::default(),
                Metric::Logical,
                SortSpec::default(),
                10,
                None,
            )
            .expect("alice files");
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].name, "alice.bin");

        let unknown = service
            .owners_files_page(
                &snapshot,
                "unknown",
                &FilterSet::default(),
                Metric::Logical,
                SortSpec::default(),
                10,
                None,
            )
            .expect("unknown files");
        assert_eq!(unknown.rows.len(), 1);
        assert_eq!(unknown.rows[0].name, "shared.bin");

        let missing = service
            .owners_files_page(
                &snapshot,
                r"PC\Nobody",
                &FilterSet::default(),
                Metric::Logical,
                SortSpec::default(),
                10,
                None,
            )
            .expect_err("unknown owner");
        assert_eq!(missing.code, UiErrorCode::NotFound);
    }
}
