//! `freespace delete` — Recycle Bin or permanent deletion.

use std::path::PathBuf;

use freespace_core::{
    Error, Result,
    size::{Units, format_bytes},
};
use freespace_ops::{DeleteOptions, Store, delete::delete_paths, operation::DeleteMode};
use freespace_platform::{paths, volumes};
use serde_json::{Value, json};

use super::Outcome;

/// Runs `delete`.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for an unknown mode,
/// [`Error::ConfirmationRequired`] without confirmation, and [`Error::Partial`]
/// when only some objects were deleted.
pub fn run(
    paths_to_delete: &[PathBuf],
    mode: &str,
    confirmed: bool,
    store: &Store,
    units: Units,
) -> Result<Outcome> {
    let mode = DeleteMode::parse_value(mode).ok_or_else(|| {
        Error::InvalidArgument(format!(
            "unknown deletion mode `{mode}`; expected `recycle` or `permanent`"
        ))
    })?;

    let available_bytes = paths_to_delete
        .first()
        .and_then(|path| volumes::describe_volume(path).ok())
        .map(|volume| volume.free_bytes);
    let options = DeleteOptions {
        mode,
        reason: "cli delete".to_owned(),
        available_bytes,
        allow_protected: false,
    };

    let outcome = delete_paths(store, paths_to_delete, &options, confirmed)?;

    let items: Vec<Value> = outcome
        .items
        .iter()
        .map(|item| {
            json!({
                "path": paths::display(&item.path),
                "size": item.size,
                "error": item.error,
            })
        })
        .collect();
    let mut warnings = Vec::new();
    for item in &outcome.items {
        if let Some(error) = &item.error {
            warnings.push(format!("{}: {error}", paths::display(&item.path)));
        }
    }
    let mut human = format!(
        "{} objects deleted ({}), {} failed, {} freed\n",
        outcome.summary.succeeded,
        mode.as_str(),
        outcome.summary.failed,
        format_bytes(outcome.summary.bytes, units)
    );
    if outcome.undoable {
        human.push_str("The objects can be restored from the Recycle Bin.\n");
    }

    if let Some(error) = outcome.first_error() {
        return Err(Error::Partial(format!(
            "{} of {} objects were deleted; first failure: {error}",
            outcome.summary.succeeded,
            paths_to_delete.len()
        )));
    }

    Ok(Outcome::new(
        json!({
            "mode": mode.as_str(),
            "journal_id": outcome.journal_id,
            "undoable": outcome.undoable,
            "summary": outcome.summary,
            "items": items,
        }),
        human,
    )
    .with_warnings(warnings))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use freespace_core::size::Units;
    use freespace_ops::{Store, operation::DeleteMode};

    use super::run;

    #[test]
    fn refuses_without_confirmation() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("keep.txt");
        std::fs::write(&path, b"payload").expect("write");
        let store = Store::open_in_memory().expect("store");

        let error = run(
            std::slice::from_ref(&path),
            DeleteMode::Permanent.as_str(),
            false,
            &store,
            Units::Binary,
        )
        .expect_err("must refuse");

        assert_eq!(error.code().as_str(), "E_CONFIRMATION_REQUIRED");
        assert!(path.exists());
    }

    #[test]
    fn permanently_deletes_with_confirmation_and_journals() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("gone.txt");
        std::fs::write(&path, b"payload").expect("write");
        let store = Store::open_in_memory().expect("store");

        let outcome = run(
            std::slice::from_ref(&path),
            "permanent",
            true,
            &store,
            Units::Binary,
        )
        .expect("delete");

        assert_eq!(outcome.data["mode"], "permanent");
        assert_eq!(outcome.data["summary"]["succeeded"], 1);
        assert!(!path.exists());
        assert_eq!(store.journal_list(10).expect("journal").len(), 1);
    }

    #[test]
    fn rejects_the_removed_quarantine_mode_and_unknown_modes() {
        let store = Store::open_in_memory().expect("store");
        for mode in ["magic", "quarantine"] {
            let error = run(
                &[PathBuf::from(r"C:\temp\file.txt")],
                mode,
                true,
                &store,
                Units::Binary,
            )
            .expect_err("mode");

            assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
        }
    }
}
