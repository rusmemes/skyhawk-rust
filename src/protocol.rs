use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize, Clone, Serialize)]
pub struct Log {
    pub season: String,
    pub team: String,
    pub player: String,
    pub points: Option<i16>,
    pub rebounds: Option<i16>,
    pub assists: Option<i16>,
    pub steals: Option<i16>,
    pub blocks: Option<i16>,
    pub fouls: Option<i16>,
    pub turnovers: Option<i16>,
    pub minutes_played: Option<f32>,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct TimeKey(i64, i64);

impl TimeKey {
    fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();

        Self(now.as_millis() as i64, now.as_nanos() as i64)
    }
}

#[derive(Clone, Serialize)]
pub struct CacheRecord {
    pub time_key: TimeKey,
    pub log: Log,
}
