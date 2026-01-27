# Redis fixture (dev)

Bring up a local Redis for the DB smokes:

```bash
cd ci/fixtures/bench/db/redis/v1
docker compose up -d
```

It exposes:

- host: `127.0.0.1`
- port: `6379`

Tear down:

```bash
docker compose down -v
```
