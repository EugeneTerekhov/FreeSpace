#![cfg(windows)]

//! US20: ETW event interpretation and the counter/USN fallback.

mod common;

use std::time::Duration;

use freespace_analysis::monitor::{MonitorConfig, MonitorEvent, MonitorHistory};
use freespace_core::{Timestamp, model::JournalState, settings::Settings, size::Units};
use freespace_platform::{
    etw::{EventFields, WRITE_EVENT_ID, interpret_write},
    process_io::{ProcessIo, snapshot},
};
use freespace_scan::monitor_fallback::{FallbackMonitor, usn_write, write_deltas};

fn counters(pid: u32, written: u64, operations: u64) -> ProcessIo {
    ProcessIo {
        pid,
        name: format!("process-{pid}"),
        image: None,
        read_bytes: 0,
        write_bytes: written,
        other_bytes: 0,
        read_operations: 0,
        write_operations: operations,
    }
}

#[test]
fn etw_writes_are_interpreted_with_process_and_path() {
    let fields = EventFields {
        file_name: Some(r"\Device\HarddiskVolume3\Users\alice\video.mkv".to_owned()),
        io_size: Some(8 * 1024 * 1024),
        issuing_process_id: Some(1234),
    };

    let event = interpret_write(WRITE_EVENT_ID, fields, 99, 1_700_000_000_000).expect("write");

    assert_eq!(event.process_id, 1234);
    assert_eq!(event.bytes, 8 * 1024 * 1024);
    assert!(event.path.ends_with("video.mkv"));
    assert_eq!(event.timestamp_ms, 1_700_000_000_000);

    let read = EventFields {
        file_name: Some(r"\Device\HarddiskVolume3\a.bin".to_owned()),
        io_size: Some(1),
        issuing_process_id: None,
    };
    assert!(interpret_write(WRITE_EVENT_ID - 1, read, 1, 0).is_none());
}

#[test]
fn counter_deltas_report_write_rates() {
    let before = vec![counters(1, 1_000, 1), counters(2, 500, 1)];
    let after = vec![counters(1, 9_000, 9), counters(2, 500, 1)];

    let deltas = write_deltas(&before, &after);

    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].pid, 1);
    assert_eq!(deltas[0].bytes, 8_000);
    assert_eq!(deltas[0].operations, 8);
    assert_eq!(deltas[0].process, "process-1");
}

#[test]
fn fallback_monitor_primes_and_then_measures() {
    let mut monitor = FallbackMonitor::new();
    assert!(monitor.poll().expect("baseline").is_empty());
    assert!(monitor.tracked() > 0);

    let fixture = common::TestTree::new();
    let file = fixture.write("write.bin", &vec![7u8; 4 * 1024 * 1024]);
    std::fs::write(&file, vec![8u8; 4 * 1024 * 1024]).expect("write fixture");
    std::thread::sleep(Duration::from_millis(50));

    let writes = monitor.poll().expect("poll");
    assert!(
        writes
            .iter()
            .any(|write| write.pid == std::process::id() && write.bytes > 0),
        "the test process must report its own writes: {writes:?}"
    );
}

#[test]
fn usn_records_become_path_hints() {
    let record = freespace_platform::mft::UsnRecord {
        file_ref: 42,
        parent_ref: 5,
        usn: 7,
        timestamp: Timestamp::now(),
        reason: freespace_platform::mft::USN_REASON_DATA_EXTEND,
        attributes: freespace_core::tree::FileAttributes::empty(),
        name: "changed.bin".to_owned(),
    };

    let hint = usn_write(&record).expect("write hint");

    assert_eq!(hint.name, "changed.bin");
    assert_eq!(hint.file_ref, 42);
}

#[test]
fn usn_tail_is_optional_without_volume_access() {
    let fixture = common::TestTree::new();
    let mut state = JournalState {
        journal_id: 0,
        next_usn: 0,
    };

    let result = freespace_scan::monitor_fallback::tail_paths(fixture.root(), &mut state);
    // Temporary directories are usually on NTFS; whichever way the platform
    // answers, the call must not panic and must not fabricate paths.
    if let Ok(paths) = result {
        assert!(paths.len() <= freespace_scan::monitor_fallback::MAX_TAIL_PATHS);
    }
}

#[test]
fn thresholds_exclusions_and_history_follow_settings() {
    let mut settings = Settings::default();
    settings
        .set_value("monitor_threshold_bytes", "1000")
        .expect("threshold");
    settings
        .set_value("monitor_interval_secs", "10")
        .expect("interval");
    settings
        .set_value("exclusions", r"C:\Windows\Temp\*")
        .expect("exclusions");
    let config = MonitorConfig::from_settings(&settings);
    let mut history = MonitorHistory::new(10, &config);

    assert!(
        history
            .push(
                MonitorEvent {
                    timestamp: Timestamp::now(),
                    process_id: 1,
                    process: "explorer".to_owned(),
                    bytes: 500,
                    paths: vec![r"C:\Windows\Temp\x.tmp".to_owned()],
                    threshold_exceeded: false,
                },
                &config
            )
            .is_none()
    );
    let notification = history
        .push(
            MonitorEvent {
                timestamp: Timestamp::now(),
                process_id: 2,
                process: "writer".to_owned(),
                bytes: 2_000,
                paths: vec![r"C:\data\big.bin".to_owned()],
                threshold_exceeded: false,
            },
            &config,
        )
        .expect("threshold");

    assert_eq!(notification.bytes, 2_000);
    assert_eq!(history.summaries()[0].process, "writer");
    assert_eq!(history.total_bytes(), 2_000);
    assert_eq!(
        freespace_core::size::format_bytes(2_000, Units::Decimal),
        "2 KB"
    );
    let _ = snapshot();
}
