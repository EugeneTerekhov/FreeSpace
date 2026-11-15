//! `freespace drives` — dashboard of logical drives.

use freespace_core::{Result, model::VolumeStatus, settings::Settings, size::format_bytes};
use freespace_platform::{paths, volumes};
use serde_json::json;

use super::Outcome;

/// Lists logical drives, including volumes that cannot be queried.
///
/// # Errors
/// Returns an error when the drive list cannot be obtained.
pub fn run(settings: &Settings) -> Result<Outcome> {
    let infos = volumes::list_volumes()?;
    let mut items = Vec::with_capacity(infos.len());
    let mut warnings = Vec::new();
    let mut human = String::new();

    for info in infos {
        if info.status != VolumeStatus::Available {
            warnings.push(format!(
                "volume `{}` is inaccessible",
                paths::display(&info.mount_point)
            ));
        }

        let domain = info.clone().into_volume();
        domain.validate()?;

        let mount_point = paths::display(&info.mount_point);
        let label = if info.label.is_empty() {
            String::new()
        } else {
            info.label.clone()
        };

        items.push(json!({
            "mount_point": mount_point,
            "label": label,
            "file_system": info.file_system.as_str(),
            "total_bytes": info.total_bytes,
            "free_bytes": info.free_bytes,
            "used_bytes": domain.used_bytes(),
            "cluster_size": info.cluster_size,
            "serial_number": info.serial_number,
            "drive_type": info.drive_type.as_str(),
            "is_removable": info.drive_type.is_removable(),
            "status": info.status.as_str(),
        }));

        let size = if info.status == VolumeStatus::Available {
            format!(
                "{:>10} free of {:>10}",
                format_bytes(info.free_bytes, settings.units),
                format_bytes(info.total_bytes, settings.units)
            )
        } else {
            format!("{:>10}", info.status.as_str())
        };
        human.push_str(&format!(
            "{:<4} {:<6} {size}  {label}\n",
            mount_point,
            info.file_system.as_str().to_uppercase(),
        ));
    }

    Ok(Outcome::new(json!({ "items": items }), human).with_warnings(warnings))
}
