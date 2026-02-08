# Diagnostic codes catalog

This file is generated from `catalog/diagnostics.json` using `x07 diag catalog`.

- total codes: 130
- quickfix support (`sometimes` or `always`): 108
- quickfix coverage: 83.08%

| Code | Origins | Quickfix | Summary |
| ---- | ------- | -------- | ------- |
| `COMPONENT_MISSING` | x07up / lint / error | sometimes | Requested toolchain component is missing. |
| `ECLI_COMPILE_FAILED` | x07 / lint / error | sometimes | CLI specrows tooling diagnostic `ECLI_COMPILE_FAILED`. |
| `ECLI_JSON_PARSE` | x07 / parse / error | sometimes | CLI specrows tooling diagnostic `ECLI_JSON_PARSE`. |
| `ECLI_SCHEMA_INVALID` | x07 / lint / error | sometimes | CLI specrows tooling diagnostic `ECLI_SCHEMA_INVALID`. |
| `ECLI_SEMANTIC` | x07 / lint / error | sometimes | CLI specrows tooling diagnostic `ECLI_SEMANTIC`. |
| `ECLI_TOOL` | x07 / lint / error | sometimes | CLI specrows tooling diagnostic `ECLI_TOOL`. |
| `ETEST_ENTRY_INVALID` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_ENTRY_INVALID`. |
| `ETEST_EXPECT_INVALID` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_EXPECT_INVALID`. |
| `ETEST_FIXTURE_FORBIDDEN` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_FIXTURE_FORBIDDEN`. |
| `ETEST_FIXTURE_MISSING` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_FIXTURE_MISSING`. |
| `ETEST_FIXTURE_REQUIRED` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_FIXTURE_REQUIRED`. |
| `ETEST_FIXTURE_UNSAFE_PATH` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_FIXTURE_UNSAFE_PATH`. |
| `ETEST_ID_DUPLICATE` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_ID_DUPLICATE`. |
| `ETEST_ID_EMPTY` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_ID_EMPTY`. |
| `ETEST_ID_NON_ASCII` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_ID_NON_ASCII`. |
| `ETEST_MANIFEST_IO` | x07 / run / error | never | Diagnostic code `ETEST_MANIFEST_IO`. |
| `ETEST_MANIFEST_JSON` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_MANIFEST_JSON`. |
| `ETEST_POLICY_FORBIDDEN` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_POLICY_FORBIDDEN`. |
| `ETEST_POLICY_MISSING` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_POLICY_MISSING`. |
| `ETEST_POLICY_REQUIRED` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_POLICY_REQUIRED`. |
| `ETEST_POLICY_UNSAFE_PATH` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_POLICY_UNSAFE_PATH`. |
| `ETEST_RETURNS_INVALID` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_RETURNS_INVALID`. |
| `ETEST_SCHEMA_VERSION` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_SCHEMA_VERSION`. |
| `ETEST_TESTS_EMPTY` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_TESTS_EMPTY`. |
| `ETEST_TIMEOUT_INVALID` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_TIMEOUT_INVALID`. |
| `ETEST_WORLD_INVALID` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_WORLD_INVALID`. |
| `E_ARCH_BUDGET_SCOPE_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGET_SCOPE_MISSING`. |
| `E_ARCH_LOCK_READ` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_LOCK_READ`. |
| `E_ARCH_MODULE_PARSE` | x07 / parse / error | sometimes | Architecture contract diagnostic `E_ARCH_MODULE_PARSE`. |
| `E_ARCH_TOOL_BUDGET_EXCEEDED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_TOOL_BUDGET_EXCEEDED`. |
| `W_ARCH_CONTRACTS_LOCK_MISSING` | x07 / lint / warning | sometimes | Architecture contract diagnostic `W_ARCH_CONTRACTS_LOCK_MISSING`. |
| `W_ARCH_CONTRACT_OPAQUE_USAGE` | x07 / lint / warning | sometimes | Architecture contract diagnostic `W_ARCH_CONTRACT_OPAQUE_USAGE`. |
| `X07-ARITY-0000` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-0000`. |
| `X07-ARITY-BEGIN-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-BEGIN-0001`. |
| `X07-ARITY-FOR-0001` | x07c / lint / error | sometimes | `for` has invalid arity. |
| `X07-ARITY-IF-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-IF-0001`. |
| `X07-ARITY-LET-0001` | x07c / lint / error | sometimes | `let`/`set` has invalid arity. |
| `X07-ARITY-RETURN-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-RETURN-0001`. |
| `X07-ARITY-UNSAFE-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-UNSAFE-0001`. |
| `X07-AST-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-AST-0001`. |
| `X07-BORROW-0001` | x07c / lint / error | sometimes | Borrowing view/subview from a temporary expression is invalid. |
| `X07-GENERICS-0001` | x07c / lint / error | sometimes | Type variable is not declared in type_params. |
| `X07-GENERICS-0002` | x07c / lint / warning | always | Unused type parameter. |
| `X07-INTERNAL-0001` | x07 / run / error | never | Internal tool failure `X07-INTERNAL-0001`. |
| `X07-MOVE-0001` | x07c / lint / error | always | `bytes.concat` uses the same identifier on both sides (use-after-move risk). |
| `X07-MOVE-0002` | x07c / lint / error | always | `if` condition and branch both borrow `bytes.view` from the same owner. |
| `X07-POLICY-SCHEMA-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-POLICY-SCHEMA-0001`. |
| `X07-SCHEMA-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-SCHEMA-0001`. |
| `X07-SCHEMA-0002` | x07 / parse / error | always | Unsupported x07AST schema_version. |
| `X07-TAPP-ELAB-0001` | x07c / rewrite / error | always | Typechecker inferred generic type arguments and proposes inserting an explicit `tapp`. |
| `X07-TAPP-INFER-0001` | x07c / type / info | never | Typechecker cannot infer generic type arguments; explicit `tapp` is required. |
| `X07-TOOL-ARGS-0001` | x07 / parse / error | never | Tool wrapper argument error `X07-TOOL-ARGS-0001`. |
| `X07-TOOL-EXEC-0001` | x07 / run / error | never | Tool wrapper execution error `X07-TOOL-EXEC-0001`. |
| `X07-TYPE-0001` | x07c / type / info | never | Unknown identifier during local typechecking. |
| `X07-TYPE-CALL-0001` | x07c / type / info | never | Call refers to an unknown callee during local typechecking. |
| `X07-TYPE-CALL-0002` | x07c / type / info | never | Call argument type mismatch during local typechecking. |
| `X07-TYPE-CALL-0003` | x07c / type / info | never | Call arity mismatch during local typechecking. |
| `X07-TYPE-IF-0001` | x07c / type / info | never | `if` condition must be an `i32` (boolean-as-i32) during local typechecking. |
| `X07-TYPE-IF-0002` | x07c / type / info | never | `if` branches have incompatible types during local typechecking. |
| `X07-TYPE-RET-0001` | x07c / type / info | never | Return expression type does not match the function result type. |
| `X07-TYPE-SET-0001` | x07c / type / info | never | `set` references an unknown local during local typechecking. |
| `X07-TYPE-SET-0002` | x07c / type / info | never | `set` assignment type mismatch during local typechecking. |
| `X07-TYPE-UNIFY-0001` | x07c / type / info | never | Type unification failed during local inference. |
| `X07-UNSAFE-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-UNSAFE-0001`. |
| `X07-WORLD-0001` | x07c / lint / error | always | Program imports capabilities not allowed by the selected world flags. |
| `X07-WORLD-FFI-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-WORLD-FFI-0001`. |
| `X07-WORLD-FS-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-WORLD-FS-0001`. |
| `X07-WORLD-KV-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-WORLD-KV-0001`. |
| `X07-WORLD-OS-0001` | x07c / lint / error | always | std.os imports are forbidden in solve-* worlds. |
| `X07-WORLD-OS-0002` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-WORLD-OS-0002`. |
| `X07-WORLD-RR-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-WORLD-RR-0001`. |
| `X07-WORLD-UNSAFE-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-WORLD-UNSAFE-0001`. |
| `X07-WORLD-UNSAFE-0002` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-WORLD-UNSAFE-0002`. |
| `X07-X07AST-PARSE-0001` | x07 / parse / error | sometimes | Core lint/schema diagnostic `X07-X07AST-PARSE-0001`. |
| `X07INIT_AGENT` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_AGENT`. |
| `X07INIT_AGENT_DIR` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_AGENT_DIR`. |
| `X07INIT_ARGS` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_ARGS`. |
| `X07INIT_CWD` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_CWD`. |
| `X07INIT_EXISTS` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_EXISTS`. |
| `X07INIT_GITIGNORE` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_GITIGNORE`. |
| `X07INIT_IO` | x07 / lint / error | never | Diagnostic code `X07INIT_IO`. |
| `X07INIT_MODULES` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_MODULES`. |
| `X07INIT_PKG_LOCK` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_PKG_LOCK`. |
| `X07INIT_SRC` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_SRC`. |
| `X07INIT_TESTS` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_TESTS`. |
| `X07PKG_API_URL` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_API_URL`. |
| `X07PKG_DEP_EXISTS` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_DEP_EXISTS`. |
| `X07PKG_DEP_NOT_FOUND` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_DEP_NOT_FOUND`. |
| `X07PKG_DOWNLOAD_FAILED` | x07 / lint / error | never | Diagnostic code `X07PKG_DOWNLOAD_FAILED`. |
| `X07PKG_INDEX_CONFIG` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_INDEX_CONFIG`. |
| `X07PKG_INDEX_FETCH` | x07 / lint / error | never | Diagnostic code `X07PKG_INDEX_FETCH`. |
| `X07PKG_INDEX_NO_MATCH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_INDEX_NO_MATCH`. |
| `X07PKG_LOCK_MISMATCH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOCK_MISMATCH`. |
| `X07PKG_LOCK_MISSING` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOCK_MISSING`. |
| `X07PKG_LOGIN_FAILED` | x07 / lint / error | never | Diagnostic code `X07PKG_LOGIN_FAILED`. |
| `X07PKG_LOGIN_TOKEN` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOGIN_TOKEN`. |
| `X07PKG_OFFLINE_MISSING_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_OFFLINE_MISSING_DEP`. |
| `X07PKG_PUBLISH_FAILED` | x07 / lint / error | never | Diagnostic code `X07PKG_PUBLISH_FAILED`. |
| `X07PKG_PUBLISH_RESPONSE` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_PUBLISH_RESPONSE`. |
| `X07PKG_PUBLISH_RESPONSE_MISMATCH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_PUBLISH_RESPONSE_MISMATCH`. |
| `X07PKG_SPEC_INVALID` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_SPEC_INVALID`. |
| `X07PKG_TRANSITIVE_MISSING` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_TRANSITIVE_MISSING`. |
| `X07RR_ENTRY_EXISTS` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_ENTRY_EXISTS`. |
| `X07RR_HTTP` | x07 / run / error | never | Diagnostic code `X07RR_HTTP`. |
| `X07RR_KEY_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_KEY_EMPTY`. |
| `X07RR_KIND_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_KIND_EMPTY`. |
| `X07RR_LATENCY_OUT_OF_RANGE` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_LATENCY_OUT_OF_RANGE`. |
| `X07RR_OP_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_OP_EMPTY`. |
| `X07RR_URL_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_URL_EMPTY`. |
| `X7I0001` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0001`. |
| `X7I0100` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0100`. |
| `X7I0110` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0110`. |
| `X7I0111` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0111`. |
| `X7I0120` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0120`. |
| `X7I0122` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0122`. |
| `X7I0123` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0123`. |
| `X7I0200` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0200`. |
| `X7I0201` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0201`. |
| `X7I0210` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0210`. |
| `X7I0211` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0211`. |
| `X7I0300` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0300`. |
| `X7I0301` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0301`. |
| `X7I0310` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0310`. |
| `X7I0311` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0311`. |
| `X7I0320` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0320`. |
| `X7I0330` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0330`. |
| `X7I0340` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0340`. |
| `X7I0350` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0350`. |
| `X7I0360` | x07import-core / lint / error | sometimes | x07import subset compatibility diagnostic `X7I0360`. |
| `X7I0901` | x07import-core / lint / error | never | Diagnostic code `X7I0901`. |

## `COMPONENT_MISSING`

Summary: Requested toolchain component is missing.

Origins:
- x07up (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The installer/updater can usually remediate by installing the missing component.

Agent strategy:

- Run `x07up` install/update for the missing component.
- Re-run the failed command.


## `ECLI_COMPILE_FAILED`

Summary: CLI specrows tooling diagnostic `ECLI_COMPILE_FAILED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The input spec JSON is parse/schema/semantic-invalid or compile-invalid. Repair is usually deterministic by formatting/fixing schema shape and rerunning spec commands.

