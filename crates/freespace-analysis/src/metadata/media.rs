//! Audio and video metadata through `lofty` and `mp4parse` (FR-061).

use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path};

use freespace_core::{Error, Result};
use lofty::file::{AudioFile as _, TaggedFileExt as _};
use mp4parse::{TrackType, read_mp4};

use super::tags;

/// Reads audio tags and stream properties.
///
/// # Errors
/// Returns [`Error::Operation`] when the file is not an audio container or the
/// container is damaged.
pub fn read_audio(path: &Path) -> Result<BTreeMap<String, String>> {
    let tagged = lofty::read_from_path(path)
        .map_err(|error| Error::Operation(format!("cannot read audio metadata: {error}")))?;
    let properties = tagged.properties();
    let mut tags_map = BTreeMap::new();

    tags_map.insert(
        tags::CODEC.to_owned(),
        format!("{:?}", tagged.file_type()).to_lowercase(),
    );
    let duration = properties.duration();
    if !duration.is_zero() {
        tags_map.insert(
            tags::DURATION_SECS.to_owned(),
            duration.as_secs().to_string(),
        );
    }
    if let Some(bitrate) = properties
        .audio_bitrate()
        .or_else(|| properties.overall_bitrate())
    {
        tags_map.insert(tags::BITRATE_KBPS.to_owned(), bitrate.to_string());
    }
    if let Some(sample_rate) = properties.sample_rate() {
        tags_map.insert(tags::SAMPLE_RATE.to_owned(), sample_rate.to_string());
    }
    if let Some(channels) = properties.channels() {
        tags_map.insert(tags::CHANNELS.to_owned(), channels.to_string());
    }

    Ok(tags_map)
}

/// Reads video dimensions, duration and codec from an ISO base media file.
///
/// # Errors
/// Returns [`Error::Operation`] when the file is not an MP4/MOV container or
/// the container is damaged.
pub fn read_video(path: &Path) -> Result<BTreeMap<String, String>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let context = read_mp4(&mut reader)
        .map_err(|error| Error::Operation(format!("cannot read video metadata: {error}")))?;

    let mut tags_map = BTreeMap::new();
    let mut best: Option<(u32, u32, Option<u64>, Option<String>)> = None;

    for track in &context.tracks {
        if !matches!(
            track.track_type,
            TrackType::Video | TrackType::AuxiliaryVideo
        ) {
            continue;
        }

        let (mut width, mut height) = (0u32, 0u32);
        if let Some(header) = &track.tkhd {
            width = from_fixed_point(header.width);
            height = from_fixed_point(header.height);
        }
        if (width == 0 || height == 0)
            && let Some(description) = &track.stsd
        {
            for entry in &description.descriptions {
                if let mp4parse::SampleEntry::Video(video) = entry {
                    width = u32::from(video.width);
                    height = u32::from(video.height);
                    break;
                }
            }
        }

        let duration = match (track.duration, track.timescale) {
            (Some(duration), Some(scale)) if scale.0 > 0 => Some(duration.0 / scale.0),
            _ => None,
        };
        let codec = track.stsd.as_ref().and_then(|description| {
            description
                .descriptions
                .iter()
                .find_map(|entry| match entry {
                    mp4parse::SampleEntry::Video(video) => {
                        Some(format!("{:?}", video.codec_type).to_lowercase())
                    }
                    _ => None,
                })
        });

        let area = width.saturating_mul(height);
        if best
            .as_ref()
            .is_none_or(|(current_width, current_height, _, _)| {
                area > current_width.saturating_mul(*current_height)
            })
        {
            best = Some((width, height, duration, codec));
        }
    }

    if let Some((width, height, duration, codec)) = best {
        if width > 0 && height > 0 {
            tags_map.insert(tags::WIDTH.to_owned(), width.to_string());
            tags_map.insert(tags::HEIGHT.to_owned(), height.to_string());
        }
        if let Some(duration) = duration {
            tags_map.insert(tags::DURATION_SECS.to_owned(), duration.to_string());
        }
        if let Some(codec) = codec {
            tags_map.insert(tags::CODEC.to_owned(), codec);
        }
    }

    if tags_map.is_empty() {
        return Err(Error::Operation(
            "the container has no readable video track".to_owned(),
        ));
    }

    Ok(tags_map)
}

/// Converts a 16.16 fixed-point dimension to whole pixels.
///
/// Some writers store plain pixels in `tkhd`; values below one pixel unit are
/// returned unchanged instead of collapsing to zero.
fn from_fixed_point(value: u32) -> u32 {
    if value >= 0x0001_0000 {
        value >> 16
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{read_audio, read_video};

    #[test]
    fn rejects_corrupted_media() {
        let directory = tempfile::tempdir().expect("temp dir");
        let audio = directory.path().join("broken.mp3");
        std::fs::write(&audio, b"not audio").expect("write");
        assert!(read_audio(Path::new(&audio)).is_err());

        let video = directory.path().join("broken.mp4");
        std::fs::write(&video, b"not video").expect("write");
        assert!(read_video(Path::new(&video)).is_err());
    }
}
