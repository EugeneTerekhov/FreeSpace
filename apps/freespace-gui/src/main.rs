#![forbid(unsafe_code)]

//! FreeSpace graphical interface.
//!
//! Phase 1 skeleton: a native window wired to the shared core crate and the
//! embedded translation tables.

#[macro_use]
extern crate rust_i18n;

use eframe::egui;
use freespace_core::i18n::{self, Locale};

i18n!("../../locales", fallback = "en");

fn main() -> eframe::Result {
    freespace_core::logging::init();
    i18n::set_locale(Locale::DEFAULT);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "FreeSpace",
        options,
        Box::new(|_creation_context| Ok(Box::new(FreeSpaceApp))),
    )
}

/// Root application state.
struct FreeSpaceApp;

impl eframe::App for FreeSpaceApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(context, |ui| {
            ui.heading(t!("app.name").to_string());
            ui.label(t!("app.tagline").to_string());
            ui.separator();
            ui.label(t!("app.phase1_ready").to_string());
        });
    }
}
