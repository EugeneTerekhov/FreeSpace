//! `clap` command tree for the FreeSpace CLI.
//!
//! The shape follows `specs/001-freespace-disk-analyzer/contracts/cli.md`.
//! Commands that arrive with later user stories parse their full argument set
//! but currently return `E_UNSUPPORTED`; `drives`, `config`, `scan`, `tree`,
//! `top`, `types`, `filter`, `duplicates`, `owners`, `cleanup`, `unlock`,
//! `delete`, `shred`, `rename`, `attrs`, `acl`, `snapshot`, `export`,
//! `archives` and `schedule` are functional.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// FreeSpace command line interface.
#[derive(Debug, Parser)]
#[command(
    name = "freespace",
    version,
    about = "Analyze and reclaim disk space",
    long_about = "FreeSpace analyzes and reclaims disk space on Windows. Every GUI capability is \
                  available from the command line with a machine-readable JSON envelope."
)]
pub struct Cli {
    /// Emit the machine-readable JSON envelope.
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress human-readable output.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Confirm destructive operations non-interactively.
    #[arg(long = "yes", global = true)]
    pub yes: bool,

    /// Disable ANSI colours.
    #[arg(long = "no-color", global = true)]
    pub no_color: bool,

    /// Interface language (`ru` or `en`).
    #[arg(long, global = true, value_name = "ru|en")]
    pub lang: Option<String>,

