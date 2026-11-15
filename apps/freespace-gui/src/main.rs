#![forbid(unsafe_code)]

//! FreeSpace graphical interface.
//!
//! The binary is deliberately thin: [`app`] owns the shell, navigation,
//! background tasks and settings, while the shared crates provide all domain
//! logic.

#[macro_use]
extern crate rust_i18n;

mod app;
mod treemap;
mod views;

use eframe::egui;

i18n!("../../locales", fallback = "en");

fn main() -> eframe::Result {
    freespace_core::logging::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title(app::window_title()),
        ..Default::default()
    };

    eframe::run_native(
        "FreeSpace",
        options,
        Box::new(|creation_context| Ok(Box::new(app::FreeSpaceApp::new(creation_context)))),
    )
}
