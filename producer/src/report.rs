//! Per-file result logging (stdout).

use crate::proto::Status;
use crate::uploader::Outcome;

/// Log label for a consumer status. ACCEPTED means queued, not saved.
pub fn status_label(status: Status) -> &'static str {
    match status {
        Status::Accepted => "ACCEPTED (queued, not yet saved)",
        Status::QueueFull => "QUEUE_FULL",
        Status::Duplicate => "DUPLICATE",
        Status::Error => "ERROR",
        Status::Unspecified => "ERROR (unspecified status)",
    }
}

pub fn log(thread_id: usize, subject: &str, label: &str, detail: &str) {
    println!("[producer-{thread_id}] {subject}: {label} {detail}");
}

pub fn log_outcome(thread_id: usize, file: &str, outcome: &Outcome) {
    log(
        thread_id,
        file,
        status_label(outcome.status),
        &outcome.message,
    );
}

pub fn log_error(thread_id: usize, subject: &str, detail: &str) {
    log(thread_id, subject, "ERROR", detail);
}
