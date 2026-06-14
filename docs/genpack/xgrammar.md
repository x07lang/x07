# XGrammar Cookbook (x07 genpack)

Export the schema and grammar bundle first — see [Export the generation artifacts](index.md#export-the-generation-artifacts).

## Recipe A: compile from JSON Schema (preferred)

```python
import pathlib
import xgrammar as xgr

schema_str = pathlib.Path("/tmp/x07ast.schema.json").read_text(encoding="utf-8")
compiler = xgr.GrammarCompiler(tokenizer_info=...)
compiled = compiler.compile_json_schema(schema_str)
```

## Recipe B: compile from the shipped grammar bundle

```python
import json
import pathlib
import xgrammar as xgr

bundle = json.loads(pathlib.Path("/tmp/x07ast.grammar_bundle.json").read_text(encoding="utf-8"))
min_cfg = next(v["cfg"] for v in bundle["variants"] if v["name"] == "min")
compiler = xgr.GrammarCompiler(tokenizer_info=...)
compiled = compiler.compile_grammar(min_cfg)
```

For the `min` vs `pretty` variant tradeoff, see the [Whitespace note](index.md#whitespace-note).
