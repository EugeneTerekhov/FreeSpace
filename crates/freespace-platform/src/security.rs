//! NTFS attributes, ownership and ACL operations (FR-057…FR-060).
//!
//! Ownership changes need the `SeTakeOwnershipPrivilege`; ACL restoration needs
//! write-DACL access. Every failure is reported per object by the operations
//! layer, so this module keeps the WinAPI surface small and predictable.

use std::{ffi::OsStr, io, path::Path};

use freespace_core::{Error, Result};
use windows::{
    Win32::{
        Foundation::{ERROR_SUCCESS, GetLastError, HLOCAL, LUID, LocalFree, WIN32_ERROR},
        Security::Authorization::{
            ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT, SetNamedSecurityInfoW,
        },
        Security::{
            ACL, DACL_SECURITY_INFORMATION, LookupAccountNameW, LookupAccountSidW,
            LookupPrivilegeValueW, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
            SE_PRIVILEGE_ENABLED, SE_RESTORE_NAME, SE_TAKE_OWNERSHIP_NAME, SID_NAME_USE,
            TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
            UNPROTECTED_DACL_SECURITY_INFORMATION,
        },
        Storage::FileSystem::{FILE_FLAGS_AND_ATTRIBUTES, GetFileAttributesW, SetFileAttributesW},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    },
    core::{PCWSTR, PWSTR},
};

use super::{paths, win};

/// `INVALID_FILE_ATTRIBUTES`.
const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFF_FFFF;

/// Reads the raw Windows attribute mask.
///
/// # Errors
/// Returns [`Error::NotFound`] for a missing object and [`Error::Io`] otherwise.
pub fn get_attributes(path: &Path) -> Result<u32> {
    let verbatim = paths::to_verbatim(path)?;
    let wide = win::to_wide(verbatim.as_os_str());

    // SAFETY: `wide` is NUL-terminated and lives for the call.
    let attributes = unsafe { GetFileAttributesW(PCWSTR(wide.as_ptr())) };
    if attributes == INVALID_FILE_ATTRIBUTES {
        let error = win::last_error();
        return Err(if error.kind() == io::ErrorKind::NotFound {
            Error::NotFound(paths::display(path))
        } else {
            Error::Io(error)
        });
    }

    Ok(attributes)
}

/// Applies `set` bits and clears `clear` bits atomically.
///
/// # Errors
/// Returns an error when the attributes cannot be read or written.
pub fn set_attributes(path: &Path, set: u32, clear: u32) -> Result<u32> {
    let current = get_attributes(path)?;
    let updated = (current | set) & !clear;
    let verbatim = paths::to_verbatim(path)?;
    let wide = win::to_wide(verbatim.as_os_str());

    // SAFETY: `wide` is NUL-terminated and lives for the call.
    unsafe { SetFileAttributesW(PCWSTR(wide.as_ptr()), FILE_FLAGS_AND_ATTRIBUTES(updated)) }
        .map_err(|error| Error::Io(win::win_error(&error)))?;

    Ok(updated)
}

/// Returns the owner account name, or its SID when the name cannot be resolved.
///
/// # Errors
/// Returns an error when the security descriptor cannot be read.
pub fn owner(path: &Path) -> Result<Option<String>> {
    let (owner, descriptor) = owner_sid(path)?;
    if owner.0.is_null() {
        free_descriptor(descriptor);
        return Ok(None);
    }

    let name = sid_to_name(owner).or_else(|| sid_to_string(owner));
    free_descriptor(descriptor);

    Ok(name)
}

/// Owner identity used by the per-owner disk-usage report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerIdentity {
    /// Security identifier in string form (`S-1-5-...`).
    pub sid: String,
    /// Resolved account name, or the SID when the account no longer exists.
    pub display_name: String,
}

/// Returns the SID and display name of an object's owner.
///
/// # Errors
/// Returns an error when the security descriptor cannot be read.
pub fn owner_identity(path: &Path) -> Result<Option<OwnerIdentity>> {
    let (owner, descriptor) = owner_sid(path)?;
    if owner.0.is_null() {
        free_descriptor(descriptor);
        return Ok(None);
    }

    let sid = sid_to_string(owner).unwrap_or_default();
    let display_name = sid_to_name(owner).unwrap_or_else(|| sid.clone());
    free_descriptor(descriptor);

    if sid.is_empty() {
        return Ok(None);
    }

    Ok(Some(OwnerIdentity { sid, display_name }))
}

/// Sets the owner of an object.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for an unknown account and
/// [`Error::AccessDenied`] when the privilege or write access is missing.
pub fn set_owner(path: &Path, account: &str) -> Result<()> {
    enable_privilege(SE_TAKE_OWNERSHIP_NAME)?;
    enable_privilege(SE_RESTORE_NAME)?;
    let mut sid = lookup_account(account)?;
    let verbatim = paths::to_verbatim(path)?;
    let wide = win::to_wide(verbatim.as_os_str());

    // SAFETY: `wide` is NUL-terminated, `sid` lives through the call and is a
    // valid account SID.
    let status = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(PSID(sid.as_mut_ptr().cast())),
            None,
            None,
            None,
        )
    };
    check_status(status, "cannot set the owner")
}

