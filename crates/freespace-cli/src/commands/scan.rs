//! `freespace scan` and `freespace tree` — analysis of a volume or directory.

use std::path::Path;

use freespace_analysis::{SizeMode, enrich_ads};
use freespace_core::{
    Error, Result,
    size::{SizeMetrics, Units, format_bytes},
    tree::{AccessState, FsTree, NodeId},
};
use freespace_platform::paths;
use freespace_report::{SnapshotSettings, from_tree};
use freespace_scan::{
    MethodChoice, NoopObserver, ScanManyResult, ScanRequest, ScanResult, scan, scan_many,
};
use serde_json::{Value, json};

use super::Outcome;

/// Arguments of the multi-root `scan` command.
#[derive(Debug, Clone, Copy)]
pub struct ScanManyArgs<'a> {
    /// Volumes or directories, in scan order.
    pub paths: &'a [std::path::PathBuf],
    /// Requested method (`auto`, `mft`, `walk`).
    pub method: &'a str,
    /// Request an incremental update.
    pub incremental: bool,
    /// Exclusion globs.
    pub exclusions: &'a [String],
}

/// Arguments of the single-root `scan` command.
#[derive(Debug, Clone, Copy)]
pub struct ScanArgs<'a> {
    /// Volume or directory.
    pub path: &'a Path,
    /// Requested method (`auto`, `mft`, `walk`).
    pub method: &'a str,
    /// Request an incremental update.
    pub incremental: bool,
    /// Exclusion globs.
    pub exclusions: &'a [String],
}

/// Runs `scan` and renders its result, optionally writing a snapshot.
///
/// # Errors
/// Returns an error when the method is unknown, the scan fails or the snapshot
/// cannot be written.
pub fn run_scan(
    args: ScanManyArgs<'_>,
    units: Units,
    snapshot_out: Option<&Path>,
) -> Result<Outcome> {
    run_scan_with(args, units, snapshot_out, false, "logical")
}

/// Runs `scan` with ADS accounting and an explicit reported size mode.
///
/// # Errors
/// Returns an error for an unknown size mode or a failed scan.
pub fn run_scan_with(
    args: ScanManyArgs<'_>,
    units: Units,
    snapshot_out: Option<&Path>,
    include_ads: bool,
    size_mode: &str,
) -> Result<Outcome> {
    let size_mode = SizeMode::parse_value(size_mode).ok_or_else(|| {
        Error::InvalidArgument(format!(
            "unknown size mode `{size_mode}`; expected `logical`, `on-disk` or `physical`"
        ))
    })?;
    let mut result = execute_many(args)?;
    let mut warnings = warnings_for(&result);
    if include_ads {
        let report = enrich_ads(&mut result.tree);
        warnings.push(format!(
            "{} files inspected for alternate streams, {} streams in {} files",
            report.files, report.streams, report.files_with_streams
        ));
        if report.errors > 0 {
            warnings.push(format!(
                "{} files could not be inspected for alternate streams",
                report.errors
            ));
        }
    }
    let mut data = json_data(&result);
    data["size_mode"] = json!(size_mode.as_str());
    let total = total_size(&result.tree);
    data["total_selected"] = json!(size_mode.value(total));

    if let Some(out) = snapshot_out {
        let volume = result
            .roots
            .first()
            .map(|root| &root.volume)
            .ok_or_else(|| Error::Cancelled("no root was scanned".to_owned()))?;
        let roots: Vec<std::path::PathBuf> =
            result.tree.roots().map(|id| result.tree.path(id)).collect();
        let payload = from_tree(
            &result.tree,
            volume,
            &roots,
            SnapshotSettings {
                units: units.as_str().to_owned(),
                follow_links: false,
                include_ads,
            },
            freespace_core::Timestamp::now(),
        )?;
        let checksum = payload.compute_checksum()?;
        payload.write(out)?;
        data["snapshot"] = json!({
            "path": paths::display(out),
            "node_count": payload.node_count,
            "checksum": checksum,
        });
        if result.roots.len() > 1 {
            warnings.push(
                "the snapshot records the first volume and lists every scanned root".to_owned(),
            );
        }
        warnings.push(format!("snapshot written to `{}`", paths::display(out)));
    }

    Ok(Outcome::new(data, human_summary(&result, units)).with_warnings(warnings))
}

