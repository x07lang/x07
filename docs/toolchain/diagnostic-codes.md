# Diagnostic codes (common)

X07 uses stable diagnostic identifiers so failures are searchable and agent-recoverable.

This page documents the codes you’ll most commonly see in front-door commands.

## `x07 test` manifest validation (`ETEST_*`)

These codes appear when `tests/tests.json` (or `--manifest ...`) is invalid.

- `ETEST_MANIFEST_IO`: manifest file could not be read.
- `ETEST_MANIFEST_JSON`: manifest is not valid JSON.
- `ETEST_SCHEMA_VERSION`: `schema_version` is not `x07.tests_manifest@0.1.0`.
- `ETEST_TESTS_EMPTY`: `tests[]` is empty.
- `ETEST_ID_EMPTY`: `tests[i].id` is empty.
- `ETEST_ID_NON_ASCII`: `tests[i].id` is not ASCII-printable.
- `ETEST_ID_DUPLICATE`: duplicate `tests[i].id`.
- `ETEST_WORLD_INVALID`: invalid `tests[i].world` (allowed: `run-os`, `run-os-sandboxed`).
- `ETEST_ENTRY_INVALID`: invalid `tests[i].entry` (must be `module.symbol`).
- `ETEST_EXPECT_INVALID`: invalid `tests[i].expect` (allowed: `pass`, `fail`, `skip`).
- `ETEST_RETURNS_INVALID`: invalid `tests[i].returns` (allowed: `result_i32`, `bytes_status_v1`).
- `ETEST_TIMEOUT_INVALID`: invalid `tests[i].timeout_ms` (must be `>= 1` when present).
- `ETEST_FIXTURE_FORBIDDEN`: `fixture_root` must not be set for OS worlds.

See: [Test manifest](tests-manifest.md).

## `x07 test` execution (selected)

- `ETEST_COMPILE`: compile failed for the generated test driver or test module.
- `EDETERMINISM`: nondeterminism detected across `--repeat` runs.
- `EBAD_STATUS`: a test returned an invalid `bytes_status_v1` payload.
- `ETEST_NO_RUN_UNSUPPORTED`: `--no-run` is not supported for OS-world tests.
- `ETEST_OS_RUNNER_JSON`: `x07-os-runner` did not emit a valid JSON report.

## `x07 run` common errors (stderr prefixes)

These codes appear as `[...]` prefixes in `x07 run` stderr output:

- `X07PROJECT_READ`: failed to read `x07.json`.
- `X07PROJECT_PARSE`: invalid `x07.json` JSON.
- `X07LOCK_READ`: failed to read `x07.lock.json` (hint: run `x07 pkg lock`).
- `X07LOCK_PARSE`: invalid `x07.lock.json` JSON (hint: run `x07 pkg lock`).
- `X07LOCK_SCHEMA`: lockfile schema mismatch (hint: run `x07 pkg lock`).
- `X07ENTRY_READ`: failed to read the project `entry` module (hint: check `x07.json` `entry`).

## `x07 pkg` JSON errors (`error.code`)

`x07 pkg` commands emit JSON reports; on failure, look at `error.code`:

- `X07PKG_SPEC_INVALID`: invalid `NAME@VERSION` spec (versions must be SemVer).
- `X07PKG_DEP_EXISTS`: dependency already present in `x07.json`.
- `X07PKG_INDEX_CONFIG`: invalid index URL / configuration.
- `X07PKG_INDEX_FETCH`: could not fetch entries from the index.
- `X07PKG_INDEX_NO_MATCH`: no non-yanked entry matched `name@version`.
- `X07PKG_DOWNLOAD_FAILED`: failed to download an archive from the index.
- `X07PKG_LOCK_MISSING`: missing lockfile.
- `X07PKG_LOCK_MISMATCH`: lockfile out of date.
- `X07PKG_TRANSITIVE_MISSING`: transitive dependencies missing from `x07.json`.

## `x07 arch check` (selected `E_ARCH_*`)

Manifest / lock / scan:

- `E_ARCH_MANIFEST_READ`: failed to read the manifest file.
- `E_ARCH_MANIFEST_JSON_PARSE`: manifest is not valid JSON.
- `E_ARCH_MANIFEST_SCHEMA_INVALID`: manifest JSON does not match `x07.arch.manifest@0.1.0`.
- `E_ARCH_MANIFEST_INVALID`: manifest is schema-valid but semantically unsupported (for example, an invalid `world`).
- `E_ARCH_LOCK_READ`: failed to read the lock file.
- `E_ARCH_LOCK_JSON_PARSE`: lock is not valid JSON.
- `E_ARCH_LOCK_SCHEMA_INVALID`: lock JSON does not match `x07.arch.manifest.lock@0.1.0`.
- `E_ARCH_LOCK_INVALID`: lock is schema-valid but semantically unusable.
- `E_ARCH_LOCK_MISMATCH`: manifest path/hash does not match the lock.
- `E_ARCH_MODULE_PARSE`: failed to parse a scanned `*.x07.json` module.
- `E_ARCH_DUPLICATE_MODULE_ID`: the scan found duplicate `module_id` values across files.
- `E_ARCH_TOOL_BUDGET_EXCEEDED`: tool budget exceeded (exit code `4`).

