//! Media and executable metadata projection (US5, T111).
//!
//! Extraction is best effort in `freespace-analysis`: damaged or unsupported
//! containers never panic, and a readable file without tags yields an empty
//! record instead of an error. This module maps those outcomes onto the
//! interface contract:
//!
//! * a successful read produces [`MetadataView`] with every tag it found;
//! * an unsupported extension or container produces `E_UNSUPPORTED` with a
//!   localisable key, so the screen can render "no data" instead of a crash;
//! * a cancelled or oversized read stops before touching the file.
//!
//! Readers are header-bounded except for Portable Executables, which are loaded
//! whole; therefore PE images above [`MAX_PE_IMAGE_BYTES`] are refused before
//! extraction. The returned tag list is bounded by [`MAX_METADATA_TAGS`] and
//! [`MAX_METADATA_TAG_CHARS`].

use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use freespace_analysis::metadata::{self, MetadataKind, MetadataStatus};

use crate::{
    dto::{MAX_PE_IMAGE_BYTES, MetadataView, bounded_metadata_tags, metadata_source_key},
    error::{UiError, UiErrorCode},
    tree::TreeSnapshot,
};

/// Projects the metadata of one node.
///
/// # Errors
/// Returns `E_STALE_NODE` when the node is not part of `snapshot`,
/// `E_NOT_FOUND` when the file disappeared, `E_ACCESS_DENIED` when it cannot be
/// read, `E_CANCELLED` when `cancel` was requested, and `E_UNSUPPORTED` for an
/// unsupported, corrupted or oversized container.
pub fn metadata_view(snapshot: &TreeSnapshot, node_id: u32) -> Result<MetadataView, UiError> {
    metadata_view_cancellable(snapshot, node_id, None)
}

