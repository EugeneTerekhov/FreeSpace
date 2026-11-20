//! US14: EXIF, audio, PE metadata, corruption handling and filters.

mod common;

use std::path::Path;

use freespace_analysis::{
    MetadataCache, MetadataFilter, MetadataKind, MetadataStatus, extract, parse_resolution,
};

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn exif_jpeg() -> Vec<u8> {
    let make = b"TestCam\0";
    let model = b"Model X\0";
    let taken = b"2024:05:06 07:08:09\0";

    let ifd0_size = 2 + 3 * 12 + 4;
    let ifd0_data = 8 + ifd0_size;
    let make_offset = ifd0_data;
    let model_offset = make_offset + make.len();
    let exif_ifd_offset = model_offset + model.len();
    let exif_ifd_size = 2 + 3 * 12 + 4;
    let exif_data = exif_ifd_offset + exif_ifd_size;
    let taken_offset = exif_data;

    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II");
    push_u16(&mut tiff, 42);
    push_u32(&mut tiff, 8);

    // IFD0: camera make, model and the Exif sub-IFD pointer.
    push_u16(&mut tiff, 3);
    push_u16(&mut tiff, 0x010F);
    push_u16(&mut tiff, 2);
    push_u32(&mut tiff, u32::try_from(make.len()).expect("make"));
    push_u32(&mut tiff, u32::try_from(make_offset).expect("offset"));
    push_u16(&mut tiff, 0x0110);
    push_u16(&mut tiff, 2);
    push_u32(&mut tiff, u32::try_from(model.len()).expect("model"));
    push_u32(&mut tiff, u32::try_from(model_offset).expect("offset"));
    push_u16(&mut tiff, 0x8769);
    push_u16(&mut tiff, 4);
    push_u32(&mut tiff, 1);
    push_u32(&mut tiff, u32::try_from(exif_ifd_offset).expect("offset"));
    push_u32(&mut tiff, 0);

    tiff.extend_from_slice(make);
    tiff.extend_from_slice(model);

    // Exif sub-IFD: capture time and pixel dimensions.
    push_u16(&mut tiff, 3);
    push_u16(&mut tiff, 0x9003);
    push_u16(&mut tiff, 2);
    push_u32(&mut tiff, u32::try_from(taken.len()).expect("taken"));
    push_u32(&mut tiff, u32::try_from(taken_offset).expect("offset"));
    push_u16(&mut tiff, 0xA002);
    push_u16(&mut tiff, 4);
    push_u32(&mut tiff, 1);
    push_u32(&mut tiff, 3840);
    push_u16(&mut tiff, 0xA003);
    push_u16(&mut tiff, 4);
    push_u32(&mut tiff, 1);
    push_u32(&mut tiff, 2160);
    push_u32(&mut tiff, 0);
    tiff.extend_from_slice(taken);

    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);

    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
    jpeg.extend_from_slice(&u16::try_from(app1.len() + 2).expect("length").to_be_bytes());
    jpeg.extend_from_slice(&app1);
    jpeg.push(0xFF);
    jpeg.push(0xD9);

    jpeg
}

fn mp4_box(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + payload.len());
    bytes.extend_from_slice(
        &u32::try_from(8 + payload.len())
            .expect("box size")
            .to_be_bytes(),
    );
    bytes.extend_from_slice(name);
    bytes.extend_from_slice(payload);

    bytes
}

fn push_be32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn identity_matrix() -> Vec<u8> {
    let mut matrix = Vec::new();
    for value in [0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
        push_be32(&mut matrix, value);
    }

    matrix
}