/// Runs `tree`: scans the paths and renders a hierarchical list.
///
/// # Errors
/// Returns an error when the scan fails.
pub fn run_tree(
    paths: &[std::path::PathBuf],
    depth: Option<u32>,
    min_size: Option<u64>,
    units: Units,
) -> Result<Outcome> {
    let result = execute_many(ScanManyArgs {
        paths,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;

    let mut lines = String::new();
    for root in result.tree.roots() {
        render_human(&result.tree, root, 0, depth, units, &mut lines);
    }

    let roots: Vec<Value> = result
        .tree
        .roots()
        .map(|root| node_json(&result.tree, root, 0, depth, min_size))
        .collect();
    let total = total_size(&result.tree);

    Ok(Outcome::new(
        json!({
            "method": summary_method(&result),
            "session_id": result.roots.first().map(|root| root.session.id),
            "nodes_count": result.roots.iter().map(|root| root.session.nodes_count).sum::<u64>(),
            "total": total,
            "roots": roots,
        }),
        lines,
    )
    .with_warnings(warnings_for(&result)))
}

pub(crate) fn execute(args: ScanArgs<'_>) -> Result<ScanResult> {
    let method = MethodChoice::parse_value(args.method).ok_or_else(|| {
        Error::InvalidArgument(format!(
            "unknown scan method `{}`; expected `auto`, `mft` or `walk`",
            args.method
        ))
    })?;
    let request = ScanRequest {
        root: args.path.to_path_buf(),
        method,
        exclusions: args.exclusions.to_vec(),
        incremental: args.incremental,
    };

    scan(&request, &mut NoopObserver)
}

pub(crate) fn execute_many(args: ScanManyArgs<'_>) -> Result<ScanManyResult> {
    let method = MethodChoice::parse_value(args.method).ok_or_else(|| {
        Error::InvalidArgument(format!(
            "unknown scan method `{}`; expected `auto`, `mft` or `walk`",
            args.method
        ))
    })?;
    let requests: Vec<ScanRequest> = args
        .paths
        .iter()
        .map(|path| ScanRequest {
            root: path.clone(),
            method,
            exclusions: args.exclusions.to_vec(),
            incremental: args.incremental,
        })
        .collect();

    let result = scan_many(&requests, &mut NoopObserver)?;
    if result.roots.is_empty() {
        return Err(Error::Cancelled(
            "the scan was cancelled before the first root".to_owned(),
        ));
    }

    Ok(result)
}

fn warnings_for(result: &ScanManyResult) -> Vec<String> {
    let mut warnings: Vec<String> = result
        .roots
        .iter()
        .flat_map(|root| root.warnings.iter().cloned())
        .collect();
    for failure in &result.failures {
        warnings.push(format!(
            "{}: {}",
            paths::display(&failure.root),
            failure.error
        ));
    }

    warnings
}

fn summary_method(result: &ScanManyResult) -> String {
    let methods: Vec<&str> = result
        .roots
        .iter()
        .map(|root| root.method.as_str())
        .collect();
    match methods.first() {
        Some(first) if methods.iter().all(|method| method == first) => (*first).to_owned(),
        Some(_) => "mixed".to_owned(),
        None => "unknown".to_owned(),
    }
}

fn total_size(tree: &FsTree) -> SizeMetrics {
    tree.roots().map(|root| tree.node(root).size).sum()
}

fn json_data(result: &ScanManyResult) -> Value {
    let roots: Vec<String> = result
        .tree
        .roots()
        .map(|id| paths::display(&result.tree.path(id)))
        .collect();
    let mut unique_volumes: Vec<&freespace_core::model::Volume> = Vec::new();
    for root in &result.roots {
        if !unique_volumes
            .iter()
            .any(|volume| volume.id == root.volume.id)
        {
            unique_volumes.push(&root.volume);
        }
    }
    let volumes: Vec<Value> = unique_volumes
        .iter()
        .map(|volume| {
            json!({
                "id": volume.id,
                "mount_point": paths::display(&volume.mount_point),
                "label": volume.label,
                "file_system": volume.file_system.as_str(),
                "total_bytes": volume.total_bytes,
                "free_bytes": volume.free_bytes,
            })
        })
        .collect();
    let methods: Vec<&str> = result
        .roots
        .iter()
        .map(|root| root.method.as_str())
        .collect();
    let total = total_size(&result.tree);
    let issues: Vec<Value> = result
        .roots
        .iter()
        .flat_map(|root| root.issues.iter())
        .map(|issue| {
            json!({
                "path": paths::display(&issue.path),
                "kind": issue.kind.as_str(),
                "message": issue.message,
            })
        })
        .collect();

    json!({
        "session_id": result.roots.first().map(|root| root.session.id),
        "volume": unique_volumes.first().map(|volume| json!({
            "id": volume.id,
            "mount_point": paths::display(&volume.mount_point),
            "label": volume.label,
            "file_system": volume.file_system.as_str(),
            "total_bytes": volume.total_bytes,
            "free_bytes": volume.free_bytes,
        })),
        "volumes": volumes,
        "nodes_count": result.roots.iter().map(|root| root.session.nodes_count).sum::<u64>(),
        "total": total,
        "roots": roots,
        "methods": methods,
        "method": summary_method(result),
        "excluded": result.roots.iter().map(|root| root.excluded).sum::<u64>(),
        "issues": issues,
    })
}

fn human_summary(result: &ScanManyResult, units: Units) -> String {
    let total = total_size(&result.tree);
    let issues: usize = result.roots.iter().map(|root| root.issues.len()).sum();
    let mut text = format!(
        "{} objects, {} on disk, method {}\n",
        result
            .roots
            .iter()
            .map(|root| root.session.nodes_count)
            .sum::<u64>(),
        format_bytes(total.total_on_disk(), units),
        summary_method(result)
    );
    if issues > 0 {
        text.push_str(&format!("{issues} inaccessible or unreadable objects\n"));
    }
    for root in result.tree.roots() {
        text.push_str(&format!("{}\n", paths::display(&result.tree.path(root))));
    }

    text
}

fn render_human(
    tree: &FsTree,
    id: NodeId,
    level: usize,
    depth: Option<u32>,
    units: Units,
    out: &mut String,
) {
    let node = tree.node(id);
    let suffix = match node.access {
        AccessState::Ok => "",
        AccessState::Denied => "  [access denied]",
        AccessState::Error => "  [read error]",
    };
    out.push_str(&format!(
        "{}{:>12}  {}{}\n",
        "  ".repeat(level),
        format_bytes(node.size.total_on_disk(), units),
        tree.name(id),
        suffix
    ));

    if depth.is_some_and(|maximum| level as u32 >= maximum) {
        return;
    }
    let mut children: Vec<NodeId> = tree.children(id).collect();
    children.sort_unstable_by_key(|child| std::cmp::Reverse(tree.node(*child).size.logical));
    for child in children {
        render_human(tree, child, level + 1, depth, units, out);
    }
}

fn node_json(
    tree: &FsTree,
    id: NodeId,
    level: usize,
    depth: Option<u32>,
    min_size: Option<u64>,
) -> Value {
    let node = tree.node(id);
    let mut children: Vec<(NodeId, Value)> = Vec::new();

    if depth.is_none_or(|maximum| level < maximum as usize) {
        for child in tree.children(id) {
            if min_size.is_some_and(|minimum| tree.node(child).size.logical < minimum) {
                continue;
            }
            children.push((child, node_json(tree, child, level + 1, depth, min_size)));
        }
    }
    children.sort_by_key(|node| std::cmp::Reverse(tree.node(node.0).size.logical));

    json!({
        "name": tree.name(id),
        "path": paths::display(&tree.path(id)),
        "kind": node.kind.as_str(),
        "size": node.size,
        "file_count": node.file_count(),
        "dir_count": node.dir_count(),
        "access": node.access.as_str(),
        "children": children.into_iter().map(|(_, value)| value).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use freespace_core::size::Units;

    use super::{ScanArgs, ScanManyArgs, run_scan, run_tree};

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(directory.path().join("nested")).expect("nested");
        std::fs::write(directory.path().join("top.bin"), [b'a'; 40]).expect("top");
        std::fs::write(directory.path().join("nested").join("deep.bin"), [b'b'; 60]).expect("deep");

        directory
    }

    fn paths(directory: &tempfile::TempDir) -> Vec<std::path::PathBuf> {
        vec![directory.path().to_path_buf()]
    }

    #[test]
    fn scan_reports_nodes_and_totals() {
        let directory = fixture();
        let scanned = paths(&directory);
        let outcome = run_scan(
            ScanManyArgs {
                paths: &scanned,
                method: "walk",
                incremental: false,
                exclusions: &[],
            },
            Units::Binary,
            None,
        )
        .expect("scan");

        assert_eq!(outcome.data["nodes_count"].as_u64(), Some(4));
        assert_eq!(outcome.data["total"]["logical"].as_u64(), Some(100));
        assert_eq!(outcome.data["method"].as_str(), Some("directory-walk"));
        assert!(outcome.human.contains("method directory-walk"));
        assert!(outcome.data["roots"][0].as_str().is_some());
        assert_eq!(
            outcome.data["volumes"].as_array().expect("volumes").len(),
            1
        );
    }

    #[test]
    fn scan_reports_every_root_of_a_multi_path_run() {
        let first = fixture();
        let second = fixture();
        let scanned = vec![first.path().to_path_buf(), second.path().to_path_buf()];
        let outcome = run_scan(
            ScanManyArgs {
                paths: &scanned,
                method: "walk",
                incremental: false,
                exclusions: &[],
            },
            Units::Binary,
            None,
        )
        .expect("scan");

        assert_eq!(outcome.data["roots"].as_array().expect("roots").len(), 2);
        assert_eq!(
            outcome.data["volumes"].as_array().expect("volumes").len(),
            1
        );
        assert_eq!(outcome.data["nodes_count"].as_u64(), Some(8));
        assert_eq!(outcome.data["total"]["logical"].as_u64(), Some(200));
    }

    #[test]
    fn tree_renders_a_hierarchy_and_respects_limits() {
        let directory = fixture();
        let scanned = paths(&directory);
        let outcome = run_tree(&scanned, Some(1), Some(50), Units::Binary).expect("tree");
        let root = &outcome.data["roots"][0];

        assert!(root["name"].as_str().is_some());
        assert!(root["children"].as_array().expect("children").len() <= 1);
        assert!(outcome.human.contains("nested"));
    }

    #[test]
    fn rejects_unknown_methods() {
        let directory = fixture();
        let scanned = paths(&directory);
        let error = run_scan(
            ScanManyArgs {
                paths: &scanned,
                method: "magic",
                incremental: false,
                exclusions: &[],
            },
            Units::Binary,
            None,
        )
        .expect_err("must fail");

        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
    }

    #[test]
    fn single_root_execute_still_works_for_other_commands() {
        let directory = fixture();
        let result = super::execute(ScanArgs {
            path: directory.path(),
            method: "walk",
            incremental: false,
            exclusions: &[],
        })
        .expect("scan");

        assert_eq!(result.tree.roots().count(), 1);
        assert_eq!(result.session.nodes_count, 4);
    }
}
