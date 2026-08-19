use crate::domain::{CacheRecord, TimeKey};

use crossbeam_skiplist::SkipMap;
use dashmap::DashMap;
use std::sync::Arc;

type Season = String;
type Team = String;
type Player = String;

type TimeMap = SkipMap<TimeKey, Arc<CacheRecord>>;
type PlayerMap = DashMap<Player, Arc<TimeMap>>;
type TeamMap = DashMap<Team, Arc<PlayerMap>>;
type Cache = DashMap<Season, Arc<TeamMap>>;

#[derive(Clone)]
pub struct RuntimeStore {
    cache: Cache,
}

impl RuntimeStore {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    pub fn log(&self, record: CacheRecord) {
        self.log_arc(Arc::new(record));
    }

    pub fn log_arc(&self, record: Arc<CacheRecord>) {
        let log = &record.log;

        let team_map = self
            .cache
            .entry(log.season.to_string())
            .or_insert_with(|| Arc::new(DashMap::new()))
            .clone();

        let player_map = team_map
            .entry(log.team.to_string())
            .or_insert_with(|| Arc::new(DashMap::new()))
            .clone();

        let time_map = player_map
            .entry(log.player.to_string())
            .or_insert_with(|| Arc::new(SkipMap::new()))
            .clone();

        time_map.insert(record.time_key, record);
    }

    pub fn remove(&self, record: &CacheRecord) {
        let log = &record.log;

        let Some(team_map) = self.cache.get(&log.season) else {
            return;
        };
        let Some(player_map) = team_map.get(&log.team) else {
            return;
        };
        let Some(time_map) = player_map.get(&log.player) else {
            return;
        };

        let target = record.time_key;
        loop {
            let first = time_map.iter().next();
            match first {
                Some(entry) if *entry.key() <= target => {
                    time_map.remove(entry.key());
                }
                _ => break,
            }
        }
    }

    pub fn view(&self, season: &str) -> Vec<Arc<CacheRecord>> {
        let Some(team_map) = self.cache.get(season) else {
            return Vec::new();
        };

        let mut res = Vec::new();

        for player_map in team_map.iter() {
            for time_map in player_map.value().iter() {
                for entry in time_map.iter() {
                    res.push(entry.value().clone());
                }
            }
        }

        res
    }
}

impl Default for RuntimeStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeStore;
    use crate::domain::{CacheRecord, Log, TimeKey};

    fn record(season: &str, team: &str, player: &str, time: i64) -> CacheRecord {
        CacheRecord {
            time_key: TimeKey(time, time),
            log: Log {
                season: season.into(),
                team: team.into(),
                player: player.into(),
                steals: Some(1),
                points: None,
                rebounds: None,
                assists: None,
                blocks: None,
                fouls: None,
                turnovers: None,
                minutes_played: None,
            },
        }
    }

    #[test]
    fn stores_and_reads_records_for_requested_season_only() {
        let store = RuntimeStore::new();
        store.log(record("S1", "T", "P", 1));
        store.log(record("S2", "T", "P", 2));

        assert_eq!(store.view("S1").len(), 1);
        assert_eq!(store.view("S2").len(), 1);
        assert!(store.view("UNKNOWN").is_empty());
    }

    #[test]
    fn same_time_key_replaces_existing_record() {
        let store = RuntimeStore::new();
        store.log(record("S", "T", "P", 1));
        store.log(record("S", "T", "P", 1));

        assert_eq!(store.view("S").len(), 1);
    }

    #[test]
    fn removal_marker_removes_only_records_up_to_marker_for_same_player() {
        let store = RuntimeStore::new();
        store.log(record("S", "T", "P", 1));
        store.log(record("S", "T", "P", 2));
        store.log(record("S", "T", "P", 3));
        store.log(record("S", "T", "OTHER", 1));

        store.remove(&record("S", "T", "P", 2));

        let view = store.view("S");
        assert_eq!(view.len(), 2);
        assert!(
            view.iter()
                .any(|record| record.log.player == "P" && record.time_key.0 == 3)
        );
        assert!(view.iter().any(|record| record.log.player == "OTHER"));
    }

    #[test]
    fn removing_unknown_path_is_a_noop() {
        let store = RuntimeStore::new();
        store.log(record("S", "T", "P", 1));

        store.remove(&record("UNKNOWN", "T", "P", 5));
        store.remove(&record("S", "UNKNOWN", "P", 5));
        store.remove(&record("S", "T", "UNKNOWN", 5));

        assert_eq!(store.view("S").len(), 1);
    }
}
