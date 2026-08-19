use crate::error::AppError;
use futures::future::select_all;
use tokio::signal::unix::{SignalKind, signal};
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
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::error!(%error, "Background task failed");
                token.cancel();
                break;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn successful_tasks_do_not_cancel_token() {
        let token = CancellationToken::new();
        join_tasks(token.clone(), vec![tokio::spawn(async { Ok(()) })]).await;
        assert!(!token.is_cancelled());
    }

    #[tokio::test]
    async fn failed_task_cancels_token() {
        let token = CancellationToken::new();
        join_tasks(
            token.clone(),
            vec![tokio::spawn(async {
                Err(AppError::Custom("failure".into()))
            })],
        )
        .await;
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn panicked_task_cancels_token() {
        let token = CancellationToken::new();
        join_tasks(
            token.clone(),
            vec![tokio::spawn(async { panic!("failure") })],
        )
        .await;
        assert!(token.is_cancelled());
    }
}
