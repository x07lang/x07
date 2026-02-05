# Guide: Data interoperability (codecs → DataModel)

This guide is a quick “entrypoints map” for converting between external wire formats and X07’s canonical in-memory representation: `ext.data_model`.

## Result docs (OK/ERR)

Most codec entrypoints return a deterministic “result doc” in bytes form:

- OK: first byte is `1`, followed by the payload bytes.
- ERR: first byte is `0` (use `ext.data_model.doc_error_code` / `ext.data_model.doc_error_msg`).

For OK docs, the payload bytes are:

- `payload = doc[1..]` (slice from offset 1 to end)

## Canonical decode/encode entrypoints

Text formats typically follow a split:

- **decode**: `ext.<format>.data_model.parse(...)` → OK payload is a DataModel value encoding
- **encode**: `ext.data_model.<format>.emit_canon(...)` → OK payload is the encoded bytes for that format

Binary formats usually expose both decode and encode in the same package module.

| Format | Decode (bytes → DataModel) | Encode (DataModel → bytes) |
|---|---|---|
| JSON | `ext.json.data_model.parse` | `ext.data_model.json.emit_canon` |
| TOML | `ext.toml.data_model.parse` | `ext.data_model.toml.emit_canon` |
| YAML | `ext.yaml.data_model.parse` | `ext.data_model.yaml.emit_canon` |
| XML | `ext.xml.data_model.parse` | `ext.data_model.xml.emit_canon` |
| INI | `ext.ini.data_model.parse` | `ext.data_model.ini.emit_canon` |
| CSV | `ext.csv.data_model.parse` | `ext.data_model.csv.emit_canon` |
| CBOR | `ext.cbor.data_model.parse_v1` | `ext.cbor.data_model.emit_v1` |
| MessagePack | `ext.msgpack.data_model.parse_v1` | `ext.msgpack.data_model.emit_v1` |
| Protobuf | `ext.pb.data_model.decode_v1` | `ext.pb.data_model.emit_v1` |

Notes:

- For CBOR/MessagePack/Protobuf, the `ext.*.data_model` modules also provide helpers like `is_err`, `err_code`, and `get_bytes` for extracting OK payload bytes.
- For JSON, the parser canonicalizes input first (stable key order, stable formatting) and then produces a DataModel value.

## Canonical roundtrip check (recommended)

To verify semantic equivalence across codecs:

1. Decode input bytes into a DataModel doc (result doc with OK payload = DataModel value bytes).
2. Re-encode from the decoded doc into each output format (result doc with OK payload = encoded bytes).
3. Decode each output back into DataModel, and compare the OK payload bytes (byte equality) or emit a deterministic “lossy” explanation.

## Expert (conventions)

- Treat decoded data as untrusted input: validate the DataModel value against your contracts before acting on it.
- Keep module boundaries stable by passing “result doc” bytes across worlds; decode to structured forms only where needed.
- Prefer comparing canonical DataModel bytes for “semantic equality” across codecs; compare emitted text bytes only when exact formatting is part of the contract.
