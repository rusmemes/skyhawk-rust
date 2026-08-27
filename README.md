# Skyhawk (Rust)

Skyhawk is a distributed service for ingesting and aggregating NBA-style game statistics. Front nodes expose the HTTP API and keep recent events in memory, Kafka transports events, back nodes persist them in PostgreSQL, and Nginx balances requests across front nodes.

## Architecture

```text
client -> Nginx -> front nodes -> Kafka (main) -> back nodes -> PostgreSQL
                   |  ^ ^ ^                           |             | 
                   |  | | +----- Kafka (removal) -----+             |
                   +--+ +-------------------------------------------+
```

- `front`: validates `POST /log`, publishes events, maintains a synchronized in-memory cache, and serves `POST /stat`.
- `back`: consumes Kafka events in batches, persists them transactionally, and publishes cache-removal markers.
- `PostgreSQL`: stores historical events and front-node discovery heartbeats.
- `Kafka`: uses the `main` and `removal` topics.

Statistics combine persisted rows with in-memory events that have not yet been removed. Active front nodes exchange their current season cache through the internal `/stat-copy` endpoint.

## Run with Docker

Requirements: Docker with Compose support.

```bash
docker compose up --build -d
```

The public API is available at `http://localhost:8080`. Stop the stack with:

```bash
docker compose down
```

Add `-v` only if you also want to delete the PostgreSQL volume.

## API

### `POST /log`

At least one numeric statistic must be present. `season`, `team`, and `player` are trimmed and normalized to uppercase. Numeric values must not be negative; zero values are accepted but do not count as the required statistic on their own.

```json
{
  "season": "season3",
  "team": "team3",
  "player": "player3",
  "points": 20,
  "rebounds": 10,
  "assists": 5,
  "minutesPlayed": 32.5
}
```

Supported statistics: `points`, `rebounds`, `assists`, `steals`, `blocks`, `fouls`, `turnovers`, and `minutesPlayed`.

Success response: `202 Accepted`.

### `POST /stat`

`per` must be `team` or `player`. Duplicate entries in `values` are ignored, and `season` is normalized in the same way as `/log`.

```json
{
  "season": "season3",
  "per": "player",
  "values": ["points", "rebounds", "minutesPlayed"]
}
```

Example response:

```json
{
  "PLAYER1": {
    "points": 20.0,
    "rebounds": 10.0,
    "minutesPlayed": 32.5
  }
}
```

`POST /stat-copy` is an internal endpoint used between front nodes and should not be exposed independently.

## Local development

The project uses Rust edition 2024. SQLx compile-time queries require either a reachable database through `DATABASE_URL` or the checked-in offline metadata:

```bash
SQLX_OFFLINE=true cargo test --all-targets
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Environment variables:

| Variable | Used by | Description |
| --- | --- | --- |
| `DATABASE_URL` | front, back | PostgreSQL connection string |
| `KAFKA_TOPIC_MAIN` | front, back | Incoming-event topic |
| `KAFKA_TOPIC_REMOVAL` | front, back | Cache-removal topic |
| `KAFKA_GROUP_ID` | front, back | Consumer group; `random` generates a UUID |
| `KAFKA_BOOTSTRAP_SERVERS` | front, back | Kafka broker list |
| `SERVICE_DISCOVERY_SELF_URL` | front | Optional advertised URL; `docker.host` resolves from `HOSTNAME` |

Database migrations run automatically when either binary starts and are stored in `migrations/`.

## Project layout

```text
src/
├── bin/                 thin front and back entry points
├── api/                 HTTP handlers and HTTP error mapping
├── domain/              request, response, and event models
├── services/            statistics use case and front synchronization
├── storage/             PostgreSQL access and in-memory runtime store
├── kafka/               front consumer and back persistence worker
├── config.rs            environment configuration
├── state.rs             Axum application state
├── discovery.rs         front-node discovery
├── shutdown.rs          task supervision and graceful shutdown
└── error.rs             shared infrastructure errors
migrations/              PostgreSQL schema
front/, back/, lb/        container definitions
```

The dependency flow is `api -> services -> storage/domain`. The binaries assemble dependencies and start workers; business logic does not live in the entry points.
