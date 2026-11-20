//! Read a file's facts with ffprobe and decide whether it is a video at all.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::MediaError;
use crate::ffmpeg::{args, run};
use crate::MediaConfig;

/// What we learned about the uploaded file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaInfo {
    /// Seconds; None when the container does not say (rare, e.g. some live-recorded WebM).
    pub duration_secs: Option<f64>,
    pub width: u32,
    pub height: u32,
    /// e.g. "h264", "mpeg4", "vp9".
    pub video_codec: String,
    /// None when the file has no audio track.
    pub audio_codec: Option<String>,
    /// ffprobe's format name, e.g. "mov,mp4,m4a,3gp,3g2,mj2", "avi", "matroska,webm".
    pub container: String,
    pub size_bytes: u64,
    /// ffprobe stream index of the real video stream (skips cover art).
    pub video_stream_index: usize,
}

impl MediaInfo {
    /// Already H.264 (+AAC or silent) in MP4/MOV: a browser can play it as is.
    pub fn is_browser_ready(&self) -> bool {
        self.container.contains("mp4")
            && self.video_codec == "h264"
            && matches!(self.audio_codec.as_deref(), None | Some("aac"))
    }
}

#[derive(Deserialize)]
struct ProbeJson {
    #[serde(default)]
    streams: Vec<StreamJson>,
    format: Option<FormatJson>,
}

#[derive(Deserialize)]
struct StreamJson {
    index: usize,
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<String>,
    #[serde(default)]
    disposition: Disposition,
}

#[derive(Deserialize, Default)]
struct Disposition {
    #[serde(default)]
    attached_pic: u8,
}

#[derive(Deserialize)]
struct FormatJson {
    format_name: Option<String>,
    duration: Option<String>,
}

pub fn probe(cfg: &MediaConfig, input: &Path) -> Result<MediaInfo, MediaError> {
    let size_bytes = std::fs::metadata(input)?.len();
    if size_bytes == 0 {
        return Err(MediaError::InvalidMedia("file is empty".into()));
    }

    let mut a = args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"]);
    a.push(input.into());

    let out = match run(&cfg.ffprobe, &a, cfg.probe_timeout) {
        Ok(out) => out,
        // ffprobe could not open/parse it: corrupt or not media.
        Err(MediaError::ToolFailed { stderr, .. }) => {
            let reason = stderr.lines().last().unwrap_or("unreadable file").to_string();
            return Err(MediaError::InvalidMedia(reason));
        }
        Err(other) => return Err(other),
    };

    let parsed: ProbeJson = serde_json::from_slice(&out.stdout)
        .map_err(|e| MediaError::InvalidMedia(format!("unexpected ffprobe output: {e}")))?;
    let format = parsed.format.ok_or_else(|| MediaError::InvalidMedia("no container format".into()))?;
    let container = format.format_name.unwrap_or_default();

    // Still images (png_pipe, jpeg_pipe, image2) show up as one-frame "video".
    if container == "image2" || container.ends_with("_pipe") {
        return Err(MediaError::InvalidMedia(format!("still image ({container}), not a video")));
    }

    let video = parsed
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video") && s.disposition.attached_pic == 0)
        .ok_or_else(|| MediaError::InvalidMedia("no video stream (audio-only or not media)".into()))?;

    let (width, height) = match (video.width, video.height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
        _ => return Err(MediaError::InvalidMedia("video stream has no frame size".into())),
    };

    let audio_codec = parsed
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"))
        .and_then(|s| s.codec_name.clone());

    let duration_secs = parse_secs(format.duration.as_deref())
        .or_else(|| parse_secs(video.duration.as_deref()));

    Ok(MediaInfo {
        duration_secs,
        width,
        height,
        video_codec: video.codec_name.clone().unwrap_or_else(|| "unknown".into()),
        audio_codec,
        container,
        size_bytes,
        video_stream_index: video.index,
    })
}

/// Length of a file we produced ourselves (preview, compressed copy).
/// The source's header can lie when the file is damaged; our own output cannot.
pub(crate) fn duration_of(cfg: &MediaConfig, path: &Path) -> Option<f64> {
    let mut a = args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"]);
    a.push(path.into());
    let out = run(&cfg.ffprobe, &a, cfg.probe_timeout).ok()?;
    parse_secs(Some(String::from_utf8_lossy(&out.stdout).trim()))
}

/// ffprobe prints durations as strings ("12.003000") or "N/A".
fn parse_secs(s: Option<&str>) -> Option<f64> {
    s?.parse::<f64>().ok().filter(|d| d.is_finite() && *d > 0.0)
}
