mod common;

use common::dummy_job;
use consumer::queue::{build_leaky_bucket, try_enqueue};

#[tokio::test]
async fn leaky_bucket_drops_beyond_capacity() {
    let (tx, mut rx) = build_leaky_bucket(2);
    assert!(try_enqueue(&tx, dummy_job()), "slot 1");
    assert!(try_enqueue(&tx, dummy_job()), "slot 2");
    assert!(!try_enqueue(&tx, dummy_job()), "slot 3 must be dropped");
    let _ = rx.recv().await.unwrap();
    assert!(try_enqueue(&tx, dummy_job()), "slot reopened");
}

#[tokio::test]
async fn capacity_reports_remaining_slots() {
    let (tx, _rx) = build_leaky_bucket(3);
    assert_eq!(tx.capacity(), 3);
    assert!(try_enqueue(&tx, dummy_job()));
    assert_eq!(tx.capacity(), 2);
}

#[tokio::test]
async fn try_enqueue_fails_when_channel_closed() {
    let (tx, rx) = build_leaky_bucket(2);
    drop(rx);
    assert!(!try_enqueue(&tx, dummy_job()));
}