use crate::Config;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub async fn service_discovery(token: CancellationToken, config: Arc<Config>) {
    tracing::info!("Service discovery worker started");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                tracing::info!("Service discovery worker shutting down");
                break;
            }
            _ = sleep(Duration::from_secs(1)) => {
                iteration().await;
            }
        }
    }
}

async fn iteration() {
    todo!()
}
