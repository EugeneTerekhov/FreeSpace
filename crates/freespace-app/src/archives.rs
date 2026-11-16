//! Archive entries and list export without unpacking (US5, T113).
//!
//! Archiving containers are only listed: `freespace-analysis` reads central
//! directories and headers, never extracts payloads to disk. Password-protected
//! and damaged containers are reported as `E_UNSUPPORTED` with a stable key, so
//! the screen shows a clear message instead of an I/O failure.
//!
//! Cursors are bound to the archive fingerprint and to the filter: replacing the
//! file or changing the filter makes the previous cursor stale.

use std::{path::Path, time::UNIX_EPOCH};

use freespace_analysis::{ArchiveEntry, ArchiveInspection, ArchiveLimits, inspect};
use freespace_core::ErrorCode;

use crate::{
    dto::{
        ArchiveEntryPage, ArchiveEntryView, ExportFormat, ExportResult, FilterSet, Metric, SortKey,
        SortSpec, inspection::scoped_generation,
    },
    error::{UiError, UiErrorCode},
    export::validate_output_path,
    paging,
    validation::{validate_filter, validate_page_size, validate_path},
};

/// One page of entries of an archive.
///
/// The archive is listed on every call, so an entry page never keeps a file
/// handle open. The cursor is validated against the archive's size and
/// modification time.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed path, filter or page size,
/// `E_NOT_FOUND` when the archive is missing, `E_ACCESS_DENIED` when it cannot
/// be read, `E_UNSUPPORTED` for unsupported, password-protected or damaged
/// containers, and `E_STALE_CURSOR` for a cursor from another request.
pub fn entries_page(
    path: &Path,
    filter: &FilterSet,
    page_size: u16,
    cursor: Option<&str>,
) -> Result<ArchiveEntryPage, UiError> {
    validate_filter(filter)?;
    validate_page_size(page_size)?;
    let path = validate_path(&path.to_string_lossy())?;
    let metadata = std::fs::metadata(&path).map_err(io_error)?;
    if !metadata.is_file() {
        return Err(UiError::validation("the archive path must be a file"));
    }

    let inspection = inspect_archive(&path)?;
    let generation = fingerprint(&metadata);
    page_inspection(&path, &inspection, generation, filter, page_size, cursor)
}

/// Exports only the content list of one archive.
///
/// The archive itself is never extracted. `format` decides the writer and must
/// match the extension of `out`.
///
/// # Errors
/// Returns `E_VALIDATION` for an invalid output path or a format/extension
/// mismatch, `E_UNSUPPORTED` for containers or formats that cannot be listed,
/// `E_NOT_FOUND` for a missing archive and `E_IO` when the file cannot be
/// written.
pub fn export_list(path: &Path, format: ExportFormat, out: &Path) -> Result<ExportResult, UiError> {
    if !format.supports_archive_list() {
        return Err(UiError::new(UiErrorCode::Unsupported)
            .with_details("archive.unsupported.format: use csv, json, xml or html"));
    }
    ensure_extension(out, format)?;

    let path = validate_path(&path.to_string_lossy())?;
    let inspection = inspect_archive(&path)?;

    write_inspection_list(&inspection, format, out)
}

fn inspect_archive(path: &Path) -> Result<ArchiveInspection, UiError> {
    let inspection = inspect(path, ArchiveLimits::default()).map_err(map_archive_error)?;
    refuse_encrypted(&inspection)?;

    Ok(inspection)
}

fn refuse_encrypted(inspection: &ArchiveInspection) -> Result<(), UiError> {
    if inspection.encrypted_count > 0 {
        return Err(UiError::new(UiErrorCode::Unsupported).with_details(format!(
            "archive.unsupported.password: {} entries are encrypted",
            inspection.encrypted_count
        )));
    }

    Ok(())
}

fn page_inspection(
    path: &Path,
    inspection: &ArchiveInspection,
    generation: u64,
    filter: &FilterSet,
    page_size: u16,
    cursor: Option<&str>,
) -> Result<ArchiveEntryPage, UiError> {
    let mut entries: Vec<&ArchiveEntry> = inspection
        .entries
        .iter()
        .filter(|entry| entry_matches(entry, filter))
        .collect();
    entries.sort_by(|left, right| {
        right
            .size
            .cmp(&left.size)
            .then_with(|| left.internal_path.cmp(&right.internal_path))
    });

    let sort = SortSpec {
        key: SortKey::Size,
        descending: true,
    };
    let scope = format!("archives|{}", filter_scope(filter));
    let window = paging::page_window(
        entries.len(),
        page_size,
        cursor,
        scoped_generation(generation, &scope),
        Metric::Logical,
        sort,
    )?;

    let rows = entries[window.start..window.end]
        .iter()
        .map(|entry| entry_view(entry))
        .collect();

    Ok(ArchiveEntryPage {
        path: path.to_string_lossy().into_owned(),
        format: inspection.format.as_str().to_owned(),
        cursor: cursor.map(str::to_owned),
        next_cursor: window.next_cursor,
        entries: rows,
        total_entries: u64::try_from(inspection.entries.len()).unwrap_or(u64::MAX),
        total_files: inspection.file_count(),
        total_size: inspection.total_size,
        encrypted_count: inspection.encrypted_count,
        warnings: inspection.warnings.clone(),
        stale_cursor: false,
    })
}

