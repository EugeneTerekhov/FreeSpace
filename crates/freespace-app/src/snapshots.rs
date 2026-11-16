//! Stored `.fssnap` snapshots and their comparison.
//!
//! The container format and the diff algorithm live in `freespace-report`. This
//! service owns the application snapshot directory, validates identifiers so a
//! command can never escape it, caches comparisons under a short-lived task id
//! and serves the diff page by page.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

use freespace_core::Timestamp;
use freespace_report::{
    SnapshotPayload, SnapshotSettings, compare, diff::DiffEntry, from_tree,
    snapshot::SnapshotVolume,
};

use crate::{
    dto::{
        Metric, SnapshotCreateRequest, SnapshotCreated, SnapshotDiffPage, SnapshotDiffRow,
        SnapshotDiffSort, SnapshotDiffSortKey, SnapshotGrowth, SnapshotSummary, SortSpec,
    },
    error::{UiError, UiErrorCode},
    paging,
    tree::TreeSnapshot,
    validation::validate_page_size,
};
use uuid::Uuid;

/// Extension of a stored snapshot.
pub const SNAPSHOT_EXTENSION: &str = "fssnap";

/// Process-wide sequence that keeps identifiers unique inside one millisecond.
static SNAPSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Snapshot workspace rooted in the application data directory.
#[derive(Debug)]
pub struct SnapshotService {
    directory: PathBuf,
    comparisons: Mutex<HashMap<String, CachedComparison>>,
}

#[derive(Debug, Clone)]
struct CachedComparison {
    base: String,
    target: String,
    added: u64,
    removed: u64,
    changed: u64,
    total_growth_bytes: i64,
    rows: Vec<SnapshotDiffRow>,
    growth: Vec<SnapshotGrowth>,
}

