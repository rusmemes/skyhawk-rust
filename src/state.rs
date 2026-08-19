use crate::config::Config;
use crate::error::AppError;
use crate::shutdown::Result;
use crate::storage::runtime::RuntimeStore;
use axum::extract::FromRef;
use rdkafka::ClientConfig;
use rdkafka::producer::FutureProducer;
use reqwest::Client;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

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

impl Default for ServiceList {
    fn default() -> Self {
        Self::new()
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

impl FrontState {
    pub fn new(pool: PgPool, config: Config) -> Result<Self> {
        Self::with_http_client(pool, config, Client::new())
    }

    pub fn with_http_client(pool: PgPool, config: Config, http: Client) -> Result<Self> {
        let producer = ClientConfig::new()
            .set("bootstrap.servers", &config.kafka_bootstrap_servers)
            .create()
            .map_err(|error| AppError::Custom(format!("Kafka producer creation error: {error}")))?;

        Ok(Self {
            producer,
            config: Arc::new(config),
            runtime_store: Arc::new(RuntimeStore::new()),
            service_list: Arc::new(ServiceList::new()),
            http,
            pool,
        })
    }
}

macro_rules! from_front_state {
    ($type:ty, $field:ident) => {
        impl FromRef<FrontState> for $type {
            fn from_ref(state: &FrontState) -> Self {
                state.$field.clone()
            }
        }
    };
}

from_front_state!(Arc<RuntimeStore>, runtime_store);
from_front_state!(Arc<ServiceList>, service_list);
from_front_state!(Client, http);
from_front_state!(PgPool, pool);
from_front_state!(FutureProducer, producer);
from_front_state!(Arc<Config>, config);

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    fn config() -> Config {
        Config {
            kafka_topic_main: "main".into(),
            kafka_topic_removal: "removal".into(),
            kafka_group_id: "front-test".into(),
            kafka_bootstrap_servers: "localhost:9092".into(),
            database_url: "postgres://localhost/test".into(),
            instance_id: "instance-1".into(),
            service_discovery_self_url: None,
        }
    }

    #[test]
    fn service_list_starts_empty() {
        let list = ServiceList::default();
        assert!(list.list.try_read().unwrap().is_empty());
    }

    #[tokio::test]
    async fn front_state_builds_and_exposes_axum_substates() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/test")
            .unwrap();
        let http = Client::builder().no_proxy().build().unwrap();
        let state = FrontState::with_http_client(pool, config(), http).unwrap();

        let config = <Arc<Config>>::from_ref(&state);
        let store = <Arc<RuntimeStore>>::from_ref(&state);
        let services = <Arc<ServiceList>>::from_ref(&state);

        assert_eq!(config.instance_id, "instance-1");
        assert!(store.view("unknown").is_empty());
        assert!(services.list.try_read().unwrap().is_empty());
    }
}
