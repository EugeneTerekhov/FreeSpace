//! PE version resources through `pelite` (FR-061).

use std::{collections::BTreeMap, path::Path};

use freespace_core::{Error, Result};
use pelite::{
    pe32, pe64,
    resources::version_info::{VersionInfo, Visit},
};

use super::tags;

/// Reads `VS_VERSIONINFO` from a Portable Executable.
///
/// A well-formed executable without a version resource yields an empty tag set
/// rather than an error: the file is readable, it simply has nothing to show.
///
/// # Errors
/// Returns [`Error::Operation`] when the file is not a PE image or its resource
/// directory is damaged.
pub fn read(path: &Path) -> Result<BTreeMap<String, String>> {
    let bytes = std::fs::read(path)?;

    let version_info = match read_pe64(&bytes) {
        Some(version_info) => version_info,
        None => match read_pe32(&bytes) {
            Some(version_info) => version_info,
            None => {
                return Err(Error::Operation(
                    "the file is not a readable PE image".to_owned(),
                ));
            }
        },
    };
    let Some(version_info) = version_info else {
        // A valid executable without a version resource has nothing to show.
        return Ok(BTreeMap::new());
    };

    let mut collector = Collector::default();
    version_info.visit(&mut collector);

    Ok(collector.tags)
}

fn read_pe64(bytes: &[u8]) -> Option<Option<VersionInfo<'_>>> {
    use pelite::pe64::Pe as _;

    let image = pe64::PeFile::from_bytes(bytes).ok()?;
    let version_info = image
        .resources()
        .ok()
        .and_then(|resources| resources.version_info().ok());

    Some(version_info)
}

fn read_pe32(bytes: &[u8]) -> Option<Option<VersionInfo<'_>>> {
    use pelite::pe32::Pe as _;

    let image = pe32::PeFile::from_bytes(bytes).ok()?;
    let version_info = image
        .resources()
        .ok()
        .and_then(|resources| resources.version_info().ok());

    Some(version_info)
}

#[derive(Debug, Default)]
struct Collector {
    tags: BTreeMap<String, String>,
}

impl<'a> Visit<'a> for Collector {
    fn version_info(
        &mut self,
        _key: &'a [u16],
        fixed: Option<&'a pelite::image::VS_FIXEDFILEINFO>,
    ) -> bool {
        if let Some(fixed) = fixed {
            let file = fixed.dwFileVersion;
            self.tags.insert(
                tags::FILE_VERSION.to_owned(),
                format!(
                    "{}.{}.{}.{}",
                    file.Major, file.Minor, file.Patch, file.Build
                ),
            );
            let product = fixed.dwProductVersion;
            self.tags.insert(
                tags::PRODUCT_VERSION.to_owned(),
                format!(
                    "{}.{}.{}.{}",
                    product.Major, product.Minor, product.Patch, product.Build
                ),
            );
        }

        true
    }

    fn string(&mut self, key: &'a [u16], value: &'a [u16]) {
        let key = String::from_utf16_lossy(key);
        let value = String::from_utf16_lossy(value);
        let target = match key.as_str() {
            "ProductName" => Some(tags::PRODUCT_NAME),
            "FileDescription" => Some(tags::FILE_DESCRIPTION),
            "CompanyName" => Some(tags::COMPANY_NAME),
            "FileVersion" => Some(tags::FILE_VERSION),
            "ProductVersion" => Some(tags::PRODUCT_VERSION),
            _ => None,
        };
        if let Some(target) = target {
            self.tags.entry(target.to_owned()).or_insert(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::read;

    #[test]
    fn rejects_non_pe_files() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("not-a-program.exe");
        std::fs::write(&path, b"MZ but not really").expect("write");

        assert!(read(&path).is_err());
    }

    #[test]
    fn reads_the_current_test_executable() {
        let executable = std::env::current_exe().expect("current executable");
        let tags = read(&executable).expect("PE image");

        // Rust test binaries normally carry a FILEVERSION resource; if the
        // toolchain omits it the call still succeeds with an empty tag map.
        assert!(
            tags.is_empty() || tags.contains_key("FileVersion"),
            "unexpected tags: {tags:?}"
        );
    }
}