Agent strategy:

- Run `x07 cli spec fmt` and `x07 cli spec check`.
- Fix schema or semantic issues in the spec document.
- Re-run compile/check to verify.


## `ECLI_JSON_PARSE`

Summary: CLI specrows tooling diagnostic `ECLI_JSON_PARSE`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The input spec JSON is parse/schema/semantic-invalid or compile-invalid. Repair is usually deterministic by formatting/fixing schema shape and rerunning spec commands.

Agent strategy:

- Run `x07 cli spec fmt` and `x07 cli spec check`.
- Fix schema or semantic issues in the spec document.
- Re-run compile/check to verify.


## `ECLI_SCHEMA_INVALID`

Summary: CLI specrows tooling diagnostic `ECLI_SCHEMA_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The input spec JSON is parse/schema/semantic-invalid or compile-invalid. Repair is usually deterministic by formatting/fixing schema shape and rerunning spec commands.

Agent strategy:

- Run `x07 cli spec fmt` and `x07 cli spec check`.
- Fix schema or semantic issues in the spec document.
- Re-run compile/check to verify.


## `ECLI_SEMANTIC`

Summary: CLI specrows tooling diagnostic `ECLI_SEMANTIC`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The input spec JSON is parse/schema/semantic-invalid or compile-invalid. Repair is usually deterministic by formatting/fixing schema shape and rerunning spec commands.

Agent strategy:

- Run `x07 cli spec fmt` and `x07 cli spec check`.
- Fix schema or semantic issues in the spec document.
- Re-run compile/check to verify.


## `ECLI_TOOL`

Summary: CLI specrows tooling diagnostic `ECLI_TOOL`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The input spec JSON is parse/schema/semantic-invalid or compile-invalid. Repair is usually deterministic by formatting/fixing schema shape and rerunning spec commands.

Agent strategy:

- Run `x07 cli spec fmt` and `x07 cli spec check`.
- Fix schema or semantic issues in the spec document.
- Re-run compile/check to verify.


## `ETEST_ENTRY_INVALID`

Summary: Test manifest validation diagnostic `ETEST_ENTRY_INVALID`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_EXPECT_INVALID`