fn video_mp4() -> Vec<u8> {
    let mut ftyp = Vec::from(*b"isom");
    push_be32(&mut ftyp, 0);
    ftyp.extend_from_slice(b"isom");
    ftyp.extend_from_slice(b"iso2");
    ftyp.extend_from_slice(b"mp41");

    // mvhd: version/flags, creation, modification, timescale, duration.
    let mut mvhd = vec![0, 0, 0, 0];
    push_be32(&mut mvhd, 0);
    push_be32(&mut mvhd, 0);
    push_be32(&mut mvhd, 1_000);
    push_be32(&mut mvhd, 5_000);
    push_be32(&mut mvhd, 0x0001_0000);
    mvhd.extend_from_slice(&[0x01, 0x00]);
    mvhd.extend_from_slice(&[0; 10]);
    mvhd.extend_from_slice(&identity_matrix());
    mvhd.extend_from_slice(&[0; 24]);
    push_be32(&mut mvhd, 2);

    // tkhd: version/flags, creation, modification, track id, duration, matrix,
    // width and height as 16.16 fixed-point values.
    let mut tkhd = vec![0, 0, 0, 3];
    push_be32(&mut tkhd, 0);
    push_be32(&mut tkhd, 0);
    push_be32(&mut tkhd, 1);
    push_be32(&mut tkhd, 0);
    push_be32(&mut tkhd, 5_000);
    tkhd.extend_from_slice(&[0; 8]);
    tkhd.extend_from_slice(&[0; 2]);
    tkhd.extend_from_slice(&[0; 2]);
    tkhd.extend_from_slice(&[0; 2]);
    tkhd.extend_from_slice(&[0; 2]);
    tkhd.extend_from_slice(&identity_matrix());
    push_be32(&mut tkhd, 1920 << 16);
    push_be32(&mut tkhd, 1080 << 16);

    // mdhd: version/flags, creation, modification, timescale, duration.
    let mut mdhd = vec![0, 0, 0, 0];
    push_be32(&mut mdhd, 0);
    push_be32(&mut mdhd, 0);
    push_be32(&mut mdhd, 1_000);
    push_be32(&mut mdhd, 5_000);
    mdhd.extend_from_slice(&[0x55, 0xC4]);
    mdhd.extend_from_slice(&[0; 2]);

    // hdlr: version/flags, predefined, handler type, reserved, name.
    let mut hdlr = vec![0, 0, 0, 0];
    push_be32(&mut hdlr, 0);
    hdlr.extend_from_slice(b"vide");
    hdlr.extend_from_slice(&[0; 12]);
    hdlr.extend_from_slice(b"Video\0");

    let mdia = mp4_box(
        b"mdia",
        &[mp4_box(b"mdhd", &mdhd), mp4_box(b"hdlr", &hdlr)].concat(),
    );
    let trak = mp4_box(b"trak", &[mp4_box(b"tkhd", &tkhd), mdia].concat());
    let moov = mp4_box(b"moov", &[mp4_box(b"mvhd", &mvhd), trak].concat());

    [mp4_box(b"ftyp", &ftyp), moov].concat()
}

fn wav() -> Vec<u8> {
    let samples = vec![0u8; 1600];
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    push_u32(
        &mut wav,
        u32::try_from(samples.len() + 36).expect("riff size"),
    );
    wav.extend_from_slice(b"WAVEfmt ");
    push_u32(&mut wav, 16);
    push_u16(&mut wav, 1); // PCM
    push_u16(&mut wav, 1); // mono
    push_u32(&mut wav, 8_000);
    push_u32(&mut wav, 16_000);
    push_u16(&mut wav, 2);
    push_u16(&mut wav, 16);
    wav.extend_from_slice(b"data");
    push_u32(&mut wav, u32::try_from(samples.len()).expect("data size"));
    wav.extend_from_slice(&samples);

    wav
}

#[test]
fn reads_exif_camera_resolution_and_capture_time() {
    let fixture = common::TestTree::new();
    let path = fixture.write("photo.jpg", &exif_jpeg());

    let record = extract(1, &path);

    assert_eq!(record.kind, MetadataKind::Exif);
    assert_eq!(record.status, MetadataStatus::Ok);
    assert_eq!(record.tag("CameraModel"), Some("Model X"));
    assert_eq!(record.tag("CameraMake"), Some("TestCam"));
    assert_eq!(record.width(), Some(3840));
    assert_eq!(record.height(), Some(2160));
    assert_eq!(record.tag("TakenAt"), Some("2024-05-06 07:08:09"));
}

#[test]
fn reads_audio_properties_from_a_wav_file() {
    let fixture = common::TestTree::new();
    let path = fixture.write("tone.wav", &wav());

    let record = extract(2, &path);

    assert_eq!(record.kind, MetadataKind::Audio);
    assert_eq!(record.status, MetadataStatus::Ok);
    assert_eq!(record.tag("Codec"), Some("wav"));
    assert!(record.tag("SampleRate").is_some());
    assert!(record.tag("Channels").is_some());
}

