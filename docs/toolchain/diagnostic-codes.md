# Diagnostic codes catalog

This file is generated from `catalog/diagnostics.json` using `x07 diag catalog`.

- total codes: 182
- quickfix support (`sometimes` or `always`): 155
- quickfix coverage: 85.16%

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
| `ETEST_INPUT_B64_INVALID` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_INPUT_B64_INVALID`. |
| `ETEST_INPUT_CONFLICT` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_INPUT_CONFLICT`. |
| `ETEST_INPUT_NOT_ALLOWED_V010` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_INPUT_NOT_ALLOWED_V010`. |
| `ETEST_INPUT_PATH_READ_FAILED` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_INPUT_PATH_READ_FAILED`. |
| `ETEST_INPUT_UNSAFE_PATH` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_INPUT_UNSAFE_PATH`. |
| `ETEST_INPUT_UNSUPPORTED_WORLD` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_INPUT_UNSUPPORTED_WORLD`. |
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
| `E_DEPS_CAP_POLICY_DENY` | x07 / lint / error | never | Dependency violates capability policy. |
| `E_SBOM_GENERATION_FAILED` | x07 / lint / error | never | SBOM generation failed. |
| `W_ARCH_CONTRACTS_LOCK_MISSING` | x07 / lint / warning | sometimes | Architecture contract diagnostic `W_ARCH_CONTRACTS_LOCK_MISSING`. |
| `W_ARCH_CONTRACT_OPAQUE_USAGE` | x07 / lint / warning | sometimes | Architecture contract diagnostic `W_ARCH_CONTRACT_OPAQUE_USAGE`. |
| `W_DEPS_CAP_POLICY_MISSING` | x07 / lint / warning | never | Dependency capability policy missing. |
| `X07-ARITY-0000` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-0000`. |
| `X07-ARITY-BEGIN-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-BEGIN-0001`. |
| `X07-ARITY-FOR-0001` | x07c / lint / error | sometimes | `for` has invalid arity. |
| `X07-ARITY-IF-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-IF-0001`. |
| `X07-ARITY-LET-0001` | x07c / lint / error | sometimes | `let`/`set` has invalid arity. |
| `X07-ARITY-RETURN-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-RETURN-0001`. |
| `X07-ARITY-UNSAFE-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-UNSAFE-0001`. |
| `X07-AST-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-AST-0001`. |
| `X07-AST-SLICE-0001` | x07 / run / info | never | AST slice truncated `X07-AST-SLICE-0001`. |
| `X07-BORROW-0001` | x07c / lint / error | sometimes | Borrowing view/subview from a temporary expression is invalid. |
| `X07-CONTRACT-0001` | x07c / type / error | sometimes | Contract clause must typecheck to i32. |
| `X07-CONTRACT-0002` | x07c / type / error | sometimes | Contract expression is not contract-pure. |
| `X07-CONTRACT-0003` | x07c / type / error | sometimes | Reserved identifier `__result` is only available in ensures. |
| `X07-CONTRACT-0004` | x07c / type / error | sometimes | Reserved name collision with `__result`. |
| `X07-CONTRACT-0005` | x07c / type / error | sometimes | Contract witness has an unsupported type. |
| `X07-GENERICS-0001` | x07c / lint / error | sometimes | Type variable is not declared in type_params. |
| `X07-GENERICS-0002` | x07c / lint / warning | always | Unused type parameter. |
| `X07-INTERNAL-0001` | x07 / run / error | never | Internal tool failure `X07-INTERNAL-0001`. |
| `X07-MOVE-0001` | x07c / lint / error | always | `bytes.concat` uses the same identifier on both sides (use-after-move risk). |
| `X07-MOVE-0002` | x07c / lint / error | always | `if` condition and branch both borrow `bytes.view` from the same owner. |
| `X07-PBT-FIX-ARGS-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-PBT-FIX-ARGS-0001`. |
| `X07-PBT-FIX-CONFLICT-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-PBT-FIX-CONFLICT-0001`. |
| `X07-PBT-FIX-INFO-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-PBT-FIX-INFO-0001`. |
| `X07-PBT-FIX-MANIFEST-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-PBT-FIX-MANIFEST-0001`. |
| `X07-PBT-FIX-TEST-NOT-FOUND-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-PBT-FIX-TEST-NOT-FOUND-0001`. |
| `X07-PBT-FIX-UNSUPPORTED-TY-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-PBT-FIX-UNSUPPORTED-TY-0001`. |
| `X07-PBT-REPRO-PARSE-0001` | x07 / parse / error | sometimes | Core lint/schema diagnostic `X07-PBT-REPRO-PARSE-0001`. |
| `X07-PBT-REPRO-READ-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-PBT-REPRO-READ-0001`. |
| `X07-PBT-REPRO-SCHEMA-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-PBT-REPRO-SCHEMA-0001`. |
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
| `X07INIT_MCP_FAILED` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_MCP_FAILED`. |
| `X07INIT_MCP_MISSING` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_MCP_MISSING`. |
| `X07INIT_MCP_REPORT` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_MCP_REPORT`. |
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
| `X07T_EPBT_MANIFEST_INVALID` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07T_EPBT_MANIFEST_INVALID`. |
| `X07T_EPBT_PARAM_EMPTY` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07T_EPBT_PARAM_EMPTY`. |
| `X07T_EPBT_UNKNOWN_GEN_KIND` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07T_EPBT_UNKNOWN_GEN_KIND`. |
| `X07T_EPBT_UNSUPPORTED_WORLD` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07T_EPBT_UNSUPPORTED_WORLD`. |
| `X07V_EARGS` | x07 / run / error | sometimes | Invalid `x07 verify` arguments. |
| `X07V_ECBMC_ERROR` | x07 / run / error | sometimes | CBMC reported an error. |
| `X07V_ECBMC_FAILURE` | x07 / run / error | sometimes | CBMC reported a failing property that is not a contract. |
| `X07V_ECBMC_JSON_PARSE` | x07 / run / error | sometimes | Failed to parse CBMC JSON output. |
| `X07V_ECBMC_MISSING` | x07 / run / error | sometimes | CBMC is missing. |
| `X07V_ECBMC_SMT2` | x07 / run / error | sometimes | CBMC failed to emit SMT2. |
| `X07V_ECBMC_STDERR` | x07 / run / error | sometimes | CBMC wrote unexpected stderr output. |
| `X07V_EMODULE_ROOTS` | x07 / run / error | sometimes | Could not resolve module roots for `x07 verify`. |
| `X07V_EPROJECT` | x07 / run / error | sometimes | Could not resolve project manifest for `x07 verify`. |
| `X07V_ETARGET` | x07 / run / error | sometimes | Verification target could not be loaded. |
| `X07V_EZ3_MISSING` | x07 / run / error | sometimes | Z3 is missing. |
| `X07V_EZ3_RUN` | x07 / run / error | sometimes | Z3 failed to run. |
| `X07V_INTERNAL` | x07 / run / error | never | Internal `x07 verify` error. |
| `X07V_NO_CONTRACTS` | x07 / run / error | sometimes | Target has no contracts to verify. |
| `X07V_SMT_SAT` | x07 / run / error | sometimes | SMT solver returned SAT (counterexample found). |
| `X07V_SMT_UNKNOWN` | x07 / run / error | sometimes | SMT solver returned an unknown status. |
| `X07V_UNSUPPORTED_ASYNC` | x07 / run / error | sometimes | `x07 verify` does not support `defasync` targets. |
| `X07V_UNSUPPORTED_FOR_BOUNDS` | x07 / run / error | sometimes | `for` loop bounds must be literal for `x07 verify` v0.1. |
| `X07V_UNSUPPORTED_PARAM` | x07 / run / error | sometimes | Unsupported parameter type for `x07 verify`. |
| `X07V_UNSUPPORTED_RECURSION` | x07 / run / error | sometimes | Recursive targets are not supported by `x07 verify` v0.1. |
| `X07V_UNWIND_INCOMPLETE` | x07 / run / error | sometimes | CBMC unwinding is incomplete. |
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