Summary: Test manifest validation diagnostic `ETEST_EXPECT_INVALID`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_FIXTURE_FORBIDDEN`

Summary: Test manifest validation diagnostic `ETEST_FIXTURE_FORBIDDEN`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_FIXTURE_MISSING`

Summary: Test manifest validation diagnostic `ETEST_FIXTURE_MISSING`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_FIXTURE_REQUIRED`

Summary: Test manifest validation diagnostic `ETEST_FIXTURE_REQUIRED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_FIXTURE_UNSAFE_PATH`

Summary: Test manifest validation diagnostic `ETEST_FIXTURE_UNSAFE_PATH`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_ID_DUPLICATE`

Summary: Test manifest validation diagnostic `ETEST_ID_DUPLICATE`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_ID_EMPTY`

Summary: Test manifest validation diagnostic `ETEST_ID_EMPTY`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_ID_NON_ASCII`

Summary: Test manifest validation diagnostic `ETEST_ID_NON_ASCII`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_MANIFEST_IO`

Summary: Diagnostic code `ETEST_MANIFEST_IO`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires filesystem availability and possibly creating/restoring external files.

Details:

This diagnostic generally requires external state changes or human intent to resolve (`ETEST_MANIFEST_IO`).

Agent strategy:

- Reproduce `ETEST_MANIFEST_IO` and capture structured context.
- Resolve the external dependency/state.
- Re-run the command and continue repair loop.


## `ETEST_MANIFEST_JSON`

Summary: Test manifest validation diagnostic `ETEST_MANIFEST_JSON`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_POLICY_FORBIDDEN`

