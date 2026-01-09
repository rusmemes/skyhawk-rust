use crate::runtime_store::RuntimeStore;
use axum::extract::FromRef;
use rdkafka::producer::FutureProducer;
use rdkafka::ClientConfig;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod handlers;
pub mod kafka_removal_reading;
pub mod protocol;
pub mod runtime_store;
pub mod service_discovery;
pub mod utils;

pub const HEADER_SENDER: &str = "sender";

pub struct ServiceList {
    pub list: RwLock<Vec<String>>,
}

impl ServiceList {
    pub fn new() -> Self {
        Self {
            list: RwLock::new(Vec::new()),
        }
    }
}

#[derive(Clone)]
pub struct FrontState {
    pub producer: FutureProducer,
    pub config: Arc<Config>,
    pub runtime_store: Arc<RuntimeStore>,
    pub service_list: Arc<ServiceList>,
}

impl FromRef<FrontState> for Arc<RuntimeStore> {
    fn from_ref(input: &FrontState) -> Self {
        input.runtime_store.clone()
    }
}

impl FromRef<FrontState> for (FutureProducer, Arc<Config>, Arc<RuntimeStore>) {
    fn from_ref(input: &FrontState) -> Self {
        (
            input.producer.clone(),
            input.config.clone(),
            input.runtime_store.clone(),
        )
    }
}

impl FrontState {
    pub fn new() -> Self {
        let config = Arc::new(Config::new());

        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &config.kafka_bootstrap_servers)
            .create()
            .expect("Kafka producer creation error");

        FrontState {
            producer,
            config,
            runtime_store: Arc::new(RuntimeStore::new()),
            service_list: Arc::new(ServiceList::new()),
        }
    }
}

pub struct Config {
    pub kafka_topic_main: String,
    pub kafka_topic_removal: String,
    pub kafka_group_id: String,
    pub kafka_bootstrap_servers: String,
    pub database_url: String,
    pub instance_id: String,
    pub service_discovery_self_url: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            kafka_topic_main: env::var("KAFKA_TOPIC_MAIN").expect("KAFKA_TOPIC_MAIN must be set"),
            kafka_topic_removal: env::var("KAFKA_TOPIC_REMOVAL")
                .expect("KAFKA_TOPIC_REMOVAL must be set"),
            kafka_group_id: get_kafka_group_id(),
            kafka_bootstrap_servers: env::var("KAFKA_BOOTSTRAP_SERVERS")
                .expect("KAFKA_BOOTSTRAP_SERVERS must be set"),
            instance_id: Uuid::new_v4().to_string(),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            service_discovery_self_url: get_service_discovery_url(),
        }
    }
}

fn get_kafka_group_id() -> String {
    let group_id = env::var("KAFKA_GROUP_ID").expect("KAFKA_GROUP_ID must be set");
    if group_id == "random" {
        Uuid::new_v4().to_string()
    } else {
        group_id
    }
}

fn get_service_discovery_url() -> Option<String> {
    let url = env::var("SERVICE_DISCOVERY_SELF_URL")
        .map(Some)
        .unwrap_or(None)?;

    Some(if url == "docker.host" {
        let docker_host = env::var("HOSTNAME")
            .expect("HOSTNAME must be set")
            .to_string();
        format!("http://{}:8080", docker_host)
    } else {
        url
    })
}
