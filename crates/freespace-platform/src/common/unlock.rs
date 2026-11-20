//! Platform-neutral records about processes holding a file.

/// Type of the application that holds a file.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApplicationType {
    /// Unknown application type.
    #[default]
    Unknown,
    /// Application with a main window.
    MainWindow,
    /// Application with another window.
    OtherWindow,
    /// Windows service.
    Service,
    /// Windows Explorer.
    Explorer,
    /// Console application.
    Console,
    /// Critical system process; must not be terminated.
    Critical,
}

impl ApplicationType {
    /// Maps the raw `RM_APP_TYPE` value.
    #[must_use]
    pub const fn from_raw(value: i32) -> Self {
        match value {
            1 => Self::MainWindow,
            2 => Self::OtherWindow,
            3 => Self::Service,
            4 => Self::Explorer,
            5 => Self::Console,
            1000 => Self::Critical,
            _ => Self::Unknown,
        }
    }

    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::MainWindow => "main-window",
            Self::OtherWindow => "other-window",
            Self::Service => "service",
            Self::Explorer => "explorer",
            Self::Console => "console",
            Self::Critical => "critical",
        }
    }

    /// Returns `true` when terminating the application is safe.
    #[must_use]
    pub const fn is_terminable(self) -> bool {
        !matches!(self, Self::Critical | Self::Unknown)
    }
}

/// One process that uses a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLock {
    /// Process identifier.
    pub process_id: u32,
    /// Application name reported by the platform.
    pub process_name: String,
    /// Service short name, when the holder is a service.
    pub service_name: String,
    /// Application type.
    pub application_type: ApplicationType,
    /// Whether the manager can ask the application to close.
    pub can_soft_close: bool,
    /// Whether a reboot is required to release the object.
    pub requires_reboot: bool,
}

impl FileLock {
    /// Returns `true` when the process is critical and must be left alone.
    #[must_use]
    pub const fn is_critical(&self) -> bool {
        matches!(self.application_type, ApplicationType::Critical)
    }
}
