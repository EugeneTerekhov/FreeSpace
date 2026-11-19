//! US16: ZIP, 7z and TAR inspection, limits and error handling.

mod common;

use std::{
    io::{Cursor, Write as _},
    path::Path,
};

use freespace_analysis::{ArchiveFormat, ArchiveLimits, find_archives, inspect};
use freespace_core::size::Units;
use zip::write::SimpleFileOptions;

fn zip_archive(fixture: &common::TestTree) -> std::path::PathBuf {
    let path = fixture.path("sample.zip");
    let file = std::fs::File::create(&path).expect("create");
    let mut writer = zip::ZipWriter::new(file);
    writer
        .start_file("docs/readme.txt", SimpleFileOptions::default())
        .expect("entry");
    writer.write_all(b"hello").expect("write");
    writer
        .start_file("payload.bin", SimpleFileOptions::default())
        .expect("entry");
    writer.write_all(&[0u8; 1024]).expect("write");
    writer.finish().expect("finish");

    path
}

fn tar_archive(fixture: &common::TestTree, gzip: bool) -> std::path::PathBuf {
    let path = fixture.path(if gzip { "backup.tar.gz" } else { "backup.tar" });
    let mut builder = tar::Builder::new(Vec::new());
    let payload = b"tar payload";
    let mut header = tar::Header::new_gnu();
    header.set_size(u64::try_from(payload.len()).expect("size"));
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, "nested/payload.txt", &payload[..])
        .expect("append");
    let bytes = builder.into_inner().expect("finish");

    if gzip {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&bytes).expect("gzip");
        let bytes = encoder.finish().expect("gzip finish");
        std::fs::write(&path, bytes).expect("write");
    } else {
        std::fs::write(&path, bytes).expect("write");
    }

    path
}

fn seven_zip_archive(fixture: &common::TestTree) -> std::path::PathBuf {
    let path = fixture.path("sample.7z");
    let mut writer = sevenz_rust::SevenZWriter::new(Cursor::new(Vec::new())).expect("writer");
    let mut entry = sevenz_rust::SevenZArchiveEntry::new();
    entry.name = "inner/data.txt".to_owned();
    entry.has_stream = true;
    entry.size = 7;
    writer
        .push_archive_entry(entry, Some(Cursor::new(b"7z data".to_vec())))
        .expect("entry");
    let cursor = writer.finish().expect("finish");
    std::fs::write(&path, cursor.into_inner()).expect("write");

    path
}

#[test]
fn lists_zip_contents_with_sizes_and_shares() {
    let fixture = common::TestTree::new();
    let path = zip_archive(&fixture);

    let inspection = inspect(&path, ArchiveLimits::default()).expect("inspect");

    assert_eq!(inspection.format, ArchiveFormat::Zip);
    assert_eq!(inspection.entries.len(), 2);
    assert_eq!(inspection.total_size, 1029);
    assert!(
        inspection
            .entries
            .iter()
            .any(|entry| entry.internal_path == "payload.bin" && entry.size == 1024)
    );
    let largest = inspection.largest(1);
    assert_eq!(largest[0].internal_path, "payload.bin");
    assert!((largest[0].share - 1024.0 / 1029.0).abs() < 1e-9);
}

#[test]
fn lists_tar_tar_gz_and_7z_contents() {
    let fixture = common::TestTree::new();

    let tar = inspect(&tar_archive(&fixture, false), ArchiveLimits::default()).expect("tar");
    assert_eq!(tar.format, ArchiveFormat::Tar);
    assert_eq!(tar.entries[0].internal_path, "nested/payload.txt");
    assert_eq!(tar.entries[0].size, 11);

    let tar_gz = inspect(&tar_archive(&fixture, true), ArchiveLimits::default()).expect("tar.gz");
    assert_eq!(tar_gz.format, ArchiveFormat::TarGz);
    assert_eq!(tar_gz.total_size, 11);

    let seven = inspect(&seven_zip_archive(&fixture), ArchiveLimits::default()).expect("7z");
    assert_eq!(seven.format, ArchiveFormat::SevenZip);
    assert_eq!(seven.entries[0].internal_path, "inner/data.txt");
    assert_eq!(seven.entries[0].size, 7);
}

#[test]
fn rejects_corrupted_and_unsupported_archives() {
    let fixture = common::TestTree::new();
    let broken_zip = fixture.write("broken.zip", b"PK\x03\x04 not a real archive");
    let broken_seven = fixture.write("broken.7z", b"7z\xBC\xAF\x27\x1C broken");
    let text = fixture.write("notes.txt", b"plain text");

    assert!(inspect(&broken_zip, ArchiveLimits::default()).is_err());
    assert!(inspect(&broken_seven, ArchiveLimits::default()).is_err());
    let unsupported = inspect(&text, ArchiveLimits::default()).expect_err("unsupported");
    assert_eq!(unsupported.code().as_str(), "E_UNSUPPORTED");
    assert!(ArchiveFormat::for_path(Path::new("archive.tar.xz")).is_none());
}

#[test]
fn enforces_entry_limits() {
    let fixture = common::TestTree::new();
    let path = zip_archive(&fixture);
    let limits = ArchiveLimits {
        max_entries: 1,
        ..ArchiveLimits::default()
    };

    let error = inspect(&path, limits).expect_err("too many entries");
    assert_eq!(error.code().as_str(), "E_UNSUPPORTED");
}

#[test]
fn finds_archives_in_a_directory() {
    let fixture = common::TestTree::new();
    zip_archive(&fixture);
    tar_archive(&fixture, false);
    fixture.write("notes.txt", b"text");

    let found = find_archives(fixture.root(), 10);

    assert_eq!(found.len(), 2);
    assert!(found.iter().any(|path| path.ends_with("sample.zip")));
}

#[test]
fn the_cli_exports_the_content_list() {
    let fixture = common::TestTree::new();
    let archive = zip_archive(&fixture);
    let output = fixture.path("archive-list.csv");

    let outcome = freespace_cli::commands::archives::run(&archive, Some(&output), Units::Binary)
        .expect("archives");

    assert_eq!(outcome.data["count"].as_u64(), Some(1));
    assert!(output.exists());
    let text = std::fs::read_to_string(&output).expect("read");
    assert!(text.contains("payload.bin"));
    assert!(text.contains("internal_path"));
}
