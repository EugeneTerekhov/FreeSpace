//! Validation of every value that crosses the IPC boundary.
//!
//! Commands validate their input before touching the file system or the tree.
//! The limits mirror `data-model.md` §5 and the IPC contract so that the same
//! rules are tested once and reused by every command.

use std::path::PathBuf;

use crate::{
    dto::{FilterSet, MAX_PAGE_SIZE, ScanRequest, SortSpec, UiSettings, WindowState},
    error::UiError,
};

/// Maximum length of a full path, in characters.
pub const MAX_PATH_CHARS: usize = 32_000;
/// Maximum length of a name, in characters.
pub const MAX_NAME_CHARS: usize = 32_000;
/// Maximum length of a free-text filter.
pub const MAX_FILTER_TEXT_CHARS: usize = 256;
/// Maximum length of an opaque cursor.
pub const MAX_CURSOR_CHARS: usize = 512;
/// Maximum length of an exclusion pattern.
pub const MAX_EXCLUSION_CHARS: usize = 512;
/// Maximum number of exclusions in a request.
pub const MAX_EXCLUSIONS: usize = 64;
/// Maximum number of saved filters per screen.
pub const MAX_SAVED_FILTERS: usize = 20;
/// Maximum length of a saved filter name.
pub const MAX_FILTER_NAME_CHARS: usize = 64;
/// Maximum window width or height.
pub const MAX_WINDOW_DIMENSION: u32 = 16_384;
/// Smallest supported UI schema version.
pub const UI_SCHEMA_VERSION: u16 = 1;

/// Rejects an empty, NUL-containing or relative path.
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_path(text: &str) -> Result<PathBuf, UiError> {
    if text.trim().is_empty() {
        return Err(UiError::validation("path is empty"));
    }
    if text.chars().count() > MAX_PATH_CHARS {
        return Err(UiError::validation(format!(
            "path exceeds {MAX_PATH_CHARS} characters"
        )));
    }
    if text.contains('\0') {
        return Err(UiError::validation("path contains a NUL character"));
    }

    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err(UiError::validation("path must be absolute"));
    }

    Ok(path)
}

/// Validates a display name.
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_name(text: &str) -> Result<(), UiError> {
    if text.is_empty() {
        return Err(UiError::validation("name is empty"));
    }
    if text.chars().count() > MAX_NAME_CHARS {
        return Err(UiError::validation(format!(
            "name exceeds {MAX_NAME_CHARS} characters"
        )));
    }
    if text.contains('\0') {
        return Err(UiError::validation("name contains a NUL character"));
    }
    if text.contains(['\\', '/']) {
        return Err(UiError::validation("name must not contain path separators"));
    }

    Ok(())
}

/// Validates a free-text filter.
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_filter_text(text: &str) -> Result<(), UiError> {
    if text.chars().count() > MAX_FILTER_TEXT_CHARS {
        return Err(UiError::validation(format!(
            "filter text exceeds {MAX_FILTER_TEXT_CHARS} characters"
        )));
    }
    if text.contains('\0') {
        return Err(UiError::validation("filter text contains a NUL character"));
    }

    Ok(())
}

/// Validates a page size and returns it.
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_page_size(page_size: u16) -> Result<u16, UiError> {
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        return Err(UiError::validation(format!(
            "page_size must be between 1 and {MAX_PAGE_SIZE}"
        )));
    }

    Ok(page_size)
}

/// Validates an opaque cursor before it is decoded.
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_cursor(cursor: &str) -> Result<(), UiError> {
    if cursor.is_empty() || cursor.chars().count() > MAX_CURSOR_CHARS {
        return Err(UiError::validation(format!(
            "cursor must be between 1 and {MAX_CURSOR_CHARS} characters"
        )));
    }
    if cursor.chars().any(char::is_control) {
        return Err(UiError::validation("cursor contains control characters"));
    }

    Ok(())
}

