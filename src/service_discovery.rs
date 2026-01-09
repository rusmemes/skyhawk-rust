use crate::{Config, ServiceList};
use sqlx::{Executor, PgPool, Pool, Postgres, Row};
use std::sync::Arc;
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
    config: Arc<Config>,
    service_list: Arc<ServiceList>,
) {
    tracing::info!("Service discovery worker started");

    let self_url = config
        .service_discovery_self_url
        .as_ref()
        .expect("No self url provided");

    let pool = PgPool::connect(&config.database_url)
        .await
        .expect("Error connecting to database");

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

async fn remove_expired_records(pool: &Pool<Postgres>, self_url: &str) {
    let cutoff_time = OffsetDateTime::now_utc() - Duration::from_secs(5);
    sqlx::query("DELETE FROM service_discovery WHERE url == $1 OR last_heartbeat_time < $2")
        .bind(self_url)
        .bind(cutoff_time)
        .execute(pool)
        .await
        .expect("Error executing database table for service discovery");
}

async fn run_ddl(pool: &Pool<Postgres>) {
    pool.execute(DDL)
        .await
        .expect("Error executing database table for service discovery");
}

async fn sync(pool: &Pool<Postgres>, self_url: &str, service_list: Arc<ServiceList>) {
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

async fn get_active_urls(pool: &Pool<Postgres>, self_url: &str) -> Vec<String> {
    let cutoff_time = OffsetDateTime::now_utc() - Duration::from_secs(5);

    let rows = sqlx::query(
        r#"
                SELECT url
                FROM service_discovery
                WHERE url != $1
                  AND last_heartbeat_time > $2
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

async fn work_on_state(state: Vec<String>, service_list: Arc<ServiceList>) {
    let mut list = service_list.list.write().await;
    *list = state;
}
