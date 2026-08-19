use super::StatValue;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize, Clone, Serialize, Debug)]
pub struct Log {
    pub season: String,
    pub team: String,
    pub player: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebounds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assists: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steals: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fouls: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turnovers: Option<i32>,
    #[serde(alias = "minutesPlayed")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes_played: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TimeKey(pub i64, pub i64);

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CacheRecord {
    pub time_key: TimeKey,
    pub log: Log,
}

impl CacheRecord {
    pub fn new(log: Log) -> Self {
        Self {
            time_key: TimeKey::new(),
            log,
        }
    }
}

impl TimeKey {
    fn new() -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        Self(now.as_millis() as i64, now.as_nanos() as i64)
    }
}

impl<'r> FromRow<'r, PgRow> for CacheRecord {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let t1: i64 = row.try_get("t1")?;
        let t2: i64 = row.try_get("t2")?;

        Ok(Self {
            time_key: TimeKey(t1, t2),
            log: Log {
                season: row.try_get("season")?,
                team: row.try_get("team")?,
                player: row.try_get("player")?,
                points: row.try_get(StatValue::Points.database_column()).ok(),
                rebounds: row.try_get(StatValue::Rebounds.database_column()).ok(),
                assists: row.try_get(StatValue::Assists.database_column()).ok(),
                steals: row.try_get(StatValue::Steals.database_column()).ok(),
                blocks: row.try_get(StatValue::Blocks.database_column()).ok(),
                fouls: row.try_get(StatValue::Fouls.database_column()).ok(),
                turnovers: row.try_get(StatValue::Turnovers.database_column()).ok(),
                minutes_played: row.try_get(StatValue::MinutesPlayed.database_column()).ok(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> Log {
        Log {
            season: "S1".into(),
            team: "TEAM".into(),
            player: "PLAYER".into(),
            points: Some(12),
            rebounds: None,
            assists: None,
            steals: None,
            blocks: None,
            fouls: None,
            turnovers: None,
            minutes_played: Some(20.5),
        }
    }

    #[test]
    fn log_uses_camel_case_minutes_and_omits_missing_values() {
        let json = serde_json::to_value(log()).unwrap();

        assert_eq!(json["minutes_played"], 20.5);
        assert!(json.get("rebounds").is_none());
    }

    #[test]
    fn log_accepts_camel_case_minutes_alias() {
        let log: Log =
            serde_json::from_str(r#"{"season":"S1","team":"T","player":"P","minutesPlayed":8.5}"#)
                .unwrap();

        assert_eq!(log.minutes_played, Some(8.5));
    }

    #[test]
    fn cache_record_generates_orderable_nonzero_time_key() {
        let first = CacheRecord::new(log());
        let second = CacheRecord::new(log());

        assert!(first.time_key.0 > 0);
        assert!(first.time_key <= second.time_key);
    }
}
