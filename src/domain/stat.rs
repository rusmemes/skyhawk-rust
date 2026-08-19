use super::Log;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatPer {
    Team,
    Player,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
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
    pub fn database_column(self) -> &'static str {
        match self {
            Self::Points => "points",
            Self::Rebounds => "rebounds",
            Self::Assists => "assists",
            Self::Steals => "steals",
            Self::Blocks => "blocks",
            Self::Fouls => "fouls",
            Self::Turnovers => "turnovers",
            Self::MinutesPlayed => "minutes_played",
        }
    }

    pub fn value_from(self, log: &Log) -> f64 {
        match self {
            Self::Points => log.points.unwrap_or_default() as f64,
            Self::Rebounds => log.rebounds.unwrap_or_default() as f64,
            Self::Assists => log.assists.unwrap_or_default() as f64,
            Self::Steals => log.steals.unwrap_or_default() as f64,
            Self::Blocks => log.blocks.unwrap_or_default() as f64,
            Self::Fouls => log.fouls.unwrap_or_default() as f64,
            Self::Turnovers => log.turnovers.unwrap_or_default() as f64,
            Self::MinutesPlayed => log.minutes_played.unwrap_or_default(),
        }
    }
}

#[derive(Deserialize)]
pub struct StatRequest {
    pub season: String,
    pub per: StatPer,
    pub values: Vec<StatValue>,
}

pub type StatResponse = HashMap<String, HashMap<StatValue, f64>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> Log {
        Log {
            season: "S".into(),
            team: "T".into(),
            player: "P".into(),
            points: Some(3),
            rebounds: None,
            assists: Some(4),
            steals: None,
            blocks: None,
            fouls: None,
            turnovers: None,
            minutes_played: Some(5.5),
        }
    }

    #[test]
    fn every_stat_has_a_safe_database_column() {
        let values = [
            StatValue::Points,
            StatValue::Rebounds,
            StatValue::Assists,
            StatValue::Steals,
            StatValue::Blocks,
            StatValue::Fouls,
            StatValue::Turnovers,
            StatValue::MinutesPlayed,
        ];

        assert_eq!(
            values.map(StatValue::database_column),
            [
                "points",
                "rebounds",
                "assists",
                "steals",
                "blocks",
                "fouls",
                "turnovers",
                "minutes_played",
            ]
        );
    }

    #[test]
    fn missing_stat_values_are_zero() {
        assert_eq!(StatValue::Points.value_from(&log()), 3.0);
        assert_eq!(StatValue::Rebounds.value_from(&log()), 0.0);
        assert_eq!(StatValue::MinutesPlayed.value_from(&log()), 5.5);
    }

    #[test]
    fn stat_request_deserializes_public_api_values() {
        let request: StatRequest = serde_json::from_str(
            r#"{"season":"S","per":"player","values":["points","minutesPlayed"]}"#,
        )
        .unwrap();

        assert!(matches!(request.per, StatPer::Player));
        assert_eq!(
            request.values,
            [StatValue::Points, StatValue::MinutesPlayed]
        );
    }
}
