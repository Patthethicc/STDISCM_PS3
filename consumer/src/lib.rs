pub mod cli;
pub mod job;
pub mod pb; // <-- Important: Move your pb module here (see note below)
pub mod queue;
pub mod server;
pub mod shutdown;
pub mod storage;
pub mod worker;

use clap::Parser;
use std::sync::Arc;
use tokio_util::task::TaskTracker;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = cli::Cli::parse();

    // Storage: creates <data_dir>/{uploads,previews,thumbnails}.
    let storage = Arc::new(storage::Storage::new(&cli.data_dir)?);

    // Catalog: external crate, persists to <data_dir>/catalog.json.
    let catalog = Arc::new(catalog::Catalog::open(&cli.data_dir)?);

    // Seed the in-flight SHA-256 reservation set from the persisted catalog,
    // so restarts keep rejecting duplicates.
    storage.seed_reserved(catalog.list().into_iter().map(|e| e.sha256));

    // Leaky bucket: capacity = q.
    let (queue_tx, queue_rx) = queue::build_leaky_bucket(cli.queue_capacity);

    // Shutdown broadcast.
    let shutdown = shutdown::Shutdown::new();
    let token = shutdown.token();

    // Spawn c workers.
    let tracker = TaskTracker::new();
    worker::spawn_workers(
        queue_rx,
        storage.clone(),
        catalog.clone(),
        token.clone(),
        cli.concurrency,
        &tracker,
    );

    // gRPC server.
    let addr = cli.bind.parse()?;
    let service = server::build_service(queue_tx, storage.clone());

    let server = tonic::transport::Server::builder()
        .add_service(service)
        .serve_with_shutdown(addr, async move {
            shutdown.wait_for_signal().await;
        });

    tokio::select! {
        r = server => { r?; }
        _ = token.cancelled() => {}
    }

    // Graceful drain: stop accepting work, give in-flight jobs 30s.
    tracker.close();
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tracker.wait(),
    ).await;

    tracing::info!("shutdown complete");
    Ok(())
}