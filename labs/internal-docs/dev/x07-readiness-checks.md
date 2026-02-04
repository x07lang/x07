# X07 readiness checks (2026-02-04)

This note summarizes the readiness reports produced under `/Users/webik/projects/x07-tests/` against toolchain `v0.0.84`, and tracks what was verified as a real toolchain/package issue vs what was a misunderstanding of existing APIs/docs.

## Verified issues (real) and fixes

### Toolchain stdlib: FS path typing (blocker)

**Symptom:** compilation fails when calling `std.fs.read` / `std.world.fs.*` with a `bytes_view` path.

**Root cause:** some stdlib FS wrappers passed `bytes_view` directly into APIs that require owned `bytes`.

**Fix (toolchain):**
- Convert `bytes_view` → `bytes` at the boundary (`view.to_bytes`) in the relevant stdlib wrappers.

### Tooling UX: `x07 test` module-root drift

**Symptom:** generated manifests (for example under `gen/sm/`) required manual `--module-root` flags even when the project already defined `module_roots` in `x07.json`.

**Fix (toolchain):**
- `x07 test` now incorporates project module roots automatically, so manifests under subdirectories remain runnable without manual flags.

### Tooling UX: `x07 arch check --write-lock` mismatch recovery

**Symptom:** users hit `E_ARCH_CONTRACTS_LOCK_MISMATCH` without a clear, deterministic recovery flow.

**Fix (toolchain + docs):**
- Align `x07 arch check --write-lock` behavior and document the canonical workflow.

### Compiler budgets (common blocker)

**Symptom:** large pipelines hit default limits (`max locals`, `max ast nodes`, `max emitted C bytes`) even for valid code.

**Fix (toolchain + docs):**
- Raise defaults to match the CI-gated workloads and document env var overrides (`X07_MAX_LOCALS`, `X07_MAX_AST_NODES`, `X07_MAX_C_BYTES`).

### Stream plugin config docs (common blocker)

**Symptom:** agents could not use `std.stream.xf.plugin_v1` with pinned plugin ids because the `cfg` bytes layout wasn’t documented.

**Fix (docs):**
- Document the `cfg` binary layouts for `xf.split_lines_v1`, `xf.deframe_u32le_v1`, `xf.frame_u32le_v1`, and `xf.json_canon_stream_v1`.

### ext packages: `ext.data_model.toml.emit_canon` bug (blocker)

**Symptom:** TOML emission returned `unsupported_kind` for supported tags due to control-flow fallthrough.

**Fix (ext-data-model `0.1.8`):**
- Correct `_emit_value_or_err` returns for success paths and fix string escaping so it does not double-emit raw characters.

### ext packages: CSV parse bug

**Symptom:** CSV → DataModel parse produced incorrect results due to loop variable reuse in nested loops.

**Fix (ext-csv-rs `0.1.5`):**
- Use distinct loop variables in nested loops.

### ext packages: dependency pin conflicts after bumping ext-data-model

**Symptom:** `x07 pkg lock --offline` failed for example projects with a version conflict: the project used `ext-data-model@0.1.8` while dependencies pinned `ext-data-model@0.1.7` via `meta.requires_packages`.

**Fix (ext packages):**
- Bump the dependent packages to new patch versions with `meta.requires_packages` updated to `ext-data-model@0.1.8`.
- Run `python3 scripts/publish_ext_packages.py sync --write` to update the capability map and example/fixture lockfiles deterministically.

## Misunderstandings / doc gaps found in reports

### “Encode is missing for XML/INI/CSV”

Not verified. The canonical encode APIs are provided by `ext-data-model` (for example `ext.data_model.xml.emit_canon`, `ext.data_model.ini.emit_canon`, `ext.data_model.csv.emit_canon`).

Action: keep the agent quickstart and package docs emphasizing `x07 doc <module-or-symbol>` as the canonical discovery path.

### “ext.streams cannot provide file streaming readers”

Not verified. `ext.streams.fs.open_read` is a thin wrapper over `std.fs.open_read`.

Action: ensure docs/examples mention `ext.streams.fs` explicitly when the goal is file-backed streaming.

## Cross-repo consistency tasks (release checklist)

### x07 (toolchain repo)
- Run `./scripts/ci/check_all.sh`.
- Bump toolchain to the next version using `scripts/bump_toolchain_version.py`, and tag that version.
- If any ext packages were bumped/added, regenerate `locks/external-packages.lock` and ensure `catalog/capabilities.json` points at the latest versions.

### x07-website (x07lang.org)
- Generate a docs bundle from the toolchain repo and sync it into `docs/vX.Y.Z` + `docs/latest`.
- Sync agent portal content (schemas/skills/stdlib index/packages index) for the same toolchain version.
- Run `python3 scripts/check_site.py --check`.

### x07-registry / x07-registry-web (x07.io)
- After tagging the toolchain, bump `x07-registry` git tag dependencies (`x07c`, `x07-worlds`) to that tag and run `cargo test`.
- No changes expected in `x07-registry-web` for these readiness fixes; verify against the latest registry API after deployment.

## Follow-ups (optional, not required for the immediate release)

- Publish a codec capability matrix (parse/emit/streaming semantics/known lossy behavior) for the ext codecs used in readiness prompts.
- Consider a small stdlib helper for framing u32-length items to reduce manual byte writes in tests.
- Improve `x07 test` failures to optionally surface ERR-doc payloads (err_code/msg + structured payload when present).
