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
                points: row.try_get("points").ok(),
                rebounds: row.try_get("rebounds").ok(),
                assists: row.try_get("assists").ok(),
                steals: row.try_get("steals").ok(),
                blocks: row.try_get("blocks").ok(),
                fouls: row.try_get("fouls").ok(),
                turnovers: row.try_get("turnovers").ok(),
                minutes_played: row.try_get("minutes_played").ok(),
            },
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatPer {
    Team, Player
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum StatValue {
    Points,
    Rebounds,
    Assists,
    Steals,
    Blocks,
    Fouls,
    Turnovers,
    MinutesPlayed,
}

impl StatValue {
    pub fn to_db_column_name(&self) -> &'static str {
        match self {
            StatValue::Points => "points",
            StatValue::Rebounds => "rebounds",
            StatValue::Assists => "assists",
            StatValue::Steals => "steals",
            StatValue::Blocks => "blocks",
            StatValue::Fouls => "fouls",
            StatValue::Turnovers => "turnovers",
            StatValue::MinutesPlayed => "minutes_played",
        }
    }
}

#[derive(Deserialize)]
pub struct StatRequest {
    pub season: String,
    pub per: StatPer,
    pub values: Vec<StatValue>,
}
