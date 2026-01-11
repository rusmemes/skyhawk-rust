use crate::runtime_store::RuntimeStore;
use axum::extract::FromRef;
use rdkafka::producer::FutureProducer;
use rdkafka::ClientConfig;
use reqwest::Client;
use sqlx::PgPool;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod handlers;
pub mod kafka_front_worker;
pub mod domain;
pub mod runtime_store;
pub mod service_discovery;
pub mod utils;

pub const HEADER_SENDER: &str = "sender";

#[derive(Clone)]
pub struct ServiceList {
    pub list: Arc<RwLock<Vec<String>>>,
}

impl ServiceList {
    pub fn new() -> Self {
        Self {
            list: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[derive(Clone)]
pub struct FrontState {
    pub producer: FutureProducer,
    pub config: Config,
    pub runtime_store: RuntimeStore,
    pub service_list: ServiceList,
    pub http: Client,
    pub pool: PgPool,
}

impl FromRef<FrontState> for RuntimeStore {
    fn from_ref(input: &FrontState) -> Self {
        input.runtime_store.clone()
    }
}

impl FromRef<FrontState> for ServiceList {
    fn from_ref(input: &FrontState) -> Self {
        input.service_list.clone()
    }
}

impl FromRef<FrontState> for Client {
    fn from_ref(input: &FrontState) -> Self {
        input.http.clone()
    }
}

impl FromRef<FrontState> for PgPool {
    fn from_ref(input: &FrontState) -> Self {
        input.pool.clone()
    }
}

impl FromRef<FrontState> for FutureProducer {
    fn from_ref(input: &FrontState) -> Self {
        input.producer.clone()
    }
}

impl FromRef<FrontState> for Config {
    fn from_ref(input: &FrontState) -> Self {
        input.config.clone()
    }
}

impl FrontState {
    pub fn new(pool: PgPool, config: Config) -> Self {

        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", config.kafka_bootstrap_servers.as_ref())
            .create()
            .expect("Kafka producer creation error");

        FrontState {
            producer,
            config,
            runtime_store: RuntimeStore::new(),
            service_list: ServiceList::new(),
            http: Client::new(),
            pool,
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub kafka_topic_main: Arc<String>,
    pub kafka_topic_removal: Arc<String>,
    pub kafka_group_id: Arc<String>,
    pub kafka_bootstrap_servers: Arc<String>,
    pub database_url: Arc<String>,
    pub instance_id: Arc<String>,
    pub service_discovery_self_url: Arc<Option<String>>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            kafka_topic_main: Arc::new(env::var("KAFKA_TOPIC_MAIN").expect("KAFKA_TOPIC_MAIN must be set")),
            kafka_topic_removal: Arc::new(env::var("KAFKA_TOPIC_REMOVAL")
                .expect("KAFKA_TOPIC_REMOVAL must be set")),
            kafka_group_id: Arc::new(get_kafka_group_id()),
            kafka_bootstrap_servers: Arc::new(env::var("KAFKA_BOOTSTRAP_SERVERS")
                .expect("KAFKA_BOOTSTRAP_SERVERS must be set")),
            instance_id: Arc::new(Uuid::new_v4().to_string()),
            database_url: Arc::new(env::var("DATABASE_URL").expect("DATABASE_URL must be set")),
            service_discovery_self_url: Arc::new(get_service_discovery_url()),
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
