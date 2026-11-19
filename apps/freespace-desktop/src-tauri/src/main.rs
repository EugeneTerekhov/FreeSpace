//! FreeSpace desktop entry point.
//!
//! The binary keeps the historical name `freespace-gui.exe` so shortcuts, scripts,
//! the installer and the Explorer context menu keep working after the egui client
//! is retired.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    freespace_desktop::run();
}
