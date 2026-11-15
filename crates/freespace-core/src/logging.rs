//! Process-wide tracing setup.
//!
//! Diagnostics are written to stderr: FreeSpace is a desktop utility and stdout
//! stays reserved for machine-readable CLI output.

use tracing_subscriber::EnvFilter;

/// Environment variable that overrides the default log filter.
pub const LOG_FILTER_ENV: &str = "FREESPACE_LOG";

/// Installs the global tracing subscriber.
///
/// Returns `true` when this call installed the subscriber and `false` when one
/// was already installed (for example by a test harness). Ignoring the result
/// is expected at process startup.
pub fn init() -> bool {
    let filter = EnvFilter::try_from_env(LOG_FILTER_ENV).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
        .is_ok()
}
