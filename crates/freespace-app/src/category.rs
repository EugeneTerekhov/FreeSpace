//! File-category keys shared by filters and Treemap colours.
//!
//! The keys are stable on the wire and map to the palette in
//! `ui/src/treemap/palettes.ts`.

/// Category key used when a name has no known extension.
pub const OTHER: &str = "other";
/// Category key for directories.
pub const FOLDER: &str = "folder";

/// Returns the stable category key of a file name.
#[must_use]
pub fn category_key(name: &str) -> &'static str {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "flv" | "m4v" => "video",
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "svg" | "heic"
        | "raw" => "image",
        "mp3" | "flac" | "wav" | "aac" | "ogg" | "m4a" | "wma" | "opus" => "audio",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "rtf" | "odt"
        | "csv" => "document",
        "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" | "iso" => "archive",
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "cpp" | "c" | "h" | "hpp" | "cs" | "java"
        | "go" | "html" | "css" | "json" | "toml" | "yaml" | "yml" | "sql" | "sh" | "ps1" => "code",
        "exe" | "dll" | "msi" | "sys" | "bin" | "com" => "executable",
        _ => OTHER,
    }
}

/// Returns the category key of a node, including the `folder` key.
#[must_use]
pub fn node_category_key(name: &str, is_directory: bool) -> &'static str {
    if is_directory {
        FOLDER
    } else {
        category_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::{FOLDER, category_key, node_category_key};

    #[test]
    fn classifies_extensions_case_insensitively() {
        assert_eq!(category_key("clip.MP4"), "video");
        assert_eq!(category_key("photo.jpeg"), "image");
        assert_eq!(category_key("report.pdf"), "document");
        assert_eq!(category_key("backup.zip"), "archive");
        assert_eq!(category_key("lib.rs"), "code");
        assert_eq!(category_key("app.exe"), "executable");
        assert_eq!(category_key("README"), "other");
    }

    #[test]
    fn marks_directories_as_folders() {
        assert_eq!(node_category_key("Windows", true), FOLDER);
        assert_eq!(node_category_key("app.exe", false), "executable");
    }
}
