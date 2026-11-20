//! Long paths beyond `MAX_PATH` (US4, T099).
//!
//! The screen lists the objects whose full path is longer than the legacy 260
//! character limit and validates user input before any operation touches the
//! file system. FreeSpace never relies on the system-wide `LongPathsEnabled`
//! policy: [`validate`] reports the canonical form accepted by every operation,
//! and the scanner has already recorded the same full paths. The concept is
//! Windows-specific (`long_paths` is `false` on Linux), but the listing helpers
//! remain platform-neutral so the shared table can render a snapshot from any
//! source.

use std::path::Path;

use freespace_core::tree::NodeId;
use freespace_platform::paths as platform_paths;

use crate::{
    dto::{LongPathPageRequest, LongPathValidation, Metric, SortSpec, TreePage},
    error::{UiError, UiErrorCode},
    paging,
    tree::TreeSnapshot,
    validation::MAX_PATH_CHARS,
};

/// Length above which a path needs a verbatim prefix, in UTF-16 units.
pub const LEGACY_MAX_PATH_UNITS: usize = 260;

/// Returns `true` when `path` is longer than the legacy limit.
#[must_use]
pub fn is_long_path(path: &str) -> bool {
    path.encode_utf16().count() > LEGACY_MAX_PATH_UNITS
}

/// Returns one page of the long paths of the current generation.
///
/// Rows use the same `TreePage` shape as the tree so the shared table can render
/// them; the cursor is bound to the generation, metric and ordering exactly like
/// every other page.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed request and `E_STALE_CURSOR` when the
/// cursor belongs to another generation or ordering.
pub fn long_paths_page(
    snapshot: &TreeSnapshot,
    request: &LongPathPageRequest,
) -> Result<TreePage, UiError> {
    request.validate()?;
    if request.generation != snapshot.generation() {
        return Err(UiError::new(UiErrorCode::StaleCursor)
            .with_details("the requested generation is no longer current"));
    }

    let mut ids = Vec::new();
    for index in 0..snapshot.tree().len() {
        let Ok(id) = NodeId::try_from(index) else {
            break;
        };
        if !is_long_path(&snapshot.tree().path(id).to_string_lossy()) {
            continue;
        }
        if !snapshot.matches_filter(id, &request.filter) {
            continue;
        }
        ids.push(id);
    }

    let metric = Metric::OnDisk;
    let sort = SortSpec::default();
    ids.sort_by(|left, right| {
        let left_size = metric.of(snapshot.tree().node(*left).size);
        let right_size = metric.of(snapshot.tree().node(*right).size);
        right_size.cmp(&left_size).then(left.cmp(right))
    });

    let window = paging::page_window(
        ids.len(),
        request.page_size,
        request.cursor.as_deref(),
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
        cursor: request.cursor.clone(),
        next_cursor: window.next_cursor,
        rows,
        total_estimate: Some(ids.len() as u64),
        metric,
        sort,
        stale_cursor: false,
    })
}

