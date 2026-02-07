# x07-genpack (Python)

`x07-genpack` is a tiny SDK for retrieving x07AST generation-pack artifacts from the `x07` CLI.

It exposes three primary calls:

- `get_x07ast_schema()`
- `get_x07ast_grammar_bundle()`
- `get_x07ast_genpack()`

The client supports:

- `cli` source mode (default)
- `dir` source mode (for pre-materialized artifacts)
- strict schema/version checks with stable error codes
- optional local cache

## Quick start

```python
from x07_genpack import GenpackClient

client = GenpackClient()
genpack = client.get_x07ast_genpack()

print(genpack.schema.sha256_hex)
print(genpack.grammar.variants["min"].cfg)
```

## Materialize artifacts

```python
from pathlib import Path
from x07_genpack import GenpackClient

client = GenpackClient()
client.materialize(Path(".cache/x07-genpack"))
```
