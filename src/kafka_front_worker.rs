use crate::domain::CacheRecord;
use crate::runtime_store::RuntimeStore;
use crate::{utils::Result, Config, HEADER_SENDER};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{BorrowedMessage, Headers};
use rdkafka::{ClientConfig, Message};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub async fn kafka_front_worker(
    token: CancellationToken,
    config: Config,
    runtime_store: RuntimeStore,
) -> Result<()> {
    tracing::info!("Kafka worker started");

    let consumer: Arc<StreamConsumer> = Arc::new(
        ClientConfig::new()
            .set("bootstrap.servers", config.kafka_bootstrap_servers.as_str())
            .set("group.id", config.kafka_group_id.as_str())
            .set("auto.offset.reset", "latest")
            .set("enable.auto.commit", "true")
            .create()?,
    );

    consumer.subscribe(&[
        config.kafka_topic_main.as_str(),
        config.kafka_topic_removal.as_str(),
    ])?;

    tracing::info!("Consumer started");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                tracing::info!("Kafka worker is shutting down");
                break;
            }
            result = consumer.recv() => process_msg(result?, &runtime_store, &config)?
        }
    }

    Ok(())
}

fn process_msg(msg: BorrowedMessage, runtime_store: &RuntimeStore, config: &Config) -> Result<()> {
    if msg.topic() == config.kafka_topic_main.as_str() {
        let headers = match msg.headers() {
            Some(h) => h,
            None => return Ok(()),
        };

        let sender = headers
            .iter()
            .find(|h| h.key == HEADER_SENDER)
            .and_then(|h| h.value)
            .and_then(|v| std::str::from_utf8(v).ok());

        if sender.is_some() && sender != Some(config.instance_id.as_str()) {
            process(msg.payload(), |record| runtime_store.log(record))?;
        }
    } else if msg.topic() == config.kafka_topic_removal.as_str() {
        process(msg.payload(), |record| runtime_store.remove(&record))?;
    }

    Ok(())
}

fn process<F>(payload: Option<&[u8]>, ok_processor: F) -> Result<()>
where
    F: FnOnce(CacheRecord),
{
    if let Some(payload) = payload {
        ok_processor(serde_json::from_slice(payload)?);
    }

    Ok(())
}
