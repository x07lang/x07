# Messaging broker fixtures (Docker Compose)

This folder contains a local-only messaging broker setup for readiness checks and development.

It is designed for fixed-port, loopback access so it works with `run-os-sandboxed` + explicit `--allow-host` allowlists.

## Services + ports

- Kafka-compatible broker (Redpanda): `127.0.0.1:9092`
- AMQP 0-9-1 broker (RabbitMQ): `127.0.0.1:5672`

## Start

Start both:

```bash
docker compose up -d
docker compose ps
```

Or start only one:

```bash
docker compose up -d redpanda
docker compose up -d rabbitmq
```

## Teardown

```bash
docker compose down -v
```

