#!/bin/bash

# This script spins up a postgres database instance 

echo "spinnig up the 'pachydurable-demo' postgres container with these settings:"
echo "PSQL_PORT=${PSQL_PORT:=5432}"
echo "PSQL_USER=${PSQL_USER:=postgres}"
echo "PSQL_DB=${PSQL_DB:=postgres}"
echo "PSQL_PW=${PSQL_PW:=abc123}"
echo ""
echo "Don't forget to run 'export PSQL_PW=${PSQL_PW}'"
echo ""

sudo docker run -d --rm  \
  -p "127.0.0.1:${PSQL_PORT}:5432" \
  -v "${PWD}/schema.sql:/docker-entrypoint-initdb.d/postgres-tables.sql" \
  --env=POSTGRES_USER=${PSQL_USER} \
  --env=POSTGRES_PASSWORD=${PSQL_PW} \
  --env=POSTGRES_DB=${PSQL_DB} \
  --name=pachydurable-demo \
  postgres:15.1 
