//! One JPEG frame for the grid card.

use std::path::Path;

use crate::error::MediaError;
use crate::ffmpeg::{args, encode, QUIET};
use crate::probe::MediaInfo;
use crate::MediaConfig;

/// Grabs one frame a little way in (min(1 s, a quarter of the video)), so the
/// card does not show the black first frame many videos start with.
pub fn make_thumbnail(cfg: &MediaConfig, input: &Path, info: &MediaInfo, out: &Path) -> Result<(), MediaError> {
    let at = info.duration_secs.map_or(0.0, |d| (d / 4.0).min(1.0));
    match grab(cfg, input, info, out, at) {
        // Seeking can land past the last decodable frame on odd files; retry at 0.
        Err(e) if at > 0.0 && !matches!(e, MediaError::ToolMissing(_)) => grab(cfg, input, info, out, 0.0),
        other => other,
    }
}

fn grab(cfg: &MediaConfig, input: &Path, info: &MediaInfo, out: &Path, at: f64) -> Result<(), MediaError> {
    let mut a = args(QUIET);
    // -ss before -i = fast seek on the input
    a.extend(args(["-ss".to_string(), format!("{at:.3}")]));
    a.extend(args(["-i".into(), input.as_os_str().to_owned()]));
    a.extend(args([
        "-map".to_string(),
        format!("0:{}", info.video_stream_index),
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        format!("scale='trunc(min({w},iw)/2)*2':-2", w = cfg.thumb_width),
        "-c:v".into(),
        "mjpeg".into(),
        "-q:v".into(),
        "3".into(), // JPEG quality: 2 = best, 31 = worst
        "-update".into(),
        "1".into(),
        "-f".into(),
        "image2".into(),
    ]));
    encode(&cfg.ffmpeg, a, out, cfg.probe_timeout)
}
