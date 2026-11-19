//! NTFS MFT and USN journal access.
//!
//! Two mechanisms are exposed:
//!
//! * [`enumerate_mft`] reads the `$MFT` system file sequentially and parses
//!   `FILE` records. This is the only way to obtain names *and* sizes in one
//!   pass, which is what the 8 TB / 60 s budget needs (FR-003).
//! * [`enumerate_usn`], [`query_journal`] and [`read_journal`] use
//!   `FSCTL_ENUM_USN_DATA`, `FSCTL_QUERY_USN_JOURNAL` and
//!   `FSCTL_READ_USN_JOURNAL`; they power incremental updates (FR-009).
//!
//! Both mechanisms need a volume handle and therefore administrator rights.
//! Non-elevated runs use the directory walk in `freespace-scan`.

use std::{ffi::c_void, path::Path};

use freespace_core::{
    Error, Result, Timestamp,
    model::JournalState,
    tree::{FileAttributes, Timestamps},
};
use windows::{
    Win32::{
        Storage::FileSystem::{
            CreateFileW, FILE_GENERIC_READ, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            OPEN_EXISTING, ReadFile,
        },
        System::IO::DeviceIoControl,
        System::Ioctl::{
            FSCTL_ENUM_USN_DATA, FSCTL_GET_NTFS_VOLUME_DATA, FSCTL_QUERY_USN_JOURNAL,
            FSCTL_READ_USN_JOURNAL, MFT_ENUM_DATA_V0, NTFS_VOLUME_DATA_BUFFER,
            READ_USN_JOURNAL_DATA_V0, USN_JOURNAL_DATA_V2,
        },
    },
    core::PCWSTR,
};

use super::win::{self, Handle};

/// Milliseconds between the Windows epoch (1601-01-01) and the Unix epoch.
const WINDOWS_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;

/// Size of the buffer used for MFT and USN reads.
const IO_BUFFER_BYTES: usize = 1 << 20;

/// Maximum number of records accepted from a single device call.
const MAX_USN_BATCH: usize = 1 << 18;

/// One `$FILE_NAME` attribute of an MFT record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MftName {
    /// File reference number of the parent directory (sequence removed).
    pub parent_ref: u64,
    /// Long name (Win32 namespace).
    pub name: String,
}

/// Parsed `FILE` record of the NTFS master file table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MftRecord {
    /// Record number, used as the stable file identity.
    pub file_ref: u64,
    /// Every long name of the record; hard links produce several.
    pub names: Vec<MftName>,
    /// Windows file attributes.
    pub attributes: FileAttributes,
    /// Unnamed `$DATA` logical size.
    pub size: u64,
    /// Unnamed `$DATA` allocated size.
    pub allocated: u64,
    /// Standard-information timestamps, when present.
    pub timestamps: Timestamps,
    /// Whether the record describes a directory.
    pub is_directory: bool,
    /// Number of hard links reported by the record header.
    pub hard_link_count: u16,
}

impl MftRecord {
    /// Returns `true` when the record describes a reparse point.
    #[must_use]
    pub fn is_reparse_point(&self) -> bool {
        self.attributes.contains(FileAttributes::REPARSE_POINT)
    }
}

/// One `USN_RECORD_V2` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsnRecord {
    /// File reference number of the changed object.
    pub file_ref: u64,
    /// Parent directory reference number.
    pub parent_ref: u64,
    /// Journal offset of the record.
    pub usn: i64,
    /// When the change was recorded.
    pub timestamp: Timestamp,
    /// Bit mask of `USN_REASON_*` flags.
    pub reason: u32,
    /// File attributes.
    pub attributes: FileAttributes,
    /// File name.
    pub name: String,
}

/// The change created the object.
pub const USN_REASON_FILE_CREATE: u32 = 0x0000_0100;
/// The change removed the object.
pub const USN_REASON_FILE_DELETE: u32 = 0x0000_0200;
/// The object was renamed; this record carries the old name.
pub const USN_REASON_RENAME_OLD_NAME: u32 = 0x0000_1000;
/// The object was renamed; this record carries the new name.
pub const USN_REASON_RENAME_NEW_NAME: u32 = 0x0000_2000;
/// File data was overwritten.
pub const USN_REASON_DATA_OVERWRITE: u32 = 0x0000_0001;
/// File data was appended.
pub const USN_REASON_DATA_EXTEND: u32 = 0x0000_0002;
/// File data was truncated.
pub const USN_REASON_DATA_TRUNCATION: u32 = 0x0000_0004;
/// The object was closed; safe point for incremental updates.
pub const USN_REASON_CLOSE: u32 = 0x8000_0000;

