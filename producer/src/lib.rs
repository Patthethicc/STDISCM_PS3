//! Producer: uploads videos to the consumer over client-streaming gRPC.

/// Code generated from `proto/media_upload.proto`.
pub mod proto {
    tonic::include_proto!("media");
}

pub mod cli;
pub mod hasher;
pub mod report;
pub mod scanner;
pub mod uploader;
pub mod worker;
