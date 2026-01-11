use crate::domain::CacheRecord;
use crate::runtime_store::RuntimeStore;
use crate::{Config, HEADER_SENDER};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Headers;
use rdkafka::{ClientConfig, Message};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub async fn kafka_front_worker(
    token: CancellationToken,
    config: Config,
    runtime_store: RuntimeStore,
) {
    tracing::info!("Kafka removal reading worker started");

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
                tracing::info!("Kafka removal reading worker shutting down");
                break;
            }
            _ = sleep(Duration::from_millis(1)) => {
                iteration(consumer.clone(), runtime_store.clone(), config.clone()).await;
            }
        }
    }
}

async fn iteration(
    consumer: Arc<StreamConsumer>,
    runtime_store: RuntimeStore,
    config: Config,
) {
    loop {
        match consumer.recv().await {
            Err(e) => tracing::error!("Kafka error: {}", e),
            Ok(msg) => {
                if msg.topic() == config.kafka_topic_main.as_str() {
                    if let Some(headers) = msg.headers() {
                        if let Some(bytes) = headers
                            .iter()
                            .find(|header| header.key == HEADER_SENDER)
                            .and_then(|header| header.value)
                        {
                            let header_value = std::str::from_utf8(bytes)
                                .expect("Failed to parse kafka sender header value");

                            if header_value != config.instance_id.as_str() {
                                cache_record(runtime_store.clone(), msg.payload());
                            }
                        }
                    }
                } else if msg.topic() == config.kafka_topic_removal.as_str() {
                    clear_runtime_store(runtime_store.clone(), msg.payload());
                }
            }
        }
    }
}

fn cache_record(runtime_store: RuntimeStore, payload: Option<&[u8]>) {
    if let Some(payload) = payload {
        let record: CacheRecord =
            serde_json::from_slice(&payload).expect("Failed to parse kafka payload");

        runtime_store.log(record);
    }
}

fn clear_runtime_store(runtime_store: RuntimeStore, payload: Option<&[u8]>) {
    if let Some(payload) = payload {
        let record: CacheRecord =
            serde_json::from_slice(&payload).expect("Failed to parse kafka payload");

        runtime_store.remove(&record);
    }
}
