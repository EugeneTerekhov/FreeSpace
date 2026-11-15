//! Duplicate search screen: verified groups, keep rules and safe removal.
//!
//! The search itself runs on a worker thread so the frame stays responsive
//! (principle III); results arrive through a channel polled by the UI.
//! Removal never destroys data directly: selected copies go to the quarantine,
//! and the screen refuses to select every copy of a group (FR-029).

use std::{
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
    thread,
    time::Duration,
};

use eframe::egui;
use freespace_analysis::{
    DuplicateOptions, DuplicateSearch, HashAlgorithm, HashCache, KeepRule, apply_keep_rule,
    find_duplicates,
};
use freespace_core::{
    Result,
    size::{Units, format_bytes},
};
use freespace_ops::{Quarantine, Store, store};
use freespace_platform::paths;
use freespace_scan::ScanResult;

use crate::treemap::colors::CategoryColor;

/// Retention of duplicate copies in the quarantine.
const QUARANTINE_DAYS: u32 = 30;

/// Background search state.
#[derive(Debug)]
enum SearchState {
    Idle,
    Running(Receiver<Result<DuplicateSearch>>),
    Done(DuplicateSearch),
    Failed(String),
}

/// State of the duplicates screen.
#[derive(Debug)]
pub struct DuplicatesState {
    min_size: u64,
    algorithm: HashAlgorithm,
    keep_rule: KeepRule,
    search: SearchState,
    confirm_delete: bool,
    message: Option<String>,
    error: Option<String>,
}

impl Default for DuplicatesState {
    fn default() -> Self {
        Self {
            min_size: 1,
            algorithm: HashAlgorithm::default(),
            keep_rule: KeepRule::default(),
            search: SearchState::Idle,
            confirm_delete: false,
            message: None,
            error: None,
        }
    }
}

impl DuplicatesState {
    /// Clears results after a new scan.
    pub fn reset(&mut self) {
        self.search = SearchState::Idle;
        self.confirm_delete = false;
        self.message = None;
        self.error = None;
    }

