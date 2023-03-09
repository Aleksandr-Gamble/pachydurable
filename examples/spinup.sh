#!/bin/bash

# This script spins up a postgres database instance 

sudo docker run -d --rm  \
  -p "127.0.0.1:${PSQL_PORT:-5432}:5432" \
  -v "${PWD}/schema.sql:/docker-entrypoint-initdb.d/postgres-tables.sql" \
  --env=POSTGRES_USER=${PSQL_USER:-postgres} \
  --env=POSTGRES_PASSWORD=${PSQL_PW:-abc123} \
  --env=POSTGRES_DB=${PSQL_DB:-postgres} \
  --name=pachydurable-demo \
  postgres:15.1 
