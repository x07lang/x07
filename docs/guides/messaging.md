# Guide: Messaging and event streaming

X07 supports messaging through a **pure core + OS-bound drivers** split:

- `ext-msg-core` (pinned by `arch/msg/`): canonical message envelope encoding, RR cassette helpers, driver interface (pure worlds)
- `ext-msg-kafka-c` (pinned by `arch/msg/kafka/`): Kafka driver (OS worlds only)
- `ext-msg-amqp-c` (pinned by `arch/msg/amqp/`): AMQP 0-9-1 driver (OS worlds only)

## Canonical workflow

1. Keep your application logic pure by passing around `msg.envelope_v1` bytes.
2. Put broker-specific I/O behind a thin adapter that runs in `run-os` / `run-os-sandboxed`.
3. Record real broker interactions once (under an rr policy like `msg_rr_v1`) and keep the cassette under `.x07_rr/`.
4. Replay broker interactions deterministically in `solve-rr` using the replay driver wrappers.

`x07 arch check` validates the pinned contracts under `arch/msg/**` and rejects importing OS-only drivers into solve worlds.

## Adding packages

Use the capability map (`msg.core`, `msg.kafka`, `msg.amqp`) and sync the lockfile.

Canonical: add the driver you need; it pulls `ext-msg-core` via `meta.requires_packages`:

```bash
x07 pkg add ext-msg-kafka-c --sync
# or:
x07 pkg add ext-msg-amqp-c --sync
```

If you pin `ext-msg-core` manually and get a version conflict, remove it and re-lock:

```bash
x07 pkg remove ext-msg-core --sync
```

```bash
# Pick NAME@VERSION from /agent/latest/catalog/capabilities.json.
x07 pkg add NAME@VERSION --sync
```

## Local broker fixtures (Docker Compose)

For readiness checks and local development, prefer a fixed-port, local-only broker setup:

- Kafka-compatible (Redpanda): `127.0.0.1:9092`
- AMQP 0-9-1 (RabbitMQ): `127.0.0.1:5672`

Copy/paste:

```bash
cd docs/examples/messaging_brokers
docker compose up -d
docker compose ps
```

Sandboxed run examples (explicit allowlist):

```bash
# Kafka / Redpanda
x07 run --profile sandbox --allow-host 127.0.0.1:9092 -- <your-args...>

# AMQP / RabbitMQ
x07 run --profile sandbox --allow-host 127.0.0.1:5672 -- <your-args...>
```

See `docs/examples/messaging_brokers/README.md` for service details and teardown commands.

## Expert (operations)

- Prefer `run-os-sandboxed` for broker access in CI; relax to `run-os` only when the execution host is trusted and locked down.
- Keep broker credentials in secrets and reference them from `arch/msg/**` profiles; do not embed secrets in modules or cassettes.
