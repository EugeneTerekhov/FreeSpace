//! Real-time write attribution through the `Microsoft-Windows-Kernel-File`
//! ETW provider (US20).
//!
//! Starting a kernel ETW session needs administrator rights. When it is not
//! available the caller falls back to process I/O counters and the USN journal;
//! this module therefore reports an [`freespace_core::Error::AccessDenied`]
//! instead of a generic failure, which makes the degradation explicit.
//!
//! Event payloads are interpreted through [`EventFields`] so the mapping from an
//! ETW record to a [`WriteEvent`] can be tested without a live session.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use ferrisetw::{
    EventRecord, parser::Parser, provider::Provider, schema_locator::SchemaLocator,
    trace::UserTrace,
};
use freespace_core::{Error, Result};

/// GUID of `Microsoft-Windows-Kernel-File`.
pub const KERNEL_FILE_PROVIDER: &str = "edd08927-9cc4-4e65-b970-c2560fb5c289";
/// Event identifier of the manifest `Write` event.
pub const WRITE_EVENT_ID: u16 = 16;
/// Time to wait for the consumer thread to stop.
const STOP_TIMEOUT: Duration = Duration::from_secs(2);

/// One write observed by the ETW consumer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteEvent {
    /// Process that issued the write.
    pub process_id: u32,
    /// Kernel path reported by the provider (`\Device\HarddiskVolumeN\...`).
    pub path: String,
    /// Number of bytes written.
    pub bytes: u64,
    /// Event time in Unix milliseconds.
    pub timestamp_ms: i64,
}

/// Raw fields of a provider event, already decoded by ferrisetw.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EventFields {
    /// `FileName` or `FilePath` field.
    pub file_name: Option<String>,
    /// `IoSize` field.
    pub io_size: Option<u64>,
    /// `IssuingProcessId` field when the provider reports it separately.
    pub issuing_process_id: Option<u32>,
}

impl EventFields {
    /// Returns a copy with the payload whitespace trimmed and empty names
    /// treated as absent.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.file_name = self
            .file_name
            .map(|name| name.trim_matches('\0').trim().to_owned())
            .filter(|name| !name.is_empty());

        self
    }
}

/// Converts one decoded ETW event into a write event.
///
/// Returns `None` for other event identifiers, missing names and zero-length
/// writes, which keeps read/flush noise out of the monitor history.
#[must_use]
pub fn interpret_write(
    event_id: u16,
    fields: EventFields,
    record_process_id: u32,
    timestamp_ms: i64,
) -> Option<WriteEvent> {
    if event_id != WRITE_EVENT_ID {
        return None;
    }

    let fields = fields.normalized();
    let path = fields.file_name?;
    let bytes = fields.io_size.filter(|bytes| *bytes > 0)?;
    let process_id = fields.issuing_process_id.unwrap_or(record_process_id);

    Some(WriteEvent {
        process_id,
        path,
        bytes,
        timestamp_ms,
    })
}

/// A running real-time ETW session.
#[derive(Debug)]
pub struct EtwSession {
    trace: Option<UserTrace>,
}

impl EtwSession {
    /// Starts a session and invokes `handler` for every write event.
    ///
    /// # Errors
    /// Returns [`Error::AccessDenied`] when the kernel provider cannot be enabled
    /// (the usual outcome without elevation) and [`Error::Operation`] for other
    /// ETW failures.
    pub fn start<F>(handler: F) -> Result<Self>
    where
        F: FnMut(WriteEvent) + Send + 'static,
    {
        let handler = Arc::new(Mutex::new(handler));
        let callback_handler = Arc::clone(&handler);
        let provider = Provider::by_guid(KERNEL_FILE_PROVIDER)
            .add_callback(move |record: &EventRecord, locator: &SchemaLocator| {
                let fields = extract_fields(record, locator);
                let timestamp = event_timestamp_millis(record);
                if let Some(event) =
                    interpret_write(record.event_id(), fields, record.process_id(), timestamp)
                {
                    if let Ok(mut handler) = callback_handler.lock() {
                        handler(event);
                    }
                }
            })
            .build();

        let trace = UserTrace::new()
            .enable(provider)
            .start_and_process()
            .map_err(|error| {
                Error::AccessDenied(format!(
                    "cannot start the Kernel-File ETW session: {error:?}"
                ))
            })?;
        // Give the provider a moment to attach so `start` is not reported as
        // successful while the consumer thread silently fails.
        std::thread::sleep(Duration::from_millis(50));

        Ok(Self { trace: Some(trace) })
    }

