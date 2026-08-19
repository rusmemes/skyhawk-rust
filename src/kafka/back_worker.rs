use crate::config::Config;
use crate::domain::CacheRecord;
use crate::kafka::topic_is_not_available;
use crate::shutdown::Result;
use crate::storage::postgres;
use futures::stream::{FuturesUnordered, StreamExt};
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::BorrowedMessage;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub async fn run(token: CancellationToken, config: Arc<Config>, pool: PgPool) -> Result<()> {
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

    let mut batch = Vec::with_capacity(MAX_BATCH_SIZE);

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                tracing::info!("Kafka removal reading worker shutting down");
                break;
            }
            res = collect_batch(&consumer, &mut batch) => {
                res?;
                iteration(&pool, &consumer, &producer, &config, &batch).await?;
                batch.clear();
            }
        }
    }

    Ok(())
}

const MAX_BATCH_SIZE: usize = 100;
const MAX_WAIT: Duration = Duration::from_millis(100);

async fn iteration(
    pool: &PgPool,
    consumer: &StreamConsumer,
    producer: &FutureProducer,
    config: &Config,
    batch: &[BorrowedMessage<'_>],
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

    let futures = FuturesUnordered::new();

    for (key, v) in map {
        futures.push(process_messages_of_the_same_key(
            pool, producer, config, key, v,
        ));
    }

    for result in futures.collect::<Vec<_>>().await {
        result?;
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
    postgres::store_records(pool, &v).await?;

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

async fn collect_batch<'a>(
    consumer: &'a StreamConsumer,
    batch: &mut Vec<BorrowedMessage<'a>>,
) -> Result<()> {
    match consumer.recv().await {
        Ok(message) => batch.push(message),
        Err(error) if topic_is_not_available(&error) => {
            tracing::warn!(%error, "Kafka topic is not available yet; retrying");
            tokio::time::sleep(Duration::from_secs(1)).await;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    }
    let deadline = tokio::time::Instant::now() + MAX_WAIT;

    while batch.len() < MAX_BATCH_SIZE {
        match tokio::time::timeout_at(deadline, consumer.recv()).await {
            Ok(Ok(message)) => batch.push(message),
            Ok(Err(error)) if topic_is_not_available(&error) => {
                tracing::warn!(%error, "Kafka topic is not available yet; processing collected messages");
                break;
            }
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => break,
        }
    }

    Ok(())
}
