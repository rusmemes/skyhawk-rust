use crate::{Config, ServiceList};
use sqlx::PgPool;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub async fn service_discovery(
    token: CancellationToken,
    config: Config,
    service_list: ServiceList,
    pool: PgPool,
) {
    tracing::info!("Service discovery worker started");

    let self_url = config
        .service_discovery_self_url
        .as_deref()
        .expect("No self url provided");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                remove_expired_records(&pool, self_url).await;
                tracing::info!("Service discovery worker shutting down");
                break;
            }
            _ = sleep(Duration::from_secs(1)) => {
                sync(&pool, self_url, &service_list).await;
            }
        }
    }
}

async fn remove_expired_records(pool: &PgPool, self_url: &str) {
    const DURATION: Duration = Duration::from_secs(5);

    let cutoff_time = OffsetDateTime::now_utc() - DURATION;
    sqlx::query!(
        "DELETE FROM service_discovery WHERE url = $1 OR last_heartbeat_time < $2",
        self_url,
        cutoff_time
    )
    .execute(pool)
    .await
    .expect("Error executing database table for service discovery");
}

async fn sync(pool: &PgPool, self_url: &str, service_list: &ServiceList) {
    heartbeat(pool, self_url).await;
    let urls = get_active_urls(pool, self_url).await;
    work_on_state(urls, service_list).await;
}

async fn heartbeat(pool: &PgPool, self_url: &str) {
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
    .await
    .expect("Error executing database table for service discovery");
}

async fn get_active_urls(pool: &PgPool, self_url: &str) -> Vec<String> {
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
    .await
    .expect("query failed");

    rows.into_iter().map(|r| r.url).collect()
}

async fn work_on_state(state: Vec<String>, service_list: &ServiceList) {
    if lists_different(&state, service_list).await {
        let mut list = service_list.list.write().await;
        *list = state;
    }
}

async fn lists_different(state: &Vec<String>, service_list: &ServiceList) -> bool {
    let guard = service_list.list.read().await;
    *guard != *state
}
