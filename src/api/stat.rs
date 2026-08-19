use crate::domain::{StatRequest, StatResponse};
use crate::services::statistics::{StatisticsError, StatisticsService};
use crate::state::FrontState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

pub async fn stat(
    State(state): State<FrontState>,
    Json(request): Json<StatRequest>,
) -> Result<Json<StatResponse>, (StatusCode, String)> {
    StatisticsService::new(
        state.runtime_store.as_ref(),
        &state.pool,
        &state.http,
        state.service_list.as_ref(),
    )
    .execute(request)
    .await
    .map(Json)
    .map_err(to_http_error)
}

fn to_http_error(error: StatisticsError) -> (StatusCode, String) {
    match error {
        StatisticsError::InvalidRequest(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
        StatisticsError::Database(error) => {
            tracing::error!(%error, "Failed to load statistics from the database");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".into(),
            )
        }
    }
}
