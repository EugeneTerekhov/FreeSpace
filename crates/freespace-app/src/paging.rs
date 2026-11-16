//! Opaque cursor pagination bound to a generation.
//!
//! A tree is immutable for a generation, so a cursor only needs the offset in
//! the sorted result plus the generation, metric and sort spec it was produced
//! with. A cursor from another generation or another ordering is rejected with
//! `E_STALE_CURSOR`, and the interface restarts from the first page.

use serde::{Deserialize, Serialize};

use crate::{
    dto::{Metric, SortKey, SortSpec},
    error::{UiError, UiErrorCode},
    validation::{MAX_CURSOR_CHARS, validate_cursor},
};

const CURSOR_PREFIX: &str = "v1.";

/// Payload hidden inside an opaque cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct CursorPayload {
    generation: u64,
    metric: Metric,
    sort: SortKey,
    descending: bool,
    offset: u64,
}

/// One requested page of an already sorted collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageWindow {
    /// Start index (inclusive).
    pub start: usize,
    /// End index (exclusive).
    pub end: usize,
    /// Cursor for the next page, `None` at the end.
    pub next_cursor: Option<String>,
}

impl PageWindow {
    /// Number of rows in the window.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns `true` when the window is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Encodes a cursor for the given offset.
///
/// # Errors
/// Returns `E_VALIDATION` when the encoded cursor would exceed the contract
/// limit; this cannot happen for the fixed payload.
pub fn encode_cursor(
    generation: u64,
    metric: Metric,
    sort: SortSpec,
    offset: u64,
) -> Result<String, UiError> {
    let payload = CursorPayload {
        generation,
        metric,
        sort: sort.key,
        descending: sort.descending,
        offset,
    };
    let json = serde_json::to_vec(&payload)?;
    let mut cursor = String::with_capacity(CURSOR_PREFIX.len() + json.len() * 2);
    cursor.push_str(CURSOR_PREFIX);
    for byte in json {
        use std::fmt::Write as _;
        write!(&mut cursor, "{byte:02x}")
            .map_err(|error| UiError::new(UiErrorCode::Internal).with_details(error.to_string()))?;
    }

    if cursor.chars().count() > MAX_CURSOR_CHARS {
        return Err(UiError::validation("encoded cursor exceeds the limit"));
    }

    Ok(cursor)
}

/// Decodes and validates a cursor against the current request.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed cursor and `E_STALE_CURSOR` when the
/// generation, metric or ordering no longer matches.
pub fn decode_cursor(
    cursor: &str,
    generation: u64,
    metric: Metric,
    sort: SortSpec,
) -> Result<u64, UiError> {
    validate_cursor(cursor)?;

    let hex = cursor
        .strip_prefix(CURSOR_PREFIX)
        .ok_or_else(|| UiError::validation("cursor has an unknown prefix"))?;
    if hex.len() % 2 != 0 {
        return Err(UiError::validation("cursor has an odd hex length"));
    }

    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(pair)
            .map_err(|_| UiError::validation("cursor contains non-ASCII bytes"))?;
        let byte = u8::from_str_radix(text, 16)
            .map_err(|_| UiError::validation("cursor contains invalid hex"))?;
        bytes.push(byte);
    }

    let payload: CursorPayload = serde_json::from_slice(&bytes)
        .map_err(|_| UiError::validation("cursor payload is malformed"))?;

    if payload.generation != generation
        || payload.metric != metric
        || payload.sort != sort.key
        || payload.descending != sort.descending
    {
        return Err(UiError::new(UiErrorCode::StaleCursor)
            .with_details("cursor belongs to another generation or ordering"));
    }

    Ok(payload.offset)
}

