use crate::Config;
use sqlx::{Executor, PgPool, Pool, Postgres};
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

pub async fn service_discovery(token: CancellationToken, config: Arc<Config>) {
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
                tracing::info!("Service discovery worker shutting down");
                break;
            }
            _ = sleep(Duration::from_secs(1)) => {
                sync(&pool, self_url).await;
            }
        }
    }
}

async fn run_ddl(pool: &Pool<Postgres>) {
    pool.execute(DDL)
        .await
        .expect("Error executing database table for service discovery");
}

async fn sync(pool: &Pool<Postgres>, self_url: &str) {
    heartbeat(pool, self_url).await;
    todo!()
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