## `ETEST_INPUT_B64_INVALID`

Summary: Test manifest validation diagnostic `ETEST_INPUT_B64_INVALID`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_INPUT_CONFLICT`

Summary: Test manifest validation diagnostic `ETEST_INPUT_CONFLICT`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_INPUT_NOT_ALLOWED_V010`

Summary: Test manifest validation diagnostic `ETEST_INPUT_NOT_ALLOWED_V010`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_INPUT_PATH_READ_FAILED`

Summary: Test manifest validation diagnostic `ETEST_INPUT_PATH_READ_FAILED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_INPUT_UNSAFE_PATH`

Summary: Test manifest validation diagnostic `ETEST_INPUT_UNSAFE_PATH`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is deterministic in `tests/tests.json` or related test assets. It can usually be repaired by structured edits to manifest fields or fixture/policy paths.

Agent strategy:

- Validate `tests/tests.json` fields and world requirements.
- Apply canonical manifest edits (id/world/entry/expect/returns/paths).
- Re-run `x07 test`.


## `ETEST_INPUT_UNSUPPORTED_WORLD`

Summary: Test manifest validation diagnostic `ETEST_INPUT_UNSUPPORTED_WORLD`.

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


## `E_DEPS_CAP_POLICY_DENY`

Summary: Dependency violates capability policy.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a policy decision (allow vs deny) and/or changing the dependency code.

Details:

A locked dependency statically uses one or more sensitive namespaces that are denied by the dependency capability policy.

This gate is based on the same sensitive namespace scan surfaced in `x07 trust report` (for example `std.os.net.` or `std.os.process.`).

Agent strategy:

- Inspect `diagnostics[].data.offending_namespaces` and the `policy` fields (path/rule).
- If the dependency is expected to use the capability, add an explicit allow entry for that package.
- Otherwise, replace the dependency or remove the sensitive namespace usage.
- Re-run `x07 trust report --fail-on deps-capability` to verify the policy is enforced.


## `E_SBOM_GENERATION_FAILED`

Summary: SBOM generation failed.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: SBOM output depends on filesystem availability and explicit configuration.

Details:

`x07 trust report` failed to generate or write a deterministic SBOM artifact (CycloneDX/SPDX).

If CI is gating on `--fail-on sbom-missing`, this will cause the trust report command to fail.

Agent strategy:

- Check the `diagnostics[].data.path` and filesystem permissions for the output directory.
- If SBOM output is intentionally disabled, avoid gating on `sbom-missing` or remove `--sbom-format none`.
- Re-run `x07 trust report` to confirm the SBOM artifact is generated and stable.


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


## `W_DEPS_CAP_POLICY_MISSING`

Summary: Dependency capability policy missing.

Origins:
- x07 (stage: lint, severity: warning)

Quickfix support: `never`
No quickfix reason: Requires author intent (define a deny/allow policy for the project dependencies).

Details:

The project declares dependencies but `x07 trust report` did not find a dependency capability policy file.

By default, the policy is discovered at `x07.deps.capability-policy.json` in the project root (or overridden via `--deps-cap-policy`).

Agent strategy:

- Create `x07.deps.capability-policy.json` in the project root (schema: `x07.deps.capability_policy@0.1.0`).
- Or pass `--deps-cap-policy <path>` to point at an existing policy file.
- To enforce in CI, add `--fail-on deps-capability`.


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


## `X07-AST-SLICE-0001`

Summary: AST slice truncated `X07-AST-SLICE-0001`.

Origins:
- x07 (stage: run, severity: info)

Quickfix support: `never`
No quickfix reason: Truncation is controlled by slice bounds.

Details:

Emitted when `x07 ast slice` truncates the output due to `--max_nodes` or `--max_bytes`. The slice remains deterministic but may omit context needed for repair.

Agent strategy:

- Re-run with a higher `--max_nodes` / `--max_bytes` bound.
- Use `--enclosure=module` and/or `--closure=all` if more context is required.


## `X07-BORROW-0001`

Summary: Borrowing view/subview from a temporary expression is invalid.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

Quickfix is context-dependent: when a safe insertion point exists, linter emits JSON Patch introducing a temporary `let` binding and rewriting the borrow site.

Agent strategy:

- Read `diagnostics[].data.mem_provenance` to identify the temporary owner and the borrow call.
- Prefer binding owner expressions to locals before `bytes.view`/`bytes.subview`/`vec_u8.as_view`.
- If `mem_provenance.hints[]` contains `bind_owner_to_local`, introduce `let tmp = <owner>` and borrow from `tmp`.
- Apply quickfix when present.
- If quickfix is absent, perform equivalent manual rewrite and re-run lint.


## `X07-CONTRACT-0001`

Summary: Contract clause must typecheck to i32.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

Contract expressions in `requires[]`, `ensures[]`, and `invariant[]` must typecheck to `i32` (x07 uses `i32` as a boolean: `0` = false, non-zero = true).

Agent strategy:

- Locate the reported clause expression and ensure it evaluates to an `i32` predicate.
- Add an explicit comparison (for example `i32.eq`, `i32.lt`, etc.) when the expression currently returns a non-`i32` value.
- Re-run `x07 lint` / `x07 test` to confirm the contract typechecks.


## `X07-CONTRACT-0002`

Summary: Contract expression is not contract-pure.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

Contract clause expressions (and witness expressions) must be contract-pure so they do not change program semantics. Disallowed operations include `world.*`, `task.*`, mutation forms (for example `set`, `unsafe`), and other impure/nondeterministic constructs.

Agent strategy:

- Replace the impure subexpression with a pure computation (compute in the main body and reference the pure value in the contract).
- If the clause needs external state, move that check into program logic/tests instead of a contract.
- Re-run `x07 lint` to confirm the impurity is gone.


## `X07-CONTRACT-0003`

Summary: Reserved identifier `__result` is only available in ensures.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

`__result` is reserved for postconditions: it is bound to the function return value only while checking `ensures[]`. It is an error to reference `__result` in `requires[]`, `invariant[]`, or their witnesses.

Agent strategy:

- If you need the return value, move the clause to `ensures[]`.
- Otherwise, remove/replace the `__result` reference with an expression over parameters/locals available at the clause site.
- Re-run `x07 lint` to confirm the contract is valid.


## `X07-CONTRACT-0004`

Summary: Reserved name collision with `__result`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

When a function has contracts, `__result` is reserved for the postcondition binding used in `ensures[]`. It is an error for a parameter or local binding to be named `__result`.

Agent strategy:

- Rename the parameter/local to a non-reserved name.
- If you need a local alias of the return value, use a different identifier and reference `__result` only inside `ensures[]`.
- Re-run `x07 lint` to confirm there are no remaining collisions.


## `X07-CONTRACT-0005`

Summary: Contract witness has an unsupported type.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

Witness expressions are evaluated only on contract failure and are recorded in the failure payload. In v0.1, witnesses are restricted to simple, serializable types (currently `i32`, `bytes`, `bytes_view`, and `result_*`).

Agent strategy:

- Change the witness expression to an allowed type (for example witness `view.len` as `i32`, or include a `bytes_view` slice).
- If you need richer context, encode it into `bytes`/`bytes_view` explicitly.
- Re-run `x07 lint` to confirm witness types are accepted.


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

- Read `diagnostics[].data.mem_provenance` to confirm the use-after-move chain.
- If `mem_provenance.hints[]` contains `clone_before_use`, copy one side (for example via `view.to_bytes(bytes.view(name))`) before reuse.
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

- Read `diagnostics[].data.mem_provenance` to see the conflicting `bytes.view` borrows across condition/branch.
- If `mem_provenance.hints[]` contains `introduce_tmp_copy`, copy bytes for the `if` condition and use the copy in the condition.
- Apply quickfix.
- Validate condition logic still matches intent.
- Re-run lint/build.


## `X07-PBT-FIX-ARGS-0001`

Summary: Core lint/schema diagnostic `X07-PBT-FIX-ARGS-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-PBT-FIX-CONFLICT-0001`

Summary: Core lint/schema diagnostic `X07-PBT-FIX-CONFLICT-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-PBT-FIX-INFO-0001`

Summary: Core lint/schema diagnostic `X07-PBT-FIX-INFO-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-PBT-FIX-MANIFEST-0001`

Summary: Core lint/schema diagnostic `X07-PBT-FIX-MANIFEST-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-PBT-FIX-TEST-NOT-FOUND-0001`

Summary: Core lint/schema diagnostic `X07-PBT-FIX-TEST-NOT-FOUND-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-PBT-FIX-UNSUPPORTED-TY-0001`

Summary: Core lint/schema diagnostic `X07-PBT-FIX-UNSUPPORTED-TY-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-PBT-REPRO-PARSE-0001`

Summary: Core lint/schema diagnostic `X07-PBT-REPRO-PARSE-0001`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-PBT-REPRO-READ-0001`

