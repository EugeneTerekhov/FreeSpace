//! `freespace filter` — query nodes by attributes, owner and metadata
//! (FR-062, FR-065).

use std::path::{Path, PathBuf};

use freespace_analysis::{
    MetadataCache, MetadataFilter, MetadataKind, MetadataRecord, OwnerScan, OwnerTable, annotate,
};
use freespace_core::{
    Error, Result, Timestamp,
    query::{FilterSpec, NodeFilter},
    size::{Units, format_bytes},
    time::MILLIS_PER_DAY,
    tree::{FsTree, NodeId, NodeKind},
};
use freespace_platform::paths;
use serde_json::{Value, json};

use super::{
    Outcome,
    scan::{ScanArgs, execute},
};

/// Options of `freespace filter`.
#[derive(Debug, Clone)]
pub struct FilterArgs<'a> {
    /// Volume or directory.
    pub path: &'a Path,
    /// Object kind (`file`, `dir`, `link`).
    pub kind: Option<&'a str>,
    /// Minimum size in bytes.
    pub min_size: Option<u64>,
    /// Last modified at least this many days ago.
    pub older_than: Option<u32>,
    /// Comma-separated extension list.
    pub types: Option<&'a str>,
    /// Owner account or `unknown`.
    pub owner: Option<&'a str>,
    /// Minimum resolution (`WIDTHxHEIGHT`).
    pub min_resolution: Option<&'a str>,
    /// Minimum audio bitrate in kbps.
    pub min_bitrate: Option<u32>,
    /// Minimum duration in seconds.
    pub min_duration: Option<u64>,
    /// Comma-separated metadata families.
    pub metadata: Option<&'a str>,
}

impl<'a> FilterArgs<'a> {
    /// Creates an unfiltered query for `path`.
    #[must_use]
    pub const fn new(path: &'a Path) -> Self {
        Self {
            path,
            kind: None,
            min_size: None,
            older_than: None,
            types: None,
            owner: None,
            min_resolution: None,
            min_bitrate: None,
            min_duration: None,
            metadata: None,
        }
    }
}

