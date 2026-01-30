# Schema derive

`x07 schema derive` generates schema-specific X07 modules (encoders/validators/accessors) plus deterministic tests from a pinned `x07schema.*` JSON file.

## Command

- `x07 schema derive --input <schema.x07schema.json> --out-dir <dir> --write`
  - Writes generated outputs under `<dir>/modules/**` and `<dir>/tests/tests.json`.
- `x07 schema derive --input <schema.x07schema.json> --out-dir <dir> --check`
  - Fails if any generated output would change (drift check).
- `x07 schema derive ... --report-json`
  - Prints a machine-readable derive report to stdout (`schema_version: "x07.schema.derive.report@0.1.0"`).

`--write` and `--check` are mutually exclusive.

## Input schema

Supported schema versions:

- `x07schema.specrows@0.1.0`
- `x07schema.specrows@0.2.0`

The tool accepts either a `types`-based schema or a `rows`-based schema (a flat, ordered table).

Notes:

- Boundary encoding is pinned to `ext.data_model.doc_v1`.
- Example values for `number` fields must be JSON strings (decimal bytes), not JSON numbers.

## Output layout

For each schema type, the tool emits:

- a runtime module: `modules/<pkg>/schema/<type_path>_vN.x07.json`
- a test module: `modules/<pkg>/schema/<type_path>_vN/tests.x07.json`

And a test manifest:

- `tests/tests.json` (run with `x07 test --manifest tests/tests.json`)

## Canonicalization tightening (`x07schema.specrows@0.2.0`)

In `x07schema.specrows@0.2.0`, derived validators and encoders enforce canonical DataModel docs:

- root must be `doc_ok(map(...))`
- map keys must be sorted lexicographically by raw bytes and must be unique
- `number` and `seq:number` fields must declare `number_style_v1`:
  - `uint_ascii_v1`: digits only; no leading zeros unless exactly `"0"`
  - `int_ascii_v1`: optional `-` then digits; no leading zeros unless exactly `"0"`; forbids `"-0"`

Derived tests include deterministic negative vectors for:

- non-canonical map ordering
- duplicate keys
- non-canonical number representations (validator + encoder rejection)

