//! Window geometry persisted in `data/ui-state.toml`.
//!
//! The state is written only by Rust and only after a successful resize or move.
//! Coordinates outside every monitor are centred instead of being restored, so a
//! window never reappears on a disconnected display.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{dto::WindowState, error::UiError, validation::validate_window_state};

/// File name of the window state inside the data directory.
pub const WINDOW_STATE_FILE_NAME: &str = "ui-state.toml";

/// Minimum visible width of a restored window, in pixels.
const MIN_VISIBLE_WIDTH: i32 = 160;
/// Minimum visible height of a restored window, in pixels.
const MIN_VISIBLE_HEIGHT: i32 = 80;

/// Work area of one monitor in physical pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorBounds {
    /// Left edge.
    pub left: i32,
    /// Top edge.
    pub top: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Monitor name, when the platform reports one.
    pub name: Option<String>,
}

impl MonitorBounds {
    /// Right edge.
    #[must_use]
    pub fn right(&self) -> i32 {
        self.left
            .saturating_add(i32::try_from(self.width).unwrap_or(i32::MAX))
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(&self) -> i32 {
        self.top
            .saturating_add(i32::try_from(self.height).unwrap_or(i32::MAX))
    }

    /// Returns `true` when a rectangle has enough visible area on this monitor.
    #[must_use]
    pub fn has_visible_area(&self, x: i32, y: i32, width: u32, height: u32) -> bool {
        let visible_width = width.min(u32::try_from((self.right() - x).max(0)).unwrap_or(u32::MAX));
        let visible_height =
            height.min(u32::try_from((self.bottom() - y).max(0)).unwrap_or(u32::MAX));

        x < self.right()
            && y < self.bottom()
            && visible_width >= u32::try_from(MIN_VISIBLE_WIDTH).unwrap_or(0)
            && visible_height >= u32::try_from(MIN_VISIBLE_HEIGHT).unwrap_or(0)
    }
}

impl WindowState {
    /// Clamps the size and moves the window into a visible monitor.
    ///
    /// Without monitor information the coordinates are dropped and the operating
    /// system centres the window.
    #[must_use]
    pub fn sanitized(&self, monitors: &[MonitorBounds]) -> Self {
        let mut state = self.clone();
        state.width = state.width.clamp(800, 16_384);
        state.height = state.height.clamp(600, 16_384);

        if monitors.is_empty() {
            state.x = None;
            state.y = None;
            return state;
        }

        let preferred = state.monitor_id.as_deref().and_then(|id| {
            monitors
                .iter()
                .find(|monitor| monitor.name.as_deref() == Some(id))
        });

        let monitor = preferred.or_else(|| {
            let (Some(x), Some(y)) = (state.x, state.y) else {
                return None;
            };
            monitors
                .iter()
                .find(|monitor| monitor.has_visible_area(x, y, state.width, state.height))
        });

        match (monitor, state.x, state.y) {
            (Some(monitor), Some(x), Some(y))
                if monitor.has_visible_area(x, y, state.width, state.height) =>
            {
                state.width = state.width.min(monitor.width.max(800));
                state.height = state.height.min(monitor.height.max(600));
                state.x = Some(x);
                state.y = Some(y);
            }
            _ => {
                state.x = None;
                state.y = None;
            }
        }

        state
    }
}

/// Loads a window state; a missing or malformed file yields the defaults.
///
/// # Errors
/// Returns `E_IO` when the file exists but cannot be read.
pub fn load_window_state(path: &Path) -> Result<WindowState, UiError> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let state: WindowState = toml::from_str(&text)
                .map_err(|error| UiError::validation(format!("invalid ui-state.toml: {error}")))?;
            // Malformed values fall back to defaults instead of blocking startup.
            if validate_window_state(&state).is_ok() {
                Ok(state)
            } else {
                Ok(WindowState::default())
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(WindowState::default()),
        Err(error) => Err(error.into()),
    }
}

/// Writes the window state atomically.
///
/// # Errors
/// Returns `E_VALIDATION` for invalid geometry and `E_IO` for write failures.
pub fn save_window_state(path: &Path, state: &WindowState) -> Result<(), UiError> {
    validate_window_state(state)?;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let text = toml::to_string_pretty(state)?;
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)?;

    Ok(())
}

