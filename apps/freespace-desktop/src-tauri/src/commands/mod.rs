//! Tauri command adapters. Commands validate input, call `freespace-app` services
//! and serialise the result; domain logic never lives here.

pub mod analysis;
pub mod app;
pub mod filesystem;
pub mod inspection;
pub mod monitor;
pub mod operations;
pub mod scan;
pub mod settings;
pub mod tree;
pub mod volumes;
