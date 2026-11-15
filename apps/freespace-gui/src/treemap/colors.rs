//! Colour coding by file category.
//!
//! A category is derived from the extension only; the mapping is deliberately
//! small and stable so that the legend and the rectangles can never disagree.

use eframe::egui::Color32;
use freespace_core::query::extension_of;

/// Coarse category used for colouring.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
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
            Self::Other => "other",
        }
    }

    /// Translation key used by the legend.
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
            Self::Other => "treemap.category.other",
        }
    }

    /// Fill colour of the category.
    #[must_use]
    pub const fn color(self) -> Color32 {
        match self {
            Self::Video => Color32::from_rgb(196, 88, 88),
            Self::Audio => Color32::from_rgb(196, 140, 70),
            Self::Image => Color32::from_rgb(88, 148, 196),
            Self::Archive => Color32::from_rgb(148, 110, 176),
            Self::Document => Color32::from_rgb(96, 164, 120),
            Self::Code => Color32::from_rgb(86, 150, 160),
            Self::Executable => Color32::from_rgb(170, 170, 90),
            Self::Database => Color32::from_rgb(120, 120, 190),
            Self::Font => Color32::from_rgb(180, 120, 150),
            Self::Other => Color32::from_rgb(128, 132, 140),
        }
    }

    /// Every category in legend order.
    pub const ALL: [Self; 10] = [
        Self::Video,
        Self::Audio,
        Self::Image,
        Self::Archive,
        Self::Document,
        Self::Code,
        Self::Executable,
        Self::Database,
        Self::Font,
        Self::Other,
    ];
}

/// Maps a file name to its category.
#[must_use]
pub fn category_for(name: &str) -> FileCategory {
    let extension = extension_of(name).to_ascii_lowercase();

    match extension.as_str() {
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

/// Colour of a directory rectangle.
#[must_use]
pub const fn directory_color(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(64, 70, 82)
    } else {
        Color32::from_rgb(214, 218, 226)
    }
}

#[cfg(test)]
mod tests {
    use super::{FileCategory, category_for, directory_color};

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
    fn every_category_has_a_label_and_a_colour() {
        let mut colours = Vec::new();
        for category in FileCategory::ALL {
            assert!(category.label_key().starts_with("treemap.category."));
            assert_ne!(
                t!(category.label_key()).to_string(),
                category.label_key(),
                "missing translation for {}",
                category.label_key()
            );
            colours.push(category.color());
        }
        colours.sort_by_key(|colour| colour.to_array());
        colours.dedup();

        assert_eq!(colours.len(), FileCategory::ALL.len());
        assert_ne!(directory_color(true), directory_color(false));
    }
}
