use crate::domain::CacheRecord;
use crate::kafka::topic_is_not_available;
use crate::shutdown::Result;
use crate::storage::runtime::RuntimeStore;
use crate::{Config, HEADER_SENDER};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{BorrowedMessage, Headers};
use rdkafka::{ClientConfig, Message};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub async fn run(
    token: CancellationToken,
    config: Arc<Config>,
    runtime_store: Arc<RuntimeStore>,
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
            result = consumer.recv() => match result {
                Ok(message) => process_msg(message, &runtime_store, &config)?,
                Err(error) if topic_is_not_available(&error) => {
                    tracing::warn!(%error, "Kafka topics are not available yet; retrying");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(error) => return Err(error.into()),
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Log, TimeKey};
    use std::cell::RefCell;

    fn record() -> CacheRecord {
        CacheRecord {
            time_key: TimeKey(1, 2),
            log: Log {
                season: "S".into(),
                team: "T".into(),
                player: "P".into(),
                points: Some(9),
                rebounds: None,
                assists: None,
                steals: None,
                blocks: None,
                fouls: None,
                turnovers: None,
                minutes_played: None,
            },
        }
    }

    #[test]
    fn process_deserializes_payload_and_calls_processor() {
        let json = serde_json::to_vec(&record()).unwrap();
        let captured = RefCell::new(None);

        process(Some(&json), |record| {
            captured.replace(Some(record));
        })
        .unwrap();

        assert_eq!(captured.borrow().as_ref().unwrap().log.points, Some(9));
    }

    #[test]
    fn process_ignores_missing_payload() {
        process(None, |_| panic!("processor must not be called")).unwrap();
    }

    #[test]
    fn process_rejects_invalid_json() {
        let result = process(Some(b"not-json"), |_| {});
        assert!(result.is_err());
    }
}
