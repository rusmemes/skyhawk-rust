use crate::domain::CacheRecord;
use crate::runtime_store::RuntimeStore;
use crate::{Config, HEADER_SENDER};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{BorrowedMessage, Headers};
use rdkafka::{ClientConfig, Message};
use serde_json::Error;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub async fn kafka_front_worker(
    token: CancellationToken,
    config: Config,
    runtime_store: RuntimeStore,
) {
    tracing::info!("Kafka worker started");

    let consumer: Arc<StreamConsumer> = Arc::new(
        ClientConfig::new()
            .set("bootstrap.servers", config.kafka_bootstrap_servers.as_str())
            .set("group.id", config.kafka_group_id.as_str())
            .set("auto.offset.reset", "latest")
            .set("enable.auto.commit", "true")
            .create()
            .expect("consumer creation failed"),
    );

    consumer
        .subscribe(&[&config.kafka_topic_main, &config.kafka_topic_removal])
        .expect("can't subscribe to kafka topics");

    tracing::info!("Consumer started");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                tracing::info!("Kafka worker is shutting down");
                break;
            }
            result = consumer.recv() => {
                match result {
                    Err(e) => tracing::error!("Kafka error: {}", e),
                    Ok(msg) => process_msg(msg, &runtime_store, &config)
                }
            }
        }
    }
}

fn process_msg(msg: BorrowedMessage, runtime_store: &RuntimeStore, config: &Config) {
    if msg.topic() == config.kafka_topic_main.as_str() {
        let headers = match msg.headers() {
            Some(h) => h,
            None => return,
        };

        let sender = headers
            .iter()
            .find(|h| h.key == HEADER_SENDER)
            .and_then(|h| h.value)
            .and_then(|v| std::str::from_utf8(v).ok());

        if sender != Some(config.instance_id.as_str()) {
            process(msg.payload(), |record| runtime_store.log(record));
        }
    } else if msg.topic() == config.kafka_topic_removal.as_str() {
        process(msg.payload(), |record| runtime_store.remove(&record));
    }
}

fn process<F>(payload: Option<&[u8]>, ok_processor: F)
where
    F: FnOnce(CacheRecord),
{
    if let Some(payload) = payload {
        let record: Result<CacheRecord, Error> = serde_json::from_slice(payload);

        match record {
            Ok(record) => ok_processor(record),
            Err(e) => {
                tracing::error!(error=%e, "Error deserializing cached record");
            }
        }
    }
}
