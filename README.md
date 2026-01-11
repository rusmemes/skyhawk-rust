Skyhawk test task
============

## To run

    docker-compose up -d

Then load balancer will start listening port 8080 on localhost

## Containers

* Postgres database
* Zookeeper
* Kafka
* Load balancer
* Front
* Back

## Front container

The engine providing an api to handle /log and /stat requests. See below for the both endpoints' description.

## Back container

The engine which works on stat data: it writes all the incoming log records to the database.

## Front API

### /log

Consumers json requests fit to schema

~~~json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "properties": {
    "season": {
      "type": "string"
    },
    "team": {
      "type": "string"
    },
    "player": {
      "type": "string"
    },
    "points": {
      "type": "integer"
    },
    "rebounds": {
      "type": "integer"
    },
    "assists": {
      "type": "integer"
    },
    "steals": {
      "type": "integer"
    },
    "blocks": {
      "type": "integer"
    },
    "fouls": {
      "type": "integer"
    },
    "turnovers": {
      "type": "integer"
    },
    "minutesPlayed": {
      "type": "number"
    }
  },
  "required": [
    "season",
    "team",
    "player"
  ]
}
~~~

Example request

~~~json
{
  "season": "season3",
  "team": "team3",
  "player": "player3",
  "steals": 3,
  "minutesPlayed": 3.0
}
~~~

### /stat

Consumers json requests fit to schema

~~~json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "properties": {
    "season": {
      "type": "string"
    },
    "per": {
      "type": "string"
    },
    "values": {
      "type": "array",
      "items": [
        {
          "type": "string"
        }
      ]
    }
  },
  "required": [
    "season",
    "per",
    "values"
  ]
}
~~~

Example request

~~~json
{
  "season": "season",
  "per": "player",
  "values": [
    "steals",
    "blocks",
    "fouls",
    "minutesPlayed",
    "points",
    "rebounds",
    "assists",
    "turnovers"
  ]
}
~~~

The list of possible aggregation keys:

* team
* player

Example response:

~~~json
{
  "PLAYER1": {
    "minutesPlayed": 3.0
  },
  "PLAYER2": {
    "minutesPlayed": 4.0
  }
}
~~~

## How it works

The one of the available front containers receives a request on /log endpoint, validates the payload,
generates timestamps pair to make the combination of season, team, player and both generated timestamp unique across all
requests, the uniqueness is important to store the received log record correctly in the database and in memory.

If the validation process succeed, front container sends the log record and generated timestamps pair to kafka, then
stores the same info in the local memory. In case when an attempt to write to kafka fails nothing is being stored in
memory and the 503 status code is being returned also.

Front container writes all incoming info to the kafka topic, lets call it main, and also listens this topic with its
own randomly generated group id to get also the records which are being written to the main topic by other front
containers.

So mainly we have all incoming requests written to the main topic and stored in the memory of all existing front
containers.

Then the back containers start playing their roles.
All they do is reading main topic with the same static group id and writing all the read data to the database.
After the next batch of records is saved in the database, back container send one record with the maximum timestamp pair
per kafka key to another topic, lets call it removal.

Front container read the removal topic also, each front container does it with its own unique group id to read all
the records for all keys. When some record is read from the removal topic, front container removes all the records with
the same combinations of season + team + player from its own memory, but it does it only for record with timestamps
lower of equal to the timestamp of the record read from the removal topic.

When a stat request arrives to /stat endpoint, front container grabs all the available data from the database,
from its own memory and from the memory of other existing containers, merges the data and calculates statistics
according to the provided season, aggregation key and values.

All the dataflows processing in the system is idempotent, there will be no problems if case of any kafka or database
connection issues.
