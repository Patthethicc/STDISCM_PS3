mod file_serving;
mod videos_api;

use axum::{routing::get, Router};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub data_dir: Arc<PathBuf>,
}

pub fn router(data_dir: PathBuf) -> Router {
    let state = AppState { data_dir: Arc::new(data_dir.clone()) };
    Router::new()
        .route("/", get(videos_api::home))
        .route("/watch/{id}", get(videos_api::watch))
        .route("/fragments/grid", get(videos_api::grid_fragment))
        .route("/api/videos", get(videos_api::list_videos))
        .nest_service("/media", file_serving::media_service(&data_dir))
        .with_state(state)
}
