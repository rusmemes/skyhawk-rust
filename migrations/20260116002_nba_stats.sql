create table if not exists nba_stats
(
    t1             bigint not null,
    t2             bigint not null,
    season         text   not null,
    team           text   not null,
    player         text   not null,
    points         integer,
    rebounds       integer,
    assists        integer,
    steals         integer,
    blocks         integer,
    fouls          integer,
    turnovers      integer,
    minutes_played double precision
);

create unique index if not exists nba_stats_unique_idx ON nba_stats (season, player, team, t1, t2);
create index if not exists nba_stats_agg_idx ON nba_stats (season, player, team);
