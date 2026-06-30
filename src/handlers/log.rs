use crate::domain::{CacheRecord, Log};
use crate::runtime_store::RuntimeStore;
use crate::{Config, HEADER_SENDER};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::sync::Arc;
use std::time::Duration;

pub async fn log(
    State(producer): State<FutureProducer>,
    State(config): State<Arc<Config>>,
    State(runtime_store): State<Arc<RuntimeStore>>,
    Json(log): Json<Log>,
) -> Result<StatusCode, (StatusCode, String)> {
    let errors = log.validate();
    if !errors.is_empty() {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, errors.join("\n")));
    }

    let log = Log {
        season: log.season.trim().to_uppercase(),
        team: log.team.trim().to_uppercase(),
        player: log.player.trim().to_uppercase(),
        ..log
    };

    let record = CacheRecord::new(log);

    let Ok(json) = serde_json::to_string(&record) else {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            String::from("Internal Server Error"),
        ));
    };

    let headers = OwnedHeaders::new().insert(Header {
        key: HEADER_SENDER,
        value: Some(config.instance_id.as_str()),
    });

    let delivery = producer
        .send(
            FutureRecord::to(&config.kafka_topic_main)
                .key(&record.log.kafka_key())
                .payload(&json)
                .headers(headers),
            Duration::from_secs(5),
        )
        .await;

    match delivery {
        Ok(_) => {
            runtime_store.log(record);
            Ok(StatusCode::ACCEPTED)
        }
        Err((e, _)) => {
            tracing::debug!(%e, "Failed to deliver record");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            ))
        }
    }
}

impl Log {
    fn kafka_key(&self) -> String {
        format!("log-{}-{}-{}", self.season, self.team, self.player)
    }
    fn validate(&self) -> Vec<String> {
        let mut errors = vec![];
        Self::check_string(&self.season, &mut errors, "Season");
        Self::check_string(&self.team, &mut errors, "Team");
        Self::check_string(&self.player, &mut errors, "Player");

        let mut empty = true;
        Self::check_number(self.assists, &mut errors, "Assists", &mut empty);
        Self::check_number(self.blocks, &mut errors, "Blocks", &mut empty);
        Self::check_number(self.fouls, &mut errors, "Fouls", &mut empty);
        Self::check_number(self.points, &mut errors, "Points", &mut empty);
        Self::check_number(self.rebounds, &mut errors, "Rebounds", &mut empty);
        Self::check_number(self.steals, &mut errors, "Steals", &mut empty);
        Self::check_number(self.turnovers, &mut errors, "Turnovers", &mut empty);
        Self::check_number(
            self.minutes_played,
            &mut errors,
            "Minutes Played",
            &mut empty,
        );

        if empty {
            errors.push(String::from("The Request contains no values"))
        }
        errors
    }

    fn check_string(s: &str, errors: &mut Vec<String>, label: &str) {
        if s.is_empty() || s.chars().all(char::is_whitespace) {
            errors.push(String::from(format!("{} value is not correct", label)))
        }
    }

    fn check_number<T>(opt: Option<T>, errors: &mut Vec<String>, label: &str, empty: &mut bool)
    where
        T: PartialOrd + Default,
    {
        if let Some(value) = opt {
            if value > T::default() {
                *empty = false;
            } else if value < T::default() {
                errors.push(String::from(format!(
                    "{} value must a positive value",
                    label
                )))
            }
        }
    }
}
