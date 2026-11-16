//! `freespace snapshot create|diff` — disk-state snapshots and comparison.

use std::path::Path;

use freespace_core::{
    Error, Result, Timestamp,
    size::{Units, format_bytes},
};
use freespace_report::{
    DiffChange, SnapshotDiff, SnapshotPayload, SnapshotSettings, compare, from_tree,
};
use serde_json::{Value, json};

use super::{
    Outcome,
    scan::{ScanArgs, execute},
};

/// Creates a snapshot of `path`.
///
/// # Errors
/// Returns an error when the scan fails or the snapshot cannot be written.
pub fn create(path: &Path, out: &Path, units: Units) -> Result<Outcome> {
    let result = execute(ScanArgs {
        path,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;
    let payload = from_tree(
        &result.tree,
        &result.volume,
        &result.session.roots,
        SnapshotSettings {
            units: units.as_str().to_owned(),
            follow_links: false,
            include_ads: false,
        },
        Timestamp::now(),
    )?;
    let checksum = payload.compute_checksum()?;
    payload.write(out)?;

    Ok(Outcome::new(
        json!({
            "path": out.to_string_lossy(),
            "format_version": payload.format_version,
            "created_at": payload.created_at,
            "volume": payload.volume,
            "node_count": payload.node_count,
            "checksum": checksum,
        }),
        format!(
            "snapshot `{}` written: {} nodes, checksum {}\n",
            out.display(),
            payload.node_count,
            &checksum[..12.min(checksum.len())]
        ),
    )
    .with_warnings(result.warnings))
}

/// Compares two snapshots and optionally exports the report.
///
/// # Errors
/// Returns an error when a snapshot cannot be read, the volumes differ or the
/// export cannot be written.
pub fn diff(base: &Path, target: &Path, export: Option<&Path>, units: Units) -> Result<Outcome> {
    let base_snapshot = SnapshotPayload::read(base)?;
    let target_snapshot = SnapshotPayload::read(target)?;
    let diff = compare(&base_snapshot, &target_snapshot)?;

    if let Some(path) = export {
        export_report(&diff, path)?;
    }

    let mut human = format!(
        "{} added, {} removed, {} changed; total growth {}\n",
        diff.added.len(),
        diff.removed.len(),
        diff.changed.len(),
        signed_bytes(diff.total_growth_bytes, units)
    );
    for entry in diff.added.iter().chain(diff.changed.iter()).take(20) {
        human.push_str(&format!(
            "{:>+12}  {:<8} {}\n",
            signed_bytes(entry.size_delta, units),
            entry.change.as_str(),
            entry.path
        ));
    }

    Ok(Outcome::new(
        json!({
            "volume_id": diff.volume_id,
            "base_created_at": diff.base_created_at,
            "target_created_at": diff.target_created_at,
            "added": diff.added.iter().map(entry_json).collect::<Vec<_>>(),
            "removed": diff.removed.iter().map(entry_json).collect::<Vec<_>>(),
            "changed": diff.changed.iter().map(entry_json).collect::<Vec<_>>(),
            "growth_by_directory": diff.growth_by_directory.iter().map(|entry| {
                json!({"path": entry.path, "delta": entry.delta})
            }).collect::<Vec<_>>(),
            "total_growth_bytes": diff.total_growth_bytes,
            "export": export.map(|path| path.to_string_lossy().into_owned()),
        }),
        human,
    ))
}

fn entry_json(entry: &freespace_report::DiffEntry) -> Value {
    json!({
        "path": entry.path,
        "kind": entry.change.as_str(),
        "is_directory": entry.is_directory,
        "size_delta": entry.size_delta,
        "mtime_delta": entry.mtime_delta,
        "base_size": entry.base_size,
        "target_size": entry.target_size,
    })
}

fn export_report(diff: &SnapshotDiff, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let contents = if extension == "csv" {
        let mut text = String::from("change,path,size_delta,mtime_delta\n");
        for entry in diff
            .added
            .iter()
            .chain(diff.removed.iter())
            .chain(diff.changed.iter())
        {
            text.push_str(&format!(
                "{},\"{}\",{},{}\n",
                entry.change.as_str(),
                entry.path.replace('"', "\"\""),
                entry.size_delta,
                entry.mtime_delta
            ));
        }
        text
    } else {
        serde_json::to_string_pretty(&json!({
            "volume_id": diff.volume_id,
            "total_growth_bytes": diff.total_growth_bytes,
            "added": diff.added.iter().map(entry_json).collect::<Vec<_>>(),
            "removed": diff.removed.iter().map(entry_json).collect::<Vec<_>>(),
            "changed": diff.changed.iter().map(entry_json).collect::<Vec<_>>(),
            "growth_by_directory": diff.growth_by_directory.iter().map(|entry| {
                json!({"path": entry.path, "delta": entry.delta})
            }).collect::<Vec<_>>(),
        }))
        .map_err(|error| Error::Other(format!("cannot encode the report: {error}")))?
    };

    std::fs::write(path, contents)?;

    Ok(())
}

fn signed_bytes(value: i64, units: Units) -> String {
    if value < 0 {
        format!("-{}", format_bytes(value.unsigned_abs(), units))
    } else {
        format!("+{}", format_bytes(value.unsigned_abs(), units))
    }
}

/// Parses a stored change name; used by GUI screens.
#[must_use]
pub fn parse_change(value: &str) -> Option<DiffChange> {
    match value {
        "added" => Some(DiffChange::Added),
        "removed" => Some(DiffChange::Removed),
        "changed" => Some(DiffChange::Changed),
        _ => None,
    }
}
