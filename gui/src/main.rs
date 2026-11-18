use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Web GUI for the media upload service.
#[derive(Parser)]
struct Args {
    /// Consumer data folder (contains catalog.json, uploads/, previews/, thumbnails/).
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    /// Address to listen on. Use 0.0.0.0 so other machines can open it.
    #[arg(long, default_value = "0.0.0.0:8080")]
    bind: SocketAddr,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let app = gui::server::router(args.data_dir.clone());
    let listener = tokio::net::TcpListener::bind(args.bind).await.expect("failed to bind");
    println!("GUI on http://{} (data dir: {})", args.bind, args.data_dir.display());
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server error");
}
