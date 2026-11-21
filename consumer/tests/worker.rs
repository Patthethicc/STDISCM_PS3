use consumer::job::Job;
use consumer::storage::Storage;
use consumer::worker::process_job;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tonic::Status;

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn job_with_bytes(
    filename: &str,
    bytes: Vec<u8>,
    declared_sha: String,
    declared_size: u64,
) -> (Job, oneshot::Receiver<Result<String, String>>) {
    let (chunk_tx, chunk_rx) = mpsc::channel::<Result<Vec<u8>, Status>>(8);
    let (respond_tx, respond_rx) = oneshot::channel();
    let job = Job {
        filename: filename.into(),
        sha256: declared_sha,
        expected_size: declared_size,
        chunks: chunk_rx,
        respond: respond_tx,
    };
    tokio::spawn(async move {
        let _ = chunk_tx.send(Ok(bytes)).await;
    });
    (job, respond_rx)
}

#[tokio::test]
async fn sha_mismatch_releases_reservation_and_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::new(tmp.path()).unwrap());
    let catalog = Arc::new(catalog::Catalog::open(tmp.path()).unwrap());

    let bytes = b"hello world".to_vec();
    let wrong_sha = "f".repeat(64);
    assert!(storage.try_reserve(&wrong_sha));

    let (job, _rx) = job_with_bytes("x.mp4", bytes.clone(), wrong_sha.clone(), bytes.len() as u64);
    let err = process_job(job, &storage, &catalog).await.unwrap_err();
    assert!(err.contains("sha256 mismatch"));
    assert!(storage.try_reserve(&wrong_sha));
}

#[tokio::test]
async fn size_mismatch_releases_reservation_and_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::new(tmp.path()).unwrap());
    let catalog = Arc::new(catalog::Catalog::open(tmp.path()).unwrap());

    let bytes = b"hello".to_vec();
    let correct_sha = sha256_hex(&bytes);
    assert!(storage.try_reserve(&correct_sha));

    let (job, _rx) = job_with_bytes("x.mp4", bytes, correct_sha.clone(), 9999);
    let err = process_job(job, &storage, &catalog).await.unwrap_err();
    assert!(err.contains("size mismatch"));
    assert!(storage.try_reserve(&correct_sha));
}

#[tokio::test]
#[ignore = "requires ffmpeg and ffprobe on PATH"]
async fn happy_path_produces_video_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::new(tmp.path()).unwrap());
    let catalog = Arc::new(catalog::Catalog::open(tmp.path()).unwrap());

    let src = tmp.path().join("src.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "lavfi",
               "-i", "testsrc=duration=2:size=128x128:rate=10",
               "-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(&src).status().expect("ffmpeg missing").success();
    assert!(ok);

    let bytes = std::fs::read(&src).unwrap();
    let sha = sha256_hex(&bytes);
    assert!(storage.try_reserve(&sha));

    let (job, _rx) = job_with_bytes("clip.mp4", bytes.clone(), sha.clone(), bytes.len() as u64);
    let id = process_job(job, &storage, &catalog).await.unwrap();
    assert_eq!(id, &sha[..16]);

    let listed = catalog.list();
    assert_eq!(listed.len(), 1);
    let e = &listed[0];
    assert_eq!(e.id, &sha[..16]);
    assert!(e.duration_secs > 0.0);
    assert_eq!(e.original_size, bytes.len() as u64);
    assert!(tmp.path().join(&e.video_path).exists());
}