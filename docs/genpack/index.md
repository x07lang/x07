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
