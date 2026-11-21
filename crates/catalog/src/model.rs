use serde::{Deserialize, Serialize};

/// One processed video. Paths are relative to the data directory
/// (e.g. `uploads/3fa9c2e1.mp4`), so the GUI can serve them under `/media/`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoEntry {
    /// Unique id. Using the first 16 hex chars of the SHA-256 is fine,
    /// since duplicates are rejected before they get here.
    pub id: String,
    /// Original filename sent by the producer.
    pub filename: String,
    /// Hex SHA-256 of the original upload.
    pub sha256: String,
    /// Length of the full video in seconds.
    pub duration_secs: f64,
    /// Size of the upload as received, in bytes.
    pub original_size: u64,
    /// Size after compression (Bonus 3). `None` if not compressed.
    pub compressed_size: Option<u64>,
    /// Full video, e.g. `uploads/<id>.mp4`.
    pub video_path: String,
    /// First-10-seconds clip, e.g. `previews/<id>.mp4`. `None` if generation failed.
    pub preview_path: Option<String>,
    /// Thumbnail image, e.g. `thumbnails/<id>.jpg`. `None` if generation failed.
    pub thumbnail_path: Option<String>,
    /// When the video finished processing, Unix time in milliseconds.
    pub added_at_ms: u64,
}