fn write_inspection_list(
    inspection: &ArchiveInspection,
    format: ExportFormat,
    out: &Path,
) -> Result<ExportResult, UiError> {
    if !format.supports_archive_list() {
        return Err(
            UiError::new(UiErrorCode::Unsupported).with_details("archive.unsupported.format")
        );
    }
    ensure_extension(out, format)?;
    let output = validate_output_path(out)?;

    freespace_report::archive_list::write(std::slice::from_ref(inspection), &output)
        .map_err(map_archive_error)?;

    let bytes_written = std::fs::metadata(&output)
        .map(|metadata| metadata.len())
        .unwrap_or_default();

    Ok(ExportResult {
        path: output.to_string_lossy().into_owned(),
        format,
        bytes_written,
        rows_written: u64::try_from(inspection.entries.len()).unwrap_or(u64::MAX),
        sections: Vec::new(),
        warnings: inspection.warnings.clone(),
    })
}

fn entry_view(entry: &ArchiveEntry) -> ArchiveEntryView {
    let depth = u8::try_from(entry.internal_path.matches('/').count()).unwrap_or(u8::MAX);
    let extension = (!entry.is_directory)
        .then(|| {
            entry
                .internal_path
                .rsplit_once('/')
                .map_or(entry.internal_path.as_str(), |(_, name)| name)
                .rsplit_once('.')
                .map(|(_, extension)| extension.to_ascii_lowercase())
        })
        .flatten()
        .filter(|extension| !extension.is_empty());

    ArchiveEntryView {
        internal_path: entry.internal_path.clone(),
        size: entry.size,
        compressed_size: entry.compressed_size,
        is_directory: entry.is_directory,
        encrypted: entry.encrypted,
        share: entry.share,
        depth,
        extension,
    }
}

fn entry_matches(entry: &ArchiveEntry, filter: &FilterSet) -> bool {
    let text = filter.text.trim().to_lowercase();
    if !text.is_empty() && !entry.internal_path.to_lowercase().contains(&text) {
        return false;
    }
    if !filter.extensions.is_empty() {
        if entry.is_directory {
            return false;
        }
        let name = entry
            .internal_path
            .rsplit_once('/')
            .map_or(entry.internal_path.as_str(), |(_, name)| name);
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
    if filter.min_size.is_some_and(|minimum| entry.size < minimum) {
        return false;
    }
    if filter.max_size.is_some_and(|maximum| entry.size > maximum) {
        return false;
    }
    if !filter.exclude_paths.is_empty() {
        let internal = entry.internal_path.replace('\\', "/").to_lowercase();
        let excluded = filter.exclude_paths.iter().any(|candidate| {
            let candidate = candidate
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_lowercase();
            !candidate.is_empty()
                && (internal == candidate || internal.starts_with(&format!("{candidate}/")))
        });
        if excluded {
            return false;
        }
    }

    true
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

fn ensure_extension(out: &Path, format: ExportFormat) -> Result<(), UiError> {
    let extension = out
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let expected = format.extension();
    let matches = extension == expected || (format == ExportFormat::Html && extension == "htm");
    if matches {
        Ok(())
    } else {
        Err(UiError::validation(format!(
            "archive.error.extension: expected `.{expected}`, got `.{extension}`"
        )))
    }
}

fn fingerprint(metadata: &std::fs::Metadata) -> u64 {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        });
    let mixed = metadata.len().rotate_left(13) ^ modified;
    if mixed == 0 { 1 } else { mixed }
}

fn io_error(error: std::io::Error) -> UiError {
    match error.kind() {
        std::io::ErrorKind::NotFound => {
            UiError::new(UiErrorCode::NotFound).with_details(error.to_string())
        }
        std::io::ErrorKind::PermissionDenied => {
            UiError::new(UiErrorCode::AccessDenied).with_details(error.to_string())
        }
        _ => UiError::new(UiErrorCode::Io).with_details(error.to_string()),
    }
}

fn map_archive_error(error: freespace_core::Error) -> UiError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    let key = if lower.contains("password") || lower.contains("encrypt") {
        "archive.unsupported.password"
    } else if lower.contains("bomb") || lower.contains("larger than") || lower.contains("limit") {
        "archive.unsupported.limits"
    } else if lower.contains("corrupt")
        || lower.contains("damaged")
        || lower.contains("invalid")
        || lower.contains("cannot read")
    {
        "archive.unsupported.corrupt"
    } else {
        "archive.unsupported.container"
    };

    let code = match error.code() {
        ErrorCode::AccessDenied | ErrorCode::ProtectedPath => UiErrorCode::AccessDenied,
        ErrorCode::NotFound => UiErrorCode::NotFound,
        ErrorCode::InvalidArgument | ErrorCode::InvalidPath => UiErrorCode::Validation,
        _ => UiErrorCode::Unsupported,
    };

    UiError::new(code).with_details(format!("{key}: {message}"))
}

