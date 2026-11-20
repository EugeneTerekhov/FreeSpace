#![forbid(unsafe_code)]

//! FreeSpace command line interface.
//!
//! The crate exposes the `clap` command tree, the JSON envelope and the
//! documented exit codes; [`main_entry`] is the thin process entry point.

pub mod cli;
pub mod commands;
pub mod lock;
pub mod output;

use std::{
    ffi::OsString,
    io::{self, Write},
    path::Path,
    sync::Mutex,
    time::Instant,
};

use clap::Parser;
use freespace_core::{
    Error, Result, i18n, logging,
    settings::{self, Settings},
};
use serde_json::Value;

use cli::{Cli, Command};
use commands::Outcome;
use output::{ErrorItem, Status, envelope};

/// Serialises runs inside one process so that the on-disk lock protects
/// separate processes only and cargo tests can call [`run`] concurrently.
static PROCESS_LOCK: Mutex<()> = Mutex::new(());

/// Parses process arguments, initialises logging and runs the command.
///
/// Returns the process exit code.
#[must_use]
pub fn main_entry() -> u8 {
    main_entry_from(std::env::args_os())
}

/// Parses an explicit argument list, initialises logging and runs the command.
///
/// Splitting the entry point from [`main_entry`] keeps the Explorer path
/// handling testable and lets the binary forward Unicode paths unchanged
/// (FR-081).
#[must_use]
pub fn main_entry_from<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = normalize_arguments(args.into_iter().map(Into::into));
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code();
            let _ = error.print();

            return u8::try_from(code).unwrap_or(2);
        }
    };

    match &cli.log_level {
        Some(level) => {
            logging::init_with(level);
        }
        None => {
            logging::init();
        }
    }

    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut stdout = stdout.lock();
    let mut stderr = stderr.lock();

    run(cli, &mut stdout, &mut stderr)
}

