use crate::domain::{CacheRecord, StatPer, StatRequest, StatResponse};
use crate::services::front_sync;
use crate::state::ServiceList;
use crate::storage::{postgres, runtime::RuntimeStore};
use reqwest::Client;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;

pub struct StatisticsService<'a> {
    runtime_store: &'a RuntimeStore,
    pool: &'a PgPool,
    http: &'a Client,
    service_list: &'a ServiceList,
}

impl<'a> StatisticsService<'a> {
    pub fn new(
        runtime_store: &'a RuntimeStore,
        pool: &'a PgPool,
        http: &'a Client,
        service_list: &'a ServiceList,
    ) -> Self {
        Self {
            runtime_store,
            pool,
            http,
            service_list,
        }
    }

    pub async fn execute(&self, mut request: StatRequest) -> Result<StatResponse, StatisticsError> {
        normalize_and_validate(&mut request)?;

        let (database_records, front_records) = tokio::join!(
            postgres::load_statistics(self.pool, &request.season, &request.values),
            front_sync::fetch_records(self.service_list, self.http, &request.season)
        );
        let database_records = database_records?;

        for record in front_records {
            self.runtime_store.log(record);
        }

        let combined_store = RuntimeStore::new();
        for record in database_records {
            combined_store.log(record);
        }
        for record in self.runtime_store.view(&request.season) {
            combined_store.log_arc(record);
        }

        Ok(aggregate(combined_store.view(&request.season), &request))
    }
}

fn normalize_and_validate(request: &mut StatRequest) -> Result<(), StatisticsError> {
    request.season = request.season.trim().to_uppercase();
    if request.season.is_empty() || request.values.is_empty() {
        return Err(StatisticsError::InvalidRequest(
            "Request body is incorrect: provide season and values".into(),
        ));
    }

    let mut seen = HashSet::new();
    request.values.retain(|value| seen.insert(*value));
    Ok(())
}

fn aggregate(view: Vec<Arc<CacheRecord>>, request: &StatRequest) -> StatResponse {
    let mut response = HashMap::new();
    for record in view {
        let group = match request.per {
            StatPer::Team => &record.log.team,
            StatPer::Player => &record.log.player,
        };
        let values = response.entry(group.clone()).or_insert_with(HashMap::new);
        for stat in &request.values {
            *values.entry(*stat).or_default() += stat.value_from(&record.log);
        }
    }
    response
}

#[derive(Debug, Error)]
pub enum StatisticsError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Log, StatValue};

    #[test]
    fn aggregates_selected_values_per_team() {
        let records = vec![Arc::new(CacheRecord::new(Log {
            season: "S1".into(),
            team: "TEAM".into(),
            player: "PLAYER".into(),
            points: Some(10),
            rebounds: Some(4),
            assists: None,
            steals: None,
            blocks: None,
            fouls: None,
            turnovers: None,
            minutes_played: Some(12.5),
        }))];
        let request = StatRequest {
            season: "S1".into(),
            per: StatPer::Team,
            values: vec![StatValue::Points, StatValue::MinutesPlayed],
        };

        let result = aggregate(records, &request);

        assert_eq!(result["TEAM"][&StatValue::Points], 10.0);
        assert_eq!(result["TEAM"][&StatValue::MinutesPlayed], 12.5);
        assert!(!result["TEAM"].contains_key(&StatValue::Rebounds));
    }

    #[test]
    fn aggregates_multiple_records_per_player() {
        let make_record = |team: &str, points| {
            Arc::new(CacheRecord::new(Log {
                season: "S1".into(),
                team: team.into(),
                player: "PLAYER".into(),
                points: Some(points),
                rebounds: None,
                assists: None,
                steals: None,
                blocks: None,
                fouls: None,
                turnovers: None,
                minutes_played: None,
            }))
        };
        let request = StatRequest {
            season: "S1".into(),
            per: StatPer::Player,
            values: vec![StatValue::Points],
        };

        let result = aggregate(vec![make_record("A", 2), make_record("B", 3)], &request);

        assert_eq!(result["PLAYER"][&StatValue::Points], 5.0);
    }

    #[test]
    fn normalizes_season_and_deduplicates_values_without_reordering() {
        let mut request = StatRequest {
            season: "  season  ".into(),
            per: StatPer::Team,
            values: vec![StatValue::Points, StatValue::Rebounds, StatValue::Points],
        };

        normalize_and_validate(&mut request).unwrap();

        assert_eq!(request.season, "SEASON");
        assert_eq!(request.values, [StatValue::Points, StatValue::Rebounds]);
    }

    #[test]
    fn rejects_blank_season_or_empty_values() {
        for mut request in [
            StatRequest {
                season: "  ".into(),
                per: StatPer::Team,
                values: vec![StatValue::Points],
            },
            StatRequest {
                season: "S1".into(),
                per: StatPer::Team,
                values: vec![],
            },
        ] {
            assert!(matches!(
                normalize_and_validate(&mut request),
                Err(StatisticsError::InvalidRequest(_))
            ));
        }
    }
}