#[cfg(windows)]
#[test]
fn reads_pe_versions_from_a_real_executable() {
    let executable = std::env::current_exe().expect("current executable");
    let record = extract(3, &executable);

    assert_eq!(record.kind, MetadataKind::Pe);
    assert_eq!(record.status, MetadataStatus::Ok);
}

#[test]
fn reads_video_dimensions_and_duration() {
    let fixture = common::TestTree::new();
    let path = fixture.write("clip.mp4", &video_mp4());

    let record = extract(13, &path);

    assert_eq!(record.kind, MetadataKind::Video);
    assert_eq!(record.status, MetadataStatus::Ok);
    assert_eq!(record.width(), Some(1920));
    assert_eq!(record.height(), Some(1080));
    assert_eq!(record.duration_secs(), Some(5));
}

#[test]
fn keeps_corrupted_and_unsupported_files_visible() {
    let fixture = common::TestTree::new();
    let broken_image = fixture.write("broken.jpg", b"\xFF\xD8\xFF\xE0 not a JPEG");
    let broken_audio = fixture.write("broken.mp3", b"not audio");
    let broken_video = fixture.write("broken.mp4", b"not video");
    let text = fixture.write("notes.txt", b"plain text");

    for path in [broken_image, broken_audio, broken_video] {
        let record = extract(4, &path);
        assert_eq!(
            record.status,
            MetadataStatus::Corrupted,
            "{}",
            path.display()
        );
        assert!(!record.is_available());
    }
    let record = extract(5, &text);
    assert_eq!(record.status, MetadataStatus::Unsupported);
    assert!(!record.is_available());
}

#[test]
fn filters_by_resolution_and_family() {
    let fixture = common::TestTree::new();
    let path = fixture.write("photo.jpg", &exif_jpeg());
    let record = extract(6, &path);

    let filter = MetadataFilter {
        kinds: vec![MetadataKind::Exif],
        min_width: Some(1920),
        min_height: Some(1080),
        ..MetadataFilter::default()
    };
    assert!(filter.matches(&record));
    assert!(
        !MetadataFilter {
            min_width: Some(7680),
            ..MetadataFilter::default()
        }
        .matches(&record)
    );
    assert_eq!(
        parse_resolution("3840x2160").expect("resolution"),
        (3840, 2160)
    );
    assert!(parse_resolution("4K").is_err());
}

#[test]
fn caches_extraction_by_path_and_modification_time() {
    let fixture = common::TestTree::new();
    let path = fixture.write("photo.jpg", &exif_jpeg());
    let cache = MetadataCache::new();

    let first = cache.extract(10, &path);
    let second = cache.extract(11, &path);
    assert_eq!(first.tags, second.tags);
    assert_eq!(second.node_id, 11);
    assert_eq!(cache.len(), 1);

    // Replacing the file invalidates the record.
    std::fs::write(&path, b"not an image").expect("rewrite");
    let file = std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("open for timestamp");
    file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
        .expect("set mtime");
    let third = cache.extract(12, &path);
    assert_eq!(third.status, MetadataStatus::Corrupted);
}

#[test]
fn the_cli_filter_applies_metadata_constraints() {
    let fixture = common::TestTree::new();
    fixture.write("photo.jpg", &exif_jpeg());
    fixture.write("payload.bin", &[0u8; 64]);

    let outcome = freespace_cli::commands::filter::run(
        freespace_cli::commands::filter::FilterArgs {
            min_resolution: Some("1920x1080"),
            ..freespace_cli::commands::filter::FilterArgs::new(fixture.root())
        },
        freespace_core::size::Units::Binary,
    )
    .expect("filter");

    assert_eq!(outcome.data["count"].as_u64(), Some(1));
    let item = &outcome.data["items"][0];
    assert!(item["path"].as_str().expect("path").ends_with("photo.jpg"));
    assert_eq!(item["metadata"]["tags"]["Width"], "3840");
    assert!(Path::new(item["path"].as_str().expect("path")).exists());
}
