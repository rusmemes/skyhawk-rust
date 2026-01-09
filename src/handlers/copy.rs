use crate::protocol::CacheRecord;
use crate::runtime_store::RuntimeStore;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;

pub async fn copy(
    State(runtime_store): State<Arc<RuntimeStore>>,
    season: String,
) -> Result<Json<Vec<CacheRecord>>, StatusCode> {
    let vec: Vec<CacheRecord> = runtime_store.copy(&season);
    if vec.is_empty() {
        Err(StatusCode::NO_CONTENT)
    } else {
        Ok(Json(vec))
    }
}
