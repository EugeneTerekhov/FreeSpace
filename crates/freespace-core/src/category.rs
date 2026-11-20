//! Coarse file categories derived from the extension.
//!
//! Both the analysis crate and the graphical frontend need to agree on what a
//! "video" or an "archive" is, so the mapping lives in the core crate. It is
//! deliberately small and stable: adding an extension must never change the
//! meaning of an existing category.

use serde::{Deserialize, Serialize};

use crate::query::extension_of;

/// Coarse category of a file, used for grouping and colouring.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileCategory {
    /// Video files.
    Video,
    /// Audio files.
    Audio,
    /// Images and photos.
    Image,
    /// Archives and disk images.
    Archive,
    /// Documents.
    Document,
    /// Source code and scripts.
    Code,
    /// Executables and libraries.
    Executable,
    /// Databases and virtual machine disks.
    Database,
    /// Fonts.
    Font,
    /// Directories (volume roots are not directories in the type report).
    Folder,
    /// Anything else.
    #[default]
    Other,
}

impl FileCategory {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Image => "image",
            Self::Archive => "archive",
            Self::Document => "document",
            Self::Code => "code",
            Self::Executable => "executable",
            Self::Database => "database",
            Self::Font => "font",
            Self::Folder => "folder",
            Self::Other => "other",
        }
    }

    /// Translation key used by the legend and by the type report.
    #[must_use]
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Video => "treemap.category.video",
            Self::Audio => "treemap.category.audio",
            Self::Image => "treemap.category.image",
            Self::Archive => "treemap.category.archive",
            Self::Document => "treemap.category.document",
            Self::Code => "treemap.category.code",
            Self::Executable => "treemap.category.executable",
            Self::Database => "treemap.category.database",
            Self::Font => "treemap.category.font",
            Self::Folder => "treemap.category.folder",
            Self::Other => "treemap.category.other",
        }
    }

    /// Every category in legend order.
    pub const ALL: [Self; 11] = [
        Self::Video,
        Self::Audio,
        Self::Image,
        Self::Archive,
        Self::Document,
        Self::Code,
        Self::Executable,
        Self::Database,
        Self::Font,
        Self::Folder,
        Self::Other,
    ];
}

/// Maps a file name to its category.
#[must_use]
pub fn category_for(name: &str) -> FileCategory {
    category_for_extension(&extension_of(name).to_ascii_lowercase())
}

/// Maps an already lower-cased extension (without the dot) to a category.
#[must_use]
pub fn category_for_extension(extension: &str) -> FileCategory {
    match extension {
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "mpg" | "mpeg" => {
            FileCategory::Video
        }
        "mp3" | "flac" | "wav" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "mid" => {
            FileCategory::Audio
        }
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tif" | "tiff" | "webp" | "heic" | "svg"
        | "raw" | "psd" => FileCategory::Image,
        "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" | "cab" | "iso" | "vhd" | "vhdx" => {
            FileCategory::Archive
        }
        "pdf" | "doc" | "docx" | "txt" | "rtf" | "odt" | "xls" | "xlsx" | "ods" | "ppt"
        | "pptx" | "odp" | "md" | "epub" => FileCategory::Document,
        "rs" | "c" | "h" | "cpp" | "hpp" | "cs" | "js" | "ts" | "tsx" | "py" | "go" | "java"
        | "kt" | "sh" | "ps1" | "bat" | "cmd" | "html" | "css" | "json" | "toml" | "yaml"
        | "yml" | "xml" | "sql" => FileCategory::Code,
        "exe" | "dll" | "sys" | "msi" | "com" | "so" | "lib" | "bin" => FileCategory::Executable,
        "db" | "sqlite" | "sqlite3" | "mdf" | "ldf" | "vmdk" | "qcow2" | "ova" => {
            FileCategory::Database
        }
        "ttf" | "otf" | "woff" | "woff2" | "fon" | "ttc" => FileCategory::Font,
        _ => FileCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::{FileCategory, category_for, category_for_extension};

    #[test]
    fn maps_common_extensions() {
        assert_eq!(category_for("movie.MKV"), FileCategory::Video);
        assert_eq!(category_for("song.flac"), FileCategory::Audio);
        assert_eq!(category_for("photo.jpeg"), FileCategory::Image);
        assert_eq!(category_for("backup.7z"), FileCategory::Archive);
        assert_eq!(category_for("report.pdf"), FileCategory::Document);
        assert_eq!(category_for("main.rs"), FileCategory::Code);
        assert_eq!(category_for("app.exe"), FileCategory::Executable);
        assert_eq!(category_for("cache.sqlite"), FileCategory::Database);
        assert_eq!(category_for("font.woff2"), FileCategory::Font);
        assert_eq!(category_for("README"), FileCategory::Other);
        assert_eq!(category_for("no.extension."), FileCategory::Other);
    }

    #[test]
    fn accepts_an_already_lower_cased_extension() {
        assert_eq!(category_for_extension("mkv"), FileCategory::Video);
        assert_eq!(category_for_extension("MKV"), FileCategory::Other);
        assert_eq!(category_for_extension(""), FileCategory::Other);
    }

    #[test]
    fn categories_have_unique_names_and_labels() {
        let mut names = Vec::new();
        let mut labels = Vec::new();
        for category in FileCategory::ALL {
            names.push(category.as_str());
            labels.push(category.label_key());
            assert!(category.label_key().starts_with("treemap.category."));
        }
        names.sort_unstable();
        names.dedup();
        labels.sort_unstable();
        labels.dedup();

        assert_eq!(names.len(), FileCategory::ALL.len());
        assert_eq!(labels.len(), FileCategory::ALL.len());
    }
}