    /// Use an explicit configuration file.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Diagnostic verbosity.
    #[arg(
        long = "log-level",
        global = true,
        value_name = "error|warn|info|debug"
    )]
    pub log_level: Option<String>,

    /// Seconds to wait for another FreeSpace process before failing.
    #[arg(long = "lock-timeout", global = true, default_value_t = 30)]
    pub lock_timeout: u64,

    /// Command to execute.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List logical drives with sizes and file systems.
    Drives,

    /// Describe the capabilities available on the current platform.
    Capabilities,

    /// Read and write configuration values.
    Config {
        /// Configuration action.
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Scan one or more volumes or directories and build a size tree.
    Scan {
        /// Volumes or directories to analyze.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Scan method: `auto`, `mft` or `walk`.
        #[arg(long, default_value = "auto")]
        method: String,
        /// Update a previous NTFS scan through the USN journal.
        #[arg(long)]
        incremental: bool,
        /// Glob pattern excluded from the analysis.
        #[arg(long = "exclude")]
        exclusions: Vec<String>,
        /// Write a snapshot after scanning.
        #[arg(long = "snapshot-out")]
        snapshot_out: Option<PathBuf>,
        /// Enumerate NTFS alternate data streams.
        #[arg(long = "include-ads")]
        include_ads: bool,
        /// Size dimension to report: `logical`, `on-disk` or `physical`.
        #[arg(long = "size-mode", default_value = "logical")]
        size_mode: String,
    },

    /// Print the size tree for one or more paths.
    Tree {
        /// Volumes or directories.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Maximum depth to print.
        #[arg(long)]
        depth: Option<u32>,
        /// Hide nodes smaller than this many bytes.
        #[arg(long = "min-size")]
        min_size: Option<u64>,
    },

    /// List the largest files.
    Top {
        /// Volume or directory.
        path: PathBuf,
        /// Number of files to print.
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },

    /// Group occupied space by extension and category.
    Types {
        /// Volume or directory.
        path: PathBuf,
    },

    /// Filter tree nodes by attributes.
    Filter {
        /// Volume or directory.
        path: PathBuf,
        /// Object kind: `file`, `dir` or `link`.
        #[arg(long)]
        kind: Option<String>,
        /// Minimum size in bytes.
        #[arg(long = "min-size")]
        min_size: Option<u64>,
        /// Last modified at least this many days ago.
        #[arg(long = "older-than")]
        older_than: Option<u32>,
        /// Comma-separated extension list.
        #[arg(long = "type")]
        types: Option<String>,
        /// Owner account.
        #[arg(long)]
        owner: Option<String>,
        /// Minimum image or video resolution (`WIDTHxHEIGHT`).
        #[arg(long = "min-resolution")]
        min_resolution: Option<String>,
        /// Minimum audio bitrate in kbps.
        #[arg(long = "min-bitrate")]
        min_bitrate: Option<u32>,
        /// Minimum audio or video duration in seconds.
        #[arg(long = "min-duration")]
        min_duration: Option<u64>,
        /// Comma-separated metadata families (`exif`, `audio`, `video`, `pe`).
        #[arg(long = "metadata")]
        metadata: Option<String>,
    },

    /// Find duplicate files.
    Duplicates {
        /// Volume or directory.
        path: PathBuf,
        /// Ignore files smaller than this many bytes.
        #[arg(long = "min-size")]
        min_size: Option<u64>,
        /// Hash algorithm: `sha256` or `md5`.
        #[arg(long)]
        algorithm: Option<String>,
        /// Rule for the copy that is kept.
        #[arg(long = "keep-rule")]
        keep_rule: Option<String>,
    },

    /// Suggest cleanup candidates or find empty directories.
    Cleanup {
        /// Cleanup action.
        #[command(subcommand)]
        action: CleanupAction,
    },

    /// Inspect and release file locks (Windows only).
    Unlock {
        /// Locked file.
        path: PathBuf,
        /// List locking processes (default).
        #[arg(long)]
        list: bool,
        /// Ask applications to close the file gracefully.
        #[arg(long)]
        soft: bool,
        /// Terminate the locking processes after confirmation.
        #[arg(long)]
        force: bool,
    },

    /// Delete objects through the Recycle Bin or permanently.
    Delete {
        /// Objects to delete.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Deletion mode: `recycle` or `permanent`.
        #[arg(long)]
        mode: Option<String>,
    },

    /// Overwrite and remove files (Windows only).
    Shred {
        /// Files to shred.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Number of overwrite passes.
        #[arg(long, default_value_t = 1)]
        passes: u32,
        /// Overwrite pattern: `zeros` or `random`.
        #[arg(long, default_value = "zeros")]
        pattern: String,
    },

    /// Rename objects by a regular expression.
    Rename {
        /// Objects to rename.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Regular expression matched against file names.
        #[arg(long)]
        regex: String,
        /// Replacement pattern.
        #[arg(long)]
        replacement: String,
        /// Case transformation: `lower`, `upper` or `preserve`.
        #[arg(long)]
        case: Option<String>,
        /// Only show the planned renames.
        #[arg(long)]
        preview: bool,
    },

    /// Set or clear file attributes (Windows only).
    Attrs {
        /// Objects to update.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Comma-separated attributes to set.
        #[arg(long)]
        set: Option<String>,
        /// Comma-separated attributes to clear.
        #[arg(long)]
        clear: Option<String>,
    },

    /// Change NTFS ownership or permissions (Windows only).
    Acl {
        /// Objects to update.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// New owner account.
        #[arg(long)]
        owner: Option<String>,
        /// Reset permissions to inherited defaults.
        #[arg(long)]
        reset: bool,
        /// Copy permissions from another object.
        #[arg(long = "copy-from")]
        copy_from: Option<PathBuf>,
    },

    /// Summarise occupied space by file owner.
    Owners {
        /// Volume or directory.
        path: PathBuf,
    },

    /// Create or compare disk snapshots.
    Snapshot {
        /// Snapshot action.
        #[command(subcommand)]
        action: SnapshotAction,
    },

    /// Export an analysis report.
    Export {
        /// Volume or directory.
        path: PathBuf,
        /// Report format: `csv`, `xml`, `html` or `xlsx`.
        #[arg(long)]
        format: String,
        /// Output file.
        #[arg(long)]
        out: PathBuf,
        /// Comma-separated report sections.
        #[arg(long)]
        include: Option<String>,
    },

    /// Inspect archive contents without unpacking.
    Archives {
        /// Volume, directory or archive.
        path: PathBuf,
        /// Export the content list to a file (`.csv`, `.json`, `.xml`, `.html`).
        #[arg(long)]
        export: Option<PathBuf>,
    },

    /// Monitor disk writes in real time (JSON Lines).
    Monitor {
        /// Notification threshold in bytes.
        #[arg(long)]
        threshold: Option<u64>,
        /// Polling interval in seconds.
        #[arg(long)]
        interval: Option<u64>,
        /// Number of batches to report; `0` runs until interrupted.
        #[arg(long, default_value_t = 0)]
        iterations: u32,
        /// Volume or directory whose USN journal supplies path hints.
        #[arg(long)]
        root: Option<PathBuf>,
        /// Skip the ETW session and use process counters only.
        #[arg(long = "no-etw")]
        no_etw: bool,
    },

    /// Install or remove Explorer context-menu entries (Windows only).
    ContextMenu {
        /// Integration action.
        #[command(subcommand)]
        action: ContextMenuAction,
    },

    /// Export a Task Scheduler definition.
    Schedule {
        /// Scheduling action.
        #[command(subcommand)]
        action: ScheduleAction,
    },
}