/// Statistics of a full MFT pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MftStats {
    /// Records read from the table.
    pub records: u64,
    /// Records skipped because the fixup sequence did not match.
    pub corrupt: u64,
}

/// Opens `\\.\X:` for the volume that contains `volume_root`.
fn open_volume(volume_root: &Path) -> Result<Handle> {
    let root = volume_root.to_string_lossy();
    let drive = root
        .chars()
        .next()
        .filter(char::is_ascii_alphabetic)
        .ok_or_else(|| {
            Error::Unsupported(format!(
                "`{}` is not a local drive root",
                volume_root.display()
            ))
        })?;

    open_device(&format!(r"\\.\{drive}:"), 0)
}

/// Opens `\\.\X:\$MFT`.
fn open_mft(volume_root: &Path) -> Result<Handle> {
    let root = volume_root.to_string_lossy();
    let drive = root
        .chars()
        .next()
        .filter(char::is_ascii_alphabetic)
        .ok_or_else(|| {
            Error::Unsupported(format!(
                "`{}` is not a local drive root",
                volume_root.display()
            ))
        })?;

    open_device(&format!(r"\\.\{drive}:\$MFT"), 0)
}

fn open_device(device: &str, flags: u32) -> Result<Handle> {
    let wide = win::to_wide(std::ffi::OsStr::new(device));

    // SAFETY: the device string is NUL-terminated and lives for the call.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(flags),
            None,
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;

    Handle::from_raw(handle)
}

