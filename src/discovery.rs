use crate::shutdown::Result;
use crate::{Config, ServiceList};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub async fn service_discovery(
    token: CancellationToken,
    config: Arc<Config>,
    service_list: Arc<ServiceList>,
    pool: PgPool,
) -> Result<()> {
    tracing::info!("Service discovery worker started");

    let self_url = config.service_discovery_self_url.as_deref();

    let Some(self_url) = self_url else {
        tracing::info!("Service discovery disabled: no self URL configured");
        return Ok(());
    };

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                remove_expired_records(&pool, self_url).await?;
                tracing::info!("Service discovery worker shutting down");
                break;
            }
            _ = sleep(Duration::from_secs(1)) => {
                sync(&pool, self_url, &service_list).await?;
            }
        }
    }

    Ok(())
}

async fn remove_expired_records(pool: &PgPool, self_url: &str) -> Result<()> {
    const DURATION: Duration = Duration::from_secs(5);

    let cutoff_time = OffsetDateTime::now_utc() - DURATION;
    sqlx::query!(
        "DELETE FROM service_discovery WHERE url = $1 OR last_heartbeat_time < $2",
        self_url,
        cutoff_time
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn sync(pool: &PgPool, self_url: &str, service_list: &ServiceList) -> Result<()> {
    heartbeat(pool, self_url).await?;
    let urls = get_active_urls(pool, self_url).await?;
    work_on_state(urls, service_list).await;
    Ok(())
}

async fn heartbeat(pool: &PgPool, self_url: &str) -> Result<()> {
    let now = OffsetDateTime::now_utc();

    sqlx::query_as!(
        Self,
        r#"
        INSERT INTO service_discovery (url, last_heartbeat_time)
        VALUES ($1, $2)
        ON CONFLICT (url) DO UPDATE
          SET last_heartbeat_time = EXCLUDED.last_heartbeat_time
        "#,
        self_url,
        now
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn get_active_urls(pool: &PgPool, self_url: &str) -> Result<Vec<String>> {
    const DURATION: Duration = Duration::from_secs(5);
    let cutoff_time = OffsetDateTime::now_utc() - DURATION;

    let rows = sqlx::query!(
        r#"
        SELECT url
        FROM service_discovery
        WHERE url != $1
          AND last_heartbeat_time > $2
        ORDER BY url
        "#,
        self_url,
        cutoff_time
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.url).collect())
}

async fn work_on_state(state: Vec<String>, service_list: &ServiceList) {
    if lists_different(&state, service_list).await {
        let mut list = service_list.list.write().await;
        *list = state;
    }
}

async fn lists_different(state: &[String], service_list: &ServiceList) -> bool {
    let guard = service_list.list.read().await;
    *guard != *state
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn state_comparison_and_update_work() {
        let services = ServiceList::new();
        assert!(lists_different(&["http://front-1".into()], &services).await);

        work_on_state(vec!["http://front-1".into()], &services).await;

        assert!(!lists_different(&["http://front-1".into()], &services).await);
        assert_eq!(&*services.list.read().await, &["http://front-1"]);
    }

    #[tokio::test]
    async fn discovery_without_self_url_exits_without_database_access() {
        let config = Arc::new(Config {
            kafka_topic_main: "main".into(),
            kafka_topic_removal: "removal".into(),
            kafka_group_id: "group".into(),
            kafka_bootstrap_servers: "localhost:9092".into(),
            database_url: "postgres://localhost/test".into(),
            instance_id: "id".into(),
            service_discovery_self_url: None,
        });
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/test")
            .unwrap();

        let result = service_discovery(
            CancellationToken::new(),
            config,
            Arc::new(ServiceList::new()),
            pool,
        )
        .await;

        assert!(result.is_ok());
    }
}
