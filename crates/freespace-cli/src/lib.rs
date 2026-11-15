#![forbid(unsafe_code)]

//! FreeSpace command line interface.
//!
//! The crate exposes the `clap` command tree, the JSON envelope and the
//! documented exit codes; [`main_entry`] is the thin process entry point.

pub mod cli;
pub mod commands;
pub mod output;

use std::{
    io::{self, Write},
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

/// Parses process arguments, initialises logging and runs the command.
///
/// Returns the process exit code.
#[must_use]
pub fn main_entry() -> u8 {
    let cli = match Cli::try_parse() {
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
        Command::Config { action } => commands::config::run(cli.config.as_deref(), action),
        Command::Scan {
            path,
            method,
            incremental,
            exclusions,
            snapshot_out,
        } => {
            let settings = load_settings(cli)?;
            let mut outcome = commands::scan::run_scan(
                commands::scan::ScanArgs {
                    path,
                    method,
                    incremental: *incremental,
                    exclusions,
                },
                settings.units,
            )?;
            if snapshot_out.is_some() {
                outcome
                    .warnings
                    .push("snapshot output arrives with user story 8".to_owned());
            }
            Ok(outcome)
        }
        Command::Tree {
            path,
            depth,
            min_size,
        } => {
            let settings = load_settings(cli)?;
            commands::scan::run_tree(path, *depth, *min_size, settings.units)
        }
        Command::Top { path, limit } => {
            let settings = load_settings(cli)?;
            commands::top::run_top(path, *limit, settings.units)
        }
        Command::Types { path } => {
            let settings = load_settings(cli)?;
            commands::top::run_types(path, settings.units)
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
        Command::Shred {
            paths,
            passes,
            pattern,
        } => {
            let settings = load_settings(cli)?;
            let store = freespace_ops::store::default_path()
                .ok()
                .and_then(|path| freespace_ops::Store::open(&path).ok());
            commands::shred::run(
                paths,
                *passes,
                pattern,
                cli.yes,
                store.as_ref(),
                settings.units,
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
            let store = if mode.is_some() {
                freespace_ops::store::default_path()
                    .ok()
                    .and_then(|path| freespace_ops::Store::open(&path).ok())
            } else {
                None
            };
            commands::unlock::run(path, mode, cli.yes, store.as_ref())
        }
        command => Err(Error::Unsupported(format!(
            "`{}` is not implemented yet",
            command.name()
        ))),
    }
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
    if cli.json {
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
    fn unsupported_commands_return_exit_code_8() {
        let (code, stdout, stderr) = run_args(&["freespace", "filter", "C:\\", "--json"]);

        assert_eq!(code, 8);
        assert!(stderr.is_empty());
        let value: Value = serde_json::from_str(&stdout).expect("valid JSON");
        assert_eq!(value["status"], "error");
        assert_eq!(value["errors"][0]["code"], "E_UNSUPPORTED");
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
}
