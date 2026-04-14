# Guide: JSON reporting (DataModel → canonical JSON)

This guide shows the canonical way to produce a **stable JSON report** from an X07 program:

- build a report as an `ext.data_model` map (version it with a `schema_version` field)
- emit **canonical JSON** (stable key ordering and formatting)
- optionally pretty-print for humans

See the runnable reference project: `docs/examples/agent-gate/json-report/`.

## Canonical workflow

### Why DataModel first

X07’s ecosystem uses `ext.data_model` as the common “structured value” representation. Using it for reports means:

- predictable, validated shapes (map/seq/string/number/bool)
- shared encode/decode tooling across codecs (`json`, `yaml`, `toml`, `cbor`, …)
- one canonical place to standardize error/report envelopes

### Emit canonical JSON

To emit canonical JSON from a DataModel *doc*:

- `ext.data_model.json.emit_canon(doc) -> bytes`
  - returns a **result doc** (OK/ERR) in bytes form
  - OK payload bytes are the JSON bytes

The reference project uses a small helper that:

1. calls `ext.data_model.json.emit_canon`
2. checks `ext.data_model.doc_is_err`
3. strips the 1-byte OK tag and returns the payload bytes

### Pretty JSON (optional)

Once you have canonical JSON bytes, you can pretty-print deterministically:

- `std.json.pretty_encode(json_bytes_view) -> bytes`

Canonicalize first (so key order is stable), then pretty-print.

### Reference implementation

Starting points:

- Template (new project): `x07 init --template json-report`
- Repo reference (includes a focused test suite): `docs/examples/agent-gate/json-report/`

- In the repo reference project, run: `x07 test --manifest tests/tests.json`
- `x07 run --profile os -- tool --count 3 --label hello --pretty false`

`x07 run` prints a machine-readable run report. The program output bytes are in `solve.solve_output_b64`.

Extract (portable):

```bash
x07 run --report wrapped --report-out run.wrapped.json --profile os -- tool --count 3 --label hello --pretty false
python3 - <<'PY'
import base64, json
doc = json.load(open("run.wrapped.json", "r", encoding="utf-8"))
runner = doc["report"] if doc.get("schema_version") == "x07.run.report@0.3.0" else doc
print(base64.b64decode(runner["solve"]["solve_output_b64"]).decode("utf-8", errors="replace"), end="")
PY
```

Output is a JSON object with a `schema_version` field:

```json
{"count":3,"label":"hello","pretty":false,"schema_version":"x07.example.json_report@0.1.0"}
```

## Expert notes

- Treat emitted JSON as part of an interface: version the shape (`schema_version`) and keep it stable.
- Prefer DataModel construction over string-building JSON by hand; it keeps key ordering and escaping deterministic.
- For machine consumers, prefer canonical JSON; pretty JSON is for humans.