/// Runs `freespace filter`.
///
/// # Errors
/// Returns an error for invalid arguments or when the scan fails.
pub fn run(args: FilterArgs<'_>, units: Units) -> Result<Outcome> {
    let mut result = execute(ScanArgs {
        path: args.path,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;

    let mut owner_scan: Option<OwnerScan> = None;
    let mut owner_ids = Vec::new();
    if let Some(owner) = args.owner {
        let scan = annotate(&mut result.tree)?;
        if is_unknown(owner) {
            owner_ids.push(u32::MAX);
        } else if let Some(owner_id) = scan.table.find(owner) {
            owner_ids.push(owner_id);
        } else {
            return Ok(Outcome::new(
                json!({
                    "items": [],
                    "count": 0,
                    "owner": owner,
                    "matched_owner": false,
                }),
                format!("no owner `{owner}` was found\n"),
            )
            .with_warnings(result.warnings));
        }
        owner_scan = Some(scan);
    }

    let mut filter_spec = FilterSpec {
        include_hidden: true,
        owner_ids,
        ..FilterSpec::default()
    };
    if let Some(kind) = args.kind {
        filter_spec.kinds = vec![parse_kind(kind)?];
    }
    filter_spec.min_size = args.min_size;
    if let Some(days) = args.older_than {
        let limit = Timestamp::now()
            .as_millis()
            .saturating_sub(i64::from(days).saturating_mul(MILLIS_PER_DAY));
        filter_spec.modified_before = Some(Timestamp::from_millis(limit));
    }
    if let Some(types) = args.types {
        filter_spec.extensions = types
            .split(',')
            .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect();
    }
    let core_filter = filter_spec.compile()?;

    let metadata_filter = build_metadata_filter(&args)?;
    let mut matches = collect_matches(
        &result.tree,
        &core_filter,
        &metadata_filter,
        &mut result.warnings,
    );
    if args.owner.is_some() {
        // Ownership is recorded for files; directories stay in the tree as
        // containers but are not reported as owned objects.
        matches.retain(|item| item.kind != NodeKind::Directory.as_str());
    }

    matches.sort_by(|left, right| {
        right
            .size
            .cmp(&left.size)
            .then_with(|| left.path.cmp(&right.path))
    });

    let owner_table = owner_scan.as_ref().map(|scan| &scan.table);
    let items: Vec<Value> = matches
        .iter()
        .map(|item| {
            let mut value = json!({
                "path": paths::display(&item.path),
                "kind": item.kind,
                "size": item.size,
                "modified": item.modified.format_iso8601(),
            });
            if let Some(owner) = owner_name(owner_table, item.owner_id) {
                value["owner"] = json!(owner);
            }
            if let Some(record) = &item.metadata {
                value["metadata"] = metadata_json(record);
            }
            value
        })
        .collect();

    let mut warnings = result.warnings;
    if let Some(scan) = &owner_scan {
        if scan.error_count > 0 {
            warnings.push(format!(
                "{} files could not be inspected for ownership",
                scan.error_count
            ));
        }
    }

    let mut human = format!("{} matching objects\n", matches.len());
    for item in matches.iter().take(100) {
        human.push_str(&format!(
            "{:>12}  {:<9} {}\n",
            format_bytes(item.size, units),
            item.kind,
            paths::display(&item.path)
        ));
    }

    Ok(Outcome::new(
        json!({
            "items": items,
            "count": matches.len(),
        }),
        human,
    )
    .with_warnings(warnings))
}

#[derive(Debug, Clone)]
struct Match {
    path: PathBuf,
    kind: &'static str,
    size: u64,
    modified: Timestamp,
    owner_id: Option<u32>,
    metadata: Option<MetadataRecord>,
}

fn collect_matches(
    tree: &FsTree,
    core: &NodeFilter,
    metadata: &MetadataFilter,
    warnings: &mut Vec<String>,
) -> Vec<Match> {
    let mut candidates = Vec::new();
    for index in 0..tree.len() {
        let Ok(node_id) = NodeId::try_from(index) else {
            continue;
        };
        if !core.matches(tree, node_id) {
            continue;
        }
        candidates.push((node_id, tree.path(node_id)));
    }

    if metadata.is_empty() {
        return candidates
            .into_iter()
            .map(|(node_id, path)| make_match(tree, node_id, path, None))
            .collect();
    }

    let cache = MetadataCache::new();
    let files: Vec<(NodeId, PathBuf)> = candidates
        .iter()
        .filter(|(node_id, _)| tree.node(*node_id).kind == NodeKind::File)
        .cloned()
        .collect();
    let records = cache.extract_many(&files);
    let mut by_node = std::collections::HashMap::new();
    let mut unavailable = 0usize;
    for record in records {
        if !record.is_available() {
            unavailable += 1;
            if unavailable <= 20 {
                warnings.push(format!(
                    "metadata unavailable for node {}: {}",
                    record.node_id,
                    record.error.as_deref().unwrap_or(record.status.as_str())
                ));
            }
        }
        by_node.insert(record.node_id, record);
    }
    if unavailable > 20 {
        warnings.push(format!(
            "metadata unavailable for {} more files",
            unavailable - 20
        ));
    }

    let mut matches = Vec::new();
    for (node_id, path) in candidates {
        let Some(record) = by_node.remove(&node_id) else {
            continue;
        };
        if metadata.matches(&record) {
            matches.push(make_match(tree, node_id, path, Some(record)));
        }
    }

    matches
}

fn make_match(
    tree: &FsTree,
    node_id: NodeId,
    path: PathBuf,
    metadata: Option<MetadataRecord>,
) -> Match {
    let node = tree.node(node_id);
    Match {
        path,
        kind: node.kind.as_str(),
        size: node.size.logical,
        modified: node.timestamps.modified,
        owner_id: node.owner_id,
        metadata,
    }
}

fn build_metadata_filter(args: &FilterArgs<'_>) -> Result<MetadataFilter> {
    let mut filter = MetadataFilter::default();
    if let Some(resolution) = args.min_resolution {
        let (width, height) = freespace_analysis::parse_resolution(resolution)?;
        filter.min_width = Some(width);
        filter.min_height = Some(height);
    }
    filter.min_bitrate_kbps = args.min_bitrate;
    filter.min_duration_secs = args.min_duration;
    if let Some(kinds) = args.metadata {
        for kind in kinds.split(',') {
            let kind = kind.trim();
            if kind.is_empty() {
                continue;
            }
            let kind = MetadataKind::parse_value(kind).ok_or_else(|| {
                Error::InvalidArgument(format!(
                    "unknown metadata family `{kind}`; expected exif, audio, video or pe"
                ))
            })?;
            filter.kinds.push(kind);
        }
    }

    Ok(filter)
}

fn parse_kind(value: &str) -> Result<NodeKind> {
    match value.to_ascii_lowercase().as_str() {
        "file" => Ok(NodeKind::File),
        "dir" | "directory" | "folder" => Ok(NodeKind::Directory),
        "link" | "symlink" | "junction" => Ok(NodeKind::Symlink),
        other => Err(Error::InvalidArgument(format!(
            "unknown kind `{other}`; expected file, dir or link"
        ))),
    }
}

fn is_unknown(owner: &str) -> bool {
    matches!(
        owner.to_ascii_lowercase().as_str(),
        "unknown" | "неизвестно" | "н/д"
    )
}

fn owner_name(table: Option<&OwnerTable>, owner_id: Option<u32>) -> Option<String> {
    let table = table?;

    Some(
        owner_id
            .and_then(|owner_id| table.get(owner_id))
            .map(|info| info.display_name.clone())
            .unwrap_or_else(|| freespace_analysis::UNKNOWN_OWNER.to_owned()),
    )
}

fn metadata_json(record: &MetadataRecord) -> Value {
    json!({
        "kind": record.kind.as_str(),
        "status": record.status.as_str(),
        "tags": &record.tags,
        "read_at": record.read_at,
    })
}

#[cfg(test)]
mod tests {
    use freespace_core::{size::Units, tree::NodeKind};

    use super::{FilterArgs, is_unknown, parse_kind, run};

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(directory.path().join("nested")).expect("nested");
        std::fs::write(directory.path().join("small.txt"), [0u8; 10]).expect("small");
        std::fs::write(directory.path().join("large.bin"), [0u8; 2048]).expect("large");
        std::fs::write(directory.path().join("nested").join("deep.txt"), [0u8; 32]).expect("deep");
        directory
    }

    #[test]
    fn filters_by_size_extension_and_kind() {
        let directory = fixture();
        let outcome = run(
            FilterArgs {
                min_size: Some(100),
                types: Some("bin"),
                ..FilterArgs::new(directory.path())
            },
            Units::Binary,
        )
        .expect("filter");

        assert_eq!(outcome.data["count"].as_u64(), Some(1));
        let item = &outcome.data["items"][0];
        assert!(item["path"].as_str().expect("path").ends_with("large.bin"));
        assert_eq!(item["size"].as_u64(), Some(2048));
    }

    #[test]
    fn filters_by_kind_and_rejects_unknown_values() {
        let directory = fixture();
        let outcome = run(
            FilterArgs {
                kind: Some("dir"),
                ..FilterArgs::new(directory.path())
            },
            Units::Binary,
        )
        .expect("filter");
        assert!(outcome.data["count"].as_u64().unwrap_or(0) >= 1);

        assert_eq!(parse_kind("FILE").expect("kind"), NodeKind::File);
        assert!(parse_kind("socket").is_err());
        assert!(is_unknown("неизвестно"));
    }

    #[test]
    fn reports_an_unknown_owner_without_matches() {
        let directory = fixture();
        let outcome = run(
            FilterArgs {
                owner: Some("definitely-not-an-account"),
                ..FilterArgs::new(directory.path())
            },
            Units::Binary,
        )
        .expect("filter");

        assert_eq!(outcome.data["count"].as_u64(), Some(0));
        assert_eq!(outcome.data["matched_owner"], false);
    }
}
