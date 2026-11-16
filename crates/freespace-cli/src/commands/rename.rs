//! `freespace rename` — bulk rename by a regular expression.

use std::path::PathBuf;

use freespace_core::{Error, Result};
use freespace_ops::{
    CaseTransform, RenamePlan, RenameRule, Store,
    rename_plan::{apply_plan, build_plan},
};
use freespace_platform::paths;
use serde_json::{Value, json};

use super::Outcome;

/// Runs `rename`.
///
/// # Errors
/// Returns an error for a malformed pattern, a plan with conflicts, or a
/// file-system failure.
pub fn run(
    targets: &[PathBuf],
    regex: &str,
    replacement: &str,
    case: Option<&str>,
    preview: bool,
    confirmed: bool,
    store: Option<&Store>,
) -> Result<Outcome> {
    let case = match case {
        Some(value) => CaseTransform::parse_value(value).ok_or_else(|| {
            Error::InvalidArgument(format!(
                "unknown case transform `{value}`; expected `preserve`, `lower` or `upper`"
            ))
        })?,
        None => CaseTransform::Preserve,
    };
    let rule = RenameRule::new(regex, replacement, case)?;
    let plan = build_plan(targets, &rule)?;
    let apply = confirmed && !preview;

    if !apply {
        return Ok(Outcome::new(
            json!({
                "preview": true,
                "pattern": regex,
                "replacement": replacement,
                "case": case.as_str(),
                "items": plan.items.iter().map(item_json).collect::<Vec<_>>(),
                "conflicts": plan.conflicts.iter().map(conflict_json).collect::<Vec<_>>(),
            }),
            human_preview(&plan),
        ));
    }

    if !plan.is_clear() {
        return Err(Error::Conflict(format!(
            "{} conflicts block the rename; resolve them first",
            plan.conflicts.len()
        )));
    }
    let outcome = apply_plan(&plan, store)?;

    Ok(Outcome::new(
        json!({
            "preview": false,
            "pattern": regex,
            "replacement": replacement,
            "case": case.as_str(),
            "renamed": outcome.renamed,
            "operation_id": outcome.operation_id,
            "journal_id": outcome.journal_id,
            "undoable": outcome.undoable,
            "items": outcome.renames.iter().map(|(from, to)| json!({
                "from": paths::display(from),
                "to": paths::display(to),
            })).collect::<Vec<_>>(),
        }),
        format!("{} objects renamed\n", outcome.renamed),
    ))
}

fn item_json(item: &freespace_ops::RenameItem) -> Value {
    json!({
        "from": paths::display(&item.from),
        "to": paths::display(&item.to),
        "old_name": item.old_name,
        "new_name": item.new_name,
        "size": item.size,
    })
}

fn conflict_json(conflict: &freespace_ops::RenameConflict) -> Value {
    json!({
        "path": paths::display(&conflict.path),
        "target": conflict.target.as_deref().map(paths::display),
        "reason": conflict.reason,
    })
}

fn human_preview(plan: &RenamePlan) -> String {
    let mut text = format!(
        "preview: {} renames, {} conflicts\n",
        plan.items.len(),
        plan.conflicts.len()
    );
    for item in &plan.items {
        text.push_str(&format!(
            "  {} → {}\n",
            paths::display(&item.from),
            paths::display(&item.to)
        ));
    }
    for conflict in &plan.conflicts {
        text.push_str(&format!(
            "  conflict {}: {}\n",
            paths::display(&conflict.path),
            conflict.reason
        ));
    }

    text
}

#[cfg(test)]
mod tests {
    use freespace_core::ErrorCode;
    use freespace_ops::Store;

    use super::run;

    fn fixture() -> (tempfile::TempDir, Vec<std::path::PathBuf>) {
        let directory = tempfile::tempdir().expect("temp dir");
        let paths = ["IMG_0001.jpg", "IMG_0002.jpg"]
            .into_iter()
            .map(|name| {
                let path = directory.path().join(name);
                std::fs::write(&path, b"payload").expect("write");
                path
            })
            .collect();

        (directory, paths)
    }

    #[test]
    fn previews_without_confirmation_and_applies_with_it() {
        let (directory, paths) = fixture();

        let preview = run(
            &paths,
            r"^IMG_(\d+)",
            "photo-$1",
            Some("lower"),
            false,
            false,
            None,
        )
        .expect("preview");
        assert_eq!(preview.data["preview"], true);
        assert_eq!(preview.data["items"].as_array().expect("items").len(), 2);
        assert!(paths[0].exists(), "preview changes nothing");

        let store = Store::open_in_memory().expect("store");
        let applied = run(
            &paths,
            r"^IMG_(\d+)",
            "photo-$1",
            Some("lower"),
            false,
            true,
            Some(&store),
        )
        .expect("apply");
        assert_eq!(applied.data["renamed"], 2);
        assert!(directory.path().join("photo-0001.jpg").exists());
        assert_eq!(store.journal_list(10).expect("journal").len(), 1);
    }

    #[test]
    fn blocks_conflicting_plans() {
        let (_directory, paths) = fixture();

        let error =
            run(&paths, r"IMG_\d+", "same", None, false, true, None).expect_err("conflicts");

        assert_eq!(error.code(), ErrorCode::Conflict);
    }

    #[test]
    fn rejects_invalid_patterns_and_case_names() {
        let (_directory, paths) = fixture();

        assert_eq!(
            run(&paths, "[", "x", None, true, false, None)
                .expect_err("pattern")
                .code(),
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            run(&paths, "^IMG", "x", Some("magic"), true, false, None)
                .expect_err("case")
                .code(),
            ErrorCode::InvalidArgument
        );
    }
}
