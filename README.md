# Skyhawk (Rust)

Skyhawk is a distributed statistics processing service written in Rust.

It ingests NBA-like game log events, processes them through Kafka,
stores them in PostgreSQL, and serves aggregated statistics through a
REST API.

The system is designed as a scalable microservice architecture with
stateless API nodes, stream processing via Kafka, and persistent
storage.

------------------------------------------------------------------------

## Architecture Overview

The system is composed of several services running in containers:

                 +-------------+
                 |  Load Balancer |
                 +-------+-----+
                         |
               +---------+---------+
               |                   |
            +--v---+           +---v--+
            | Front |  ...     | Front |
            +--+---+           +---+--+
               |                   |
               +---------+---------+
                         |
                       Kafka
                         |
                   +-----+-----+
                   |           |
                +--v--+    +---v---+
                | Back |    | Back |
                +--+---+    +---+--+
                   |
                PostgreSQL

------------------------------------------------------------------------

## Components

### Front Service

Responsible for:

-   Accepting incoming API requests
-   Validating payloads
-   Writing log records to Kafka
-   Maintaining an in‑memory cache of recent records
-   Serving statistics requests

Each front instance:

-   produces messages to Kafka
-   consumes the same Kafka topic to stay in sync with other instances

Implementation: `src/bin/front.rs`

------------------------------------------------------------------------

### Back Service

Responsible for:

-   Consuming log events from Kafka
-   Persisting them into PostgreSQL
-   Publishing cleanup signals to Kafka once records are stored

Implementation: `src/bin/back.rs`

------------------------------------------------------------------------

### Kafka

Kafka acts as the main event streaming backbone of the system.

Two topics are used:

**main**\
Contains all incoming log events.

**removal**\
Contains markers indicating which events have already been persisted and
can be removed from front-node memory.

------------------------------------------------------------------------

### PostgreSQL

Stores all historical statistics data.

Database schema is created via SQL migrations located in:

    migrations/

------------------------------------------------------------------------

### Load Balancer

An Nginx container distributes requests across multiple Front nodes.

Configuration:

    lb/nginx.conf

------------------------------------------------------------------------

## Data Flow

1.  Client sends `/log` request
2.  Front node validates the request
3.  Front node generates a unique timestamp pair
4.  Event is published to **Kafka main topic**
5.  All Front nodes consume the event and store it in memory
6.  Back nodes consume events and persist them into PostgreSQL
7.  Back nodes publish a marker to **Kafka removal topic**
8.  Front nodes receive the marker and remove old events from memory

This ensures:

-   consistency across front nodes
-   idempotent processing
-   safe recovery after failures

------------------------------------------------------------------------

## Running the System

The easiest way to run the full stack is with Docker.

    docker-compose up -d

This starts:

-   PostgreSQL
-   Zookeeper
-   Kafka
-   Front service
-   Back service
-   Nginx load balancer

The API will be available at:

    http://localhost:8080

------------------------------------------------------------------------

## API

### POST /log

Registers a game log event.

Example request:

``` json
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

Fields include:

-   season
-   team
-   player
-   points
-   rebounds
-   assists
-   steals
-   blocks
-   fouls
-   turnovers
-   minutesPlayed

Only `season`, `team`, and `player` are required.

------------------------------------------------------------------------

### POST /stat

Returns aggregated statistics.

Example request:

``` json
{
  "season": "season3",
  "per": "player",
  "values": [
    "points",
    "rebounds",
    "assists"
  ]
}
```

Parameters:

  Field    Description
  -------- --------------------------------------
  season   Season identifier
  per      Aggregation key (`team` or `player`)
  values   Metrics to aggregate

Example response:

``` json
{
  "PLAYER1": {
    "points": 20,
    "rebounds": 10
  },
  "PLAYER2": {
    "points": 18,
    "rebounds": 7
  }
}
```

------------------------------------------------------------------------

## Technology Stack

-   Rust
-   Axum
-   Tokio
-   Kafka (rdkafka)
-   PostgreSQL
-   SQLx
-   Docker
-   Nginx

------------------------------------------------------------------------

## Project Structure

    src/
     ├ bin/
     │  ├ front.rs
     │  └ back.rs
     ├ handlers/
     │  ├ log.rs
     │  ├ stat.rs
     │  └ copy.rs
     ├ domain.rs
     ├ service_discovery.rs
     ├ runtime_store.rs
     └ utils.rs

    migrations/
    front/
    back/
    lb/
    docker-compose.yaml

------------------------------------------------------------------------

## Design Characteristics

Skyhawk focuses on:

-   horizontal scalability
-   event-driven architecture
-   idempotent data processing
-   fault tolerance
-   low-latency statistics queries

The combination of **Kafka + in-memory caching + persistent storage**
allows the system to handle high write throughput while still providing
fast query responses.

------------------------------------------------------------------------

## Development

Build the project:

    cargo build

Run services locally:

    cargo run --bin front
    cargo run --bin back

------------------------------------------------------------------------

## License

MIT