Summary: Core lint/schema diagnostic `X07-PBT-REPRO-READ-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-PBT-REPRO-SCHEMA-0001`

Summary: Core lint/schema diagnostic `X07-PBT-REPRO-SCHEMA-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


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


## `X07INIT_MCP_FAILED`

Summary: Project/package scaffold diagnostic `X07INIT_MCP_FAILED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

`x07-mcp scaffold init` returned a non-success result or error report.

Agent strategy:

- Run `x07-mcp scaffold init --template mcp-server-stdio --dir <DIR> --machine json` to reproduce.
- Fix the reported issue and re-run `x07 init`.


## `X07INIT_MCP_MISSING`

Summary: Project/package scaffold diagnostic `X07INIT_MCP_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The MCP server templates are provided by `x07-mcp` (an external tool).

Agent strategy:

- Install `x07-mcp` and ensure it is discoverable on PATH.
- Re-run `x07 init --template mcp-server-stdio` (or another MCP template).


## `X07INIT_MCP_REPORT`

Summary: Project/package scaffold diagnostic `X07INIT_MCP_REPORT`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The `x07-mcp scaffold init --machine json` report could not be parsed as JSON.

Agent strategy:

- Ensure `x07-mcp` is up to date and on PATH.
- Re-run the command and capture stdout/stderr for debugging.


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


## `X07T_EPBT_MANIFEST_INVALID`

Summary: Core lint/schema diagnostic `X07T_EPBT_MANIFEST_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07T_EPBT_PARAM_EMPTY`

Summary: Core lint/schema diagnostic `X07T_EPBT_PARAM_EMPTY`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07T_EPBT_UNKNOWN_GEN_KIND`

