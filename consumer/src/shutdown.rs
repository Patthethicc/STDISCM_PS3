use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Shutdown {
    token: CancellationToken,
}

impl Shutdown {
    pub fn new() -> Self {
        Self { token: CancellationToken::new() }
    }

    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Waits for Ctrl+C or SIGTERM and then cancels all clones.
    pub async fn wait_for_signal(&self) {
        let ctrl_c = async {
            tokio::signal::ctrl_c().await.expect("ctrl_c handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        tracing::info!("Shutdown signal received — draining workers");
        self.token.cancel();
    }
}