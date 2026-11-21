use crate::job::Job;
use crate::queue::JobReceiver;
use crate::storage::Storage;
use catalog::{Catalog, VideoEntry};
use sha2::{Digest, Sha256};
use std::io::Write; // <-- Use standard sync Write for the temp file
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

/// One read-only media config shared by every worker (no locking needed).
static MEDIA_CFG: OnceLock<media::MediaConfig> = OnceLock::new();

fn media_cfg() -> &'static media::MediaConfig {
    MEDIA_CFG.get_or_init(media::MediaConfig::default)
}

pub async fn worker_loop(
    mut rx: JobReceiver,
    storage: Arc<Storage>,
    catalog: Arc<Catalog>,
    token: CancellationToken,
    worker_id: usize,
) {
    loop {
        let job = tokio::select! {
            maybe = rx.recv() => match maybe { Some(j) => j, None => break },
            _ = token.cancelled() => break,
        };
        let result = process_job(job, &storage, &catalog).await;
        tracing::info!(worker_id, ok = result.is_ok(), "job finished");
    }
    tracing::debug!(worker_id, "worker exiting");
}

pub async fn process_job(
    job: Job,
    storage: &Storage,
    catalog: &Catalog,
) -> Result<String, String> {
    // 1. Destructure the job so we can extract the `respond` channel
    let Job { filename, sha256, expected_size, chunks, respond } = job;

    // 2. Run the actual processing in an inner function
    let result = process_job_inner(filename, sha256, expected_size, chunks, storage, catalog).await;

    // 3. Send the result back to the gRPC server so it can reply to the client!
    let _ = respond.send(result.clone());

    result
}

// Rename the old process_job to process_job_inner and update its arguments
async fn process_job_inner(
    filename: String,
    sha256: String,
    expected_size: u64,
    mut chunks: tokio::sync::mpsc::Receiver<Result<Vec<u8>, tonic::Status>>,
    storage: &Storage,
    catalog: &Catalog,
) -> Result<String, String> {
    let mut staging = storage.create_staging().map_err(|e| format!("staging: {e}"))?;
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    
    while let Some(chunk) = chunks.recv().await {
        match chunk {
            Ok(bytes) => {
                hasher.update(&bytes);
                total += bytes.len() as u64;
                staging.as_file_mut().write_all(&bytes).map_err(|e| format!("write: {e}"))?;
            }
            Err(status) => {
                storage.release(&sha256);
                return Err(format!("client stream error: {status}"));
            }
        }
    }

    let actual_hex: String = hasher.finalize().iter()
        .map(|b| format!("{b:02x}")).collect();
    if actual_hex != sha256.to_ascii_lowercase() {
        storage.release(&sha256);
        return Err(format!("sha256 mismatch: expected {}, got {}", sha256, actual_hex));
    }
    if total != expected_size {
        storage.release(&sha256);
        return Err(format!("size mismatch: expected {}, received {}", expected_size, total));
    }
    
    staging.as_file_mut().flush().map_err(|e| {
        storage.release(&sha256);
        format!("flush: {e}")
    })?;

    // Media processing (probe, preview, thumbnail, transcode) runs on the staging
    // file, so the transcode can be written to uploads/<id>.mp4 without clashing
    // with the original. process_video blocks (it waits on ffmpeg), so it goes to
    // the blocking pool; at most one runs per worker.
    let id = Storage::id_from_sha(&sha256);
    let out = media::OutputPaths {
        preview: storage.preview_abs(&id),
        thumbnail: storage.thumbnail_abs(&id),
        compressed: storage.video_abs(&id),
    };
    let input = staging.path().to_path_buf();
    let processed = tokio::task::spawn_blocking(move || media::process_video(media_cfg(), &input, &out)).await;
    let m = match processed {
        Ok(Ok(m)) => m,
        Ok(Err(e)) => {
            // process_video already removed any partial outputs; dropping `staging` deletes the upload.
            storage.release(&sha256);
            let kind = if e.is_bad_input() { "rejected" } else { "media error" };
            return Err(format!("{kind}: {e}"));
        }
        Err(e) => { storage.release(&sha256); return Err(format!("media task failed: {e}")); }
    };
    for w in &m.warnings {
        tracing::warn!(%id, "{w}");
    }

    // Transcode succeeded: it replaces the original at uploads/<id>.mp4.
    // Otherwise keep the original upload as the playable file.
    let video_path = if m.compression.is_some() {
        drop(staging);
        Storage::transcoded_rel(&id)
    } else {
        if let Err(e) = storage.persist(staging, &sha256, &filename) {
            let _ = std::fs::remove_file(&m.preview);
            if let Some(t) = &m.thumbnail { let _ = std::fs::remove_file(t); }
            storage.release(&sha256);
            return Err(e);
        }
        Storage::video_rel(&id, &filename)
    };
    let preview_path = Some(Storage::preview_rel(&id));
    let thumbnail_path = m.thumbnail.as_ref().map(|_| Storage::thumbnail_rel(&id));
    let duration_secs = m.duration_secs.or(m.info.duration_secs).unwrap_or(0.0);

    let added_at_ms = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_millis() as u64;

    let entry = VideoEntry {
        id: id.clone(),
        filename: filename.clone(),
        sha256: sha256.to_ascii_lowercase(),
        duration_secs,
        original_size: expected_size,
        compressed_size: m.compression.as_ref().map(|c| c.compressed_bytes),
        video_path,
        preview_path,
        thumbnail_path,
        added_at_ms,
    };

    if let Err(e) = catalog.add(entry) {
        storage.release(&sha256);
        return Err(format!("catalog add: {e}"));
    }

    Ok(id)
}

pub fn spawn_workers(
    mut queue_rx: JobReceiver,
    storage: Arc<Storage>,
    catalog: Arc<Catalog>,
    token: CancellationToken,
    c: usize,
    tracker: &tokio_util::task::TaskTracker,
) {
    // ffmpeg is CPU-bound: share the cores between the c workers instead of letting
    // every ffmpeg process grab all of them.
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let _ = MEDIA_CFG.set(media::MediaConfig { threads: (cores / c.max(1)).max(1) as u32, ..Default::default() });

    let mut txs = Vec::with_capacity(c);
    let mut rxs = Vec::with_capacity(c);
    for _ in 0..c {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        txs.push(tx); rxs.push(rx);
    }
    let token_d = token.clone();
    tracker.spawn(async move {
        let mut i = 0usize;
        loop {
            tokio::select! {
                maybe = queue_rx.recv() => match maybe {
                    Some(job) => { if txs[i % c].send(job).await.is_err() { break; } i += 1; }
                    None => break,
                },
                _ = token_d.cancelled() => break,
            }
        }
        drop(txs);
    });
    for (id, rx) in rxs.into_iter().enumerate() {
        let st = storage.clone();
        let cat = catalog.clone();
        let tok = token.clone();
        tracker.spawn(worker_loop(rx, st, cat, tok, id));
    }
}