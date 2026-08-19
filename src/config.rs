use crate::error::AppError;
use crate::shutdown::Result;
use std::env;
use uuid::Uuid;

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
            kafka_topic_main: required_env("KAFKA_TOPIC_MAIN")?,
            kafka_topic_removal: required_env("KAFKA_TOPIC_REMOVAL")?,
            kafka_group_id: kafka_group_id()?,
            kafka_bootstrap_servers: required_env("KAFKA_BOOTSTRAP_SERVERS")?,
            database_url: required_env("DATABASE_URL")?,
            instance_id: Uuid::new_v4().to_string(),
            service_discovery_self_url: service_discovery_url()?,
        })
    }
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).map_err(|_| AppError::Custom(format!("{name} must be set")))
}

fn kafka_group_id() -> Result<String> {
    let group_id = required_env("KAFKA_GROUP_ID")?;
    Ok(if group_id == "random" {
        Uuid::new_v4().to_string()
    } else {
        group_id
    })
}

fn service_discovery_url() -> Result<Option<String>> {
    let Some(url) = env::var("SERVICE_DISCOVERY_SELF_URL").ok() else {
        return Ok(None);
    };

    if url == "docker.host" {
        let hostname = required_env("HOSTNAME")?;
        Ok(Some(format!("http://{hostname}:8080")))
    } else {
        Ok(Some(url))
    }
}
