//! Runs the producer's upload logic against an in-process mock consumer.

use std::sync::{Arc, Mutex};

use producer::hasher::sha256_hex;
use producer::proto::media_upload_server::{MediaUpload, MediaUploadServer};
use producer::proto::upload_request::Payload;
use producer::proto::{Status, UploadRequest, UploadResponse};
use producer::uploader::{upload_file, CHUNK_SIZE};
use tonic::{Request, Response, Streaming};

#[derive(Clone)]
struct Mock {
    reply_after_header: Option<Status>,
    seen: Arc<Mutex<(Option<String>, usize)>>, // (sha256 from header, bytes received)
}

#[tonic::async_trait]
impl MediaUpload for Mock {
    async fn upload(
        &self,
        req: Request<Streaming<UploadRequest>>,
    ) -> Result<Response<UploadResponse>, tonic::Status> {
        let mut stream = req.into_inner();
        while let Some(msg) = stream.message().await? {
            match msg.payload {
                Some(Payload::Header(h)) => {
                    self.seen.lock().unwrap().0 = Some(h.sha256);
                    if let Some(s) = self.reply_after_header {
                        return Ok(Response::new(UploadResponse {
                            status: s as i32,
                            message: "early".into(),
                        }));
                    }
                }
                Some(Payload::Chunk(c)) => self.seen.lock().unwrap().1 += c.len(),
                None => {}
            }
        }
        Ok(Response::new(UploadResponse {
            status: Status::Accepted as i32,
            message: "queued".into(),
        }))
    }
}

async fn start(
    mock: Mock,
) -> producer::proto::media_upload_client::MediaUploadClient<tonic::transport::Channel> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(MediaUploadServer::new(mock))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener)),
    );
    producer::proto::media_upload_client::MediaUploadClient::connect(format!("http://{addr}"))
        .await
        .unwrap()
}

fn temp_file(len: usize) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("producer-test-{len}-{}.mp4", std::process::id()));
    std::fs::write(&path, vec![7u8; len]).unwrap();
    path
}

#[tokio::test]
async fn streams_all_bytes_with_correct_hash() {
    let seen = Arc::new(Mutex::new((None, 0)));
    let mut client = start(Mock {
        reply_after_header: None,
        seen: seen.clone(),
    })
    .await;
    let path = temp_file(CHUNK_SIZE * 3 + 123);
    let out = upload_file(&mut client, &path).await.unwrap();
    assert_eq!(out.status, Status::Accepted);
    let (sha, bytes) = seen.lock().unwrap().clone();
    assert_eq!(bytes, CHUNK_SIZE * 3 + 123);
    assert_eq!(sha.unwrap(), sha256_hex(&path).unwrap());
}

#[tokio::test]
async fn early_queue_full_is_reported_without_sending_everything() {
    let seen = Arc::new(Mutex::new((None, 0)));
    let mut client = start(Mock {
        reply_after_header: Some(Status::QueueFull),
        seen: seen.clone(),
    })
    .await;
    let path = temp_file(CHUNK_SIZE * 40);
    let out = upload_file(&mut client, &path).await.unwrap();
    assert_eq!(out.status, Status::QueueFull);
    assert!(seen.lock().unwrap().1 < CHUNK_SIZE * 40);
}