/// Joins the window state file to a data directory.
#[must_use]
pub fn window_state_path(data_dir: &Path) -> PathBuf {
    data_dir.join(WINDOW_STATE_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::{
        MonitorBounds, WINDOW_STATE_FILE_NAME, load_window_state, save_window_state,
        window_state_path,
    };
    use crate::dto::WindowState;

    fn monitor() -> MonitorBounds {
        MonitorBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            name: Some("primary".to_owned()),
        }
    }

    #[test]
    fn centres_the_window_without_monitor_information() {
        let state = WindowState {
            x: Some(-500),
            y: Some(50),
            ..WindowState::default()
        };
        let sanitized = state.sanitized(&[]);

        assert_eq!(sanitized.x, None);
        assert_eq!(sanitized.y, None);
        assert_eq!(sanitized.width, 1280);
    }

    #[test]
    fn keeps_a_visible_position() {
        let state = WindowState {
            x: Some(100),
            y: Some(100),
            ..WindowState::default()
        };
        let sanitized = state.sanitized(&[monitor()]);

        assert_eq!(sanitized.x, Some(100));
        assert_eq!(sanitized.y, Some(100));
    }

    #[test]
    fn centres_an_offscreen_window() {
        let state = WindowState {
            x: Some(5_000),
            y: Some(-4_000),
            ..WindowState::default()
        };
        let sanitized = state.sanitized(&[monitor()]);

        assert_eq!(sanitized.x, None);
        assert_eq!(sanitized.y, None);
    }

    #[test]
    fn clamps_sizes_to_the_documented_bounds() {
        let too_small = WindowState {
            width: 100,
            height: 100,
            ..WindowState::default()
        };
        let sanitized = too_small.sanitized(&[monitor()]);
        assert_eq!((sanitized.width, sanitized.height), (800, 600));

        let too_large = WindowState {
            width: 40_000,
            height: 40_000,
            ..WindowState::default()
        };
        let sanitized = too_large.sanitized(&[monitor()]);
        assert_eq!((sanitized.width, sanitized.height), (16_384, 16_384));
    }

    #[test]
    fn uses_the_named_monitor_when_it_exists() {
        let secondary = MonitorBounds {
            left: 1920,
            top: 0,
            width: 1280,
            height: 1024,
            name: Some("secondary".to_owned()),
        };
        let state = WindowState {
            x: Some(2_000),
            y: Some(100),
            monitor_id: Some("secondary".to_owned()),
            ..WindowState::default()
        };
        let sanitized = state.sanitized(&[monitor(), secondary]);

        assert_eq!(sanitized.x, Some(2_000));
        assert_eq!(sanitized.y, Some(100));
    }

    #[test]
    fn centres_a_window_when_its_monitor_is_disconnected() {
        let secondary = MonitorBounds {
            left: 1920,
            top: 0,
            width: 2560,
            height: 1440,
            name: Some("secondary".to_owned()),
        };
        let state = WindowState {
            x: Some(2_100),
            y: Some(120),
            monitor_id: Some("secondary".to_owned()),
            ..WindowState::default()
        };
        assert_eq!(
            state.sanitized(&[monitor(), secondary]).x,
            Some(2_100),
            "the named monitor is available"
        );

        let sanitized = state.sanitized(&[monitor()]);
        assert_eq!(sanitized.x, None);
        assert_eq!(sanitized.y, None);
    }

    #[test]
    fn dpi_change_that_shrinks_the_monitor_recentres_the_window() {
        let hidpi = MonitorBounds {
            left: 0,
            top: 0,
            width: 3840,
            height: 2160,
            name: Some("primary".to_owned()),
        };
        let state = WindowState {
            width: 1800,
            height: 1100,
            x: Some(2_400),
            y: Some(900),
            monitor_id: Some("primary".to_owned()),
            ..WindowState::default()
        };
        assert_eq!(state.sanitized(&[hidpi]).x, Some(2_400));

        // The same physical panel at 200% DPI reports half the logical work area.
        let scaled = MonitorBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            name: Some("primary".to_owned()),
        };
        let sanitized = state.sanitized(&[scaled]);
        assert_eq!(sanitized.x, None);
        assert_eq!(sanitized.y, None);
    }

    #[test]
    fn keeps_a_window_on_a_monitor_left_of_the_primary() {
        let left = MonitorBounds {
            left: -1920,
            top: 0,
            width: 1920,
            height: 1080,
            name: Some("left".to_owned()),
        };
        let state = WindowState {
            x: Some(-1800),
            y: Some(100),
            ..WindowState::default()
        };
        let sanitized = state.sanitized(&[monitor(), left]);

        assert_eq!(sanitized.x, Some(-1800));
        assert_eq!(sanitized.y, Some(100));
    }

    #[test]
    fn a_renamed_monitor_falls_back_to_coordinates() {
        let renamed = MonitorBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            name: Some(r"\\?\DISPLAY#NEW".to_owned()),
        };
        let state = WindowState {
            x: Some(200),
            y: Some(150),
            monitor_id: Some("old-device-id".to_owned()),
            ..WindowState::default()
        };
        let sanitized = state.sanitized(&[renamed]);

        assert_eq!(sanitized.x, Some(200));
        assert_eq!(sanitized.y, Some(150));
    }

    #[test]
    fn round_trips_through_a_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = window_state_path(directory.path());
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(WINDOW_STATE_FILE_NAME)
        );

        let state = WindowState {
            width: 1600,
            height: 900,
            x: Some(10),
            y: Some(20),
            maximized: true,
            monitor_id: Some("primary".to_owned()),
        };
        save_window_state(&path, &state).expect("save");
        assert_eq!(load_window_state(&path).expect("load"), state);

        std::fs::write(&path, "width = 10").expect("write invalid");
        assert_eq!(
            load_window_state(&path).expect("fallback"),
            WindowState::default()
        );
    }
}
