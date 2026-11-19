//! `freespace owners` — occupied space by file owner (FR-064).

use std::path::Path;

use freespace_analysis::{OwnerSummary, annotate};
use freespace_core::{
    Result,
    size::{Units, format_bytes},
};
use freespace_platform::paths;
use serde_json::{Value, json};

use super::{
    Outcome,
    scan::{ScanArgs, execute},
};

/// Runs `freespace owners`.
///
/// # Errors
/// Returns an error when the scan fails.
pub fn run(path: &Path, units: Units) -> Result<Outcome> {
    let mut result = execute(ScanArgs {
        path,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;
    let scan = annotate(&mut result.tree)?;

    let items: Vec<Value> = scan
        .summaries
        .iter()
        .map(|summary| {
            json!({
                "owner_id": summary.owner_id,
                "sid": summary.sid,
                "display_name": summary.display_name,
                "total_bytes": summary.total_bytes,
                "file_count": summary.file_count,
                "share": summary.share(scan.total_bytes()),
            })
        })
        .collect();

    let mut warnings = result.warnings;
    if scan.error_count > 0 {
        warnings.push(format!(
            "{} files could not be inspected for ownership",
            scan.error_count
        ));
    }
    for error in scan.errors.iter().take(20) {
        warnings.push(format!(
            "{}: {}",
            paths::display(&error.path),
            error.message
        ));
    }

    let mut human = format!("{} owners\n", scan.summaries.len());
    for summary in &scan.summaries {
        human.push_str(&format!(
            "{:>12}  {:>8}  {}\n",
            format_bytes(summary.total_bytes, units),
            summary.file_count,
            summary.display_name
        ));
    }

    Ok(Outcome::new(
        json!({
            "items": items,
            "count": scan.summaries.len(),
            "total_bytes": scan.total_bytes(),
            "unknown_count": scan.unknown_count,
            "error_count": scan.error_count,
        }),
        human,
    )
    .with_warnings(warnings))
}

/// Renders one summary; used by tests and the GUI.
#[must_use]
pub fn summary_json(summary: &OwnerSummary, total: u64) -> Value {
    json!({
        "owner_id": summary.owner_id,
        "sid": summary.sid,
        "display_name": summary.display_name,
        "total_bytes": summary.total_bytes,
        "file_count": summary.file_count,
        "share": summary.share(total),
    })
}

#[cfg(test)]
mod tests {
    use freespace_core::size::Units;

    use super::run;

    #[test]
    fn summarises_the_current_user() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("a.bin"), [0u8; 10]).expect("a");
        std::fs::write(directory.path().join("b.bin"), [0u8; 20]).expect("b");

        let outcome = run(directory.path(), Units::Binary).expect("owners");
        let items = outcome.data["items"].as_array().expect("items");

        assert!(!items.is_empty());
        assert_eq!(outcome.data["total_bytes"].as_u64(), Some(30));
        assert!(items.iter().all(|item| item["sid"].as_str().is_some()));
    }
}
