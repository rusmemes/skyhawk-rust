use crate::{Config, ServiceList};
use sqlx::{Executor, PgPool, Row};
use std::time::Duration;
use time::OffsetDateTime;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

const DDL: &str = r#"
    create table if not exists service_discovery
    (
      url                 text      not null,
      last_heartbeat_time timestamp not null
    );
    create unique index if not exists service_discovery_url_unique_idx ON service_discovery (url);
"#;

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

    run_ddl(&pool).await;

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                remove_expired_records(&pool, self_url).await;
                tracing::info!("Service discovery worker shutting down");
                break;
            }
            _ = sleep(Duration::from_secs(1)) => {
                sync(&pool, self_url, service_list.clone()).await;
            }
        }
    }
}

async fn remove_expired_records(pool: &PgPool, self_url: &str) {
    const DURATION: Duration = Duration::from_secs(5);

    let cutoff_time = OffsetDateTime::now_utc() - DURATION;
    sqlx::query("DELETE FROM service_discovery WHERE url = $1 OR last_heartbeat_time < $2")
        .bind(self_url)
        .bind(cutoff_time)
        .execute(pool)
        .await
        .expect("Error executing database table for service discovery");
}

async fn run_ddl(pool: &PgPool) {
    pool.execute(DDL)
        .await
        .expect("Error executing database table for service discovery");
}

async fn sync(pool: &PgPool, self_url: &str, service_list: ServiceList) {
    heartbeat(pool, self_url).await;
    let urls = get_active_urls(pool, self_url).await;
    work_on_state(urls, service_list).await;
}

async fn heartbeat(pool: &PgPool, self_url: &str) {
    let now = OffsetDateTime::now_utc();

    sqlx::query(
        r#"
        INSERT INTO service_discovery (url, last_heartbeat_time)
        VALUES ($1, $2)
        ON CONFLICT (url) DO UPDATE
          SET last_heartbeat_time = EXCLUDED.last_heartbeat_time
        "#,
    )
    .bind(self_url)
    .bind(now)
    .execute(pool)
    .await
    .expect("Error executing database table for service discovery");
}

async fn get_active_urls(pool: &PgPool, self_url: &str) -> Vec<String> {

    const DURATION: Duration = Duration::from_secs(5);
    let cutoff_time = OffsetDateTime::now_utc() - DURATION;

    let rows = sqlx::query(
        r#"
                SELECT url
                FROM service_discovery
                WHERE url != $1
                  AND last_heartbeat_time > $2
                ORDER BY url
                "#,
    )
    .bind(self_url)
    .bind(cutoff_time)
    .fetch_all(pool)
    .await
    .expect("Error executing database table for service discovery");

    rows.into_iter()
        .map(|row| row.try_get("url"))
        .collect::<Result<_, _>>()
        .expect("Error getting state from database")
}

async fn work_on_state(state: Vec<String>, service_list: ServiceList) {
    if lists_different(&state, service_list.clone()).await {
        let mut list = service_list.list.write().await;
        *list = state;
    }
}

async fn lists_different(state: &Vec<String>, service_list: ServiceList) -> bool {
    let guard = service_list.list.read().await;
    *guard != *state
}
