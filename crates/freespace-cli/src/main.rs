#![forbid(unsafe_code)]

//! FreeSpace CLI binary entry point.

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(freespace_cli::main_entry())
}
