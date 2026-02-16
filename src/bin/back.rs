use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::BorrowedMessage;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use skyhawk_rust::domain::CacheRecord;
use skyhawk_rust::utils::{join_tasks, shutdown_signal, Result};
use skyhawk_rust::Config;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    let shutdown = CancellationToken::new();
    let supervisor = tokio::spawn(spawn_background_tasks(shutdown.clone()));

    let app = Router::new().route("/health", get(|| async { StatusCode::OK }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("Listening on http://0.0.0.0:8080");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown))
        .await?;

    let _ = timeout(Duration::from_secs(30), supervisor).await;

    Ok(())
}

async fn spawn_background_tasks(token: CancellationToken) {
    let child_token = token.child_token();
    join_tasks(token, vec![tokio::spawn(run_kafka_worker(child_token))]).await
}

async fn run_kafka_worker(token: CancellationToken) -> Result<()> {
    let config = Arc::new(Config::new()?);

    let pool = PgPool::connect(&config.database_url).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", config.kafka_bootstrap_servers.as_str())
        .create()?;

    let consumer: Arc<StreamConsumer> = Arc::new(
        ClientConfig::new()
            .set("bootstrap.servers", config.kafka_bootstrap_servers.as_str())
            .set("group.id", config.kafka_group_id.as_str())
            .set("auto.offset.reset", "earliest")
            .set("enable.auto.commit", "false")
            .create()?,
    );

    consumer.subscribe(&[&config.kafka_topic_main])?;

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                tracing::info!("Kafka removal reading worker shutting down");
                break;
            }
            batch = collect_batch(&consumer) => {
                iteration(&pool, &consumer, &producer, &config, batch?).await?;
            }
        }
    }

    Ok(())
}

async fn iteration(
    pool: &PgPool,
    consumer: &StreamConsumer,
    producer: &FutureProducer,
    config: &Config,
    batch: Vec<BorrowedMessage<'_>>,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    let map = batch
        .iter()
        .filter_map(|msg| Some((msg.key()?, msg.payload()?)))
        .fold(HashMap::new(), |mut map, item| {
            map.entry(item.0).or_insert_with(Vec::new).push(item.1);
            map
        });

    for (key, v) in map {
        let _ = process_messages_of_the_same_key(pool, producer, &config, key, v).await?;
    }

    if let Some(last) = batch.last() {
        consumer.commit_message(last, CommitMode::Async)?;
    }

    Ok(())
}

async fn process_messages_of_the_same_key(
    pool: &PgPool,
    producer: &FutureProducer,
    config: &Config,
    key: &[u8],
    v: Vec<&[u8]>,
) -> Result<()> {
    if v.is_empty() {
        return Ok(());
    }

    let mut v = v
        .into_iter()
        .filter_map(|bytes| serde_json::from_slice::<CacheRecord>(bytes).ok())
        .collect::<Vec<_>>();

    v.sort_by_key(|rec| rec.time_key);
    insert(&v, &pool).await?;

    if let Some(last_record) = v.last() {
        let json = serde_json::to_string(&last_record)?;

        producer
            .send(
                FutureRecord::to(&config.kafka_topic_removal)
                    .key(key)
                    .payload(&json),
                Duration::from_secs(5),
            )
            .await
            .map_err(|(error, _)| error)?;
    }

    Ok(())
}

async fn insert(records: &Vec<CacheRecord>, pool: &PgPool) -> Result<()> {
    let mut tx = pool.begin().await?;

    for rec in records {
        sqlx::query_as!(
            Self,
            r#"
            INSERT INTO nba_stats (
                t1, t2, season, team, player,
                points, rebounds, assists, steals,
                blocks, fouls, turnovers, minutes_played
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            ON CONFLICT (season, player, team, t1, t2)
            DO UPDATE SET
                points = EXCLUDED.points,
                rebounds = EXCLUDED.rebounds,
                assists = EXCLUDED.assists,
                steals = EXCLUDED.steals,
                blocks = EXCLUDED.blocks,
                fouls = EXCLUDED.fouls,
                turnovers = EXCLUDED.turnovers,
                minutes_played = EXCLUDED.minutes_played
            "#,
            rec.time_key.0,
            rec.time_key.1,
            &rec.log.season,
            &rec.log.team,
            &rec.log.player,
            rec.log.points,
            rec.log.rebounds,
            rec.log.assists,
            rec.log.steals,
            rec.log.blocks,
            rec.log.fouls,
            rec.log.turnovers,
            rec.log.minutes_played
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

async fn collect_batch(consumer: &StreamConsumer) -> Result<Vec<BorrowedMessage<'_>>> {
    const MAX_BATCH_SIZE: usize = 100;
    const MAX_WAIT: Duration = Duration::from_millis(100);

    let mut batch = Vec::with_capacity(MAX_BATCH_SIZE);
    let start = tokio::time::Instant::now();

    while batch.len() < MAX_BATCH_SIZE {
        let msg = consumer.recv().await?;
        batch.push(msg);
        if batch.len() >= MAX_BATCH_SIZE || start.elapsed() >= MAX_WAIT {
            break;
        }
    }

    Ok(batch)
}
