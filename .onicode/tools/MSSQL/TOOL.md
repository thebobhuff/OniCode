---
name: MSSQL
description: Execute SQL queries against a Microsoft SQL Server database
---

# MSSQL Tool

Execute SQL queries against a Microsoft SQL Server database.

## Usage

Run the script at `.onicode/scripts/mssql.sh` with a query:

```bash
.onicode/scripts/mssql.sh "SELECT * FROM users"
```

## Parameters

- `SERVER` — SQL Server hostname (default: localhost)
- `DATABASE` — Database name (required)
- `QUERY` — SQL query to execute (required)

## Environment Variables

Set these in your shell or `.env` file:

- `MSSQL_SERVER` — Server hostname
- `MSSQL_DATABASE` — Database name
- `MSSQL_USER` — Username
- `MSSQL_PASSWORD` — Password
