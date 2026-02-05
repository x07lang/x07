# Prompt 3 — Data model + full codec roundtrip laboratory

```text
You are an X07 coding agent building a “data interchange torture test” project named `x07-data-interop-lab`.

GOAL
Build an executable `interop.main` that:
1) accepts input bytes + argv_v1 args: --in-format and --out-dir
2) parses the input into ext.data_model
3) roundtrips the same logical data through ALL supported codecs, producing artifacts in out-dir
4) verifies semantic equivalence (or documented lossy behavior) and prints a canonical JSON report.

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir myapp
cd myapp
x07 init

DEPENDENCIES (add via x07 pkg add … --sync and lock)
- ext-data-model
- ext-json-rs
- ext-toml-rs
- ext-yaml-rs
- ext-xml-rs
- ext-ini-rs
- ext-csv-rs
- ext-cbor-rs
- ext-msgpack-rs
- ext-pb-rs
- ext-streams (stream readers)
- ext-cli (argv parsing)

See also: [Data interoperability guide](../guides/data-interop.md) (codec entrypoints map + canonical roundtrip strategy).

REQUIREMENTS
A) Use ext.data_model as the canonical internal representation.
B) For each format, implement:
   - decode(bytes) -> data_model
   - encode(data_model) -> bytes
   - verify roundtrip invariants and emit per-format diagnostics
C) Enforce canonical encodings where the ecosystem provides them (use _v1 / canon modules where available).
D) Include at least one large/streaming test: read from a stream reader (ext.streams) and use std.io.bufread.

TESTS
- Provide a corpus folder `fixtures/` containing representative documents:
  - nested maps/arrays, unicode text, numbers, edge cases
  - include at least one case that triggers known differences (e.g., CSV typing ambiguity) and explicitly mark it “expected lossy”.
- Provide `tests/tests.json` with multiple entries that call `interop.tests.*` entrypoints under run-os-sandboxed with a policy that allows:
  - read-only access to fixtures/
  - write access only to a temp out-dir under repo (e.g., out/)
  - no network.

OUTPUT
- Write artifacts: out-dir/{json,toml,yaml,xml,ini,csv,cbor,msgpack,pb}/…
- Print one canonical JSON report summarizing:
  - per-format parse/encode success
  - equivalence checks
  - byte sizes and timing counters
```

