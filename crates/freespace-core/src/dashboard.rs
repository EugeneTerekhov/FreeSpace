//! Volume dashboard summary and snapshot trends (US22, FR-089, FR-090).
//!
//! The dashboard is a pure aggregation over [`Volume`] records and stored
//! snapshot points. Keeping it in `freespace-core` lets the CLI and the GUI show
//! exactly the same numbers; the platform layer only collects the raw volumes.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    Timestamp, Uuid,
    model::{Volume, VolumeStatus},
};

/// Maximum number of trend points kept per volume.
pub const MAX_TREND_POINTS: usize = 256;

/// One snapshot observation of a volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotPoint {
    /// When the snapshot was taken.
    pub taken_at: Timestamp,
    /// Occupied bytes at that moment.
    pub used_bytes: u64,
    /// Free bytes at that moment.
    pub free_bytes: u64,
}

impl SnapshotPoint {
    /// Builds a point from the volume figures stored in a snapshot.
    #[must_use]
    pub const fn new(taken_at: Timestamp, used_bytes: u64, free_bytes: u64) -> Self {
        Self {
            taken_at,
            used_bytes,
            free_bytes,
        }
    }

    /// Total size reported by the snapshot.
    #[must_use]
    pub const fn total_bytes(self) -> u64 {
        self.used_bytes.saturating_add(self.free_bytes)
    }
}

/// Occupancy history of one volume.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeTrend {
    /// Points ordered by time.
    pub points: Vec<SnapshotPoint>,
}

impl VolumeTrend {
    /// Orders, deduplicates and bounds the points.
    #[must_use]
    pub fn from_points(mut points: Vec<SnapshotPoint>) -> Self {
        points.sort_by_key(|point| point.taken_at.as_millis());
        points.dedup_by_key(|point| point.taken_at.as_millis());
        if points.len() > MAX_TREND_POINTS {
            let excess = points.len() - MAX_TREND_POINTS;
            points.drain(..excess);
        }

        Self { points }
    }

    /// Returns `true` when no snapshots are known.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// First and latest observation.
    #[must_use]
    pub fn bounds(&self) -> Option<(SnapshotPoint, SnapshotPoint)> {
        let first = *self.points.first()?;
        let latest = *self.points.last()?;

        Some((first, latest))
    }

    /// Change in used bytes between the first and the latest snapshot.
    ///
    /// Positive values mean the volume is filling up.
    #[must_use]
    pub fn change_bytes(&self) -> i64 {
        let Some((first, latest)) = self.bounds() else {
            return 0;
        };

        let difference = i128::from(latest.used_bytes) - i128::from(first.used_bytes);

        i64::try_from(difference).unwrap_or(if difference.is_negative() {
            i64::MIN
        } else {
            i64::MAX
        })
    }

    /// Average growth per day over the observed period.
    #[must_use]
    pub fn growth_per_day(&self) -> f64 {
        let Some((first, latest)) = self.bounds() else {
            return 0.0;
        };
        let elapsed_ms = latest
            .taken_at
            .as_millis()
            .saturating_sub(first.taken_at.as_millis());
        if elapsed_ms <= 0 {
            return 0.0;
        }

        #[allow(clippy::cast_precision_loss)]
        let days = elapsed_ms as f64 / 86_400_000.0;
        #[allow(clippy::cast_precision_loss)]
        let change = self.change_bytes() as f64;

        change / days
    }
}

/// Aggregated state of one volume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeSummary {
    /// Volume itself.
    pub volume: Volume,
    /// Occupied bytes (`total - free`, saturating).
    pub used_bytes: u64,
    /// Usage in whole percent.
    pub used_percent: u8,
    /// Snapshot history.
    pub trend: VolumeTrend,
}

impl VolumeSummary {
    /// Summarises one volume without snapshot data.
    #[must_use]
    pub fn new(volume: Volume) -> Self {
        let used_bytes = volume.used_bytes();
        let used_percent = if volume.total_bytes == 0 {
            0
        } else {
            let percent = (u128::from(used_bytes) * 100) / u128::from(volume.total_bytes);
            u8::try_from(percent.min(100)).unwrap_or(100)
        };

        Self {
            volume,
            used_bytes,
            used_percent,
            trend: VolumeTrend::default(),
        }
    }

    /// Usage as a ratio in `0.0..=1.0`.
    #[must_use]
    pub fn usage_ratio(&self) -> f32 {
        if self.volume.total_bytes == 0 {
            return 0.0;
        }

        #[allow(clippy::cast_precision_loss)]
        let ratio = self.used_bytes as f32 / self.volume.total_bytes as f32;

        ratio.clamp(0.0, 1.0)
    }

    /// Returns `true` when the volume can be analysed.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.volume.status == VolumeStatus::Available
    }

    /// Mount point used to start an analysis.
    #[must_use]
    pub fn mount_point(&self) -> &Path {
        &self.volume.mount_point
    }
}

/// Totals across every volume.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardTotals {
    /// Number of volumes.
    pub volumes: usize,
    /// Number of available volumes.
    pub available: usize,
    /// Total bytes.
    pub total_bytes: u64,
    /// Used bytes.
    pub used_bytes: u64,
    /// Free bytes.
    pub free_bytes: u64,
}

/// Summary of every volume on the machine.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dashboard {
    /// When the summary was collected.
    pub generated_at: Timestamp,
    /// One record per volume, in enumeration order.
    pub volumes: Vec<VolumeSummary>,
}

impl Dashboard {
    /// Builds a dashboard from volume records.
    #[must_use]
    pub fn from_volumes(volumes: Vec<Volume>) -> Self {
        Self {
            generated_at: Timestamp::now(),
            volumes: volumes.into_iter().map(VolumeSummary::new).collect(),
        }
    }

