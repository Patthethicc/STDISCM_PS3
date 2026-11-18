use super::video_card::VideoCard;
use catalog::VideoEntry;
use leptos::prelude::*;

/// The grid of uploaded videos. Also served alone at `/fragments/grid`
/// so app.js can refresh it without reloading the page.
#[component]
pub fn VideoGrid(videos: Vec<VideoEntry>) -> impl IntoView {
    let count = videos.len();
    view! {
        <div id="grid-root">
            <p class="count">{format!("{count} video{}", if count == 1 { "" } else { "s" })}</p>
            {if videos.is_empty() {
                view! { <p class="empty">"No uploads yet. New videos appear here automatically."</p> }.into_any()
            } else {
                view! {
                    <div class="grid">
                        {videos.into_iter().map(|v| view! { <VideoCard video=v/> }).collect_view()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}
