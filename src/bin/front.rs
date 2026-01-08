use axum::routing::{post, Router};
use skyhawk_rust::handlers::log::log;
use skyhawk_rust::FrontState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    let app = Router::new()
        .route("/log", post(log))
        .with_state(FrontState::new());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("Listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}