fn device_io<T, U>(
    handle: &Handle,
    control: u32,
    input: Option<&T>,
    output: &mut U,
) -> Result<u32> {
    let mut returned = 0u32;
    let input_ptr = input.map_or(std::ptr::null(), |value| {
        std::ptr::from_ref(value).cast::<c_void>()
    });
    let output_size = u32::try_from(size_of::<U>()).unwrap_or(u32::MAX);
    let input_size = input.map_or(0, |_| u32::try_from(size_of::<T>()).unwrap_or(0));

    // SAFETY: both pointers refer to live values of the declared sizes for the
    // duration of the call; the handle is owned by `handle`.
    unsafe {
        DeviceIoControl(
            handle.raw(),
            control,
            input.map(|_| input_ptr),
            input_size,
            Some(std::ptr::from_mut(output).cast::<c_void>()),
            output_size,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;

    Ok(returned)
}

/// Reads the NTFS volume control block and returns the MFT record size.
///
/// # Errors
/// Returns an error when the volume handle cannot be opened or queried.
pub fn mft_record_size(volume_root: &Path) -> Result<u32> {
    let handle = open_volume(volume_root)?;
    let mut data = NTFS_VOLUME_DATA_BUFFER::default();
    device_io(&handle, FSCTL_GET_NTFS_VOLUME_DATA, None::<&u64>, &mut data)?;

    if data.BytesPerFileRecordSegment == 0 {
        return Err(Error::Operation(
            "the volume reports a zero MFT record size".to_owned(),
        ));
    }

    Ok(data.BytesPerFileRecordSegment)
}

/// Reads every `FILE` record of the NTFS master file table.
///
/// `cancel` is polled between batches so that a scan can stop early. The sink
/// receives records in table order.
///
/// # Errors
/// Returns [`Error::AccessDenied`] (wrapped I/O error) when the volume handle
/// cannot be opened, for example without administrator rights.
pub fn enumerate_mft(
    volume_root: &Path,
    cancel: &dyn Fn() -> bool,
    mut sink: impl FnMut(MftRecord),
) -> Result<MftStats> {
    let record_size = mft_record_size(volume_root)? as usize;
    let handle = open_mft(volume_root)?;
    let mut chunk = vec![0u8; IO_BUFFER_BYTES - IO_BUFFER_BYTES % record_size];
    let mut carry: Vec<u8> = Vec::with_capacity(record_size * 2);
    let mut stats = MftStats::default();

    loop {
        if cancel() {
            break;
        }

        let mut read = 0u32;
        // SAFETY: the buffer is valid and owned for the duration of the call.
        let result = unsafe { ReadFile(handle.raw(), Some(&mut chunk), Some(&mut read), None) };
        if let Err(error) = result {
            return Err(Error::Io(win::win_error(&error)));
        }
        if read == 0 {
            break;
        }

        carry.extend_from_slice(&chunk[..read as usize]);
        let complete = carry.len() / record_size * record_size;
        for record in carry[..complete].chunks_exact(record_size) {
            let record_number = stats.records;
            match parse_mft_record(record, record_number) {
                Some(parsed) => sink(parsed),
                None => stats.corrupt = stats.corrupt.saturating_add(1),
            }
            stats.records = stats.records.saturating_add(1);
        }
        carry.drain(..complete);
    }

    Ok(stats)
}

/// Enumerates MFT entries through `FSCTL_ENUM_USN_DATA`.
///
/// The enumeration returns names, parent references, attributes and journal
/// metadata, but no file sizes; [`enumerate_mft`] is used when sizes matter.
///
/// # Errors
/// Returns an error when the volume cannot be opened or the control call fails.
pub fn enumerate_usn(volume_root: &Path, mut sink: impl FnMut(UsnRecord)) -> Result<u64> {
    let handle = open_volume(volume_root)?;
    let mut input = MFT_ENUM_DATA_V0 {
        StartFileReferenceNumber: 0,
        LowUsn: 0,
        HighUsn: i64::MAX,
    };
    let mut buffer = vec![0u8; IO_BUFFER_BYTES];
    let mut total = 0u64;

    loop {
        let mut returned = 0u32;
        // SAFETY: input and output buffers are live for the call.
        let result = unsafe {
            DeviceIoControl(
                handle.raw(),
                FSCTL_ENUM_USN_DATA,
                Some(std::ptr::from_ref(&input).cast::<c_void>()),
                u32::try_from(size_of::<MFT_ENUM_DATA_V0>()).unwrap_or(0),
                Some(buffer.as_mut_ptr().cast::<c_void>()),
                u32::try_from(buffer.len()).unwrap_or(0),
                Some(&mut returned),
                None,
            )
        };
        if let Err(error) = result {
            let io = win::win_error(&error);
            if io.raw_os_error() == Some(38) {
                // ERROR_HANDLE_EOF: the enumeration finished.
                break;
            }
            return Err(Error::Io(io));
        }
        if returned < 8 {
            break;
        }

        input.StartFileReferenceNumber =
            u64::from_le_bytes(buffer[..8].try_into().unwrap_or_default());
        let records = parse_usn_buffer(&buffer[8..returned as usize], &mut sink)?;
        total = total.saturating_add(records);
        if returned as usize >= buffer.len() {
            continue;
        }
        if records == 0 {
            break;
        }
    }

    Ok(total)
}

/// Captures the journal identifier and the next record offset.
///
/// # Errors
/// Returns an error when the volume cannot be queried.
pub fn query_journal(volume_root: &Path) -> Result<JournalState> {
    let handle = open_volume(volume_root)?;
    let mut data = USN_JOURNAL_DATA_V2::default();
    device_io(&handle, FSCTL_QUERY_USN_JOURNAL, None::<&u64>, &mut data)?;

    Ok(JournalState {
        journal_id: data.UsnJournalID,
        next_usn: data.NextUsn,
    })
}

/// Reads journal records starting at `state.next_usn`.
///
/// Returns the records and the journal state to store for the next update. A
/// changed journal identifier or a trimmed journal is reported through
/// [`JournalRead::requires_rescan`].
///
/// # Errors
/// Returns an error when the volume cannot be queried.
pub fn read_journal(volume_root: &Path, state: JournalState) -> Result<JournalRead> {
    let handle = open_volume(volume_root)?;
    let mut buffer = vec![0u8; IO_BUFFER_BYTES];
    let mut records = Vec::new();
    let mut next_state = state;

    loop {
        let input = READ_USN_JOURNAL_DATA_V0 {
            StartUsn: next_state.next_usn,
            ReasonMask: u32::MAX,
            ReturnOnlyOnClose: 0,
            Timeout: 0,
            BytesToWaitFor: 0,
            UsnJournalID: next_state.journal_id,
        };
        let mut returned = 0u32;
        // SAFETY: input and output buffers are live for the call.
        let result = unsafe {
            DeviceIoControl(
                handle.raw(),
                FSCTL_READ_USN_JOURNAL,
                Some(std::ptr::from_ref(&input).cast::<c_void>()),
                u32::try_from(size_of::<READ_USN_JOURNAL_DATA_V0>()).unwrap_or(0),
                Some(buffer.as_mut_ptr().cast::<c_void>()),
                u32::try_from(buffer.len()).unwrap_or(0),
                Some(&mut returned),
                None,
            )
        };
        if let Err(error) = result {
            let io = win::win_error(&error);
            match io.raw_os_error() {
                // ERROR_HANDLE_EOF: the journal tail was reached.
                Some(38) => break,
                // The journal was deleted, reset or trimmed; a full rescan is
                // the only safe recovery.
                Some(1178 | 1179 | 1181) => {
                    return Ok(JournalRead {
                        records,
                        state,
                        requires_rescan: true,
                    });
                }
                _ => return Err(Error::Io(io)),
            }
        }
        if returned < 8 {
            break;
        }

        let next_usn = i64::from_le_bytes(buffer[..8].try_into().unwrap_or_default());
        let bytes = &buffer[8..returned as usize];
        let before = records.len();
        parse_usn_buffer(bytes, &mut |record| records.push(record))?;
        next_state.next_usn = next_usn;

        if records.len() == before || returned as usize >= buffer.len() {
            break;
        }
    }

    Ok(JournalRead {
        records,
        state: next_state,
        requires_rescan: false,
    })
}

/// Outcome of [`read_journal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRead {
    /// Records to apply to the tree.
    pub records: Vec<UsnRecord>,
    /// Journal position after the batch.
    pub state: JournalState,
    /// Whether a full rescan is required instead of the delta.
    pub requires_rescan: bool,
}

/// Parses a buffer of `USN_RECORD_V2` entries.
///
/// # Errors
/// Returns an error when a record length is nonsensical.
pub fn parse_usn_buffer(buffer: &[u8], sink: &mut impl FnMut(UsnRecord)) -> Result<u64> {
    let mut offset = 0usize;
    let mut count = 0u64;

    while offset + 8 <= buffer.len() {
        let record_length = u32::from_le_bytes(
            buffer[offset..offset + 4]
                .try_into()
                .map_err(|_| Error::Operation("truncated USN record".to_owned()))?,
        ) as usize;
        if record_length < 60 || offset + record_length > buffer.len() {
            if record_length == 0 {
                break;
            }
            return Err(Error::Operation(format!(
                "USN record length {record_length} does not fit the buffer"
            )));
        }
        if let Some(record) = parse_usn_record(&buffer[offset..offset + record_length]) {
            sink(record);
            count += 1;
        }
        offset += record_length;
        if count as usize >= MAX_USN_BATCH {
            break;
        }
    }

    Ok(count)
}

fn parse_usn_record(record: &[u8]) -> Option<UsnRecord> {
    if record.len() < 60 {
        return None;
    }

    let major = u16::from_le_bytes([record[4], record[5]]);
    if major != 2 {
        // V3/V4 records use 128-bit references; they are not produced on
        // NTFS volumes that FreeSpace supports in v1.
        return None;
    }

    let file_ref = u64::from_le_bytes(record[8..16].try_into().ok()?);
    let parent_ref = u64::from_le_bytes(record[16..24].try_into().ok()?) & 0x0000_FFFF_FFFF_FFFF;
    let usn = i64::from_le_bytes(record[24..32].try_into().ok()?);
    let timestamp = Timestamp::from_millis(filetime_to_millis(i64::from_le_bytes(
        record[32..40].try_into().ok()?,
    )));
    let reason = u32::from_le_bytes(record[40..44].try_into().ok()?);
    let attributes = FileAttributes::from_windows_attributes(u32::from_le_bytes(
        record[52..56].try_into().ok()?,
    ));
    let name_length = u16::from_le_bytes(record[56..58].try_into().ok()?) as usize;
    let name_offset = u16::from_le_bytes(record[58..60].try_into().ok()?) as usize;

    if name_offset + name_length * 2 > record.len() {
        return None;
    }

    let name = utf16_name(&record[name_offset..name_offset + name_length * 2]);
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }

    Some(UsnRecord {
        file_ref: file_ref & 0x0000_FFFF_FFFF_FFFF,
        parent_ref,
        usn,
        timestamp,
        reason,
        attributes,
        name,
    })
}

