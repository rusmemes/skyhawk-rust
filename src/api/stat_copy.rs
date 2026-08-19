use crate::domain::CacheRecord;
use crate::storage::runtime::RuntimeStore;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use std::sync::Arc;

pub async fn copy(
    State(runtime_store): State<Arc<RuntimeStore>>,
    season: String,
) -> Result<Json<Vec<Arc<CacheRecord>>>, StatusCode> {
    let vec: Vec<Arc<CacheRecord>> = runtime_store.view(&season.to_uppercase());
    if vec.is_empty() {
        Err(StatusCode::NO_CONTENT)
    } else {
        Ok(Json(vec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Log, TimeKey};

    fn record() -> CacheRecord {
        CacheRecord {
            time_key: TimeKey(1, 1),
            log: Log {
                season: "S1".into(),
                team: "T".into(),
                player: "P".into(),
                points: Some(1),
                rebounds: None,
                assists: None,
                steals: None,
                blocks: None,
                fouls: None,
                turnovers: None,
                minutes_played: None,
            },
        }
    }

    #[tokio::test]
    async fn returns_no_content_for_missing_season() {
        let result = copy(State(Arc::new(RuntimeStore::new())), "S1".into()).await;
        assert_eq!(result.unwrap_err(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn normalizes_season_and_returns_records() {
        let store = Arc::new(RuntimeStore::new());
        store.log(record());

        let Json(records) = copy(State(store), "s1".into()).await.unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].log.player, "P");
    }
}
