use crate::runtime_store::RuntimeStore;
use rdkafka::producer::FutureProducer;
use std::sync::Arc;

pub mod protocol;
pub mod runtime_store;
pub mod handlers;

#[derive(Clone)]
pub struct FrontState {
    pub producer: FutureProducer,
    pub config: Arc<Config>,
    pub runtime_store: Arc<RuntimeStore>,
}

pub struct Config {
    pub kafka_topic_main: String,
    pub kafka_topic_removal: String,
    pub kafka_group_id: String,
    pub service_discovery_self_url: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            kafka_topic_main: std::env::var("KAFKA_TOPIC_MAIN").expect("KAFKA_TOPIC_MAIN must be set"),
            kafka_topic_removal: std::env::var("KAFKA_TOPIC_REMOVAL").expect("KAFKA_TOPIC_REMOVAL must be set"),
            kafka_group_id: std::env::var("KAFKA_GROUP_ID").expect("KAFKA_GROUP_ID must be set"),
            service_discovery_self_url: std::env::var("SERVICE_DISCOVERY_SELF_URL").map(Some).unwrap_or(None),
        }
    }
}
