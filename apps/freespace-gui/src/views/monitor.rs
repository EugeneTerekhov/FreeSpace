//! Write-monitor screen (US20).
//!
//! The ETW consumer or the counter fallback runs on a worker thread and reports
//! events through a channel; the UI only drains the channel, so the frame stays
//! responsive and monitoring keeps running while another page is open.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use eframe::egui;
use freespace_analysis::monitor::{
    DEFAULT_HISTORY_CAPACITY, MonitorConfig, MonitorEvent, MonitorHistory,
};
use freespace_core::{
    Timestamp,
    settings::Settings,
    size::{Units, format_bytes},
};

/// State of the monitor screen.
#[derive(Debug, Default)]
pub struct MonitorState {
    threshold: String,
    interval: String,
    receiver: Option<Receiver<MonitorEvent>>,
    stop: Option<Arc<AtomicBool>>,
    worker: Option<JoinHandle<()>>,
    history: Option<MonitorHistory>,
    config: Option<MonitorConfig>,
    notification: Option<String>,
    source: String,
    error: Option<String>,
}

impl MonitorState {
    /// Returns whether the worker is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.worker.is_some()
    }

    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, settings: &Settings, units: Units) {
        ui.label(t!("monitor.intro").to_string());
        ui.add_space(8.0);

        if !self.is_running() {
            if self.threshold.is_empty() {
                self.threshold = settings.monitor_threshold.threshold_bytes.to_string();
            }
            if self.interval.is_empty() {
                self.interval = settings.monitor_threshold.interval_secs.to_string();
            }
        }

        ui.horizontal(|ui| {
            ui.label(t!("monitor.threshold").to_string());
            ui.add(egui::TextEdit::singleline(&mut self.threshold).desired_width(120.0));
            ui.label(t!("monitor.interval").to_string());
            ui.add(egui::TextEdit::singleline(&mut self.interval).desired_width(60.0));
            if self.is_running() {
                if ui.button(t!("monitor.stop").to_string()).clicked() {
                    self.stop();
                }
            } else if ui.button(t!("monitor.start").to_string()).clicked() {
                self.start(settings);
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(notification) = &self.notification {
            ui.colored_label(ui.visuals().warn_fg_color, notification);
        }
        if !self.source.is_empty() {
            ui.label(&self.source);
        }

        self.drain(units);

        let Some(history) = &self.history else {
            return;
        };

        ui.add_space(8.0);
        ui.separator();
        if history.events().is_empty() {
            ui.label(t!("monitor.no_events").to_string());
            return;
        }

        ui.label(format!(
            "{}: {} · {}",
            t!("monitor.summary"),
            history.events().len(),
            format_bytes(history.total_bytes(), units)
        ));
        egui::Grid::new("monitor_grid")
            .num_columns(4)
            .striped(true)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                for heading in [
                    t!("monitor.pid").to_string(),
                    t!("monitor.process").to_string(),
                    t!("monitor.bytes").to_string(),
                    t!("monitor.paths").to_string(),
                ] {
                    ui.strong(heading);
                }
                ui.end_row();

                for event in history.recent(100) {
                    ui.label(event.process_id.to_string());
                    ui.label(&event.process);
                    ui.label(format_bytes(event.bytes, units));
                    ui.label(event.paths.join("; "));
                    ui.end_row();
                }
            });
    }

    fn start(&mut self, settings: &Settings) {
        self.stop();
        let config = MonitorConfig::from_settings(settings);
        let mut config = config;
        if let Ok(threshold) = self.threshold.trim().parse::<u64>() {
            config.threshold_bytes = threshold;
        }
        if let Ok(interval) = self.interval.trim().parse::<u64>() {
            config.interval = Duration::from_secs(interval.max(1));
        }

        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let interval = config.interval.max(Duration::from_millis(250));

        let worker = thread::Builder::new()
            .name("freespace-monitor".to_owned())
            .spawn(move || {
                let (etw_sender, etw_receiver) = mpsc::channel::<MonitorEvent>();
                let etw = freespace_platform::etw::EtwSession::start(move |event| {
                    let process = freespace_platform::process_io::counters_for(event.process_id)
                        .map(|counters| counters.name)
                        .unwrap_or_default();
                    let _ = etw_sender.send(MonitorEvent {
                        timestamp: Timestamp::from_millis(event.timestamp_ms),
                        process_id: event.process_id,
                        process,
                        bytes: event.bytes,
                        paths: vec![event.path],
                        threshold_exceeded: false,
                    });
                })
                .ok();
                let mut fallback = if etw.is_some() {
                    None
                } else {
                    let mut monitor = freespace_scan::monitor_fallback::FallbackMonitor::new();
                    let _ = monitor.poll();
                    Some(monitor)
                };

                while !worker_stop.load(Ordering::Relaxed) {
                    let deadline = std::time::Instant::now() + interval;
                    while !worker_stop.load(Ordering::Relaxed)
                        && std::time::Instant::now() < deadline
                    {
                        thread::sleep(Duration::from_millis(100));
                    }
                    if worker_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    if etw.is_some() {
                        while let Ok(event) = etw_receiver.try_recv() {
                            if sender.send(event).is_err() {
                                return;
                            }
                        }
                    } else if let Some(monitor) = &mut fallback {
                        match monitor.poll() {
                            Ok(writes) => {
                                for write in writes {
                                    let event = MonitorEvent {
                                        timestamp: Timestamp::now(),
                                        process_id: write.pid,
                                        process: write.process,
                                        bytes: write.bytes,
                                        paths: Vec::new(),
                                        threshold_exceeded: false,
                                    };
                                    if sender.send(event).is_err() {
                                        return;
                                    }
                                }
                            }
                            Err(error) => {
                                let _ = sender.send(MonitorEvent {
                                    timestamp: Timestamp::now(),
                                    process_id: 0,
                                    process: format!("error: {error}"),
                                    bytes: 0,
                                    paths: Vec::new(),
                                    threshold_exceeded: false,
                                });
                            }
                        }
                    }
                }
                drop(etw);
            })
            .ok();

        self.receiver = Some(receiver);
        self.stop = Some(stop);
        self.worker = worker;
        self.config = Some(config.clone());
        self.history = Some(MonitorHistory::new(DEFAULT_HISTORY_CAPACITY, &config));
        self.notification = None;
        self.error = None;
        self.source = if self.worker.is_some() {
            t!("monitor.running").to_string()
        } else {
            "cannot start the monitor thread".to_owned()
        };
    }

    fn stop(&mut self) {
        if let Some(stop) = &self.stop {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.stop = None;
        self.receiver = None;
        self.source = String::new();
    }

    fn drain(&mut self, units: Units) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        let (Some(config), Some(history)) = (&self.config, &mut self.history) else {
            return;
        };
        for event in receiver.try_iter() {
            if event.bytes == 0 {
                continue;
            }
            if let Some(notification) = history.push(event, config) {
                self.notification = Some(
                    t!(
                        "monitor.notification",
                        size = format_bytes(notification.bytes, units),
                        window = notification.window_ms
                    )
                    .to_string(),
                );
            }
        }
    }
}

impl Drop for MonitorState {
    fn drop(&mut self) {
        self.stop();
    }
}