/// Validates every field of a [`FilterSet`].
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_filter(filter: &FilterSet) -> Result<(), UiError> {
    validate_filter_text(&filter.text)?;

    if let Some(name) = &filter.name {
        if name.chars().count() > MAX_FILTER_NAME_CHARS {
            return Err(UiError::validation(format!(
                "filter name exceeds {MAX_FILTER_NAME_CHARS} characters"
            )));
        }
    }

    if let (Some(min), Some(max)) = (filter.min_size, filter.max_size) {
        if min > max {
            return Err(UiError::validation("min_size must not exceed max_size"));
        }
    }
    if let (Some(after), Some(before)) = (filter.modified_after, filter.modified_before) {
        if after > before {
            return Err(UiError::validation(
                "modified_after must not be later than modified_before",
            ));
        }
    }

    for extension in &filter.extensions {
        let extension = extension.strip_prefix('.').unwrap_or(extension);
        if extension.is_empty() || extension.len() > 32 {
            return Err(UiError::validation("extension must be 1..=32 characters"));
        }
        if !extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '-'))
        {
            return Err(UiError::validation(format!(
                "unsupported extension `{extension}`"
            )));
        }
    }

    for category in &filter.categories {
        if category.is_empty() || category.len() > 64 {
            return Err(UiError::validation("category must be 1..=64 characters"));
        }
    }

    for excluded in &filter.exclude_paths {
        if excluded.chars().count() > MAX_PATH_CHARS {
            return Err(UiError::validation("excluded path is too long"));
        }
    }

    Ok(())
}

/// Validates a sort specification.
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_sort(sort: SortSpec) -> Result<SortSpec, UiError> {
    // The enum itself is typed, so only the wire name is checked for diagnostics.
    if sort.key.as_str().is_empty() {
        return Err(UiError::validation("sort key is empty"));
    }

    Ok(sort)
}

/// Validates a scan request.
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_scan_request(request: &ScanRequest) -> Result<(), UiError> {
    validate_path(&request.path)?;

    if request.exclusions.len() > MAX_EXCLUSIONS {
        return Err(UiError::validation(format!(
            "at most {MAX_EXCLUSIONS} exclusions are supported"
        )));
    }

    for exclusion in &request.exclusions {
        if exclusion.trim().is_empty() || exclusion.chars().count() > MAX_EXCLUSION_CHARS {
            return Err(UiError::validation(format!(
                "exclusion must be 1..={MAX_EXCLUSION_CHARS} characters"
            )));
        }
        if exclusion.contains('\0') {
            return Err(UiError::validation("exclusion contains a NUL character"));
        }
    }

    Ok(())
}

/// Validates persisted interface settings and their schema version.
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_ui_settings(settings: &UiSettings) -> Result<(), UiError> {
    if settings.schema_version != UI_SCHEMA_VERSION {
        return Err(UiError::validation(format!(
            "unsupported ui schema version {}; expected {UI_SCHEMA_VERSION}",
            settings.schema_version
        )));
    }

    for (screen, columns) in &settings.column_layouts {
        if columns.len() > 64 {
            return Err(UiError::validation(format!(
                "screen `{}` has too many columns",
                screen.as_str()
            )));
        }

        let mut orders = std::collections::HashSet::new();
        for column in columns {
            if column.id.is_empty() || column.id.len() > 128 {
                return Err(UiError::validation("column id must be 1..=128 bytes"));
            }
            if !(40..=1200).contains(&column.width_px) {
                return Err(UiError::validation(
                    "column width must be between 40 and 1200 pixels",
                ));
            }
            if !orders.insert(column.order) {
                return Err(UiError::validation(format!(
                    "duplicate column order on `{}`",
                    screen.as_str()
                )));
            }
        }
    }

    for (screen, filters) in &settings.saved_filters {
        if filters.len() > MAX_SAVED_FILTERS {
            return Err(UiError::validation(format!(
                "screen `{}` has more than {MAX_SAVED_FILTERS} saved filters",
                screen.as_str()
            )));
        }
        for filter in filters {
            validate_filter(filter)?;
        }
    }

    Ok(())
}

