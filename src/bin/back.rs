use skyhawk_rust::Config;
use sqlx::PgPool;

#[tokio::main]
async fn main() {
    let config = Config::new();
    let pool = PgPool::connect(&config.database_url)
        .await
        .expect("Error connecting to database");
}