/// Parses one `FILE` record of the MFT.
///
/// Returns `None` for unused, corrupt or metadata records without a usable
/// long name.
#[must_use]
pub fn parse_mft_record(record: &[u8], record_number: u64) -> Option<MftRecord> {
    if record.len() < 48 || &record[..4] != b"FILE" {
        return None;
    }

    let flags = u16::from_le_bytes(record[22..24].try_into().ok()?);
    if flags & 0x0001 == 0 {
        return None;
    }

    let usa_offset = usize::from(u16::from_le_bytes(record[4..6].try_into().ok()?));
    let usa_count = usize::from(u16::from_le_bytes(record[6..8].try_into().ok()?));
    if usa_count < 2 || usa_offset + usa_count * 2 > record.len() {
        return None;
    }
    let sector_size = record.len() / (usa_count - 1);
    let update_sequence = u16::from_le_bytes(record[usa_offset..usa_offset + 2].try_into().ok()?);

    let mut fixed = record.to_vec();
    for index in 1..usa_count {
        let check = index * sector_size - 2;
        if check + 2 > fixed.len() {
            return None;
        }
        let stored = u16::from_le_bytes(fixed[check..check + 2].try_into().ok()?);
        if stored != update_sequence {
            return None;
        }
        let replacement = u16::from_le_bytes(
            record[usa_offset + index * 2..usa_offset + index * 2 + 2]
                .try_into()
                .ok()?,
        );
        fixed[check..check + 2].copy_from_slice(&replacement.to_le_bytes());
    }

    let first_attribute = usize::from(u16::from_le_bytes(fixed[20..22].try_into().ok()?));
    let hard_link_count = u16::from_le_bytes(fixed[18..20].try_into().ok()?);
    let mut names = Vec::new();
    let mut attributes = FileAttributes::empty();
    let mut timestamps = Timestamps::default();
    let mut size = 0u64;
    let mut allocated = 0u64;
    let mut offset = first_attribute;

    while offset + 16 <= fixed.len() {
        let attribute_type = u32::from_le_bytes(fixed[offset..offset + 4].try_into().ok()?);
        if attribute_type == 0xFFFF_FFFF {
            break;
        }
        let length = u32::from_le_bytes(fixed[offset + 4..offset + 8].try_into().ok()?) as usize;
        if length < 16 || offset + length > fixed.len() {
            break;
        }

        let non_resident = fixed[offset + 8] != 0;
        let name_length = fixed[offset + 9] as usize;
        let value = match attribute_type {
            0x10 => parse_standard_information(&fixed[offset..offset + length]),
            0x30 => {
                if let Some(name) = parse_file_name(&fixed[offset..offset + length]) {
                    if !names
                        .iter()
                        .any(|existing: &MftName| existing.name == name.name)
                    {
                        names.push(name);
                    }
                }
                None
            }
            0x80 if name_length == 0 => {
                if non_resident {
                    if length >= 56 {
                        allocated =
                            u64::from_le_bytes(fixed[offset + 40..offset + 48].try_into().ok()?);
                        size = u64::from_le_bytes(fixed[offset + 48..offset + 56].try_into().ok()?);
                    }
                } else if length >= 20 {
                    size = u64::from(u32::from_le_bytes(
                        fixed[offset + 16..offset + 20].try_into().ok()?,
                    ));
                    allocated = size;
                }
                None
            }
            _ => None,
        };

        if let Some((parsed_timestamps, parsed_attributes)) = value {
            timestamps = parsed_timestamps;
            attributes = parsed_attributes;
        }

        offset += length;
    }

    if names.is_empty() {
        return None;
    }
    if attributes.is_empty() {
        attributes = FileAttributes::from_windows_attributes(if flags & 0x0002 != 0 {
            FileAttributes::DIRECTORY.bits()
        } else {
            0
        });
    }

    Some(MftRecord {
        file_ref: record_number & 0x0000_FFFF_FFFF_FFFF,
        names,
        attributes,
        size,
        allocated,
        timestamps,
        is_directory: flags & 0x0002 != 0 || attributes.contains(FileAttributes::DIRECTORY),
        hard_link_count,
    })
}