    /// Attaches snapshot trends keyed by volume id.
    pub fn attach_trends(&mut self, trends: &HashMap<Uuid, Vec<SnapshotPoint>>) {
        for summary in &mut self.volumes {
            if let Some(points) = trends.get(&summary.volume.id) {
                summary.trend = VolumeTrend::from_points(points.clone());
            }
        }
    }

    /// Finds a volume by mount point.
    #[must_use]
    pub fn find(&self, mount_point: &Path) -> Option<&VolumeSummary> {
        let wanted = mount_point.to_string_lossy();

        self.volumes.iter().find(|summary| {
            summary
                .volume
                .mount_point
                .to_string_lossy()
                .eq_ignore_ascii_case(&wanted)
        })
    }

    /// Finds a volume by its stable identifier.
    #[must_use]
    pub fn find_by_id(&self, id: Uuid) -> Option<&VolumeSummary> {
        self.volumes.iter().find(|summary| summary.volume.id == id)
    }

    /// Aggregated totals.
    #[must_use]
    pub fn totals(&self) -> DashboardTotals {
        let mut totals = DashboardTotals {
            volumes: self.volumes.len(),
            ..DashboardTotals::default()
        };
        for summary in &self.volumes {
            if summary.is_available() {
                totals.available += 1;
            }
            totals.total_bytes = totals
                .total_bytes
                .saturating_add(summary.volume.total_bytes);
            totals.used_bytes = totals.used_bytes.saturating_add(summary.used_bytes);
            totals.free_bytes = totals.free_bytes.saturating_add(summary.volume.free_bytes);
        }

        totals
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Dashboard, SnapshotPoint, VolumeSummary, VolumeTrend};
    use crate::{
        Timestamp, Uuid,
        model::{FileSystem, Volume, VolumeStatus},
    };

    fn volume(letter: char, total: u64, free: u64) -> Volume {
        let mount_point = PathBuf::from(format!("{letter}:\\"));

        Volume {
            id: Volume::id_for_mount_point(&mount_point),
            mount_point,
            label: format!("Disk {letter}"),
            file_system: FileSystem::Ntfs,
            total_bytes: total,
            free_bytes: free,
            cluster_size: Some(4096),
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        }
    }

    #[test]
    fn summarises_usage() {
        let summary = VolumeSummary::new(volume('C', 1_000, 250));

        assert_eq!(summary.used_bytes, 750);
        assert_eq!(summary.used_percent, 75);
        assert!((summary.usage_ratio() - 0.75).abs() < f32::EPSILON);
        assert!(summary.is_available());
        assert_eq!(summary.mount_point(), std::path::Path::new(r"C:\"));
    }

    #[test]
    fn handles_empty_and_inaccessible_volumes() {
        let empty = VolumeSummary::new(volume('D', 0, 0));
        let mut inaccessible = volume('E', 100, 10);
        inaccessible.status = VolumeStatus::Inaccessible;
        let summary = VolumeSummary::new(inaccessible);

        assert_eq!(empty.used_percent, 0);
        assert_eq!(empty.usage_ratio(), 0.0);
        assert!(!summary.is_available());
        assert_eq!(summary.used_bytes, 90);
    }

    #[test]
    fn aggregates_totals() {
        let mut offline = volume('F', 200, 100);
        offline.status = VolumeStatus::Offline;
        let dashboard = Dashboard::from_volumes(vec![
            volume('C', 1_000, 250),
            volume('D', 2_000, 500),
            offline,
        ]);
        let totals = dashboard.totals();

        assert_eq!(totals.volumes, 3);
        assert_eq!(totals.available, 2);
        assert_eq!(totals.total_bytes, 3_200);
        assert_eq!(totals.used_bytes, 2_350);
        assert_eq!(totals.free_bytes, 850);
        assert!(dashboard.find(std::path::Path::new(r"c:\")).is_some());
        assert!(dashboard.find(std::path::Path::new(r"Z:\")).is_none());
    }

    #[test]
    fn orders_and_bounds_trends() {
        let trend = VolumeTrend::from_points(vec![
            SnapshotPoint::new(Timestamp::from_millis(2_000), 300, 700),
            SnapshotPoint::new(Timestamp::from_millis(1_000), 100, 900),
            SnapshotPoint::new(Timestamp::from_millis(2_000), 300, 700),
        ]);

        assert_eq!(trend.points.len(), 2);
        assert_eq!(trend.change_bytes(), 200);
        assert!(trend.growth_per_day() > 0.0);
        let (first, latest) = trend.bounds().expect("bounds");
        assert_eq!(first.used_bytes, 100);
        assert_eq!(latest.used_bytes, 300);
    }

    #[test]
    fn attaches_trends_by_volume_id() {
        let c = volume('C', 1_000, 500);
        let d = volume('D', 2_000, 1_500);
        let c_id = c.id;
        let mut dashboard = Dashboard::from_volumes(vec![c, d]);
        let mut trends = std::collections::HashMap::new();
        trends.insert(
            c_id,
            vec![SnapshotPoint::new(Timestamp::from_millis(0), 100, 900)],
        );

        dashboard.attach_trends(&trends);

        assert_eq!(dashboard.find_by_id(c_id).expect("C").trend.points.len(), 1);
        assert!(
            dashboard
                .volumes
                .iter()
                .all(|summary| summary.volume.id != Uuid::nil())
        );
        assert!(dashboard.volumes[1].trend.is_empty());
    }

    #[test]
    fn empty_trends_are_neutral() {
        let trend = VolumeTrend::default();

        assert!(trend.is_empty());
        assert_eq!(trend.change_bytes(), 0);
        assert_eq!(trend.growth_per_day(), 0.0);
        assert!(trend.bounds().is_none());
    }
}
