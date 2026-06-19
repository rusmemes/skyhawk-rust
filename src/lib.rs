use crate::errors::AppError;
use crate::runtime_store::RuntimeStore;
use crate::utils::Result;
use axum::extract::FromRef;
use rdkafka::producer::FutureProducer;
use rdkafka::ClientConfig;
use reqwest::Client;
use sqlx::PgPool;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod domain;
pub mod handlers;
pub mod kafka_front_worker;
pub mod runtime_store;
pub mod service_discovery;
pub mod utils;
pub mod errors;

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
    pub http: Client,
    pub pool: PgPool,
}

impl FromRef<FrontState> for Arc<RuntimeStore> {
    fn from_ref(input: &FrontState) -> Self {
        input.runtime_store.clone()
    }
}

impl FromRef<FrontState> for Arc<ServiceList> {
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

impl FromRef<FrontState> for Arc<Config> {
    fn from_ref(input: &FrontState) -> Self {
        input.config.clone()
    }
}

impl FrontState {
    pub fn new(pool: PgPool, config: Config) -> Result<Self> {
        let producer = ClientConfig::new()
            .set("bootstrap.servers", config.kafka_bootstrap_servers.clone())
            .create();

        let Ok(producer) = producer else {
            return Err(AppError::Custom("Kafka producer creation error".into()));
        };

        Ok(FrontState {
            producer,
            config: Arc::new(config),
            runtime_store: Arc::new(RuntimeStore::new()),
            service_list: Arc::new(ServiceList::new()),
            http: Client::new(),
            pool,
        })
    }
}

#[derive(Clone)]
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
    pub fn new() -> Result<Self> {
        Ok(Self {
            kafka_topic_main: 
                env::var("KAFKA_TOPIC_MAIN")
                    .map_err(|_| AppError::Custom("KAFKA_TOPIC_MAIN not found".into()))?,
            kafka_topic_removal: 
                env::var("KAFKA_TOPIC_REMOVAL")
                    .map_err(|_| AppError::Custom("KAFKA_TOPIC_REMOVAL must be set".into()))?,
            kafka_group_id: get_kafka_group_id()?,
            kafka_bootstrap_servers: 
                env::var("KAFKA_BOOTSTRAP_SERVERS")
                    .map_err(|_| AppError::Custom("KAFKA_BOOTSTRAP_SERVERS must be set".into()))?,
            instance_id: Uuid::new_v4().to_string(),
            database_url: 
                env::var("DATABASE_URL")
                    .map_err(|_| AppError::Custom("DATABASE_URL must be set".into()))?,
            service_discovery_self_url: get_service_discovery_url()?
        })
    }
}

fn get_kafka_group_id() -> Result<String> {
    let group_id = env::var("KAFKA_GROUP_ID")
        .map_err(|_| AppError::Custom("KAFKA_GROUP_ID not set".into()))?;
    Ok(if group_id == "random" {
        Uuid::new_v4().to_string()
    } else {
        group_id
    })
}

fn get_service_discovery_url() -> Result<Option<String>> {
    let url = env::var("SERVICE_DISCOVERY_SELF_URL")
        .map(Some)
        .unwrap_or(None);

    let Some(url) = url else {
        return Ok(None);
    };

    Ok(Some(if url == "docker.host" {
        let docker_host = env::var("HOSTNAME")
            .map_err(|_| AppError::Custom("HOSTNAME not set".into()))?
            .to_string();
        format!("http://{}:8080", docker_host)
    } else {
        url
    }))
}
