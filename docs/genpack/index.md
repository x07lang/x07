# x07 Generation Pack

Generation-pack exports make x07AST a first-class target for structured generation engines.

Canonical commands:

- `x07 ast schema`
- `x07 ast grammar --cfg`
- `x07 ast grammar --cfg --out-dir <dir>`

SDK packages:

- Python: `sdk/genpack-py` (`x07-genpack`)
- TypeScript: `sdk/genpack-ts` (`@x07/genpack`)

Cookbooks:

- [XGrammar](xgrammar.md)
- [Outlines](outlines.md)

MCP-friendly note:

- `x07 ast schema` and `x07 ast grammar --cfg` intentionally emit tool-style, schema-first payloads so they can be exposed as MCP tools without changing formats.

## Export the generation artifacts

Use the `x07` CLI as the canonical source of x07AST generation artifacts. Every engine cookbook starts from these exports.

Emit the canonical JSON Schema:

```bash
x07 ast schema > /tmp/x07ast.schema.json
```

Emit the grammar bundle for constrained-decoding runtimes:

```bash
x07 ast grammar --cfg > /tmp/x07ast.grammar_bundle.json
```

Write the bundle variants to a directory instead of stdout:

```bash
x07 ast grammar --cfg --out-dir /tmp/x07ast-grammar
```

## Whitespace note

The grammar bundle ships `min` and `pretty` variants. Use the `min` variant for smaller models and higher throughput. The `pretty` variant is included for readability/debugging and may increase repetition risk on weak decoders.

Prefer schema-driven generation and then run `x07 fmt` / `x07 lint` / `x07 fix` for deterministic convergence. Keep whitespace controls conservative for smaller models to avoid repetition loops.
