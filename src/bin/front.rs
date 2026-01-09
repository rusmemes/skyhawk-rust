use axum::routing::{post, Router};
use skyhawk_rust::handlers::copy::copy;
use skyhawk_rust::handlers::log::log;
use skyhawk_rust::kafka_removal_reading::kafka_removal_reading;
use skyhawk_rust::runtime_store::RuntimeStore;
use skyhawk_rust::service_discovery::service_discovery;
use skyhawk_rust::utils::{join_tasks, shutdown_signal};
use skyhawk_rust::{Config, FrontState, ServiceList};
use std::sync::Arc;
use std::time::Duration;
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
        .with_graceful_shutdown(shutdown_signal(shutdown))
        .await
        .unwrap();

    let _ = timeout(Duration::from_secs(30), supervisor).await;
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

    join_tasks(token, handles).await
}