/// Validates window geometry without monitor information.
///
/// Coordinates are sanitised separately against the available monitors.
///
/// # Errors
/// Returns [`UiErrorCode::Validation`](crate::error::UiErrorCode::Validation).
pub fn validate_window_state(state: &WindowState) -> Result<(), UiError> {
    if !(800..=MAX_WINDOW_DIMENSION).contains(&state.width) {
        return Err(UiError::validation(
            "window width must be between 800 and 16384",
        ));
    }
    if !(600..=MAX_WINDOW_DIMENSION).contains(&state.height) {
        return Err(UiError::validation(
            "window height must be between 600 and 16384",
        ));
    }
    if let Some(monitor_id) = &state.monitor_id {
        if monitor_id.len() > 128 {
            return Err(UiError::validation("monitor_id must be at most 128 bytes"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_CURSOR_CHARS, MAX_EXCLUSIONS, MAX_PAGE_SIZE, validate_cursor, validate_filter,
        validate_filter_text, validate_name, validate_page_size, validate_path,
        validate_scan_request, validate_ui_settings, validate_window_state,
    };
    use crate::dto::{ColumnState, FilterSet, ScanRequest, ScreenId, UiSettings, WindowState};

    /// Absolute path that is valid on the host platform.
    fn absolute(name: &str) -> String {
        std::env::temp_dir()
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn accepts_absolute_paths_and_rejects_malformed_ones() {
        assert!(validate_path(&absolute("Public")).is_ok());

        #[cfg(windows)]
        assert!(validate_path(r"\\?\C:\very\long").is_ok());

        assert!(validate_path("").is_err());
        assert!(validate_path("relative\\folder").is_err());
        assert!(validate_path(&format!("{}\0", absolute("a"))).is_err());
    }

    #[test]
    fn validates_names_and_filter_text() {
        assert!(validate_name("payload.bin").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("a\\b").is_err());
        assert!(validate_name(&"x".repeat(32_001)).is_err());

        assert!(validate_filter_text("report").is_ok());
        assert!(validate_filter_text(&"x".repeat(257)).is_err());
    }

    #[test]
    fn validates_pages_and_cursors() {
        assert_eq!(validate_page_size(1).expect("minimum"), 1);
        assert_eq!(
            validate_page_size(MAX_PAGE_SIZE).expect("maximum"),
            MAX_PAGE_SIZE
        );
        assert!(validate_page_size(0).is_err());
        assert!(validate_page_size(MAX_PAGE_SIZE + 1).is_err());

        assert!(validate_cursor("v1.abc").is_ok());
        assert!(validate_cursor("").is_err());
        assert!(validate_cursor(&"x".repeat(MAX_CURSOR_CHARS + 1)).is_err());
        assert!(validate_cursor("bad\ncursor").is_err());
    }

    #[test]
    fn validates_filter_ranges_and_extensions() {
        let valid = FilterSet {
            text: "report".to_owned(),
            extensions: vec!["pdf".to_owned(), ".zip".to_owned()],
            min_size: Some(10),
            max_size: Some(100),
            modified_after: Some(1),
            modified_before: Some(2),
            name: Some("Documents".to_owned()),
            ..FilterSet::default()
        };
        assert!(validate_filter(&valid).is_ok());

        let inverted_sizes = FilterSet {
            min_size: Some(100),
            max_size: Some(10),
            ..FilterSet::default()
        };
        assert!(validate_filter(&inverted_sizes).is_err());

        let inverted_dates = FilterSet {
            modified_after: Some(2),
            modified_before: Some(1),
            ..FilterSet::default()
        };
        assert!(validate_filter(&inverted_dates).is_err());

        let bad_extension = FilterSet {
            extensions: vec!["a/b".to_owned()],
            ..FilterSet::default()
        };
        assert!(validate_filter(&bad_extension).is_err());
    }

    #[test]
    fn validates_scan_requests() {
        let request = ScanRequest {
            path: std::env::temp_dir().to_string_lossy().into_owned(),
            method: crate::dto::ScanMethodChoice::Auto,
            incremental: false,
            exclusions: vec!["**/node_modules".to_owned()],
            include_ads: false,
            metric: crate::dto::Metric::OnDisk,
        };
        assert!(validate_scan_request(&request).is_ok());

        let too_many = ScanRequest {
            exclusions: vec!["x".to_owned(); MAX_EXCLUSIONS + 1],
            ..request
        };
        assert!(validate_scan_request(&too_many).is_err());
    }

    #[test]
    fn validates_ui_settings_and_window_state() {
        let mut settings = UiSettings::default();
        assert!(validate_ui_settings(&settings).is_ok());

        settings.schema_version = 99;
        assert!(validate_ui_settings(&settings).is_err());

        let mut filters = UiSettings::default();
        filters.saved_filters.insert(
            ScreenId::Tree,
            vec![FilterSet::default(); super::MAX_SAVED_FILTERS + 1],
        );
        assert!(validate_ui_settings(&filters).is_err());

        let mut columns = UiSettings::default();
        columns.column_layouts.insert(
            ScreenId::Tree,
            vec![ColumnState {
                id: "name".to_owned(),
                visible: true,
                width_px: 10,
                order: 0,
                sortable: true,
            }],
        );
        assert!(validate_ui_settings(&columns).is_err());

        assert!(validate_window_state(&WindowState::default()).is_ok());
        assert!(
            validate_window_state(&WindowState {
                width: 100,
                ..WindowState::default()
            })
            .is_err()
        );
        assert!(
            validate_window_state(&WindowState {
                height: 100,
                ..WindowState::default()
            })
            .is_err()
        );
    }
}