Summary: Test manifest validation diagnostic `ETEST_POLICY_FORBIDDEN`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_POLICY_MISSING`

Summary: Test manifest validation diagnostic `ETEST_POLICY_MISSING`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_POLICY_REQUIRED`

Summary: Test manifest validation diagnostic `ETEST_POLICY_REQUIRED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_POLICY_UNSAFE_PATH`

Summary: Test manifest validation diagnostic `ETEST_POLICY_UNSAFE_PATH`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_RETURNS_INVALID`

Summary: Test manifest validation diagnostic `ETEST_RETURNS_INVALID`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_SCHEMA_VERSION`

Summary: Test manifest validation diagnostic `ETEST_SCHEMA_VERSION`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_TESTS_EMPTY`

Summary: Test manifest validation diagnostic `ETEST_TESTS_EMPTY`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_TIMEOUT_INVALID`

Summary: Test manifest validation diagnostic `ETEST_TIMEOUT_INVALID`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_WORLD_INVALID`

Summary: Test manifest validation diagnostic `ETEST_WORLD_INVALID`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `E_ARCH_BUDGET_SCOPE_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_BUDGET_SCOPE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_LOCK_READ`

Summary: Architecture contract diagnostic `E_ARCH_LOCK_READ`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MODULE_PARSE`

Summary: Architecture contract diagnostic `E_ARCH_MODULE_PARSE`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_TOOL_BUDGET_EXCEEDED`

Summary: Architecture contract diagnostic `E_ARCH_TOOL_BUDGET_EXCEEDED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `W_ARCH_CONTRACTS_LOCK_MISSING`

Summary: Architecture contract diagnostic `W_ARCH_CONTRACTS_LOCK_MISSING`.

Origins:
- x07 (stage: lint, severity: warning)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `W_ARCH_CONTRACT_OPAQUE_USAGE`

Summary: Architecture contract diagnostic `W_ARCH_CONTRACT_OPAQUE_USAGE`.

Origins:
- x07 (stage: lint, severity: warning)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `X07-ARITY-0000`

