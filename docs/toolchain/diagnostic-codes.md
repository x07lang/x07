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
- `ETEST_WORLD_INVALID`: invalid `tests[i].world` (only `solve-*` worlds are allowed).
- `ETEST_ENTRY_INVALID`: invalid `tests[i].entry` (must be `module.symbol`).
- `ETEST_EXPECT_INVALID`: invalid `tests[i].expect` (allowed: `pass`, `fail`, `skip`).
- `ETEST_RETURNS_INVALID`: invalid `tests[i].returns` (allowed: `result_i32`, `bytes_status_v1`).
- `ETEST_TIMEOUT_INVALID`: invalid `tests[i].timeout_ms` (must be `>= 1` when present).
- `ETEST_FIXTURE_FORBIDDEN`: `fixture_root` must not be set for `solve-pure`.
- `ETEST_FIXTURE_REQUIRED`: `fixture_root` is required for `solve-fs`, `solve-rr`, `solve-kv`, and `solve-full`.
- `ETEST_FIXTURE_UNSAFE_PATH`: `fixture_root` is an unsafe path (absolute, contains `..`, or contains `\\`).
- `ETEST_FIXTURE_MISSING`: `fixture_root` directory is missing, or (for `solve-full`) required subdirectories are missing.

See: [Test manifest](tests-manifest.md).

## `x07 test` execution (selected)

- `ETEST_COMPILE`: compile failed for the generated test driver or test module.
- `EDETERMINISM`: nondeterminism detected across `--repeat` runs.
- `EBAD_STATUS`: a test returned an invalid `bytes_status_v1` payload.

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

