use axum::routing::{get, Router};
use sqlx::PgPool;
use std::sync::Arc;
use tiny_kafka::KafkaProducer;

#[derive(Clone)]
struct FrontState {
    pool: PgPool,
    producer: Arc<KafkaProducer>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Error connecting to database");

    let kafka_bootstrap_servers =
        std::env::var("KAFKA_BOOTSTRAP_SERVERS").expect("KAFKA_BOOTSTRAP_SERVERS must be set");
    let producer = Arc::new(
        KafkaProducer::new(kafka_bootstrap_servers)
            .await
            .expect("Error creating Kafka producer"),
    );

    let app = Router::new()
        .route("/", get(root))
        .with_state(FrontState { pool, producer });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("Listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> String {
    let mut env = String::new();
    env.push_str(
        format!(
            "DATABASE_URL={}\n",
            std::env::var("DATABASE_URL").unwrap_or_default()
        )
        .as_str(),
    );
    env.push_str(
        format!(
            "KAFKA_TOPIC_MAIN={}\n",
            std::env::var("KAFKA_TOPIC_MAIN").unwrap_or_default()
        )
        .as_str(),
    );
    env.push_str(
        format!(
            "KAFKA_TOPIC_REMOVAL={}\n",
            std::env::var("KAFKA_TOPIC_REMOVAL").unwrap_or_default()
        )
        .as_str(),
    );
    env.push_str(
        format!(
            "KAFKA_GROUP_ID={}\n",
            std::env::var("KAFKA_GROUP_ID").unwrap_or_default()
        )
        .as_str(),
    );
    env.push_str(
        format!(
            "KAFKA_BOOTSTRAP_SERVERS={}\n",
            std::env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_default()
        )
        .as_str(),
    );
    env.push_str(
        format!(
            "SERVICE_DISCOVERY_SELF_URL={}\n",
            std::env::var("SERVICE_DISCOVERY_SELF_URL").unwrap_or_default()
        )
        .as_str(),
    );
    env.push_str(
        format!(
            "SERVICE_DISCOVERY_HEARTBEAT_ENABLED={}\n",
            std::env::var("SERVICE_DISCOVERY_HEARTBEAT_ENABLED").unwrap_or_default()
        )
        .as_str(),
    );
    env
}
