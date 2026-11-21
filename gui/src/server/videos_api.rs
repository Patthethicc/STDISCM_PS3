use super::AppState;
use crate::app::{document, render};
use crate::components::video_grid::VideoGrid;
use crate::pages::{home::HomePage, watch::WatchPage};
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Json, Response};
use catalog::{read_catalog, VideoEntry};

/// Re-reads catalog.json on every request so new uploads show up.
/// The consumer replaces the file atomically, so a read never sees a partial write.
fn load(state: &AppState) -> Result<Vec<VideoEntry>, Response> {
    read_catalog(state.data_dir.as_path()).map_err(|e| {
        eprintln!("failed to read catalog: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "failed to read catalog").into_response()
    })
}

pub async fn list_videos(State(state): State<AppState>) -> Response {
    match load(&state) {
        Ok(v) => Json(v).into_response(),
        Err(r) => r,
    }
}

pub async fn home(State(state): State<AppState>) -> Response {
    match load(&state) {
        Ok(videos) => Html(document("Media Upload Service", render(move || view! { <HomePage videos=videos/> }))).into_response(),
        Err(r) => r,
    }
}

pub async fn grid_fragment(State(state): State<AppState>) -> Response {
    match load(&state) {
        Ok(videos) => (
            [(header::CACHE_CONTROL, "no-store")],
            Html(render(move || view! { <VideoGrid videos=videos/> })),
        )
            .into_response(),
        Err(r) => r,
    }
}

pub async fn watch(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match load(&state) {
        Ok(videos) => {
            let video = videos.into_iter().find(|v| v.id == id);
            let status = if video.is_some() { StatusCode::OK } else { StatusCode::NOT_FOUND };
            let title = video.as_ref().map(|v| v.filename.clone()).unwrap_or_else(|| "Not found".into());
            (status, Html(document(&title, render(move || view! { <WatchPage video=video/> })))).into_response()
        }
        Err(r) => r,
    }
}

use leptos::prelude::view;