impl SnapshotService {
    /// Creates a service for `directory`; the directory is created on first use.
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            comparisons: Mutex::new(HashMap::new()),
        }
    }

    /// Default directory: `<data>/snapshots`.
    #[must_use]
    pub fn default_dir(data_dir: &Path) -> PathBuf {
        data_dir.join("snapshots")
    }

    /// Snapshot directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Lists stored snapshots, newest first.
    ///
    /// # Errors
    /// Returns `E_IO` when the directory cannot be read.
    pub fn list(&self, volume_id: Option<&str>) -> Result<Vec<SnapshotSummary>, UiError> {
        let mut summaries = Vec::new();
        if !self.directory.exists() {
            return Ok(summaries);
        }
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some(SNAPSHOT_EXTENSION) {
                continue;
            }
            let Ok(payload) = SnapshotPayload::read(&path) else {
                continue;
            };
            if volume_id.is_some_and(|wanted| payload.volume.id != wanted) {
                continue;
            }
            let file_size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            summaries.push(SnapshotSummary {
                snapshot_id: self.id_of(&path),
                path: path.to_string_lossy().into_owned(),
                volume_id: payload.volume.id.clone(),
                mount_point: payload.volume.mount_point.clone(),
                created_at: payload.created_at.clone(),
                node_count: payload.node_count,
                file_size,
            });
        }
        summaries.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.snapshot_id.cmp(&left.snapshot_id))
        });

        Ok(summaries)
    }

    /// Creates a snapshot from a finished generation.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` when `path` does not describe the scanned volume or
    /// `E_IO` when the file cannot be written.
    pub fn create(
        &self,
        snapshot: &TreeSnapshot,
        request: &SnapshotCreateRequest,
    ) -> Result<SnapshotCreated, UiError> {
        if let Some(requested) = &request.path {
            let requested = crate::validation::validate_path(requested)?;
            let mount = PathBuf::from(snapshot.volume().mount_point.as_os_str());
            if !crate::paths_within(&mount, &requested) {
                return Err(UiError::validation(
                    "create a snapshot from the currently scanned volume",
                ));
            }
        }

        fs::create_dir_all(&self.directory)?;
        let sequence = SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let snapshot_id = format!("snap-{}-{sequence}", Timestamp::now().as_millis());
        let path = match &request.out {
            Some(out) => crate::validation::validate_path(out)?,
            None => self
                .directory
                .join(format!("{snapshot_id}.{SNAPSHOT_EXTENSION}")),
        };
        let payload = from_tree(
            snapshot.tree(),
            snapshot.volume(),
            &[],
            SnapshotSettings::default(),
            Timestamp::now(),
        )?;
        payload.write(&path)?;
        let node_count = payload.node_count;
        let written_path = path.to_string_lossy().into_owned();

        Ok(SnapshotCreated {
            task_id: Uuid::new_v4().to_string(),
            snapshot_id: self.id_of(&path),
            path: written_path,
            node_count,
        })
    }

    /// Compares two stored snapshots and caches the result under a task id.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for unknown identifiers, `E_UNSUPPORTED` when the
    /// snapshots describe different volumes and `E_IO` for damaged files.
    pub fn compare(&self, base_id: &str, target_id: &str) -> Result<String, UiError> {
        let base = self.load(base_id)?;
        let target = self.load(target_id)?;
        if base.volume.id != target.volume.id {
            return Err(UiError::new(UiErrorCode::Unsupported).with_details(format!(
                "snapshots are from different volumes: `{}` and `{}`",
                base.volume.mount_point, target.volume.mount_point
            )));
        }
        let diff = compare(&base, &target)?;
        let rows = diff
            .added
            .iter()
            .chain(diff.removed.iter())
            .chain(diff.changed.iter())
            .map(row_of)
            .collect();
        let growth = diff
            .growth_by_directory
            .iter()
            .map(|entry| SnapshotGrowth {
                path: entry.path.clone(),
                delta: entry.delta,
            })
            .collect();
        let task_id = Uuid::new_v4().to_string();
        self.lock().insert(
            task_id.clone(),
            CachedComparison {
                base: base_id.to_owned(),
                target: target_id.to_owned(),
                added: diff.added.len() as u64,
                removed: diff.removed.len() as u64,
                changed: diff.changed.len() as u64,
                total_growth_bytes: diff.total_growth_bytes,
                rows,
                growth,
            },
        );

        Ok(task_id)
    }

    /// Returns one page of a cached comparison.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task, `E_VALIDATION` for a bad page
    /// size and `E_STALE_CURSOR` for a cursor from another comparison.
    pub fn diff_page(
        &self,
        task_id: &str,
        sort: SnapshotDiffSort,
        page_size: u16,
        cursor: Option<&str>,
    ) -> Result<SnapshotDiffPage, UiError> {
        validate_page_size(page_size)?;
        let mut comparison = self.lock().get(task_id).cloned().ok_or_else(|| {
            UiError::new(UiErrorCode::NotFound).with_details("unknown comparison")
        })?;
        comparison
            .rows
            .sort_by(|left, right| compare_rows(left, right, sort));
        let window = page_window(comparison.rows.len(), page_size, cursor, sort, task_id)?;
        let rows = comparison.rows[window.start..window.end].to_vec();

        Ok(SnapshotDiffPage {
            task_id: task_id.to_owned(),
            cursor: cursor.map(str::to_owned),
            next_cursor: window.next_cursor,
            rows,
            total_estimate: comparison.rows.len() as u64,
            added: comparison.added,
            removed: comparison.removed,
            changed: comparison.changed,
            total_growth_bytes: comparison.total_growth_bytes,
            growth_by_directory: comparison.growth.clone(),
        })
    }

    /// Deletes one stored snapshot.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown identifier and `E_IO` on failure.
    pub fn delete(&self, snapshot_id: &str) -> Result<(), UiError> {
        let path = self.resolve(snapshot_id)?;
        if !path.exists() {
            return Err(UiError::new(UiErrorCode::NotFound));
        }
        fs::remove_file(path)?;
        self.lock().retain(|_, comparison| {
            comparison.base != snapshot_id && comparison.target != snapshot_id
        });

        Ok(())
    }

    /// Metadata of one stored snapshot.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown identifier and `E_IO` when it cannot
    /// be read.
    pub fn summary(&self, snapshot_id: &str) -> Result<SnapshotSummary, UiError> {
        let path = self.resolve(snapshot_id)?;
        let payload = self.load(snapshot_id)?;
        let file_size = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);

        Ok(SnapshotSummary {
            snapshot_id: self.id_of(&path),
            path: path.to_string_lossy().into_owned(),
            volume_id: payload.volume.id.clone(),
            mount_point: payload.volume.mount_point.clone(),
            created_at: payload.created_at,
            node_count: payload.node_count,
            file_size,
        })
    }

    /// Resolves an identifier to a file inside the snapshot directory.
    fn resolve(&self, snapshot_id: &str) -> Result<PathBuf, UiError> {
        if snapshot_id.is_empty()
            || snapshot_id.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|'])
        {
            return Err(UiError::validation("invalid snapshot identifier"));
        }
        let name = if snapshot_id.ends_with(&format!(".{SNAPSHOT_EXTENSION}")) {
            snapshot_id.to_owned()
        } else {
            format!("{snapshot_id}.{SNAPSHOT_EXTENSION}")
        };
        let path = self.directory.join(name);
        if !crate::paths_within(&self.directory, &path) {
            return Err(UiError::new(UiErrorCode::AccessDenied));
        }

        Ok(path)
    }

    fn load(&self, snapshot_id: &str) -> Result<SnapshotPayload, UiError> {
        let path = self.resolve(snapshot_id)?;
        if !path.exists() {
            return Err(UiError::new(UiErrorCode::NotFound));
        }
        SnapshotPayload::read(&path).map_err(UiError::from)
    }

    fn id_of(&self, path: &Path) -> String {
        path.file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, CachedComparison>> {
        self.comparisons
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn row_of(entry: &DiffEntry) -> SnapshotDiffRow {
    SnapshotDiffRow {
        path: entry.path.clone(),
        change: entry.change.as_str().to_owned(),
        is_directory: entry.is_directory,
        size_delta: entry.size_delta,
        mtime_delta: entry.mtime_delta,
        base_size: entry.base_size,
        target_size: entry.target_size,
    }
}

fn compare_rows(
    left: &SnapshotDiffRow,
    right: &SnapshotDiffRow,
    sort: SnapshotDiffSort,
) -> std::cmp::Ordering {
    let ordering = match sort.key {
        SnapshotDiffSortKey::Path => left.path.cmp(&right.path),
        SnapshotDiffSortKey::Change => left.change.cmp(&right.change),
        SnapshotDiffSortKey::SizeDelta => left
            .size_delta
            .unsigned_abs()
            .cmp(&right.size_delta.unsigned_abs()),
    };
    let ordering = if sort.descending {
        ordering.reverse()
    } else {
        ordering
    };

    ordering.then_with(|| left.path.cmp(&right.path))
}

fn page_window(
    total: usize,
    page_size: u16,
    cursor: Option<&str>,
    sort: SnapshotDiffSort,
    task_id: &str,
) -> Result<paging::PageWindow, UiError> {
    let spec = SortSpec {
        key: match sort.key {
            SnapshotDiffSortKey::Path => crate::dto::SortKey::Name,
            SnapshotDiffSortKey::Change => crate::dto::SortKey::Modified,
            SnapshotDiffSortKey::SizeDelta => crate::dto::SortKey::Size,
        },
        descending: sort.descending,
    };
    let generation = task_id
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    paging::page_window(total, page_size, cursor, generation, Metric::OnDisk, spec)
}

/// Volume metadata of a stored snapshot; used by the dashboard.
#[must_use]
pub fn volume_of(payload: &SnapshotPayload) -> SnapshotVolume {
    payload.volume.clone()
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        model::{FileSystem, Volume, VolumeStatus},
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::{SnapshotService, SnapshotSummary};
    use crate::dto::{SnapshotCreateRequest, SnapshotDiffSort, SnapshotDiffSortKey};
    use crate::error::UiErrorCode;
    use crate::tree::TreeSnapshot;

    fn snapshot(root: &std::path::Path, files: &[(&str, u64)]) -> TreeSnapshot {
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
        for (name, size) in files {
            tree.add_node(
                Some(root_id),
                NodeSpec {
                    name: (*name).to_owned(),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(*size),
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

        TreeSnapshot::new(tree, 1, volume, None)
    }

    fn create(service: &SnapshotService, snapshot: &TreeSnapshot) -> SnapshotSummary {
        let created = service
            .create(
                snapshot,
                &SnapshotCreateRequest {
                    path: None,
                    out: None,
                },
            )
            .expect("create");
        service.summary(&created.snapshot_id).expect("summary")
    }

    #[test]
    fn creates_lists_compares_and_deletes_snapshots() {
        let directory = tempfile::tempdir().expect("temp dir");
        let data = tempfile::tempdir().expect("data");
        let service = SnapshotService::new(SnapshotService::default_dir(data.path()));

        let base = create(
            &service,
            &snapshot(directory.path(), &[("keep.bin", 100), ("gone.bin", 50)]),
        );
        let target = create(
            &service,
            &snapshot(directory.path(), &[("keep.bin", 300), ("new.bin", 70)]),
        );

        let list = service.list(None).expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].snapshot_id, target.snapshot_id);

        let task = service
            .compare(&base.snapshot_id, &target.snapshot_id)
            .expect("compare");
        let page = service
            .diff_page(
                &task,
                SnapshotDiffSort {
                    key: SnapshotDiffSortKey::SizeDelta,
                    descending: true,
                },
                10,
                None,
            )
            .expect("page");
        assert_eq!(page.added, 1);
        assert_eq!(page.removed, 1);
        assert_eq!(page.changed, 1);
        assert_eq!(page.rows.len(), 3);
        assert_eq!(page.rows[0].path, "keep.bin", "largest delta first");

        service.delete(&target.snapshot_id).expect("delete");
        assert_eq!(service.list(None).expect("list").len(), 1);
        let error = service
            .diff_page(&task, SnapshotDiffSort::default(), 10, None)
            .expect_err("task is gone");
        assert_eq!(error.code, UiErrorCode::NotFound);
    }

    #[test]
    fn rejects_incompatible_volumes_and_paths() {
        let first = tempfile::tempdir().expect("first");
        let second = tempfile::tempdir().expect("second");
        let data = tempfile::tempdir().expect("data");
        let service = SnapshotService::new(SnapshotService::default_dir(data.path()));
        let a = create(&service, &snapshot(first.path(), &[("a.bin", 10)]));
        let b = create(&service, &snapshot(second.path(), &[("b.bin", 10)]));

        let error = service
            .compare(&a.snapshot_id, &b.snapshot_id)
            .expect_err("different volumes");
        assert_eq!(error.code, UiErrorCode::Unsupported);

        let error = service
            .delete("../escape")
            .expect_err("escapes the directory");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    #[test]
    fn create_requires_the_scanned_volume() {
        let scanned = tempfile::tempdir().expect("scanned");
        let other = tempfile::tempdir().expect("other");
        let service = SnapshotService::new(tempfile::tempdir().expect("data").path());
        let snapshot = snapshot(scanned.path(), &[("a.bin", 10)]);

        let error = service
            .create(
                &snapshot,
                &SnapshotCreateRequest {
                    path: Some(other.path().to_string_lossy().into_owned()),
                    out: None,
                },
            )
            .expect_err("other volume");
        assert_eq!(error.code, UiErrorCode::Validation);
    }
}