    /// Stops the session and waits briefly for the consumer thread.
    ///
    /// # Errors
    /// Returns [`Error::Operation`] when ETW refuses to stop the session.
    pub fn stop(mut self) -> Result<()> {
        self.stop_inner()
    }

    fn stop_inner(&mut self) -> Result<()> {
        let Some(trace) = self.trace.take() else {
            return Ok(());
        };
        trace.stop().map_err(|error| {
            Error::Operation(format!(
                "cannot stop the Kernel-File ETW session: {error:?}"
            ))
        })?;
        std::thread::sleep(STOP_TIMEOUT.min(Duration::from_millis(50)));

        Ok(())
    }
}

impl Drop for EtwSession {
    fn drop(&mut self) {
        let _ = self.stop_inner();
    }
}

fn extract_fields(record: &EventRecord, locator: &SchemaLocator) -> EventFields {
    let Ok(schema) = locator.event_schema(record) else {
        return EventFields::default();
    };
    let parser = Parser::create(record, &schema);

    EventFields {
        file_name: parser
            .try_parse::<String>("FileName")
            .or_else(|_| parser.try_parse::<String>("FilePath"))
            .ok(),
        io_size: parser
            .try_parse::<u64>("IoSize")
            .or_else(|_| parser.try_parse::<u32>("IoSize").map(u64::from))
            .ok(),
        issuing_process_id: parser.try_parse::<u32>("IssuingProcessId").ok(),
    }
}

fn event_timestamp_millis(record: &EventRecord) -> i64 {
    let raw = record.raw_timestamp();
    if raw <= 0 {
        return freespace_core::time::Timestamp::now().as_millis();
    }

    super::win::filetime_to_millis(u64::try_from(raw).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::{EventFields, WRITE_EVENT_ID, interpret_write};

    fn fields() -> EventFields {
        EventFields {
            file_name: Some(r"\Device\HarddiskVolume3\Users\alice\data.bin".to_owned()),
            io_size: Some(4096),
            issuing_process_id: None,
        }
    }

    #[test]
    fn interprets_a_write_event() {
        let event = interpret_write(WRITE_EVENT_ID, fields(), 4242, 1_700_000_000_000)
            .expect("write event");

        assert_eq!(event.process_id, 4242);
        assert_eq!(event.bytes, 4096);
        assert!(event.path.ends_with(r"data.bin"));
        assert_eq!(event.timestamp_ms, 1_700_000_000_000);
    }

    #[test]
    fn prefers_the_issuing_process_id() {
        let mut fields = fields();
        fields.issuing_process_id = Some(7);

        let event = interpret_write(WRITE_EVENT_ID, fields, 4242, 0).expect("event");

        assert_eq!(event.process_id, 7);
    }

    #[test]
    fn ignores_other_events_and_noise() {
        assert!(interpret_write(WRITE_EVENT_ID - 1, fields(), 1, 0).is_none());

        let mut empty_name = fields();
        empty_name.file_name = Some("  \0".to_owned());
        assert!(interpret_write(WRITE_EVENT_ID, empty_name, 1, 0).is_none());

        let mut zero = fields();
        zero.io_size = Some(0);
        assert!(interpret_write(WRITE_EVENT_ID, zero, 1, 0).is_none());

        assert!(interpret_write(WRITE_EVENT_ID, EventFields::default(), 1, 0).is_none());
    }

    #[test]
    fn trims_trailing_nul_bytes_in_paths() {
        let mut fields = fields();
        fields.file_name = Some("\\Device\\HarddiskVolume3\\a.txt\0".to_owned());

        let event = interpret_write(WRITE_EVENT_ID, fields, 1, 0).expect("event");

        assert_eq!(event.path, r"\Device\HarddiskVolume3\a.txt");
    }
}
