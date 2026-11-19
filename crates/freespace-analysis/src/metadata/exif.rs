//! EXIF extraction through `kamadak-exif` (FR-061).

use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path};

use exif::{In, Reader, Tag};
use freespace_core::{Error, Result};

use super::tags;

/// Reads the EXIF tags FreeSpace displays.
///
/// # Errors
/// Returns [`Error::Operation`] when the file has no EXIF block or the block is
/// damaged.
pub fn read(path: &Path) -> Result<BTreeMap<String, String>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let exif = Reader::new()
        .read_from_container(&mut reader)
        .map_err(|error| Error::Operation(format!("cannot read EXIF metadata: {error}")))?;

    let mut tags_map = BTreeMap::new();
    if let Some(value) = text(&exif, Tag::Make) {
        tags_map.insert(tags::CAMERA_MAKE.to_owned(), value);
    }
    if let Some(value) = text(&exif, Tag::Model) {
        tags_map.insert(tags::CAMERA_MODEL.to_owned(), value);
    }
    if let Some(value) = text(&exif, Tag::DateTimeOriginal).or_else(|| text(&exif, Tag::DateTime)) {
        tags_map.insert(tags::TAKEN_AT.to_owned(), value);
    }
    if let Some(width) =
        number(&exif, Tag::PixelXDimension).or_else(|| number(&exif, Tag::ImageWidth))
    {
        tags_map.insert(tags::WIDTH.to_owned(), width.to_string());
    }
    if let Some(height) =
        number(&exif, Tag::PixelYDimension).or_else(|| number(&exif, Tag::ImageLength))
    {
        tags_map.insert(tags::HEIGHT.to_owned(), height.to_string());
    }

    Ok(tags_map)
}

/// Returns a human-readable field value.
fn text(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let field = find_field(exif, tag)?;
    let value = field.display_value().to_string();
    let value = value.trim().trim_matches('"').trim();

    (!value.is_empty()).then(|| value.to_owned())
}

/// Returns an unsigned integer field.
fn number(exif: &exif::Exif, tag: Tag) -> Option<u32> {
    find_field(exif, tag)?.value.get_uint(0)
}

/// Looks a tag up in the primary IFD first and then in every IFD.
///
/// Capture time and pixel dimensions often live in the Exif sub-IFD, while
/// camera make and model live in IFD0; scanning every field keeps both paths
/// working without hard-coding the vendor layout.
fn find_field(exif: &exif::Exif, tag: Tag) -> Option<&exif::Field> {
    exif.get_field(tag, In::PRIMARY)
        .or_else(|| exif.fields().find(|field| field.tag == tag))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::read;

    #[test]
    fn rejects_files_without_exif() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("broken.jpg");
        std::fs::write(&path, b"\xFF\xD8\xFF\xE0not really a JPEG").expect("write");

        assert!(read(Path::new(&path)).is_err());
    }
}