fn parse_standard_information(attribute: &[u8]) -> Option<(Timestamps, FileAttributes)> {
    let value_offset = usize::from(u16::from_le_bytes(attribute[20..22].try_into().ok()?));
    let value_length = u32::from_le_bytes(attribute[16..20].try_into().ok()?) as usize;
    if value_offset + value_length > attribute.len() || value_length < 36 {
        return None;
    }
    let value = &attribute[value_offset..value_offset + value_length];

    let timestamps = Timestamps {
        created: Timestamp::from_millis(filetime_to_millis(i64::from_le_bytes(
            value[0..8].try_into().ok()?,
        ))),
        modified: Timestamp::from_millis(filetime_to_millis(i64::from_le_bytes(
            value[8..16].try_into().ok()?,
        ))),
        accessed: Timestamp::from_millis(filetime_to_millis(i64::from_le_bytes(
            value[24..32].try_into().ok()?,
        ))),
    };
    let attributes =
        FileAttributes::from_windows_attributes(u32::from_le_bytes(value[32..36].try_into().ok()?));

    Some((timestamps, attributes))
}

fn parse_file_name(attribute: &[u8]) -> Option<MftName> {
    let value_offset = usize::from(u16::from_le_bytes(attribute[20..22].try_into().ok()?));
    let value_length = u32::from_le_bytes(attribute[16..20].try_into().ok()?) as usize;
    if value_offset + value_length > attribute.len() || value_length < 66 {
        return None;
    }
    let value = &attribute[value_offset..value_offset + value_length];
    let namespace = value[65];
    if namespace == 2 {
        // DOS-only 8.3 names must not produce duplicate nodes.
        return None;
    }

    let name_length = usize::from(value[64]);
    if 66 + name_length * 2 > value.len() {
        return None;
    }
    let name = utf16_name(&value[66..66 + name_length * 2]);
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }

    let parent_ref = u64::from_le_bytes(value[0..8].try_into().ok()?) & 0x0000_FFFF_FFFF_FFFF;

    Some(MftName { parent_ref, name })
}

