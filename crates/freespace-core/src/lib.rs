#![forbid(unsafe_code)]

//! FreeSpace core: domain model, tree arena, sizes, settings and shared services.
//!
//! The crate is platform independent by design: it must not call Windows APIs
//! directly. Platform-specific code lives in `freespace-platform`.

#[macro_use]
extern crate rust_i18n;

i18n!("../../locales", fallback = "en");

pub mod category;
pub mod error;
pub mod i18n;
pub mod logging;
pub mod model;
pub mod query;
pub mod settings;
pub mod size;
pub mod time;
pub mod tree;

pub use error::{Error, ErrorCode, Result};
pub use time::Timestamp;
pub use uuid::Uuid;
