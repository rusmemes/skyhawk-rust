use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::BorrowedMessage;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use skyhawk_rust::protocol::CacheRecord;
use skyhawk_rust::utils::{join_tasks, shutdown_signal};
use skyhawk_rust::Config;
use sqlx::{Executor, PgPool};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tokio_util::sync::CancellationToken;

const DDL: &str = r#"
    create table if not exists nba_stats
    (
        t1             bigint not null,
        t2             bigint not null,
        season         text   not null,
        team           text   not null,
        player         text   not null,
        points         integer,
        rebounds       integer,
        assists        integer,
        steals         integer,
        blocks         integer,
        fouls          integer,
        turnovers      integer,
        minutes_played decimal(10, 4)
    );
    create unique index if not exists nba_stats_unique_idx ON nba_stats (season,player,team,t1,t2);
    create index if not exists nba_stats_agg_idx ON nba_stats (season,player,team);
"#;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    let shutdown = CancellationToken::new();
    let supervisor = tokio::spawn(spawn_background_tasks(shutdown.clone()));

    let app = Router::new().route("/health", get(|| async { StatusCode::OK }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("Listening on http://0.0.0.0:8080");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown))
        .await
        .unwrap();

    let _ = timeout(Duration::from_secs(30), supervisor).await;
}

async fn spawn_background_tasks(token: CancellationToken) {
    let child_token = token.child_token();
    join_tasks(token, vec![tokio::spawn(run_kafka_worker(child_token))]).await
}

async fn run_kafka_worker(token: CancellationToken) {
    let config = Arc::new(Config::new());

    let pool = PgPool::connect(&config.database_url)
        .await
        .expect("Error connecting to database");

    run_ddl(&pool).await;

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", config.kafka_bootstrap_servers.as_str())
        .create()
        .expect("Kafka producer creation error");

    let consumer: Arc<StreamConsumer> = Arc::new(
        ClientConfig::new()
            .set("bootstrap.servers", config.kafka_bootstrap_servers.as_str())
            .set("group.id", config.kafka_group_id.as_str())
            .set("auto.offset.reset", "earliest")
            .set("enable.auto.commit", "false")
            .create()
            .expect("consumer creation failed"),
    );

    consumer
        .subscribe(&[&config.kafka_topic_main])
        .expect("can't subscribe to main topic");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                tracing::info!("Kafka removal reading worker shutting down");
                break;
            }
            _ = sleep(Duration::from_millis(1)) => {
                iteration(pool.clone(), consumer.clone(), producer.clone(), config.clone()).await;
            }
        }
    }
}

async fn run_ddl(pool: &PgPool) {
    pool.execute(DDL)
        .await
        .expect("Error executing database table for nba stats");
}

async fn iteration(
    pool: PgPool,
    consumer: Arc<StreamConsumer>,
    producer: FutureProducer,
    config: Arc<Config>,
) {
    let batch = collect_batch(&consumer).await;
    if batch.is_empty() {
        return;
    }

    let map = batch
        .iter()
        .map(|msg| {
            let bytes = msg.payload().expect("Error getting payload");
            (msg.key().expect("Failed to get record key"), bytes)
        })
        .fold(HashMap::new(), |mut map, item| {
            map.entry(item.0).or_insert_with(Vec::new).push(item.1);
            map
        });

    for (key, v) in map {
        process_messages_of_the_same_key(&pool, producer.clone(), &config, key, v).await;
    }

    consumer
        .commit_message(
            batch.last().expect("Error committing batch"),
            CommitMode::Async,
        )
        .expect("Error committing batch");
}

async fn process_messages_of_the_same_key(
    pool: &PgPool,
    producer: FutureProducer,
    config: &Arc<Config>,
    key: &[u8],
    v: Vec<&[u8]>,
) {
    if v.is_empty() {
        return;
    }

    let mut v = v
        .into_iter()
        .map(|bytes| {
            serde_json::from_slice::<CacheRecord>(bytes).expect("Error deserializing payload")
        })
        .collect::<Vec<_>>();

    v.sort_by_key(|rec| rec.time_key);
    insert(&v, &pool).await;

    let last_record = v.last().expect("Error getting last entry");
    let json = serde_json::to_string(&last_record).expect("Error serializing last record");

    producer
        .send(
            FutureRecord::to(&config.kafka_topic_removal)
                .key(key)
                .payload(&json),
            Duration::from_secs(5),
        )
        .await
        .expect("Error sending message");
}

async fn insert(records: &Vec<CacheRecord>, pool: &PgPool) {
    let mut tx = pool.begin().await.unwrap();

    for rec in records {
        sqlx::query(
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
        )
        .bind(rec.time_key.0)
        .bind(rec.time_key.1)
        .bind(&rec.log.season)
        .bind(&rec.log.team)
        .bind(&rec.log.player)
        .bind(rec.log.points)
        .bind(rec.log.rebounds)
        .bind(rec.log.assists)
        .bind(rec.log.steals)
        .bind(rec.log.blocks)
        .bind(rec.log.fouls)
        .bind(rec.log.turnovers)
        .bind(rec.log.minutes_played)
        .execute(&mut *tx)
        .await
        .expect("Error executing insertion");
    }

    tx.commit().await.expect("Error committing batch");
}

async fn collect_batch(consumer: &StreamConsumer) -> Vec<BorrowedMessage<'_>> {
    const MAX_BATCH_SIZE: usize = 100;
    const MAX_WAIT: Duration = Duration::from_millis(100);

    let mut batch = Vec::with_capacity(MAX_BATCH_SIZE);
    let start = tokio::time::Instant::now();

    while batch.len() < MAX_BATCH_SIZE {
        match consumer.recv().await {
            Ok(msg) => {
                batch.push(msg);
                if batch.len() >= MAX_BATCH_SIZE {
                    break;
                }
            }
            Err(e) => {
                tracing::warn!("Ошибка при recv: {}", e);
                break;
            }
        }

        if start.elapsed() >= MAX_WAIT {
            break;
        }
    }

    batch
}
