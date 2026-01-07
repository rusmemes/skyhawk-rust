use crate::protocol::{CacheRecord, TimeKey};

use crossbeam_skiplist::SkipMap;
use dashmap::DashMap;
use std::sync::Arc;

type Season = String;
type Team = String;
type Player = String;

type TimeMap = SkipMap<TimeKey, CacheRecord>;
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

    pub fn copy(&self, season: &str) -> Vec<CacheRecord> {
        
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
