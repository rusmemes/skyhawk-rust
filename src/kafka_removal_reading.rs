use crate::runtime_store::RuntimeStore;
use crate::Config;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub async fn kafka_removal_reading(
    token: CancellationToken,
    config: Arc<Config>,
    runtime_store: Arc<RuntimeStore>,
) {
    tracing::info!("Kafka removal reading worker started");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                tracing::info!("Kafka removal reading worker shutting down");
                break;
            }
            _ = sleep(Duration::from_millis(1)) => {
                iteration().await;
            }
        }
    }
}

async fn iteration() {
    todo!()
}
