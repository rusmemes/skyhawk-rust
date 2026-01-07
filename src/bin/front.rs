use axum::routing::{post, Router};
use rdkafka::config::ClientConfig;
use rdkafka::producer::FutureProducer;
use skyhawk_rust::handlers::log::log;
use skyhawk_rust::runtime_store::RuntimeStore;
use skyhawk_rust::{Config, FrontState};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv::dotenv().ok();
    let config = Arc::new(Config::new());

    let kafka_bootstrap_servers =
        std::env::var("KAFKA_BOOTSTRAP_SERVERS").expect("KAFKA_BOOTSTRAP_SERVERS must be set");

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", kafka_bootstrap_servers)
        .create()
        .expect("Kafka producer creation error");

    let app = Router::new()
        .route("/log", post(log))
        .with_state(FrontState {
            producer,
            config,
            runtime_store: Arc::new(RuntimeStore::new()),
        });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("Listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}