Summary: Core lint/schema diagnostic `X07T_EPBT_UNKNOWN_GEN_KIND`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07T_EPBT_UNSUPPORTED_WORLD`

Summary: Core lint/schema diagnostic `X07T_EPBT_UNSUPPORTED_WORLD`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_EARGS`

Summary: Invalid `x07 verify` arguments.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

`x07 verify` requires selecting exactly one verification mode.

Agent strategy:

- Pass exactly one of `--bmc` or `--smt`.
- Re-run the command with the corrected flags.


## `X07V_ECBMC_ERROR`

Summary: CBMC reported an error.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

CBMC reported one or more `ERROR` messages while running the verification.

Agent strategy:

- Inspect the saved `cbmc.json` artifact (when present) for the reported error messages.
- Adjust bounds or simplify the target to stay within the supported subset.
- Re-run `x07 verify`.


## `X07V_ECBMC_FAILURE`

Summary: CBMC reported a failing property that is not a contract.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

`x07 verify` expects verification failures to correspond to X07 contract assertions. This diagnostic indicates CBMC reported a failing property that could not be mapped to a contract clause.

Agent strategy:

- Inspect `cbmc.json` and the generated `verify.c` harness to identify the failing property.
- Simplify the target program and retry.
- If the failure is reproducible, file a toolchain issue with the artifacts.


