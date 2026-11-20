//! `freespace capabilities` — describe the current platform backend.

use freespace_core::{Error, Result};
use freespace_platform::capabilities::Capabilities;

use super::Outcome;

/// Returns the capability descriptor for the running platform.
///
/// # Errors
/// Returns an error when the descriptor cannot be serialised.
pub fn run() -> Result<Outcome> {
    let capabilities = Capabilities::current();
    let data = serde_json::to_value(capabilities)
        .map_err(|error| Error::Other(format!("cannot serialise capabilities: {error}")))?;

    let scan = if capabilities.scan_methods.mft {
        "mft, usn, walk"
    } else if capabilities.scan_methods.walk {
        "walk"
    } else {
        "none"
    };
    let human = format!(
        "platform: {platform}\nscan: {scan}\ntrash: {trash}\npermanent: {permanent}\n\
         shred: {shred}\nacl: {acl}\nattrs: {attrs}\nads: {ads}\nunlock: {unlock}\n\
         context-menu: {context_menu}\nmonitor: {monitor}\nelevation: {elevation}\n\
         long-paths: {long_paths}\ncase-sensitive: {case_sensitive}\nmax-path: {max_path}\n",
        platform = capabilities.platform.as_str(),
        scan = scan,
        trash = capabilities.trash,
        permanent = capabilities.permanent,
        shred = capabilities.shred,
        acl = capabilities.acl,
        attrs = capabilities.attrs,
        ads = capabilities.ads,
        unlock = capabilities.unlock,
        context_menu = capabilities.context_menu,
        monitor = capabilities.monitor,
        elevation = capabilities.elevation,
        long_paths = capabilities.long_paths,
        case_sensitive = capabilities.case_sensitive,
        max_path = capabilities
            .max_path
            .map_or_else(|| "unlimited".to_owned(), |value| value.to_string()),
    );

    Ok(Outcome::new(data, human))
}
