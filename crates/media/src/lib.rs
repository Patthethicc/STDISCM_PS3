//! Media processing for the P3 consumer (via ffmpeg):
//! probe, 10-second preview, thumbnail and (bonus) H.264/AAC compression
//!
//! Workers call [`process_video`] once per finished upload. It is a plain
//! blocking function: call it straight from a worker thread, or through
//! `tokio::task::spawn_blocking` from async code.

mod compress;
mod error;
mod ffmpeg;
mod preview;
mod probe;
mod thumbnail;

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use compress::{compress, CompressionStats};
pub use error::MediaError;
pub use ffmpeg::check_tools;
pub use preview::make_preview;
pub use probe::{probe, MediaInfo};
pub use thumbnail::make_thumbnail;

#[derive(Debug, Clone)]
pub struct MediaConfig {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub preview_secs: f64,
    pub preview_width: u32,
    pub thumb_width: u32,
    /// x264 quality: 18 near-lossless, 23 default, 28 small. Lower = bigger file.
    pub crf: u8,
    /// x264 speed/size trade-off: ultrafast … veryfast … medium … slow.
    pub preset: String,
    pub audio_bitrate: String,
    /// Threads per ffmpeg process; 0 lets ffmpeg decide (it uses every core).
    pub threads: u32,
    pub probe_timeout: Duration,
    pub encode_timeout: Duration,
    /// Turn Bonus 3 off for fast test runs.
    pub compress: bool,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            preview_secs: 10.0,
            preview_width: 480,
            thumb_width: 320,
            crf: 23,
            preset: "veryfast".into(),
            audio_bitrate: "128k".into(),
            threads: 0,
            probe_timeout: Duration::from_secs(30),
            encode_timeout: Duration::from_secs(600),
            compress: true,
        }
    }
}

/// Where the outputs for one video go.
/// Named by video id so GUI can build URLs from id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputPaths {
    pub preview: PathBuf,
    pub thumbnail: PathBuf,
    pub compressed: PathBuf,
}

impl OutputPaths {
    /// data/previews/<id>.mp4, data/thumbnails/<id>.jpg, data/uploads/<id>.mp4
    /// (`compressed` is the transcode and is the file the GUI plays).
    pub fn for_id(data_dir: &Path, id: &str) -> Self {
        Self {
            preview: data_dir.join("previews").join(format!("{id}.mp4")),
            thumbnail: data_dir.join("thumbnails").join(format!("{id}.jpg")),
            compressed: data_dir.join("uploads").join(format!("{id}.mp4")),
        }
    }

    fn remove_all(&self) {
        for p in [&self.preview, &self.thumbnail, &self.compressed] {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Everything the catalog needs about one processed video.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessedMedia {
    pub info: MediaInfo,
    pub preview: PathBuf,
    /// Real preview length: 10.0, or less for short videos.
    pub preview_secs: f64,
    /// Length of the file the GUI will play (what to show on the card).
    /// Can be shorter than `info.duration_secs` when the upload was damaged.
    pub duration_secs: Option<f64>,
    /// None if the frame grab failed (the GUI shows a placeholder).
    pub thumbnail: Option<PathBuf>,
    /// None if compression is off or failed.
    pub compression: Option<CompressionStats>,
    /// The file the GUI should play on click: the transcode (`uploads/<id>.mp4`),
    /// or the original upload if compression is off or failed.
    pub playback: PathBuf,
    /// Non-fatal problems, for the log.
    pub warnings: Vec<String>,
}

/// Probe → thumbnail → preview → compress.
///
/// Probe and preview are required: if either fails the whole video fails,
/// partial outputs are deleted, and the error says why. Thumbnail and
/// compression are best-effort and only add a warning.
pub fn process_video(cfg: &MediaConfig, input: &Path, out: &OutputPaths) -> Result<ProcessedMedia, MediaError> {
    // Never write an output over the input (and never let cleanup delete it).
    if [&out.preview, &out.thumbnail, &out.compressed].contains(&&input.to_path_buf()) {
        return Err(MediaError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "an output path equals the input file")));
    }
    let result = run_pipeline(cfg, input, out);
    if result.is_err() {
        out.remove_all();
    }
    result
}

fn run_pipeline(cfg: &MediaConfig, input: &Path, out: &OutputPaths) -> Result<ProcessedMedia, MediaError> {
    let info = probe(cfg, input)?;
    let mut warnings = Vec::new();

    let thumbnail = match make_thumbnail(cfg, input, &info, &out.thumbnail) {
        Ok(()) => Some(out.thumbnail.clone()),
        Err(MediaError::ToolMissing(t)) => return Err(MediaError::ToolMissing(t)),
        Err(e) => {
            warnings.push(format!("thumbnail: {e}"));
            None
        }
    };

    let preview_secs = make_preview(cfg, input, &info, &out.preview)?;

    let mut playback = input.to_path_buf();
    let compression = if cfg.compress {
        match compress(cfg, input, &info, &out.compressed) {
            Ok(stats) => {
                playback = out.compressed.clone();
                Some(stats)
            }
            Err(MediaError::ToolMissing(t)) => return Err(MediaError::ToolMissing(t)),
            Err(e) => {
                warnings.push(format!("compression: {e}"));
                None
            }
        }
    } else {
        None
    };

    let duration_secs = if playback == input { info.duration_secs } else { probe::duration_of(cfg, &playback) };
    if let (Some(claimed), Some(real)) = (info.duration_secs, duration_secs) {
        if claimed - real > 1.0 {
            warnings.push(format!("source looks damaged: header says {claimed:.1}s but only {real:.1}s could be decoded"));
        }
    }

    Ok(ProcessedMedia {
        info,
        preview: out.preview.clone(),
        preview_secs,
        duration_secs,
        thumbnail,
        compression,
        playback,
        warnings,
    })
}
