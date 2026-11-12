//! One producer thread: uploads every video of its own folder, one after another.

use std::path::PathBuf;

use crate::proto::media_upload_client::MediaUploadClient;
use crate::report;
use crate::scanner::list_videos;
use crate::uploader::upload_file;

pub async fn run(id: usize, folder: PathBuf, endpoint: String) {
    let folder_name = folder.display().to_string();

    let videos = match list_videos(&folder) {
        Ok(v) if !v.is_empty() => v,
        Ok(_) => return report::log_error(id, &folder_name, "no videos found"),
        Err(e) => return report::log_error(id, &folder_name, &format!("cannot read folder: {e}")),
    };

    let mut client = match MediaUploadClient::connect(endpoint).await {
        Ok(c) => c,
        Err(e) => {
            return report::log_error(
                id,
                &folder_name,
                &format!("cannot connect to consumer: {e}"),
            )
        }
    };

    for path in videos {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        match upload_file(&mut client, &path).await {
            Ok(outcome) => report::log_outcome(id, &name, &outcome),
            Err(e) => report::log_error(id, &name, &e),
        }
    }
}
