use crate::errors::AppError;
use futures::future::select_all;
use tokio::signal::unix::{signal, SignalKind};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub type Result<T> = std::result::Result<T, AppError>;

pub async fn shutdown_signal(token: CancellationToken) {
    let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler");

    tokio::select! {
        _ = sigterm.recv() => tracing::info!("SIGTERM received"),
        _ = sigint.recv() => tracing::info!("SIGINT received"),
        _ = token.cancelled() => {}, // shutdown caused by a panic
    }

    token.cancel();
}

pub async fn join_tasks(token: CancellationToken, mut handles: Vec<JoinHandle<Result<()>>>) {
    while !handles.is_empty() {
        let (res, _idx, remaining) = select_all(handles).await;
        handles = remaining;

        match res {
            Ok(_) => {}
            Err(e) if e.is_panic() => {
                // that is really unexpected as there are no places in the app where the code can panic intentionally
                tracing::error!("Background task panicked: {e}");
                token.cancel();
                break;
            }
            Err(e) => {
                tracing::error!("Background task aborted: {e}");
                token.cancel();
                break;
            }
        }
    }
}
