mod common;

use common::dummy_job;
use consumer::pb::{self, media_upload_client::MediaUploadClient, upload_request::Payload, Header, UploadRequest};
use consumer::queue::{build_leaky_bucket, try_enqueue};
use consumer::server::build_service;
use consumer::storage::Storage;

use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::transport::Channel;

fn header_msg(filename: &str, sha: &str, size: u64) -> UploadRequest {
    UploadRequest {
        payload: Some(Payload::Header(Header {
            filename: filename.into(),
            size_bytes: size,
            sha256: sha.into(),
        })),
    }
}

#[tokio::test]
async fn queue_full_before_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::new(tmp.path()).unwrap());

    let (queue_tx, queue_rx) = build_leaky_bucket(1);
    assert!(try_enqueue(&queue_tx, dummy_job()));
    assert_eq!(queue_tx.capacity(), 0);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = build_service(queue_tx.clone(), storage);
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    let channel = Channel::from_shared(format!("http://{addr}"))
        .unwrap().connect().await.unwrap();
    let mut client = MediaUploadClient::new(channel);

    // Only a header. If the server waited for video bytes, this would hang.
    let (tx, rx) = mpsc::channel::<UploadRequest>(1);
    tx.send(header_msg("test.mp4", &"a".repeat(64), 100)).await.unwrap();

    let body = client.upload(ReceiverStream::new(rx)).await
        .expect("transport ok").into_inner();

    assert_eq!(body.status, pb::Status::QueueFull as i32);
    assert!(!body.message.is_empty());

    drop(tx);
    drop(queue_rx);
    server.abort();
}

#[tokio::test]
async fn duplicate_before_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::new(tmp.path()).unwrap());
    let sha = "b".repeat(64);
    assert!(storage.try_reserve(&sha));

    let (queue_tx, _queue_rx) = build_leaky_bucket(8);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = build_service(queue_tx, storage);
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    let channel = Channel::from_shared(format!("http://{addr}"))
        .unwrap().connect().await.unwrap();
    let mut client = MediaUploadClient::new(channel);

    let (tx, rx) = mpsc::channel::<UploadRequest>(1);
    tx.send(header_msg("dup.mp4", &sha, 100)).await.unwrap();

    let body = client.upload(ReceiverStream::new(rx)).await
        .expect("transport ok").into_inner();

    assert_eq!(body.status, pb::Status::Duplicate as i32);
    assert!(body.message.contains(&sha[..16]));

    drop(tx);
    server.abort();
}

#[tokio::test]
async fn empty_stream_is_invalid_argument() {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::new(tmp.path()).unwrap());
    let (queue_tx, _queue_rx) = build_leaky_bucket(4);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = build_service(queue_tx, storage);
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    let channel = Channel::from_shared(format!("http://{addr}"))
        .unwrap().connect().await.unwrap();
    let mut client = MediaUploadClient::new(channel);

    let (tx, rx) = mpsc::channel::<UploadRequest>(1);
    drop(tx);

    let err = client.upload(ReceiverStream::new(rx)).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);

    server.abort();
}