    /// Number of groups in the current result.
    #[cfg(test)]
    #[must_use]
    pub fn group_count(&self) -> usize {
        match &self.search {
            SearchState::Done(search) => search.groups.len(),
            _ => 0,
        }
    }

    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, result: &Arc<ScanResult>, units: Units) {
        self.poll();

        ui.label(t!("duplicates.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            let area = result
                .session
                .roots
                .iter()
                .map(|root| paths::display(root))
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(t!("duplicates.area").to_string());
            ui.code(area);

            ui.label(t!("duplicates.min_size").to_string());
            ui.add(egui::DragValue::new(&mut self.min_size).range(1..=u64::MAX));

            ui.label(t!("duplicates.algorithm").to_string());
            egui::ComboBox::from_id_salt("duplicates_algorithm")
                .selected_text(self.algorithm.as_str())
                .show_ui(ui, |ui| {
                    for algorithm in [HashAlgorithm::Sha256, HashAlgorithm::Md5] {
                        ui.selectable_value(&mut self.algorithm, algorithm, algorithm.as_str());
                    }
                });

            ui.label(t!("duplicates.keep_rule").to_string());
            egui::ComboBox::from_id_salt("duplicates_keep_rule")
                .selected_text(self.keep_rule.as_str())
                .show_ui(ui, |ui| {
                    for rule in [
                        KeepRule::Oldest,
                        KeepRule::Newest,
                        KeepRule::Shortest,
                        KeepRule::OriginalDir,
                    ] {
                        ui.selectable_value(&mut self.keep_rule, rule, rule.as_str());
                    }
                });

            let running = matches!(self.search, SearchState::Running(_));
            if ui
                .add_enabled(
                    !running,
                    egui::Button::new(t!("duplicates.search").to_string()),
                )
                .clicked()
            {
                self.start(result);
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        if matches!(self.search, SearchState::Running(_)) {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(t!("duplicates.searching").to_string());
            });
            ui.ctx().request_repaint_after(Duration::from_millis(100));

            return;
        }

        if let SearchState::Failed(message) = &self.search {
            ui.colored_label(ui.visuals().error_fg_color, message.clone());

            return;
        }

        let mut delete_requested = false;
        {
            let SearchState::Done(search) = &mut self.search else {
                ui.label(t!("duplicates.empty").to_string());

                return;
            };

            ui.add_space(8.0);
            ui.label(format!(
                "{} · {} · {}",
                t!("duplicates.groups", count = search.groups.len()),
                t!(
                    "duplicates.reclaimable",
                    size = format_bytes(search.reclaimable_bytes, units)
                ),
                t!("duplicates.hashed", count = search.hashed)
            ));
            ui.separator();
            ui.add(egui::Label::new(t!("duplicates.kept_hint").to_string()).wrap());

            let mut toggle: Option<(usize, usize)> = None;
            for (group_index, group) in search.groups.iter().enumerate() {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.strong(if group.hash.len() > 12 {
                        &group.hash[..12]
                    } else {
                        &group.hash
                    });
                    ui.label(format!(
                        "{} × {}",
                        group.members.len(),
                        format_bytes(group.size, units)
                    ));
                    ui.label(format_bytes(group.reclaimable_bytes, units));
                    if group.stale {
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            t!("duplicates.stale").to_string(),
                        );
                    }
                });

                let last_kept = group.selected_count() + 1 == group.members.len();
                for (member_index, member) in group.members.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        let mut checked = member.selected_for_deletion;
                        let only_kept = !member.selected_for_deletion && last_kept;
                        if ui
                            .add_enabled(
                                !group.stale && !only_kept,
                                egui::Checkbox::without_text(&mut checked),
                            )
                            .changed()
                        {
                            toggle = Some((group_index, member_index));
                        }
                        ui.label(paths::display(&member.path));
                        let category = member.path.file_name().map_or_else(
                            || freespace_analysis::FileCategory::Other,
                            |name| freespace_analysis::category_for(&name.to_string_lossy()),
                        );
                        ui.colored_label(category.color(), t!(category.label_key()).to_string());
                        if member.selected_for_deletion {
                            ui.colored_label(
                                ui.visuals().warn_fg_color,
                                t!("duplicates.delete").to_string(),
                            );
                        } else {
                            ui.label(t!("duplicates.keep").to_string());
                        }
                    });
                }
            }

            if let Some((group_index, member_index)) = toggle {
                if let Some(group) = search.groups.get_mut(group_index) {
                    if let Some(member) = group.members.get_mut(member_index) {
                        member.selected_for_deletion = !member.selected_for_deletion;
                    }
                }
            }

            ui.add_space(12.0);
            ui.separator();
            let selected = search
                .groups
                .iter()
                .map(freespace_analysis::DuplicateGroup::selected_count)
                .sum::<usize>();
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.confirm_delete,
                    t!("duplicates.confirm").to_string(),
                );
                if ui
                    .add_enabled(
                        self.confirm_delete && selected > 0,
                        egui::Button::new(t!("duplicates.quarantine").to_string()),
                    )
                    .clicked()
                {
                    delete_requested = true;
                }
            });
        }

        if delete_requested {
            self.confirm_delete = false;
            self.delete_selected();
        }
    }

    fn start(&mut self, result: &Arc<ScanResult>) {
        self.error = None;
        self.message = None;
        self.confirm_delete = false;

        let options = DuplicateOptions {
            min_size: self.min_size,
            algorithm: self.algorithm,
        };
        let result = Arc::clone(result);
        let (sender, receiver) = mpsc::channel();
        self.search = SearchState::Running(receiver);

        let spawned = thread::Builder::new()
            .name("duplicates".to_owned())
            .spawn(move || {
                let cache = store::default_path()
                    .ok()
                    .and_then(|path| HashCache::open(&path).ok());
                let outcome = find_duplicates(&result.tree, options, cache.as_ref());
                let _ = sender.send(outcome);
            });

        if let Err(error) = spawned {
            self.search = SearchState::Failed(error.to_string());
        }
    }

    fn poll(&mut self) {
        let SearchState::Running(receiver) = &self.search else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(mut search)) => {
                for group in &mut search.groups {
                    apply_keep_rule(group, self.keep_rule);
                }
                self.search = SearchState::Done(search);
            }
            Ok(Err(error)) => self.search = SearchState::Failed(error.to_string()),
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.search = SearchState::Failed("the search thread stopped".to_owned());
            }
        }
    }

    fn delete_selected(&mut self) {
        let SearchState::Done(search) = &mut self.search else {
            return;
        };

        let Ok(database_path) = store::default_path() else {
            self.error = Some("cannot resolve the journal path".to_owned());
            return;
        };
        let Ok(database) = Store::open(&database_path) else {
            self.error = Some("cannot open the journal".to_owned());
            return;
        };
        let Ok(root) = Quarantine::default_root(false) else {
            self.error = Some("cannot resolve the quarantine directory".to_owned());
            return;
        };
        let Ok(quarantine) = Quarantine::new(root) else {
            self.error = Some("cannot create the quarantine directory".to_owned());
            return;
        };

        let mut removed = 0usize;
        let mut failures = 0usize;
        let mut kept_groups = Vec::new();

        for mut group in search.groups.drain(..) {
            if group.stale {
                kept_groups.push(group);
                continue;
            }
            let selected: Vec<PathBuf> = group
                .members
                .iter()
                .filter(|member| member.selected_for_deletion)
                .map(|member| member.path.clone())
                .collect();
            if selected.is_empty() {
                kept_groups.push(group);
                continue;
            }
            if selected.len() >= group.members.len() {
                self.error = Some(t!("duplicates.keep_one").to_string());
                kept_groups.push(group);
                continue;
            }

            let mut failed = false;
            for path in selected {
                match quarantine.place(&database, &path, "duplicate", QUARANTINE_DAYS) {
                    Ok(_) => removed += 1,
                    Err(error) => {
                        failures += 1;
                        failed = true;
                        self.error = Some(error.to_string());
                    }
                }
            }
            if failed {
                group.stale = true;
                kept_groups.push(group);
            }
        }

        search.groups = kept_groups;
        search.reclaimable_bytes = search
            .groups
            .iter()
            .map(|group| group.reclaimable_bytes)
            .fold(0u64, u64::saturating_add);
        self.message = Some(format!(
            "{} · {} · {}",
            t!("duplicates.moved", count = removed),
            t!("duplicates.failed", count = failures),
            t!("duplicates.undo_hint")
        ));
    }

    /// Paths that would be deleted, used by tests.
    #[cfg(test)]
    #[must_use]
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        let SearchState::Done(search) = &self.search else {
            return Vec::new();
        };
        search
            .groups
            .iter()
            .flat_map(|group| group.members.iter())
            .filter(|member| member.selected_for_deletion)
            .map(|member| member.path.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::DuplicatesState;

    #[test]
    fn starts_empty() {
        let state = DuplicatesState::default();

        assert_eq!(state.group_count(), 0);
        assert!(state.selected_paths().is_empty());
    }

    #[test]
    fn surfaces_search_failures() {
        let state = DuplicatesState {
            search: super::SearchState::Failed("boom".to_owned()),
            ..DuplicatesState::default()
        };

        assert_eq!(state.group_count(), 0);
    }
}
