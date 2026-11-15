//! `freespace scan` and `freespace tree` — analysis of a volume or directory.

use std::path::Path;

use freespace_core::{
    Error, Result,
    size::{SizeMetrics, Units, format_bytes},
    tree::{AccessState, FsTree, NodeId},
};
use freespace_platform::paths;
use freespace_scan::{MethodChoice, NoopObserver, ScanRequest, ScanResult, scan};
use serde_json::{Value, json};

use super::Outcome;

/// Arguments of the `scan` command.
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

/// Runs `scan` and renders its result.
///
/// # Errors
/// Returns an error when the method is unknown or the scan fails.
pub fn run_scan(args: ScanArgs<'_>, units: Units) -> Result<Outcome> {
    let result = execute(args)?;

    Ok(
        Outcome::new(json_data(&result), human_summary(&result, units))
            .with_warnings(result.warnings.clone()),
    )
}

/// Runs `tree`: scans the path and renders a hierarchical list.
///
/// # Errors
/// Returns an error when the scan fails.
pub fn run_tree(
    path: &Path,
    depth: Option<u32>,
    min_size: Option<u64>,
    units: Units,
) -> Result<Outcome> {
    let result = execute(ScanArgs {
        path,
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
    let total = result
        .tree
        .roots()
        .next()
        .map_or(SizeMetrics::ZERO, |root| result.tree.node(root).size);

    Ok(Outcome::new(
        json!({
            "method": result.method.as_str(),
            "session_id": result.session.id,
            "nodes_count": result.session.nodes_count,
            "total": total,
            "roots": roots,
        }),
        lines,
    )
    .with_warnings(result.warnings.clone()))
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

fn json_data(result: &ScanResult) -> Value {
    let roots: Vec<String> = result
        .session
        .roots
        .iter()
        .map(|root| paths::display(root))
        .collect();
    let total = result
        .tree
        .roots()
        .next()
        .map_or(SizeMetrics::ZERO, |root| result.tree.node(root).size);
    let issues: Vec<Value> = result
        .issues
        .iter()
        .map(|issue| {
            json!({
                "path": paths::display(&issue.path),
                "kind": issue.kind.as_str(),
                "message": issue.message,
            })
        })
        .collect();

    json!({
        "session_id": result.session.id,
        "volume": {
            "id": result.volume.id,
            "mount_point": paths::display(&result.volume.mount_point),
            "label": result.volume.label,
            "file_system": result.volume.file_system.as_str(),
            "total_bytes": result.volume.total_bytes,
            "free_bytes": result.volume.free_bytes,
        },
        "nodes_count": result.session.nodes_count,
        "total": total,
        "roots": roots,
        "method": result.method.as_str(),
        "excluded": result.excluded,
        "issues": issues,
    })
}

fn human_summary(result: &ScanResult, units: Units) -> String {
    let total = result
        .tree
        .roots()
        .next()
        .map_or(SizeMetrics::ZERO, |root| result.tree.node(root).size);
    let mut text = format!(
        "{} objects, {} on disk, method {}\n",
        result.session.nodes_count,
        format_bytes(total.total_on_disk(), units),
        result.method.as_str()
    );
    if !result.issues.is_empty() {
        text.push_str(&format!(
            "{} inaccessible or unreadable objects\n",
            result.issues.len()
        ));
    }
    for root in result.session.roots.iter() {
        text.push_str(&format!("{}\n", paths::display(root)));
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
    children.sort_by(|left, right| {
        tree.node(right.0)
            .size
            .logical
            .cmp(&tree.node(left.0).size.logical)
    });

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

    use super::{ScanArgs, run_scan, run_tree};

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(directory.path().join("nested")).expect("nested");
        std::fs::write(directory.path().join("top.bin"), [b'a'; 40]).expect("top");
        std::fs::write(directory.path().join("nested").join("deep.bin"), [b'b'; 60]).expect("deep");

        directory
    }

    #[test]
    fn scan_reports_nodes_and_totals() {
        let directory = fixture();
        let outcome = run_scan(
            ScanArgs {
                path: directory.path(),
                method: "walk",
                incremental: false,
                exclusions: &[],
            },
            Units::Binary,
        )
        .expect("scan");

        assert_eq!(outcome.data["nodes_count"].as_u64(), Some(4));
        assert_eq!(outcome.data["total"]["logical"].as_u64(), Some(100));
        assert_eq!(outcome.data["method"].as_str(), Some("directory-walk"));
        assert!(outcome.human.contains("method directory-walk"));
        assert!(outcome.data["roots"][0].as_str().is_some());
    }

    #[test]
    fn tree_renders_a_hierarchy_and_respects_limits() {
        let directory = fixture();
        let outcome = run_tree(directory.path(), Some(1), Some(50), Units::Binary).expect("tree");
        let root = &outcome.data["roots"][0];

        assert!(root["name"].as_str().is_some());
        assert!(root["children"].as_array().expect("children").len() <= 1);
        assert!(outcome.human.contains("nested"));
    }

    #[test]
    fn rejects_unknown_methods() {
        let directory = fixture();
        let error = run_scan(
            ScanArgs {
                path: directory.path(),
                method: "magic",
                incremental: false,
                exclusions: &[],
            },
            Units::Binary,
        )
        .expect_err("must fail");

        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
    }
}
