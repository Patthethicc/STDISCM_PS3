use crate::job::Job;
use crate::queue::JobReceiver;
use crate::storage::Storage;
use catalog::{Catalog, VideoEntry};
use sha2::{Digest, Sha256};
use std::io::Write; // <-- Use standard sync Write for the temp file
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

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

    let duration_secs = match ffprobe_duration(staging.path()).await {
        Ok(d) => d,
        Err(e) => { storage.release(&sha256); return Err(format!("ffprobe: {e}")); }
    };

    let uploaded_path = match storage.persist(staging, &sha256, &filename) {
        Ok(p) => p,
        Err(e) => { storage.release(&sha256); return Err(e); }
    };
    let id = Storage::id_from_sha(&sha256);

    let preview_abs = storage.preview_abs(&id);
    let preview_path = match generate_preview(&uploaded_path, &preview_abs).await {
        Ok(()) => Some(Storage::preview_rel(&id)),
        Err(e) => { tracing::warn!(%id, error = %e, "preview failed");
                    let _ = tokio::fs::remove_file(&preview_abs).await; None }
    };
    let thumb_abs = storage.thumbnail_abs(&id);
    let thumbnail_path = match generate_thumbnail(&uploaded_path, &thumb_abs).await {
        Ok(()) => Some(Storage::thumbnail_rel(&id)),
        Err(e) => { tracing::warn!(%id, error = %e, "thumbnail failed");
                    let _ = tokio::fs::remove_file(&thumb_abs).await; None }
    };

    let added_at_ms = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_millis() as u64;

    let entry = VideoEntry {
        id: id.clone(),
        filename: filename.clone(),
        sha256: sha256.to_ascii_lowercase(),
        duration_secs,
        original_size: expected_size,
        compressed_size: None,
        video_path: Storage::video_rel(&id, &filename),
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

// ── ffmpeg / ffprobe helpers ─────────────────────────────────────────

async fn ffprobe_duration(path: &Path) -> Result<f64, String> {
    let out = tokio::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(path)
        .output()
        .await
        .map_err(|e| format!("ffprobe spawn: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim().parse::<f64>().map_err(|e| format!("duration parse: {e}"))
}

async fn generate_preview(input: &Path, output: &Path) -> Result<(), String> {
    let status = tokio::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-t", "10", "-c:v", "libx264", "-preset", "veryfast", "-c:a", "aac", "-movflags", "+faststart"])
        .arg(output)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|e| format!("ffmpeg preview spawn: {e}"))?;
    if !status.success() {
        return Err("ffmpeg preview failed".into());
    }
    Ok(())
}

async fn generate_thumbnail(input: &Path, output: &Path) -> Result<(), String> {
    let status = tokio::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-ss", "1", "-vframes", "1", "-q:v", "3"])
        .arg(output)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|e| format!("ffmpeg thumb spawn: {e}"))?;
    if !status.success() {
        return Err("ffmpeg thumbnail failed".into());
    }
    Ok(())
}

pub fn spawn_workers(
    mut queue_rx: JobReceiver,
    storage: Arc<Storage>,
    catalog: Arc<Catalog>,
    token: CancellationToken,
    c: usize,
    tracker: &tokio_util::task::TaskTracker,
) {
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