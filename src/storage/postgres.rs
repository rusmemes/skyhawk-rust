use crate::domain::{CacheRecord, StatValue};
use sqlx::PgPool;

pub async fn load_statistics(
    pool: &PgPool,
    season: &str,
    values: &[StatValue],
) -> Result<Vec<CacheRecord>, sqlx::Error> {
    let sql = statistics_query(values);

    sqlx::query_as(&sql).bind(season).fetch_all(pool).await
}

fn statistics_query(values: &[StatValue]) -> String {
    let columns = values
        .iter()
        .map(|value| value.database_column())
        .collect::<Vec<_>>()
        .join(", ");
    format!("SELECT t1, t2, season, team, player, {columns} FROM nba_stats WHERE season = $1")
}

pub async fn store_records(pool: &PgPool, records: &[CacheRecord]) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    for record in records {
        sqlx::query_as!(
            Self,
            r#"
            INSERT INTO nba_stats (
                t1, t2, season, team, player,
                points, rebounds, assists, steals,
                blocks, fouls, turnovers, minutes_played
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            ON CONFLICT (season, player, team, t1, t2)
            DO UPDATE SET
                points = EXCLUDED.points,
                rebounds = EXCLUDED.rebounds,
                assists = EXCLUDED.assists,
                steals = EXCLUDED.steals,
                blocks = EXCLUDED.blocks,
                fouls = EXCLUDED.fouls,
                turnovers = EXCLUDED.turnovers,
                minutes_played = EXCLUDED.minutes_played
            "#,
            record.time_key.0,
            record.time_key.1,
            &record.log.season,
            &record.log.team,
            &record.log.player,
            record.log.points,
            record.log.rebounds,
            record.log.assists,
            record.log.steals,
            record.log.blocks,
            record.log.fouls,
            record.log.turnovers,
            record.log.minutes_played
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_query_contains_only_requested_whitelisted_columns() {
        let query = statistics_query(&[StatValue::Points, StatValue::MinutesPlayed]);

        assert_eq!(
            query,
            "SELECT t1, t2, season, team, player, points, minutes_played FROM nba_stats WHERE season = $1"
        );
    }
}
