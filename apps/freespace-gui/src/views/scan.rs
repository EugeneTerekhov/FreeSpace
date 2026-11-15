//! Scan screen: choose a volume or directory, watch progress and cancel.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

use eframe::egui;
use freespace_core::{
    model::ScanIssue,
    size::{Units, format_bytes},
};
use freespace_platform::paths;
use freespace_scan::{
    MethodChoice, ScanObserver, ScanPhase, ScanProgress, ScanRequest, ScanResult,
};

/// Message sent from the scan worker to the UI.
#[derive(Debug)]
enum Message {
    Progress(ScanProgress),
    Issue(ScanIssue),
    Finished(Box<ScanResult>),
    Failed(String),
}

struct Bridge {
    sender: Sender<Message>,
    cancel: Arc<AtomicBool>,
}

impl ScanObserver for Bridge {
    fn progress(&mut self, progress: &ScanProgress) {
        let _ = self.sender.send(Message::Progress(progress.clone()));
    }

    fn issue(&mut self, issue: &ScanIssue) {
        let _ = self.sender.send(Message::Issue(issue.clone()));
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// State of the scan page.
#[derive(Debug)]
pub struct ScanState {
    /// Path being edited.
    pub path: String,
    /// Method as typed by the user (`auto`, `mft`, `walk`).
    pub method: String,
    /// Semicolon-separated exclusion globs.
    pub exclusions: String,
    running: bool,
    progress: Option<ScanProgress>,
    issues: Vec<ScanIssue>,
    error: Option<String>,
    result: Option<Arc<ScanResult>>,
    receiver: Option<Receiver<Message>>,
    cancel: Arc<AtomicBool>,
    finished_now: bool,
}

impl Default for ScanState {
    fn default() -> Self {
        let path = std::env::current_dir()
            .map(|path| paths::display(&path))
            .unwrap_or_else(|_| r"C:\".to_owned());

        Self {
            path,
            method: "auto".to_owned(),
            exclusions: String::new(),
            running: false,
            progress: None,
            issues: Vec::new(),
            error: None,
            result: None,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            finished_now: false,
        }
    }
}

impl ScanState {
    /// Returns the freshly completed scan exactly once.
    pub fn take_finished(&mut self) -> Option<Arc<ScanResult>> {
        if !self.finished_now {
            return None;
        }
        self.finished_now = false;

        self.result.clone()
    }

    /// Currently displayed result.
    #[cfg(test)]
    fn result(&self) -> Option<&Arc<ScanResult>> {
        self.result.as_ref()
    }

    fn start(&mut self) {
        if self.running {
            return;
        }
        let path = PathBuf::from(self.path.trim());
        if path.as_os_str().is_empty() {
            self.error = Some(t!("scan.empty_path").to_string());

            return;
        }
        let Some(method) = MethodChoice::parse_value(&self.method) else {
            self.error = Some(t!("scan.invalid_method").to_string());

            return;
        };
        let exclusions: Vec<String> = self
            .exclusions
            .split([';', ','])
            .map(str::trim)
            .filter(|pattern| !pattern.is_empty())
            .map(str::to_owned)
            .collect();
        let request = ScanRequest {
            root: path,
            method,
            exclusions,
            incremental: false,
        };

        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let name = t!("nav.scan").to_string();

        match thread::Builder::new().name(name).spawn(move || {
            let mut bridge = Bridge {
                sender,
                cancel: worker_cancel,
            };
            match freespace_scan::scan(&request, &mut bridge) {
                Ok(result) => {
                    let _ = bridge.sender.send(Message::Finished(Box::new(result)));
                }
                Err(error) => {
                    let _ = bridge.sender.send(Message::Failed(error.to_string()));
                }
            }
        }) {
            Ok(_handle) => {
                self.receiver = Some(receiver);
                self.cancel = cancel;
                self.running = true;
                self.finished_now = false;
                self.issues.clear();
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn poll(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        let mut finished = false;
        while let Ok(message) = receiver.try_recv() {
            match message {
                Message::Progress(progress) => self.progress = Some(progress),
                Message::Issue(issue) => self.issues.push(issue),
                Message::Finished(result) => {
                    self.result = Some(Arc::from(result));
                    self.running = false;
                    self.finished_now = true;
                    self.progress = None;
                    finished = true;
                }
                Message::Failed(error) => {
                    self.error = Some(error);
                    self.running = false;
                    self.progress = None;
                    finished = true;
                }
            }
        }
        if finished {
            self.receiver = None;
        }
    }

    /// Draws the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, units: Units) {
        self.poll();

        egui::Grid::new("scan_form")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(t!("scan.path").to_string());
                ui.add_enabled(
                    !self.running,
                    egui::TextEdit::singleline(&mut self.path).desired_width(520.0),
                );
                ui.end_row();

                ui.label(t!("scan.method").to_string());
                ui.add_enabled(
                    !self.running,
                    egui::TextEdit::singleline(&mut self.method).desired_width(120.0),
                );
                ui.end_row();

                ui.label(t!("scan.exclusions").to_string());
                ui.add_enabled(
                    !self.running,
                    egui::TextEdit::singleline(&mut self.exclusions).desired_width(520.0),
                );
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.running,
                    egui::Button::new(t!("scan.start").to_string()),
                )
                .clicked()
            {
                self.start();
            }
            if ui
                .add_enabled(
                    self.running,
                    egui::Button::new(t!("common.cancel").to_string()),
                )
                .clicked()
            {
                self.cancel();
            }
            if self.running {
                ui.spinner();
                ui.label(t!("status.running").to_string());
            }
        });

        if let Some(progress) = &self.progress {
            ui.add_space(8.0);
            let phase = match progress.phase {
                ScanPhase::Preparing => t!("scan.phase.preparing").to_string(),
                ScanPhase::Enumerating => t!("scan.phase.enumerating").to_string(),
                ScanPhase::Building => t!("scan.phase.building").to_string(),
                ScanPhase::Aggregating => t!("scan.phase.aggregating").to_string(),
                ScanPhase::Finished => t!("status.finished").to_string(),
            };
            ui.label(format!(
                "{phase} · {} · {}",
                t!("scan.objects", count = progress.nodes),
                format_bytes(progress.bytes, units)
            ));
            ui.add(egui::ProgressBar::new(progress.fraction).show_percentage());
            if !progress.current.as_os_str().is_empty() {
                ui.label(paths::display(&progress.current));
            }
        }

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }

        for issue in &self.issues {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!(
                    "{}: {} — {}",
                    issue.kind.as_str(),
                    paths::display(&issue.path),
                    issue.message
                ),
            );
        }

        if let Some(result) = &self.result {
            ui.add_space(12.0);
            ui.separator();
            let total = result
                .tree
                .roots()
                .next()
                .map_or(0, |root| result.tree.node(root).size.logical);
            ui.label(format!(
                "{} · {} · {}",
                t!("scan.method_applied"),
                result.method.as_str(),
                format_bytes(total, units)
            ));
            ui.label(format!(
                "{}: {}",
                t!("scan.objects_label"),
                result.session.nodes_count
            ));
            for warning in &result.warnings {
                ui.colored_label(ui.visuals().warn_fg_color, warning);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ScanState;

    #[test]
    fn has_sensible_defaults() {
        let state = ScanState::default();

        assert!(!state.path.is_empty());
        assert_eq!(state.method, "auto");
        assert!(!state.running);
        assert!(state.result().is_none());
        assert!(state.receiver.is_none());
    }

    #[test]
    fn rejects_an_empty_path() {
        let mut state = ScanState {
            path: "   ".to_owned(),
            ..ScanState::default()
        };

        state.start();

        assert!(!state.running);
        assert!(state.error.is_some());
    }

    #[test]
    fn rejects_an_unknown_method() {
        let mut state = ScanState {
            method: "magic".to_owned(),
            ..ScanState::default()
        };

        state.start();

        assert!(!state.running);
        assert!(state.error.is_some());
    }
}
