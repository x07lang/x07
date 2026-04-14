# ext-json-rs

`ext-json-rs` provides JSON helpers:

- canonicalization (stable key ordering + formatting)
- JSON → DataModel decoding

See also:

- Guide: [Data interoperability](../guides/data-interop.md)
- Guide: [JSON reporting](../guides/json-reporting.md)
- Runnable reference: `docs/examples/agent-gate/json-report/`

## Canonical JSON (module: `ext.json.canon`)

- `ext.json.canon.canonicalize(input: bytes_view) -> bytes`

This produces a canonical JSON encoding suitable for stable hashing, comparisons, or as an input to `std.json.pretty_encode`.

## JSON → DataModel (module: `ext.json.data_model`)

- `ext.json.data_model.parse(input: bytes_view) -> bytes`

This returns a result doc; for OK, the payload bytes are a DataModel *value* encoding.

To emit JSON from a DataModel doc, use `ext.data_model.json.emit_canon` (from `ext-data-model`).

