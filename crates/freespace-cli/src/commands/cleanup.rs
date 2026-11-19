//! `freespace cleanup suggest|empty-dirs` — stale data and empty folders.

use std::path::Path;

use freespace_analysis::{RecommendOptions, recommend};
use freespace_core::{
    Result, Timestamp,
    size::{Units, format_bytes},
};
use freespace_platform::paths;
use freespace_scan::find_empty_dirs;
use serde_json::{Value, json};

use super::{
    Outcome,
    scan::{ScanArgs, execute},
};

/// Runs `cleanup suggest`.
///
/// # Errors
/// Returns an error when the scan fails.
pub fn run_suggest(path: &Path, older_than: Option<u32>, units: Units) -> Result<Outcome> {
    let result = execute(ScanArgs {
        path,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;
    let candidates = recommend(
        &result.tree,
        RecommendOptions {
            now: Timestamp::now(),
            older_than_days: older_than.unwrap_or(0),
        },
    );

    let items: Vec<Value> = candidates
        .iter()
        .map(|candidate| {
            json!({
                "path": paths::display(&candidate.path),
                "category": candidate.category.as_str(),
                "reason": candidate.reason,
                "reclaimable_bytes": candidate.reclaimable_bytes,
                "risk": candidate.risk.as_str(),
                "recommended_action": candidate.recommended_action.as_str(),
            })
        })
        .collect();
    let reclaimable: u64 = candidates
        .iter()
        .map(|candidate| candidate.reclaimable_bytes)
        .sum();

    let mut human = format!(
        "{} cleanup candidates, {} reclaimable\n",
        candidates.len(),
        format_bytes(reclaimable, units)
    );
    for candidate in &candidates {
        human.push_str(&format!(
            "{:>12}  {:<9} {:<6}  {}\n",
            format_bytes(candidate.reclaimable_bytes, units),
            candidate.category.as_str(),
            candidate.risk.as_str(),
            paths::display(&candidate.path)
        ));
    }

    Ok(Outcome::new(
        json!({
            "candidates": items,
            "reclaimable_bytes": reclaimable,
            "older_than_days": older_than,
        }),
        human,
    )
    .with_warnings(result.warnings))
}

/// Runs `cleanup empty-dirs`.
///
/// # Errors
/// Returns an error when the scan fails.
pub fn run_empty_dirs(path: &Path) -> Result<Outcome> {
    let result = execute(ScanArgs {
        path,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;
    let directories = find_empty_dirs(&result.tree, false);
    let items: Vec<Value> = directories
        .iter()
        .map(|entry| {
            json!({
                "path": paths::display(&entry.path),
                "depth": entry.depth,
            })
        })
        .collect();

    let mut human = format!("{} empty directories\n", directories.len());
    for entry in &directories {
        human.push_str(&format!("  {}\n", paths::display(&entry.path)));
    }

    Ok(Outcome::new(
        json!({
            "items": items,
            "count": directories.len(),
        }),
        human,
    )
    .with_warnings(result.warnings))
}

#[cfg(test)]
mod tests {
    use freespace_core::size::Units;

    use super::{run_empty_dirs, run_suggest};

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(directory.path().join("Cache")).expect("cache");
        std::fs::write(directory.path().join("Cache").join("blob.bin"), [0u8; 64])
            .expect("cache file");
        std::fs::write(directory.path().join("report.tmp"), [1u8; 10]).expect("temp file");
        std::fs::create_dir_all(directory.path().join("empty").join("nested")).expect("empty");

        directory
    }

    #[test]
    fn suggests_categories_and_sizes() {
        let directory = fixture();
        let outcome = run_suggest(directory.path(), None, Units::Binary).expect("suggest");
        let candidates = outcome.data["candidates"].as_array().expect("candidates");

        assert!(candidates.iter().any(|item| item["category"] == "cache"));
        assert!(candidates.iter().any(|item| item["category"] == "temp"));
        assert!(outcome.data["reclaimable_bytes"].as_u64().unwrap_or(0) >= 74);
        assert!(outcome.human.contains("cleanup candidates"));
    }

    #[test]
    fn lists_nested_empty_directories() {
        let directory = fixture();
        let outcome = run_empty_dirs(directory.path()).expect("empty dirs");

        assert_eq!(outcome.data["count"].as_u64(), Some(2));
        assert_eq!(
            outcome.data["items"]
                .as_array()
                .expect("items")
                .first()
                .and_then(|item| item["path"].as_str())
                .map(|path| path.ends_with("nested")),
            Some(true)
        );
    }
}
