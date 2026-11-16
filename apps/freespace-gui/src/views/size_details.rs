//! Exact size details: logical, allocated, savings, reparse points and ADS
//! (FR-047…FR-050).

use std::{fs, path::PathBuf};

use eframe::egui;
use freespace_core::size::{Units, format_bytes};
use freespace_platform::{
    ads::{StreamInfo, list_streams},
    allocated::allocated_size,
    paths,
    reparse::{ReparseInfo, inspect},
};

/// Inspected properties of one object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectDetails {
    /// Path that was inspected.
    pub path: PathBuf,
    /// Logical length in bytes.
    pub logical: u64,
    /// Space allocated on disk.
    pub allocated: u64,
    /// Named alternate data streams.
    pub streams: Vec<StreamInfo>,
    /// Reparse-point information.
    pub reparse: Option<ReparseInfo>,
}

impl ObjectDetails {
    /// Bytes saved by compression or sparseness.
    #[must_use]
    pub const fn savings(&self) -> u64 {
        self.logical.saturating_sub(self.allocated)
    }

    /// Total size of alternate data streams.
    #[must_use]
    pub fn ads_bytes(&self) -> u64 {
        self.streams.iter().map(|stream| stream.size).sum()
    }
}

/// State of the size-details screen.
#[derive(Debug, Default)]
pub struct SizeDetailsState {
    path: String,
    details: Option<ObjectDetails>,
    error: Option<String>,
}

impl SizeDetailsState {
    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, units: Units) {
        ui.label(t!("details.intro").to_string());
        ui.horizontal(|ui| {
            ui.label(t!("details.path").to_string());
            ui.add(
                egui::TextEdit::singleline(&mut self.path)
                    .desired_width(420.0)
                    .hint_text(r"C:\data\file.bin"),
            );
            if ui.button(t!("details.inspect").to_string()).clicked() {
                self.inspect();
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }

        let Some(details) = &self.details else {
            return;
        };

        ui.add_space(8.0);
        egui::Grid::new("size_details")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.label(t!("details.logical").to_string());
                ui.label(format_bytes(details.logical, units));
                ui.end_row();
                ui.label(t!("details.allocated").to_string());
                ui.label(format_bytes(details.allocated, units));
                ui.end_row();
                ui.label(t!("details.savings").to_string());
                ui.label(format_bytes(details.savings(), units));
                ui.end_row();
                ui.label(t!("details.ads").to_string());
                ui.label(format_bytes(details.ads_bytes(), units));
                ui.end_row();
                if let Some(reparse) = &details.reparse {
                    ui.label(t!("details.reparse").to_string());
                    ui.label(format!(
                        "{} (0x{:08X}){}",
                        reparse.kind.as_str(),
                        reparse.tag,
                        reparse
                            .target
                            .as_deref()
                            .map(|target| format!(" → {}", paths::display(target)))
                            .unwrap_or_default()
                    ));
                    ui.end_row();
                }
            });

        if !details.streams.is_empty() {
            ui.add_space(8.0);
            ui.heading(t!("details.streams").to_string());
            for stream in &details.streams {
                ui.label(format!(
                    "{:>12}  {}",
                    format_bytes(stream.size, units),
                    stream.name
                ));
            }
        }
    }

    fn inspect(&mut self) {
        self.error = None;
        self.details = None;
        let path = PathBuf::from(self.path.trim());
        if path.as_os_str().is_empty() {
            self.error = Some(t!("details.path_required").to_string());
            return;
        }

        let result = (|| {
            let verbatim = paths::to_verbatim(&path)?;
            let metadata = fs::metadata(&verbatim)?;
            Ok::<_, freespace_core::Error>(ObjectDetails {
                path,
                logical: metadata.len(),
                allocated: allocated_size(&verbatim).unwrap_or(metadata.len()),
                streams: list_streams(&verbatim).unwrap_or_default(),
                reparse: inspect(&verbatim).ok().flatten(),
            })
        })();

        match result {
            Ok(details) => self.details = Some(details),
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ObjectDetails, SizeDetailsState};
    use freespace_platform::ads::StreamInfo;
    use std::path::PathBuf;

    #[test]
    fn starts_without_details() {
        assert!(SizeDetailsState::default().details.is_none());
    }

    #[test]
    fn computes_savings_and_stream_bytes() {
        let details = ObjectDetails {
            path: PathBuf::from(r"C:\data\file.bin"),
            logical: 1_000,
            allocated: 400,
            streams: vec![StreamInfo {
                name: "Zone.Identifier".to_owned(),
                size: 26,
            }],
            reparse: None,
        };

        assert_eq!(details.savings(), 600);
        assert_eq!(details.ads_bytes(), 26);
    }
}