/// Same as [`metadata_view`] but observes a cooperative cancellation flag.
///
/// # Errors
/// See [`metadata_view`].
pub fn metadata_view_cancellable(
    snapshot: &TreeSnapshot,
    node_id: u32,
    cancel: Option<&AtomicBool>,
) -> Result<MetadataView, UiError> {
    let row = snapshot.node_row(node_id)?;
    if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        return Err(UiError::new(UiErrorCode::Cancelled));
    }
    if row.kind != crate::dto::NodeKind::File {
        return Err(
            UiError::new(UiErrorCode::Unsupported).with_details("metadata.unsupported.not_a_file")
        );
    }

    let path = Path::new(&row.path);
    let Some(kind) = MetadataKind::for_path(path) else {
        return Err(
            UiError::new(UiErrorCode::Unsupported).with_details("metadata.unsupported.extension")
        );
    };

    let metadata = std::fs::metadata(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => UiError::new(UiErrorCode::NotFound)
            .with_details(format!("metadata.not_found: {}", row.path)),
        std::io::ErrorKind::PermissionDenied => UiError::new(UiErrorCode::AccessDenied)
            .with_details(format!("metadata.access_denied: {}", row.path)),
        _ => UiError::new(UiErrorCode::Io).with_details(error.to_string()),
    })?;
    if !metadata.is_file() {
        return Err(
            UiError::new(UiErrorCode::Unsupported).with_details("metadata.unsupported.not_a_file")
        );
    }
    if matches!(kind, MetadataKind::Pe) && metadata.len() > MAX_PE_IMAGE_BYTES {
        return Err(UiError::new(UiErrorCode::Unsupported).with_details(format!(
            "metadata.unsupported.too_large: {} bytes exceeds {MAX_PE_IMAGE_BYTES}",
            metadata.len()
        )));
    }

    let record = metadata::extract(node_id, path);

    if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        return Err(UiError::new(UiErrorCode::Cancelled));
    }

    match record.status {
        MetadataStatus::Ok => {
            let source_key = metadata_source_key(record.kind.as_str());
            let (tags, truncated) = bounded_metadata_tags(&record.tags, &source_key);

            Ok(MetadataView {
                node_id: row.id,
                path: row.path,
                kind: record.kind.as_str().to_owned(),
                source_key,
                status: record.status.as_str().to_owned(),
                available: true,
                tags,
                truncated,
                size_bytes: row.logical_size,
                modified_at: row.modified_at,
            })
        }
        MetadataStatus::Unsupported => {
            Err(UiError::new(UiErrorCode::Unsupported)
                .with_details("metadata.unsupported.container"))
        }
        MetadataStatus::Corrupted => {
            Err(UiError::new(UiErrorCode::Unsupported).with_details(format!(
                "metadata.corrupt: {}",
                record
                    .error
                    .unwrap_or_else(|| "the container cannot be parsed".to_owned())
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use freespace_core::{
        model::{FileSystem, ScanMethod, ScanSession, Volume, VolumeStatus},
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::metadata_view_cancellable;
    use crate::error::UiErrorCode;
    use crate::tree::TreeSnapshot;

    fn snapshot(root: &std::path::Path) -> TreeSnapshot {
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
        for name in ["notes.txt", "broken.jpg"] {
            tree.add_node(
                Some(root_id),
                NodeSpec {
                    name: name.to_owned(),
                    kind: NodeKind::File,
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        }
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
        let session = ScanSession::new(
            volume.id,
            vec![root.to_path_buf()],
            ScanMethod::DirectoryWalk,
            Vec::new(),
        );

        TreeSnapshot::new(tree, 9, volume, Some(session))
    }

    fn fixture() -> (tempfile::TempDir, TreeSnapshot, u32, u32) {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("notes.txt"), b"not metadata").expect("text");
        std::fs::write(
            directory.path().join("broken.jpg"),
            b"\xFF\xD8\xFF\xE0not really a JPEG",
        )
        .expect("image");
        let snapshot = snapshot(directory.path());
        let text = snapshot
            .search_page(
                "notes",
                crate::dto::Metric::Logical,
                crate::dto::SortSpec::default(),
                &crate::dto::FilterSet::default(),
                10,
                None,
            )
            .expect("text node")
            .rows[0]
            .id;
        let image = snapshot
            .search_page(
                "broken",
                crate::dto::Metric::Logical,
                crate::dto::SortSpec::default(),
                &crate::dto::FilterSet::default(),
                10,
                None,
            )
            .expect("image node")
            .rows[0]
            .id;

        (directory, snapshot, text, image)
    }

    #[test]
    fn unsupported_and_corrupted_files_become_unsupported_errors() {
        let (_directory, snapshot, text, image) = fixture();

        let unsupported =
            metadata_view_cancellable(&snapshot, text, None).expect_err("unsupported");
        assert_eq!(unsupported.code, UiErrorCode::Unsupported);
        assert!(
            unsupported
                .details
                .as_deref()
                .is_some_and(|details| details.contains("metadata.unsupported.extension"))
        );

        let corrupted = metadata_view_cancellable(&snapshot, image, None).expect_err("corrupt");
        assert_eq!(corrupted.code, UiErrorCode::Unsupported);
        assert!(
            corrupted
                .details
                .as_deref()
                .is_some_and(|details| details.contains("metadata.corrupt"))
        );
    }

    #[test]
    fn stale_nodes_and_directories_are_reported_cleanly() {
        let (_directory, snapshot, _text, _image) = fixture();

        let stale = metadata_view_cancellable(&snapshot, 99_999, None).expect_err("stale");
        assert_eq!(stale.code, UiErrorCode::StaleNode);

        let root = snapshot.root_id().expect("root");
        let directory = metadata_view_cancellable(&snapshot, root, None).expect_err("directory");
        assert_eq!(directory.code, UiErrorCode::Unsupported);
    }

    #[test]
    fn cancellation_stops_before_extraction() {
        let (_directory, snapshot, text, _image) = fixture();
        let cancel = Arc::new(AtomicBool::new(true));
        cancel.store(true, Ordering::Relaxed);

        let error = metadata_view_cancellable(&snapshot, text, Some(cancel.as_ref()))
            .expect_err("cancelled");

        assert_eq!(error.code, UiErrorCode::Cancelled);
    }
}