/// Resets explicit permissions to the parent directory's permission set.
///
/// Copying the parent DACL keeps inherited entries (the parent DACL already
/// contains them) and never installs a NULL DACL, which would grant full access
/// to everyone. The volume root has no parent and is refused.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for a path without a parent and an error
/// when the parent DACL cannot be read or applied.
pub fn reset_acl(path: &Path) -> Result<()> {
    let normalized = paths::normalize(path)?;
    let display = paths::strip_verbatim(&normalized);
    let parent = display.parent().ok_or_else(|| {
        Error::InvalidArgument(format!(
            "`{}` has no parent whose permissions could be inherited",
            paths::display(path)
        ))
    })?;
    if parent.as_os_str().is_empty() {
        return Err(Error::InvalidArgument(format!(
            "`{}` is a volume root and has no parent permissions",
            paths::display(path)
        )));
    }

    copy_acl(parent, path)
}

/// Copies the DACL of `from` to `to`.
///
/// # Errors
/// Returns an error when either descriptor cannot be read or written.
pub fn copy_acl(from: &Path, to: &Path) -> Result<()> {
    enable_privilege(SE_RESTORE_NAME)?;
    let (dacl, descriptor) = named_dacl(from)?;
    let target = paths::to_verbatim(to)?;
    let wide = win::to_wide(target.as_os_str());

    // SAFETY: `wide` is NUL-terminated and the DACL stays alive until the call
    // returns; the descriptor is freed afterwards.
    let status = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | UNPROTECTED_DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(dacl),
            None,
        )
    };
    free_descriptor(descriptor);

    check_status(status, "cannot copy permissions")
}

fn owner_sid(path: &Path) -> Result<(PSID, PSECURITY_DESCRIPTOR)> {
    let verbatim = paths::to_verbatim(path)?;
    let wide = win::to_wide(verbatim.as_os_str());
    let mut owner = PSID::default();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();

    // SAFETY: the path is NUL-terminated and the out-pointers are valid.
    let status = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            None,
            None,
            &mut descriptor,
        )
    };
    check_status(status, "cannot read the owner")?;

    Ok((owner, descriptor))
}

fn named_dacl(path: &Path) -> Result<(*const ACL, PSECURITY_DESCRIPTOR)> {
    let verbatim = paths::to_verbatim(path)?;
    let wide = win::to_wide(verbatim.as_os_str());
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();

    // SAFETY: the path is NUL-terminated and the out-pointers are valid.
    let status = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut dacl),
            None,
            &mut descriptor,
        )
    };
    check_status(status, "cannot read permissions")?;
    if dacl.is_null() {
        free_descriptor(descriptor);
        return Err(Error::Operation(
            "the object has a NULL DACL and cannot be copied".to_owned(),
        ));
    }

    Ok((dacl, descriptor))
}

fn lookup_account(account: &str) -> Result<Vec<u8>> {
    let wide = win::to_wide(OsStr::new(account));
    let mut sid = vec![0u8; 68]; // SECURITY_MAX_SID_SIZE
    let mut sid_len = u32::try_from(sid.len()).unwrap_or(0);
    let mut domain = [0u16; 256];
    let mut domain_len = u32::try_from(domain.len()).unwrap_or(0);
    let mut sid_type = SID_NAME_USE(0);

    // SAFETY: buffers are valid for the call and the lengths match them.
    unsafe {
        LookupAccountNameW(
            PCWSTR::null(),
            PCWSTR(wide.as_ptr()),
            Some(PSID(sid.as_mut_ptr().cast())),
            &mut sid_len,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut domain_len,
            &mut sid_type,
        )
    }
    .map_err(|error| {
        let io_error = win::win_error(&error);
        if io_error.raw_os_error() == Some(1332) {
            Error::InvalidArgument(format!("unknown account `{account}`"))
        } else {
            Error::Io(io_error)
        }
    })?;
    sid.truncate(sid_len as usize);

    Ok(sid)
}

fn sid_to_name(sid: PSID) -> Option<String> {
    let mut name = [0u16; 256];
    let mut name_len = u32::try_from(name.len()).ok()?;
    let mut domain = [0u16; 256];
    let mut domain_len = u32::try_from(domain.len()).ok()?;
    let mut sid_type = SID_NAME_USE(0);

    // SAFETY: buffers and lengths are valid for the call.
    unsafe {
        LookupAccountSidW(
            PCWSTR::null(),
            sid,
            Some(PWSTR(name.as_mut_ptr())),
            &mut name_len,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut domain_len,
            &mut sid_type,
        )
    }
    .ok()?;
    let account = win::from_wide(&name).to_string_lossy().into_owned();
    let domain = win::from_wide(&domain).to_string_lossy().into_owned();
    if domain.is_empty() {
        Some(account)
    } else {
        Some(format!("{domain}\\{account}"))
    }
}

