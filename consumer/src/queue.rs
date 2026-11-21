use tokio::sync::mpsc;
use crate::job::Job;

pub type JobSender = mpsc::Sender<Job>;
pub type JobReceiver = mpsc::Receiver<Job>;

/// Builds a bounded MPSC channel. `q` is the leaky-bucket capacity.
pub fn build_leaky_bucket(q: usize) -> (JobSender, JobReceiver) {
    mpsc::channel(q)
}

/// Non-blocking enqueue. Returns `false` when the bucket is full, meaning
/// the job must be dropped immediately.
pub fn try_enqueue(tx: &JobSender, job: Job) -> bool {
    match tx.try_send(job) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            tracing::warn!("Queue full — dropping upload");
            false // drop the job and its temp resources
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            tracing::error!("Queue closed");
            false
        }
    }
}