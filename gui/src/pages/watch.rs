use crate::components::media_url;
use catalog::VideoEntry;
use leptos::prelude::*;

/// Standalone player page at `/watch/{id}`, for direct links.
#[component]
pub fn WatchPage(video: Option<VideoEntry>) -> impl IntoView {
    view! {
        <header class="top"><a href="/">"← Back"</a></header>
        <main class="watch">
            {match video {
                Some(v) => view! {
                    <h2>{v.filename.clone()}</h2>
                    <video src=media_url(&v.video_path) controls=true autoplay=true playsinline=true></video>
                }.into_any(),
                None => view! { <p class="empty">"Video not found."</p> }.into_any(),
            }}
        </main>
    }
}