fn sid_to_string(sid: PSID) -> Option<String> {
    let mut string = PWSTR::null();

    // SAFETY: `sid` is a valid SID and the out-pointer is valid. The returned
    // string is allocated by LocalAlloc and freed below.
    unsafe { ConvertSidToStringSidW(sid, &mut string) }.ok()?;
    let text = unsafe { string.to_string() }.ok();
    // SAFETY: `string` was allocated by the security API and is owned here.
    unsafe { LocalFree(Some(HLOCAL(string.0.cast()))) };

    text
}

fn enable_privilege(name: PCWSTR) -> Result<()> {
    let mut raw_token = windows::Win32::Foundation::HANDLE::default();
    // SAFETY: the current process handle is pseudo-handle and the out-pointer
    // is valid.
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut raw_token,
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;
    let token = win::Handle::from_raw(raw_token)?;

    let mut luid = LUID::default();
    // SAFETY: the privilege name is static and the out-pointer is valid.
    unsafe { LookupPrivilegeValueW(PCWSTR::null(), name, &mut luid) }
        .map_err(|error| Error::Io(win::win_error(&error)))?;

    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [windows::Win32::Security::LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    // SAFETY: the token is valid and the structure lives for the call.
    unsafe {
        windows::Win32::Security::AdjustTokenPrivileges(
            token.raw(),
            false,
            Some(&privileges),
            0,
            None,
            None,
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;

    // `AdjustTokenPrivileges` reports success even when the privilege is not
    // held; the subsequent operation reports the real access-denied error.

    Ok(())
}

fn check_status(status: WIN32_ERROR, context: &str) -> Result<()> {
    if status.0 == ERROR_SUCCESS.0 {
        return Ok(());
    }

    let io_error = io::Error::from_raw_os_error(status.0 as i32);
    Err(match io_error.kind() {
        io::ErrorKind::PermissionDenied => Error::AccessDenied(format!("{context}: {io_error}")),
        io::ErrorKind::NotFound => Error::NotFound(format!("{context}: {io_error}")),
        _ => Error::Io(io::Error::new(
            io_error.kind(),
            format!("{context}: {io_error}"),
        )),
    })
}

fn free_descriptor(descriptor: PSECURITY_DESCRIPTOR) {
    if !descriptor.0.is_null() {
        // SAFETY: the descriptor came from GetNamedSecurityInfoW and is owned
        // by this scope.
        unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    }
}

/// Converts `GetLastError` into an error; kept for symmetry with other modules.
#[must_use]
pub fn last_error() -> io::Error {
    // SAFETY: GetLastError has no preconditions.
    let code = unsafe { GetLastError() };

    io::Error::from_raw_os_error(code.0 as i32)
}

#[cfg(test)]
mod tests {
    use super::{get_attributes, owner, owner_identity, set_attributes};
    use freespace_core::ErrorCode;

    #[test]
    fn reads_and_updates_attributes() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, b"payload").expect("write");

        let before = get_attributes(&path).expect("attributes");
        assert_eq!(
            before & 0x20,
            0x20,
            "new files carry FILE_ATTRIBUTE_ARCHIVE"
        );

        // FILE_ATTRIBUTE_HIDDEN = 0x2, FILE_ATTRIBUTE_ARCHIVE = 0x20.
        let updated = set_attributes(&path, 0x2, 0x20).expect("set");
        assert_eq!(updated & 0x2, 0x2);
        assert_eq!(updated & 0x20, 0);
        set_attributes(&path, 0, 0x2).expect("clear hidden");
        assert_eq!(get_attributes(&path).expect("attributes") & 0x2, 0);
    }

    #[test]
    fn reads_the_current_owner() {
        let directory = tempfile::tempdir().expect("temp dir");
        let owner = owner(directory.path()).expect("owner");

        assert!(owner.as_deref().is_none_or(|value| !value.is_empty()));
    }

    #[test]
    fn returns_the_owner_sid_and_name() {
        let directory = tempfile::tempdir().expect("temp dir");
        let identity = owner_identity(directory.path())
            .expect("owner identity")
            .expect("temporary directory has an owner");

        assert!(identity.sid.starts_with("S-1-"), "sid: {}", identity.sid);
        assert!(!identity.display_name.is_empty());
    }

    #[test]
    fn reports_missing_objects() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = get_attributes(&directory.path().join("missing.bin")).expect_err("missing");

        assert_eq!(error.code(), ErrorCode::NotFound);
    }
}
