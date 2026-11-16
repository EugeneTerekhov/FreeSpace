//! Application shell: navigation, settings and screen composition.
//!
//! The GUI is a thin layer over the shared crates. Long-running work never
//! blocks the frame: screens run their workers on threads and report progress
//! through channels that are drained on every update (FR-010, principle III).

use std::{path::PathBuf, sync::Arc};

use eframe::egui;
use freespace_core::{
    i18n::{self, Locale},
    settings::{self, Settings, Theme},
    tree::NodeId,
};

use crate::{
    treemap::TreemapView,
    views::{
        ArchivesState, CleanupState, DashboardState, DuplicatesState, ElevationState, ExportState,
        JournalState, LongPathsState, MetadataState, MonitorState, OwnersState, PermissionsState,
        RenameState, ScanState, SettingsAction, SettingsState, ShredState, SizeDetailsState,
        SnapshotsState, TopTypesState, TopTypesTab, TreeView, UnlockState,
    },
};

/// Top-level screens.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    /// Drive dashboard.
    #[default]
    Dashboard,
    /// Volume and directory scanning.
    Scan,
    /// Size tree.
    Tree,
    /// Treemap.
    Treemap,
    /// Largest files.
    Top,
    /// File types.
    Types,
    /// Duplicate search.
    Duplicates,
    /// Cleanup recommendations.
    Cleanup,
    /// Disk snapshots.
    Snapshots,
    /// Operation journal and undo.
    Journal,
    /// Exact size details and alternate data streams.
    SizeDetails,
    /// Long-path file manager.
    LongPaths,
    /// Bulk rename.
    Rename,
    /// Attributes and NTFS permissions.
    Permissions,
    /// EXIF, media and PE metadata.
    Metadata,
    /// Space by file owner.
    Owners,
    /// Archive contents.
    Archives,
    /// Report export.
    Export,
    /// File unlocking.
    Unlock,
    /// Irreversible destruction.
    Shred,
    /// Write monitor.
    Monitor,
    /// Elevation and protected directories.
    Elevation,
    /// Application settings.
    Settings,
    /// About screen.
    About,
}

impl Page {
    /// Every screen in navigation order.
    pub const ALL: [Self; 24] = [
        Self::Dashboard,
        Self::Scan,
        Self::Tree,
        Self::Treemap,
        Self::Top,
        Self::Types,
        Self::Duplicates,
        Self::Cleanup,
        Self::Snapshots,
        Self::Journal,
        Self::SizeDetails,
        Self::LongPaths,
        Self::Rename,
        Self::Permissions,
        Self::Metadata,
        Self::Owners,
        Self::Archives,
        Self::Export,
        Self::Unlock,
        Self::Shred,
        Self::Monitor,
        Self::Elevation,
        Self::Settings,
        Self::About,
    ];

    /// Translated page title.
    #[must_use]
    pub fn title(self) -> String {
        match self {
            Self::Dashboard => t!("nav.dashboard").to_string(),
            Self::Scan => t!("nav.scan").to_string(),
            Self::Tree => t!("nav.tree").to_string(),
            Self::Treemap => t!("nav.treemap").to_string(),
            Self::Top => t!("nav.top").to_string(),
            Self::Types => t!("nav.types").to_string(),
            Self::Duplicates => t!("nav.duplicates").to_string(),
            Self::Cleanup => t!("nav.cleanup").to_string(),
            Self::Snapshots => t!("nav.snapshots").to_string(),
            Self::Journal => t!("nav.journal").to_string(),
            Self::SizeDetails => t!("nav.size_details").to_string(),
            Self::LongPaths => t!("nav.long_paths").to_string(),
            Self::Rename => t!("nav.rename").to_string(),
            Self::Permissions => t!("nav.permissions").to_string(),
            Self::Metadata => t!("nav.metadata").to_string(),
            Self::Owners => t!("nav.owners").to_string(),
            Self::Archives => t!("nav.archives").to_string(),
            Self::Export => t!("nav.export").to_string(),
            Self::Unlock => t!("nav.unlock").to_string(),
            Self::Shred => t!("nav.shred").to_string(),
            Self::Monitor => t!("nav.monitor").to_string(),
            Self::Elevation => t!("nav.elevation").to_string(),
            Self::Settings => t!("nav.settings").to_string(),
            Self::About => t!("nav.about").to_string(),
        }
    }
}

