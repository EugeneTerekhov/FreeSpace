//! Application shell: navigation, settings and background-task plumbing.
//!
//! The GUI is a thin layer over the shared crates. Long-running work never
//! blocks the frame: tasks run on worker threads and report progress through a
//! channel that the shell drains on every update (FR-010, principle III).

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use eframe::egui;
use freespace_core::{
    i18n::{self, Locale},
    settings::{self, Settings, Theme},
    size::Units,
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
    /// File unlocking.
    Unlock,
    /// Application settings.
    Settings,
    /// About screen.
    About,
}

impl Page {
    /// Every screen in navigation order.
    pub const ALL: [Self; 12] = [
        Self::Dashboard,
        Self::Scan,
        Self::Tree,
        Self::Treemap,
        Self::Top,
        Self::Types,
        Self::Duplicates,
        Self::Cleanup,
        Self::Snapshots,
        Self::Unlock,
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
            Self::Unlock => t!("nav.unlock").to_string(),
            Self::Settings => t!("nav.settings").to_string(),
            Self::About => t!("nav.about").to_string(),
        }
    }
}

/// Cancellation flag shared with worker threads.
#[derive(Debug, Default, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Requests cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Returns `true` when cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Progress message sent from a worker thread to the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressUpdate {
    /// Task name.
    pub task: String,
    /// Completion fraction in `0.0..=1.0`.
    pub fraction: f32,
    /// Human-readable message.
    pub message: String,
    /// Whether the task finished.
    pub finished: bool,
    /// Failure message, when the task failed.
    pub failed: Option<String>,
}

impl ProgressUpdate {
    /// Creates an in-progress update; the fraction is clamped to `0.0..=1.0`.
    #[must_use]
    pub fn new(task: impl Into<String>, fraction: f32, message: impl Into<String>) -> Self {
        Self {
            task: task.into(),
            fraction: fraction.clamp(0.0, 1.0),
            message: message.into(),
            finished: false,
            failed: None,
        }
    }

    /// Creates a successful final update.
    #[must_use]
    pub fn finished(task: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            task: task.into(),
            fraction: 1.0,
            message: message.into(),
            finished: true,
            failed: None,
        }
    }

    /// Creates a failed final update.
    #[must_use]
    pub fn failed(task: impl Into<String>, message: impl Into<String>) -> Self {
        let message = message.into();

        Self {
            task: task.into(),
            fraction: 1.0,
            message: message.clone(),
            finished: true,
            failed: Some(message),
        }
    }
}

/// Worker-thread pool with a progress channel.
#[derive(Debug)]
pub struct TaskService {
    sender: Sender<ProgressUpdate>,
    receiver: Receiver<ProgressUpdate>,
    running: Vec<(String, CancellationToken)>,
}

impl TaskService {
    /// Creates an empty service.
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();

        Self {
            sender,
            receiver,
            running: Vec::new(),
        }
    }

    /// Spawns a named background task.
    pub fn spawn<F>(&mut self, name: impl Into<String>, work: F)
    where
        F: FnOnce(CancellationToken, Sender<ProgressUpdate>) + Send + 'static,
    {
        let name = name.into();
        let token = CancellationToken::default();
        let sender = self.sender.clone();
        let thread_token = token.clone();

        match thread::Builder::new()
            .name(name.clone())
            .spawn(move || work(thread_token, sender))
        {
            Ok(_handle) => self.running.push((name, token)),
            Err(error) => {
                let _ = self
                    .sender
                    .send(ProgressUpdate::failed(name, error.to_string()));
            }
        }
    }

    /// Drains pending progress updates.
    pub fn poll(&mut self) -> Vec<ProgressUpdate> {
        let updates: Vec<ProgressUpdate> = self.receiver.try_iter().collect();

        for update in &updates {
            if update.finished {
                self.running.retain(|(name, _)| name != &update.task);
            }
        }

        updates
    }

    /// Requests cancellation of every running task.
    pub fn cancel_all(&self) {
        for (_, token) in &self.running {
            token.cancel();
        }
    }

    /// Number of tasks that have not reported completion yet.
    #[must_use]
    pub fn running(&self) -> usize {
        self.running.len()
    }
}

impl Default for TaskService {
    fn default() -> Self {
        Self::new()
    }
}

