# x07AST structured editing (`x07 ast`)

Agents must not hand-author deeply nested `*.x07.json` documents. Use RFC 6902 JSON Patch ops applied to a known-good base.

Agent-facing contract tooling is `x07c` (`fmt`/`lint`/`fix` + `apply-patch`). `x07 ast` exists as a repo-friendly structured editing helper (notably `--validate` and `--x07diag`).

## Workflow

1. Create a valid base document:

   - Entry skeleton: `cargo run -p x07 -- ast init --world solve-pure --module main --kind entry --out main.x07.json`
   - Module skeleton: `cargo run -p x07 -- ast init --world solve-pure --module ext.foo --kind module --out foo.x07.json`

2. (Optional) Canonicalize any existing x07AST file (RFC 8785 / JCS semantics):

   - `cargo run -p x07 -- ast canon --in path/to/module.x07.json --out path/to/module.x07.json`

3. Write a JSON Patch file (RFC 6902). Prefer adding a `test` op first to prevent patch drift.

   Example `patch.json`:

   ```json
   [
     {"op":"test","path":"/module_id","value":"main"},
     {"op":"add","path":"/imports/-","value":"std.bytes"}
   ]
   ```

4. Apply the patch and validate the result:

   - `cargo run -p x07 -- ast apply-patch --in main.x07.json --patch patch.json --out main.x07.json --validate`

5. Validate and emit an x07diag report (JSON Pointers on failures):

   - `cargo run -p x07 -- ast validate --in main.x07.json --x07diag out.x07diag.json`

## Generation pack exports (constrained decoding)

Use `x07 ast` as the canonical export surface for tool-builders:

- Schema export:
  - `cargo run -p x07 -- ast schema`
- Grammar bundle export:
  - `cargo run -p x07 -- ast grammar --cfg`
- Materialize to files (schema/grammar/supplement/manifest):
  - `cargo run -p x07 -- ast grammar --cfg --out-dir target/genpack`