Summary: Core lint/schema diagnostic `X07-ARITY-0000`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-ARITY-BEGIN-0001`

Summary: Core lint/schema diagnostic `X07-ARITY-BEGIN-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-ARITY-FOR-0001`

Summary: `for` has invalid arity.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

A quickfix is emitted when extra trailing expressions are present: they are wrapped into a `begin` body so the loop keeps canonical arity.

Agent strategy:

- If quickfix is present, apply it to wrap extra body expressions.
- Otherwise rewrite to `for <id> <iter> <init> <body>` manually.
- Re-run lint.


## `X07-ARITY-IF-0001`

Summary: Core lint/schema diagnostic `X07-ARITY-IF-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-ARITY-LET-0001`

Summary: `let`/`set` has invalid arity.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

A quickfix is emitted when extra trailing expressions exist: the assignment is rewritten into a `begin` block preserving later expressions.

Agent strategy:

- Apply quickfix when available.
- Otherwise rewrite to canonical `let <name> <expr>` / `set <name> <expr>`.
- Re-run lint.


## `X07-ARITY-RETURN-0001`

Summary: Core lint/schema diagnostic `X07-ARITY-RETURN-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-ARITY-UNSAFE-0001`

Summary: Core lint/schema diagnostic `X07-ARITY-UNSAFE-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-AST-0001`

Summary: Core lint/schema diagnostic `X07-AST-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-BORROW-0001`

Summary: Borrowing view/subview from a temporary expression is invalid.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

Quickfix is context-dependent: when a safe insertion point exists, linter emits JSON Patch introducing a temporary `let` binding and rewriting the borrow site.

Agent strategy:

- Prefer binding owner expressions to locals before `bytes.view`/`bytes.subview`/`vec_u8.as_view`.
- Apply quickfix when present.
- If quickfix is absent, perform equivalent manual rewrite and re-run lint.


## `X07-GENERICS-0001`

Summary: Type variable is not declared in type_params.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

A type expression references a type variable like `["t","A"]`, but the surrounding function does not declare `A` in `type_params`.

Agent strategy:

- If the function should be generic, add `{"name":"A"}` (and optional `bound`) to `type_params`.
- Otherwise, replace the type variable with a concrete type name (e.g. `"u32"`).
- Re-run `x07 lint` to confirm the signature is consistent.


## `X07-GENERICS-0002`

Summary: Unused type parameter.

Origins:
- x07c (stage: lint, severity: warning)

Quickfix support: `always`
Quickfix kinds: `json_patch`

Details:

A function declares a `type_params` entry that is never referenced in its signature types.

Agent strategy:

- Remove the unused type param from `type_params`, or reference it from `params[].ty` / `result`.
- Re-run `x07 lint`.


## `X07-INTERNAL-0001`

Summary: Internal tool failure `X07-INTERNAL-0001`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a toolchain fix; no safe source-level quickfix exists.

Details:

The toolchain encountered an internal error or panic while producing a machine report. In machine mode, the wrapper emits this stable code with `exit_code=3`.

Agent strategy:

- Re-run with `--verbose` / `--debug` if available.
- Reduce to a minimal repro.
- File a toolchain bug with the machine report JSON (especially `diagnostics[].data`).


## `X07-MOVE-0001`

Summary: `bytes.concat` uses the same identifier on both sides (use-after-move risk).

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `always`
Quickfix kinds: `json_patch`

Details:

Linter emits a JSON Patch quickfix that copies one side using `view.to_bytes(bytes.view(name))` before concat.

Agent strategy:

- Apply quickfix.
- Confirm resulting expression keeps ownership semantics.
- Re-run lint/build.


## `X07-MOVE-0002`

Summary: `if` condition and branch both borrow `bytes.view` from the same owner.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `always`
Quickfix kinds: `json_patch`

Details:

Linter emits a JSON Patch quickfix that copies bytes for condition use (`_x07_tmp_copy`) and rewrites the condition to avoid move conflicts.

Agent strategy:

- Apply quickfix.
- Validate condition logic still matches intent.
- Re-run lint/build.


## `X07-POLICY-SCHEMA-0001`

Summary: Core lint/schema diagnostic `X07-POLICY-SCHEMA-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-SCHEMA-0001`

Summary: Core lint/schema diagnostic `X07-SCHEMA-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-SCHEMA-0002`

Summary: Unsupported x07AST schema_version.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `always`
Quickfix kinds: `json_patch`

Details:

The x07AST file declares a `schema_version` that is not supported by this toolchain.

Use one of the supported schema versions, or upgrade/downgrade your toolchain to match the file.

Agent strategy:

- Inspect `/schema_version` in the failing `*.x07.json` file.
- Replace it with a supported value (see the diagnostic `data.supported`).
- Re-run `x07 ast validate` (or `x07 fmt` / `x07 lint`).


## `X07-TAPP-ELAB-0001`

Summary: Typechecker inferred generic type arguments and proposes inserting an explicit `tapp`.

Origins:
- x07c (stage: rewrite, severity: error)

Quickfix support: `always`
Quickfix kinds: `json_patch`

Details:

The local typecheck pass inferred concrete type arguments for a generic call that omitted `tapp`. Applying the quickfix rewrites the call into the canonical `tapp` form so later compiler stages see an explicit specialization.

Agent strategy:

- Run `x07 fix --write` to apply the JSON Patch quickfix.
- Re-run `x07 lint` or `x07 build` to confirm the rewrite eliminated the diagnostic.


## `X07-TAPP-INFER-0001`

Summary: Typechecker cannot infer generic type arguments; explicit `tapp` is required.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires choosing explicit type arguments; cannot be inferred deterministically from local constraints.

Details:

The local type inference pass could not determine one or more type arguments for a generic call from the available argument and expected-result constraints.

Agent strategy:

- Add an explicit `tapp` with concrete type arguments.
- Or rewrite the code to provide additional constraints so inference can resolve the type parameters deterministically.


## `X07-TOOL-ARGS-0001`

Summary: Tool wrapper argument error `X07-TOOL-ARGS-0001`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `never`
No quickfix reason: Requires changing the CLI invocation.

Details:

The invocation requested machine output (`--json`, `--jsonl`, `--json-schema`, etc.) but the wrapper flags were invalid or inconsistent.

Agent strategy:

- Fix the CLI flags (for example: add a missing path for `--report-out`, remove conflicting flags).
- Re-run the command.


## `X07-TOOL-EXEC-0001`

Summary: Tool wrapper execution error `X07-TOOL-EXEC-0001`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Depends on the underlying wrapped command failure.

Details:

The wrapped command exited non-zero but did not produce a structured diagnostic. The wrapper emitted this stable tool-level error and captured stdout/stderr in the report payload.

Agent strategy:

- Inspect `diagnostics[].message` and any captured stderr/stdout fields in the report.
- Fix the underlying failure and re-run the command.


## `X07-TYPE-0001`

Summary: Unknown identifier during local typechecking.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires a semantic choice (rename vs introduce a binding).

Details:

A referenced identifier was not found in the local environment (parameters or `let` bindings).

Agent strategy:

- Check spelling and scope.
- If the name is intended to be a local, introduce it with `let` before use.


## `X07-TYPE-CALL-0001`

Summary: Call refers to an unknown callee during local typechecking.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires selecting the intended callee symbol or updating imports/exports.

Details:

The typechecker could not find a signature for the callee symbol and cannot typecheck the call.

Agent strategy:

- Check the callee name and module prefix.
- Ensure the defining module is imported and the symbol is exported.


## `X07-TYPE-CALL-0002`

Summary: Call argument type mismatch during local typechecking.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires changing program semantics; no single deterministic patch applies.

Details:

An argument did not match the expected parameter type for the callee signature.

Agent strategy:

- Adjust the argument expression so its inferred type matches the parameter.
- Or call a different function whose signature matches the provided arguments.


## `X07-TYPE-CALL-0003`

Summary: Call arity mismatch during local typechecking.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires selecting the correct argument list for the intended call.

Details:

The number of arguments at a call site does not match the callee signature.

Agent strategy:

- Add or remove arguments to match the function signature.
- Or switch to a different callee with the intended arity.


## `X07-TYPE-IF-0001`

Summary: `if` condition must be an `i32` (boolean-as-i32) during local typechecking.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires choosing the intended condition semantics.

Details:

The condition expression of an `if` was inferred to a non-`i32` type.

Agent strategy:

- Rewrite the condition to produce an `i32` (for example, comparisons yield `i32`).
- Or adjust the condition expression so it matches boolean-as-i32 conventions.


## `X07-TYPE-IF-0002`

Summary: `if` branches have incompatible types during local typechecking.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires changing program structure or semantics to align branch result types.

Details:

The then/else branches could not be unified to a single result type for the `if` expression.

Agent strategy:

- Make both branches produce the same type.
- Or add an explicit structure that normalizes types (for example, return in one branch and a value in the other).


## `X07-TYPE-RET-0001`

Summary: Return expression type does not match the function result type.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires deciding whether to change the expression or the function signature.

Details:

A `return` expression was inferred to a type that does not match the declared function result type.

Agent strategy:

- Adjust the returned expression to match the function result type.
- Or update the function signature if the intended return type is different.


## `X07-TYPE-SET-0001`

Summary: `set` references an unknown local during local typechecking.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires choosing whether to introduce a new binding or rename the target.

Details:

A `set` attempted to assign to a local name that is not currently bound in the environment.

Agent strategy:

- Introduce the local with `let` before `set`.
- Or correct the target name to an existing binding.


## `X07-TYPE-SET-0002`

Summary: `set` assignment type mismatch during local typechecking.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Requires changing program semantics (assignment or variable structure).

Details:

The assigned expression type does not match the previously inferred type of the target local.

Agent strategy:

- Keep locals monomorphic: assign the same type across all `set` uses.
- If a new value of a different type is needed, introduce a new local with `let`.


## `X07-TYPE-UNIFY-0001`

Summary: Type unification failed during local inference.

Origins:
- x07c (stage: type, severity: info)

Quickfix support: `never`
No quickfix reason: Unification failures are context-dependent and do not have a single deterministic fix.

Details:

The local type inference engine encountered an unsatisfiable constraint while unifying types.

Agent strategy:

- Inspect the diagnostic data payload for the conflicting types.
- Rewrite the expression(s) at the reported ptr so the inferred types unify.


## `X07-UNSAFE-0001`

Summary: Core lint/schema diagnostic `X07-UNSAFE-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-WORLD-0001`

Summary: Program imports capabilities not allowed by the selected world flags.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `always`
Quickfix kinds: `json_patch`

Details:

The linter computes forbidden imports for the current world/config and emits a JSON Patch quickfix replacing `/imports` with the allowed subset.

Agent strategy:

- Review world toggles (`--enable-fs`, `--enable-rr`, `--enable-kv`) and imports.
- Apply quickfix to drop forbidden imports.
- Re-run lint and tests for the same world profile.


## `X07-WORLD-FFI-0001`

Summary: Core lint/schema diagnostic `X07-WORLD-FFI-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-WORLD-FS-0001`

Summary: Core lint/schema diagnostic `X07-WORLD-FS-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-WORLD-KV-0001`

Summary: Core lint/schema diagnostic `X07-WORLD-KV-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-WORLD-OS-0001`

Summary: std.os imports are forbidden in solve-* worlds.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `always`
Quickfix kinds: `json_patch`

Details:

The linter emits this when a solve-world program imports `std.os.*`. A JSON Patch quickfix is emitted to remove those imports from `/imports`.

Agent strategy:

- Use solve adapters (`std.fs`, `std.rr`, `std.kv`) for deterministic worlds.
- Apply quickfix to remove `std.os.*` imports.
- Re-run lint/build in the target world.


## `X07-WORLD-OS-0002`

Summary: Core lint/schema diagnostic `X07-WORLD-OS-0002`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-WORLD-RR-0001`

