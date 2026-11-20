//! US18: JSON envelopes, documented exit codes and non-interactive behaviour.

mod common;

use std::{io::Write as _, path::Path};

use clap::Parser;
use freespace_cli::cli::Cli;
use serde_json::Value;
use zip::write::SimpleFileOptions;

fn run(args: &[&str]) -> (u8, String, String) {
    let cli = Cli::try_parse_from(args).expect("parse arguments");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = freespace_cli::run(cli, &mut stdout, &mut stderr);
    (
        code,
        String::from_utf8(stdout).expect("stdout UTF-8"),
        String::from_utf8(stderr).expect("stderr UTF-8"),
    )
}

fn json(args: &[&str]) -> (u8, Value) {
    let (code, stdout, _stderr) = run(args);
    let value: Value = serde_json::from_str(&stdout).expect("valid JSON envelope");

    (code, value)
}

fn zip(path: &Path) {
    let file = std::fs::File::create(path).expect("create zip");
    let mut writer = zip::ZipWriter::new(file);
    writer
        .start_file("payload.bin", SimpleFileOptions::default())
        .expect("entry");
    writer.write_all(&[0u8; 32]).expect("write");
    writer.finish().expect("finish");
}

#[test]
fn read_only_commands_emit_documented_envelopes() {
    let fixture = common::TestTree::new();
    fixture.write("a.txt", &[0u8; 10]);
    let root = fixture.root().to_string_lossy().into_owned();

    for (command, args) in [
        ("scan", vec!["freespace", "scan", root.as_str(), "--json"]),
        ("tree", vec!["freespace", "tree", root.as_str(), "--json"]),
        ("top", vec!["freespace", "top", root.as_str(), "--json"]),
        ("types", vec!["freespace", "types", root.as_str(), "--json"]),
        (
            "duplicates",
            vec!["freespace", "duplicates", root.as_str(), "--json"],
        ),
        (
            "owners",
            vec!["freespace", "owners", root.as_str(), "--json"],
        ),
        (
            "filter",
            vec!["freespace", "filter", root.as_str(), "--json"],
        ),
    ] {
        let (code, value) = json(&args);
        assert_eq!(code, 0, "{command}: {}", value["errors"]);
        assert_eq!(value["version"], "1");
        assert_eq!(value["command"], command);
        assert_eq!(value["status"], "ok");
        assert!(value["data"].is_object());
        assert!(value["errors"].as_array().expect("errors").is_empty());
        assert!(value["duration_ms"].as_u64().is_some());
    }
}

#[test]
fn export_archives_and_snapshot_produce_files() {
    let fixture = common::TestTree::new();
    fixture.write("a.txt", &[0u8; 10]);
    let archive = fixture.path("sample.zip");
    zip(&archive);
    let output = tempfile::tempdir().expect("output");

    let report = output.path().join("report.csv");
    let (code, value) = json(&[
        "freespace",
        "export",
        fixture.root().to_str().expect("path"),
        "--format",
        "csv",
        "--out",
        report.to_str().expect("path"),
        "--include",
        "tree,top,types",
        "--json",
    ]);
    assert_eq!(code, 0, "{}", value["errors"]);
    assert!(report.exists());

    let list = output.path().join("archives.json");
    let (code, value) = json(&[
        "freespace",
        "archives",
        archive.to_str().expect("path"),
        "--export",
        list.to_str().expect("path"),
        "--json",
    ]);
    assert_eq!(code, 0, "{}", value["errors"]);
    assert_eq!(value["data"]["count"], 1);
    assert!(list.exists());

    let snapshot = output.path().join("disk.fssnap");
    let (code, value) = json(&[
        "freespace",
        "snapshot",
        "create",
        fixture.root().to_str().expect("path"),
        "--out",
        snapshot.to_str().expect("path"),
        "--json",
    ]);
    assert_eq!(code, 0, "{}", value["errors"]);
    assert!(snapshot.exists());
}

#[test]
fn schedule_export_writes_utf16_xml() {
    let output = tempfile::tempdir().expect("output");
    let path = output.path().join("daily.xml");
    let (code, value) = json(&[
        "freespace",
        "schedule",
        "export",
        "daily-scan",
        "--out",
        path.to_str().expect("path"),
        "--json",
    ]);

    assert_eq!(code, 0, "{}", value["errors"]);
    assert_eq!(value["data"]["profile"], "daily-scan");
    let bytes = std::fs::read(&path).expect("read");
    assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
}

#[test]
fn destructive_commands_require_confirmation_and_return_code_5() {
    let fixture = common::TestTree::new();
    let file = fixture.write("keep.bin", b"payload");

    let (code, value) = json(&[
        "freespace",
        "delete",
        file.to_str().expect("path"),
        "--mode",
        "permanent",
        "--json",
    ]);

    assert_eq!(code, 5);
    assert_eq!(value["status"], "error");
    assert_eq!(value["errors"][0]["code"], "E_CONFIRMATION_REQUIRED");
    assert!(file.exists(), "nothing may change without --yes");
}

#[test]
fn invalid_arguments_use_documented_codes() {
    let (code, value) = json(&["freespace", "config", "get", "unknown", "--json"]);
    assert_eq!(code, 2);
    assert_eq!(value["errors"][0]["code"], "E_INVALID_ARGUMENT");

    let (code, stdout, stderr) = run(&[
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
        let value: Value = serde_json::from_str(line).expect("every monitor line is JSON");
        assert!(value["type"].is_string());
    }

    let (code, value) = json(&[
        "freespace",
        "export",
        "C:\\",
        "--format",
        "pdf",
        "--out",
        "report.pdf",
        "--json",
    ]);
    assert_eq!(code, 2);
    assert_eq!(value["errors"][0]["code"], "E_INVALID_ARGUMENT");
}

#[cfg(unix)]
#[test]
fn windows_only_commands_report_e_unsupported() {
    let fixture = common::TestTree::new();
    let file = fixture.write("secret.bin", b"payload");
    let file = file.to_str().expect("path");

    for args in [
        vec!["freespace", "shred", file, "--yes", "--json"],
        vec![
            "freespace",
            "attrs",
            file,
            "--set",
            "hidden",
            "--yes",
            "--json",
        ],
        vec!["freespace", "acl", file, "--reset", "--yes", "--json"],
        vec!["freespace", "unlock", file, "--json"],
        vec!["freespace", "context-menu", "install", "--json"],
    ] {
        let (code, value) = json(&args);
        assert_eq!(code, 8, "{}: {}", args[1], value["errors"]);
        assert_eq!(value["status"], "error");
        assert_eq!(value["errors"][0]["code"], "E_UNSUPPORTED");
    }
    assert!(Path::new(file).exists());
}

#[test]
fn owners_and_filter_share_the_same_accounts() {
    let fixture = common::TestTree::new();
    fixture.write("a.bin", &[0u8; 10]);

    let (code, owners) = json(&[
        "freespace",
        "owners",
        fixture.root().to_str().expect("path"),
        "--json",
    ]);
    assert_eq!(code, 0);
    let account = owners["data"]["items"][0]["display_name"]
        .as_str()
        .expect("account");

    let (code, filtered) = json(&[
        "freespace",
        "filter",
        fixture.root().to_str().expect("path"),
        "--owner",
        account,
        "--json",
    ]);
    assert_eq!(code, 0);
    assert_eq!(filtered["data"]["count"].as_u64(), Some(1));
    assert_eq!(filtered["data"]["items"][0]["owner"], account);
}
