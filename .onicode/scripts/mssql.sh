#!/bin/bash
# MSSQL query script
# Usage: mssql.sh "SELECT * FROM table"

SERVER="${MSSQL_SERVER:-localhost}"
DATABASE="${MSSQL_DATABASE}"
QUERY="$1"

if [ -z "$DATABASE" ]; then
    echo "Error: MSSQL_DATABASE environment variable is required"
    exit 1
fi

if [ -z "$QUERY" ]; then
    echo "Usage: mssql.sh \"<sql query>\""
    exit 1
fi

sqlcmd -S "$SERVER" -d "$DATABASE" -U "${MSSQL_USER:-sa}" -P "${MSSQL_PASSWORD:-}" -Q "$QUERY" -W -s ","