Summary: Core lint/schema diagnostic `X07-WORLD-RR-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-WORLD-UNSAFE-0001`

Summary: Core lint/schema diagnostic `X07-WORLD-UNSAFE-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-WORLD-UNSAFE-0002`

Summary: Core lint/schema diagnostic `X07-WORLD-UNSAFE-0002`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-X07AST-PARSE-0001`

Summary: Core lint/schema diagnostic `X07-X07AST-PARSE-0001`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07INIT_AGENT`

Summary: Project/package scaffold diagnostic `X07INIT_AGENT`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_AGENT_DIR`

Summary: Project/package scaffold diagnostic `X07INIT_AGENT_DIR`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_ARGS`

Summary: Project/package scaffold diagnostic `X07INIT_ARGS`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_CWD`

Summary: Project/package scaffold diagnostic `X07INIT_CWD`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_EXISTS`

Summary: Project/package scaffold diagnostic `X07INIT_EXISTS`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_GITIGNORE`

Summary: Project/package scaffold diagnostic `X07INIT_GITIGNORE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_IO`

Summary: Diagnostic code `X07INIT_IO`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Depends on host filesystem/permissions state outside AST/config patching.

Details:

This diagnostic generally requires external state changes or human intent to resolve (`X07INIT_IO`).

Agent strategy:

- Reproduce `X07INIT_IO` and capture structured context.
- Resolve the external dependency/state.
- Re-run the command and continue repair loop.


## `X07INIT_MODULES`

Summary: Project/package scaffold diagnostic `X07INIT_MODULES`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_PKG_LOCK`

