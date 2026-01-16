use crate::domain::{CacheRecord, StatPer, StatRequest, StatValue};
use crate::runtime_store::RuntimeStore;
use crate::ServiceList;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use reqwest::Client;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
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
        season: stat_request.season,
        per: stat_request.per,
        values: stat_request
            .values
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect(),
    };

    Ok(Json(
        process_request(runtime_store, pool, http, service_list, stat_request).await,
    ))
}

async fn process_request(
    runtime_store: RuntimeStore,
    pool: PgPool,
    http: Client,
    service_list: ServiceList,
    request: StatRequest,
) -> HashMap<String, HashMap<StatValue, f64>> {

    let season_uppercased = request.season.to_uppercase();
    let sync_state_handle = tokio::spawn(sync_state(
        runtime_store.clone(),
        service_list,
        http,
        season_uppercased.clone(),
    ));

    let value_columns = request
        .values
        .iter()
        .map(|value| value.to_db_column_name().to_string())
        .collect::<Vec<_>>()
        .join(",");

    let sql = format!(
        "select t1,t2,season,team,player,{} from nba_stats where season = $1",
        value_columns
    );

    let option: Option<Vec<CacheRecord>> = sqlx::query_as(&sql)
        .bind(&season_uppercased)
        .fetch_all(&pool)
        .await
        .ok();

    let temp_store = RuntimeStore::new();
    if let Some(db_records) = option {
        for db_record in db_records {
            temp_store.log_arc(Arc::new(db_record));
        }
    }

    match sync_state_handle.await {
        Ok(_) => {}
        Err(error) => tracing::error!("{}", error),
    };

    let in_memory_records = runtime_store.view(&season_uppercased);
    for in_memory_record in in_memory_records {
        temp_store.log_arc(in_memory_record);
    }

    convert_to_response(temp_store.view(&season_uppercased), request)
}

fn convert_to_response(
    view: Vec<Arc<CacheRecord>>,
    request: StatRequest,
) -> HashMap<String, HashMap<StatValue, f64>> {
    let mut map = HashMap::new();

    for record in view {
        let record = &record.log;

        let key = match request.per {
            StatPer::Team => record.team.clone(),
            StatPer::Player => record.player.clone(),
        };

        let key_map = map.entry(key).or_insert_with(HashMap::new);

        for request_stat_value in &request.values {
            match request_stat_value {
                StatValue::Points => {
                    let value = key_map.entry(StatValue::Points).or_insert(0.0);
                    *value += record.points.unwrap_or(0) as f64
                }
                StatValue::Rebounds => {
                    let value = key_map.entry(StatValue::Rebounds).or_insert(0.0);
                    *value += record.rebounds.unwrap_or(0) as f64
                }
                StatValue::Assists => {
                    let value = key_map.entry(StatValue::Assists).or_insert(0.0);
                    *value += record.assists.unwrap_or(0) as f64
                }
                StatValue::Steals => {
                    let value = key_map.entry(StatValue::Steals).or_insert(0.0);
                    *value += record.steals.unwrap_or(0) as f64
                }
                StatValue::Blocks => {
                    let value = key_map.entry(StatValue::Blocks).or_insert(0.0);
                    *value += record.blocks.unwrap_or(0) as f64
                }
                StatValue::Fouls => {
                    let value = key_map.entry(StatValue::Fouls).or_insert(0.0);
                    *value += record.fouls.unwrap_or(0) as f64
                }
                StatValue::Turnovers => {
                    let value = key_map.entry(StatValue::Turnovers).or_insert(0.0);
                    *value += record.turnovers.unwrap_or(0) as f64
                }
                StatValue::MinutesPlayed => {
                    let value = key_map.entry(StatValue::MinutesPlayed).or_insert(0.0);
                    *value += record.minutes_played.unwrap_or(0.0)
                }
            }
        }
    }

    map
}

async fn sync_state(
    runtime_store: RuntimeStore,
    service_list: ServiceList,
    http: Client,
    season: String,
) {
    let vec = call_another_front_instances(service_list, http, season, runtime_store).await;

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
