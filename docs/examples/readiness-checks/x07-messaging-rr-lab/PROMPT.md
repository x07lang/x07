# Prompt 9 — Messaging + rr determinism + obs metrics

```text
You are an X07 coding agent building `x07-messaging-rr-lab`.

GOAL
Exercise the messaging stack + deterministic record/replay:
- real broker integration in OS mode (Kafka OR AMQP)
- deterministic replay in solve-rr mode using recorded transcripts
- canonical JSON + OpenMetrics output for CI

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir x07-messaging-rr-lab
cd x07-messaging-rr-lab
x07 init --template worker

DEPENDENCIES (lock)
Core:
- ext-msg-core (std.msg.* including std.msg.rr)
Driver (choose one; you may include both if time permits):
- ext-msg-kafka-c
- ext-msg-amqp-c
Observability:
- ext-obs
- ext-tracing
- ext-log
CLI:
- ext-cli
- ext-cli-ux
Data model:
- ext-data-model
- ext-json-rs

LOCAL BROKER FIXTURES
- Use the Docker Compose fixtures in `x07/docs/examples/messaging_brokers/`.
- Fixed loopback ports:
  - Kafka (Redpanda): 127.0.0.1:9092
  - AMQP (RabbitMQ): 127.0.0.1:5672

PROFILES
1) os_record: run-os-sandboxed
   - policy allows ONLY loopback connections to the local broker port(s)
   - allows write to `.x07_rr/` and `out/`
2) rr_replay: solve-rr
   - no network
   - reads `.x07_rr/` cassette
3) ci_replay: solve-rr (same as rr_replay, used by CI)

FUNCTIONAL REQUIREMENTS
A) OS mode (record):
- Produce N messages with a deterministic envelope:
  - key, headers, payload (payload is canonical JSON)
- Consume them and process into a deterministic aggregate (counts, hashes).
- Record publish/deliver interactions using std.msg.rr helpers.
- Wrap the whole run in std.rr.with_policy_v1 so the capture is a proper rr cassette.

B) Replay mode:
- Do NOT touch the network.
- Parse the recorded transcript and replay the same processing.
- Assert that aggregate output bytes are identical to OS mode for the same transcript.

C) Observability:
- Maintain metrics for produced/consumed counts and processing latency buckets.
- Emit:
  - canonical JSON report
  - OpenMetrics text snapshot

ARCH CONTRACTS
- Add arch/manifest.x07arch.json enabling:
  - contracts_v1.msg (and msg_kafka/msg_amqp depending on driver)
  - contracts_v1.rr
  - contracts_v1.canonical_json
- Run `x07 arch check --write-lock` and commit locks.

TESTS
- CI must run replay-only mode deterministically (no broker required).
- Provide README instructions to run the local broker and record a new cassette.
```