/// Runs a parsed command and writes its output.
///
/// This function is the testable core of the binary: it never touches global
/// process state except for the active locale.
pub fn run(cli: Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> u8 {
    let command = cli.command.name();
    let started = Instant::now();
    let _process_guard = PROCESS_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _run_lock = match lock::acquire(cli.lock_timeout) {
        Ok(mut lock) => {
            let _ = lock::describe(lock.as_mut(), command);
            Some(lock)
        }
        Err(error) => {
            let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            emit_error(&cli, command, &error, duration_ms, stdout, stderr);
            return error.exit_code();
        }
    };
    let result = execute(&cli);
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    match result {
        Ok(outcome) => {
            emit_success(&cli, command, outcome, duration_ms, stdout, stderr);
            0
        }
        Err(error) => {
            emit_error(&cli, command, &error, duration_ms, stdout, stderr);
            error.exit_code()
        }
    }
}

/// Expands the bare-path form used by Explorer menu entries.
///
/// `freespace "D:\some folder"` is treated as `freespace scan "D:\some folder"`,
/// which lets the registry command stay simple. Arguments that are options or
/// known subcommands are left untouched, and Unicode characters are preserved
/// because the values stay [`OsString`] instances (FR-081).
#[must_use]
pub fn normalize_arguments(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    let mut args: Vec<OsString> = args.into_iter().collect();

    if args.len() != 2 {
        return args;
    }

    let candidate = &args[1];
    let text = candidate.to_string_lossy();
    if text.starts_with('-') {
        return args;
    }

    let path = Path::new(candidate);
    let looks_like_path = path.is_absolute()
        || path
            .components()
            .next()
            .is_some_and(|component| matches!(component, std::path::Component::Prefix(_)))
        || text.contains(['\\', '/'])
        || path.exists();
    if looks_like_path {
        args.insert(1, OsString::from("scan"));
    }

    args
}

fn execute(cli: &Cli) -> Result<Outcome> {
    if let Some(code) = &cli.lang {
        if i18n::set_locale_from_code(code).is_none() {
            return Err(Error::InvalidArgument(format!(
                "unknown language `{code}`; expected `ru` or `en`"
            )));
        }
    }

    match &cli.command {
        Command::Drives => {
            let settings = load_settings(cli)?;
            commands::drives::run(&settings)
        }
        Command::Capabilities => commands::capabilities::run(),
        Command::Config { action } => commands::config::run(cli.config.as_deref(), action),
        Command::Scan {
            paths,
            method,
            incremental,
            exclusions,
            snapshot_out,
            include_ads,
            size_mode,
        } => {
            let settings = load_settings(cli)?;
            commands::scan::run_scan_with(
                commands::scan::ScanManyArgs {
                    paths,
                    method,
                    incremental: *incremental,
                    exclusions,
                },
                settings.units,
                snapshot_out.as_deref(),
                *include_ads,
                size_mode,
            )
        }
        Command::Tree {
            paths,
            depth,
            min_size,
        } => {
            let settings = load_settings(cli)?;
            commands::scan::run_tree(paths, *depth, *min_size, settings.units)
        }
        Command::Top { path, limit } => {
            let settings = load_settings(cli)?;
            commands::top::run_top(path, *limit, settings.units)
        }
        Command::Types { path } => {
            let settings = load_settings(cli)?;
            commands::top::run_types(path, settings.units)
        }
        Command::Filter {
            path,
            kind,
            min_size,
            older_than,
            types,
            owner,
            min_resolution,
            min_bitrate,
            min_duration,
            metadata,
        } => {
            let settings = load_settings(cli)?;
            commands::filter::run(
                commands::filter::FilterArgs {
                    path,
                    kind: kind.as_deref(),
                    min_size: *min_size,
                    older_than: *older_than,
                    types: types.as_deref(),
                    owner: owner.as_deref(),
                    min_resolution: min_resolution.as_deref(),
                    min_bitrate: *min_bitrate,
                    min_duration: *min_duration,
                    metadata: metadata.as_deref(),
                },
                settings.units,
            )
        }
        Command::Owners { path } => {
            let settings = load_settings(cli)?;
            commands::owners::run(path, settings.units)
        }
        Command::Duplicates {
            path,
            min_size,
            algorithm,
            keep_rule,
        } => {
            let settings = load_settings(cli)?;
            let cache = freespace_ops::store::default_path()
                .ok()
                .and_then(|path| freespace_analysis::HashCache::open(&path).ok());
            commands::duplicates::run(
                commands::duplicates::DuplicatesArgs {
                    path,
                    min_size: *min_size,
                    algorithm: algorithm.as_deref(),
                    keep_rule: keep_rule.as_deref(),
                },
                settings.units,
                cache.as_ref(),
            )
        }
        Command::Cleanup { action } => {
            let settings = load_settings(cli)?;
            match action {
                cli::CleanupAction::Suggest { path, older_than } => {
                    commands::cleanup::run_suggest(path, *older_than, settings.units)
                }
                cli::CleanupAction::EmptyDirs { path } => commands::cleanup::run_empty_dirs(path),
            }
        }
        Command::Snapshot { action } => {
            let settings = load_settings(cli)?;
            match action {
                cli::SnapshotAction::Create { path, out } => {
                    commands::snapshot::create(path, out, settings.units)
                }
                cli::SnapshotAction::Diff {
                    base,
                    target,
                    export,
                } => commands::snapshot::diff(base, target, export.as_deref(), settings.units),
            }
        }
        Command::Export {
            path,
            format,
            out,
            include,
        } => {
            let settings = load_settings(cli)?;
            commands::export::run(
                commands::export::ExportArgs {
                    path,
                    format,
                    out,
                    include: include.as_deref(),
                },
                settings.units,
            )
        }
        Command::Archives { path, export } => {
            let settings = load_settings(cli)?;
            commands::archives::run(path, export.as_deref(), settings.units)
        }
        Command::Schedule { action } => match action {
            cli::ScheduleAction::Export { profile, out } => {
                commands::schedule::export(profile, out)
            }
        },
        Command::Delete {
            paths: targets,
            mode,
        } => {
            let settings = load_settings(cli)?;
            let mode = mode.as_deref().ok_or_else(|| {
                Error::InvalidArgument("`--mode recycle|permanent` is required".to_owned())
            })?;
            let path = freespace_ops::store::default_path()?;
            let store = freespace_ops::Store::open(&path)?;
            commands::delete::run(targets, mode, cli.yes, &store, settings.units)
        }
        Command::Shred {
            paths,
            passes,
            pattern,
        } => {
            let settings = load_settings(cli)?;
            let store = open_store(&settings);
            commands::shred::run(
                paths,
                *passes,
                pattern,
                cli.yes,
                store.as_ref(),
                settings.units,
            )
        }
        Command::Rename {
            paths: targets,
            regex,
            replacement,
            case,
            preview,
        } => {
            let settings = load_settings(cli)?;
            let store = if cli.yes && !*preview {
                open_store(&settings)
            } else {
                None
            };
            commands::rename::run(
                targets,
                regex,
                replacement,
                case.as_deref(),
                *preview,
                cli.yes,
                store.as_ref(),
            )
        }
        Command::Attrs {
            paths: targets,
            set,
            clear,
        } => {
            let settings = load_settings(cli)?;
            let store = open_store(&settings);
            commands::attrs::run_attrs(
                targets,
                set.as_deref(),
                clear.as_deref(),
                cli.yes,
                store.as_ref(),
            )
        }
        Command::Acl {
            paths: targets,
            owner,
            reset,
            copy_from,
        } => {
            let settings = load_settings(cli)?;
            let store = open_store(&settings);
            commands::attrs::run_acl(
                targets,
                owner.as_deref(),
                *reset,
                copy_from.as_deref(),
                cli.yes,
                store.as_ref(),
            )
        }
        Command::Unlock {
            path,
            list: _,
            soft,
            force,
        } => {
            let mode = if *soft {
                Some(freespace_ops::UnlockMode::Soft)
            } else if *force {
                Some(freespace_ops::UnlockMode::Force)
            } else {
                None
            };
            let settings = load_settings(cli)?;
            let store = if mode.is_some() {
                open_store(&settings)
            } else {
                None
            };
            commands::unlock::run(path, mode, cli.yes, store.as_ref())
        }
        Command::Monitor {
            threshold,
            interval,
            iterations,
            root,
            no_etw,
        } => {
            let settings = load_settings(cli)?;
            commands::monitor::run(
                &settings,
                commands::monitor::MonitorArgs {
                    threshold: *threshold,
                    interval: *interval,
                    iterations: *iterations,
                    root: root.as_deref(),
                    no_etw: *no_etw,
                    json: cli.json,
                },
            )
        }
        Command::ContextMenu { action } => {
            commands::context_menu::run(action, None, cli.config.as_deref())
        }
    }
}

fn open_store(_settings: &Settings) -> Option<freespace_ops::Store> {
    freespace_ops::store::default_path()
        .ok()
        .and_then(|path| freespace_ops::Store::open(&path).ok())
}

fn load_settings(cli: &Cli) -> Result<Settings> {
    match &cli.config {
        Some(path) => Settings::load(path),
        None => match settings::default_path() {
            Ok(path) => Settings::load(&path),
            Err(_) => Ok(Settings::default()),
        },
    }
}

fn emit_success(
    cli: &Cli,
    command: &str,
    outcome: Outcome,
    duration_ms: u64,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) {
    if let Some(raw) = &outcome.raw {
        let _ = write!(stdout, "{raw}");
        if !raw.ends_with('\n') {
            let _ = writeln!(stdout);
        }
    } else if cli.json {
        let value = envelope(
            command,
            Status::Ok,
            outcome.data,
            &outcome.warnings,
            &[],
            duration_ms,
        );
        write_json(stdout, &value);
    } else if !cli.quiet {
        let _ = write!(stdout, "{}", outcome.human);
        if !outcome.human.ends_with('\n') {
            let _ = writeln!(stdout);
        }
    }

    for warning in &outcome.warnings {
        let _ = writeln!(stderr, "warning: {warning}");
    }
}

fn emit_error(
    cli: &Cli,
    command: &str,
    error: &Error,
    duration_ms: u64,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) {
    let item = ErrorItem::from_error(error);

    if cli.json {
        let value = envelope(
            command,
            Status::Error,
            Value::Null,
            &[],
            &[item],
            duration_ms,
        );
        write_json(stdout, &value);
    } else {
        let _ = writeln!(stderr, "error[{}]: {}", item.code, item.message);
    }
}

fn write_json(writer: &mut dyn Write, value: &Value) {
    if serde_json::to_writer(&mut *writer, value).is_ok() {
        let _ = writeln!(writer);
    }
}

/// Parses arguments and runs them against in-memory buffers; used by tests.
#[cfg(test)]
fn run_args(args: &[&str]) -> (u8, String, String) {
    let cli = Cli::try_parse_from(args).expect("parse arguments");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run(cli, &mut stdout, &mut stderr);

    (
        code,
        String::from_utf8(stdout).expect("stdout is UTF-8"),
        String::from_utf8(stderr).expect("stderr is UTF-8"),
    )
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use serde_json::Value;
    use std::ffi::OsString;

    use super::run_args;

    #[test]
    fn drives_emits_a_valid_json_envelope() {
        let (code, stdout, stderr) = run_args(&["freespace", "drives", "--json"]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let value: Value = serde_json::from_str(&stdout).expect("valid JSON");
        assert_eq!(value["version"], "1");
        assert_eq!(value["command"], "drives");
        assert_eq!(value["status"], "ok");
        assert!(value["errors"].as_array().expect("errors").is_empty());
        assert!(
            !value["data"]["items"].as_array().expect("items").is_empty(),
            "at least one drive is reported"
        );
    }

    #[test]
    fn drives_emits_human_output() {
        let (code, stdout, stderr) = run_args(&["freespace", "drives"]);

        assert_eq!(code, 0);
        assert!(stdout.contains("free of"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn capabilities_reports_the_current_platform() {
        let (code, stdout, stderr) = run_args(&["freespace", "capabilities", "--json"]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let value: Value = serde_json::from_str(&stdout).expect("valid JSON");
        assert_eq!(value["command"], "capabilities");
        assert_eq!(value["status"], "ok");
        assert_eq!(value["data"]["permanent"], true);

        if cfg!(windows) {
            assert_eq!(value["data"]["platform"], "windows");
            assert_eq!(value["data"]["scan_methods"]["mft"], true);
            assert_eq!(value["data"]["shred"], true);
        } else if cfg!(target_os = "linux") {
            assert_eq!(value["data"]["platform"], "linux");
            assert_eq!(value["data"]["scan_methods"]["mft"], false);
            assert_eq!(value["data"]["case_sensitive"], true);
            assert_eq!(value["data"]["max_path"], 4096);
        }
    }

    #[test]
    fn capabilities_emits_human_output() {
        let (code, stdout, stderr) = run_args(&["freespace", "capabilities"]);

        assert_eq!(code, 0);
        assert!(stdout.contains("platform: "));
        assert!(stderr.is_empty());
    }

    #[test]
    fn quiet_suppresses_human_output() {
        let (code, stdout, _stderr) = run_args(&["freespace", "drives", "--quiet"]);

        assert_eq!(code, 0);
        assert!(stdout.is_empty());
    }

    #[test]
    fn config_set_and_get_round_trip() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("settings.toml");
        let path = path.to_string_lossy().into_owned();

        let (code, stdout, _) = run_args(&[
            "freespace",
            "config",
            "set",
            "language",
            "ru",
            "--config",
            &path,
            "--json",
        ]);
        assert_eq!(code, 0);
        let value: Value = serde_json::from_str(&stdout).expect("valid JSON");
        assert_eq!(value["data"]["value"], "ru");

        let (code, stdout, _) = run_args(&[
            "freespace",
            "config",
            "get",
            "language",
            "--config",
            &path,
            "--json",
        ]);
        assert_eq!(code, 0);
        let value: Value = serde_json::from_str(&stdout).expect("valid JSON");
        assert_eq!(value["data"]["value"], "ru");
    }

    #[test]
    fn monitor_runs_a_bounded_fallback_session() {
        let (code, stdout, stderr) = run_args(&[
            "freespace",
            "monitor",
            "--iterations",
            "1",
            "--interval",
            "0",
            "--no-etw",
            "--json",
        ]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
            let value: Value = serde_json::from_str(line).expect("JSON line");
            assert!(value["type"].is_string());
        }
    }

    #[test]
    fn unknown_config_keys_return_exit_code_2() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("settings.toml");
        let path = path.to_string_lossy().into_owned();
        let (code, stdout, _) = run_args(&[
            "freespace",
            "config",
            "get",
            "unknown",
            "--config",
            &path,
            "--json",
        ]);

        assert_eq!(code, 2);
        let value: Value = serde_json::from_str(&stdout).expect("valid JSON");
        assert_eq!(value["errors"][0]["code"], "E_INVALID_ARGUMENT");
    }

    #[test]
    fn invalid_language_is_rejected() {
        let (code, stdout, _) = run_args(&["freespace", "drives", "--json", "--lang", "de"]);

        assert_eq!(code, 2);
        let value: Value = serde_json::from_str(&stdout).expect("valid JSON");
        assert_eq!(value["errors"][0]["code"], "E_INVALID_ARGUMENT");
    }

    #[test]
    fn destructive_commands_parse_their_contract_arguments() {
        let parsed = super::Cli::try_parse_from([
            "freespace",
            "shred",
            "secret.txt",
            "--passes",
            "3",
            "--pattern",
            "random",
            "--yes",
            "--json",
        ])
        .expect("parse shred");

        assert!(parsed.json);
        assert!(parsed.yes);
    }

    #[test]
    fn expands_the_bare_path_form_for_explorer() {
        let args = super::normalize_arguments([
            OsString::from("freespace"),
            OsString::from(r"D:\Some Folder\Тест"),
        ]);

        assert_eq!(args[1], OsString::from("scan"));
        assert_eq!(args[2], OsString::from(r"D:\Some Folder\Тест"));
    }

    #[test]
    fn keeps_known_subcommands_and_options_untouched() {
        for invocation in [
            vec!["freespace", "drives"],
            vec!["freespace", "--json"],
            vec!["freespace", "scan"],
            vec!["freespace", "drives", "--json"],
        ] {
            let expected: Vec<OsString> = invocation.iter().map(OsString::from).collect();
            let args = super::normalize_arguments(expected.clone());
            assert_eq!(args, expected, "unexpected rewrite");
        }
    }

    #[test]
    fn expands_relative_paths_containing_a_separator() {
        let args = super::normalize_arguments([
            OsString::from("freespace"),
            OsString::from(r"fixtures\long path"),
        ]);

        assert_eq!(args[1], OsString::from("scan"));
    }
}
