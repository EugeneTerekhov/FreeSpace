//! Volume summaries for the dashboard and the scan screen.
//!
//! Enumeration and WinAPI calls stay in `freespace-platform`; this module only
//! converts the domain [`Volume`] into the interface DTO.

use freespace_core::model::Volume;
use freespace_platform::volumes;

use crate::{
    dto::{VolumeDetail, VolumeSummary},
    error::{UiError, UiErrorCode},
};

/// Lists every logical drive as a dashboard summary.
///
/// # Errors
/// Returns `E_IO` when the drive mask cannot be obtained.
pub fn list_volume_summaries() -> Result<Vec<VolumeSummary>, UiError> {
    Ok(volumes::list_volumes()?
        .into_iter()
        .map(|info| summary(&info.into_volume()))
        .collect())
}

/// Finds a volume by its stable identifier.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed identifier and `E_NOT_FOUND` when no
/// drive matches.
pub fn find_volume(volume_id: &str) -> Result<Volume, UiError> {
    let id = uuid::Uuid::parse_str(volume_id)
        .map_err(|_| UiError::validation("volume_id is not a UUID"))?;

    volumes::list_volumes()?
        .into_iter()
        .map(|info| info.into_volume())
        .find(|volume| volume.id == id)
        .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("unknown volume"))
}

/// Returns the detailed summary of one volume.
///
/// # Errors
/// Returns `E_VALIDATION` or `E_NOT_FOUND` as in [`find_volume`].
pub fn volume_detail(volume_id: &str) -> Result<VolumeDetail, UiError> {
    let volume = find_volume(volume_id)?;
    let total = volume.total_bytes;
    let used_fraction = if total == 0 {
        0.0
    } else {
        (volume.used_bytes() as f64 / total as f64) as f32
    };

    Ok(VolumeDetail {
        summary: summary(&volume),
        cluster_size: volume.cluster_size,
        used_fraction,
    })
}

/// Converts a domain volume into its dashboard projection.
#[must_use]
pub fn summary(volume: &Volume) -> VolumeSummary {
    VolumeSummary {
        volume_id: volume.id.to_string(),
        mount_point: volume.mount_point.to_string_lossy().into_owned(),
        label: volume.label.clone(),
        file_system: volume.file_system.as_str().to_owned(),
        total_bytes: volume.total_bytes,
        free_bytes: volume.free_bytes,
        used_bytes: volume.used_bytes(),
        is_removable: volume.is_removable,
        status: volume.status.as_str().to_owned(),
        scan_method: volume.scan_method.map(|method| method.as_str().to_owned()),
        is_elevated: volume.is_elevated,
    }
}

#[cfg(test)]
mod tests {
    use freespace_core::model::{FileSystem, Volume, VolumeStatus};

    use super::{find_volume, summary};

    fn volume() -> Volume {
        Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(r"C:\")),
            mount_point: r"C:\".into(),
            label: "System".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000,
            free_bytes: 250,
            cluster_size: Some(4096),
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        }
    }

    #[test]
    fn projects_the_domain_volume() {
        let summary = summary(&volume());

        assert_eq!(summary.mount_point, r"C:\");
        assert_eq!(summary.file_system, "ntfs");
        assert_eq!(summary.total_bytes, 1_000);
        assert_eq!(summary.free_bytes, 250);
        assert_eq!(summary.used_bytes, 750);
        assert_eq!(summary.status, "available");
        assert!(summary.scan_method.is_none());
    }

    #[test]
    fn rejects_malformed_volume_ids_before_enumeration() {
        let error = find_volume("not-a-uuid").expect_err("must fail");
        assert_eq!(error.code, crate::error::UiErrorCode::Validation);
    }
}
