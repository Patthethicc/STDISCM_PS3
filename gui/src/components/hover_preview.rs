use super::media_url;
use catalog::VideoEntry;
use leptos::prelude::*;

/// Thumbnail with a muted preview video on top. app.js plays the preview on
/// mouse enter and stops it on mouse leave. If the 10-second clip is missing,
/// it falls back to the full video and app.js stops it at 10 seconds.
#[component]
pub fn HoverPreview(video: VideoEntry) -> impl IntoView {
    let (src, limit) = match &video.preview_path {
        Some(p) => (media_url(p), None),
        None => (media_url(&video.video_path), Some("10")),
    };
    let thumb = video.thumbnail_path.as_deref().map(media_url);
    view! {
        <div class="preview">
            {thumb.map(|t| view! { <img class="thumb" src=t alt="" loading="lazy"/> })}
            <video class="clip" src=src data-limit=limit muted=true playsinline=true preload="none"></video>
        </div>
    }
}
