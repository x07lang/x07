# XGrammar Cookbook (x07 genpack)

Use the `x07` CLI as the canonical source of x07AST generation artifacts.

## Recipe A: compile from JSON Schema (preferred)

```bash
x07 ast schema > /tmp/x07ast.schema.json
```

```python
import pathlib
import xgrammar as xgr

schema_str = pathlib.Path("/tmp/x07ast.schema.json").read_text(encoding="utf-8")
compiler = xgr.GrammarCompiler(tokenizer_info=...)
compiled = compiler.compile_json_schema(schema_str)
```

## Recipe B: compile from the shipped grammar bundle

```bash
x07 ast grammar --cfg > /tmp/x07ast.grammar_bundle.json
```

```python
import json
import pathlib
import xgrammar as xgr

bundle = json.loads(pathlib.Path("/tmp/x07ast.grammar_bundle.json").read_text(encoding="utf-8"))
min_cfg = next(v["cfg"] for v in bundle["variants"] if v["name"] == "min")
compiler = xgr.GrammarCompiler(tokenizer_info=...)
compiled = compiler.compile_grammar(min_cfg)
```

## Whitespace note

Use the `min` variant for smaller models and higher throughput. The `pretty` variant is included for readability/debugging and may increase repetition risk on weak decoders.
