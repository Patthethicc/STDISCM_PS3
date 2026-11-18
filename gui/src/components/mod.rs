pub mod hover_preview;
pub mod video_card;
pub mod video_grid;
pub mod video_modal;

/// URL for a file in the data folder (stored as a path relative to it).
pub fn media_url(rel: &str) -> String {
    format!("/media/{}", rel.trim_start_matches('/'))
}

pub fn format_duration(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1.0 { format!("{mb:.1} MB") } else { format!("{:.0} KB", bytes as f64 / 1024.0) }
}
