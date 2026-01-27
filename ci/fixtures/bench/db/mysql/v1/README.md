# MySQL fixture (dev)

Bring up a local MySQL for the DB smokes:

```bash
cd ci/fixtures/bench/db/mysql/v1
docker compose up -d
```

It exposes:

- host: `127.0.0.1`
- port: `3306`
- db: `x07`
- user: `x07`
- pass: `x07`

Tear down:

```bash
docker compose down -v
```
