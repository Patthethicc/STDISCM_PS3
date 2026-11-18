use std::path::Path;
use tower_http::services::ServeDir;

/// Serves the data folder under /media (uploads/, previews/, thumbnails/).
/// ServeDir supports HTTP range requests, which browsers need to seek in videos.
pub fn media_service(data_dir: &Path) -> ServeDir {
    ServeDir::new(data_dir)
}
