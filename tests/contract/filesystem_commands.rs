#![cfg(windows)]

//! Contract checks for the filesystem command family (US4, T095).
//!
//! Size details, long paths, rename and permissions are exercised through the
//! public `freespace-app` services exactly as the Tauri commands call them.
//! Every fixture lives in a temporary directory or in a synthetic tree; no real
//! user data is read or changed. Checks that need an elevated or administrative
//! token are gated on `FREESPACE_ADMIN_TESTS=1`.

use std::{fs, path::Path};

use freespace_app::{
    dto::{
        AttributeChangeSet, AttributeName, AttributesPreviewRequest, FilterSet,
        LongPathPageRequest, OperationKind, PermissionsChanges, PermissionsPreviewRequest,
        RenameCase, RenamePreviewRequest,
    },
    error::UiErrorCode,
    long_paths, operations, permissions,
    permissions::PermissionsService,
    rename::{self, RenameService},
    size_details,
    tree::TreeSnapshot,
};
use freespace_core::{
    model::{FileSystem, Volume, VolumeStatus},
    size::SizeMetrics,
    tree::{FsTree, NodeKind, NodeSpec},
};
use freespace_ops::Store;

fn volume() -> Volume {
    Volume {
        id: Volume::id_for_mount_point(Path::new(r"C:\")),
        mount_point: r"C:\".into(),
        label: "Contract".to_owned(),
        file_system: FileSystem::Ntfs,
        total_bytes: 1_000,
        free_bytes: 100,
        cluster_size: Some(4_096),
        scan_method: None,
        is_elevated: false,
        is_removable: false,
        status: VolumeStatus::Available,
    }
}

fn tree_with_file(path: &Path, size: SizeMetrics) -> TreeSnapshot {
    let mut tree = FsTree::new();
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: path
                    .parent()
                    .map(|parent| parent.to_string_lossy().into_owned())
                    .unwrap_or_else(|| r"C:\".to_owned()),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
    tree.add_node(
        Some(root),
        NodeSpec {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file.bin".to_owned()),
            kind: NodeKind::File,
            size,
            ..NodeSpec::default()
        },
    )
    .expect("file");
    tree.recompute_totals();

    TreeSnapshot::new(tree, 31, volume(), None)
}

/// Synthetic tree with two long paths and one short path.
fn long_path_tree(generation: u64) -> TreeSnapshot {
    let mut tree = FsTree::new();
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: r"C:\".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
    let mut parent = root;
    for index in 0..5 {
        parent = tree
            .add_node(
                Some(parent),
                NodeSpec {
                    name: format!("segment-{index}-{}", "x".repeat(50)),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("directory");
    }
    for (name, size) in [("payload.bin", 1_024), ("second.bin", 2_048)] {
        tree.add_node(
            Some(parent),
            NodeSpec {
                name: name.to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(size),
                ..NodeSpec::default()
            },
        )
        .expect("file");
    }
    tree.add_node(
        Some(root),
        NodeSpec {
            name: "short.txt".to_owned(),
            kind: NodeKind::File,
            size: SizeMetrics::file(8),
            ..NodeSpec::default()
        },
    )
    .expect("short");
    tree.recompute_totals();

    TreeSnapshot::new(tree, generation, volume(), None)
}

fn long_request(generation: u64, page_size: u16) -> LongPathPageRequest {
    LongPathPageRequest {
        generation,
        filter: FilterSet::default(),
        cursor: None,
        page_size,
    }
}

// ---------------------------------------------------------------------------
// T098 — size details
// ---------------------------------------------------------------------------

#[test]
fn size_details_reports_exact_sizes_and_rejects_stale_nodes() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("payload.bin");
    fs::write(&path, vec![0xAB; 8_192]).expect("write");
    let snapshot = tree_with_file(&path, SizeMetrics::file(8_192));

    let details = size_details::size_details(&snapshot, 1).expect("details");

    assert_eq!(details.logical_size, 8_192);
    assert!(details.allocated_size >= 8_192);
    assert_eq!(details.ads_size, 0);
    assert_eq!(details.hardlink_contribution, 0);
    assert_eq!(details.on_disk_size, details.allocated_size);
    assert_eq!(details.name, "payload.bin");
    assert_eq!(details.metric_label, size_details::METRIC_ON_DISK);
    assert!(!details.estimated);

    let stale = size_details::size_details(&snapshot, 999).expect_err("stale node");
    assert_eq!(stale.code, UiErrorCode::StaleNode);

    let live = size_details::inspect_path(&path).expect("live");
    assert_eq!(live.logical_size, 8_192);
    assert!(!live.estimated);

    let missing =
        size_details::inspect_path(&directory.path().join("gone.bin")).expect_err("missing");
    assert_eq!(missing.code, UiErrorCode::NotFound);
}

// ---------------------------------------------------------------------------
// T099 — long paths
// ---------------------------------------------------------------------------

#[test]
fn long_paths_page_pages_and_rejects_a_stale_generation() {
    let snapshot = long_path_tree(41);
    let first = long_paths::long_paths_page(&snapshot, &long_request(41, 1)).expect("page");

    assert_eq!(first.generation, 41);
    assert_eq!(first.rows.len(), 1);
    assert!(long_paths::is_long_path(&first.rows[0].path));
    assert_eq!(first.total_estimate, Some(3));
    let cursor = first.next_cursor.clone().expect("second page");

    let second = long_paths::long_paths_page(
        &snapshot,
        &LongPathPageRequest {
            cursor: Some(cursor.clone()),
            ..long_request(41, 1)
        },
    )
    .expect("second page");
    assert_eq!(second.rows.len(), 1);
    let cursor = second.next_cursor.clone().expect("third page");

    let third = long_paths::long_paths_page(
        &snapshot,
        &LongPathPageRequest {
            cursor: Some(cursor.clone()),
            ..long_request(41, 1)
        },
    )
    .expect("third page");
    assert_eq!(third.rows.len(), 1);
    assert!(long_paths::is_long_path(&third.rows[0].path));
    assert!(third.next_cursor.is_none());

    let mut names = vec![
        first.rows[0].name.clone(),
        second.rows[0].name.clone(),
        third.rows[0].name.clone(),
    ];
    names.sort();
    assert!(names.contains(&"payload.bin".to_owned()), "{names:?}");
    assert!(names.contains(&"second.bin".to_owned()), "{names:?}");

    let stale_generation =
        long_paths::long_paths_page(&snapshot, &long_request(42, 1)).expect_err("stale generation");
    assert_eq!(stale_generation.code, UiErrorCode::StaleCursor);

    let newer = long_path_tree(42);
    let stale_cursor = long_paths::long_paths_page(
        &newer,
        &LongPathPageRequest {
            cursor: Some(cursor),
            ..long_request(42, 1)
        },
    )
    .expect_err("stale cursor");
    assert_eq!(stale_cursor.code, UiErrorCode::StaleCursor);

    let invalid = long_paths::long_paths_page(
        &snapshot,
        &LongPathPageRequest {
            page_size: 0,
            ..long_request(41, 1)
        },
    )
    .expect_err("invalid page size");
    assert_eq!(invalid.code, UiErrorCode::Validation);
}

#[test]
fn long_paths_validate_accepts_verbatim_prefixes_beyond_max_path() {
    let long = format!(r"\\?\C:\{}", vec!["segment"; 40].join(r"\"));
    assert!(long.len() > 260);

    let validation = long_paths::validate(&long).expect("validate");
    assert!(validation.valid, "{:?}", validation.reason_key);
    assert!(
        validation
            .normalized
            .as_deref()
            .is_some_and(|path| path.starts_with(r"\\?\")),
        "normalized: {:?}",
        validation.normalized
    );
    assert!(
        validation
            .display
            .as_deref()
            .is_some_and(|path| path.starts_with(r"C:\")),
        "display: {:?}",
        validation.display
    );

    let relative = long_paths::validate(r"photos\IMG_0001.jpg").expect("inline rejection");
    assert!(!relative.valid);
    assert_eq!(
        relative.reason_key.as_deref(),
        Some("long_paths.invalid.relative")
    );

    assert_eq!(
        long_paths::validate("   ").expect_err("empty").code,
        UiErrorCode::Validation
    );
    assert_eq!(
        long_paths::validate("C:\\a\0b").expect_err("nul").code,
        UiErrorCode::Validation
    );
}

// ---------------------------------------------------------------------------
// T100 — rename
// ---------------------------------------------------------------------------

fn rename_request(scope: &Path, targets: &[&std::path::PathBuf]) -> RenamePreviewRequest {
    RenamePreviewRequest {
        targets: targets
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        regex: r"^IMG_(\d+)".to_owned(),
        replacement: "photo-$1".to_owned(),
        scope: Some(scope.to_string_lossy().into_owned()),
        case: RenameCase::Preserve,
    }
}

#[test]
fn rename_preview_applies_and_burns_its_token() {
    let directory = tempfile::tempdir().expect("temp dir");
    let first = directory.path().join("IMG_0001.jpg");
    let second = directory.path().join("IMG_0002.jpg");
    fs::write(&first, b"one").expect("first");
    fs::write(&second, b"two").expect("second");

    let service = RenameService::new();
    let request = rename_request(directory.path(), &[&first, &second]);
    let preview = service.preview(&request, 0).expect("preview");

    assert_eq!(preview.change_count, 2);
    assert_eq!(preview.total_size, 6);
    assert!(preview.conflicts.is_empty());
    assert!(!preview.requires_elevation);
    assert_eq!(preview.rows[0].old_name, "IMG_0001.jpg");
    assert_eq!(preview.rows[0].new_name, "photo-0001.jpg");
    assert!(!preview.rows[0].conflict);
    assert_eq!(preview.expires_at, 600_000);
    assert!(!preview.confirmation_token.is_empty());

    // Preview must not touch the objects.
    assert!(first.exists());

    let store = Store::open_in_memory().expect("store");
    let applied = service
        .apply(
            &preview.preview_id,
            &preview.confirmation_token,
            Some(&store),
            false,
            1,
        )
        .expect("apply");

    assert_eq!(applied.renamed, 2);
    assert!(applied.journal_entry_id.is_some());
    assert!(applied.undo_available);
    assert!(directory.path().join("photo-0001.jpg").exists());
    assert!(directory.path().join("photo-0002.jpg").exists());
    assert!(!first.exists());
    assert!(!second.exists());

    let journal = operations::journal_page(
        &store,
        &freespace_app::dto::JournalFilter::default(),
        10,
        None,
    )
    .expect("journal");
    assert_eq!(journal.rows.len(), 1);
    assert_eq!(journal.rows[0].kind, OperationKind::Rename);
    assert_eq!(journal.rows[0].targets_count, 2);

    let reused = service
        .apply(
            &preview.preview_id,
            &preview.confirmation_token,
            Some(&store),
            false,
            2,
        )
        .expect_err("reuse");
    assert_eq!(reused.code, UiErrorCode::PreviewExpired);
}

#[test]
fn rename_token_expires_and_nothing_is_renamed() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("IMG_0001.jpg");
    fs::write(&path, b"payload").expect("write");
    let service = RenameService::with_ttl(1_000);
    let preview = service
        .preview(&rename_request(directory.path(), &[&path]), 0)
        .expect("preview");

    let expired = service
        .apply(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            false,
            1_001,
        )
        .expect_err("expired");
    assert_eq!(expired.code, UiErrorCode::PreviewExpired);
    assert!(path.exists());
}

#[test]
fn rename_reports_conflicts_and_rejects_bad_input() {
    let directory = tempfile::tempdir().expect("temp dir");
    let first = directory.path().join("IMG_0001.jpg");
    fs::write(&first, b"one").expect("first");
    fs::write(directory.path().join("photo-0001.jpg"), b"two").expect("existing");
    let service = RenameService::new();

    let preview = service
        .preview(&rename_request(directory.path(), &[&first]), 0)
        .expect("preview");
    assert!(preview.rows[0].conflict);
    assert_eq!(preview.conflicts[0].reason_key, rename::CONFLICT_EXISTS);
    assert!(
        preview
            .warnings
            .contains(&"rename.warning.conflicts".to_owned())
    );
    let blocked = service
        .apply(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            false,
            1,
        )
        .expect_err("conflicts");
    assert_eq!(blocked.code, UiErrorCode::Validation);
    assert!(first.exists());

    let invalid_pattern = RenamePreviewRequest {
        regex: "[".to_owned(),
        ..rename_request(directory.path(), &[&first])
    };
    assert_eq!(
        service
            .preview(&invalid_pattern, 0)
            .expect_err("invalid regex")
            .code,
        UiErrorCode::Validation
    );

    let outside = tempfile::tempdir().expect("outside");
    let escaped = outside.path().join("IMG_0001.jpg");
    fs::write(&escaped, b"payload").expect("write");
    assert_eq!(
        service
            .preview(&rename_request(directory.path(), &[&escaped]), 0)
            .expect_err("outside")
            .code,
        UiErrorCode::AccessDenied
    );

    let empty = RenamePreviewRequest {
        targets: Vec::new(),
        ..rename_request(directory.path(), &[&first])
    };
    assert_eq!(
        service.preview(&empty, 0).expect_err("no targets").code,
        UiErrorCode::Validation
    );
}

#[test]
fn rename_applies_a_path_beyond_max_path_through_verbatim_prefixes() {
    let directory = tempfile::tempdir().expect("temp dir");
    let mut current = directory.path().to_path_buf();
    for index in 0..8 {
        current.push(format!("segment-{index}-{}", "x".repeat(40)));
        fs::create_dir_all(freespace_platform::paths::to_verbatim(&current).expect("verbatim"))
            .expect("mkdir");
    }
    let path = current.join("IMG_9999.bin");
    assert!(path.as_os_str().len() > 270);
    fs::write(
        freespace_platform::paths::to_verbatim(&path).expect("verbatim"),
        b"payload",
    )
    .expect("write");

    let service = RenameService::new();
    let request = RenamePreviewRequest {
        scope: None,
        ..rename_request(directory.path(), &[&path])
    };
    let preview = service.preview(&request, 0).expect("preview");
    assert_eq!(preview.change_count, 1);
    assert!(preview.conflicts.is_empty());

    let applied = service
        .apply(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            false,
            1,
        )
        .expect("apply");
    assert_eq!(applied.renamed, 1);

    let renamed = current.join("photo-9999.bin");
    assert!(
        freespace_platform::paths::to_verbatim(&renamed)
            .expect("verbatim")
            .exists()
    );
    assert!(
        !freespace_platform::paths::to_verbatim(&path)
            .expect("verbatim")
            .exists()
    );
}

#[test]
fn rename_refuses_a_protected_target_without_elevation() {
    let protected = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()),
    )
    .join("System32")
    .join("kernel32.dll");
    let service = RenameService::new();
    let preview = service
        .preview(
            &RenamePreviewRequest {
                regex: r"^kernel32".to_owned(),
                replacement: "kernel32-backup".to_owned(),
                scope: None,
                ..rename_request(&protected, &[&protected])
            },
            0,
        )
        .expect("preview");

    assert!(preview.requires_elevation);
    let refused = service
        .apply(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            false,
            1,
        )
        .expect_err("elevation");
    assert_eq!(refused.code, UiErrorCode::ElevationRequired);
    assert!(protected.exists(), "the refusal must not rename anything");
}

// ---------------------------------------------------------------------------
// T101 — permissions and attributes
// ---------------------------------------------------------------------------

fn attribute_request(path: &Path) -> PermissionsPreviewRequest {
    PermissionsPreviewRequest {
        path: path.to_string_lossy().into_owned(),
        changes: PermissionsChanges {
            attributes: Some(AttributeChangeSet {
                set: vec![AttributeName::Hidden],
                clear: vec![AttributeName::Archive],
            }),
            ..PermissionsChanges::default()
        },
    }
}

#[test]
fn permissions_preview_apply_and_elevation_gating() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("payload.bin");
    fs::write(&path, b"payload").expect("write");
    let text = path.to_string_lossy().into_owned();

    let view = permissions::permissions_view(&text).expect("view");
    let owner = view.owner.clone().expect("owner");
    assert!(
        view.owner_sid
            .as_deref()
            .is_some_and(|sid| sid.starts_with("S-1-"))
    );
    assert!(AttributeName::Archive.is_set(view.attribute_mask));
    assert!(!view.is_protected);

    // Preview shows an exact diff and changes nothing.
    let preview = permissions::preview(&attribute_request(&path), 0).expect("preview");
    assert_eq!(preview.attribute_changes.len(), 2);
    assert!(!preview.requires_elevation);
    let hidden = preview
        .attribute_changes
        .iter()
        .find(|row| row.attribute == AttributeName::Hidden)
        .expect("hidden");
    assert!(!hidden.before);
    assert!(hidden.after);
    let archive = preview
        .attribute_changes
        .iter()
        .find(|row| row.attribute == AttributeName::Archive)
        .expect("archive");
    assert!(archive.before);
    assert!(!archive.after);
    let mask = freespace_platform::security::get_attributes(&path).expect("attributes");
    assert_eq!(
        mask & freespace_ops::attributes::ARCHIVE,
        freespace_ops::attributes::ARCHIVE,
        "the preview must not modify the object"
    );

    let store = Store::open_in_memory().expect("store");
    let applied = permissions::apply(
        &preview.preview_id,
        &preview.confirmation_token,
        Some(&store),
        false,
        1,
    )
    .expect("apply");
    assert_eq!(applied.changed, 1);
    assert_eq!(applied.failed, 0);
    assert_eq!(applied.journal_entry_ids.len(), 1);
    let mask = freespace_platform::security::get_attributes(&path).expect("attributes");
    assert_eq!(
        mask & freespace_ops::attributes::HIDDEN,
        freespace_ops::attributes::HIDDEN
    );
    assert_eq!(mask & freespace_ops::attributes::ARCHIVE, 0);

    let reused = permissions::apply(
        &preview.preview_id,
        &preview.confirmation_token,
        Some(&store),
        false,
        2,
    )
    .expect_err("reuse");
    assert_eq!(reused.code, UiErrorCode::PreviewExpired);

    // Owner changes need elevation and must leave the object untouched.
    let owner_request = PermissionsPreviewRequest {
        path: text.clone(),
        changes: PermissionsChanges {
            owner: Some(owner.clone()),
            ..PermissionsChanges::default()
        },
    };
    let owner_preview = permissions::preview(&owner_request, 0).expect("owner preview");
    assert!(owner_preview.requires_elevation);
    assert_eq!(
        owner_preview.acl_action,
        Some(freespace_app::dto::PermissionsAclAction::Owner)
    );
    let refused = permissions::apply(
        &owner_preview.preview_id,
        &owner_preview.confirmation_token,
        Some(&store),
        false,
        1,
    )
    .expect_err("elevation");
    assert_eq!(refused.code, UiErrorCode::ElevationRequired);
    assert_eq!(
        permissions::permissions_view(&text).expect("view").owner,
        Some(owner)
    );

    // A missing copy source fails during the preview, before any change.
    let copy_preview = PermissionsPreviewRequest {
        path: text.clone(),
        changes: PermissionsChanges {
            copy_from: Some(
                directory
                    .path()
                    .join("missing-source.bin")
                    .to_string_lossy()
                    .into_owned(),
            ),
            ..PermissionsChanges::default()
        },
    };
    let error = permissions::preview(&copy_preview, 0).expect_err("missing source");
    assert_eq!(error.code, UiErrorCode::NotFound);
    let mask = freespace_platform::security::get_attributes(&path).expect("attributes");
    assert_eq!(
        mask & freespace_ops::attributes::HIDDEN,
        freespace_ops::attributes::HIDDEN,
        "a failed preview must leave the object untouched"
    );
}

#[test]
fn permissions_tokens_expire() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("payload.bin");
    fs::write(&path, b"payload").expect("write");
    let service = PermissionsService::with_ttl(1_000);
    let preview = service
        .preview(&attribute_request(&path), 0)
        .expect("preview");

    let expired = service
        .apply(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            false,
            1_001,
        )
        .expect_err("expired");
    assert_eq!(expired.code, UiErrorCode::PreviewExpired);
    let mask = freespace_platform::security::get_attributes(&path).expect("attributes");
    assert_eq!(
        mask & freespace_ops::attributes::ARCHIVE,
        freespace_ops::attributes::ARCHIVE
    );
}

#[test]
fn attributes_preview_is_a_shared_operation_preview_and_applies_once() {
    let directory = tempfile::tempdir().expect("temp dir");
    let first = directory.path().join("first.txt");
    let second = directory.path().join("second.txt");
    fs::write(&first, b"one").expect("first");
    fs::write(&second, b"two").expect("second");

    let request = AttributesPreviewRequest {
        targets: vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ],
        changes: AttributeChangeSet {
            set: vec![AttributeName::ReadOnly],
            clear: Vec::new(),
        },
    };
    let preview = permissions::attributes_preview(&request, 0).expect("preview");
    assert_eq!(preview.kind, OperationKind::Permissions);
    assert_eq!(preview.targets.len(), 2);
    assert_eq!(preview.total_size, 6);
    assert!(!preview.reversible);
    assert!(!preview.requires_elevation);
    assert_eq!(preview.expires_at, 600_000);

    let mask = freespace_platform::security::get_attributes(&first).expect("attributes");
    assert_eq!(
        mask & freespace_ops::attributes::READ_ONLY,
        0,
        "the preview must not modify the objects"
    );

    let store = Store::open_in_memory().expect("store");
    let applied = permissions::attributes_apply(
        &preview.preview_id,
        &preview.confirmation_token,
        Some(&store),
        false,
        1,
    )
    .expect("apply");
    assert_eq!(applied.changed, 2);
    assert_eq!(applied.failed, 0);
    assert_eq!(applied.journal_entry_ids.len(), 1);
    for path in [&first, &second] {
        let mask = freespace_platform::security::get_attributes(path).expect("attributes");
        assert_eq!(
            mask & freespace_ops::attributes::READ_ONLY,
            freespace_ops::attributes::READ_ONLY
        );
        freespace_platform::security::set_attributes(path, 0, freespace_ops::attributes::READ_ONLY)
            .expect("clear read-only");
    }

    let reused = permissions::attributes_apply(
        &preview.preview_id,
        &preview.confirmation_token,
        Some(&store),
        false,
        2,
    )
    .expect_err("reuse");
    assert_eq!(reused.code, UiErrorCode::PreviewExpired);

    let empty = AttributesPreviewRequest {
        targets: request.targets.clone(),
        changes: AttributeChangeSet::default(),
    };
    assert_eq!(
        permissions::attributes_preview(&empty, 0)
            .expect_err("empty change")
            .code,
        UiErrorCode::Validation
    );
}

#[test]
fn acl_reset_runs_only_with_administrative_tests() {
    if std::env::var("FREESPACE_ADMIN_TESTS").as_deref() != Ok("1") {
        return;
    }

    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("payload.bin");
    fs::write(&path, b"payload").expect("write");
    let service = PermissionsService::new();
    let preview = service
        .preview(
            &PermissionsPreviewRequest {
                path: path.to_string_lossy().into_owned(),
                changes: PermissionsChanges {
                    reset: true,
                    ..PermissionsChanges::default()
                },
            },
            0,
        )
        .expect("preview");
    assert_eq!(
        preview.acl_action,
        Some(freespace_app::dto::PermissionsAclAction::Reset)
    );

    let store = Store::open_in_memory().expect("store");
    let applied = service
        .apply(
            &preview.preview_id,
            &preview.confirmation_token,
            Some(&store),
            false,
            1,
        )
        .expect("apply");
    assert_eq!(applied.failed, 0);
}
