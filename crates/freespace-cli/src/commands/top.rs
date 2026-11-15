//! `freespace top` and `freespace types` — largest files and type report.

use std::path::Path;

use freespace_analysis::{group_by_category, group_by_extension, top_files};
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

/// Runs `top`: scans `path` and prints the `limit` largest files.
///
/// # Errors
/// Returns an error when the scan fails.
pub fn run_top(path: &Path, limit: u32, units: Units) -> Result<Outcome> {
    let result = execute(ScanArgs {
        path,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;
    let files = top_files(&result.tree, usize::try_from(limit).unwrap_or(usize::MAX));

    let items: Vec<Value> = files
        .iter()
        .map(|file| {
            json!({
                "node_id": file.node_id,
                "name": file.name,
                "path": paths::display(&file.path),
                "size": file.size,
                "modified": file.modified,
                "category": file.category.as_str(),
                "access": file.access.as_str(),
            })
        })
        .collect();

    let mut human = format!(
        "{} files in the top {limit}, {}\n",
        files.len(),
        format_bytes(files.iter().map(|file| file.size.logical).sum(), units)
    );
    for (rank, file) in files.iter().enumerate() {
        human.push_str(&format!(
            "{:>4}. {:>12}  {}{}\n",
            rank + 1,
            format_bytes(file.size.logical, units),
            paths::display(&file.path),
            if file.is_accessible() {
                ""
            } else {
                "  [inaccessible]"
            }
        ));
    }

    Ok(Outcome::new(
        json!({
            "limit": limit,
            "nodes_scanned": result.session.nodes_count,
            "method": result.method.as_str(),
            "items": items,
        }),
        human,
    )
    .with_warnings(result.warnings.clone()))
}

/// Runs `types`: scans `path` and groups occupied space by extension/category.
///
/// # Errors
/// Returns an error when the scan fails.
pub fn run_types(path: &Path, units: Units) -> Result<Outcome> {
    let result = execute(ScanArgs {
        path,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;
    let groups = group_by_extension(&result.tree);
    let categories = group_by_category(&result.tree);

    let items: Vec<Value> = groups
        .iter()
        .map(|group| {
            json!({
                "extension": group.extension,
                "category": group.category.as_str(),
                "count": group.count,
                "total_size": group.size,
                "share": group.share,
            })
        })
        .collect();
    let category_items: Vec<Value> = categories
        .iter()
        .map(|category| {
            json!({
                "category": category.category.as_str(),
                "count": category.count,
                "total_size": category.size,
                "share": category.share,
            })
        })
        .collect();

    let total: u64 = groups.iter().map(|group| group.size.logical).sum();
    let mut human = format!(
        "{total} bytes in {} extensions, {} categories\n",
        groups.len(),
        categories.len()
    );
    for group in &groups {
        human.push_str(&format!(
            "{:>12}  {:>6.1}%  {:>8}  {:<10} {}\n",
            format_bytes(group.size.logical, units),
            group.share * 100.0,
            group.count,
            group.category.as_str(),
            if group.extension.is_empty() {
                "(no extension)"
            } else {
                &group.extension
            }
        ));
    }

    Ok(Outcome::new(
        json!({
            "nodes_scanned": result.session.nodes_count,
            "method": result.method.as_str(),
            "total": total,
            "items": items,
            "categories": category_items,
        }),
        human,
    )
    .with_warnings(result.warnings.clone()))
}

#[cfg(test)]
mod tests {
    use freespace_core::size::Units;

    use super::{run_top, run_types};

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(directory.path().join("nested")).expect("nested");
        std::fs::write(directory.path().join("a.txt"), [b'a'; 100]).expect("a");
        std::fs::write(directory.path().join("nested").join("b.txt"), [b'b'; 200]).expect("b");
        std::fs::write(directory.path().join("c.mkv"), [b'c'; 300]).expect("c");

        directory
    }

    #[test]
    fn top_lists_the_largest_files_with_a_limit() {
        let directory = fixture();
        let outcome = run_top(directory.path(), 2, Units::Binary).expect("top");

        let items = outcome.data["items"].as_array().expect("items");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["size"]["logical"].as_u64(), Some(300));
        assert_eq!(items[1]["size"]["logical"].as_u64(), Some(200));
        assert_eq!(outcome.data["limit"].as_u64(), Some(2));
        assert!(outcome.human.contains("c.mkv"));
    }

    #[test]
    fn types_group_by_extension_and_category() {
        let directory = fixture();
        let outcome = run_types(directory.path(), Units::Binary).expect("types");

        let items = outcome.data["items"].as_array().expect("items");
        let text = items
            .iter()
            .find(|item| item["extension"] == "txt")
            .expect("txt");
        assert_eq!(text["count"].as_u64(), Some(2));
        assert_eq!(text["total_size"]["logical"].as_u64(), Some(300));
        assert_eq!(text["category"].as_str(), Some("document"));

        let categories = outcome.data["categories"].as_array().expect("categories");
        assert!(
            categories
                .iter()
                .any(|category| category["category"] == "video")
        );
    }

    #[test]
    fn top_rejects_missing_paths() {
        let error = run_top(
            std::path::Path::new(r"C:\definitely-missing-freespace"),
            10,
            Units::Binary,
        )
        .expect_err("missing");

        assert_eq!(error.code().as_str(), "E_NOT_FOUND");
    }
}
