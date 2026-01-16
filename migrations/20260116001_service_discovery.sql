create table if not exists service_discovery
(
    url                 text        not null,
    last_heartbeat_time timestamptz not null
);
create unique index if not exists service_discovery_url_unique_idx ON service_discovery (url);
