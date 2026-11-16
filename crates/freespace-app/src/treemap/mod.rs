//! Treemap geometry and aggregation.
//!
//! The squarified layout is pure geometry and is tested without a graphics
//! context; the canvas renderer consumes the rectangles produced here. The
//! aggregation module applies the tile budget, nesting depth and the aggregate
//! "other" tile.

pub mod aggregate;
pub mod layout;

pub use aggregate::tiles;
pub use layout::{Rect, TreemapItem, squarify};