Summary: Project/package scaffold diagnostic `X07INIT_PKG_LOCK`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_SRC`

Summary: Project/package scaffold diagnostic `X07INIT_SRC`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_TESTS`

Summary: Project/package scaffold diagnostic `X07INIT_TESTS`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07PKG_API_URL`

Summary: Package workflow diagnostic `X07PKG_API_URL`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_DEP_EXISTS`

Summary: Package workflow diagnostic `X07PKG_DEP_EXISTS`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_DEP_NOT_FOUND`

Summary: Package workflow diagnostic `X07PKG_DEP_NOT_FOUND`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_DOWNLOAD_FAILED`

Summary: Diagnostic code `X07PKG_DOWNLOAD_FAILED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Depends on remote archive availability and network reliability.

Details:

This diagnostic generally requires external state changes or human intent to resolve (`X07PKG_DOWNLOAD_FAILED`).

Agent strategy:

- Reproduce `X07PKG_DOWNLOAD_FAILED` and capture structured context.
- Resolve the external dependency/state.
- Re-run the command and continue repair loop.


## `X07PKG_INDEX_CONFIG`

Summary: Package workflow diagnostic `X07PKG_INDEX_CONFIG`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_INDEX_FETCH`

Summary: Diagnostic code `X07PKG_INDEX_FETCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Depends on remote index/network availability.

Details:

This diagnostic generally requires external state changes or human intent to resolve (`X07PKG_INDEX_FETCH`).

Agent strategy:

- Reproduce `X07PKG_INDEX_FETCH` and capture structured context.
- Resolve the external dependency/state.
- Re-run the command and continue repair loop.


## `X07PKG_INDEX_NO_MATCH`

Summary: Package workflow diagnostic `X07PKG_INDEX_NO_MATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_LOCK_MISMATCH`

Summary: Package workflow diagnostic `X07PKG_LOCK_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_LOCK_MISSING`

Summary: Package workflow diagnostic `X07PKG_LOCK_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_LOGIN_FAILED`

Summary: Diagnostic code `X07PKG_LOGIN_FAILED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Depends on remote registry authentication response.

Details:

This diagnostic generally requires external state changes or human intent to resolve (`X07PKG_LOGIN_FAILED`).

Agent strategy:

- Reproduce `X07PKG_LOGIN_FAILED` and capture structured context.
- Resolve the external dependency/state.
- Re-run the command and continue repair loop.


## `X07PKG_LOGIN_TOKEN`

Summary: Package workflow diagnostic `X07PKG_LOGIN_TOKEN`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_OFFLINE_MISSING_DEP`

Summary: Package workflow diagnostic `X07PKG_OFFLINE_MISSING_DEP`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_PUBLISH_FAILED`

