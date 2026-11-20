//! `freespace monitor` — real-time disk-write monitor (US20).
//!
//! Runs the Kernel-File ETW session when the process can start one and falls
//! back to `GetProcessIoCounters` plus the USN journal tail otherwise. Output is
//! a stream of JSON Lines (one object per event), which keeps pipes and log
//! collectors happy even when the monitor runs for hours.

#[cfg(windows)]
use std::{collections::HashMap, sync::mpsc};
use std::{path::Path, time::Duration};

use freespace_analysis::monitor::{
    DEFAULT_HISTORY_CAPACITY, MonitorConfig, MonitorEvent, MonitorHistory,
};
use freespace_core::{Error, Timestamp, settings::Settings, size::format_bytes};
#[cfg(windows)]
use freespace_platform::{
    etw::{EtwSession, WriteEvent},
    process_io,
};
use freespace_scan::monitor_fallback::FallbackMonitor;
#[cfg(windows)]
use freespace_scan::{monitor_fallback::tail_paths, usn};
use serde_json::json;

use super::Outcome;

/// Arguments of the `monitor` command.
#[derive(Debug, Default, Clone)]
pub struct MonitorArgs<'a> {
    /// Overrides the configured notification threshold.
    pub threshold: Option<u64>,
    /// Overrides the configured polling interval.
    pub interval: Option<u64>,
    /// Number of batches; `0` runs until the process is interrupted.
    pub iterations: u32,
    /// Volume or directory used for USN path hints.
    pub root: Option<&'a Path>,
    /// Forces the counter fallback.
    pub no_etw: bool,
    /// Emit JSON Lines instead of the human table.
    pub json: bool,
}