## `X07V_ECBMC_JSON_PARSE`

Summary: Failed to parse CBMC JSON output.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

CBMC did not produce valid JSON output under `--json-ui`.

Agent strategy:

- Check the installed CBMC version supports `--json-ui`.
- Re-run with the same bounds and inspect the raw stdout.
- If the output is not JSON, upgrade/downgrade CBMC and retry.


## `X07V_ECBMC_MISSING`

Summary: CBMC is missing.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

`x07 verify` uses CBMC for `--bmc` and to emit SMT2 for `--smt`.

Agent strategy:

- Install CBMC (for example `brew install cbmc`) and ensure `cbmc` is on `PATH`.
- Re-run `x07 verify`.


## `X07V_ECBMC_SMT2`

Summary: CBMC failed to emit SMT2.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

CBMC returned a non-zero status while emitting an SMT-LIB2 formula (`--smt2`).

Agent strategy:

- Re-run `x07 verify --smt` and inspect CBMC output.
- Check the installed CBMC version and upgrade/downgrade if needed.
- Keep the target within the supported subset.


## `X07V_ECBMC_STDERR`

Summary: CBMC wrote unexpected stderr output.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

In JSON UI mode, `x07 verify` expects CBMC to write its report to stdout. Stderr output is treated as an error to avoid silently ignoring tool issues.