/// Root application state.
pub struct FreeSpaceApp {
    page: Page,
    settings: Settings,
    settings_path: Option<PathBuf>,
    settings_saved: bool,
    last_error: Option<String>,
    scan: ScanState,
    scan_result: Option<Arc<freespace_scan::ScanResult>>,
    tree_view: TreeView,
    treemap: TreemapView,
    top_types: TopTypesState,
    duplicates: DuplicatesState,
    cleanup: CleanupState,
    snapshots: SnapshotsState,
    journal: JournalState,
    size_details: SizeDetailsState,
    long_paths: LongPathsState,
    rename: RenameState,
    permissions: PermissionsState,
    metadata: MetadataState,
    owners: OwnersState,
    archives: ArchivesState,
    export: ExportState,
    unlock: UnlockState,
    shred: ShredState,
    dashboard: DashboardState,
    monitor: MonitorState,
    elevation: ElevationState,
    settings_state: SettingsState,
}

impl FreeSpaceApp {
    /// Creates the application, loads settings and applies the stored theme.
    #[must_use]
    pub fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        let settings_path = settings::default_path().ok();
        let settings = settings_path
            .as_deref()
            .and_then(|path| Settings::load(path).ok())
            .unwrap_or_default();
        settings.apply_language();
        apply_theme(&creation_context.egui_ctx, settings.theme);
        creation_context
            .egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Title(window_title()));

        Self::with_settings(settings, settings_path)
    }

    /// Creates the shell from explicit settings; used by tests.
    #[must_use]
    pub fn with_settings(settings: Settings, settings_path: Option<PathBuf>) -> Self {
        let mut settings_state = SettingsState::default();
        settings_state.set_path(settings_path.as_deref());

        Self {
            page: Page::default(),
            settings,
            settings_path,
            settings_saved: false,
            last_error: None,
            scan: ScanState::default(),
            scan_result: None,
            tree_view: TreeView::default(),
            treemap: TreemapView::default(),
            top_types: TopTypesState::default(),
            duplicates: DuplicatesState::default(),
            cleanup: CleanupState::default(),
            snapshots: SnapshotsState::default(),
            journal: JournalState::default(),
            size_details: SizeDetailsState::default(),
            long_paths: LongPathsState::default(),
            rename: RenameState::default(),
            permissions: PermissionsState::default(),
            metadata: MetadataState::default(),
            owners: OwnersState::default(),
            archives: ArchivesState::default(),
            export: ExportState::default(),
            unlock: UnlockState::default(),
            shred: ShredState::default(),
            dashboard: DashboardState::default(),
            monitor: MonitorState::default(),
            elevation: ElevationState::default(),
            settings_state,
        }
    }

    /// Current page.
    #[must_use]
    pub const fn page(&self) -> Page {
        self.page
    }

    /// Switches to another page.
    pub fn set_page(&mut self, page: Page) {
        self.page = page;
        match page {
            Page::Top => self.top_types.set_tab(TopTypesTab::Top),
            Page::Types => self.top_types.set_tab(TopTypesTab::Types),
            _ => {}
        }
    }

    /// Writes owner indices produced by the owners worker into the scan tree.
    fn apply_owner_assignments(&mut self, assignments: Vec<(NodeId, Option<u32>)>) {
        let Some(result) = &mut self.scan_result else {
            return;
        };
        let Some(result) = Arc::get_mut(result) else {
            self.owners
                .set_error("the scan result is shared; rescan before filtering by owner");
            return;
        };
        for (node_id, owner_id) in assignments {
            result.tree.node_mut(node_id).owner_id = owner_id;
        }
    }

    /// Persists settings; returns `true` on success.
    pub fn save_settings(&mut self) -> bool {
        let path = self
            .settings_path
            .clone()
            .or_else(|| settings::default_path().ok());

        let Some(path) = path else {
            self.last_error = Some("cannot resolve the settings path".to_owned());
            return false;
        };

        match self.settings.save(&path) {
            Ok(()) => {
                self.settings_path = Some(path.clone());
                self.settings_state.set_path(Some(&path));
                self.settings_saved = true;
                self.last_error = None;
                true
            }
            Err(error) => {
                self.settings_saved = false;
                self.last_error = Some(error.to_string());
                false
            }
        }
    }

    fn top_bar(&mut self, context: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.heading(t!("app.name").to_string());
                ui.separator();
                ui.label(t!("app.tagline").to_string());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut language = self.settings.language;
                    egui::ComboBox::from_id_salt("language")
                        .selected_text(match language {
                            Locale::Ru => "Русский",
                            Locale::En => "English",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut language, Locale::Ru, "Русский");
                            ui.selectable_value(&mut language, Locale::En, "English");
                        });
                    if language != self.settings.language {
                        self.settings.language = language;
                        i18n::set_locale(language);
                        self.settings_saved = false;
                    }
                });
            });
        });
    }

    fn sidebar(&mut self, context: &egui::Context) {
        egui::SidePanel::left("navigation")
            .resizable(false)
            .show(context, |ui| {
                ui.add_space(8.0);
                for page in Page::ALL {
                    if ui
                        .selectable_label(self.page == page, page.title())
                        .clicked()
                    {
                        self.set_page(page);
                    }
                }
            });
    }

    fn page_content(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.page().title());
        ui.add_space(8.0);

        match self.page {
            Page::Dashboard => {
                if let Some(mount_point) = self.dashboard.ui(ui, self.settings.units) {
                    self.scan.begin(&mount_point);
                    self.set_page(Page::Scan);
                }
            }
            Page::Scan => {
                self.scan.ui(ui, self.settings.units);
                if let Some(result) = self.scan.take_finished() {
                    self.tree_view.reset();
                    self.treemap.reset(result.tree.roots().next());
                    self.top_types.reset();
                    self.duplicates.reset();
                    self.cleanup.reset();
                    self.snapshots.reset();
                    self.metadata.reset();
                    self.owners.reset();
                    self.archives.reset();
                    self.export.reset();
                    self.scan_result = Some(result);
                }
            }
            Page::Tree => {
                if let Some(result) = &self.scan_result {
                    self.tree_view.ui(ui, result, self.settings.units);
                } else {
                    ui.label(t!("scan.required").to_string());
                }
            }
            Page::Treemap => {
                if let Some(result) = &self.scan_result {
                    self.treemap.ui(ui, &result.tree, self.settings.units);
                } else {
                    ui.label(t!("scan.required").to_string());
                }
            }
            Page::Top | Page::Types => {
                let mut selected = None;
                if let Some(result) = &self.scan_result {
                    selected = self.top_types.ui(ui, result, self.settings.units);
                } else {
                    ui.label(t!("scan.required").to_string());
                }
                if let Some(id) = selected {
                    if let Some(result) = &self.scan_result {
                        self.tree_view.reveal(&result.tree, id);
                    }
                    self.set_page(Page::Tree);
                }
            }
            Page::Duplicates => {
                if let Some(result) = self.scan_result.clone() {
                    self.duplicates.ui(ui, &result, self.settings.units);
                } else {
                    ui.label(t!("scan.required").to_string());
                }
            }
            Page::Cleanup => {
                if let Some(result) = &self.scan_result {
                    self.cleanup.ui(ui, result, self.settings.units);
                } else {
                    ui.label(t!("scan.required").to_string());
                }
            }
            Page::Snapshots => {
                self.snapshots
                    .ui(ui, self.scan_result.as_deref(), self.settings.units);
            }
            Page::Journal => self
                .journal
                .ui(ui, self.settings.portable_mode, self.settings.units),
            Page::SizeDetails => self.size_details.ui(ui, self.settings.units),
            Page::LongPaths => self.long_paths.ui(ui, self.settings.units),
            Page::Rename => self.rename.ui(ui),
            Page::Permissions => self.permissions.ui(ui),
            Page::Metadata => {
                if let Some(result) = &self.scan_result {
                    self.metadata.ui(ui, result, self.settings.units);
                } else {
                    ui.label(t!("scan.required").to_string());
                }
            }
            Page::Owners => {
                let mut filter = None;
                if let Some(result) = &self.scan_result {
                    filter = self.owners.ui(ui, result, self.settings.units);
                } else {
                    ui.label(t!("scan.required").to_string());
                }
                if let Some(assignments) = self.owners.take_pending() {
                    self.apply_owner_assignments(assignments);
                }
                if let Some(owner) = filter {
                    self.tree_view.set_owner_filter(Some(owner));
                    if let Some(result) = &self.scan_result {
                        self.tree_view.expand_all(&result.tree);
                    }
                    self.set_page(Page::Tree);
                }
            }
            Page::Archives => self.archives.ui(ui, self.settings.units),
            Page::Export => {
                if let Some(result) = &self.scan_result {
                    self.export.ui(ui, result, self.settings.units);
                } else {
                    ui.label(t!("scan.required").to_string());
                }
            }
            Page::Unlock => self.unlock.ui(ui),
            Page::Shred => self.shred.ui(ui, self.settings.units),
            Page::Monitor => self.monitor.ui(ui, &self.settings, self.settings.units),
            Page::Elevation => self.elevation.ui(ui, self.settings.units),
            Page::Settings => {
                let action = self.settings_state.ui(ui, &mut self.settings);
                match action {
                    SettingsAction::None => {}
                    SettingsAction::Changed => {
                        self.settings.apply_language();
                        apply_theme(ui.ctx(), self.settings.theme);
                    }
                    SettingsAction::Save => {
                        let saved = self.save_settings();
                        let result = if saved {
                            Ok(())
                        } else {
                            Err(self
                                .last_error
                                .clone()
                                .unwrap_or_else(|| "cannot save settings".to_owned()))
                        };
                        self.settings_state.set_result(result);
                    }
                }
            }
            Page::About => {
                ui.label(t!("app.phase25_ready").to_string());
                ui.separator();
                ui.label(format!("FreeSpace {}", env!("CARGO_PKG_VERSION")));
            }
        }
    }
}

