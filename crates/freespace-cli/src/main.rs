#![forbid(unsafe_code)]

//! FreeSpace CLI binary entry point.
//!
//! Arguments are collected as [`std::ffi::OsString`] so that Explorer can pass
//! paths containing spaces, Unicode characters or long verbatim prefixes without
//! an intermediate lossy conversion (FR-081).

use std::process::ExitCode;

fn main() -> ExitCode {
    let args = std::env::args_os().collect::<Vec<_>>();

    ExitCode::from(freespace_cli::main_entry_from(args))
}
