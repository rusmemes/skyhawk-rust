use crate::protocol::CacheRecord;
use crate::FrontState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

pub async fn copy(
    State(FrontState {
        producer,
        config,
        runtime_store,
    }): State<FrontState>,
    season: String,
) -> Result<Json<Vec<CacheRecord>>, StatusCode> {
    let vec: Vec<CacheRecord> = runtime_store.copy(&season);
    if vec.is_empty() {
        Err(StatusCode::NO_CONTENT)
    } else {
        Ok(Json(vec))
    }
}
