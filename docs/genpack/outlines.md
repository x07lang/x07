# Outlines Cookbook (x07 genpack)

Export the schema first — see [Export the generation artifacts](index.md#export-the-generation-artifacts).

## Recipe: JSON Schema path

```python
import pathlib
import outlines

schema_str = pathlib.Path("/tmp/x07ast.schema.json").read_text(encoding="utf-8")
generate = outlines.generate.json(model, schema_str)
ast = generate("Generate a valid x07AST module for ...")
```

For schema-driven convergence and whitespace guidance, see the [Whitespace note](index.md#whitespace-note).
