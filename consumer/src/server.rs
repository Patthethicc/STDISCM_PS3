use crate::job::Job;
use crate::queue::{try_enqueue, JobSender};
use crate::storage::Storage;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tonic::{Request, Response, Status, Streaming};

use crate::pb::{
    media_upload_server::{MediaUpload, MediaUploadServer},
    upload_request::Payload,
    Header, Status as MediaStatus, UploadRequest, UploadResponse,
};

pub struct MediaUploadImpl {
    queue_tx: JobSender,
    storage: Arc<Storage>,
}

impl MediaUploadImpl {
    pub fn new(queue_tx: JobSender, storage: Arc<Storage>) -> Self {
        Self { queue_tx, storage }
    }
}

#[tonic::async_trait]
impl MediaUpload for MediaUploadImpl {
    async fn upload(
        &self,
        request: Request<Streaming<UploadRequest>>,
    ) -> Result<Response<UploadResponse>, Status> {
        let mut stream = request.into_inner();

        // 1. Read ONLY the header
        let header: Header = match stream.message().await? {
            Some(UploadRequest { payload: Some(Payload::Header(h)) }) => h,
            _ => return Err(Status::invalid_argument("first message must be Header")),
        };

        // 2. Early queue-full check (Bonus 1)
        if self.queue_tx.capacity() == 0 {
            return Ok(Response::new(UploadResponse {
                status: MediaStatus::QueueFull as i32,
                message: "queue is full".into(),
            }));
        }

        // 3. Early duplicate check
        if !self.storage.try_reserve(&header.sha256) {
            return Ok(Response::new(UploadResponse {
                status: MediaStatus::Duplicate as i32,
                message: format!("duplicate sha256 {}", header.sha256),
            }));
        }

        // 4. Build the job & enqueue
        let (chunk_tx, chunk_rx) = mpsc::channel::<Result<Vec<u8>, Status>>(8);
        let (done_tx, done_rx) = oneshot::channel();

        let job = Job {
            filename: header.filename,
            sha256: header.sha256.clone(),
            expected_size: header.size_bytes,
            chunks: chunk_rx,
            respond: done_tx,
        };

        if !try_enqueue(&self.queue_tx, job) {
            // Race: queue filled between the capacity check and here.
            self.storage.release(&header.sha256);
            return Ok(Response::new(UploadResponse {
                status: MediaStatus::QueueFull as i32,
                message: "queue is full (race)".into(),
            }));
        }

        // 5. Forward the remaining stream to the worker
        tokio::spawn(forward_stream(stream, chunk_tx));

        // 6. Wait for the worker to finish and reply
        match done_rx.await {
            Ok(Ok(id)) => Ok(Response::new(UploadResponse {
                status: MediaStatus::Accepted as i32,
                message: id,
            })),
            Ok(Err(reason)) => Ok(Response::new(UploadResponse {
                status: MediaStatus::Error as i32,
                message: reason,
            })),
            Err(_) => Ok(Response::new(UploadResponse {
                status: MediaStatus::Error as i32,
                message: "Worker disconnected unexpectedly".into(),
            })),
        }
    }
}

async fn forward_stream(
    mut stream: Streaming<UploadRequest>,
    tx: mpsc::Sender<Result<Vec<u8>, Status>>,
) {
    loop {
        match stream.message().await {
            Ok(Some(UploadRequest { payload: Some(Payload::Chunk(bytes)) })) => {
                if tx.send(Ok(bytes)).await.is_err() {
                    // Worker dropped the receiver (e.g. failed early).
                    return;
                }
            }
            Ok(Some(_)) => {
                // Extra header mid-stream: ignore.
            }
            Ok(None) => return, // client finished
            Err(status) => {
                let _ = tx.send(Err(status)).await;
                return;
            }
        }
    }
}

pub fn build_service(
    queue_tx: JobSender,
    storage: Arc<Storage>,
) -> MediaUploadServer<MediaUploadImpl> {
    MediaUploadServer::new(MediaUploadImpl::new(queue_tx, storage))
}