#[cfg(test)]
mod tests {
    use freespace_analysis::{ArchiveEntry, ArchiveFormat, ArchiveInspection};

    use super::{entry_matches, page_inspection, refuse_encrypted, write_inspection_list};
    use crate::dto::{ExportFormat, FilterSet};
    use crate::error::UiErrorCode;

    fn inspection() -> ArchiveInspection {
        ArchiveInspection {
            archive: "sample.zip".into(),
            format: ArchiveFormat::Zip,
            entries: vec![
                ArchiveEntry::file("docs/readme.txt", 10, Some(5)),
                ArchiveEntry::file("payload.bin", 128, Some(64)),
                ArchiveEntry::directory("docs/"),
                ArchiveEntry::file("docs/notes.txt", 32, Some(16)),
                ArchiveEntry::file("image.jpg", 64, Some(64)),
            ],
            total_size: 234,
            encrypted_count: 0,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn pages_entries_and_binds_cursors_to_the_filter() {
        let inspection = inspection();
        let path = std::path::Path::new("sample.zip");

        let first =
            page_inspection(path, &inspection, 7, &FilterSet::default(), 2, None).expect("first");
        assert_eq!(first.entries.len(), 2);
        assert_eq!(first.entries[0].internal_path, "payload.bin");
        assert_eq!(first.entries[1].internal_path, "image.jpg");
        assert_eq!(first.total_entries, 5);
        assert_eq!(first.total_files, 4);
        assert!(first.next_cursor.is_some());

        let second = page_inspection(
            path,
            &inspection,
            7,
            &FilterSet::default(),
            2,
            first.next_cursor.as_deref(),
        )
        .expect("second");
        assert_eq!(second.entries.len(), 2);

        let filtered = FilterSet {
            extensions: vec!["txt".to_owned()],
            ..FilterSet::default()
        };
        let stale = page_inspection(
            path,
            &inspection,
            7,
            &filtered,
            2,
            first.next_cursor.as_deref(),
        )
        .expect_err("cursor from another filter");
        assert_eq!(stale.code, UiErrorCode::StaleCursor);

        assert!(entry_matches(
            &inspection.entries[0],
            &FilterSet {
                text: "readme".to_owned(),
                ..FilterSet::default()
            }
        ));
    }

    #[test]
    fn exports_every_list_format_without_unpacking() {
        let directory = tempfile::tempdir().expect("temp dir");
        let inspection = inspection();

        for (format, extension) in [
            (ExportFormat::Csv, "csv"),
            (ExportFormat::Json, "json"),
            (ExportFormat::Xml, "xml"),
            (ExportFormat::Html, "html"),
        ] {
            let out = directory.path().join(format!("list.{extension}"));
            let result = write_inspection_list(&inspection, format, &out).expect("export");
            let text = std::fs::read_to_string(&out).expect("read");
            assert!(text.contains("payload.bin"), "{extension}");
            assert_eq!(result.rows_written, 5);
            assert!(result.bytes_written > 0);
            assert_eq!(result.format, format);
        }

        let error = write_inspection_list(
            &inspection,
            ExportFormat::Xlsx,
            &directory.path().join("list.xlsx"),
        )
        .expect_err("xlsx is not an archive-list format");
        assert_eq!(error.code, UiErrorCode::Unsupported);
    }

    #[test]
    fn rejects_a_mismatched_extension() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = write_inspection_list(
            &inspection(),
            ExportFormat::Csv,
            &directory.path().join("list.xml"),
        )
        .expect_err("mismatch");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    #[test]
    fn refuses_password_protected_archives_with_a_key() {
        let mut encrypted = inspection();
        encrypted.encrypted_count = 1;

        let error = refuse_encrypted(&encrypted).expect_err("encrypted");
        assert_eq!(error.code, UiErrorCode::Unsupported);
        assert!(
            error
                .details
                .as_deref()
                .is_some_and(|details| details.contains("archive.unsupported.password"))
        );
        assert!(refuse_encrypted(&inspection()).is_ok());
    }

    #[test]
    fn reports_unsupported_containers_with_a_key() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("notes.txt");
        std::fs::write(&path, b"plain text").expect("write");

        let error =
            super::entries_page(&path, &FilterSet::default(), 10, None).expect_err("unsupported");
        assert_eq!(error.code, UiErrorCode::Unsupported);
        assert!(
            error
                .details
                .as_deref()
                .is_some_and(|details| details.contains("archive.unsupported."))
        );
    }
}