Agent strategy:

- Re-run the same command and capture stderr.
- Check the installed CBMC version and consider upgrading/downgrading.
- If reproducible, file a toolchain issue with the stderr and the generated `verify.c` artifact.


## `X07V_EMODULE_ROOTS`

Summary: Could not resolve module roots for `x07 verify`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

When `--module-root` is not provided, `x07 verify` attempts to infer module roots from the project manifest and lockfile. This diagnostic indicates that inference failed (for example due to a missing/invalid lockfile).

Agent strategy:

- Provide explicit `--module-root` directories.
- Or ensure the project has a valid lockfile (`x07.lock.json`) by running `x07 pkg lock`.
- Re-run `x07 verify`.


## `X07V_EPROJECT`

Summary: Could not resolve project manifest for `x07 verify`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

`x07 verify` resolves module roots from a project manifest (`x07.json`) when available. This diagnostic indicates the `--project` path is invalid or a manifest could not be found.

Agent strategy:

- Pass `--project <path-to-x07.json>` (or a directory containing it).
- If running outside a project, pass one or more `--module-root <dir>` values.
- Re-run `x07 verify`.


## `X07V_ETARGET`

Summary: Verification target could not be loaded.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The `--entry` symbol could not be resolved to a `defn` in the module roots (module file not found, function not found, or module JSON invalid).

Agent strategy:

- Ensure `--entry` is a fully qualified name with a module separator (`mod.fn`).
- Ensure the module file exists at `<module-root>/<module-id>.x07.json`.
- Re-run `x07 verify` after fixing the module roots or entry name.


## `X07V_EZ3_MISSING`

Summary: Z3 is missing.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

In `--smt` mode, `x07 verify` can emit SMT2 even without Z3 installed, but solving requires Z3.

Agent strategy:

