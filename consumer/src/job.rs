 // <-- Use the external crate, not crate::catalog
use tokio::sync::oneshot;

pub struct Job {
    pub filename: String,
    pub sha256: String,
    pub expected_size: u64,
    pub chunks: tokio::sync::mpsc::Receiver<Result<Vec<u8>, tonic::Status>>,
    // Only declare this ONCE. Returning the String ID is enough for the server.
    pub respond: oneshot::Sender<Result<String, String>>,
}