fn utf16_name(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();

    String::from_utf16_lossy(&units)
}

fn filetime_to_millis(filetime: i64) -> i64 {
    filetime
        .saturating_div(10_000)
        .saturating_sub(WINDOWS_EPOCH_OFFSET_MS)
}

#[cfg(test)]
mod tests {
    use freespace_core::tree::{FileAttributes, NodeKind};

    use super::{
        MftName, USN_REASON_FILE_CREATE, filetime_to_millis, parse_file_name, parse_mft_record,
        parse_usn_buffer,
    };

    fn usn_record(name: &str, file_ref: u64, parent_ref: u64) -> Vec<u8> {
        let name_utf16: Vec<u16> = name.encode_utf16().collect();
        let name_bytes = name_utf16.len() * 2;
        let length = 60 + name_bytes;
        let mut record = vec![0u8; length];

        record[0..4].copy_from_slice(&(length as u32).to_le_bytes());
        record[4..6].copy_from_slice(&2u16.to_le_bytes());
        record[8..16].copy_from_slice(&file_ref.to_le_bytes());
        record[16..24].copy_from_slice(&parent_ref.to_le_bytes());
        record[24..32].copy_from_slice(&42i64.to_le_bytes());
        record[32..40].copy_from_slice(&133_000_000_000_000_000i64.to_le_bytes());
        record[40..44].copy_from_slice(&USN_REASON_FILE_CREATE.to_le_bytes());
        record[56..58].copy_from_slice(&(name_utf16.len() as u16).to_le_bytes());
        record[58..60].copy_from_slice(&60u16.to_le_bytes());
        for (index, unit) in name_utf16.iter().enumerate() {
            record[60 + index * 2..62 + index * 2].copy_from_slice(&unit.to_le_bytes());
        }

        record
    }

