//! Transcode to H.264/AAC MP4 and record the size before and after. (Bonus 3)

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::MediaError;
use crate::ffmpeg::{args, encode, QUIET};
use crate::probe::MediaInfo;
use crate::MediaConfig;

/// Explicit stats about a compression operation, describes file state before/after compression
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompressionStats {
    pub original_bytes: u64,
    pub compressed_bytes: u64,
    /// compressed ÷ original; below 1.0 means smaller.
    pub ratio: f64,
    /// Formula: "(1 - ratio) * 100"; if negative -> file grew.
    pub saved_percent: f64,
}

/// Compress a file to H.264/AAC MP4 -> return stats / errors
pub fn compress(cfg: &MediaConfig, input: &Path, info: &MediaInfo, out: &Path) -> Result<CompressionStats, MediaError> {
    let mut a = args(QUIET);
    a.extend(args(["-i".into(), input.as_os_str().to_owned()]));
    a.extend(args([
        "-map".to_string(),
        format!("0:{}", info.video_stream_index),
        "-map".into(),
        "0:a:0?".into(), // "a" == first audio track, if there is one ("?" = optional)
        "-vf".into(),
        "scale=trunc(iw/2)*2:trunc(ih/2)*2".into(), // H.264 4:2:0 needs even sides
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        cfg.preset.clone(),
        "-crf".into(),
        cfg.crf.to_string(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        cfg.audio_bitrate.clone(),
        "-movflags".into(),
        "+faststart".into(), // index at front for streaming (browser-ready)
    ]));
    if cfg.threads > 0 {
        a.extend(args(["-threads".to_string(), cfg.threads.to_string()]));
    }
    a.extend(args(["-f", "mp4"]));

    encode(&cfg.ffmpeg, a, out, cfg.encode_timeout)?;

    let original_bytes = info.size_bytes;
    let compressed_bytes = std::fs::metadata(out)?.len();
    let ratio = compressed_bytes as f64 / original_bytes as f64;
    Ok(CompressionStats {
        original_bytes,
        compressed_bytes,
        ratio,
        saved_percent: (1.0 - ratio) * 100.0,
    })
}