/// Resolves a page window for a sorted collection of `len` rows.
///
/// # Errors
/// Returns `E_STALE_CURSOR` when the cursor does not match the request and
/// `E_VALIDATION` when the page size is outside the contract range.
pub fn page_window(
    len: usize,
    page_size: u16,
    cursor: Option<&str>,
    generation: u64,
    metric: Metric,
    sort: SortSpec,
) -> Result<PageWindow, UiError> {
    let page_size = usize::from(crate::validation::validate_page_size(page_size)?);
    let offset = match cursor {
        Some(cursor) => decode_cursor(cursor, generation, metric, sort)?,
        None => 0,
    };

    let len = u64::try_from(len).unwrap_or(u64::MAX);
    let start = offset.min(len);
    let end = start
        .saturating_add(u64::try_from(page_size).unwrap_or(u64::MAX))
        .min(len);
    let next_cursor = if end < len {
        Some(encode_cursor(generation, metric, sort, end)?)
    } else {
        None
    };

    let start = usize::try_from(start).map_err(|_| {
        UiError::new(UiErrorCode::Internal).with_details("page offset does not fit usize")
    })?;
    let end = usize::try_from(end).map_err(|_| {
        UiError::new(UiErrorCode::Internal).with_details("page end does not fit usize")
    })?;

    Ok(PageWindow {
        start,
        end,
        next_cursor,
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_cursor, encode_cursor, page_window};
    use crate::dto::{Metric, SortKey, SortSpec};
    use crate::error::UiErrorCode;

    fn sort() -> SortSpec {
        SortSpec {
            key: SortKey::Size,
            descending: true,
        }
    }

    #[test]
    fn round_trips_a_cursor() {
        let cursor = encode_cursor(7, Metric::OnDisk, sort(), 1_500).expect("encode");

        assert!(cursor.starts_with("v1."));
        assert!(cursor.len() <= 512);
        assert_eq!(
            decode_cursor(&cursor, 7, Metric::OnDisk, sort()).expect("decode"),
            1_500
        );
    }

    #[test]
    fn rejects_malformed_cursors() {
        for cursor in ["", "v2.abc", "v1.abc", "v1.zz", "v1."] {
            let error = decode_cursor(cursor, 1, Metric::OnDisk, sort()).expect_err("must fail");
            assert_eq!(error.code, UiErrorCode::Validation, "{cursor}");
        }
    }

    #[test]
    fn rejects_cursors_from_another_generation_or_ordering() {
        let cursor = encode_cursor(3, Metric::OnDisk, sort(), 5).expect("encode");

        let error = decode_cursor(&cursor, 4, Metric::OnDisk, sort()).expect_err("stale");
        assert_eq!(error.code, UiErrorCode::StaleCursor);

        let other_metric =
            decode_cursor(&cursor, 3, Metric::Logical, sort()).expect_err("stale metric");
        assert_eq!(other_metric.code, UiErrorCode::StaleCursor);

        let other_sort = decode_cursor(
            &cursor,
            3,
            Metric::OnDisk,
            SortSpec {
                key: SortKey::Name,
                descending: false,
            },
        )
        .expect_err("stale sort");
        assert_eq!(other_sort.code, UiErrorCode::StaleCursor);
    }

    #[test]
    fn pages_through_the_collection() {
        let first = page_window(1_200, 500, None, 1, Metric::OnDisk, sort()).expect("first");
        assert_eq!((first.start, first.end, first.len()), (0, 500, 500));
        assert!(first.next_cursor.is_some());

        let second = page_window(
            1_200,
            500,
            first.next_cursor.as_deref(),
            1,
            Metric::OnDisk,
            sort(),
        )
        .expect("second");
        assert_eq!((second.start, second.end), (500, 1_000));
        assert!(second.next_cursor.is_some());

        let third = page_window(
            1_200,
            500,
            second.next_cursor.as_deref(),
            1,
            Metric::OnDisk,
            sort(),
        )
        .expect("third");
        assert_eq!((third.start, third.end), (1_000, 1_200));
        assert!(third.next_cursor.is_none());
    }

    #[test]
    fn an_offset_past_the_end_yields_an_empty_window() {
        let cursor = encode_cursor(1, Metric::OnDisk, sort(), 10_000).expect("encode");
        let window =
            page_window(10, 500, Some(&cursor), 1, Metric::OnDisk, sort()).expect("window");

        assert!(window.is_empty());
        assert_eq!(window.start, 10);
        assert!(window.next_cursor.is_none());
    }

    #[test]
    fn rejects_a_page_size_outside_the_contract() {
        let error = page_window(10, 0, None, 1, Metric::OnDisk, sort()).expect_err("invalid");
        assert_eq!(error.code, UiErrorCode::Validation);
    }
}