/// Validates a user-supplied path for a long-path operation.
///
/// Verbatim-prefixed input is accepted and reported in its canonical form, so a
/// path such as `\\?\C:\a\..\b` normalises to `\\?\C:\b` before it reaches
/// `freespace-ops`.
///
/// # Errors
/// Returns `E_VALIDATION` for an empty, NUL-containing or over-long path: those
/// can never be repaired by editing one character.
pub fn validate(text: &str) -> Result<LongPathValidation, UiError> {
    if text.trim().is_empty() {
        return Err(UiError::validation("path is empty"));
    }
    if text.contains('\0') {
        return Err(UiError::validation("path contains a NUL character"));
    }
    if text.chars().count() > MAX_PATH_CHARS {
        return Err(UiError::validation(format!(
            "path exceeds {MAX_PATH_CHARS} characters"
        )));
    }

    let path = Path::new(text);
    if !path.is_absolute() {
        return Ok(LongPathValidation {
            valid: false,
            reason_key: Some("long_paths.invalid.relative".to_owned()),
            normalized: None,
            display: None,
        });
    }

    match platform_paths::normalize(path) {
        Ok(normalized) => Ok(LongPathValidation {
            valid: true,
            reason_key: None,
            normalized: Some(normalized.to_string_lossy().into_owned()),
            display: Some(platform_paths::display(&normalized)),
        }),
        Err(_) => Ok(LongPathValidation {
            valid: false,
            reason_key: Some("long_paths.invalid.path".to_owned()),
            normalized: None,
            display: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        model::{FileSystem, Volume, VolumeStatus},
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::{LEGACY_MAX_PATH_UNITS, is_long_path, long_paths_page, validate};
    use crate::{
        dto::{FilterSet, LongPathPageRequest},
        error::UiErrorCode,
        tree::TreeSnapshot,
    };

    fn volume() -> Volume {
        Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(r"C:\")),
            mount_point: r"C:\".into(),
            label: "Test".to_owned(),
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

    fn long_snapshot() -> (TreeSnapshot, Vec<String>) {
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
        let mut parent = root;
        let mut segments = vec!["root".to_owned()];
        for index in 0..5 {
            let name = format!("segment-{index}-{}", "x".repeat(50));
            segments.push(name.clone());
            parent = tree
                .add_node(
                    Some(parent),
                    NodeSpec {
                        name,
                        kind: NodeKind::Directory,
                        ..NodeSpec::default()
                    },
                )
                .expect("directory");
        }
        let mut file_segments = segments.clone();
        file_segments.push("payload.bin".to_owned());
        tree.add_node(
            Some(parent),
            NodeSpec {
                name: "payload.bin".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(1_024),
                ..NodeSpec::default()
            },
        )
        .expect("file");
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

        (TreeSnapshot::new(tree, 9, volume(), None), file_segments)
    }

    #[test]
    fn measures_paths_in_utf16_units() {
        assert!(!is_long_path(&"x".repeat(LEGACY_MAX_PATH_UNITS)));
        assert!(is_long_path(&"x".repeat(LEGACY_MAX_PATH_UNITS + 1)));
        let cyrillic = "ж".repeat(LEGACY_MAX_PATH_UNITS + 1);
        assert!(is_long_path(&cyrillic));
    }

    #[test]
    fn lists_only_long_paths() {
        let (snapshot, segments) = long_snapshot();
        let full = segments.join("\\");
        assert!(is_long_path(&full), "fixture path must exceed MAX_PATH");

        let page = long_paths_page(
            &snapshot,
            &LongPathPageRequest {
                generation: 9,
                filter: FilterSet::default(),
                cursor: None,
                page_size: 10,
            },
        )
        .expect("page");

        assert_eq!(page.generation, 9);
        assert_eq!(page.rows.len(), 2);
        let names: Vec<&str> = page.rows.iter().map(|row| row.name.as_str()).collect();
        assert!(names.contains(&"payload.bin"), "{names:?}");
        assert!(
            names.iter().any(|name| name.starts_with("segment-4-")),
            "{names:?}"
        );
        assert!(page.rows.iter().all(|row| is_long_path(&row.path)));
        assert!(page.next_cursor.is_none());
        assert_eq!(page.total_estimate, Some(2));
    }

    #[test]
    fn paginates_with_a_cursor_bound_to_the_generation() {
        let (snapshot, _segments) = long_snapshot();
        let request = LongPathPageRequest {
            generation: 9,
            filter: FilterSet::default(),
            cursor: None,
            page_size: 1,
        };
        let first = long_paths_page(&snapshot, &request).expect("first");
        assert_eq!(first.rows.len(), 1);

        let stale = LongPathPageRequest {
            generation: 10,
            ..request.clone()
        };
        let error = long_paths_page(&snapshot, &stale).expect_err("stale generation");
        assert_eq!(error.code, UiErrorCode::StaleCursor);

        let second = long_paths_page(
            &snapshot,
            &LongPathPageRequest {
                generation: 9,
                cursor: first.next_cursor.clone(),
                ..request
            },
        )
        .expect("second");
        assert_eq!(second.rows.len(), 1);
        assert!(second.next_cursor.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn validates_an_absolute_unix_path() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, b"payload").expect("file");

        let validation = validate(&path.to_string_lossy()).expect("validate");

        assert!(validation.valid);
        assert!(validation.reason_key.is_none());
        assert!(validation.display.is_some());
    }

    #[test]
    fn rejects_a_page_size_outside_the_contract() {
        let (snapshot, _segments) = long_snapshot();
        let error = long_paths_page(
            &snapshot,
            &LongPathPageRequest {
                generation: 9,
                filter: FilterSet::default(),
                cursor: None,
                page_size: 0,
            },
        )
        .expect_err("invalid page size");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    #[cfg(windows)]
    #[test]
    fn accepts_verbatim_input_and_reports_the_canonical_form() {
        let validation = validate(r"\\?\C:\very\long\path\..\file.bin").expect("validate");

        assert!(validation.valid);
        assert!(validation.reason_key.is_none());
        assert_eq!(
            validation.normalized.as_deref(),
            Some(r"\\?\C:\very\long\file.bin")
        );
        assert_eq!(
            validation.display.as_deref(),
            Some(r"C:\very\long\file.bin")
        );
    }

    #[cfg(windows)]
    #[test]
    fn accepts_a_path_longer_than_max_path() {
        let long = format!(r"C:\{}", vec!["segment"; 40].join(r"\"));
        assert!(is_long_path(&long));
        let validation = validate(&long).expect("validate");

        assert!(validation.valid);
        assert!(
            validation
                .normalized
                .expect("normalized")
                .starts_with(r"\\?\")
        );
    }

    #[test]
    fn rejects_unrepairable_inputs() {
        assert_eq!(
            validate("   ").expect_err("empty").code,
            UiErrorCode::Validation
        );
        assert_eq!(
            validate("C:\\a\0b").expect_err("nul").code,
            UiErrorCode::Validation
        );
        assert_eq!(
            validate(&"x".repeat(crate::validation::MAX_PATH_CHARS + 1))
                .expect_err("too long")
                .code,
            UiErrorCode::Validation
        );

        let relative = validate("folder\\file.bin").expect("inline rejection");
        assert!(!relative.valid);
        assert_eq!(
            relative.reason_key.as_deref(),
            Some("long_paths.invalid.relative")
        );
        assert!(relative.normalized.is_none());
    }
}
