use crate::protocol::{CacheRecord, StatRequest, StatValue};
use crate::runtime_store::RuntimeStore;
use crate::ServiceList;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use reqwest::Client;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use tokio::task::JoinHandle;

pub async fn stat(
    State(runtime_store): State<RuntimeStore>,
    State(service_list): State<ServiceList>,
    State(pool): State<PgPool>,
    State(http): State<Client>,
    Json(stat_request): Json<StatRequest>,
) -> Result<Json<HashMap<String, HashMap<StatValue, f64>>>, (StatusCode, String)> {

    if stat_request.season.is_empty()
        || stat_request.season.chars().all(char::is_whitespace)
        || stat_request.values.is_empty()
    {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            String::from("Request body is incorrect: provide season and values"),
        ));
    }

    let stat_request = StatRequest {
        season: stat_request.season.to_uppercase(),
        per: stat_request.per,
        values: stat_request
            .values
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect(),
    };

    Ok(Json(process_request(runtime_store, pool, http, service_list, stat_request).await))
}

async fn process_request(
    runtime_store: RuntimeStore,
    pool: PgPool,
    http: Client,
    service_list: ServiceList,
    request: StatRequest,
) -> HashMap<String, HashMap<StatValue, f64>> {
    let sync_state_handle = tokio::spawn(sync_state(
        runtime_store.clone(),
        service_list,
        http,
        request.season.clone(),
    ));

    todo!()
}

async fn sync_state(
    runtime_store: RuntimeStore,
    service_list: ServiceList,
    http: Client,
    season: String,
) {
    let vec = call_another_front_instances(service_list, http, season, runtime_store.clone()).await;

    for join_handle in vec {
        match join_handle.await {
            Ok(_) => {}
            Err(error) => {
                tracing::error!(error = %error, "Error occurred during sync state (All the possible errors are handled inside tasks)");
            }
        };
    }
}

async fn call_another_front_instances(
    service_list: ServiceList,
    http: Client,
    season: String,
    runtime_store: RuntimeStore,
) -> Vec<JoinHandle<()>> {
    let mut vec = Vec::new();

    let quard = service_list.list.read().await;
    for url in quard.iter() {
        vec.push(tokio::spawn(call_front_instance(
            url.clone(),
            http.clone(),
            season.clone(),
            runtime_store.clone(),
        )));
    }

    vec
}

async fn call_front_instance(
    url: String,
    http: Client,
    season: String,
    runtime_store: RuntimeStore,
) {
    let resp = http
        .post(format!("{}/stat-copy", url))
        .body(season)
        .header("content-type", "text/plain")
        .send()
        .await;

    if let Err(error) = resp {
        tracing::error!(error = %error, "Error occurred sending request");
        return;
    }

    let resp = resp.unwrap();

    match resp.status() {
        StatusCode::OK => {
            let data = resp.json::<Vec<CacheRecord>>().await;
            match data {
                Ok(vec) => {
                    for record in vec {
                        runtime_store.log(record);
                    }
                }
                Err(error) => {
                    tracing::error!(error = %error, "Error occurred parsing response");
                }
            }
        }
        StatusCode::NO_CONTENT => {}
        status => {
            let error = format!("Unexpected status: {:?}", status);
            let x = &error;
            tracing::info!(x);
        }
    }
}