/// Root application state.
pub struct FreeSpaceApp {
    page: Page,
    settings: Settings,
    settings_path: Option<PathBuf>,
    settings_saved: bool,
    tasks: TaskService,
    latest_progress: Option<ProgressUpdate>,
    last_error: Option<String>,
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
        Self {
            page: Page::default(),
            settings,
            settings_path,
            settings_saved: false,
            tasks: TaskService::new(),
            latest_progress: None,
            last_error: None,
        }
    }

    /// Current page.
    #[must_use]
    pub const fn page(&self) -> Page {
        self.page
    }

    /// Switches to another page.
    pub const fn set_page(&mut self, page: Page) {
        self.page = page;
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
                self.settings_path = Some(path);
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

    fn collect_progress(&mut self) {
        for update in self.tasks.poll() {
            self.latest_progress = Some(update);
        }
    }

    fn start_drive_refresh(&mut self) {
        self.tasks.spawn("drives", |token, sender| {
            if token.is_cancelled() {
                return;
            }
            let _ = sender.send(ProgressUpdate::new("drives", 0.2, "enumerating volumes"));
            match freespace_platform::volumes::list_volumes() {
                Ok(volumes) => {
                    let _ = sender.send(ProgressUpdate::finished(
                        "drives",
                        format!("{} volumes", volumes.len()),
                    ));
                }
                Err(error) => {
                    let _ = sender.send(ProgressUpdate::failed("drives", error.to_string()));
                }
            }
        });
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

                ui.separator();
                ui.label(t!("settings.tasks").to_string());
                if self.tasks.running() == 0 {
                    ui.label(t!("settings.no_tasks").to_string());
                } else {
                    ui.label(format!(
                        "{}: {}",
                        t!("status.running"),
                        self.tasks.running()
                    ));
                    if ui.button(t!("common.cancel").to_string()).clicked() {
                        self.tasks.cancel_all();
                    }
                }
            });
    }

    fn page_content(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.page().title());
        ui.add_space(8.0);

        match self.page {
            Page::Dashboard => self.dashboard(ui),
            Page::Settings => self.settings_page(ui),
            Page::About => {
                ui.label(t!("app.phase2_ready").to_string());
                ui.separator();
                ui.label(format!("FreeSpace {}", env!("CARGO_PKG_VERSION")));
            }
            _ => {
                ui.label(t!("page.placeholder").to_string());
            }
        }
    }

    fn dashboard(&mut self, ui: &mut egui::Ui) {
        ui.label(t!("app.phase2_ready").to_string());
        ui.add_space(12.0);

        if ui.button(t!("page.refresh_drives").to_string()).clicked() {
            self.start_drive_refresh();
        }

        if let Some(progress) = &self.latest_progress {
            ui.add_space(12.0);
            ui.label(format!("{}: {}", progress.task, progress.message));
            ui.add(egui::ProgressBar::new(progress.fraction).show_percentage());
            if let Some(error) = &progress.failed {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        }

        if let Some(error) = &self.last_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label(t!("settings.language").to_string());
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.settings.language, Locale::Ru, "Русский");
                    ui.selectable_value(&mut self.settings.language, Locale::En, "English");
                });
                ui.end_row();

                ui.label(t!("settings.theme").to_string());
                let mut theme = self.settings.theme;
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(format!("{theme:?}"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut theme, Theme::System, "System");
                        ui.selectable_value(&mut theme, Theme::Light, "Light");
                        ui.selectable_value(&mut theme, Theme::Dark, "Dark");
                    });
                if theme != self.settings.theme {
                    self.settings.theme = theme;
                    apply_theme(ui.ctx(), theme);
                    self.settings_saved = false;
                }
                ui.end_row();

                ui.label(t!("settings.units").to_string());
                let mut units = self.settings.units;
                egui::ComboBox::from_id_salt("units")
                    .selected_text(units.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut units, Units::Binary, "binary (KiB)");
                        ui.selectable_value(&mut units, Units::Decimal, "decimal (KB)");
                    });
                if units != self.settings.units {
                    self.settings.units = units;
                    self.settings_saved = false;
                }
                ui.end_row();

                ui.label(t!("settings.confirm").to_string());
                if ui
                    .checkbox(&mut self.settings.confirm_destructive_actions, "")
                    .changed()
                {
                    self.settings_saved = false;
                }
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(t!("settings.save").to_string()).clicked() {
                self.save_settings();
            }
            if self.settings_saved {
                ui.label(t!("settings.saved").to_string());
            }
            if let Some(error) = &self.last_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });

        if self.settings.language != i18n::locale() {
            self.settings.apply_language();
        }
    }
}

impl eframe::App for FreeSpaceApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.collect_progress();

        if self.tasks.running() > 0 {
            context.request_repaint_after(Duration::from_millis(100));
        }

        self.top_bar(context);
        self.sidebar(context);
        egui::CentralPanel::default().show(context, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.page_content(ui);
            });
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
    use std::{
        sync::Mutex,
        time::{Duration, Instant},
    };

    use freespace_core::{
        i18n::Locale,
        settings::{Settings, settings_path},
    };

    use super::{FreeSpaceApp, Page, ProgressUpdate, TaskService};

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
    fn clamps_progress_fractions() {
        assert_eq!(ProgressUpdate::new("task", -1.0, "x").fraction, 0.0);
        assert_eq!(ProgressUpdate::new("task", 2.0, "x").fraction, 1.0);
        assert!(ProgressUpdate::finished("task", "done").finished);
        assert!(ProgressUpdate::failed("task", "boom").failed.is_some());
    }

    #[test]
    fn background_tasks_report_progress() {
        let mut service = TaskService::new();

        service.spawn("demo", |token, sender| {
            assert!(!token.is_cancelled());
            let _ = sender.send(ProgressUpdate::new("demo", 0.5, "halfway"));
            let _ = sender.send(ProgressUpdate::finished("demo", "done"));
        });

        assert_eq!(service.running(), 1);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut updates = Vec::new();
        while updates.len() < 2 && Instant::now() < deadline {
            updates.extend(service.poll());
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].fraction, 0.5);
        assert!(updates[1].finished);
        assert_eq!(service.running(), 0);
    }

    #[test]
    fn cancellation_token_is_shared() {
        let token = super::CancellationToken::default();
        let clone = token.clone();

        clone.cancel();

        assert!(token.is_cancelled());
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
