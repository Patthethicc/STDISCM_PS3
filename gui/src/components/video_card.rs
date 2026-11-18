use super::{format_duration, format_size, media_url, hover_preview::HoverPreview};
use catalog::VideoEntry;
use leptos::prelude::*;

#[component]
pub fn VideoCard(video: VideoEntry) -> impl IntoView {
    let full = media_url(&video.video_path);
    let duration = format_duration(video.duration_secs);
    let size = match video.compressed_size {
        Some(c) => format!("{} → {}", format_size(video.original_size), format_size(c)),
        None => format_size(video.original_size),
    };
    view! {
        <article class="card" data-src=full data-title=video.filename.clone() tabindex="0">
            <div class="media">
                <HoverPreview video=video.clone()/>
                <span class="duration">{duration}</span>
            </div>
            <div class="info">
                <h3 title=video.filename.clone()>{video.filename.clone()}</h3>
                <p class="meta">{size}</p>
            </div>
        </article>
    }
}