/// Runs the monitor until the requested number of batches is complete.
///
/// # Errors
/// Returns an error when neither ETW nor the counter fallback can be started.
pub fn run(settings: &Settings, args: MonitorArgs<'_>) -> Result<Outcome, Error> {
    let mut config = MonitorConfig::from_settings(settings);
    if let Some(threshold) = args.threshold {
        config.threshold_bytes = threshold;
    }
    if let Some(interval) = args.interval {
        config.interval = if interval == 0 {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(interval)
        };
    }

    let mut history = MonitorHistory::new(DEFAULT_HISTORY_CAPACITY, &config);
    let mut warnings = Vec::new();
    #[cfg(windows)]
    let mut names: HashMap<u32, String> = HashMap::new();
    let mut lines = String::new();
    let mut human = String::new();

    #[cfg(windows)]
    let (sender, receiver) = mpsc::channel::<WriteEvent>();
    #[cfg(windows)]
    let etw = if args.no_etw {
        None
    } else {
        match EtwSession::start(move |event| {
            let _ = sender.send(event);
        }) {
            Ok(session) => Some(session),
            Err(error) => {
                warnings.push(format!("ETW is unavailable, using counters: {error}"));
                None
            }
        }
    };
    #[cfg(windows)]
    let started_etw = etw.is_some();
    #[cfg(not(windows))]
    let started_etw = {
        if !args.no_etw {
            warnings.push("ETW is available on Windows only; using process counters".to_owned());
        }
        false
    };
    let mut fallback = if started_etw {
        None
    } else {
        let mut monitor = FallbackMonitor::new();
        monitor.poll()?;
        Some(monitor)
    };

    #[cfg(windows)]
    let mut journal = match args.root {
        Some(root) => match usn::capture_state(root) {
            Ok(state) => Some(state),
            Err(error) => {
                warnings.push(format!("USN journal is unavailable: {error}"));
                None
            }
        },
        None => None,
    };
    #[cfg(not(windows))]
    let _ = args.root;

    let interval = config.interval.max(Duration::from_millis(100));
    let iterations = args.iterations;
    let mut batch = 0u32;
    loop {
        if iterations != 0 && batch >= iterations {
            break;
        }
        batch += 1;
        std::thread::sleep(interval);

        let mut events = Vec::new();
        if started_etw {
            #[cfg(windows)]
            while let Ok(event) = receiver.try_recv() {
                let process = names
                    .entry(event.process_id)
                    .or_insert_with(|| {
                        process_io::counters_for(event.process_id)
                            .map(|counters| counters.name)
                            .unwrap_or_default()
                    })
                    .clone();
                events.push(MonitorEvent {
                    timestamp: Timestamp::from_millis(event.timestamp_ms),
                    process_id: event.process_id,
                    process,
                    bytes: event.bytes,
                    paths: vec![event.path.clone()],
                    threshold_exceeded: false,
                });
            }
        } else if let Some(monitor) = &mut fallback {
            for write in monitor.poll()? {
                events.push(MonitorEvent {
                    timestamp: Timestamp::now(),
                    process_id: write.pid,
                    process: write.process,
                    bytes: write.bytes,
                    paths: Vec::new(),
                    threshold_exceeded: false,
                });
            }
        }

        #[cfg(windows)]
        let mut usn_hints = Vec::new();
        #[cfg(windows)]
        if let (Some(root), Some(state)) = (args.root, journal.as_mut()) {
            match tail_paths(root, state) {
                Ok(paths) => usn_hints = paths,
                Err(error) => warnings.push(format!("USN tail failed: {error}")),
            }
        }

        for event in events {
            let timestamp = event.timestamp;
            let pid = event.process_id;
            let process = event.process.clone();
            let bytes = event.bytes;
            let paths = event.paths.clone();
            let notification = history.push(event, &config);
            if args.json {
                lines.push_str(
                    &serde_json::to_string(&json!({
                        "type": "write",
                        "timestamp_ms": timestamp.as_millis(),
                        "pid": pid,
                        "process": process,
                        "bytes": bytes,
                        "paths": paths,
                        "threshold_exceeded": notification.is_some(),
                    }))
                    .map_err(|error| Error::Other(error.to_string()))?,
                );
                lines.push('\n');
            } else {
                human.push_str(&format!(
                    "{:>13} {:>7}  {:<24} {:>10}  {}\n",
                    timestamp.as_millis(),
                    pid,
                    truncate(&process, 24),
                    format_bytes(bytes, settings.units),
                    paths.join("; ")
                ));
            }
            if let Some(notification) = notification {
                if args.json {
                    lines.push_str(
                        &serde_json::to_string(&json!({
                            "type": "notification",
                            "timestamp_ms": timestamp.as_millis(),
                            "bytes": notification.bytes,
                            "window_ms": notification.window_ms,
                            "threshold_bytes": notification.threshold_bytes,
                        }))
                        .map_err(|error| Error::Other(error.to_string()))?,
                    );
                    lines.push('\n');
                } else {
                    human.push_str(&format!(
                        "notification: {} written within {} ms (threshold {})\n",
                        format_bytes(notification.bytes, settings.units),
                        notification.window_ms,
                        format_bytes(notification.threshold_bytes, settings.units)
                    ));
                }
            }
        }

        #[cfg(windows)]
        {
            if args.json {
                for hint in &usn_hints {
                    lines.push_str(
                        &serde_json::to_string(&json!({
                            "type": "usn",
                            "timestamp_ms": hint.timestamp.as_millis(),
                            "file_ref": hint.file_ref,
                            "parent_ref": hint.parent_ref,
                            "name": hint.name,
                        }))
                        .map_err(|error| Error::Other(error.to_string()))?,
                    );
                    lines.push('\n');
                }
            } else {
                for hint in &usn_hints {
                    human.push_str(&format!(
                        "{:>13} {:>7}  {:<24} {:>10}  {}\n",
                        hint.timestamp.as_millis(),
                        "-",
                        "<usn>",
                        "-",
                        hint.name
                    ));
                }
            }
        }
    }

    #[cfg(windows)]
    if let Some(session) = etw {
        session.stop()?;
    }

    let summaries = history
        .summaries()
        .into_iter()
        .map(|summary| {
            json!({
                "pid": summary.process_id,
                "process": summary.process,
                "bytes": summary.bytes,
                "events": summary.events,
                "last_path": summary.last_path,
            })
        })
        .collect::<Vec<_>>();
    let data = json!({
        "source": if fallback.is_some() { "counters" } else { "etw" },
        "batches": batch,
        "events": history.events().len(),
        "bytes": history.total_bytes(),
        "processes": summaries,
    });

    if !args.json {
        human.push_str(&format!(
            "Summary: {} events, {}\n",
            history.events().len(),
            format_bytes(history.total_bytes(), settings.units)
        ));
    }

    let mut outcome = Outcome::new(data, human).with_warnings(warnings);
    if args.json {
        outcome = outcome.with_raw(lines);
    }

    Ok(outcome)
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    let mut truncated: String = value.chars().take(limit.saturating_sub(1)).collect();
    truncated.push('…');

    truncated
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncates_at_character_boundaries() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("файлы", 3), "фа…");
    }
}