impl Command {
    /// Stable command name used in the JSON envelope.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Drives => "drives",
            Self::Capabilities => "capabilities",
            Self::Config { .. } => "config",
            Self::Scan { .. } => "scan",
            Self::Tree { .. } => "tree",
            Self::Top { .. } => "top",
            Self::Types { .. } => "types",
            Self::Filter { .. } => "filter",
            Self::Duplicates { .. } => "duplicates",
            Self::Cleanup { .. } => "cleanup",
            Self::Unlock { .. } => "unlock",
            Self::Delete { .. } => "delete",
            Self::Shred { .. } => "shred",
            Self::Rename { .. } => "rename",
            Self::Attrs { .. } => "attrs",
            Self::Acl { .. } => "acl",
            Self::Owners { .. } => "owners",
            Self::Snapshot { .. } => "snapshot",
            Self::Export { .. } => "export",
            Self::Archives { .. } => "archives",
            Self::Monitor { .. } => "monitor",
            Self::ContextMenu { .. } => "context-menu",
            Self::Schedule { .. } => "schedule",
        }
    }
}

/// `config` subcommands.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print a configuration value.
    Get {
        /// Configuration key.
        key: String,
    },
    /// Set a configuration value.
    Set {
        /// Configuration key.
        key: String,
        /// New value.
        value: String,
    },
}

/// `cleanup` subcommands.
#[derive(Debug, Subcommand)]
pub enum CleanupAction {
    /// Suggest cleanup candidates.
    Suggest {
        /// Volume or directory.
        path: PathBuf,
        /// Only suggest objects older than this many days.
        #[arg(long = "older-than")]
        older_than: Option<u32>,
    },
    /// List empty directories, including nested ones.
    EmptyDirs {
        /// Volume or directory.
        path: PathBuf,
    },
}

/// `snapshot` subcommands.
#[derive(Debug, Subcommand)]
pub enum SnapshotAction {
    /// Create a snapshot.
    Create {
        /// Volume or directory.
        path: PathBuf,
        /// Output file.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compare two snapshots.
    Diff {
        /// Base snapshot.
        base: PathBuf,
        /// Target snapshot.
        target: PathBuf,
        /// Optional report file.
        #[arg(long)]
        export: Option<PathBuf>,
    },
}

/// `context-menu` subcommands.
#[derive(Debug, Subcommand)]
pub enum ContextMenuAction {
    /// Install the integration.
    Install {
        /// Install for all users (requires elevation).
        #[arg(long)]
        machine: bool,
        /// Executable registered in the menu; defaults to the running binary.
        #[arg(long, value_name = "PATH")]
        executable: Option<PathBuf>,
    },
    /// Remove the integration.
    Uninstall {
        /// Remove the machine-wide integration.
        #[arg(long)]
        machine: bool,
        /// Remove both the current-user and machine-wide integrations.
        #[arg(long)]
        all: bool,
    },
}

/// `schedule` subcommands.
#[derive(Debug, Subcommand)]
pub enum ScheduleAction {
    /// Export a Task Scheduler definition.
    Export {
        /// Profile name.
        profile: String,
        /// Output XML file.
        #[arg(long)]
        out: PathBuf,
    },
}
