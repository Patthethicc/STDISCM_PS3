//! Streams one video file to the consumer over client-streaming gRPC.

use std::path::Path;

use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;

use crate::hasher::sha256_hex;
use crate::proto::media_upload_client::MediaUploadClient;
use crate::proto::upload_request::Payload;
use crate::proto::{Header, Status, UploadRequest};

/// Size of each streamed chunk. Well under gRPC's 4 MiB default message limit,
/// large enough to keep per-message overhead low.
pub const CHUNK_SIZE: usize = 256 * 1024;

/// Final outcome of one upload attempt, as reported by the consumer.
#[derive(Debug)]
pub struct Outcome {
    pub status: Status,
    pub message: String,
}

/// Sends the header, then the file's chunks. Stops quietly when the receiver is
/// dropped, i.e. the consumer already answered (QUEUE_FULL / DUPLICATE / ERROR).
async fn feed(tx: mpsc::Sender<UploadRequest>, mut file: tokio::fs::File, header: Header) {
    let header = UploadRequest {
        payload: Some(Payload::Header(header)),
    };
    if tx.send(header).await.is_err() {
        return;
    }
    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let chunk = UploadRequest {
                    payload: Some(Payload::Chunk(buf[..n].to_vec())),
                };
                if tx.send(chunk).await.is_err() {
                    break;
                }
            }
            Err(_) => break, // stream ends early; the consumer reports the failure
        }
    }
}

/// Uploads `path`. The header goes first so the consumer can answer
/// QUEUE_FULL / DUPLICATE before any video bytes are transferred.
pub async fn upload_file(
    client: &mut MediaUploadClient<Channel>,
    path: &Path,
) -> Result<Outcome, String> {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid file name")?
        .to_string();
    let size_bytes = std::fs::metadata(path)
        .map_err(|e| format!("cannot stat file: {e}"))?
        .len();
    let sha256 = sha256_hex(path).map_err(|e| format!("cannot hash file: {e}"))?;
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("cannot open file: {e}"))?;

    let (tx, rx) = mpsc::channel(4);
    let feeder = tokio::spawn(feed(
        tx,
        file,
        Header {
            filename,
            size_bytes,
            sha256,
        },
    ));

    let result = client.upload(ReceiverStream::new(rx)).await;
    feeder.abort();

    let resp = result
        .map_err(|e| format!("rpc failed: {}", e.message()))?
        .into_inner();
    Ok(Outcome {
        status: Status::try_from(resp.status).unwrap_or(Status::Unspecified),
        message: resp.message,
    })
}
