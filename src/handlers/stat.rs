use crate::protocol::{StatRequest, StatValue};
use crate::runtime_store::RuntimeStore;
use crate::ServiceList;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use reqwest::Client;
use sqlx::PgPool;
use std::collections::HashMap;

pub async fn stat(
    State(runtime_store): State<RuntimeStore>,
    State(service_list): State<ServiceList>,
    State(pool): State<PgPool>,
    State(http): State<Client>,
    Json(stat_request): Json<StatRequest>,
) -> Result<Json<HashMap<String, HashMap<StatValue, f64>>>, (StatusCode, String)> {
    todo!()
}
