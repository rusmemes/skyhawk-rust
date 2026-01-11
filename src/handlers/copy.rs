use crate::domain::CacheRecord;
use crate::runtime_store::RuntimeStore;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;

pub async fn copy(
    State(runtime_store): State<RuntimeStore>,
    season: String,
) -> Result<Json<Vec<Arc<CacheRecord>>>, StatusCode> {
    let vec: Vec<Arc<CacheRecord>> = runtime_store.view(&season.to_uppercase());
    if vec.is_empty() {
        Err(StatusCode::NO_CONTENT)
    } else {
        Ok(Json(vec))
    }
}