- Install Z3 (for example `brew install z3`) and ensure `z3` is on `PATH`.
- Re-run `x07 verify --smt`, or consume the emitted SMT2 artifact with another solver.


## `X07V_EZ3_RUN`

Summary: Z3 failed to run.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Z3 returned a non-zero exit status while solving the emitted SMT2 formula.

Agent strategy:

- Inspect the saved `verify.smt2` and `z3.out.txt` artifacts.
- Check the installed Z3 version and retry.
- If the solver crashes, simplify the target or reduce bounds.


## `X07V_INTERNAL`

Summary: Internal `x07 verify` error.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Internal tool failure; requires toolchain fix.

Details:

`x07 verify` encountered an unexpected internal error.

Agent strategy:

- Capture the full error text and the generated artifacts under `.x07/artifacts/verify/...`.
- Minimize a repro (smallest program + bounds).
- File a toolchain issue; this typically requires a toolchain fix.


## `X07V_NO_CONTRACTS`

Summary: Target has no contracts to verify.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

`x07 verify` requires at least one `requires[]`, `ensures[]`, or `invariant[]` clause on the target function.

Agent strategy:

- Add at least one contract clause to the target `defn`.
- Re-run `x07 verify` in the desired mode.


## `X07V_SMT_SAT`

Summary: SMT solver returned SAT (counterexample found).

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The SMT solver found a satisfying model, which indicates a counterexample within the current bounds.

Agent strategy:

- Treat the result as a counterexample: tighten contracts or fix the function logic.
- If needed, reduce the search space by strengthening `requires[]` or adding `invariant[]` clauses.
- Re-run `x07 verify` to confirm the fix.


## `X07V_SMT_UNKNOWN`

Summary: SMT solver returned an unknown status.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The SMT solver did not return `sat` or `unsat` (for example `unknown`). The result is inconclusive.

Agent strategy:

- Treat the result as inconclusive: adjust bounds or simplify the target.
- If available, use a different solver or solver version.
- Keep the function within the supported subset and retry.


## `X07V_UNSUPPORTED_ASYNC`

Summary: `x07 verify` does not support `defasync` targets.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

`x07 verify` v0.1 only supports verifying `defn` targets.

Agent strategy:

- Verify a `defn` wrapper around the async logic (or refactor the core logic into a `defn` helper).
- Keep the verified function within the supported subset (no async/concurrency).
- Re-run `x07 verify`.


## `X07V_UNSUPPORTED_FOR_BOUNDS`

Summary: `for` loop bounds must be literal for `x07 verify` v0.1.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The verification subset supports `for` loops only when bounds are statically known (literal) so CBMC unwinding is well-defined.

Agent strategy:

- Replace variable bounds with a literal bound and add a contract that relates the literal bound to input sizes.
- Or restructure the logic so loops are statically bounded.
- Re-run `x07 verify`.


## `X07V_UNSUPPORTED_PARAM`

Summary: Unsupported parameter type for `x07 verify`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The verification subset currently supports only `i32`, `u32`, `bytes`, and `bytes_view` parameters.

Agent strategy:

- Introduce a `defn` wrapper that encodes unsupported types into supported types (for example into `bytes`).
- Keep the verified target within the supported subset.
- Re-run `x07 verify`.


## `X07V_UNSUPPORTED_RECURSION`

Summary: Recursive targets are not supported by `x07 verify` v0.1.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The verification subset currently rejects recursion.

Agent strategy:

- Refactor recursion into an iterative loop with a statically bounded `for`, or split the function into a non-recursive helper.
- Re-run `x07 verify`.


## `X07V_UNWIND_INCOMPLETE`

Summary: CBMC unwinding is incomplete.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

CBMC reported an unwinding assertion failure, which means the verification result is inconclusive at the current `--unwind` bound.

Agent strategy:

- Increase `--unwind` until unwinding assertions no longer fail.
- If the function needs unbounded iteration, refactor it to a statically bounded form for v0.1 verification.


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

