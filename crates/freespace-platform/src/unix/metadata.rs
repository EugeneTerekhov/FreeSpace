//! Linux metadata: allocated size, owners and links.
//!
//! Linux has no NTFS attribute mask, ACL editor or alternate data streams in
//! this release, therefore the corresponding calls are honestly reported as
//! unsupported and `Capabilities::attrs` stays `false`. What does exist is used
//! directly: `st_blocks` gives the space actually consumed (sparse files are
//! accounted naturally), and `/etc/passwd` resolves a numeric uid to a name.
//!
//! Exact allocated extents would need `FIEMAP` or `SEEK_HOLE`; the screen only
//! needs the total, so [`allocated_ranges`] reports one range covering the
//! allocated size. That keeps the sum correct without pretending to know the
//! holes. The simplification is documented on the function.

use std::{fs, ops::Range, os::unix::fs::MetadataExt as _, path::Path};

use freespace_core::{Error, Result};

use crate::common::metadata::{LinkKind, StreamInfo};

/// Reads a Windows attribute mask; unavailable on Linux.
///
/// # Errors
/// Always returns [`Error::Unsupported`].
pub fn attributes(_path: &Path) -> Result<u32> {
    Err(Error::Unsupported(
        "Windows file attributes are not available on Linux".to_owned(),
    ))
}

/// Applies a Windows attribute mask; unavailable on Linux.
///
/// # Errors
/// Always returns [`Error::Unsupported`].
pub fn set_attributes(_path: &Path, _set: u32, _clear: u32) -> Result<u32> {
    Err(Error::Unsupported(
        "Windows file attributes are not available on Linux".to_owned(),
    ))
}

/// Returns the owner name of `path`.
///
/// The name comes from `/etc/passwd`; when the uid is unknown (containers,
/// system accounts, NSS-only users) the numeric uid is returned instead, which
/// is still a stable report value.
///
/// # Errors
/// Returns [`Error::Io`] when the metadata cannot be read.
pub fn owner(path: &Path) -> Result<Option<String>> {
    let metadata = fs::metadata(path).map_err(Error::Io)?;
    let uid = metadata.uid();

    Ok(Some(owner_name(uid).unwrap_or_else(|| uid.to_string())))
}

/// Resolves a uid through `/etc/passwd`, ignoring every other user database.
pub(crate) fn owner_name(uid: u32) -> Option<String> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;

    parse_passwd(&passwd, uid)
}

/// Parses one uid out of the colon-separated passwd format.
pub(crate) fn parse_passwd(text: &str, uid: u32) -> Option<String> {
    text.lines().find_map(|line| {
        let mut fields = line.trim_end_matches('\r').split(':');
        let name = fields.next()?;
        fields.next()?; // password placeholder
        let entry_uid = fields.next()?.parse::<u32>().ok()?;

        (entry_uid == uid && !name.is_empty()).then(|| name.to_owned())
    })
}

/// Returns the space actually allocated to `path`, in bytes.
///
/// `st_blocks` is always reported in 512-byte units regardless of the volume
/// block size, so the multiplication is exact.
///
/// # Errors
/// Returns [`Error::Io`] when the metadata cannot be read.
pub fn allocated_size(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path).map_err(Error::Io)?;

    Ok(metadata.blocks().saturating_mul(512))
}

/// Returns the allocated byte ranges of `path`.
///
/// This is a deliberate simplification: without `FIEMAP`/`SEEK_HOLE` Linux does
/// not expose the hole layout through portable metadata, so one range
/// `0..allocated_size` is returned. Its length is exact, which is what the
/// size-details screen uses; only the position of sparse holes is approximate.
///
/// # Errors
/// Returns [`Error::Io`] when the metadata cannot be read.
pub fn allocated_ranges(path: &Path) -> Result<Vec<Range<u64>>> {
    let allocated = allocated_size(path)?;

    if allocated == 0 {
        return Ok(Vec::new());
    }

    Ok(std::iter::once(0..allocated).collect())
}

/// Lists alternate data streams; Linux has none.
///
/// # Errors
/// Currently never fails.
pub fn streams(_path: &Path) -> Result<Vec<StreamInfo>> {
    Ok(Vec::new())
}

/// Classifies symbolic links; mount points are handled by the path policy.
///
/// # Errors
/// Returns [`Error::Io`] when the link metadata cannot be read.
pub fn link_kind(path: &Path) -> Result<Option<LinkKind>> {
    let metadata = fs::symlink_metadata(path).map_err(Error::Io)?;

    Ok(metadata
        .file_type()
        .is_symlink()
        .then_some(LinkKind::Symlink))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt as _;

    use super::{allocated_ranges, allocated_size, attributes, owner, parse_passwd};

    const PASSWD: &str = "\
root:x:0:0:root:/root:/bin/bash
daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin
alice:x:1000:1000:Alice:/home/alice:/bin/bash
";

    #[test]
    fn parses_passwd_entries() {
        assert_eq!(parse_passwd(PASSWD, 0).as_deref(), Some("root"));
        assert_eq!(parse_passwd(PASSWD, 1000).as_deref(), Some("alice"));
        assert_eq!(parse_passwd(PASSWD, 4242), None);
    }

    #[test]
    fn resolves_the_current_user_or_falls_back_to_the_uid() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("owned.txt");
        std::fs::write(&path, b"payload").expect("write fixture");

        let uid = std::fs::metadata(&path).expect("metadata").uid();
        let name = owner(&path).expect("owner").expect("owner value");

        assert!(!name.is_empty());
        let matches_passwd = std::fs::read_to_string("/etc/passwd")
            .ok()
            .and_then(|passwd| parse_passwd(&passwd, uid));
        assert!(name == uid.to_string() || matches_passwd.as_deref() == Some(name.as_str()));
    }

    #[test]
    fn counts_blocks_as_allocated_bytes() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, vec![0x5A; 8_192]).expect("write fixture");

        let blocks = std::fs::metadata(&path).expect("metadata").blocks();
        let allocated = allocated_size(&path).expect("allocated size");

        assert!(allocated > 0);
        assert_eq!(allocated, blocks.saturating_mul(512));
    }

    #[test]
    fn reports_one_range_covering_the_allocated_size() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, vec![0x5A; 4_096]).expect("write fixture");

        let ranges = allocated_ranges(&path).expect("ranges");
        let allocated = allocated_size(&path).expect("allocated size");

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], 0..allocated);
    }

    #[test]
    fn attributes_are_unsupported() {
        let error = attributes(std::path::Path::new("/")).expect_err("unsupported");

        assert_eq!(error.code(), freespace_core::ErrorCode::Unsupported);
    }
}