Summary: Diagnostic code `X07PKG_PUBLISH_FAILED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Depends on remote registry/network state.

Details:

This diagnostic generally requires external state changes or human intent to resolve (`X07PKG_PUBLISH_FAILED`).

Agent strategy:

- Reproduce `X07PKG_PUBLISH_FAILED` and capture structured context.
- Resolve the external dependency/state.
- Re-run the command and continue repair loop.


## `X07PKG_PUBLISH_RESPONSE`

Summary: Package workflow diagnostic `X07PKG_PUBLISH_RESPONSE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_PUBLISH_RESPONSE_MISMATCH`

Summary: Package workflow diagnostic `X07PKG_PUBLISH_RESPONSE_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_SPEC_INVALID`

Summary: Package workflow diagnostic `X07PKG_SPEC_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_TRANSITIVE_MISSING`

Summary: Package workflow diagnostic `X07PKG_TRANSITIVE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07RR_ENTRY_EXISTS`

Summary: Record/replay fixture diagnostic `X07RR_ENTRY_EXISTS`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

RR fixture fields and constraints are deterministic; repair usually means correcting entry schema values or regenerating fixture entries.

Agent strategy:

- Validate RR entry fields (key/url/kind/op/latency).
- Regenerate or edit fixtures deterministically.
- Re-run `x07 rr` operation.


## `X07RR_HTTP`

Summary: Diagnostic code `X07RR_HTTP`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Depends on external HTTP service behavior.

Details:

This diagnostic generally requires external state changes or human intent to resolve (`X07RR_HTTP`).

Agent strategy:

- Reproduce `X07RR_HTTP` and capture structured context.
- Resolve the external dependency/state.
- Re-run the command and continue repair loop.


## `X07RR_KEY_EMPTY`

Summary: Record/replay fixture diagnostic `X07RR_KEY_EMPTY`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

RR fixture fields and constraints are deterministic; repair usually means correcting entry schema values or regenerating fixture entries.

Agent strategy:

- Validate RR entry fields (key/url/kind/op/latency).
- Regenerate or edit fixtures deterministically.
- Re-run `x07 rr` operation.


## `X07RR_KIND_EMPTY`

Summary: Record/replay fixture diagnostic `X07RR_KIND_EMPTY`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

RR fixture fields and constraints are deterministic; repair usually means correcting entry schema values or regenerating fixture entries.

Agent strategy:

- Validate RR entry fields (key/url/kind/op/latency).
- Regenerate or edit fixtures deterministically.
- Re-run `x07 rr` operation.


## `X07RR_LATENCY_OUT_OF_RANGE`

Summary: Record/replay fixture diagnostic `X07RR_LATENCY_OUT_OF_RANGE`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

RR fixture fields and constraints are deterministic; repair usually means correcting entry schema values or regenerating fixture entries.

Agent strategy:

- Validate RR entry fields (key/url/kind/op/latency).
- Regenerate or edit fixtures deterministically.
- Re-run `x07 rr` operation.


## `X07RR_OP_EMPTY`

Summary: Record/replay fixture diagnostic `X07RR_OP_EMPTY`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

RR fixture fields and constraints are deterministic; repair usually means correcting entry schema values or regenerating fixture entries.

Agent strategy:

- Validate RR entry fields (key/url/kind/op/latency).
- Regenerate or edit fixtures deterministically.
- Re-run `x07 rr` operation.


## `X07RR_URL_EMPTY`

Summary: Record/replay fixture diagnostic `X07RR_URL_EMPTY`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

RR fixture fields and constraints are deterministic; repair usually means correcting entry schema values or regenerating fixture entries.

Agent strategy:

- Validate RR entry fields (key/url/kind/op/latency).
- Regenerate or edit fixtures deterministically.
- Re-run `x07 rr` operation.


## `X7I0001`

Summary: x07import subset compatibility diagnostic `X7I0001`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0100`

Summary: x07import subset compatibility diagnostic `X7I0100`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0110`

Summary: x07import subset compatibility diagnostic `X7I0110`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0111`

Summary: x07import subset compatibility diagnostic `X7I0111`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0120`

Summary: x07import subset compatibility diagnostic `X7I0120`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0122`

Summary: x07import subset compatibility diagnostic `X7I0122`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0123`

Summary: x07import subset compatibility diagnostic `X7I0123`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0200`

Summary: x07import subset compatibility diagnostic `X7I0200`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0201`

Summary: x07import subset compatibility diagnostic `X7I0201`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0210`

Summary: x07import subset compatibility diagnostic `X7I0210`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0211`

Summary: x07import subset compatibility diagnostic `X7I0211`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0300`

Summary: x07import subset compatibility diagnostic `X7I0300`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0301`

Summary: x07import subset compatibility diagnostic `X7I0301`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0310`

Summary: x07import subset compatibility diagnostic `X7I0310`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0311`

Summary: x07import subset compatibility diagnostic `X7I0311`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0320`

Summary: x07import subset compatibility diagnostic `X7I0320`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0330`

Summary: x07import subset compatibility diagnostic `X7I0330`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0340`

Summary: x07import subset compatibility diagnostic `X7I0340`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0350`

Summary: x07import subset compatibility diagnostic `X7I0350`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0360`

Summary: x07import subset compatibility diagnostic `X7I0360`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic phase/context.
- Rewrite unsupported Rust/C constructs into supported subset forms.
- Re-run x07import and tests.


## `X7I0901`

Summary: Diagnostic code `X7I0901`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Internal importer bug; requires toolchain fix rather than source-level quickfix.

Details:

This diagnostic generally requires external state changes or human intent to resolve (`X7I0901`).

Agent strategy:

- Reproduce `X7I0901` and capture structured context.
- Resolve the external dependency/state.
- Re-run the command and continue repair loop.

