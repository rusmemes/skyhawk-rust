use axum::routing::{post, Router};
use futures::future::select_all;
use skyhawk_rust::handlers::copy::copy;
use skyhawk_rust::handlers::log::log;
use skyhawk_rust::kafka_removal_reading::kafka_removal_reading;
use skyhawk_rust::runtime_store::RuntimeStore;
use skyhawk_rust::service_discovery::service_discovery;
use skyhawk_rust::{Config, FrontState, ServiceList};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    let shutdown = CancellationToken::new();
    let front_state = FrontState::new();

    let supervisor = tokio::spawn(spawn_background_tasks(
        shutdown.clone(),
        front_state.runtime_store.clone(),
        front_state.config.clone(),
        front_state.service_list.clone(),
    ));

    let app = Router::new()
        .route("/log", post(log))
        .route("/stat-copy", post(copy))
        .with_state(front_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    tracing::info!("Listening on http://0.0.0.0:8080");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown.clone()))
        .await
        .unwrap();

    let _ = timeout(Duration::from_secs(30), supervisor).await;
}

async fn shutdown_signal(token: CancellationToken) {

    let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler");

    tokio::select! {
        _ = sigterm.recv() => tracing::info!("SIGTERM received"),
        _ = sigint.recv() => tracing::info!("SIGINT received"),
        _ = token.cancelled() => {}, // shutdown caused by a panic
    }

    token.cancel();
}

async fn spawn_background_tasks(
    token: CancellationToken,
    runtime_store: Arc<RuntimeStore>,
    config: Arc<Config>,
    service_list: Arc<ServiceList>,
) {
    let mut handles = Vec::new();

    handles.push(tokio::spawn(service_discovery(
        token.child_token(),
        config.clone(),
        service_list
    )));

    handles.push(tokio::spawn(kafka_removal_reading(
        token.child_token(),
        config,
        runtime_store,
    )));

    while !handles.is_empty() {
        let (res, _idx, remaining) = select_all(handles).await;
        handles = remaining;

        match res {
            Ok(_) => {}
            Err(e) if e.is_panic() => {
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
