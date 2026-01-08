use crate::runtime_store::RuntimeStore;
use rdkafka::producer::FutureProducer;
use rdkafka::ClientConfig;
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

impl FrontState {
    pub fn new() -> Self {
        let config = Arc::new(Config::new());

        let kafka_bootstrap_servers =
            std::env::var("KAFKA_BOOTSTRAP_SERVERS").expect("KAFKA_BOOTSTRAP_SERVERS must be set");

        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", kafka_bootstrap_servers)
            .create()
            .expect("Kafka producer creation error");

        FrontState {
            producer,
            config,
            runtime_store: Arc::new(RuntimeStore::new()),
        }
    }
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