    #[test]
    fn parses_usn_records_into_names_and_references() {
        let first = usn_record("report.txt", 0x0100_0000_0000_002A, 0x0100_0000_0000_0005);
        let second = usn_record("фото.jpg", 0x0100_0000_0000_002B, 5);
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&first);
        buffer.extend_from_slice(&second);

        let mut records = Vec::new();
        let count = parse_usn_buffer(&buffer, &mut |record| records.push(record)).expect("parse");

        assert_eq!(count, 2);
        assert_eq!(records[0].name, "report.txt");
        assert_eq!(records[0].file_ref, 42);
        assert_eq!(records[0].parent_ref, 5);
        assert_eq!(records[1].name, "фото.jpg");
        assert_eq!(records[0].reason, USN_REASON_FILE_CREATE);
    }

    #[test]
    fn rejects_truncated_usn_records() {
        let record = usn_record("a.txt", 7, 5);
        let mut buffer = record[..40].to_vec();
        buffer.extend_from_slice(&[0u8; 4]);

        assert!(parse_usn_buffer(&buffer, &mut |_| {}).is_err());
    }

    #[test]
    fn converts_filetime_to_unix_millis() {
        // The Unix epoch in FILETIME units.
        assert_eq!(filetime_to_millis(116_444_736_000_000_000), 0);
        assert_eq!(
            filetime_to_millis(116_444_736_000_000_000 + 1_700_000_000_000 * 10_000),
            1_700_000_000_000
        );
    }

    #[test]
    fn parses_file_name_attributes() {
        let record = file_name_attribute(5, "payload.bin", 1);
        let parsed = parse_file_name(&record).expect("name");

        assert_eq!(
            parsed,
            MftName {
                parent_ref: 5,
                name: "payload.bin".to_owned()
            }
        );
    }

    fn file_name_attribute(parent_ref: u64, name: &str, namespace: u8) -> Vec<u8> {
        let name_utf16: Vec<u16> = name.encode_utf16().collect();
        let mut value = vec![0u8; 66 + name_utf16.len() * 2];
        value[0..8].copy_from_slice(&parent_ref.to_le_bytes());
        value[64] = u8::try_from(name_utf16.len()).expect("short name");
        value[65] = namespace;
        for (index, unit) in name_utf16.iter().enumerate() {
            value[66 + index * 2..68 + index * 2].copy_from_slice(&unit.to_le_bytes());
        }

        let mut attribute = vec![0u8; 24 + value.len()];
        let length = attribute.len() as u32;
        attribute[0..4].copy_from_slice(&0x30u32.to_le_bytes());
        attribute[4..8].copy_from_slice(&length.to_le_bytes());
        attribute[16..20].copy_from_slice(&(value.len() as u32).to_le_bytes());
        attribute[20..22].copy_from_slice(&24u16.to_le_bytes());
        attribute[24..].copy_from_slice(&value);

        attribute
    }

    #[test]
    fn rejects_unused_and_corrupt_mft_records() {
        assert!(parse_mft_record(&[0u8; 1024], 0).is_none());

        let mut record = vec![0u8; 1024];
        record[..4].copy_from_slice(b"FILE");
        record[4..6].copy_from_slice(&56u16.to_le_bytes());
        record[6..8].copy_from_slice(&3u16.to_le_bytes());
        record[20..22].copy_from_slice(&56u16.to_le_bytes());
        record[22..24].copy_from_slice(&1u16.to_le_bytes());
        // Wrong update sequence in the first sector.
        assert!(parse_mft_record(&record, 5).is_none());
    }

    #[test]
    fn maps_directory_attributes() {
        assert!(FileAttributes::DIRECTORY.contains(FileAttributes::DIRECTORY));
        assert!(!NodeKind::File.is_directory());
    }

    fn push_attribute(
        record: &mut Vec<u8>,
        attribute_type: u32,
        value: &[u8],
        non_resident: bool,
        allocated: u64,
        real: u64,
    ) {
        let header = if non_resident { 64 } else { 24 };
        let length = if non_resident {
            header
        } else {
            (header + value.len()).div_ceil(8) * 8
        };
        let offset = record.len();
        record.resize(offset + length, 0);
        record[offset..offset + 4].copy_from_slice(&attribute_type.to_le_bytes());
        record[offset + 4..offset + 8].copy_from_slice(&(length as u32).to_le_bytes());
        record[offset + 8] = u8::from(non_resident);
        if non_resident {
            record[offset + 40..offset + 48].copy_from_slice(&allocated.to_le_bytes());
            record[offset + 48..offset + 56].copy_from_slice(&real.to_le_bytes());
        } else {
            record[offset + 16..offset + 20].copy_from_slice(&(value.len() as u32).to_le_bytes());
            record[offset + 20..offset + 22].copy_from_slice(&(header as u16).to_le_bytes());
            record[offset + header..offset + header + value.len()].copy_from_slice(value);
        }
    }

    fn valid_mft_record() -> Vec<u8> {
        let mut record = vec![0u8; 1024];
        record[..4].copy_from_slice(b"FILE");
        record[4..6].copy_from_slice(&48u16.to_le_bytes());
        record[6..8].copy_from_slice(&3u16.to_le_bytes());
        record[18..20].copy_from_slice(&1u16.to_le_bytes());
        record[20..22].copy_from_slice(&56u16.to_le_bytes());
        record[22..24].copy_from_slice(&1u16.to_le_bytes());
        record.resize(56, 0);

        let mut standard = vec![0u8; 36];
        standard[0..8].copy_from_slice(&133_000_000_000_000_000i64.to_le_bytes());
        standard[8..16].copy_from_slice(&133_000_000_000_000_001i64.to_le_bytes());
        standard[24..32].copy_from_slice(&133_000_000_000_000_002i64.to_le_bytes());
        standard[32..36].copy_from_slice(&0x20u32.to_le_bytes());
        push_attribute(&mut record, 0x10, &standard, false, 0, 0);

        let name: Vec<u16> = "payload.bin".encode_utf16().collect();
        let mut file_name = vec![0u8; 66 + name.len() * 2];
        file_name[0..8].copy_from_slice(&5u64.to_le_bytes());
        file_name[64] = u8::try_from(name.len()).expect("name length");
        file_name[65] = 3;
        for (index, unit) in name.iter().enumerate() {
            file_name[66 + index * 2..68 + index * 2].copy_from_slice(&unit.to_le_bytes());
        }
        push_attribute(&mut record, 0x30, &file_name, false, 0, 0);
        push_attribute(&mut record, 0x80, &[], true, 4096, 1234);
        record.extend_from_slice(&u32::MAX.to_le_bytes());
        record.resize(1024, 0);

        let used = record.len() as u32;
        record[24..28].copy_from_slice(&used.to_le_bytes());
        record[28..32].copy_from_slice(&used.to_le_bytes());
        record[48..50].copy_from_slice(&1u16.to_le_bytes());
        record[50..52].copy_from_slice(&0xAAAAu16.to_le_bytes());
        record[52..54].copy_from_slice(&0xBBBBu16.to_le_bytes());
        record[510..512].copy_from_slice(&1u16.to_le_bytes());
        record[1022..1024].copy_from_slice(&1u16.to_le_bytes());

        record
    }

    #[test]
    fn parses_a_valid_mft_record() {
        let record = valid_mft_record();
        let parsed = parse_mft_record(&record, 42).expect("record");

        assert_eq!(parsed.file_ref, 42);
        assert!(!parsed.is_directory);
        assert_eq!(parsed.size, 1234);
        assert_eq!(parsed.allocated, 4096);
        assert_eq!(parsed.names.len(), 1);
        assert_eq!(parsed.names[0].parent_ref, 5);
        assert_eq!(parsed.names[0].name, "payload.bin");
        assert!(parsed.attributes.contains(FileAttributes::ARCHIVE));
        assert!(parsed.timestamps.modified.as_millis() > 1_600_000_000_000);
    }
}
