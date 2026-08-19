use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use skyhawk_rust::Config;
use skyhawk_rust::kafka::back_worker;
use skyhawk_rust::shutdown::{Result, join_tasks, shutdown_signal};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    let config = Arc::new(Config::new()?);
    let pool = PgPool::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let shutdown = CancellationToken::new();
    let worker_token = shutdown.child_token();
    let supervisor_token = shutdown.clone();
    let supervisor = tokio::spawn(async move {
        join_tasks(
            supervisor_token,
            vec![tokio::spawn(back_worker::run(worker_token, config, pool))],
        )
        .await;
    });

    let app = Router::new().route("/health", get(|| async { StatusCode::OK }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("Listening on http://0.0.0.0:8080");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown))
        .await?;

    let _ = timeout(Duration::from_secs(30), supervisor).await;
    Ok(())
}
