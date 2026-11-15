//! Colour coding by file category.
//!
//! The category itself lives in `freespace-core` so that the analysis report and
//! the treemap can never disagree about what a "video" is. Only the palette is
//! a graphical concern and stays here.

use eframe::egui::Color32;
pub use freespace_core::category::{FileCategory, category_for};

/// Palette used to paint rectangles, legends and type rows.
pub trait CategoryColor {
    /// Fill colour of the category.
    fn color(self) -> Color32;
}

impl CategoryColor for FileCategory {
    fn color(self) -> Color32 {
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
    use super::{CategoryColor, FileCategory, category_for, directory_color};

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
