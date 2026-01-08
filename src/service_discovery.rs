use crate::Config;
use sqlx::{Executor, PgPool, Pool, Postgres};
use std::sync::Arc;
use std::time::Duration;
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
                sync(&pool, config.clone()).await;
            }
        }
    }
}

async fn run_ddl(pool: &Pool<Postgres>) {
    pool.execute(DDL)
        .await
        .expect("Error executing database table for service discovery");
}

async fn sync(_pool: &Pool<Postgres>, config: Arc<Config>) {
    todo!()
}
