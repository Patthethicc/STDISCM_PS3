use consumer::job::Job;
use tokio::sync::{mpsc, oneshot};
use tonic::Status;

pub fn dummy_job() -> Job {
    let (_tx, rx) = mpsc::channel::<Result<Vec<u8>, Status>>(1);
    let (respond, _rrx) = oneshot::channel();
    Job {
        filename: "dummy.mp4".into(),
        sha256: "0".repeat(64),
        expected_size: 0,
        chunks: rx,
        respond,
    }
}