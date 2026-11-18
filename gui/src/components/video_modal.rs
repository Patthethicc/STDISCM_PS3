use leptos::prelude::*;

/// Full-video player shown on click. app.js fills in the source and opens it.
#[component]
pub fn VideoModal() -> impl IntoView {
    view! {
        <div id="modal" class="modal" hidden=true>
            <div class="modal-box">
                <div class="modal-head">
                    <h2 id="modal-title"></h2>
                    <button id="modal-close" type="button" aria-label="Close">"×"</button>
                </div>
                <video id="modal-video" controls=true playsinline=true></video>
            </div>
        </div>
    }
}
