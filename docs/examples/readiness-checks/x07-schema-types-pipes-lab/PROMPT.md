# Prompt 2 — Schema derive + types/brands + pipes integration lab

```text
You are an X07 coding agent building `x07-schema-types-pipes-lab`.

GOAL
Build an executable `types_pipes.main` that proves the full “types + brands + pipe elaboration” story:
- generate types/validators from schema via `x07 schema derive`
- feed those validators into a brand registry
- build a pipe that relies on compile-time brand inference AND auto-inserted runtime validation
- produce a canonical JSON report + deterministic tests + drift checks

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir x07-schema-types-pipes-lab
cd x07-schema-types-pipes-lab
x07 init

DEPENDENCIES (lock)
- ext-data-model (required by schema-derived modules)
- ext-json-rs (canonical JSON helpers where needed)
- ext-cli

SCHEMA DERIVE REQUIREMENTS
1) Create TWO schema files under `schemas/`:
   - `schemas/frame_payload.x07schema.json`
   - `schemas/event_line.x07schema.json`
2) Run:
   - `x07 schema derive --input schemas/frame_payload.x07schema.json --out-dir gen/frame_payload --write`
   - `x07 schema derive --input schemas/event_line.x07schema.json --out-dir gen/event_line --write`
3) Add the derived module roots to `x07.json.module_roots`:
   - `gen/frame_payload/modules`
   - `gen/event_line/modules`
4) Commit generated outputs and add drift checks:
   - `x07 schema derive --input ... --out-dir ... --check`

TYPES + BRANDS REQUIREMENTS
1) Import the generated modules and build a central brand registry module that re-exports brand ids + validators.
2) Demonstrate both:
   - a validator used directly in `std.stream.xf.require_brand_v1`
   - a validator resolved indirectly via `meta.brands_v1` / `brand_registry_ref_v1` (omit the validator in `require_brand_v1` and let elaboration resolve it)

PIPES REQUIREMENTS
1) Build a pipe that processes u32-framed items:
   - source: bytes source containing u32le-framed items
   - chain: deframe → require_brand (auto inserted by elaborator) → map_bytes (brand-inferred) → frame → collect
2) Pipe cfg MUST include:
   - typecheck_item_brands_v1=1
   - auto_require_brand_v1=1
   - verify_produced_brands_v1=1
   - brand_registry_ref_v1 pointing to your registry module
   - emit_stats=1, emit_payload=1
3) Include a second pipe that uses plugin_id "xf.json_canon_stream_v1" on a JSON-lines stream and shows byte-stable canonicalization.

ARCH CHECK (contracts)
1) Add `arch/manifest.x07arch.json` enabling at least:
   - contracts_v1.stream_plugins (since you use plugin stages)
   - contracts_v1.canonical_json (your report output)
2) Run `x07 arch check --write-lock` and commit:
   - arch/manifest.lock.json
   - arch/contracts.lock.json

TESTS
- Provide a `fixtures/` corpus containing:
  - valid framed payloads
  - invalid payloads (to prove brand validation triggers ERR doc)
  - JSON-lines corpus for json_canon stream stage
- Add deterministic tests that:
  - run the pipe twice and compare output bytes exactly
  - validate that ERR docs surface stable err codes and messages

OUTPUT
- One canonical JSON report that includes:
  - schema versions used
  - which brands were enforced
  - pipe stats (bytes/items) extracted from the pipe doc
  - pass/fail + stable error codes
```

