# ext-data-model

`ext-data-model` is the canonical package for **structured values** in the X07 ecosystem.

It defines a compact byte encoding for a small set of kinds (map/seq/string/number/bool/bytes/…) and standard helpers for:

- building values
- inspecting values
- emitting/consuming “result docs” (OK/ERR envelopes)

See also:

- Guide: [Data interoperability](../guides/data-interop.md)
- Guide: [JSON reporting](../guides/json-reporting.md)
- Runnable reference: `docs/examples/agent-gate/json-report/`

## Result docs (OK/ERR envelopes)

Many `ext.*` codec entrypoints return a deterministic “result doc” in bytes form:

- OK: first byte is `1`, followed by payload bytes
- ERR: first byte is `0` (use `ext.data_model.doc_error_code` / `ext.data_model.doc_error_msg`)

## Common exports (module: `ext.data_model`)

Typical constructors:

- `ext.data_model.value_string(b: bytes_view) -> bytes`
- `ext.data_model.value_number(b: bytes_view) -> bytes` (decimal ASCII)
- `ext.data_model.value_bool(x: i32) -> bytes` (`0` or `1`)
- `ext.data_model.value_map_from_entries(entries: bytes_view) -> bytes`
- `ext.data_model.doc_ok(value_bytes: bytes_view) -> bytes`

Typical inspectors/accessors:

- `ext.data_model.doc_is_err(doc: bytes_view) -> i32`
- `ext.data_model.root_kind(doc: bytes_view) -> i32`
- `ext.data_model.root_offset(doc: bytes_view) -> i32`
- `ext.data_model.map_find(doc: bytes_view, map_off: i32, key: bytes_view) -> i32`
- `ext.data_model.kind_at(doc: bytes_view, off: i32) -> i32`
- `ext.data_model.string_get(doc: bytes_view, off: i32) -> bytes`

For the full surface, use `x07 doc ext.data_model` and `x07 doc ext.data_model.<submodule>`.

## JSON emission (module: `ext.data_model.json`)

- `ext.data_model.json.emit_canon(doc: bytes_view) -> bytes`

This returns a result doc; for OK, the payload bytes are canonical JSON.

