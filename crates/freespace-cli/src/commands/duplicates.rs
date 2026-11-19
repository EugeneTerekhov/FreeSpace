//! `freespace duplicates` — find byte-identical files.
//!
//! The command only reports groups: deletion always goes through `delete` or
//! the graphical confirmation flow (contracts/cli.md).

use std::path::Path;

use freespace_analysis::{
    DuplicateOptions, HashAlgorithm, HashCache, KeepRule, apply_keep_rule, find_duplicates,
};
use freespace_core::{
    Error, Result,
    size::{Units, format_bytes},
};
use freespace_platform::paths;
use serde_json::{Value, json};

use super::{
    Outcome,
    scan::{ScanArgs, execute},
};

/// Arguments of the `duplicates` command.
#[derive(Debug, Clone, Copy)]
pub struct DuplicatesArgs<'a> {
    /// Volume or directory to analyse.
    pub path: &'a Path,
    /// Ignore smaller files.
    pub min_size: Option<u64>,
    /// Hash algorithm name.
    pub algorithm: Option<&'a str>,
    /// Keep rule name.
    pub keep_rule: Option<&'a str>,
}

/// Runs `duplicates`.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for unknown algorithms or keep rules and
/// an error when the scan or the hash cache fails.
pub fn run(args: DuplicatesArgs<'_>, units: Units, cache: Option<&HashCache>) -> Result<Outcome> {
    let algorithm = match args.algorithm {
        Some(value) => HashAlgorithm::parse_value(value).ok_or_else(|| {
            Error::InvalidArgument(format!(
                "unknown hash algorithm `{value}`; expected `sha256` or `md5`"
            ))
        })?,
        None => HashAlgorithm::default(),
    };
    let keep_rule = match args.keep_rule {
        Some(value) => Some(KeepRule::parse_value(value).ok_or_else(|| {
            Error::InvalidArgument(format!(
                "unknown keep rule `{value}`; expected `oldest`, `newest`, `shortest` or `original-dir`"
            ))
        })?),
        None => None,
    };

    let result = execute(ScanArgs {
        path: args.path,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;
    let mut search = find_duplicates(
        &result.tree,
        DuplicateOptions {
            min_size: args.min_size.unwrap_or(1),
            algorithm,
        },
        cache,
    )?;

    if let Some(rule) = keep_rule {
        for group in &mut search.groups {
            apply_keep_rule(group, rule);
        }
    }

    let groups: Vec<Value> = search.groups.iter().map(group_json).collect();
    let mut human = format!(
        "{} duplicate groups, {} reclaimable, {} files hashed\n",
        search.groups.len(),
        format_bytes(search.reclaimable_bytes, units),
        search.hashed
    );
    for group in &search.groups {
        human.push_str(&format!(
            "{}  {} × {}  reclaimable {}{}\n",
            &group.hash[..12.min(group.hash.len())],
            group.members.len(),
            format_bytes(group.size, units),
            format_bytes(group.reclaimable_bytes, units),
            if group.stale { "  [stale]" } else { "" }
        ));
        for member in &group.members {
            human.push_str(&format!(
                "    {}  {}\n",
                if member.selected_for_deletion {
                    "delete"
                } else {
                    "keep  "
                },
                paths::display(&member.path)
            ));
        }
    }

    let mut warnings = result.warnings.clone();
    warnings.extend(search.warnings.clone());

    Ok(Outcome::new(
        json!({
            "algorithm": search.algorithm.as_str(),
            "min_size": args.min_size.unwrap_or(1),
            "candidates": search.candidates,
            "hashed": search.hashed,
            "unreadable": search.unreadable,
            "reclaimable_bytes": search.reclaimable_bytes,
            "groups": groups,
        }),
        human,
    )
    .with_warnings(warnings))
}

fn group_json(group: &freespace_analysis::DuplicateGroup) -> Value {
    let members: Vec<Value> = group
        .members
        .iter()
        .map(|member| {
            json!({
                "node_id": member.node_id,
                "path": paths::display(&member.path),
                "size": member.size,
                "modified": member.modified,
                "selected_for_deletion": member.selected_for_deletion,
                "verified": member.verified,
                "reason": member.reason,
            })
        })
        .collect();

    json!({
        "id": group.id.to_string(),
        "algorithm": group.algorithm.as_str(),
        "hash": group.hash,
        "size": group.size,
        "keep_node_id": group.keep_node_id,
        "reclaimable_bytes": group.reclaimable_bytes,
        "stale": group.stale,
        "members": members,
    })
}

#[cfg(test)]
mod tests {
    use freespace_core::size::Units;

    use super::{DuplicatesArgs, run};

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        let payload = vec![9u8; 512];
        std::fs::create_dir_all(directory.path().join("nested")).expect("nested");
        std::fs::write(directory.path().join("a.bin"), &payload).expect("a");
        std::fs::write(directory.path().join("nested").join("b.bin"), &payload).expect("b");
        std::fs::write(directory.path().join("c.bin"), vec![7u8; 512]).expect("c");

        directory
    }

    #[test]
    fn reports_verified_groups_and_a_keep_choice() {
        let directory = fixture();
        let outcome = run(
            DuplicatesArgs {
                path: directory.path(),
                min_size: None,
                algorithm: Some("md5"),
                keep_rule: Some("shortest"),
            },
            Units::Binary,
            None,
        )
        .expect("duplicates");

        let groups = outcome.data["groups"].as_array().expect("groups");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["algorithm"].as_str(), Some("md5"));
        let members = groups[0]["members"].as_array().expect("members");
        assert_eq!(members.len(), 2);
        assert_eq!(
            members
                .iter()
                .filter(|member| member["selected_for_deletion"] == true)
                .count(),
            1
        );
        assert_eq!(
            members
                .iter()
                .filter(|member| member["verified"] == true)
                .count(),
            2
        );
        assert_eq!(groups[0]["reclaimable_bytes"].as_u64(), Some(512));
    }

    #[test]
    fn rejects_unknown_algorithms_and_rules() {
        let directory = fixture();
        let error = run(
            DuplicatesArgs {
                path: directory.path(),
                min_size: None,
                algorithm: Some("crc32"),
                keep_rule: None,
            },
            Units::Binary,
            None,
        )
        .expect_err("algorithm");
        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");

        let error = run(
            DuplicatesArgs {
                path: directory.path(),
                min_size: None,
                algorithm: None,
                keep_rule: Some("magic"),
            },
            Units::Binary,
            None,
        )
        .expect_err("rule");
        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
    }
}
