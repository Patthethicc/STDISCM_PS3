//! Finds the videos in one thread's folder.

use std::io;
use std::path::{Path, PathBuf};

/// Video extensions the producer picks up (case-insensitive).
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "m4v", "mpg", "mpeg", "ts", "3gp",
];

fn is_video(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Video files directly inside `dir` (not recursive), sorted by path.
pub fn list_videos(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut videos: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| is_video(p))
        .collect();
    videos.sort();
    Ok(videos)
}
