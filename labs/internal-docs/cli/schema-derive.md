# Schema derive (`x07 schema derive`)

Internal notes for the schema derivation tool (end-user docs live in `docs/toolchain/schema-derive.md`).

## Implementation

- CLI wiring: `crates/x07/src/main.rs` (`schema` subcommand)
- Implementation: `crates/x07/src/schema.rs`

## Inputs

- Supported schema versions:
  - `x07schema.specrows@0.1.0`
  - `x07schema.specrows@0.2.0`
- Accepted top-level shapes:
  - `types`: structured schema objects
  - `rows`: ordered specrows tuples

## Outputs

- Generated modules: `modules/<pkg>/schema/**`
- Generated test manifest: `tests/tests.json`
- Optional report (`--report-json`): `schema_version: "x07.schema.derive.report@0.1.0"`
- Derived runtime modules use branded bytes for validated docs:
  - brand id is derived as `<pkg>.<type_id>_vN`
  - modules export `cast_doc_view_v1(doc: bytes_view) -> result_bytes_view@brand`
  - generated modules include `meta.brands_v1[brand].validate = "<module_id>.validate_doc_v1"`

## Canonicalization (`specrows@0.2.0`)

- `rows` format requires explicit `number_style_v1` for `number` and `seq:number` fields.
- Generated validators reject:
  - non-canonical map ordering
  - duplicate keys
  - non-canonical number encodings (field-scoped code = `err_base + field_id*100 + 14`)
- Generated encoders reject non-canonical number inputs with the same field-scoped code.

## Fixtures / tests

- Schema derive smoke fixtures: `tests/fixtures/schema_derive/*.x07schema.json`
- CLI smoke tests: `crates/x07/tests/cli.rs`
