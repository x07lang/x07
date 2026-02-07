# Outlines Cookbook (x07 genpack)

Use the `x07` schema export directly with Outlines JSON generation.

## Recipe: JSON Schema path

```bash
x07 ast schema > /tmp/x07ast.schema.json
```

```python
import pathlib
import outlines

schema_str = pathlib.Path("/tmp/x07ast.schema.json").read_text(encoding="utf-8")
generate = outlines.generate.json(model, schema_str)
ast = generate("Generate a valid x07AST module for ...")
```

## Whitespace note

Prefer schema-driven generation and then run `x07 fmt` / `x07 lint` / `x07 fix` for deterministic convergence. Keep whitespace controls conservative for smaller models to avoid repetition loops.
