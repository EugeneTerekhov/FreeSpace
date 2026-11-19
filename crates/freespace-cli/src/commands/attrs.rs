//! `freespace attrs` and `freespace acl` — batch attributes, owner and rights.

use std::path::{Path, PathBuf};

use freespace_core::{Error, Result};
use freespace_ops::{
    AclAction, AttributeChange, Store, acl::apply_acl, attributes::apply_attributes,
    confirm::require_confirmation,
};
use freespace_platform::paths;
use serde_json::json;

use super::Outcome;

/// Runs `attrs`.
///
/// # Errors
/// Returns [`Error::ConfirmationRequired`] without `--yes`,
/// [`Error::InvalidArgument`] for unknown attributes, and an error when every
/// object failed.
pub fn run_attrs(
    targets: &[PathBuf],
    set: Option<&str>,
    clear: Option<&str>,
    confirmed: bool,
    store: Option<&Store>,
) -> Result<Outcome> {
    require_confirmation(confirmed)?;
    let change = AttributeChange::parse(set, clear)?;
    let report = apply_attributes(store, targets, change)?;

    if report.succeeded() == 0 {
        return Err(Error::Operation(
            report
                .items
                .first()
                .and_then(|item| item.error.clone())
                .unwrap_or_else(|| "no attribute was changed".to_owned()),
        ));
    }

    Ok(Outcome::new(
        json!({
            "set": change.set,
            "clear": change.clear,
            "succeeded": report.succeeded(),
            "failed": report.failed(),
            "journal_id": report.journal_id,
            "items": report.items.iter().map(|item| json!({
                "path": paths::display(&item.path),
                "attributes": item.attributes,
                "error": item.error,
            })).collect::<Vec<_>>(),
        }),
        format!(
            "{} objects updated, {} failed\n",
            report.succeeded(),
            report.failed()
        ),
    ))
}

/// Runs `acl`.
///
/// # Errors
/// Returns [`Error::ConfirmationRequired`] without `--yes`,
/// [`Error::InvalidArgument`] for an ambiguous action, and an error when every
/// object failed.
pub fn run_acl(
    targets: &[PathBuf],
    owner: Option<&str>,
    reset: bool,
    copy_from: Option<&Path>,
    confirmed: bool,
    store: Option<&Store>,
) -> Result<Outcome> {
    require_confirmation(confirmed)?;
    let action = match (owner, reset, copy_from) {
        (Some(account), false, None) => AclAction::SetOwner(account.to_owned()),
        (None, true, None) => AclAction::Reset,
        (None, false, Some(source)) => AclAction::CopyFrom(source.to_path_buf()),
        _ => {
            return Err(Error::InvalidArgument(
                "choose exactly one of `--owner`, `--reset` or `--copy-from`".to_owned(),
            ));
        }
    };
    let report = apply_acl(store, targets, &action)?;

    if report.succeeded() == 0 {
        return Err(Error::Operation(
            report
                .items
                .first()
                .and_then(|item| item.error.clone())
                .unwrap_or_else(|| "no permission was changed".to_owned()),
        ));
    }

    Ok(Outcome::new(
        json!({
            "action": action.as_str(),
            "succeeded": report.succeeded(),
            "failed": report.failed(),
            "journal_id": report.journal_id,
            "items": report.items.iter().map(|item| json!({
                "path": paths::display(&item.path),
                "owner": item.owner,
                "error": item.error,
            })).collect::<Vec<_>>(),
        }),
        format!(
            "{} objects updated, {} failed\n",
            report.succeeded(),
            report.failed()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use freespace_core::ErrorCode;
    use freespace_ops::{HIDDEN, Store};
    use freespace_platform::security::get_attributes;

    use super::{run_acl, run_attrs};

    #[test]
    fn requires_confirmation() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("file.txt");
        std::fs::write(&path, b"x").expect("write");

        let error = run_attrs(&[path], Some("hidden"), None, false, None).expect_err("refuse");
        assert_eq!(error.code(), ErrorCode::ConfirmationRequired);
    }

    #[test]
    fn changes_attributes() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("file.txt");
        std::fs::write(&path, b"x").expect("write");
        let store = Store::open_in_memory().expect("store");

        let outcome = run_attrs(
            std::slice::from_ref(&path),
            Some("hidden"),
            None,
            true,
            Some(&store),
        )
        .expect("attrs");

        assert_eq!(outcome.data["succeeded"], 1);
        assert_eq!(get_attributes(&path).expect("attributes") & HIDDEN, HIDDEN);
        run_attrs(&[path], None, Some("hidden"), true, None).expect("clear");
    }

    #[test]
    fn validates_acl_actions() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("file.txt");
        std::fs::write(&path, b"x").expect("write");

        let error = run_acl(
            std::slice::from_ref(&path),
            Some("a"),
            true,
            None,
            true,
            None,
        )
        .expect_err("ambiguous");
        assert_eq!(error.code(), ErrorCode::InvalidArgument);
    }
}
