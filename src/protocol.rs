use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize, Clone, Serialize, Debug)]
pub struct Log {
    pub season: String,
    pub team: String,
    pub player: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebounds: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assists: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steals: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fouls: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turnovers: Option<i16>,
    #[serde(alias = "minutesPlayed")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes_played: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TimeKey(pub i64, pub i64);

impl TimeKey {
    fn new() -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        Self(now.as_millis() as i64, now.as_nanos() as i64)
    }
}

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