Architecture checks:

- `E_ARCH_NODE_ORPHAN_MODULE`: a module matched 0 manifest nodes (when orphans are denied).
- `E_ARCH_NODE_OVERLAP_MODULE`: a module matched multiple manifest nodes.
- `E_ARCH_EXTERNAL_IMPORT_NOT_ALLOWED`: external import not allowed by `externals.*`.
- `E_ARCH_IMPORT_PREFIX_DENIED`: import denied by node `imports.deny_prefixes`.
- `E_ARCH_IMPORT_PREFIX_NOT_ALLOWED`: import not allowed by node `imports.allow_prefixes`.
- `E_ARCH_VISIBILITY`: importing node not allowed by target node visibility.
- `E_ARCH_DEPS_DENY`: node edge denied by a `deps_v1` rule.
- `E_ARCH_LAYERS_VIOLATION`: node edge violates a `layers_v1` rule.
- `E_ARCH_CYCLE`: cyclic node dependency detected.
- `E_ARCH_EDGE_NOT_ALLOWED`: allowlist mode is enabled and the node edge is not explicitly allowed.
- `E_ARCH_WORLD_EDGE_FORBIDDEN`: `solve-*` node depends on `run-os*` node.
- `E_ARCH_SMOKE_MISSING`: node is missing a required `contracts.smoke_entry` export.
- `E_ARCH_PUBLIC_BYTES_UNBRANDED`: public node exports a `bytes` boundary without a brand.

Stream plugins contracts (`contracts_v1.stream_plugins`):

- `E_ARCH_STREAM_PLUGINS_DIR_MISSING`: stream plugins contract root directory is missing.
- `E_ARCH_STREAM_PLUGINS_GEN_DIR_MISSING`: stream plugins `gen_dir` is missing (when `require_registry_uptodate=true`).
- `E_ARCH_STREAM_PLUGINS_INDEX_READ`: failed to read the stream plugins index file.
- `E_ARCH_STREAM_PLUGINS_INDEX_JSON_PARSE`: index is not valid JSON.
- `E_ARCH_STREAM_PLUGINS_INDEX_SCHEMA_VERSION`: index `schema_version` mismatch.
- `E_ARCH_STREAM_PLUGINS_INDEX_SCHEMA_INVALID`: index JSON does not match the pinned schema.
- `E_ARCH_STREAM_PLUGINS_INDEX_NOT_SORTED`: index `plugins[]` is not sorted by `plugin_id`.
- `E_ARCH_STREAM_PLUGINS_INDEX_DUPLICATE_ID`: duplicate `plugin_id` in the index.
- `E_ARCH_STREAM_PLUGIN_BUDGET_MISSING`: index entry references a missing `budget_profile_id`.
- `E_ARCH_STREAM_PLUGIN_SPEC_READ`: failed to read a referenced plugin spec file.
- `E_ARCH_STREAM_PLUGIN_SPEC_JSON_PARSE`: spec is not valid JSON.
- `E_ARCH_STREAM_PLUGIN_SPEC_SCHEMA_VERSION`: spec `schema_version` mismatch.
- `E_ARCH_STREAM_PLUGIN_SPEC_SCHEMA_INVALID`: spec JSON does not match the pinned schema.
- `E_ARCH_STREAM_PLUGIN_SPEC_ID_MISMATCH`: spec `plugin_id` does not match the index entry.
- `E_ARCH_STREAM_PLUGIN_SPEC_CFG_CANON_MODE`: spec has an invalid cfg canonicalization mode.
- `E_ARCH_STREAM_PLUGIN_SPEC_LIMITS_INVALID`: spec has invalid output limit fields.
- `E_ARCH_STREAM_PLUGIN_SPEC_ABI_MISMATCH`: spec ABI fields do not match the index entry.
- `E_ARCH_STREAM_PLUGIN_SPEC_BUDGETS_MISMATCH`: spec budgets do not match the index entry.
- `E_ARCH_STREAM_PLUGIN_SPEC_BRANDS_MISMATCH`: spec brands do not match the index entry.
- `E_ARCH_STREAM_PLUGIN_SPEC_DETERMINISM_MISMATCH`: spec determinism does not match the index entry.
- `E_ARCH_STREAM_PLUGIN_SPEC_WORLDS_MISMATCH`: spec `worlds_allowed` does not match the index entry.
- `E_ARCH_STREAM_PLUGIN_NOT_FOUND`: a module references a `std.stream.xf.plugin_v1` id that is not declared.
- `E_ARCH_STREAM_PLUGIN_WORLD_VIOLATION`: plugin usage is forbidden in the module’s world.
