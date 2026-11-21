use crate::components::{video_grid::VideoGrid, video_modal::VideoModal};
use catalog::VideoEntry;
use leptos::prelude::*;

#[component]
pub fn HomePage(videos: Vec<VideoEntry>) -> impl IntoView {
    view! {
        <header class="top">
            <h1>"Media Upload Service"</h1>
            <span class="live" title="Refreshes automatically">"● live"</span>
        </header>
        <main>
            <VideoGrid videos=videos/>
        </main>
        <VideoModal/>
    }
}