impl eframe::App for FreeSpaceApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        if self.monitor.is_running() {
            context.request_repaint_after(std::time::Duration::from_millis(500));
        }

        self.top_bar(context);
        self.sidebar(context);
        egui::CentralPanel::default().show(context, |ui| {
            if matches!(self.page, Page::Tree | Page::Treemap) {
                self.page_content(ui);
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.page_content(ui);
                });
            }
        });
    }
}

fn apply_theme(context: &egui::Context, theme: Theme) {
    let preference = match theme {
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
        Theme::System => egui::ThemePreference::System,
    };
    context.set_theme(preference);
}

/// Renders the window title, including the configured language.
#[must_use]
pub fn window_title() -> String {
    t!("app.name").to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use freespace_core::{
        i18n::Locale,
        settings::{Settings, settings_path},
    };

    use super::{FreeSpaceApp, Page};

    static LOCALE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn every_page_has_a_title() {
        let _guard = LOCALE_LOCK.lock().expect("locale lock");
        freespace_core::i18n::set_locale(Locale::En);

        let mut titles = Vec::new();
        for page in Page::ALL {
            let title = page.title();
            assert!(!title.is_empty(), "page {page:?} has no title");
            titles.push(title);
        }

        titles.sort();
        titles.dedup();
        assert_eq!(titles.len(), Page::ALL.len(), "titles must be unique");
    }

    #[test]
    fn settings_are_persisted() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("settings.toml");
        let mut settings = Settings::default();
        settings.set_value("units", "decimal").expect("units");
        let mut app = FreeSpaceApp::with_settings(settings, Some(path.clone()));

        assert_eq!(app.page(), Page::Dashboard);
        assert!(app.save_settings());

        let loaded = Settings::load(&path).expect("load");
        assert_eq!(loaded.units, freespace_core::size::Units::Decimal);
    }

    #[test]
    fn portable_settings_path_is_used() {
        let path = settings_path(std::path::Path::new(r"C:\Tools\FreeSpace"), None, true);

        assert!(path.ends_with("settings.toml"));
    }
}
