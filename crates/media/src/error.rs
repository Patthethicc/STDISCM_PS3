use std::path::PathBuf;

/// All possible errors in crates/media. Workers log these and move on
/// No panics on bad input in this crate.
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    /// ffmpeg/ffprobe is not installed or not on PATH (check at startup).
    #[error("{0} is not installed or not on PATH")]
    ToolMissing(String),

    /// The file is corrupt, empty, audio-only, a still image, or not media at all.
    #[error("not a playable video: {0}")]
    InvalidMedia(String),

    /// ffmpeg/ffprobe ran longer than the configured limit and was killed.
    #[error("{tool} timed out after {secs}s")]
    Timeout { tool: String, secs: u64 },

    /// ffmpeg exited with a non-zero status. `stderr` holds its last lines.
    #[error("{tool} failed (exit code {code:?}): {stderr}")]
    ToolFailed { tool: String, code: Option<i32>, stderr: String },

    /// ffmpeg reported success but wrote nothing.
    #[error("{0} is empty after encoding")]
    EmptyOutput(PathBuf),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl MediaError {
    /// TRUE = uploaded file is bad/corrupt/invalid (log as "rejected"),
    /// FALSE = error came from environment (missing tool, disk, timeout).
    pub fn is_bad_input(&self) -> bool {
        matches!(self, MediaError::InvalidMedia(_) | MediaError::EmptyOutput(_))
    }
}
