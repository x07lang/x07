# Postgres fixture (dev)

Bring up a local Postgres for the DB smokes:

```bash
cd ci/fixtures/bench/db/pg/v1
docker compose up -d
```

It exposes:

- host: `127.0.0.1`
- port: `5432`
- db: `x07`
- user: `x07`
- pass: `x07`

Tear down:

```bash
docker compose down -v
```
