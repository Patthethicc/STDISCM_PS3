//! The hover preview: the first N seconds (default 10), small and silent.

use std::path::Path;

use crate::error::MediaError;
use crate::ffmpeg::{args, encode, QUIET};
use crate::probe::{duration_of, MediaInfo};
use crate::MediaConfig;

/// Writes an H.264 MP4 of the first `cfg.preview_secs` seconds to `out`.
/// Returns the preview's real length: shorter videos give a shorter preview.
pub fn make_preview(cfg: &MediaConfig, input: &Path, info: &MediaInfo, out: &Path) -> Result<f64, MediaError> {
    let length = match info.duration_secs {
        Some(d) => d.min(cfg.preview_secs),
        None => cfg.preview_secs,
    };

    let mut a = args(QUIET);
    a.extend(args(["-i".into(), input.as_os_str().to_owned()]));
    a.extend(args([
        "-t".to_string(),
        format!("{}", cfg.preview_secs), // stops at 10 s, or at the end if shorter
        "-map".into(),
        format!("0:{}", info.video_stream_index),
        "-an".into(), // hover previews autoplay muted anyway
        "-vf".into(),
        // never upscale; keep both sides even (libx264 + yuv420p need that)
        format!("scale='trunc(min({w},iw)/2)*2':-2", w = cfg.preview_width),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-crf".into(),
        "28".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-movflags".into(),
        "+faststart".into(),
    ]));
    if cfg.threads > 0 {
        a.extend(args(["-threads".to_string(), cfg.threads.to_string()]));
    }
    a.extend(args(["-f", "mp4"]));

    encode(&cfg.ffmpeg, a, out, cfg.encode_timeout)?;
    // A damaged source can end early; report what was really written.
    Ok(duration_of(cfg, out).unwrap_or(length))
}
