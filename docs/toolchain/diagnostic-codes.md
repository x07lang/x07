# Diagnostic codes catalog

This file is generated from `catalog/diagnostics.json` using `x07 diag catalog`.

- total codes: 637
- quickfix support (`sometimes` or `always`): 577
- quickfix coverage: 90.58%

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
| `ETEST_SOLVE_FUEL_INVALID` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_SOLVE_FUEL_INVALID`. |
| `ETEST_TESTS_EMPTY` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_TESTS_EMPTY`. |
| `ETEST_TIMEOUT_INVALID` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_TIMEOUT_INVALID`. |
| `ETEST_WORLD_INVALID` | x07 / run / error | sometimes | Test manifest validation diagnostic `ETEST_WORLD_INVALID`. |
| `EXTAL_CERTIFY_ENTRY_REQUIRED` | x07 / parse / error | sometimes | XTAL certification requires an explicit entry selection. |
| `EXTAL_CERTIFY_MANIFEST_MISSING` | x07 / parse / error | sometimes | XTAL certification manifest is required. |
| `EXTAL_CERTIFY_MANIFEST_PARSE_FAILED` | x07 / parse / error | sometimes | XTAL certification manifest is invalid. |
| `EXTAL_CERTIFY_NO_ENTRYPOINTS` | x07 / parse / error | sometimes | XTAL certification has no entrypoints. |
| `EXTAL_CERTIFY_PRECHECKS_FAILED` | x07 / run / error | never | XTAL certification prechecks failed. |
| `EXTAL_CERTIFY_TRUST_CERTIFY_FAILED` | x07 / run / error | sometimes | Trust certification failed for a certification entry. |
| `EXTAL_DEV_NO_SPECS` | x07 / run / error | sometimes | XTAL dev pipeline found no spec modules. |
| `EXTAL_EVENTS_INVALID` | x07 / parse / error | sometimes | Recovery events log is invalid. |
| `EXTAL_EVENTS_IO` | x07 / parse / error | sometimes | Recovery events log I/O failed. |
| `EXTAL_EVENTS_UNSUPPORTED_VERSION` | x07 / parse / error | sometimes | Recovery event schema_version is unsupported. |
| `EXTAL_EXAMPLES_ARGS_EXTRA` | x07 / lint / error | sometimes | Example provides an extra arg. |
| `EXTAL_EXAMPLES_ARGS_MISSING` | x07 / lint / error | sometimes | Example is missing a required arg. |
| `EXTAL_EXAMPLES_ARG_KIND_UNSUPPORTED` | x07 / lint / error | sometimes | Example arg encoding kind is unsupported in the current XTAL subset. |
| `EXTAL_EXAMPLES_B64_INVALID` | x07 / lint / error | sometimes | Example contains invalid base64 payload. |
| `EXTAL_EXAMPLES_EXPECT_KIND_UNSUPPORTED` | x07 / lint / error | sometimes | Example expect encoding kind is unsupported. |
| `EXTAL_EXAMPLES_IO_READ_FAILED` | x07 / parse / error | sometimes | Cannot read examples JSONL file. |
| `EXTAL_EXAMPLES_JSON_PARSE` | x07 / parse / error | sometimes | Examples JSONL line contains invalid JSON. |
| `EXTAL_EXAMPLES_OP_UNKNOWN` | x07 / lint / error | sometimes | Example references an unknown op id. |
| `EXTAL_EXAMPLES_SCHEMA_INVALID` | x07 / parse / error | sometimes | Examples JSONL line violates the structural schema. |
| `EXTAL_EXAMPLES_SCHEMA_VERSION_UNSUPPORTED` | x07 / parse / error | sometimes | Examples schema_version is unsupported. |
| `EXTAL_GEN_DRIFT` | x07 / run / error | sometimes | Generated outputs drifted (check mode). |
| `EXTAL_GEN_INDEX_MISSING` | x07 / parse / error | sometimes | Generator index file is missing. |
| `EXTAL_GEN_NO_EXAMPLES` | x07 / run / error | sometimes | Spec modules contain no examples to generate tests from. |
| `EXTAL_GEN_NO_SPECS` | x07 / run / error | sometimes | No spec modules found for generation. |
| `EXTAL_GEN_UNSUPPORTED_TY` | x07 / run / error | sometimes | Generator cannot embed a value for the requested type/kind. |
| `EXTAL_IMPL_CONTRACT_EXTRA` | x07 / lint / warning | sometimes | Implementation has extra contract clauses beyond the spec. |
| `EXTAL_IMPL_CONTRACT_EXTRA_REQUIRES` | x07 / lint / error | sometimes | Implementation has extra requires clauses not present in the spec. |
| `EXTAL_IMPL_CONTRACT_MISMATCH` | x07 / lint / error | sometimes | Implementation contract clause does not match the spec clause. |
| `EXTAL_IMPL_CONTRACT_MISSING` | x07 / lint / error | sometimes | Implementation is missing a contract-core clause from the spec. |
| `EXTAL_IMPL_EXPORT_MISSING` | x07 / lint / error | sometimes | Operation is missing from the implementation export list. |
| `EXTAL_IMPL_IO_READ_FAILED` | x07 / parse / error | sometimes | Failed to read an implementation module file. |
| `EXTAL_IMPL_KIND_UNSUPPORTED` | x07 / lint / error | sometimes | Implementation file kind is unsupported for module conformance. |
| `EXTAL_IMPL_MODULE_ID_MISMATCH` | x07 / lint / error | sometimes | Implementation module_id does not match the spec module_id. |
| `EXTAL_IMPL_MODULE_MISSING` | x07 / lint / error | sometimes | Implementation module is missing for a spec module. |
| `EXTAL_IMPL_NO_SPECS` | x07 / parse / error | sometimes | No spec files found for implementation conformance checks. |
| `EXTAL_IMPL_PROP_DEFN_MISSING` | x07 / lint / error | sometimes | Property function definition is missing. |
| `EXTAL_IMPL_PROP_EXPORT_MISSING` | x07 / lint / error | sometimes | Property function is not exported. |
| `EXTAL_IMPL_PROP_MODULE_MISSING` | x07 / lint / error | sometimes | Property function module is missing. |
| `EXTAL_IMPL_PROP_RESULT_TYPE_MISMATCH` | x07 / lint / error | sometimes | Property function result type is invalid. |
| `EXTAL_IMPL_PROP_SIGNATURE_MISMATCH` | x07 / lint / error | sometimes | Property function signature does not match the spec. |
| `EXTAL_IMPL_SIGNATURE_MISMATCH` | x07 / lint / error | sometimes | Implementation signature does not match the spec. |
| `EXTAL_IMPL_SYNC_REQUIRED` | x07 / lint / error | sometimes | Implementation sync is required. |
| `EXTAL_IMPL_UNSUPPORTED_TY` | x07 / lower / error | sometimes | Unsupported type for generated implementation stubs. |
| `EXTAL_IMPL_X07AST_PARSE` | x07 / parse / error | sometimes | Implementation module is not valid x07AST JSON. |
| `EXTAL_IMPROVE_DISALLOWED_PATCH_PATHS` | x07 / run / error | sometimes | XTAL improve patchset touches disallowed paths. |
| `EXTAL_IMPROVE_FAILED` | x07 / run / error | sometimes | XTAL improve failed. |
| `EXTAL_IMPROVE_MANIFEST_PARSE_FAILED` | x07 / parse / error | sometimes | XTAL improve failed to parse arch/xtal/xtal.json. |
| `EXTAL_IMPROVE_NO_INCIDENTS` | x07 / parse / error | sometimes | XTAL improve found no ingestable incidents at the provided input path. |
| `EXTAL_IMPROVE_NO_PATCH` | x07 / run / error | sometimes | XTAL improve produced no patchset. |
| `EXTAL_IMPROVE_PATCH_APPLY_FAILED` | x07 / run / error | sometimes | XTAL improve failed to apply a patchset. |
| `EXTAL_IMPROVE_SPEC_CHANGE_REQUIRES_FLAG` | x07 / run / error | sometimes | XTAL improve blocked a spec change. |
| `EXTAL_IMPROVE_TASKS_FAILED` | x07 / run / error | sometimes | XTAL improve: recovery tasks failed. |
| `EXTAL_IMPROVE_WRITE_REQUIRES_MANIFEST` | x07 / parse / error | sometimes | XTAL improve --write requires an XTAL manifest. |
| `EXTAL_INGEST_FAILED` | x07 / run / error | sometimes | XTAL ingest failed. |
| `EXTAL_INGEST_INPUT_SCHEMA_INVALID` | x07 / parse / error | sometimes | XTAL ingest input is invalid. |
| `EXTAL_INGEST_INPUT_SCHEMA_VERSION_UNSUPPORTED` | x07 / parse / error | sometimes | XTAL ingest input schema_version is unsupported. |
| `EXTAL_INGEST_INTEGRITY_MISMATCH` | x07 / run / error | sometimes | XTAL ingest detected a mismatch between violation.json and repro.json. |
| `EXTAL_INGEST_REPRO_NOT_FOUND` | x07 / parse / error | sometimes | XTAL ingest could not find the repro referenced by the violation bundle. |
| `EXTAL_INGEST_REPRO_PATH_UNSAFE` | x07 / parse / error | sometimes | XTAL ingest refused an unsafe repro path in the violation bundle. |
| `EXTAL_INGEST_REPRO_SCHEMA_INVALID` | x07 / parse / error | sometimes | XTAL ingest repro.json is not schema-valid. |
| `EXTAL_REPAIR_APPLY_FAILED` | x07 / run / error | never | XTAL repair patch application failed. |
| `EXTAL_REPAIR_BASELINE_MISSING` | x07 / parse / error | never | XTAL repair baseline is missing. |
| `EXTAL_REPAIR_MANIFEST_PARSE_FAILED` | x07 / parse / error | sometimes | XTAL repair manifest parse failed. |
| `EXTAL_REPAIR_NO_ACTIONABLE_FAILURE` | x07 / run / error | never | XTAL repair could not find an actionable target. |
| `EXTAL_REPAIR_NO_PATCH_FOUND` | x07 / run / error | never | XTAL repair exhausted its budget without finding a patch. |
| `EXTAL_REPAIR_PATCHSET_WRITE_FAILED` | x07 / run / error | never | XTAL repair could not write patch artifacts. |
| `EXTAL_REPAIR_PATCH_OUTSIDE_ALLOWED_PATHS` | x07 / run / error | never | XTAL repair patch violates autonomy boundaries. |
| `EXTAL_REPAIR_SEMANTIC_NO_EXAMPLES` | x07 / run / error | sometimes | XTAL semantic repair requires examples. |
| `EXTAL_REPAIR_SEMANTIC_SEARCH_EXHAUSTED` | x07 / run / error | sometimes | XTAL semantic repair exhausted its search budget. |
| `EXTAL_REPAIR_SEMANTIC_UNSUPPORTED_RETURN_TYPE` | x07 / run / error | never | XTAL semantic repair does not support this return type. |
| `EXTAL_REPAIR_TARGET_NOT_ELIGIBLE` | x07 / run / error | never | XTAL repair refused to edit a non-stub implementation. |
| `EXTAL_REPAIR_VERIFY_FAILED` | x07 / run / error | never | XTAL repair patch did not pass verification after applying it. |
| `EXTAL_REPAIR_WRITE_REQUIRES_MANIFEST` | x07 / parse / error | never | XTAL repair --write requires an explicit autonomy manifest. |
| `EXTAL_SPEC_CONTRACT_BUILTIN_DISALLOWED` | x07 / type / error | sometimes | Contract clause uses a disallowed builtin/head. |
| `EXTAL_SPEC_CONTRACT_EXPR_NOT_I32` | x07 / type / error | sometimes | Contract clause does not typecheck to i32. |
| `EXTAL_SPEC_CONTRACT_EXPR_PARSE` | x07 / parse / error | sometimes | Contract clause expression is not valid XTAL JSON expr. |
| `EXTAL_SPEC_CONTRACT_MODULE_CALL_FORBIDDEN` | x07 / type / error | sometimes | Contract clause contains a forbidden module call. |
| `EXTAL_SPEC_CONTRACT_USES_RESULT_OUTSIDE_ENSURES` | x07 / type / error | sometimes | `__result` used outside ensures. |
| `EXTAL_SPEC_CONTRACT_WITNESS_INVALID` | x07 / type / error | sometimes | Contract witness is invalid or not contract-pure. |
| `EXTAL_SPEC_EXAMPLES_REF_MISSING` | x07 / lint / error | sometimes | Spec examples_ref points to a missing file. |
| `EXTAL_SPEC_EXTRACT_DIFFERS` | x07 / lint / error | sometimes | Extracted spec differs from the existing spec. |
| `EXTAL_SPEC_EXTRACT_IMPL_MODULE_MISSING` | x07 / parse / error | sometimes | Implementation module is missing for spec extraction. |
| `EXTAL_SPEC_EXTRACT_NO_EXPORTS` | x07 / lint / error | sometimes | No eligible exports found for spec extraction. |
| `EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE` | x07 / lint / warning | sometimes | Exported function signature is unsupported for spec extraction. |
| `EXTAL_SPEC_HAS_ASSUMPTIONS` | x07 / lint / warning | sometimes | Spec declares assumptions. |
| `EXTAL_SPEC_IDS_REQUIRED_FOR_SYNC` | x07 / lint / error | always | Contract-core clause ids are required for implementation syncing. |
| `EXTAL_SPEC_IO_READ_FAILED` | x07 / parse / error | sometimes | Cannot read spec file. |
| `EXTAL_SPEC_JSON_PARSE` | x07 / parse / error | sometimes | Spec file contains invalid JSON. |
| `EXTAL_SPEC_MODULE_ID_INVALID` | x07 / lint / error | sometimes | Spec module_id is invalid. |
| `EXTAL_SPEC_OPS_EMPTY` | x07 / lint / error | sometimes | Spec has zero operations. |
| `EXTAL_SPEC_OP_ID_DUPLICATE` | x07 / lint / error | sometimes | Spec has duplicate operation ids. |
| `EXTAL_SPEC_OP_ID_MISSING` | x07 / lint / error | sometimes | Spec operation is missing id. |
| `EXTAL_SPEC_OP_NAME_INVALID` | x07 / lint / error | sometimes | Spec operation name is invalid. |
| `EXTAL_SPEC_OP_NAME_MISSING` | x07 / lint / error | sometimes | Spec operation is missing name. |
| `EXTAL_SPEC_PARAM_NAME_DUPLICATE` | x07 / lint / error | sometimes | Spec has duplicate parameter names. |
| `EXTAL_SPEC_PARAM_NAME_INVALID` | x07 / lint / error | sometimes | Spec parameter name is invalid. |
| `EXTAL_SPEC_PARAM_TY_UNSUPPORTED` | x07 / lint / error | sometimes | Spec parameter type is unsupported in the current XTAL subset. |
| `EXTAL_SPEC_PROP_ARGS_EMPTY` | x07 / lint / error | sometimes | Property args list is empty. |
| `EXTAL_SPEC_PROP_ARG_DUPLICATE` | x07 / lint / error | sometimes | Property args contain a duplicate name. |
| `EXTAL_SPEC_PROP_ARG_NAME_INVALID` | x07 / lint / error | sometimes | Property wrapper parameter name is invalid. |
| `EXTAL_SPEC_PROP_ARG_UNKNOWN` | x07 / lint / error | sometimes | Property arg name does not match an operation parameter. |
| `EXTAL_SPEC_PROP_NAME_INVALID` | x07 / lint / error | sometimes | Property name is invalid. |
| `EXTAL_SPEC_PROP_TY_UNSUPPORTED` | x07 / lint / error | sometimes | Property argument type is unsupported for PBT generation. |
| `EXTAL_SPEC_RESULT_TY_UNSUPPORTED` | x07 / lint / error | sometimes | Spec result type is unsupported in the current XTAL subset. |
| `EXTAL_SPEC_SCHEMA_INVALID` | x07 / parse / error | sometimes | Spec JSON violates the structural schema. |
| `EXTAL_SPEC_SCHEMA_VERSION_UNSUPPORTED` | x07 / parse / error | sometimes | Spec schema_version is unsupported. |
| `EXTAL_TASKS_INDEX_INVALID` | x07 / parse / error | sometimes | Tasks index is invalid. |
| `EXTAL_TASKS_INDEX_MISSING` | x07 / parse / error | sometimes | Tasks index not found. |
| `EXTAL_TASKS_PROJECT_MISSING` | x07 / parse / error | sometimes | XTAL tasks run requires a project. |
| `EXTAL_TASKS_TASK_FAILED` | x07 / run / error | sometimes | Recovery task failed. |
| `EXTAL_TASKS_TASK_SKIPPED` | x07 / run / warning | sometimes | Recovery task was skipped. |
| `EXTAL_VERIFY_COVERAGE_FAILED` | x07 / run / error | sometimes | Coverage verification failed. |
| `EXTAL_VERIFY_PROVE_COUNTEREXAMPLE` | x07 / run / error | sometimes | Proof attempt found a counterexample. |
| `EXTAL_VERIFY_PROVE_ERROR` | x07 / run / error | sometimes | Proof attempt failed with an internal error. |
| `EXTAL_VERIFY_PROVE_TOOL_MISSING` | x07 / run / error | sometimes | Proof tool is missing or unavailable. |
| `EXTAL_VERIFY_REPORT_MISSING` | x07 / run / error | sometimes | Expected verify report was not produced. |
| `EXTAL_VERIFY_TESTS_FAILED` | x07 / run / error | sometimes | XTAL verify: x07 test failed. |
| `EXTAL_VERIFY_WORLD_UNSAFE` | x07 / lint / error | never | XTAL verify requires a deterministic world by default. |
| `E_ARCH_ARCHIVE_BUDGET_PROFILE_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_ARCHIVE_BUDGET_PROFILE_MISSING`. |
| `E_ARCH_ARCHIVE_OP_UNKNOWN` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_ARCHIVE_OP_UNKNOWN`. |
| `E_ARCH_ARCHIVE_SCHEMA_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_ARCHIVE_SCHEMA_INVALID`. |
| `E_ARCH_BOUNDARY_BRAND_OR_SCHEMA_REQUIRED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BOUNDARY_BRAND_OR_SCHEMA_REQUIRED`. |
| `E_ARCH_BOUNDARY_ERROR_SPACE_REQUIRED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BOUNDARY_ERROR_SPACE_REQUIRED`. |
| `E_ARCH_BOUNDARY_INDEX_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BOUNDARY_INDEX_INVALID`. |
| `E_ARCH_BOUNDARY_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BOUNDARY_MISSING`. |
| `E_ARCH_BOUNDARY_VERIFY_REQUIRED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BOUNDARY_VERIFY_REQUIRED`. |
| `E_ARCH_BUDGETS_DIR_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGETS_DIR_MISSING`. |
| `E_ARCH_BUDGETS_GEN_DIR_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGETS_GEN_DIR_MISSING`. |
| `E_ARCH_BUDGETS_INDEX_SCHEMA_VERSION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGETS_INDEX_SCHEMA_VERSION`. |
| `E_ARCH_BUDGET_PROFILE_NOT_FOUND` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGET_PROFILE_NOT_FOUND`. |
| `E_ARCH_BUDGET_PROFILE_SCHEMA_VERSION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGET_PROFILE_SCHEMA_VERSION`. |
| `E_ARCH_BUDGET_SCOPE_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGET_SCOPE_MISSING`. |
| `E_ARCH_BUDGET_SELECTOR_FN_NOT_FOUND` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGET_SELECTOR_FN_NOT_FOUND`. |
| `E_ARCH_BUDGET_SELECTOR_MODULE_PREFIX_NOT_FOUND` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGET_SELECTOR_MODULE_PREFIX_NOT_FOUND`. |
| `E_ARCH_BUDGET_WORLD_VIOLATION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_BUDGET_WORLD_VIOLATION`. |
| `E_ARCH_CLI_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CLI_INDEX_MISSING`. |
| `E_ARCH_CLI_PROFILE_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CLI_PROFILE_INVALID`. |
| `E_ARCH_CONTRACTS_CANONICAL_JSON_UNSUPPORTED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CONTRACTS_CANONICAL_JSON_UNSUPPORTED`. |
| `E_ARCH_CONTRACTS_LOCK_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CONTRACTS_LOCK_INVALID`. |
| `E_ARCH_CONTRACTS_LOCK_JSON_PARSE` | x07 / parse / error | sometimes | Architecture contract diagnostic `E_ARCH_CONTRACTS_LOCK_JSON_PARSE`. |
| `E_ARCH_CONTRACTS_LOCK_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CONTRACTS_LOCK_MISMATCH`. |
| `E_ARCH_CONTRACTS_LOCK_SCHEMA_VERSION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CONTRACTS_LOCK_SCHEMA_VERSION`. |
| `E_ARCH_CRAWL_BUDGET_PROFILE_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CRAWL_BUDGET_PROFILE_MISSING`. |
| `E_ARCH_CRAWL_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CRAWL_INDEX_MISSING`. |
| `E_ARCH_CRAWL_POLICY_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CRAWL_POLICY_INVALID`. |
| `E_ARCH_CRYPTO_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CRYPTO_INDEX_MISSING`. |
| `E_ARCH_CRYPTO_JWT_PROFILE_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CRYPTO_JWT_PROFILE_INVALID`. |
| `E_ARCH_CRYPTO_KEY_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CRYPTO_KEY_MISSING`. |
| `E_ARCH_CYCLE` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_CYCLE`. |
| `E_ARCH_DB_BUDGET_PROFILE_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_DB_BUDGET_PROFILE_MISSING`. |
| `E_ARCH_DB_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_DB_INDEX_MISSING`. |
| `E_ARCH_DB_MIGRATE_PLAN_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_DB_MIGRATE_PLAN_MISSING`. |
| `E_ARCH_DB_QUERY_CATALOG_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_DB_QUERY_CATALOG_INVALID`. |
| `E_ARCH_DB_SQL_HASH_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_DB_SQL_HASH_MISMATCH`. |
| `E_ARCH_DB_SQL_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_DB_SQL_MISSING`. |
| `E_ARCH_DEPS_DENY` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_DEPS_DENY`. |
| `E_ARCH_DUPLICATE_MODULE_ID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_DUPLICATE_MODULE_ID`. |
| `E_ARCH_EDGE_NOT_ALLOWED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_EDGE_NOT_ALLOWED`. |
| `E_ARCH_EXTERNAL_IMPORT_NOT_ALLOWED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_EXTERNAL_IMPORT_NOT_ALLOWED`. |
| `E_ARCH_IMPORT_PREFIX_DENIED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_IMPORT_PREFIX_DENIED`. |
| `E_ARCH_IMPORT_PREFIX_NOT_ALLOWED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_IMPORT_PREFIX_NOT_ALLOWED`. |
| `E_ARCH_LAYERS_VIOLATION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_LAYERS_VIOLATION`. |
| `E_ARCH_LOCK_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_LOCK_INVALID`. |
| `E_ARCH_LOCK_JSON_PARSE` | x07 / parse / error | sometimes | Architecture contract diagnostic `E_ARCH_LOCK_JSON_PARSE`. |
| `E_ARCH_LOCK_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_LOCK_MISMATCH`. |
| `E_ARCH_LOCK_READ` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_LOCK_READ`. |
| `E_ARCH_MANIFEST_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MANIFEST_INVALID`. |
| `E_ARCH_MANIFEST_JSON_PARSE` | x07 / parse / error | sometimes | Architecture contract diagnostic `E_ARCH_MANIFEST_JSON_PARSE`. |
| `E_ARCH_MANIFEST_READ` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MANIFEST_READ`. |
| `E_ARCH_MODULE_PARSE` | x07 / parse / error | sometimes | Architecture contract diagnostic `E_ARCH_MODULE_PARSE`. |
| `E_ARCH_MSG_AMQP_BUDGET_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MSG_AMQP_BUDGET_MISSING`. |
| `E_ARCH_MSG_AMQP_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MSG_AMQP_INDEX_MISSING`. |
| `E_ARCH_MSG_AMQP_PROFILE_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MSG_AMQP_PROFILE_INVALID`. |
| `E_ARCH_MSG_AMQP_TOPOLOGY_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MSG_AMQP_TOPOLOGY_INVALID`. |
| `E_ARCH_MSG_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MSG_INDEX_MISSING`. |
| `E_ARCH_MSG_KAFKA_BUDGET_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MSG_KAFKA_BUDGET_MISSING`. |
| `E_ARCH_MSG_KAFKA_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MSG_KAFKA_INDEX_MISSING`. |
| `E_ARCH_MSG_KAFKA_PROFILE_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_MSG_KAFKA_PROFILE_INVALID`. |
| `E_ARCH_NET_BUDGET_PROFILE_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_NET_BUDGET_PROFILE_MISSING`. |
| `E_ARCH_NET_GRPC_SERVICES_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_NET_GRPC_SERVICES_INVALID`. |
| `E_ARCH_NET_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_NET_INDEX_MISSING`. |
| `E_ARCH_NET_LIMITS_EXCESSIVE` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_NET_LIMITS_EXCESSIVE`. |
| `E_ARCH_NODE_ORPHAN_MODULE` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_NODE_ORPHAN_MODULE`. |
| `E_ARCH_NODE_OVERLAP_MODULE` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_NODE_OVERLAP_MODULE`. |
| `E_ARCH_OBS_BUDGET_PROFILE_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_OBS_BUDGET_PROFILE_MISSING`. |
| `E_ARCH_OBS_EXPORTER_PROFILE_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_OBS_EXPORTER_PROFILE_INVALID`. |
| `E_ARCH_OBS_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_OBS_INDEX_MISSING`. |
| `E_ARCH_OBS_REGISTRY_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_OBS_REGISTRY_MISSING`. |
| `E_ARCH_PUBLIC_BYTES_UNBRANDED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_PUBLIC_BYTES_UNBRANDED`. |
| `E_ARCH_RR_CASSETTE_BRAND_UNSUPPORTED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_CASSETTE_BRAND_UNSUPPORTED`. |
| `E_ARCH_RR_DIR_MISSING` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_DIR_MISSING`. |
| `E_ARCH_RR_GEN_DIR_MISSING` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_GEN_DIR_MISSING`. |
| `E_ARCH_RR_INDEX_DEFAULTS_DUP` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_INDEX_DEFAULTS_DUP`. |
| `E_ARCH_RR_INDEX_DEFAULTS_NOT_SORTED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_INDEX_DEFAULTS_NOT_SORTED`. |
| `E_ARCH_RR_INDEX_NOT_SORTED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_INDEX_NOT_SORTED`. |
| `E_ARCH_RR_INDEX_SCHEMA_VERSION` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_INDEX_SCHEMA_VERSION`. |
| `E_ARCH_RR_OP_KIND_NOT_ALLOWED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_OP_KIND_NOT_ALLOWED`. |
| `E_ARCH_RR_OP_KIND_UNKNOWN` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_OP_KIND_UNKNOWN`. |
| `E_ARCH_RR_OP_NOT_ALLOWED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_OP_NOT_ALLOWED`. |
| `E_ARCH_RR_POLICY_ID_MISMATCH` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_POLICY_ID_MISMATCH`. |
| `E_ARCH_RR_POLICY_KINDS_NOT_SORTED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_POLICY_KINDS_NOT_SORTED`. |
| `E_ARCH_RR_POLICY_MODE_NOT_ALLOWED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_POLICY_MODE_NOT_ALLOWED`. |
| `E_ARCH_RR_POLICY_NOT_FOUND` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_POLICY_NOT_FOUND`. |
| `E_ARCH_RR_POLICY_OPS_NOT_SORTED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_POLICY_OPS_NOT_SORTED`. |
| `E_ARCH_RR_POLICY_REQUIRED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_POLICY_REQUIRED`. |
| `E_ARCH_RR_POLICY_SCHEMA_VERSION` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_POLICY_SCHEMA_VERSION`. |
| `E_ARCH_RR_POLICY_WORLDS_NOT_SORTED` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_POLICY_WORLDS_NOT_SORTED`. |
| `E_ARCH_RR_SANITIZER_ID_MISMATCH` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_SANITIZER_ID_MISMATCH`. |
| `E_ARCH_RR_SANITIZER_SCHEMA_VERSION` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_SANITIZER_SCHEMA_VERSION`. |
| `E_ARCH_RR_WORLD_VIOLATION` | x07 / run / error | sometimes | Architecture contract diagnostic `E_ARCH_RR_WORLD_VIOLATION`. |
| `E_ARCH_SMOKE_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SMOKE_MISSING`. |
| `E_ARCH_SM_DIR_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_DIR_MISSING`. |
| `E_ARCH_SM_GEN_DIR_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_GEN_DIR_MISSING`. |
| `E_ARCH_SM_GEN_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_GEN_MISSING`. |
| `E_ARCH_SM_GEN_PARSE` | x07 / parse / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_GEN_PARSE`. |
| `E_ARCH_SM_GEN_STALE` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_GEN_STALE`. |
| `E_ARCH_SM_IMPORT_NOT_INDEXED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_IMPORT_NOT_INDEXED`. |
| `E_ARCH_SM_INDEX_SCHEMA_VERSION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_INDEX_SCHEMA_VERSION`. |
| `E_ARCH_SM_SPEC_MACHINE_ID_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_SPEC_MACHINE_ID_MISMATCH`. |
| `E_ARCH_SM_SPEC_SCHEMA_VERSION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_SPEC_SCHEMA_VERSION`. |
| `E_ARCH_SM_SPEC_VERSION_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_SPEC_VERSION_MISMATCH`. |
| `E_ARCH_SM_SPEC_WORLD_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_SM_SPEC_WORLD_MISMATCH`. |
| `E_ARCH_STREAM_PLUGINS_DIR_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_DIR_MISSING`. |
| `E_ARCH_STREAM_PLUGINS_GEN_DIR_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_GEN_DIR_MISSING`. |
| `E_ARCH_STREAM_PLUGINS_INDEX_DUPLICATE_ID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_INDEX_DUPLICATE_ID`. |
| `E_ARCH_STREAM_PLUGINS_INDEX_NOT_SORTED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_INDEX_NOT_SORTED`. |
| `E_ARCH_STREAM_PLUGINS_INDEX_SCHEMA_VERSION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_INDEX_SCHEMA_VERSION`. |
| `E_ARCH_STREAM_PLUGIN_BUDGET_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_BUDGET_MISSING`. |
| `E_ARCH_STREAM_PLUGIN_NOT_FOUND` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_NOT_FOUND`. |
| `E_ARCH_STREAM_PLUGIN_SPEC_ABI_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_ABI_MISMATCH`. |
| `E_ARCH_STREAM_PLUGIN_SPEC_BRANDS_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_BRANDS_MISMATCH`. |
| `E_ARCH_STREAM_PLUGIN_SPEC_BUDGETS_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_BUDGETS_MISMATCH`. |
| `E_ARCH_STREAM_PLUGIN_SPEC_CFG_CANON_MODE` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_CFG_CANON_MODE`. |
| `E_ARCH_STREAM_PLUGIN_SPEC_DETERMINISM_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_DETERMINISM_MISMATCH`. |
| `E_ARCH_STREAM_PLUGIN_SPEC_ID_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_ID_MISMATCH`. |
| `E_ARCH_STREAM_PLUGIN_SPEC_LIMITS_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_LIMITS_INVALID`. |
| `E_ARCH_STREAM_PLUGIN_SPEC_SCHEMA_VERSION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_SCHEMA_VERSION`. |
| `E_ARCH_STREAM_PLUGIN_SPEC_WORLDS_MISMATCH` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_WORLDS_MISMATCH`. |
| `E_ARCH_STREAM_PLUGIN_WORLD_VIOLATION` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_WORLD_VIOLATION`. |
| `E_ARCH_TASKS_INDEX_INVALID` | x07 / parse / error | sometimes | Architecture tasks index is invalid. |
| `E_ARCH_TASKS_INDEX_MISSING` | x07 / parse / error | sometimes | Architecture tasks index is missing. |
| `E_ARCH_TOOL_BUDGET_EXCEEDED` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_TOOL_BUDGET_EXCEEDED`. |
| `E_ARCH_TRUST_ZONE_EDGE` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_TRUST_ZONE_EDGE`. |
| `E_ARCH_VISIBILITY` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_VISIBILITY`. |
| `E_ARCH_WEB_API_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_WEB_API_INVALID`. |
| `E_ARCH_WEB_BUDGET_PROFILE_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_WEB_BUDGET_PROFILE_MISSING`. |
| `E_ARCH_WEB_INDEX_MISSING` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_WEB_INDEX_MISSING`. |
| `E_ARCH_WEB_OPENAPI_PROFILE_INVALID` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_WEB_OPENAPI_PROFILE_INVALID`. |
| `E_ARCH_WORLD_EDGE_FORBIDDEN` | x07 / lint / error | sometimes | Architecture contract diagnostic `E_ARCH_WORLD_EDGE_FORBIDDEN`. |
| `E_BENCH_SUITE_LOAD` | x07 / lint / error | sometimes | Diagnostic code `E_BENCH_SUITE_LOAD`. |
| `E_DEPS_CAP_POLICY_DENY` | x07 / lint / error | sometimes | Diagnostic code `E_DEPS_CAP_POLICY_DENY`. |
| `E_GEN_DRIFT` | x07 / run / error | sometimes | Committed generator outputs drifted. |
| `E_GEN_ENTRY_CHECK_ARGV_CONTAINS_WRITE` | x07 / parse / error | sometimes | Generator index entry check_argv contains --write. |
| `E_GEN_ENTRY_CHECK_ARGV_EMPTY` | x07 / parse / error | sometimes | Generator index entry check_argv is empty. |
| `E_GEN_ENTRY_MODE_UNSUPPORTED` | x07 / parse / error | sometimes | Generator index entry mode is unsupported. |
| `E_GEN_ENTRY_OUTPUTS_EMPTY` | x07 / parse / error | sometimes | Generator index entry outputs is empty. |
| `E_GEN_ENTRY_WRITE_ARGV_CONTAINS_CHECK` | x07 / parse / error | sometimes | Generator index entry write_argv contains --check. |
| `E_GEN_ENTRY_WRITE_ARGV_EMPTY` | x07 / parse / error | sometimes | Generator index entry write_argv is empty. |
| `E_GEN_INDEX_IO_READ_FAILED` | x07 / parse / error | sometimes | Cannot read generator index file. |
| `E_GEN_INDEX_JSON_PARSE` | x07 / parse / error | sometimes | Generator index file contains invalid JSON. |
| `E_GEN_INDEX_SCHEMA_INVALID` | x07 / parse / error | sometimes | Generator index violates the structural schema. |
| `E_GEN_INDEX_SCHEMA_VERSION_UNSUPPORTED` | x07 / parse / error | sometimes | Generator index schema_version is unsupported. |
| `E_GEN_MISSING_OUTPUT` | x07 / run / error | sometimes | Declared generator output root is missing. |
| `E_GEN_NONDETERMINISTIC` | x07 / run / error | sometimes | Generator outputs are not deterministic. |
| `E_GEN_RUN_FAILED` | x07 / run / error | sometimes | Generator command failed. |
| `E_SBOM_GENERATION_FAILED` | x07 / lint / error | sometimes | Diagnostic code `E_SBOM_GENERATION_FAILED`. |
| `WXTAL_IMPL_PARAM_NAME_MISMATCH` | x07 / lint / warning | sometimes | Implementation parameter names differ from the spec. |
| `WXTAL_IMPROVE_NO_PATCH` | x07 / run / warning | sometimes | XTAL improve produced no patchset. |
| `WXTAL_REPAIR_QUICKFIX_APPLIED` | x07 / run / warning | never | XTAL repair applied a quickfix fallback in an attempt workspace. |
| `WXTAL_REPAIR_SPEC_PATCH_SUGGESTED` | x07 / run / warning | sometimes | XTAL emitted a spec patch suggestion for review. |
| `WXTAL_SPEC_NONCANONICAL_JSON` | x07 / rewrite / warning | always | Spec JSON is not in canonical form. |
| `WXTAL_VERIFY_PROVE_INCONCLUSIVE` | x07 / run / warning | sometimes | Proof attempt was inconclusive. |
| `WXTAL_VERIFY_PROVE_TIMEOUT` | x07 / run / warning | sometimes | Proof attempt hit the configured budget. |
| `WXTAL_VERIFY_PROVE_TOOL_MISSING` | x07 / run / warning | sometimes | Proof tool is missing or unavailable. |
| `WXTAL_VERIFY_PROVE_UNSUPPORTED` | x07 / run / warning | sometimes | Proof attempt is unsupported for this entry. |
| `W_ARCH_CONTRACTS_LOCK_MISSING` | x07 / lint / warning | sometimes | Architecture contract diagnostic `W_ARCH_CONTRACTS_LOCK_MISSING`. |
| `W_ARCH_CONTRACT_OPAQUE_USAGE` | x07 / lint / warning | sometimes | Architecture contract diagnostic `W_ARCH_CONTRACT_OPAQUE_USAGE`. |
| `W_DEPS_CAP_POLICY_MISSING` | x07 / lint / warning | sometimes | Diagnostic code `W_DEPS_CAP_POLICY_MISSING`. |
| `X07-ARITY-0000` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-0000`. |
| `X07-ARITY-BEGIN-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-BEGIN-0001`. |
| `X07-ARITY-BINOP-0001` | x07c / lint / error | sometimes | Binary operator has invalid arity. |
| `X07-ARITY-FOR-0001` | x07c / lint / error | sometimes | `for` has invalid arity. |
| `X07-ARITY-IF-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-IF-0001`. |
| `X07-ARITY-LET-0001` | x07c / lint / error | sometimes | `let`/`set` has invalid arity. |
| `X07-ARITY-RETURN-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-RETURN-0001`. |
| `X07-ARITY-UNSAFE-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-UNSAFE-0001`. |
| `X07-ARITY-WHILE-0001` | x07c / lint / error | sometimes | `while` has invalid arity. |
| `X07-AST-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-AST-0001`. |
| `X07-ASYNC-CONTRACT-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ASYNC-CONTRACT-0001`. |
| `X07-ASYNC-CONTRACT-0002` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ASYNC-CONTRACT-0002`. |
| `X07-ASYNC-CONTRACT-0003` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ASYNC-CONTRACT-0003`. |
| `X07-BOOL-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-BOOL-0001`. |
| `X07-BORROW-0001` | x07c / lint / error | sometimes | Borrowing view/subview from a temporary expression is invalid. |
| `X07-BORROW-0002` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-BORROW-0002`. |
| `X07-CLI-ARGS-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-CLI-ARGS-0001`. |
| `X07-CLI-ARGS-0002` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-CLI-ARGS-0002`. |
| `X07-CONTRACT-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-CONTRACT-0001`. |
| `X07-CONTRACT-0002` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-CONTRACT-0002`. |
| `X07-CONTRACT-0003` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-CONTRACT-0003`. |
| `X07-CONTRACT-0004` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-CONTRACT-0004`. |
| `X07-CONTRACT-0005` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-CONTRACT-0005`. |
| `X07-CONTRACT-0009` | x07c / lint / error | sometimes | Recursive decreases clause is invalid. |
| `X07-CONTRACT-0010` | x07c / lint / error | sometimes | Recursive decreases metadata is attached to a non-certifiable target. |
| `X07-CONTRACT-0011` | x07c / lint / error | sometimes | Recursive self-call is missing decreases evidence. |
| `X07-FIX-0003` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-FIX-0003`. |
| `X07-FMT-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-FMT-0001`. |
| `X07-FOR-0001` | x07c / lint / error | never | `for` loop variable must be an identifier. |
| `X07-GENERICS-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-GENERICS-0001`. |
| `X07-GENERICS-0002` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-GENERICS-0002`. |
| `X07-INTERNAL-0001` | x07 / lint / error | sometimes | Internal toolchain/compiler error. |
| `X07-IO-READ-0001` | x07 / lint / error<br/>x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-IO-READ-0001`. |
| `X07-IO-WRITE-0001` | x07 / lint / error<br/>x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-IO-WRITE-0001`. |
| `X07-JSON-PARSE-0001` | x07c / parse / error | sometimes | Core lint/schema diagnostic `X07-JSON-PARSE-0001`. |
| `X07-MIGRATE-COERCE-0001` | x07c / lint / error | sometimes | Migration required: explicitize compat coercions. |
| `X07-MOVE-0001` | x07c / lint / error | always | `bytes.concat` uses the same identifier on both sides (use-after-move risk). |
| `X07-MOVE-0002` | x07c / lint / error | always | `if` condition and branch both borrow `bytes.view` from the same owner. |
| `X07-PATCHSET-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-PATCHSET-0001`. |
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
| `X07-SCHEMA-0002` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-SCHEMA-0002`. |
| `X07-TAPP-ELAB-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-TAPP-ELAB-0001`. |
| `X07-TAPP-INFER-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-TAPP-INFER-0001`. |
| `X07-TOOL-ARGS-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-TOOL-ARGS-0001`. |
| `X07-TOOL-EXEC-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-TOOL-EXEC-0001`. |
| `X07-TYPE-0001` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-0001`. |
| `X07-TYPE-CALL-0001` | x07c / type / error | sometimes | Unknown callee (function not found). |
| `X07-TYPE-CALL-0002` | x07c / type / error | sometimes | Call argument type mismatch. |
| `X07-TYPE-CALL-0003` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-CALL-0003`. |
| `X07-TYPE-COERCE-0001` | x07c / type / warning | sometimes | Implicit call-argument compat coercion occurred. |
| `X07-TYPE-IF-0001` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-IF-0001`. |
| `X07-TYPE-IF-0002` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-IF-0002`. |
| `X07-TYPE-RET-0001` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-RET-0001`. |
| `X07-TYPE-SET-0001` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-SET-0001`. |
| `X07-TYPE-SET-0002` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-SET-0002`. |
| `X07-TYPE-TRYDOC-0001` | x07c / type / error | sometimes | `try_doc` expects bytes or bytes_view. |
| `X07-TYPE-UNIFY-0001` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-UNIFY-0001`. |
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
| `X07-X07AST-PARSE-0001` | x07 / parse / error<br/>x07c / parse / error | sometimes | Core lint/schema diagnostic `X07-X07AST-PARSE-0001`. |
| `X07CAP_ATTEST_DIGEST_MISMATCH` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07CAP_ATTEST_DIGEST_MISMATCH`. |
| `X07CAP_CONFORMANCE_MISSING` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07CAP_CONFORMANCE_MISSING`. |
| `X07CAP_CONFORMANCE_REQUIRED` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07CAP_CONFORMANCE_REQUIRED`. |
| `X07CAP_CONTRACT_INVALID` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07CAP_CONTRACT_INVALID`. |
| `X07CAP_EFFECT_LOG_REQUIRED` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07CAP_EFFECT_LOG_REQUIRED`. |
| `X07CAP_INDEX_INVALID` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07CAP_INDEX_INVALID`. |
| `X07CAP_PEER_POLICY_REQUIRED` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07CAP_PEER_POLICY_REQUIRED`. |
| `X07CAP_TLS_POLICY_INCOMPLETE` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07CAP_TLS_POLICY_INCOMPLETE`. |
| `X07C_WASM_BACKEND_FEATURE_REQUIRED` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07C_WASM_BACKEND_FEATURE_REQUIRED`. |
| `X07C_WASM_BACKEND_UNSUPPORTED_BUILTIN` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07C_WASM_BACKEND_UNSUPPORTED_BUILTIN`. |
| `X07C_WASM_BACKEND_UNSUPPORTED_FORM` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07C_WASM_BACKEND_UNSUPPORTED_FORM`. |
| `X07C_WASM_BACKEND_UNSUPPORTED_OP` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07C_WASM_BACKEND_UNSUPPORTED_OP`. |
| `X07C_WASM_BACKEND_UNSUPPORTED_TYPE` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07C_WASM_BACKEND_UNSUPPORTED_TYPE`. |
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
| `X07INIT_TRUST` | x07 / lint / error | sometimes | Project/package scaffold diagnostic `X07INIT_TRUST`. |
| `X07PKG_ADVISED_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_ADVISED_DEP`. |
| `X07PKG_API_FETCH` | x07 / lint / error | never | Registry API request failed. |
| `X07PKG_API_PARSE` | x07 / lint / error | never | Registry API response is invalid. |
| `X07PKG_API_URL` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_API_URL`. |
| `X07PKG_DEP_EXISTS` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_DEP_EXISTS`. |
| `X07PKG_DEP_NOT_FOUND` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_DEP_NOT_FOUND`. |
| `X07PKG_DOWNLOAD_FAILED` | x07 / lint / error | never | Diagnostic code `X07PKG_DOWNLOAD_FAILED`. |
| `X07PKG_INDEX_CONFIG` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_INDEX_CONFIG`. |
| `X07PKG_INDEX_FETCH` | x07 / lint / error | never | Diagnostic code `X07PKG_INDEX_FETCH`. |
| `X07PKG_INDEX_NO_MATCH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_INDEX_NO_MATCH`. |
| `X07PKG_LIST_INDEX_MISSING` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LIST_INDEX_MISSING`. |
| `X07PKG_LIST_UNSUPPORTED` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LIST_UNSUPPORTED`. |
| `X07PKG_LOCAL_MISSING_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOCAL_MISSING_DEP`. |
| `X07PKG_LOCK_MISMATCH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOCK_MISMATCH`. |
| `X07PKG_LOCK_MISSING` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOCK_MISSING`. |
| `X07PKG_LOCK_VERSION_INVALID` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOCK_VERSION_INVALID`. |
| `X07PKG_LOGIN_FAILED` | x07 / lint / error | never | Diagnostic code `X07PKG_LOGIN_FAILED`. |
| `X07PKG_LOGIN_TOKEN` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOGIN_TOKEN`. |
| `X07PKG_MANIFEST_PARSE` | x07 / lint / error | sometimes | Package manifest could not be parsed. |
| `X07PKG_OFFLINE_INDEX` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_OFFLINE_INDEX`. |
| `X07PKG_OFFLINE_MISSING_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_OFFLINE_MISSING_DEP`. |
| `X07PKG_OFFLINE_REFRESH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_OFFLINE_REFRESH`. |
| `X07PKG_PACK_FAILED` | x07 / lint / error | sometimes | Package packing failed. |
| `X07PKG_PACK_OUT` | x07 / lint / error | sometimes | Missing output path for package packing. |
| `X07PKG_PACK_WRITE` | x07 / lint / error | sometimes | Failed to write packed archive. |
| `X07PKG_PATCH_MISSING_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_PATCH_MISSING_DEP`. |
| `X07PKG_PUBLISH_FAILED` | x07 / lint / error | never | Diagnostic code `X07PKG_PUBLISH_FAILED`. |
| `X07PKG_PUBLISH_PACK` | x07 / lint / error | sometimes | Publish failed while packing the package. |
| `X07PKG_PUBLISH_RESPONSE` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_PUBLISH_RESPONSE`. |
| `X07PKG_PUBLISH_RESPONSE_MISMATCH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_PUBLISH_RESPONSE_MISMATCH`. |
| `X07PKG_REPAIR_LOCAL_INCOMPATIBLE` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_REPAIR_LOCAL_INCOMPATIBLE`. |
| `X07PKG_REPAIR_LOCK_INVALID` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_REPAIR_LOCK_INVALID`. |
| `X07PKG_REPAIR_LOCK_MISSING` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_REPAIR_LOCK_MISSING`. |
| `X07PKG_REPAIR_NO_COMPATIBLE_VERSION` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_REPAIR_NO_COMPATIBLE_VERSION`. |
| `X07PKG_REPAIR_PATCHED_PATH_INCOMPATIBLE` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_REPAIR_PATCHED_PATH_INCOMPATIBLE`. |
| `X07PKG_REPAIR_TOOLCHAIN_INVALID` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_REPAIR_TOOLCHAIN_INVALID`. |
| `X07PKG_REPAIR_TOOLCHAIN_UNSUPPORTED` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_REPAIR_TOOLCHAIN_UNSUPPORTED`. |
| `X07PKG_SEMVER_BREAKING` | x07 / lint / error | sometimes | Breaking API changes detected. |
| `X07PKG_SEMVER_DIR` | x07 / lint / error | sometimes | Semver check input directory is invalid. |
| `X07PKG_SEMVER_LOAD` | x07 / lint / error | sometimes | Failed to load package surface for semver check. |
| `X07PKG_SEMVER_NAME_MISMATCH` | x07 / lint / error | sometimes | Package names do not match for semver check. |
| `X07PKG_SIGNATURE_INVALID` | x07 / lint / error | never | Package signature is invalid. |
| `X07PKG_SIGNATURE_KEY_MISSING` | x07 / lint / error | never | Signing key is missing from index config. |
| `X07PKG_SIGNATURE_UNSUPPORTED` | x07 / lint / error | never | Unsupported signature kind. |
| `X07PKG_SPEC_INVALID` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_SPEC_INVALID`. |
| `X07PKG_TRANSITIVE_MISSING` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_TRANSITIVE_MISSING`. |
| `X07PKG_TREE_DEP_MANIFEST` | x07 / lint / error | sometimes | pkg.tree could not read a dependency package manifest. |
| `X07PKG_TREE_DEP_PATH` | x07 / lint / error | sometimes | pkg.tree could not resolve a dependency path. |
| `X07PKG_TREE_LOCK_MISSING` | x07 / lint / error | sometimes | pkg.tree requires a lockfile. |
| `X07PKG_TREE_LOCK_PARSE` | x07 / lint / error | sometimes | pkg.tree lockfile is not valid JSON. |
| `X07PKG_TREE_LOCK_READ` | x07 / lint / error | sometimes | pkg.tree could not read the lockfile. |
| `X07PKG_TREE_LOCK_VERIFY` | x07 / lint / error | sometimes | pkg.tree lockfile does not match the project manifest. |
| `X07PKG_TREE_MODULE_ROOTS` | x07 / lint / error | sometimes | pkg.tree could not compute resolved module roots. |
| `X07PKG_TREE_PROJECT` | x07 / lint / error | sometimes | pkg.tree failed to load the project manifest. |
| `X07PKG_TREE_REQUIRES_INVALID` | x07 / lint / error | sometimes | pkg.tree found an invalid `meta.requires_packages` entry. |
| `X07PKG_UNSIGNED` | x07 / lint / error | sometimes | Package or index is unsigned. |
| `X07PKG_X07C_COMPAT_INVALID` | x07 / lint / error | sometimes | Package compatibility metadata is invalid. |
| `X07PKG_X07C_INCOMPATIBLE` | x07 / lint / error | sometimes | Package is incompatible with the current compiler. |
| `X07PKG_YANKED_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_YANKED_DEP`. |
| `X07PROOF_ECHECK_FAILED` | x07 / run / error | sometimes | Independent proof check rejected the proof bundle. |
| `X07PROOF_EIMPORTED_SUMMARY_MISMATCH` | x07 / run / error | sometimes | Semantic proof replay found imported proof-summary drift. |
| `X07PROOF_EOBJECT_INVALID` | x07 / run / error | sometimes | Proof object failed schema or structural validation. |
| `X07PROOF_EOBLIGATION_MISMATCH` | x07 / run / error | sometimes | Semantic proof replay regenerated a different verification obligation. |
| `X07PROOF_ESCHEDULER_MODEL_MISMATCH` | x07 / run / error | sometimes | Semantic async proof replay found a scheduler-model mismatch. |
| `X07PROOF_ESOLVER_REPLAY_FAILED` | x07 / run / error | sometimes | Semantic proof replay did not reproduce the expected solver verdict. |
| `X07PROOF_ESOURCE_REPLAY_FAILED` | x07 / run / error | sometimes | Semantic proof replay could not reconstruct the proved source surface. |
| `X07RD_ADVISORY_ALLOWANCE_ENABLED` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_ADVISORY_ALLOWANCE_ENABLED`. |
| `X07RD_ASSUMPTION_SURFACE_WIDEN` | x07 / lint / error | never | Review diff gate rejected a wider proof-assumption surface. |
| `X07RD_ASYNC_PROOF_COVERAGE_DECREASE` | x07 / lint / error | never | Review diff gate rejected an async proof coverage regression. |
| `X07RD_BOUNDED_PROOF_INTRODUCED` | x07 / lint / error | never | Review diff gate rejected bounded proof usage. |
| `X07RD_CAPSULE_CONTRACT_RELAXATION` | x07 / lint / error | never | Review diff gate rejected a capsule contract relaxation. |
| `X07RD_CAPSULE_NETWORK_SURFACE_WIDEN` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_CAPSULE_NETWORK_SURFACE_WIDEN`. |
| `X07RD_CAPSULE_SET_CHANGE` | x07 / lint / error | never | Review diff gate rejected a capsule inventory change. |
| `X07RD_COVERAGE_SUMMARY_IMPORTED` | x07 / lint / error | never | Review diff gate rejected a coverage-only import. |
| `X07RD_DEV_ONLY_ASSUMPTION_INTRODUCED` | x07 / lint / error | never | Review diff gate rejected a developer-only proof assumption. |
| `X07RD_LOCKFILE_HASH_CHANGE` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_LOCKFILE_HASH_CHANGE`. |
| `X07RD_NETWORK_ALLOWLIST_WIDEN` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_NETWORK_ALLOWLIST_WIDEN`. |
| `X07RD_OPERATIONAL_ENTRY_DIVERGES` | x07 / lint / error | never | Review diff gate rejected operational-entry divergence. |
| `X07RD_PACKAGE_SET_CHANGE` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_PACKAGE_SET_CHANGE`. |
| `X07RD_PEER_POLICY_RELAXATION` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_PEER_POLICY_RELAXATION`. |
| `X07RD_RECURSION_PROOF_COVERAGE_DECREASE` | x07 / lint / error | never | Review diff gate rejected a recursive proof coverage regression. |
| `X07RD_RUNTIME_ATTEST_REGRESSION` | x07 / run / error | never | Review diff gate rejected a runtime attestation regression. |
| `X07RD_SANDBOX_POLICY_WIDEN` | x07 / lint / error | never | Review diff gate rejected a sandbox policy widening. |
| `X07RD_SUMMARY_DOWNGRADE` | x07 / lint / error | never | Review diff gate rejected a proof-summary downgrade. |
| `X07REL_EBOUNDED_PROOF` | x07 / lint / error | sometimes | Release guard rejected bounded proof usage in the strict fixture. |
| `X07REL_ECOVERAGE_ONLY_IMPORT` | x07 / lint / error | sometimes | Release guard rejected a coverage-only imported summary in the strict fixture. |
| `X07REL_EDEV_ONLY_ASSUMPTION` | x07 / lint / error | sometimes | Release guard rejected a developer-only proof assumption in the strict fixture. |
| `X07REL_ESURROGATE_ENTRY` | x07 / lint / error | sometimes | Release guard rejected surrogate-entry certification in the strict fixture. |
| `X07RR_ENTRY_EXISTS` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_ENTRY_EXISTS`. |
| `X07RR_HTTP` | x07 / run / error | never | Diagnostic code `X07RR_HTTP`. |
| `X07RR_KEY_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_KEY_EMPTY`. |
| `X07RR_KIND_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_KIND_EMPTY`. |
| `X07RR_LATENCY_OUT_OF_RANGE` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_LATENCY_OUT_OF_RANGE`. |
| `X07RR_OP_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_OP_EMPTY`. |
| `X07RR_URL_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_URL_EMPTY`. |
| `X07TC_EARCH_STRICT` | x07 / run / error | sometimes | Strict architecture evidence failed certification. |
| `X07TC_EASYNC_PROOF` | x07 / run / error | never | Async proof coverage or async proof obligations failed certification. |
| `X07TC_EASYNC_PROVE_REPORT_MISSING` | x07 / run / error | sometimes | Required async prove report is missing. |
| `X07TC_EBOUNDARY_MISSING` | x07 / run / error | sometimes | Boundary declarations are missing or incomplete for certification. |
| `X07TC_EBOUNDARY_RELAXED` | x07 / run / error | never | The candidate relaxes a certified boundary contract relative to the baseline. |
| `X07TC_EBOUNDED_PROOF_FORBIDDEN` | x07 / run / error | never | Bounded recursion proof is forbidden for the active trust profile. |
| `X07TC_ECAPSULE_ATTEST` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_ECAPSULE_ATTEST`. |
| `X07TC_ECAPSULE_NETWORK_ATTEST` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_ECAPSULE_NETWORK_ATTEST`. |
| `X07TC_ECOMPILE_ATTEST` | x07 / run / error | never | Compile attestation failed or could not bind the emitted binary. |
| `X07TC_ECOVERAGE_ONLY` | x07 / run / error | never | Coverage/support evidence cannot satisfy certification. |
| `X07TC_EDEP_CLOSURE` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_EDEP_CLOSURE`. |
| `X07TC_EDEV_ONLY_ASSUMPTION` | x07 / run / error | never | Developer-only proof assumptions are forbidden for the active trust profile. |
| `X07TC_EDIFF_POSTURE` | x07 / run / error | never | Baseline review or trust-posture diff gate failed certification. |
| `X07TC_EEFFECT_LOG` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_EEFFECT_LOG`. |
| `X07TC_ENET_POLICY` | x07 / run / error | never | Network policy posture changed relative to the reviewed baseline. |
| `X07TC_ENONDET` | x07 / run / error | never | Trust report detected nondeterminism in the certified closure. |
| `X07TC_EOP_ENTRY_MISMATCH` | x07 / run / error | sometimes | Certification entry does not match the operational entry. |
| `X07TC_EOP_ENTRY_REQUIRED` | x07 / run / error | sometimes | Operational-entry metadata is required for the active trust profile. |
| `X07TC_EPBT` | x07 / run / error | sometimes | Boundary-required property tests are missing, malformed, or failing. |
| `X07TC_EPEER_POLICY` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_EPEER_POLICY`. |
| `X07TC_EPROFILE` | x07 / run / error | sometimes | Certification trust profile is missing or invalid. |
| `X07TC_EPROJECT` | x07 / run / error | sometimes | Certification could not resolve the project manifest or source closure. |
| `X07TC_EPROOF_CHECK_ENGINE_MISMATCH` | x07 / run / error | sometimes | Certification found a proof-check report whose engine did not match the proved symbol metadata. |
| `X07TC_EPROOF_CHECK_MISSING` | x07 / run / error | never | Required proof-check report is missing from prove evidence. |
| `X07TC_EPROOF_CHECK_REJECTED` | x07 / run / error | sometimes | Certification rejected a proof whose proof-check report was not accepted. |
| `X07TC_EPROOF_CHECK_SCHEMA_INVALID` | x07 / run / error | sometimes | Proof-check report failed schema validation during certification. |
| `X07TC_EPROOF_COVERAGE` | x07 / run / error | never | Reachable proof coverage is incomplete for certification. |
| `X07TC_EPROOF_OBJECT_MISSING` | x07 / run / error | never | Required proof object is missing from prove evidence. |
| `X07TC_EPROVE` | x07 / run / error | sometimes | At least one reachable proof obligation failed. |
| `X07TC_EPROVE_REPORT_MISSING` | x07 / run / error | sometimes | Required per-symbol prove report is missing. |
| `X07TC_EPROVE_UNSUPPORTED` | x07 / run / error | sometimes | A reachable symbol is outside the supported proof subset. |
| `X07TC_ERECURSION_FORBIDDEN` | x07 / run / error | never | Reachable recursion is forbidden by the active trust profile. |
| `X07TC_ERUNTIME_ATTEST` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_ERUNTIME_ATTEST`. |
| `X07TC_ERUNTIME_NETWORK_EVIDENCE` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_ERUNTIME_NETWORK_EVIDENCE`. |
| `X07TC_ESANDBOX_PROFILE` | x07 / run / error | never | Sandbox runtime evidence violates the certification profile. |
| `X07TC_ESCHEMA_DRIFT` | x07 / run / error | sometimes | Boundary-referenced schema outputs drifted or are missing. |
| `X07TC_ESURROGATE_ENTRY_FORBIDDEN` | x07 / run / error | sometimes | Surrogate certification entries are forbidden for the active trust profile. |
| `X07TC_ETESTS` | x07 / run / error | sometimes | Deterministic certification tests are missing or failing. |
| `X07TC_ETRUST_REPORT` | x07 / run / error | never | Trust report evidence failed certification. |
| `X07TC_EUNSUPPORTED_DEFASYNC` | x07 / run / error | sometimes | Reachable `defasync` logic is outside the certifiable subset. |
| `X07TC_EUNSUPPORTED_RECURSION` | x07 / run / error | sometimes | Reachable recursion is outside the certifiable subset. |
| `X07TEST_ASYNC_ENTRY_UNSUPPORTED` | x07 / lint / error | never | x07 test could not build or run the selected async entrypoint. |
| `X07TEST_CAPSULE_EVIDENCE_MISSING` | x07 / lint / error | never | A test that requires capsule evidence did not provide it. |
| `X07TEST_RUNTIME_ATTEST_REQUIRED` | x07 / run / error | sometimes | A test that requires runtime attestation did not produce it. |
| `X07TP_ARCH` | x07 / run / error | sometimes | Project architecture posture is weaker than the trust profile requires. |
| `X07TP_ASYNC_PROOF_REQUIRED` | x07 / run / error | sometimes | Sandboxed trusted-program profile is missing async proof coverage. |
| `X07TP_BACKEND_NOT_CERTIFIABLE` | x07 / run / error | sometimes | Networked trusted-program profile allows a backend posture that is not certifiable. |
| `X07TP_BOUNDARY` | x07 / run / error | sometimes | Project boundary index wiring is missing or incomplete for certification. |
| `X07TP_CAPSULES` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TP_CAPSULES`. |
| `X07TP_CAPSULE_ATTEST_REQUIRED` | x07 / run / error | sometimes | Sandboxed trusted-program profile is missing capsule attestation requirements. |
| `X07TP_DEP_CLOSURE_REQUIRED` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TP_DEP_CLOSURE_REQUIRED`. |
| `X07TP_EFFECT_LOG_REQUIRED` | x07 / run / error | sometimes | Trust profile is missing required effect-log evidence semantics. |
| `X07TP_ENTRY_FORBIDDEN` | x07 / run / error | sometimes | Requested entrypoint is not allowed by the trust profile. |
| `X07TP_INVALID` | x07 / run / error | sometimes | Trust profile JSON is missing or invalid. |
| `X07TP_LANGUAGE` | x07 / run / error | sometimes | Project uses language or policy features forbidden by the trust profile. |
| `X07TP_NETWORK_MODE_FORBIDDEN` | x07 / run / error | sometimes | Sandboxed-local trust profile allows networking where it should not. |
| `X07TP_NETWORK_PROFILE_REQUIRED` | x07 / run / error | sometimes | Networked trust profile is missing required network certification posture. |
| `X07TP_NOT_CERTIFIABLE` | x07 / run / error | sometimes | Trust profile is weaker than the current certification floor. |
| `X07TP_PEER_POLICY_REQUIRED` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TP_PEER_POLICY_REQUIRED`. |
| `X07TP_PROJECT_MISMATCH` | x07 / run / error | sometimes | Project manifest could not be resolved for trust profile validation. |
| `X07TP_RUNTIME_ATTEST_REQUIRED` | x07 / run / error | sometimes | Sandboxed trusted-program profile is missing runtime attestation requirements. |
| `X07TP_SANDBOX_BACKEND_REQUIRED` | x07 / run / error | sometimes | Sandboxed trusted-program profile is missing the VM-only isolation requirement. |
| `X07TP_WORLD` | x07 / run / error | sometimes | Project world is outside the trust profile allowlist. |
| `X07T_EPBT_MANIFEST_INVALID` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07T_EPBT_MANIFEST_INVALID`. |
| `X07T_EPBT_PARAM_EMPTY` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07T_EPBT_PARAM_EMPTY`. |
| `X07T_EPBT_UNKNOWN_GEN_KIND` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07T_EPBT_UNKNOWN_GEN_KIND`. |
| `X07T_EPBT_UNSUPPORTED_WORLD` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07T_EPBT_UNSUPPORTED_WORLD`. |
| `X07V_ASYNC_COUNTEREXAMPLE` | x07 / lint / error | never | The async proof found a counterexample. |
| `X07V_CANCELLATION_ENSURE_FAILED` | x07 / lint / error | never | The async cancellation postcondition failed under proof. |
| `X07V_COVERAGE_NOT_PROOF` | x07 / lint / warning | sometimes | Coverage/support summary is not proof evidence. |
| `X07V_COVERAGE_SUMMARY_FORBIDDEN` | x07 / lint / error | sometimes | Coverage/support summaries cannot be imported as proof. |
| `X07V_EARGS` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_EARGS`. |
| `X07V_ECBMC_ERROR` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_ECBMC_ERROR`. |
| `X07V_ECBMC_FAILURE` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_ECBMC_FAILURE`. |
| `X07V_ECBMC_JSON_PARSE` | x07 / parse / error | sometimes | Core lint/schema diagnostic `X07V_ECBMC_JSON_PARSE`. |
| `X07V_ECBMC_MISSING` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_ECBMC_MISSING`. |
| `X07V_ECBMC_SMT2` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_ECBMC_SMT2`. |
| `X07V_ECBMC_STDERR` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_ECBMC_STDERR`. |
| `X07V_EMODULE_ROOTS` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_EMODULE_ROOTS`. |
| `X07V_EPROJECT` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_EPROJECT`. |
| `X07V_ETARGET` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_ETARGET`. |
| `X07V_EZ3_MISSING` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_EZ3_MISSING`. |
| `X07V_EZ3_RUN` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07V_EZ3_RUN`. |
| `X07V_IMPORTED_STUB_FORBIDDEN` | x07 / lint / error | sometimes | Imported stub assumptions are disabled in prove mode. |
| `X07V_INTERNAL` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_INTERNAL`. |
| `X07V_NO_CONTRACTS` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_NO_CONTRACTS`. |
| `X07V_PROOF_SUMMARY_REQUIRED` | x07 / lint / error | sometimes | A reachable proof summary dependency is required but missing. |
| `X07V_PROVE_UNSUPPORTED` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_PROVE_UNSUPPORTED`. |
| `X07V_RECURSION_TERMINATION_FAILED` | x07 / lint / error | sometimes | Recursive proof could not justify the declared termination rank. |
| `X07V_RECURSIVE_DECREASES_REQUIRED` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_RECURSIVE_DECREASES_REQUIRED`. |
| `X07V_SCHEDULER_MODEL_UNTRUSTED` | x07 / lint / error | sometimes | Async proof cannot proceed without the trusted scheduler model. |
| `X07V_SCOPE_INVARIANT_FAILED` | x07 / lint / error | never | An async scope invariant failed under proof. |
| `X07V_SMT_SAT` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_SMT_SAT`. |
| `X07V_SMT_TIMEOUT` | x07 / run / error | sometimes | SMT solver returned unknown due to a timeout. |
| `X07V_SMT_UNKNOWN` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_SMT_UNKNOWN`. |
| `X07V_SUMMARY_MISMATCH` | x07 / lint / error | never | An imported proof summary does not match the current declaration. |
| `X07V_SUMMARY_MISSING` | x07 / lint / error | sometimes | A reachable imported proof summary dependency is missing. |
| `X07V_UNSUPPORTED_DEFASYNC_FORM` | x07 / lint / error | sometimes | The selected `defasync` target is outside the supported proof subset. |
| `X07V_UNSUPPORTED_FOR_BOUNDS` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_UNSUPPORTED_FOR_BOUNDS`. |
| `X07V_UNSUPPORTED_HEAP_EFFECT` | x07 / lint / error | sometimes | The proved core uses heap or pointer effects outside the supported subset. |
| `X07V_UNSUPPORTED_MUTUAL_RECURSION` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_UNSUPPORTED_MUTUAL_RECURSION`. |
| `X07V_UNSUPPORTED_RECURSION` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_UNSUPPORTED_RECURSION`. |
| `X07V_UNSUPPORTED_RICH_TYPE` | x07 / type / error | sometimes | The target signature is outside the supported richer-data proof subset. |
| `X07V_UNWIND_INCOMPLETE` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_UNWIND_INCOMPLETE`. |
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


## `ETEST_SOLVE_FUEL_INVALID`

Summary: Test manifest validation diagnostic `ETEST_SOLVE_FUEL_INVALID`.

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
- If the manifest is intentionally empty, pass `--allow-empty`.


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


## `EXTAL_CERTIFY_ENTRY_REQUIRED`

Summary: XTAL certification requires an explicit entry selection.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The selected entry is missing or invalid.

Messages:
- `invalid --entry (expected a fully-qualified symbol)`
- `multiple entrypoints configured; pass --entry or --all`

Agent strategy:

- If the manifest has multiple entrypoints, pass `--entry <SYM>` or `--all`.
- Ensure `--entry` is a fully-qualified symbol (for example `app.main`).


## `EXTAL_CERTIFY_MANIFEST_MISSING`

Summary: XTAL certification manifest is required.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

x07 xtal certify requires `arch/xtal/xtal.json` to select entrypoints and trust settings.

Message: `arch/xtal/xtal.json is required for certification`

Agent strategy:

- Add `arch/xtal/xtal.json` (schema: `x07.xtal.manifest@0.1.0`).
- Re-run `x07 xtal certify`.


## `EXTAL_CERTIFY_MANIFEST_PARSE_FAILED`

Summary: XTAL certification manifest is invalid.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Failed to parse `arch/xtal/xtal.json` or a required referenced file.

Messages:
- `failed to parse arch/xtal/xtal.json: {error}`
- `failed to parse arch/xtal/xtal.json: missing trust section`
- `failed to parse arch/xtal/xtal.json: trust.cert_profile does not exist`

Agent strategy:

- Validate `arch/xtal/xtal.json` against `docs/spec/schemas/x07.xtal.manifest@0.1.0.schema.json`.
- Ensure referenced files exist (for example `trust.cert_profile`).
- Re-run `x07 xtal certify`.


## `EXTAL_CERTIFY_NO_ENTRYPOINTS`

Summary: XTAL certification has no entrypoints.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The XTAL manifest has an empty `entrypoints[]` list.

Message: `no entrypoints configured in arch/xtal/xtal.json`

Agent strategy:

- Add at least one entry to `entrypoints[]` in `arch/xtal/xtal.json`.
- Re-run `x07 xtal certify` (or pass `--entry` / `--all` after updating the manifest).


## `EXTAL_CERTIFY_PRECHECKS_FAILED`

Summary: XTAL certification prechecks failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires resolving xtal precheck failures (x07 xtal dev).

Details:

Certification runs `x07 xtal dev` prechecks by default to enforce spec/generator/impl hygiene.

Message: `xtal prechecks failed; see diagnostics`

Agent strategy:

- Run `x07 xtal dev --project x07.json` and fix any reported diagnostics.
- Re-run `x07 xtal certify` (or pass `--no-prechecks` if you must skip prechecks).


## `EXTAL_CERTIFY_TRUST_CERTIFY_FAILED`

Summary: Trust certification failed for a certification entry.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The underlying `x07 trust certify` invocation failed for a selected entrypoint.

Message: `trust certification failed for entry '{entry}'; see {out_dir}`

Agent strategy:

- Inspect the per-entry output directory under `target/xtal/cert/<entry>/`.
- Re-run the failing command directly (`x07 trust certify ...`) to reproduce with full logs.
- Fix the reported issue (proof/test/trust gate) and re-run `x07 xtal certify`.


## `EXTAL_DEV_NO_SPECS`

Summary: XTAL dev pipeline found no spec modules.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

XTAL dev pipeline found no spec modules.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Reproduce the diagnostic and apply the recommended fix.


## `EXTAL_EVENTS_INVALID`

Summary: Recovery events log is invalid.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The recovery events input must be UTF-8 JSONL, where each non-empty line is a schema-valid `x07.xtal.recovery_event@0.1.0` document.

Agent strategy:

- Validate the file is UTF-8 and one JSON object per line.
- Validate each line against `x07.xtal.recovery_event@0.1.0`.
- Ensure the log can be associated with an incident id (directory name or `related_violation_id`).


## `EXTAL_EVENTS_IO`

Summary: Recovery events log I/O failed.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

x07 failed to read or digest the provided `events.jsonl` recovery events log.

Agent strategy:

- Confirm the `events.jsonl` path exists and is readable.
- Re-run the command after fixing filesystem permissions or the path.


## `EXTAL_EVENTS_UNSUPPORTED_VERSION`

Summary: Recovery event schema_version is unsupported.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

A line in `events.jsonl` has an unexpected `schema_version` value.

Agent strategy:

- Ensure each JSONL line is `x07.xtal.recovery_event@0.1.0`.
- If the log was produced by an older toolchain, re-emit events with the current toolchain.


## `EXTAL_EXAMPLES_ARGS_EXTRA`

Summary: Example provides an extra arg.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Example provides an extra arg.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_EXAMPLES_ARGS_MISSING`

Summary: Example is missing a required arg.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Example is missing a required arg.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_EXAMPLES_ARG_KIND_UNSUPPORTED`

Summary: Example arg encoding kind is unsupported in the current XTAL subset.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Example arg encoding kind is unsupported in the current XTAL subset.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_EXAMPLES_B64_INVALID`

Summary: Example contains invalid base64 payload.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Example contains invalid base64 payload.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_EXAMPLES_EXPECT_KIND_UNSUPPORTED`

Summary: Example expect encoding kind is unsupported.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Example expect encoding kind is unsupported.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_EXAMPLES_IO_READ_FAILED`

Summary: Cannot read examples JSONL file.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Cannot read examples JSONL file.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_EXAMPLES_JSON_PARSE`

Summary: Examples JSONL line contains invalid JSON.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Examples JSONL line contains invalid JSON.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_EXAMPLES_OP_UNKNOWN`

Summary: Example references an unknown op id.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Example references an unknown op id.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_EXAMPLES_SCHEMA_INVALID`

Summary: Examples JSONL line violates the structural schema.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Examples JSONL line violates the structural schema.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_EXAMPLES_SCHEMA_VERSION_UNSUPPORTED`

Summary: Examples schema_version is unsupported.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Examples schema_version is unsupported.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix the referenced `*.x07spec.examples.jsonl` line(s).
- Re-run `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.


## `EXTAL_GEN_DRIFT`

Summary: Generated outputs drifted (check mode).

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Generated outputs drifted (check mode).

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Re-run generation with `x07 xtal tests gen-from-spec --project x07.json --write`.
- Commit updated `gen/xtal/**` outputs and re-run `--check`.


## `EXTAL_GEN_INDEX_MISSING`

Summary: Generator index file is missing.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

XTAL wrapper commands were configured to use a generator index file, but the path does not exist.

Agent strategy:

- Create a generator index at `arch/gen/index.x07gen.json` (recommended), or pass the correct `--gen-index` path.
- Re-run `x07 xtal dev` / `x07 xtal verify`.


## `EXTAL_GEN_NO_EXAMPLES`

Summary: Spec modules contain no examples to generate tests from.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Spec modules contain no examples to generate tests from.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Re-run generation with `x07 xtal tests gen-from-spec --project x07.json --write`.
- Commit updated `gen/xtal/**` outputs and re-run `--check`.


## `EXTAL_GEN_NO_SPECS`

Summary: No spec modules found for generation.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

No spec modules found for generation.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Re-run generation with `x07 xtal tests gen-from-spec --project x07.json --write`.
- Commit updated `gen/xtal/**` outputs and re-run `--check`.


## `EXTAL_GEN_UNSUPPORTED_TY`

Summary: Generator cannot embed a value for the requested type/kind.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Generator cannot embed a value for the requested type/kind.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Re-run generation with `x07 xtal tests gen-from-spec --project x07.json --write`.
- Commit updated `gen/xtal/**` outputs and re-run `--check`.


## `EXTAL_IMPL_CONTRACT_EXTRA`

Summary: Implementation has extra contract clauses beyond the spec.

Origins:
- x07 (stage: lint, severity: warning)

Quickfix support: `sometimes`

Details:

Extra `ensures`/`invariant` clauses are allowed but should be intentional and documented.

Agent strategy:

- If the extra clauses are unintended, remove them or sync from spec.
- Otherwise, keep them and ensure they remain spec-compatible.


## `EXTAL_IMPL_CONTRACT_EXTRA_REQUIRES`

Summary: Implementation has extra requires clauses not present in the spec.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Extra preconditions strengthen the contract beyond the spec and are rejected by implementation conformance checks.

Agent strategy:

- Remove the extra `requires` clause(s), or add the intended clauses to the spec (with review).
- Re-run `x07 xtal impl check`.


## `EXTAL_IMPL_CONTRACT_MISMATCH`

Summary: Implementation contract clause does not match the spec clause.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

When a contract-core clause is matched by id, the implementation expression must match the spec expression.

Agent strategy:

- Run `x07 xtal impl sync --project x07.json --write` to rewrite mismatched contract-core clauses.
- Re-run `x07 xtal impl check`.


## `EXTAL_IMPL_CONTRACT_MISSING`

Summary: Implementation is missing a contract-core clause from the spec.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Contract-core spec clauses (requires/ensures/invariant that pass contracts-only typecheck) must be present in the implementation contracts.

Agent strategy:

- Run `x07 xtal impl sync --project x07.json --write` to insert missing contract-core clauses.
- Re-run `x07 xtal impl check`.


## `EXTAL_IMPL_EXPORT_MISSING`

Summary: Operation is missing from the implementation export list.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Each spec operation must be exported from the corresponding implementation module.

Agent strategy:

- Run `x07 xtal impl sync --project x07.json --write` to add missing exports.
- Re-run `x07 xtal impl check`.


## `EXTAL_IMPL_IO_READ_FAILED`

Summary: Failed to read an implementation module file.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The implementation module exists but could not be read from disk.

Agent strategy:

- Check file permissions and path correctness.
- Re-run `x07 xtal impl check --project x07.json`.


## `EXTAL_IMPL_KIND_UNSUPPORTED`

Summary: Implementation file kind is unsupported for module conformance.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

XTAL implementation conformance expects x07AST kind `module` for implementation files under `src/`.

Agent strategy:

- Ensure the implementation file is an x07AST module (`kind: "module"`).
- Re-run `x07 xtal impl check`.


## `EXTAL_IMPL_MODULE_ID_MISMATCH`

Summary: Implementation module_id does not match the spec module_id.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The spec module_id and the implementation module_id must match for reliable conformance checking and syncing.

Agent strategy:

- Fix `module_id` in the implementation module to match the spec.
- Re-run `x07 xtal impl check`.


## `EXTAL_IMPL_MODULE_MISSING`

Summary: Implementation module is missing for a spec module.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

For each `spec/<module_id>.x07spec.json`, XTAL expects an implementation module at `src/<module_id with '\.' replaced by '/'>.x07.json`.

Agent strategy:

- Run `x07 xtal impl sync --project x07.json --write` to create missing modules and stubs.
- Commit the new `src/**` file(s), then re-run `x07 xtal impl check`.


## `EXTAL_IMPL_NO_SPECS`

Summary: No spec files found for implementation conformance checks.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The implementation conformance command could not find any `*.x07spec.json` modules under the configured spec directory.

Agent strategy:

- Ensure your project has spec modules under `spec/` (recommended).
- Or pass the correct `--spec-dir`.
- Re-run `x07 xtal impl check --project x07.json`.


## `EXTAL_IMPL_PROP_DEFN_MISSING`

Summary: Property function definition is missing.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

A property function symbol is exported but no matching `defn` was found in the implementation module.

Message: `Property "{prop}" referenced by op "{op_id}" is exported by "{impl_path}" but no matching defn was found.`

Agent strategy:

- Run `x07 xtal impl sync --patchset-out <PATH>` (or `--write`) to scaffold a failing stub `defn`.
- Implement the property function body and re-run `x07 xtal impl check`.


## `EXTAL_IMPL_PROP_EXPORT_MISSING`

Summary: Property function is not exported.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

A property function exists in the implementation module but is not listed in `exports`, so generated property tests cannot call it.

Message: `Property "{prop}" referenced by op "{op_id}" exists but is not exported by "{impl_path}".`

Agent strategy:

- Run `x07 xtal impl sync --patchset-out <PATH>` (or `--write`) to update exports and scaffold missing stubs.
- Re-run `x07 xtal impl check` and then `x07 xtal verify`.


## `EXTAL_IMPL_PROP_MODULE_MISSING`

Summary: Property function module is missing.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

A spec operation references a property function via `ensures_props[*].prop`, but the implementation module for that symbol was not found.

Message: `Property "{prop}" referenced by op "{op_id}" was not found: module "{module_id}" does not exist at "{impl_path}".`

Agent strategy:

- Run `x07 xtal impl sync --patchset-out <PATH>` (or `--write`) to scaffold missing modules and property stubs.
- Implement the property function body and re-run `x07 xtal impl check`.


## `EXTAL_IMPL_PROP_RESULT_TYPE_MISMATCH`

Summary: Property function result type is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Property functions referenced by `ensures_props` must return a bytes status value.

Message: `Property "{prop}" referenced by op "{op_id}" must return bytes_status_v1 but returns {reason} in "{impl_path}".`

Agent strategy:

- Update the property function result type to `bytes` (status) and re-run `x07 xtal impl check`.
- Use `x07 xtal impl sync --patchset-out <PATH>` to sync stubs/signatures when appropriate.


## `EXTAL_IMPL_PROP_SIGNATURE_MISMATCH`

Summary: Property function signature does not match the spec.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The referenced property function exists, but its parameter list (count/types/brands) does not match what the spec operation requires.

Message: `Property "{prop}" referenced by op "{op_id}" has a signature mismatch in "{impl_path}". Expected: {reason}.`

Agent strategy:

- Run `x07 xtal impl sync --patchset-out <PATH>` (or `--write`) to sync the property signature to the spec-derived expectation.
- If the spec is wrong, update the spec and re-run the generation + verify lane.


## `EXTAL_IMPL_SIGNATURE_MISMATCH`

Summary: Implementation signature does not match the spec.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Implementation param counts/types/brands and result type/brand must match the spec operation signature.

Agent strategy:

- Edit the implementation `defn` signature (or the spec) so param and result types/brands match.
- Re-run `x07 xtal impl check`.


## `EXTAL_IMPL_SYNC_REQUIRED`

Summary: Implementation sync is required.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The implementation does not match the spec-derived contract core. `x07 xtal impl sync` would change one or more files.

Agent strategy:

- Run `x07 xtal impl sync --patchset-out <PATH>` to generate a reviewable patchset (or `--write` to apply directly).
- Re-run `x07 xtal impl check` and `x07 xtal verify`.


## `EXTAL_IMPL_UNSUPPORTED_TY`

Summary: Unsupported type for generated implementation stubs.

Origins:
- x07 (stage: lower, severity: error)

Quickfix support: `sometimes`

Details:

XTAL implementation syncing can create stubs for a limited type subset. A stub body could not be generated for this type.

Agent strategy:

- Implement the function body manually (or adjust the spec signature into the supported subset).
- Re-run `x07 xtal impl sync --project x07.json --write` if needed.


## `EXTAL_IMPL_X07AST_PARSE`

Summary: Implementation module is not valid x07AST JSON.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The implementation module could not be parsed as an x07AST JSON document.

Agent strategy:

- Run `x07 fmt --write --path <module.x07.json>` to canonicalize and validate shape.
- If the file is not meant to be x07AST JSON, replace it with a valid `*.x07.json` module.
- Re-run `x07 xtal impl check`.


## `EXTAL_IMPROVE_DISALLOWED_PATCH_PATHS`

Summary: XTAL improve patchset touches disallowed paths.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proposed patchset includes paths outside the allowed write set declared in `arch/xtal/xtal.json` (`autonomy.agent_write_paths[]`, `agent_write_specs`, and `agent_write_arch`).

Agent strategy:

- Review and tighten the patchset (or expand the allowed write set intentionally).
- Re-run `x07 xtal improve`.


## `EXTAL_IMPROVE_FAILED`

Summary: XTAL improve failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

XTAL improve failed to construct a valid improvement run.

Message: `{details}`

Agent strategy:

- Inspect `target/xtal/xtal.improve.diag.json` and `target/xtal/improve/summary.json`.
- Fix the reported issue (missing inputs, invalid schemas, or unsupported configuration) and retry.


## `EXTAL_IMPROVE_MANIFEST_PARSE_FAILED`

Summary: XTAL improve failed to parse arch/xtal/xtal.json.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The XTAL manifest is missing required fields or is not schema-valid.

Agent strategy:

- Validate `arch/xtal/xtal.json` against `x07.xtal.manifest@0.1.0`.
- Fix schema errors and re-run `x07 xtal improve`.


## `EXTAL_IMPROVE_NO_INCIDENTS`

Summary: XTAL improve found no ingestable incidents at the provided input path.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The `x07 xtal improve` command requires an incident input (bundle/repro/events) or a directory containing incident bundles.

Agent strategy:

- Provide `--input` as a violation bundle dir, `violation.json`, `repro.json`, `events.jsonl`, or a directory containing incident bundle subdirectories.
- Re-run `x07 xtal improve --input <path>`.


## `EXTAL_IMPROVE_NO_PATCH`

Summary: XTAL improve produced no patchset.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The bounded repair step completed without producing a patchset.

Agent strategy:

- Inspect `target/xtal/repair/summary.json` and `target/xtal/xtal.repair.diag.json`.
- Retry with a smaller incident input or add a targeted unit test to improve localization.


## `EXTAL_IMPROVE_PATCH_APPLY_FAILED`

Summary: XTAL improve failed to apply a patchset.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

x07 failed while applying the patchset to the working tree.

Agent strategy:

- Inspect the patchset at `target/xtal/repair/patchset.json`.
- Apply it manually with `x07 patch apply --in target/xtal/repair/patchset.json --write` to see the first failure.
- Resolve conflicts or invalid patches and retry.


## `EXTAL_IMPROVE_SPEC_CHANGE_REQUIRES_FLAG`

Summary: XTAL improve blocked a spec change.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proposed patchset modifies `spec/**`, which requires explicit opt-in.

Agent strategy:

- Review the patchset under `target/xtal/repair/patchset.json`.
- If the spec change is intended, re-run with `--allow-spec-change`.


## `EXTAL_IMPROVE_TASKS_FAILED`

Summary: XTAL improve: recovery tasks failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

`x07 xtal improve --run-tasks` invoked the recovery task executor and it returned a failing status.

Inspect `target/xtal/xtal.tasks.diag.json` and the recovery events log under `target/xtal/events/<incident_id>/events.jsonl` for details.

Agent strategy:

- Inspect `target/xtal/xtal.tasks.diag.json` for the task failure diagnostics.
- Inspect `target/xtal/events/<incident_id>/events.jsonl` to confirm which tasks ran, retried, skipped, or failed.
- Fix the underlying task implementation or policy and re-run `x07 xtal improve --run-tasks`.


## `EXTAL_IMPROVE_WRITE_REQUIRES_MANIFEST`

Summary: XTAL improve --write requires an XTAL manifest.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

`x07 xtal improve --write` requires `arch/xtal/xtal.json` so edit boundaries are explicit.

Agent strategy:

- Add `arch/xtal/xtal.json` to the project (XTAL manifest).
- Re-run `x07 xtal improve --write ...`.


## `EXTAL_INGEST_FAILED`

Summary: XTAL ingest failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The `x07 xtal ingest` command failed to normalize the provided input into a canonical ingest workspace.

Message: `xtal ingest failed: {details}`

Agent strategy:

- Confirm `--input` points to one of:
  - a directory containing `violation.json` and `repro.json`,
  - `violation.json` (`x07.xtal.violation@0.1.0`), or
  - `repro.json` (`x07.contract.repro@0.1.0`).
- If `violation.json` references a repro via `repro.path`, ensure that referenced file exists.
- Re-run `x07 xtal ingest --input <path>` and inspect `target/xtal/xtal.ingest.diag.json`.


## `EXTAL_INGEST_INPUT_SCHEMA_INVALID`

Summary: XTAL ingest input is invalid.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The ingest input could not be read, parsed as JSON, or validated against its schema.

Agent strategy:

- Confirm the file exists and is readable.
- Confirm the file is valid JSON and matches the expected schema.
- Re-run `x07 xtal ingest --input <path>` and inspect `target/xtal/xtal.ingest.diag.json`.


## `EXTAL_INGEST_INPUT_SCHEMA_VERSION_UNSUPPORTED`

Summary: XTAL ingest input schema_version is unsupported.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The input file has an unexpected `schema_version`. XTAL ingest accepts:

- `x07.xtal.violation@0.1.0` (`violation.json`)
- `x07.contract.repro@0.1.0` (`repro.json`)
- `events.jsonl` where each line is `x07.xtal.recovery_event@0.1.0`

Agent strategy:

- Confirm `--input` points to a supported XTAL ingest input.
- If ingesting `events.jsonl`, ensure each line is schema-valid `x07.xtal.recovery_event@0.1.0`.
- Recreate the bundle using a current x07 toolchain and retry.


## `EXTAL_INGEST_INTEGRITY_MISMATCH`

Summary: XTAL ingest detected a mismatch between violation.json and repro.json.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

XTAL ingest validates that the violation bundle is internally consistent (content-addressed id, repro digest, and repro length).

Message: `{details}`

Agent strategy:

- Treat violation bundles as immutable.
- Recreate the bundle from the original `repro.json` (or re-export it from the source environment).
- Retry `x07 xtal ingest --input <bundle_dir>`.


## `EXTAL_INGEST_REPRO_NOT_FOUND`

Summary: XTAL ingest could not find the repro referenced by the violation bundle.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The `violation.json` file referenced a `repro.path` that does not exist on disk.

Agent strategy:

- Ensure the bundle directory contains the referenced repro file (usually `repro.json`).
- If the bundle is incomplete, re-upload or re-export it from the source environment.


## `EXTAL_INGEST_REPRO_PATH_UNSAFE`

Summary: XTAL ingest refused an unsafe repro path in the violation bundle.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The violation bundle referenced a repro path that is not a safe relative path (for example it escapes the bundle directory).

Agent strategy:

- Do not hand-edit `violation.json`.
- Recreate the bundle from the original `repro.json` using a current toolchain.


## `EXTAL_INGEST_REPRO_SCHEMA_INVALID`

Summary: XTAL ingest repro.json is not schema-valid.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The referenced repro file is not a valid `x07.contract.repro@0.1.0` document.

Agent strategy:

- Recreate `repro.json` using the toolchain that produced the contract trap (from `x07 test` / `x07 run`).
- Avoid editing repro artifacts by hand; they are content-addressed inputs.


## `EXTAL_REPAIR_APPLY_FAILED`

Summary: XTAL repair patch application failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Patch application may require a clean tree or manual conflict resolution.

Details:

In `--write` mode, XTAL repair applies the suggested patchset via `x07 patch apply`.

Message: `patch application failed (exit_code={code}): {stderr}`

Agent strategy:

- Inspect `target/xtal/repair/patchset.json` and apply it manually with `x07 patch apply`.
- Re-run `x07 xtal verify`.


## `EXTAL_REPAIR_BASELINE_MISSING`

Summary: XTAL repair baseline is missing.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `never`
No quickfix reason: Requires baseline artifacts from x07 xtal verify (target/xtal/verify/summary.json).

Details:

XTAL repair consumes the baseline artifacts produced by `x07 xtal verify`.

Message: `baseline verify summary is missing (run `x07 xtal verify`): {path}: {err}`

Agent strategy:

- Run `x07 xtal verify` to generate `target/xtal/verify/summary.json` and `target/xtal/xtal.verify.diag.json`.
- Re-run `x07 xtal repair`.


## `EXTAL_REPAIR_MANIFEST_PARSE_FAILED`

Summary: XTAL repair manifest parse failed.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

XTAL repair loads `arch/xtal/xtal.json` to enforce autonomy boundaries when applying or evaluating patches.

Message: `failed to parse arch/xtal/xtal.json: {error}`

Agent strategy:

- Validate `arch/xtal/xtal.json` against `docs/spec/schemas/x07.xtal.manifest@0.1.0.schema.json`.
- Fix JSON/schema issues and re-run `x07 xtal repair`.


## `EXTAL_REPAIR_NO_ACTIONABLE_FAILURE`

Summary: XTAL repair could not find an actionable target.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires additional evidence (prove counterexample / failing tests) or a selected --entry to target.

Details:

XTAL repair requires an eligible failing entry (for example a proof counterexample) to drive semantic repair, or an entry to target for quickfix fallback.

Message: `baseline verify failed, but no eligible entries were found for repair`

Agent strategy:

- Re-run `x07 xtal verify` and ensure proof tooling is installed and enabled.
- If the baseline has no proof counterexample entries, run `x07 xtal repair --quickfix-only` or pass `--entry <symbol>` to target one operation.
- Inspect `target/xtal/verify/summary.json` for the available entries and failure modes.


## `EXTAL_REPAIR_NO_PATCH_FOUND`

Summary: XTAL repair exhausted its budget without finding a patch.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Bounded repair exhausted; may require new examples, larger budgets, or a manual fix.

Details:

XTAL repair is bounded and may fail to find a patch that satisfies verification + tests within the configured budget.

Message: `no patch found for {entry} after {n} attempt(s)`

Agent strategy:

- Inspect `target/xtal/repair/attempts/` for attempt artifacts and the failing entry prove report.
- Increase budgets (`--max-rounds`, `--max-candidates`) or allow editing non-stubs if appropriate.
- Apply a manual fix and re-run `x07 xtal verify`.


## `EXTAL_REPAIR_PATCHSET_WRITE_FAILED`

Summary: XTAL repair could not write patch artifacts.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Filesystem I/O failure requires environment or permissions fixes.

Details:

XTAL repair writes patch and diff outputs under `target/xtal/repair/`.

Message: `cannot write patchset/diff: {path}: {err}`

Agent strategy:

- Check filesystem permissions and available disk space for `target/`.
- Re-run `x07 xtal repair`.


## `EXTAL_REPAIR_PATCH_OUTSIDE_ALLOWED_PATHS`

Summary: XTAL repair patch violates autonomy boundaries.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires changing autonomy boundaries (agent_write_paths) or adjusting the patchset contents.

Details:

The selected patchset touches files outside `autonomy.agent_write_paths[]` from `arch/xtal/xtal.json`.

Message: `repair patch touches files outside agent_write_paths: {paths}`

Agent strategy:

- Restrict the patch to allowed paths, or widen `autonomy.agent_write_paths[]` (requires human review).
- Re-run `x07 xtal repair` to generate a new patchset.


## `EXTAL_REPAIR_SEMANTIC_NO_EXAMPLES`

Summary: XTAL semantic repair requires examples.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Semantic repair uses spec examples as a semantic oracle.

Message: `semantic repair requires examples for entry '{entry}', but none were found in spec`

Agent strategy:

- Add `examples_ref` to the target operation and provide `*.x07spec.examples.jsonl` cases.
- Re-run `x07 xtal verify` to regenerate `gen/xtal/` tests.
- Re-run `x07 xtal repair`.


## `EXTAL_REPAIR_SEMANTIC_SEARCH_EXHAUSTED`

Summary: XTAL semantic repair exhausted its search budget.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The enumerative search did not find a candidate expression that passes examples and validation within the configured bounds.

Message: `semantic repair exhausted its budget (max_candidates={n}, max_depth={d}) without finding a valid patch for '{entry}'`

Agent strategy:

- Increase `--max-candidates` and/or `--semantic-max-depth`.
- Add more examples to constrain the search.
- If needed, fall back to `x07 xtal repair --quickfix-only` or implement the fix manually.


## `EXTAL_REPAIR_SEMANTIC_UNSUPPORTED_RETURN_TYPE`

Summary: XTAL semantic repair does not support this return type.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Semantic repair does not support this return type in the current subset.

Details:

The semantic repair engine only supports a limited return-type subset.

Message: `semantic repair does not support return type '{ty}' for entry '{entry}'`

Agent strategy:

- Use `x07 xtal repair --quickfix-only` for diagnostic quickfix repair.
- Or implement the missing semantic repair support for the return type.


## `EXTAL_REPAIR_TARGET_NOT_ELIGIBLE`

Summary: XTAL repair refused to edit a non-stub implementation.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires explicit opt-in to edit non-stub bodies (--allow-edit-non-stubs).

Details:

XTAL repair defaults to editing only known stub bodies to avoid surprising edits to existing implementations.

Message: `stubs-only repair refused to edit non-stub body for {entry} (pass --allow-edit-non-stubs to override)`

Agent strategy:

- Re-run `x07 xtal repair --allow-edit-non-stubs` if the target is expected to be edited.
- If the implementation should still be a stub, regenerate it with `x07 xtal impl sync --write`.


## `EXTAL_REPAIR_VERIFY_FAILED`

Summary: XTAL repair patch did not pass verification after applying it.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Post-apply verification failure requires further repair iterations or manual changes.

Details:

In `--write` mode, XTAL repair validates the applied patch by rerunning `x07 xtal verify`.

Message: `post-apply verify failed (exit_code={code}): {stderr}`

Agent strategy:

- Inspect the post-apply `target/xtal/verify/` reports for the remaining failures.
- Re-run `x07 xtal repair` with a larger budget or apply a manual fix.


## `EXTAL_REPAIR_WRITE_REQUIRES_MANIFEST`

Summary: XTAL repair --write requires an explicit autonomy manifest.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `never`
No quickfix reason: Requires arch/xtal/xtal.json to define explicit autonomy boundaries for --write.

Details:

XTAL repair requires `arch/xtal/xtal.json` when `--write` is used so edit boundaries are explicit.

Message: `--write requires arch/xtal/xtal.json so edit boundaries are explicit`

Agent strategy:

- Add `arch/xtal/xtal.json` with `autonomy.agent_write_paths[]`.
- Re-run `x07 xtal repair --write`.


## `EXTAL_SPEC_CONTRACT_BUILTIN_DISALLOWED`

Summary: Contract clause uses a disallowed builtin/head.

Origins:
- x07 (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

Contract clause uses a disallowed builtin/head.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_CONTRACT_EXPR_NOT_I32`

Summary: Contract clause does not typecheck to i32.

Origins:
- x07 (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

Contract clause does not typecheck to i32.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_CONTRACT_EXPR_PARSE`

Summary: Contract clause expression is not valid XTAL JSON expr.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Contract clause expression is not valid XTAL JSON expr.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_CONTRACT_MODULE_CALL_FORBIDDEN`

Summary: Contract clause contains a forbidden module call.

Origins:
- x07 (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

Contract clause contains a forbidden module call.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_CONTRACT_USES_RESULT_OUTSIDE_ENSURES`

Summary: `__result` used outside ensures.

Origins:
- x07 (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

`__result` used outside ensures.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_CONTRACT_WITNESS_INVALID`

Summary: Contract witness is invalid or not contract-pure.

Origins:
- x07 (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

Contract witness is invalid or not contract-pure.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_EXAMPLES_REF_MISSING`

Summary: Spec examples_ref points to a missing file.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec examples_ref points to a missing file.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_EXTRACT_DIFFERS`

Summary: Extracted spec differs from the existing spec.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The extracted best-effort spec does not match the current spec file, and no output mode was selected.

Agent strategy:

- Re-run `x07 xtal spec extract --write` to update the spec file, or `--patchset-out <PATH>` to produce a reviewable patch.
- Review changes and re-run `x07 xtal spec check`.


## `EXTAL_SPEC_EXTRACT_IMPL_MODULE_MISSING`

Summary: Implementation module is missing for spec extraction.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Spec extraction could not find the requested implementation module.

Agent strategy:

- Confirm `--module-id` / `--impl-path` points at a valid `*.x07.json` module.
- If the module should exist, generate or restore it, then re-run `x07 xtal spec extract`.


## `EXTAL_SPEC_EXTRACT_NO_EXPORTS`

Summary: No eligible exports found for spec extraction.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec extraction only considers exported `defn` items with supported signatures.

Agent strategy:

- Export one or more supported `defn` functions from the implementation module.
- Re-run `x07 xtal spec extract` and write the spec with `--write` or emit a patchset with `--patchset-out`.


## `EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE`

Summary: Exported function signature is unsupported for spec extraction.

Origins:
- x07 (stage: lint, severity: warning)

Quickfix support: `sometimes`

Details:

Spec extraction only includes exported `defn` items with supported signature types (currently `bytes`, `bytes_view`, `i32`) and without generics.

Agent strategy:

- Adjust the exported surface to use supported parameter/result types.
- Re-run `x07 xtal spec extract`.


## `EXTAL_SPEC_HAS_ASSUMPTIONS`

Summary: Spec declares assumptions.

Origins:
- x07 (stage: lint, severity: warning)

Quickfix support: `sometimes`

Details:

Assumptions are recorded for review and trust gating. They may widen the assumption surface of the project.

Agent strategy:

- Review the listed assumption ids and their text in the spec module.
- If assumptions are not intended, remove them.
- Re-run `x07 xtal spec check --project x07.json`.


## `EXTAL_SPEC_IDS_REQUIRED_FOR_SYNC`

Summary: Contract-core clause ids are required for implementation syncing.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `always`

Details:

Implementation syncing uses clause ids to match and update contract-core clauses deterministically.

Agent strategy:

- Run `x07 xtal spec fmt --inject-ids --write --input <spec.x07spec.json>`.
- Re-run `x07 xtal impl sync --project x07.json --write`.


## `EXTAL_SPEC_IO_READ_FAILED`

Summary: Cannot read spec file.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Cannot read spec file.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_JSON_PARSE`

Summary: Spec file contains invalid JSON.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Spec file contains invalid JSON.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_MODULE_ID_INVALID`

Summary: Spec module_id is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec module_id is invalid.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_OPS_EMPTY`

Summary: Spec has zero operations.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec has zero operations.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_OP_ID_DUPLICATE`

Summary: Spec has duplicate operation ids.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec has duplicate operation ids.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_OP_ID_MISSING`

Summary: Spec operation is missing id.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec operation is missing id.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_OP_NAME_INVALID`

Summary: Spec operation name is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec operation name is invalid.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_OP_NAME_MISSING`

Summary: Spec operation is missing name.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec operation is missing name.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_PARAM_NAME_DUPLICATE`

Summary: Spec has duplicate parameter names.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec has duplicate parameter names.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_PARAM_NAME_INVALID`

Summary: Spec parameter name is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec parameter name is invalid.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_PARAM_TY_UNSUPPORTED`

Summary: Spec parameter type is unsupported in the current XTAL subset.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec parameter type is unsupported in the current XTAL subset.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_PROP_ARGS_EMPTY`

Summary: Property args list is empty.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Each `ensures_props` entry must specify at least one argument name that maps to an operation parameter.

Agent strategy:

- Add one or more operation parameter names to `ensures_props[].args`.
- Re-run `x07 xtal tests gen-from-spec --check`.


## `EXTAL_SPEC_PROP_ARG_DUPLICATE`

Summary: Property args contain a duplicate name.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

A property arg list must not repeat the same operation parameter name.

Agent strategy:

- Remove the duplicate arg name(s) from `ensures_props[].args`.
- Re-run `x07 xtal tests gen-from-spec --check`.


## `EXTAL_SPEC_PROP_ARG_NAME_INVALID`

Summary: Property wrapper parameter name is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The generated property wrapper could not derive a valid local parameter name from the referenced operation parameter name.

Agent strategy:

- Rename the operation parameter to a valid local name in the spec and implementation.
- Re-run `x07 xtal tests gen-from-spec --write`, then commit updated outputs.


## `EXTAL_SPEC_PROP_ARG_UNKNOWN`

Summary: Property arg name does not match an operation parameter.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Each `ensures_props[].args[]` entry must reference a parameter name declared by the operation.

Agent strategy:

- Fix the arg name to match an operation parameter name.
- Re-run `x07 xtal tests gen-from-spec --check`.


## `EXTAL_SPEC_PROP_NAME_INVALID`

Summary: Property name is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

An `ensures_props[].prop` value must be a qualified symbol of the form `module.name`.

Agent strategy:

- Fix `ensures_props[].prop` to a valid qualified symbol.
- Ensure the referenced property function exists and is exported.
- Re-run `x07 xtal tests gen-from-spec --check`.


## `EXTAL_SPEC_PROP_TY_UNSUPPORTED`

Summary: Property argument type is unsupported for PBT generation.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The current PBT generator supports `i32`, `bytes`, and `bytes_view` operation parameter types for property wrappers.

Agent strategy:

- Adjust the spec operation parameter types into the supported subset for PBT.
- Or omit the property from `ensures_props` until a generator mapping exists.
- Re-run `x07 xtal tests gen-from-spec --check`.


## `EXTAL_SPEC_RESULT_TY_UNSUPPORTED`

Summary: Spec result type is unsupported in the current XTAL subset.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Spec result type is unsupported in the current XTAL subset.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_SCHEMA_INVALID`

Summary: Spec JSON violates the structural schema.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Spec JSON violates the structural schema.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_SPEC_SCHEMA_VERSION_UNSUPPORTED`

Summary: Spec schema_version is unsupported.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Spec schema_version is unsupported.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `EXTAL_TASKS_INDEX_INVALID`

Summary: Tasks index is invalid.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The tasks index document is not schema-valid or has an unsupported `schema_version`.

Agent strategy:

- Validate `arch/tasks/index.x07tasks.json` against `spec/x07-arch.tasks.index.schema.json`.
- Ensure `schema_version` is exactly `x07.arch.tasks.index@0.1.0`.
- Re-run `x07 xtal tasks run` after fixing the document.


## `EXTAL_TASKS_INDEX_MISSING`

Summary: Tasks index not found.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The recovery task executor requires a tasks index document (default `arch/tasks/index.x07tasks.json`).

Agent strategy:

- Create `arch/tasks/index.x07tasks.json` with `schema_version` set to `x07.arch.tasks.index@0.1.0`.
- Or pass `--index <path>` to point at a different tasks index file.


## `EXTAL_TASKS_PROJECT_MISSING`

Summary: XTAL tasks run requires a project.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The recovery task executor requires a project root containing `x07.json`.

Agent strategy:

- Run the command from a project directory that contains `x07.json`.
- Or pass `--project /path/to/x07.json`.


## `EXTAL_TASKS_TASK_FAILED`

Summary: Recovery task failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

A recovery task subprocess exited with a non-zero status.

Inspect `target/xtal/xtal.tasks.diag.json` and the recovery events log under `target/xtal/events/<incident_id>/events.jsonl`.

Agent strategy:

- Inspect `target/xtal/xtal.tasks.diag.json` for the failing task id and stderr summary.
- Inspect `target/xtal/events/<incident_id>/events.jsonl` for task ordering and retries.
- Fix the task function implementation or tighten budgets/policy, then re-run `x07 xtal tasks run`.


## `EXTAL_TASKS_TASK_SKIPPED`

Summary: Recovery task was skipped.

Origins:
- x07 (stage: run, severity: warning)

Quickfix support: `sometimes`

Details:

A recovery task was skipped because a dependency failed or retries were exhausted under policy.

Inspect the recovery events log under `target/xtal/events/<incident_id>/events.jsonl` to see the skip reason.

Agent strategy:

- Inspect `target/xtal/events/<incident_id>/events.jsonl` to see why the task was skipped (dependency failure or retry exhaustion).
- If the task is required, change its `criticality`/`on_failure` policy in `arch/tasks/index.x07tasks.json` and re-run.


## `EXTAL_VERIFY_COVERAGE_FAILED`

Summary: Coverage verification failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

XTAL verify runs `x07 verify --coverage` per entry and expects success.

Message: `Coverage verification failed for "{entry}" (world="{world}"). See report: {report_path}.`

Agent strategy:

- Open the referenced coverage report JSON and inspect the first error diagnostic.
- Fix the underlying contract/type issue and re-run `x07 xtal verify`.


## `EXTAL_VERIFY_PROVE_COUNTEREXAMPLE`

Summary: Proof attempt found a counterexample.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

XTAL verify runs bounded proof attempts per entry. A counterexample indicates a real contract violation under the verification model.

Message: `Proof attempt found a counterexample for "{entry}" (world="{world}", policy="{policy}"). See report: {report_path}.`

Agent strategy:

- Inspect the prove report JSON for the counterexample payload and the failing clause id.
- Apply the minimal contract-preserving fix and re-run `x07 xtal verify`.


## `EXTAL_VERIFY_PROVE_ERROR`

Summary: Proof attempt failed with an internal error.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

A proof attempt exited unexpectedly.

Message: `Proof attempt failed with an internal error for "{entry}" (world="{world}"). Exit code {exit_code}. See report: {report_path}. stderr: {stderr_1l}`

Agent strategy:

- Inspect the prove report JSON and stderr excerpt.
- Reduce the entry surface (smaller bounds) or file a toolchain bug if the failure is internal.


## `EXTAL_VERIFY_PROVE_TOOL_MISSING`

Summary: Proof tool is missing or unavailable.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proof engine required for `x07 verify --prove` is not available on this system.

Message: `Proof tool is missing or unavailable while proving "{entry}" (world="{world}"). See report: {report_path}.`

Agent strategy:

- Install the missing solver/toolchain dependency (for example `z3`).
- Re-run `x07 xtal verify`.


## `EXTAL_VERIFY_REPORT_MISSING`

Summary: Expected verify report was not produced.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

A nested `x07 verify` or `x07 test` invocation did not produce the expected report file at the configured path.

Message: `Expected verify report was not produced for "{entry}" (world="{world}"): {report_path}.`

Agent strategy:

- Re-run with `--report-out` pointing at a writable path.
- Inspect stderr for the nested command to see why the report was not written.


## `EXTAL_VERIFY_TESTS_FAILED`

Summary: XTAL verify: x07 test failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

XTAL verify: x07 test failed.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Reproduce the diagnostic and apply the recommended fix.


## `EXTAL_VERIFY_WORLD_UNSAFE`

Summary: XTAL verify requires a deterministic world by default.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires an explicit world/capability decision (edit x07.json or pass --allow-os-world).

Details:

XTAL verification is deterministic by default and requires `solve-*` worlds unless explicitly overridden.

Message: `XTAL verify requires a deterministic solve-* world by default; found world={world} (pass --allow-os-world to override).`

Agent strategy:

- Set `world` in `x07.json` to a `solve-*` world (for example `solve-pure` or `solve-full`).
- Re-run `x07 xtal verify`.
- If OS authority is required, pass `--allow-os-world` and accept nondeterminism.


## `E_ARCH_ARCHIVE_BUDGET_PROFILE_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_ARCHIVE_BUDGET_PROFILE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_ARCHIVE_OP_UNKNOWN`

Summary: Architecture contract diagnostic `E_ARCH_ARCHIVE_OP_UNKNOWN`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_ARCHIVE_SCHEMA_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_ARCHIVE_SCHEMA_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BOUNDARY_BRAND_OR_SCHEMA_REQUIRED`

Summary: Architecture contract diagnostic `E_ARCH_BOUNDARY_BRAND_OR_SCHEMA_REQUIRED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BOUNDARY_ERROR_SPACE_REQUIRED`

Summary: Architecture contract diagnostic `E_ARCH_BOUNDARY_ERROR_SPACE_REQUIRED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BOUNDARY_INDEX_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_BOUNDARY_INDEX_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BOUNDARY_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_BOUNDARY_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BOUNDARY_VERIFY_REQUIRED`

Summary: Architecture contract diagnostic `E_ARCH_BOUNDARY_VERIFY_REQUIRED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BUDGETS_DIR_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_BUDGETS_DIR_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BUDGETS_GEN_DIR_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_BUDGETS_GEN_DIR_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BUDGETS_INDEX_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_BUDGETS_INDEX_SCHEMA_VERSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BUDGET_PROFILE_NOT_FOUND`

Summary: Architecture contract diagnostic `E_ARCH_BUDGET_PROFILE_NOT_FOUND`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BUDGET_PROFILE_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_BUDGET_PROFILE_SCHEMA_VERSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


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


## `E_ARCH_BUDGET_SELECTOR_FN_NOT_FOUND`

Summary: Architecture contract diagnostic `E_ARCH_BUDGET_SELECTOR_FN_NOT_FOUND`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BUDGET_SELECTOR_MODULE_PREFIX_NOT_FOUND`

Summary: Architecture contract diagnostic `E_ARCH_BUDGET_SELECTOR_MODULE_PREFIX_NOT_FOUND`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_BUDGET_WORLD_VIOLATION`

Summary: Architecture contract diagnostic `E_ARCH_BUDGET_WORLD_VIOLATION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CLI_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_CLI_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CLI_PROFILE_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_CLI_PROFILE_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CONTRACTS_CANONICAL_JSON_UNSUPPORTED`

Summary: Architecture contract diagnostic `E_ARCH_CONTRACTS_CANONICAL_JSON_UNSUPPORTED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CONTRACTS_LOCK_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_CONTRACTS_LOCK_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CONTRACTS_LOCK_JSON_PARSE`

Summary: Architecture contract diagnostic `E_ARCH_CONTRACTS_LOCK_JSON_PARSE`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CONTRACTS_LOCK_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_CONTRACTS_LOCK_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CONTRACTS_LOCK_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_CONTRACTS_LOCK_SCHEMA_VERSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CRAWL_BUDGET_PROFILE_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_CRAWL_BUDGET_PROFILE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CRAWL_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_CRAWL_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CRAWL_POLICY_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_CRAWL_POLICY_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CRYPTO_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_CRYPTO_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CRYPTO_JWT_PROFILE_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_CRYPTO_JWT_PROFILE_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CRYPTO_KEY_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_CRYPTO_KEY_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_CYCLE`

Summary: Architecture contract diagnostic `E_ARCH_CYCLE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_DB_BUDGET_PROFILE_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_DB_BUDGET_PROFILE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_DB_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_DB_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_DB_MIGRATE_PLAN_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_DB_MIGRATE_PLAN_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_DB_QUERY_CATALOG_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_DB_QUERY_CATALOG_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_DB_SQL_HASH_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_DB_SQL_HASH_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_DB_SQL_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_DB_SQL_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_DEPS_DENY`

Summary: Architecture contract diagnostic `E_ARCH_DEPS_DENY`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_DUPLICATE_MODULE_ID`

Summary: Architecture contract diagnostic `E_ARCH_DUPLICATE_MODULE_ID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_EDGE_NOT_ALLOWED`

Summary: Architecture contract diagnostic `E_ARCH_EDGE_NOT_ALLOWED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_EXTERNAL_IMPORT_NOT_ALLOWED`

Summary: Architecture contract diagnostic `E_ARCH_EXTERNAL_IMPORT_NOT_ALLOWED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_IMPORT_PREFIX_DENIED`

Summary: Architecture contract diagnostic `E_ARCH_IMPORT_PREFIX_DENIED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_IMPORT_PREFIX_NOT_ALLOWED`

Summary: Architecture contract diagnostic `E_ARCH_IMPORT_PREFIX_NOT_ALLOWED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_LAYERS_VIOLATION`

Summary: Architecture contract diagnostic `E_ARCH_LAYERS_VIOLATION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_LOCK_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_LOCK_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_LOCK_JSON_PARSE`

Summary: Architecture contract diagnostic `E_ARCH_LOCK_JSON_PARSE`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_LOCK_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_LOCK_MISMATCH`.

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


## `E_ARCH_MANIFEST_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_MANIFEST_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MANIFEST_JSON_PARSE`

Summary: Architecture contract diagnostic `E_ARCH_MANIFEST_JSON_PARSE`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MANIFEST_READ`

Summary: Architecture contract diagnostic `E_ARCH_MANIFEST_READ`.

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


## `E_ARCH_MSG_AMQP_BUDGET_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_MSG_AMQP_BUDGET_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MSG_AMQP_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_MSG_AMQP_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MSG_AMQP_PROFILE_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_MSG_AMQP_PROFILE_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MSG_AMQP_TOPOLOGY_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_MSG_AMQP_TOPOLOGY_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MSG_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_MSG_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MSG_KAFKA_BUDGET_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_MSG_KAFKA_BUDGET_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MSG_KAFKA_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_MSG_KAFKA_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_MSG_KAFKA_PROFILE_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_MSG_KAFKA_PROFILE_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_NET_BUDGET_PROFILE_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_NET_BUDGET_PROFILE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_NET_GRPC_SERVICES_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_NET_GRPC_SERVICES_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_NET_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_NET_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_NET_LIMITS_EXCESSIVE`

Summary: Architecture contract diagnostic `E_ARCH_NET_LIMITS_EXCESSIVE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_NODE_ORPHAN_MODULE`

Summary: Architecture contract diagnostic `E_ARCH_NODE_ORPHAN_MODULE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_NODE_OVERLAP_MODULE`

Summary: Architecture contract diagnostic `E_ARCH_NODE_OVERLAP_MODULE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_OBS_BUDGET_PROFILE_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_OBS_BUDGET_PROFILE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_OBS_EXPORTER_PROFILE_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_OBS_EXPORTER_PROFILE_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_OBS_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_OBS_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_OBS_REGISTRY_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_OBS_REGISTRY_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_PUBLIC_BYTES_UNBRANDED`

Summary: Architecture contract diagnostic `E_ARCH_PUBLIC_BYTES_UNBRANDED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_CASSETTE_BRAND_UNSUPPORTED`

Summary: Architecture contract diagnostic `E_ARCH_RR_CASSETTE_BRAND_UNSUPPORTED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_DIR_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_RR_DIR_MISSING`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_GEN_DIR_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_RR_GEN_DIR_MISSING`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_INDEX_DEFAULTS_DUP`

Summary: Architecture contract diagnostic `E_ARCH_RR_INDEX_DEFAULTS_DUP`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_INDEX_DEFAULTS_NOT_SORTED`

Summary: Architecture contract diagnostic `E_ARCH_RR_INDEX_DEFAULTS_NOT_SORTED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_INDEX_NOT_SORTED`

Summary: Architecture contract diagnostic `E_ARCH_RR_INDEX_NOT_SORTED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_INDEX_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_RR_INDEX_SCHEMA_VERSION`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_OP_KIND_NOT_ALLOWED`

Summary: Architecture contract diagnostic `E_ARCH_RR_OP_KIND_NOT_ALLOWED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_OP_KIND_UNKNOWN`

Summary: Architecture contract diagnostic `E_ARCH_RR_OP_KIND_UNKNOWN`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_OP_NOT_ALLOWED`

Summary: Architecture contract diagnostic `E_ARCH_RR_OP_NOT_ALLOWED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_POLICY_ID_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_RR_POLICY_ID_MISMATCH`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_POLICY_KINDS_NOT_SORTED`

Summary: Architecture contract diagnostic `E_ARCH_RR_POLICY_KINDS_NOT_SORTED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_POLICY_MODE_NOT_ALLOWED`

Summary: Architecture contract diagnostic `E_ARCH_RR_POLICY_MODE_NOT_ALLOWED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_POLICY_NOT_FOUND`

Summary: Architecture contract diagnostic `E_ARCH_RR_POLICY_NOT_FOUND`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_POLICY_OPS_NOT_SORTED`

Summary: Architecture contract diagnostic `E_ARCH_RR_POLICY_OPS_NOT_SORTED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_POLICY_REQUIRED`

Summary: Architecture contract diagnostic `E_ARCH_RR_POLICY_REQUIRED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_POLICY_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_RR_POLICY_SCHEMA_VERSION`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_POLICY_WORLDS_NOT_SORTED`

Summary: Architecture contract diagnostic `E_ARCH_RR_POLICY_WORLDS_NOT_SORTED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_SANITIZER_ID_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_RR_SANITIZER_ID_MISMATCH`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_SANITIZER_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_RR_SANITIZER_SCHEMA_VERSION`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_RR_WORLD_VIOLATION`

Summary: Architecture contract diagnostic `E_ARCH_RR_WORLD_VIOLATION`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SMOKE_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_SMOKE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_DIR_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_SM_DIR_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_GEN_DIR_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_SM_GEN_DIR_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_GEN_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_SM_GEN_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_GEN_PARSE`

Summary: Architecture contract diagnostic `E_ARCH_SM_GEN_PARSE`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_GEN_STALE`

Summary: Architecture contract diagnostic `E_ARCH_SM_GEN_STALE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_IMPORT_NOT_INDEXED`

Summary: Architecture contract diagnostic `E_ARCH_SM_IMPORT_NOT_INDEXED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_INDEX_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_SM_INDEX_SCHEMA_VERSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_SPEC_MACHINE_ID_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_SM_SPEC_MACHINE_ID_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_SPEC_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_SM_SPEC_SCHEMA_VERSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_SPEC_VERSION_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_SM_SPEC_VERSION_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_SM_SPEC_WORLD_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_SM_SPEC_WORLD_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGINS_DIR_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_DIR_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGINS_GEN_DIR_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_GEN_DIR_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGINS_INDEX_DUPLICATE_ID`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_INDEX_DUPLICATE_ID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGINS_INDEX_NOT_SORTED`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_INDEX_NOT_SORTED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGINS_INDEX_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGINS_INDEX_SCHEMA_VERSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_BUDGET_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_BUDGET_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_NOT_FOUND`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_NOT_FOUND`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_SPEC_ABI_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_ABI_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_SPEC_BRANDS_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_BRANDS_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_SPEC_BUDGETS_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_BUDGETS_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_SPEC_CFG_CANON_MODE`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_CFG_CANON_MODE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_SPEC_DETERMINISM_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_DETERMINISM_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_SPEC_ID_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_ID_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_SPEC_LIMITS_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_LIMITS_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_SPEC_SCHEMA_VERSION`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_SCHEMA_VERSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_SPEC_WORLDS_MISMATCH`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_SPEC_WORLDS_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_STREAM_PLUGIN_WORLD_VIOLATION`

Summary: Architecture contract diagnostic `E_ARCH_STREAM_PLUGIN_WORLD_VIOLATION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_TASKS_INDEX_INVALID`

Summary: Architecture tasks index is invalid.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The tasks index file is not schema-valid or violates semantic constraints (unique ids, acyclic deps, and policy consistency).

Agent strategy:

- Validate `arch/tasks/index.x07tasks.json` against `x07.arch.tasks.index@0.1.0`.
- Fix invalid task ids, missing deps, or dependency cycles.
- Re-run `x07 arch check`.


## `E_ARCH_TASKS_INDEX_MISSING`

Summary: Architecture tasks index is missing.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The architecture manifest referenced a tasks index file, but it was not found.

Agent strategy:

- Ensure `arch/tasks/index.x07tasks.json` exists (or update `contracts_v1.tasks.index_path`).
- Re-run `x07 arch check`.


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


## `E_ARCH_TRUST_ZONE_EDGE`

Summary: Architecture contract diagnostic `E_ARCH_TRUST_ZONE_EDGE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_VISIBILITY`

Summary: Architecture contract diagnostic `E_ARCH_VISIBILITY`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_WEB_API_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_WEB_API_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_WEB_BUDGET_PROFILE_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_WEB_BUDGET_PROFILE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_WEB_INDEX_MISSING`

Summary: Architecture contract diagnostic `E_ARCH_WEB_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_WEB_OPENAPI_PROFILE_INVALID`

Summary: Architecture contract diagnostic `E_ARCH_WEB_OPENAPI_PROFILE_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_ARCH_WORLD_EDGE_FORBIDDEN`

Summary: Architecture contract diagnostic `E_ARCH_WORLD_EDGE_FORBIDDEN`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The repository contracts under `arch/` or scanned module graph violate deterministic policy. Repair is often mechanical via manifest/lock/contracts updates.

Agent strategy:

- Run `x07 arch check --write-lock`.
- Apply suggested manifest/contracts updates.
- Re-run `x07 arch check` until green.


## `E_BENCH_SUITE_LOAD`

Summary: Diagnostic code `E_BENCH_SUITE_LOAD`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Fallback catalog entry for `E_BENCH_SUITE_LOAD`. Investigate producer-specific remediation.

Agent strategy:

- Reproduce `E_BENCH_SUITE_LOAD` with the failing command.
- Inspect structured diagnostic fields.
- Apply deterministic edits and re-run.


## `E_DEPS_CAP_POLICY_DENY`

Summary: Diagnostic code `E_DEPS_CAP_POLICY_DENY`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Fallback catalog entry for `E_DEPS_CAP_POLICY_DENY`. Investigate producer-specific remediation.

Agent strategy:

- Reproduce `E_DEPS_CAP_POLICY_DENY` with the failing command.
- Inspect structured diagnostic fields.
- Apply deterministic edits and re-run.


## `E_GEN_DRIFT`

Summary: Committed generator outputs drifted.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Committed generator outputs drifted.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_ENTRY_CHECK_ARGV_CONTAINS_WRITE`

Summary: Generator index entry check_argv contains --write.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Generator index entry check_argv contains --write.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_ENTRY_CHECK_ARGV_EMPTY`

Summary: Generator index entry check_argv is empty.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Generator index entry check_argv is empty.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_ENTRY_MODE_UNSUPPORTED`

Summary: Generator index entry mode is unsupported.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Generator index entry mode is unsupported.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_ENTRY_OUTPUTS_EMPTY`

Summary: Generator index entry outputs is empty.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Generator index entry outputs is empty.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_ENTRY_WRITE_ARGV_CONTAINS_CHECK`

Summary: Generator index entry write_argv contains --check.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Generator index entry write_argv contains --check.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_ENTRY_WRITE_ARGV_EMPTY`

Summary: Generator index entry write_argv is empty.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Generator index entry write_argv is empty.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_INDEX_IO_READ_FAILED`

Summary: Cannot read generator index file.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Cannot read generator index file.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_INDEX_JSON_PARSE`

Summary: Generator index file contains invalid JSON.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Generator index file contains invalid JSON.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_INDEX_SCHEMA_INVALID`

Summary: Generator index violates the structural schema.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Generator index violates the structural schema.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_INDEX_SCHEMA_VERSION_UNSUPPORTED`

Summary: Generator index schema_version is unsupported.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

Generator index schema_version is unsupported.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_MISSING_OUTPUT`

Summary: Declared generator output root is missing.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Declared generator output root is missing.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_NONDETERMINISTIC`

Summary: Generator outputs are not deterministic.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Generator outputs are not deterministic.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_GEN_RUN_FAILED`

Summary: Generator command failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Generator command failed.

This diagnostic is deterministic and intended to be actionable in the standard x07 repair loop.

Agent strategy:

- Fix `arch/gen/index.x07gen.json` (schema/version/argv/outputs).
- Regenerate outputs with `x07 gen write --index arch/gen/index.x07gen.json`.
- Commit outputs and verify with `x07 gen verify --index arch/gen/index.x07gen.json`.


## `E_SBOM_GENERATION_FAILED`

Summary: Diagnostic code `E_SBOM_GENERATION_FAILED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Fallback catalog entry for `E_SBOM_GENERATION_FAILED`. Investigate producer-specific remediation.

Agent strategy:

- Reproduce `E_SBOM_GENERATION_FAILED` with the failing command.
- Inspect structured diagnostic fields.
- Apply deterministic edits and re-run.


## `WXTAL_IMPL_PARAM_NAME_MISMATCH`

Summary: Implementation parameter names differ from the spec.

Origins:
- x07 (stage: lint, severity: warning)

Quickfix support: `sometimes`

Details:

Parameter names are currently checked at warning level; types/brands must still match exactly.

Agent strategy:

- Rename implementation parameters to match the spec parameter names for consistency.
- Re-run `x07 xtal impl check`.


## `WXTAL_IMPROVE_NO_PATCH`

Summary: XTAL improve produced no patchset.

Origins:
- x07 (stage: run, severity: warning)

Quickfix support: `sometimes`

Details:

The bounded repair step completed without producing a patchset. In this lane it is recorded as a warning and no edits are applied.

Agent strategy:

- Inspect `target/xtal/repair/summary.json` and `target/xtal/xtal.repair.diag.json`.
- Retry with a smaller incident input or add a targeted regression test to improve localization.


## `WXTAL_REPAIR_QUICKFIX_APPLIED`

Summary: XTAL repair applied a quickfix fallback in an attempt workspace.

Origins:
- x07 (stage: run, severity: warning)

Quickfix support: `never`
No quickfix reason: Informational warning; not a fixable error condition.

Details:

XTAL repair may fall back to `x07 fix` for a targeted attempt when semantic repair is unavailable or fails.

Agent strategy:

- Review the attempt diff under `target/xtal/repair/attempts/` and decide whether to apply the suggested patchset.


## `WXTAL_REPAIR_SPEC_PATCH_SUGGESTED`

Summary: XTAL emitted a spec patch suggestion for review.

Origins:
- x07 (stage: run, severity: warning)

Quickfix support: `sometimes`

Details:

No spec-preserving patch was found; XTAL emitted a spec witness suggestion patchset for review.

Message: `no spec-preserving patch found; emitted a spec patch suggestion for review`

Agent strategy:

- Review the suggested patchset under `target/xtal/repair/attempts/`.
- Apply it manually if it matches the intended behavior, then re-run `x07 xtal verify`.


## `WXTAL_SPEC_NONCANONICAL_JSON`

Summary: Spec JSON is not in canonical form.

Origins:
- x07 (stage: rewrite, severity: warning)

Quickfix support: `always`

Details:

The spec parses and passes schema checks, but its bytes are not in canonical JSON form. Canonical formatting is deterministic and intended for reviewable diffs.

Agent strategy:

- Run `x07 xtal spec fmt --write --input <spec.x07spec.json>` for canonical JSON.
- Run `x07 xtal spec lint --input <spec.x07spec.json>` and `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`.
- Apply deterministic edits to fix reported pointers, then re-run.


## `WXTAL_VERIFY_PROVE_INCONCLUSIVE`

Summary: Proof attempt was inconclusive.

Origins:
- x07 (stage: run, severity: warning)

Quickfix support: `sometimes`

Details:

A bounded proof attempt returned `inconclusive`. In balanced policy this is recorded as a warning; in strict policy it fails the command.

Message: `Proof attempt was inconclusive for "{entry}" (world="{world}", policy="{policy}"). See report: {report_path}.`

Agent strategy:

- Inspect the prove report JSON to see why it was inconclusive.
- If needed, raise bounds (`--unwind`, `--max-bytes-len`) or switch to `--proof-policy strict` only when the codebase is ready.


## `WXTAL_VERIFY_PROVE_TIMEOUT`

Summary: Proof attempt hit the configured budget.

Origins:
- x07 (stage: run, severity: warning)

Quickfix support: `sometimes`

Details:

The solver reported a timeout (within configured bounds). In balanced policy this is recorded as a warning; in strict policy it fails the command.

Message: `Proof attempt hit the configured budget for "{entry}" (world="{world}", policy="{policy}"). See report: {report_path}.`

Agent strategy:

- Increase budgets (`--unwind`, `--max-bytes-len`, `--input-len-bytes`) and re-run `x07 xtal verify`.
- If timeouts are expected, keep balanced policy so the lane remains usable.


## `WXTAL_VERIFY_PROVE_TOOL_MISSING`

Summary: Proof tool is missing or unavailable.

Origins:
- x07 (stage: run, severity: warning)

Quickfix support: `sometimes`

Details:

The proof engine required for `x07 verify --prove` is not available on this system. In balanced policy this is recorded as a warning; in strict policy it fails the command.

Message: `Proof tool is missing or unavailable while proving "{entry}" (world="{world}", policy="{policy}"). See report: {report_path}.`

Agent strategy:

- Install the missing solver/toolchain dependency (for example `z3`).
- Re-run `x07 xtal verify`.
- If the environment cannot provide proof tools, keep balanced proof policy and rely on tests/runtime monitoring for that surface.


## `WXTAL_VERIFY_PROVE_UNSUPPORTED`

Summary: Proof attempt is unsupported for this entry.

Origins:
- x07 (stage: run, severity: warning)

Quickfix support: `sometimes`

Details:

The proof engine reported `unsupported` for this entry. In balanced policy this is recorded as a warning; in strict policy it fails the command.

Message: `Proof attempt is unsupported for "{entry}" (world="{world}", policy="{policy}"). See report: {report_path}.`

Agent strategy:

- Inspect the prove report JSON for the unsupported reason code.
- If the entry is outside the supported subset, keep balanced policy and rely on tests/runtime checks for that surface.


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

Summary: Diagnostic code `W_DEPS_CAP_POLICY_MISSING`.

Origins:
- x07 (stage: lint, severity: warning)

Quickfix support: `sometimes`

Details:

Fallback catalog entry for `W_DEPS_CAP_POLICY_MISSING`. Investigate producer-specific remediation.

Agent strategy:

- Reproduce `W_DEPS_CAP_POLICY_MISSING` with the failing command.
- Inspect structured diagnostic fields.
- Apply deterministic edits and re-run.


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


## `X07-ARITY-BINOP-0001`

Summary: Binary operator has invalid arity.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

Binary operators in x07AST take exactly two arguments.

Valid form: `["<op>", <a>, <b>].`

If you need more than two operands, nest the operator (left-associative), for example: ["+", ["+", a, b], c].

A quickfix may be emitted for some cases (for example n-ary `+`).

Agent strategy:

- Rewrite to the binary form: `["<op>", <a>, <b>].`
- For n-ary uses, nest left-associatively: `["<op>", ["<op>", a, b], c].`
- If quickfix is present, apply it (for example via `x07 fix --write --input <PATH>`).
- Re-run lint.


## `X07-ARITY-FOR-0001`

Summary: `for` has invalid arity.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

A quickfix is emitted when extra trailing expressions are present: they are wrapped into a `begin` body so the loop keeps canonical arity.

Agent strategy:

- Valid form: `["for", "i", <start:i32>, <end:i32>, <body:any>].`
- If quickfix is present, apply it to wrap extra body expressions in `begin` (or run `x07 fix --write --input <PATH>`).
- Otherwise rewrite the loop to canonical arity; if you need multiple body expressions, wrap them in `begin`.
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


## `X07-ARITY-WHILE-0001`

Summary: `while` has invalid arity.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

A quickfix is emitted when extra trailing expressions are present: they are wrapped into a `begin` body so the loop keeps canonical arity.

Agent strategy:

- If quickfix is present, apply it to wrap extra body expressions.
- Otherwise rewrite to `while <cond> <body>` manually.
- Re-run lint.


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


## `X07-ASYNC-CONTRACT-0001`

Summary: Core lint/schema diagnostic `X07-ASYNC-CONTRACT-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-ASYNC-CONTRACT-0002`

Summary: Core lint/schema diagnostic `X07-ASYNC-CONTRACT-0002`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-ASYNC-CONTRACT-0003`

Summary: Core lint/schema diagnostic `X07-ASYNC-CONTRACT-0003`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-BOOL-0001`

Summary: Core lint/schema diagnostic `X07-BOOL-0001`.

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


## `X07-BORROW-0002`

Summary: Core lint/schema diagnostic `X07-BORROW-0002`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-CLI-ARGS-0001`

Summary: Core lint/schema diagnostic `X07-CLI-ARGS-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-CLI-ARGS-0002`

Summary: Core lint/schema diagnostic `X07-CLI-ARGS-0002`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-CONTRACT-0001`

Summary: Core lint/schema diagnostic `X07-CONTRACT-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-CONTRACT-0002`

Summary: Core lint/schema diagnostic `X07-CONTRACT-0002`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-CONTRACT-0003`

Summary: Core lint/schema diagnostic `X07-CONTRACT-0003`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-CONTRACT-0004`

Summary: Core lint/schema diagnostic `X07-CONTRACT-0004`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-CONTRACT-0005`

Summary: Core lint/schema diagnostic `X07-CONTRACT-0005`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-CONTRACT-0009`

Summary: Recursive decreases clause is invalid.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

A `decreases[]` clause must stay contract-pure, typecheck to `i32`, and keep its witness payload within the supported contract witness subset.

Agent strategy:

- Keep `decreases[].expr` in the contract-pure subset.
- Make each `decreases[].expr` typecheck to `i32`.
- Keep `decreases[].witness[]` values in the supported contract witness subset.
- Re-run `x07 lint` or `x07 check`.


## `X07-CONTRACT-0010`

Summary: Recursive decreases metadata is attached to a non-certifiable target.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The enclosing `defn` declares `decreases[]`, but the function is not a directly self-recursive proof target.

Agent strategy:

- Keep `decreases[]` only on directly self-recursive `defn` targets.
- Remove `decreases[]` from non-recursive helpers.
- Re-run `x07 lint` or `x07 check`.


## `X07-CONTRACT-0011`

Summary: Recursive self-call is missing decreases evidence.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

The function body makes a direct self-recursive call, but the enclosing `defn` does not declare `decreases[]` to justify the termination rank.

Agent strategy:

- Add `decreases[]` to the enclosing `defn`.
- Keep the recursive target inside the certifiable pure subset.
- Re-run `x07 lint` or `x07 check`.


## `X07-FIX-0003`

Summary: Core lint/schema diagnostic `X07-FIX-0003`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-FMT-0001`

Summary: Core lint/schema diagnostic `X07-FMT-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-FOR-0001`

Summary: `for` loop variable must be an identifier.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires semantic intent (choosing a loop variable name).

Details:

The `for` form expects an identifier as its first argument after the head.

Valid form: `["for", "i", <start:i32>, <end:i32>, <body:any>].`

Agent strategy:

- Replace the loop variable with an identifier string (for example "i").
- Re-run lint.


## `X07-GENERICS-0001`

Summary: Core lint/schema diagnostic `X07-GENERICS-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-GENERICS-0002`

Summary: Core lint/schema diagnostic `X07-GENERICS-0002`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-INTERNAL-0001`

Summary: Internal toolchain/compiler error.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

`X07-INTERNAL-0001` indicates an unexpected toolchain/compiler failure (an invariant violation or missing implementation).

This should not be emitted for ordinary user type errors; if you see it from `x07 check`, please file a bug with a minimal repro.

Agent strategy:

- Reduce to the smallest x07AST that reproduces the failure.
- Run `x07 fmt`, `x07 lint`, and `x07 fix` to rule out shape/schema issues.
- Capture the machine report (`--json=pretty`) and toolchain version (`x07 --version`) when filing a bug.


## `X07-IO-READ-0001`

Summary: Core lint/schema diagnostic `X07-IO-READ-0001`.

Origins:
- x07 (stage: lint, severity: error)
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-IO-WRITE-0001`

Summary: Core lint/schema diagnostic `X07-IO-WRITE-0001`.

Origins:
- x07 (stage: lint, severity: error)
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-JSON-PARSE-0001`

Summary: Core lint/schema diagnostic `X07-JSON-PARSE-0001`.

Origins:
- x07c (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-MIGRATE-COERCE-0001`

Summary: Migration required: explicitize compat coercions.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

This program relies on call-argument compatibility coercions (for example `bytes`/`vec_u8` where `bytes_view` is expected). Use `x07 migrate` to rewrite calls to explicit conversions such as `bytes.view`, `bytes.view_lit`, or `vec_u8.as_view`, introducing temporaries when needed to avoid borrowing from temporaries.

Agent strategy:

- Run `x07 migrate --check --to 0.5` to find required migrations.
- Run `x07 migrate --write --to 0.5` to apply deterministic rewrites.
- Re-run `x07 check` / tests with `--compat strict` (or `X07_COMPAT=strict`) to confirm no remaining implicit coercions.


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


## `X07-PATCHSET-0001`

Summary: Core lint/schema diagnostic `X07-PATCHSET-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


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

Summary: Core lint/schema diagnostic `X07-SCHEMA-0002`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TAPP-ELAB-0001`

Summary: Core lint/schema diagnostic `X07-TAPP-ELAB-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TAPP-INFER-0001`

Summary: Core lint/schema diagnostic `X07-TAPP-INFER-0001`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TOOL-ARGS-0001`

Summary: Core lint/schema diagnostic `X07-TOOL-ARGS-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TOOL-EXEC-0001`

Summary: Core lint/schema diagnostic `X07-TOOL-EXEC-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TYPE-0001`

Summary: Core lint/schema diagnostic `X07-TYPE-0001`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TYPE-CALL-0001`

Summary: Unknown callee (function not found).

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The callee name is not present in the available type signatures (usually a typo, a missing import, or a missing export). `data.callee` is provided.

This diagnostic applies to imported modules (including builtin stdlib modules such as `std.bytes.*` and `std.vec.*`).

Agent strategy:

- Check the spelling of `data.callee`.
- Ensure the defining module is imported and exports the symbol.
- Use `x07 doc`/`x07 guide` to find the canonical function name and signature.
- Re-run `x07 check`.


## `X07-TYPE-CALL-0002`

Summary: Call argument type mismatch.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The argument type does not match the callee signature. The diagnostic includes:

- `data.callee` (string)
- `data.arg_index` (0-based)
- `data.expected` (type)
- `data.got` (type)

Agent strategy:

- Adjust the argument expression, or insert an explicit conversion to the expected type.
- Re-run `x07 check` (or `x07 run`) to confirm the type error is resolved.


## `X07-TYPE-CALL-0003`

Summary: Core lint/schema diagnostic `X07-TYPE-CALL-0003`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TYPE-COERCE-0001`

Summary: Implicit call-argument compat coercion occurred.

Origins:
- x07c (stage: type, severity: warning)

Quickfix support: `sometimes`

Details:

In strict compat mode, the typechecker reports when it accepted an implicit call-argument coercion (for example `bytes`/`vec_u8` into `bytes_view`). The compiler still permits these coercions for now, but the warning indicates a mechanical migration is available.

Agent strategy:

- Run `x07 migrate --write --to 0.5` to make coercions explicit.
- Or rewrite the call site manually using `bytes.view` / `bytes.view_lit` / `vec_u8.as_view`.
- Re-run `x07 check --compat strict` (or `X07_COMPAT=strict`) until the warning is gone.


## `X07-TYPE-IF-0001`

Summary: Core lint/schema diagnostic `X07-TYPE-IF-0001`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TYPE-IF-0002`

Summary: Core lint/schema diagnostic `X07-TYPE-IF-0002`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TYPE-RET-0001`

Summary: Core lint/schema diagnostic `X07-TYPE-RET-0001`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TYPE-SET-0001`

Summary: Core lint/schema diagnostic `X07-TYPE-SET-0001`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TYPE-SET-0002`

Summary: Core lint/schema diagnostic `X07-TYPE-SET-0002`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TYPE-TRYDOC-0001`

Summary: `try_doc` expects bytes or bytes_view.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

`try_doc` consumes a doc envelope (tag byte + payload). Ensure the argument expression evaluates to `bytes` or `bytes_view`.

Agent strategy:

- Fix the `try_doc` argument to produce a doc envelope (`bytes` / `bytes_view`).
- Re-run `x07 check` / `x07 test`.


## `X07-TYPE-UNIFY-0001`

Summary: Core lint/schema diagnostic `X07-TYPE-UNIFY-0001`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


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
- x07c (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07CAP_ATTEST_DIGEST_MISMATCH`

Summary: Core lint/schema diagnostic `X07CAP_ATTEST_DIGEST_MISMATCH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07CAP_CONFORMANCE_MISSING`

Summary: Core lint/schema diagnostic `X07CAP_CONFORMANCE_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07CAP_CONFORMANCE_REQUIRED`

Summary: Core lint/schema diagnostic `X07CAP_CONFORMANCE_REQUIRED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07CAP_CONTRACT_INVALID`

Summary: Core lint/schema diagnostic `X07CAP_CONTRACT_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07CAP_EFFECT_LOG_REQUIRED`

Summary: Core lint/schema diagnostic `X07CAP_EFFECT_LOG_REQUIRED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07CAP_INDEX_INVALID`

Summary: Core lint/schema diagnostic `X07CAP_INDEX_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07CAP_PEER_POLICY_REQUIRED`

Summary: Core lint/schema diagnostic `X07CAP_PEER_POLICY_REQUIRED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07CAP_TLS_POLICY_INCOMPLETE`

Summary: Core lint/schema diagnostic `X07CAP_TLS_POLICY_INCOMPLETE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07C_WASM_BACKEND_FEATURE_REQUIRED`

Summary: Core lint/schema diagnostic `X07C_WASM_BACKEND_FEATURE_REQUIRED`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07C_WASM_BACKEND_UNSUPPORTED_BUILTIN`

Summary: Core lint/schema diagnostic `X07C_WASM_BACKEND_UNSUPPORTED_BUILTIN`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07C_WASM_BACKEND_UNSUPPORTED_FORM`

Summary: Core lint/schema diagnostic `X07C_WASM_BACKEND_UNSUPPORTED_FORM`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07C_WASM_BACKEND_UNSUPPORTED_OP`

Summary: Core lint/schema diagnostic `X07C_WASM_BACKEND_UNSUPPORTED_OP`.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07C_WASM_BACKEND_UNSUPPORTED_TYPE`

Summary: Core lint/schema diagnostic `X07C_WASM_BACKEND_UNSUPPORTED_TYPE`.

Origins:
- x07c (stage: type, severity: error)

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

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_MCP_MISSING`

Summary: Project/package scaffold diagnostic `X07INIT_MCP_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07INIT_MCP_REPORT`

Summary: Project/package scaffold diagnostic `X07INIT_MCP_REPORT`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


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


## `X07INIT_TRUST`

Summary: Project/package scaffold diagnostic `X07INIT_TRUST`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Initialization constraints (existing files/layout/arguments) can usually be resolved by deterministic filesystem and argument changes.

Agent strategy:

- Adjust init target path and flags.
- Ensure required directories/files are in expected state.
- Re-run `x07 init` command variant.


## `X07PKG_ADVISED_DEP`

Summary: Package workflow diagnostic `X07PKG_ADVISED_DEP`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_API_FETCH`

Summary: Registry API request failed.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Depends on remote registry API/network availability.

Details:

A registry API request failed while fetching package metadata. This can be caused by network issues, authentication problems, or a registry outage.

Agent strategy:

- Check the network connection and registry availability.
- If you need offline operation, install the package locally and re-run with `--offline` (where supported).
- Verify your registry/index URL and credentials if the registry requires auth.
- Retry the command after the registry is healthy.


## `X07PKG_API_PARSE`

Summary: Registry API response is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Indicates a registry API contract mismatch or server-side bug.

Details:

The registry API response could not be parsed or did not match the expected shape.

Agent strategy:

- Retry once to rule out transient proxy/caching issues.
- If the failure persists, the registry API may be returning a contract-incompatible response; upgrade the toolchain or the registry, or file a bug with the response body.
- Use local package metadata (installed deps) when offline operation is required.


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


## `X07PKG_LIST_INDEX_MISSING`

Summary: Package workflow diagnostic `X07PKG_LIST_INDEX_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_LIST_UNSUPPORTED`

Summary: Package workflow diagnostic `X07PKG_LIST_UNSUPPORTED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_LOCAL_MISSING_DEP`

Summary: Package workflow diagnostic `X07PKG_LOCAL_MISSING_DEP`.

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


## `X07PKG_LOCK_VERSION_INVALID`

Summary: Package workflow diagnostic `X07PKG_LOCK_VERSION_INVALID`.

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


## `X07PKG_MANIFEST_PARSE`

Summary: Package manifest could not be parsed.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package manifest (`x07-package.json`) is not valid JSON, so dependency resolution cannot proceed.

Agent strategy:

- If the package is local/vendored: fix JSON syntax in `x07-package.json` and re-run `x07 pkg lock`.
- If it was fetched: delete the bad vendored directory (or cache entry) and re-run `x07 pkg lock` to re-fetch.
- Re-run `x07 pkg lock --check` in CI once the workspace is clean.


## `X07PKG_OFFLINE_INDEX`

Summary: Package workflow diagnostic `X07PKG_OFFLINE_INDEX`.

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


## `X07PKG_OFFLINE_REFRESH`

Summary: Package workflow diagnostic `X07PKG_OFFLINE_REFRESH`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_PACK_FAILED`

Summary: Package packing failed.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package directory could not be packed into a `.x07pkg` archive. This typically indicates invalid `x07-package.json` metadata, missing module files, or unsupported files under `ffi/`.

Agent strategy:

- Validate `x07-package.json` fields (including required publishing metadata) and re-run `x07 pkg pack`.
- Ensure all modules listed in `x07-package.json.modules` exist under `module_root`.
- If the package contains `ffi/`, ensure it only contains regular files and directories.
- Re-run with a clean workspace once the package directory is fixed.


## `X07PKG_PACK_OUT`

Summary: Missing output path for package packing.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

`x07 pkg pack` requires an output path via `--out <PATH>` to write the archive.

Agent strategy:

- Re-run with `--out dist/<name>-<version>.x07pkg`.
- Ensure the output directory exists or is writable.


## `X07PKG_PACK_WRITE`

Summary: Failed to write packed archive.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The `.x07pkg` archive was built but could not be written to the output path.

Agent strategy:

- Check filesystem permissions and available disk space.
- Ensure the `--out` parent directory exists.
- Re-run `x07 pkg pack` after fixing the output path.


## `X07PKG_PATCH_MISSING_DEP`

Summary: Package workflow diagnostic `X07PKG_PATCH_MISSING_DEP`.

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


## `X07PKG_PUBLISH_PACK`

Summary: Publish failed while packing the package.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

`x07 pkg publish` failed because `x07 pkg pack` failed for the package directory.

Agent strategy:

- Fix the package directory so `x07 pkg pack --package . --out /tmp/pkg.x07pkg` succeeds.
- Re-run `x07 pkg publish` once packing succeeds.


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


## `X07PKG_REPAIR_LOCAL_INCOMPATIBLE`

Summary: Package workflow diagnostic `X07PKG_REPAIR_LOCAL_INCOMPATIBLE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_REPAIR_LOCK_INVALID`

Summary: Package workflow diagnostic `X07PKG_REPAIR_LOCK_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_REPAIR_LOCK_MISSING`

Summary: Package workflow diagnostic `X07PKG_REPAIR_LOCK_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_REPAIR_NO_COMPATIBLE_VERSION`

Summary: Package workflow diagnostic `X07PKG_REPAIR_NO_COMPATIBLE_VERSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_REPAIR_PATCHED_PATH_INCOMPATIBLE`

Summary: Package workflow diagnostic `X07PKG_REPAIR_PATCHED_PATH_INCOMPATIBLE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_REPAIR_TOOLCHAIN_INVALID`

Summary: Package workflow diagnostic `X07PKG_REPAIR_TOOLCHAIN_INVALID`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_REPAIR_TOOLCHAIN_UNSUPPORTED`

Summary: Package workflow diagnostic `X07PKG_REPAIR_TOOLCHAIN_UNSUPPORTED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PKG_SEMVER_BREAKING`

Summary: Breaking API changes detected.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

`x07 pkg check-semver` detected breaking changes between the old and new package surfaces (removed exports, removed modules, or signature changes).

Agent strategy:

- If the change is intended, bump the major version.
- If the change is unintended, restore the removed export/module or reintroduce a compatible signature.
- Re-run `x07 pkg check-semver` to confirm the surface is compatible.


## `X07PKG_SEMVER_DIR`

Summary: Semver check input directory is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

`x07 pkg check-semver` requires `--old` and `--new` directories that each contain `x07-package.json`.

Agent strategy:

- Ensure both directories exist and contain `x07-package.json`.
- Re-run `x07 pkg check-semver --old <dir> --new <dir>`.


## `X07PKG_SEMVER_LOAD`

Summary: Failed to load package surface for semver check.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The semver checker could not read or typecheck the exported surface of one of the packages.

Agent strategy:

- Ensure the package manifest lists valid modules and that the module files exist.
- Fix any parse/type errors in exported modules.
- Re-run `x07 pkg check-semver` after the package directories are healthy.


## `X07PKG_SEMVER_NAME_MISMATCH`

Summary: Package names do not match for semver check.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

`x07 pkg check-semver` requires the old and new directories to describe the same package name.

Agent strategy:

- Ensure both `x07-package.json` files have the same `name`.
- Re-run `x07 pkg check-semver` with the correct directories.


## `X07PKG_SIGNATURE_INVALID`

Summary: Package signature is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires registry-side re-signing or republishing.

Details:

`x07 pkg verify` found a signature, but it did not validate against the advertised public key.

Agent strategy:

- Treat the package as untrusted and do not use it for sensitive workflows.
- Verify the registry index URL and signing key configuration.
- If you control the registry, republish/re-sign the package and ensure the index entry signature matches the package sha256.


## `X07PKG_SIGNATURE_KEY_MISSING`

Summary: Signing key is missing from index config.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires registry index configuration to include the signing public key.

Details:

`x07 pkg verify` found a signature key id in the index entry, but the corresponding public key was not present in `config.json`.

Agent strategy:

- Verify the index URL points at the expected sparse index root.
- If you control the registry, ensure `config.json` lists the signing public key used for published entries.
- Retry once caches are refreshed.


## `X07PKG_SIGNATURE_UNSUPPORTED`

Summary: Unsupported signature kind.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires toolchain or registry configuration changes.

Details:

`x07 pkg verify` encountered a signing kind that the toolchain does not support.

Agent strategy:

- Upgrade the toolchain if the registry uses a newer signature scheme.
- If you control the registry, configure it to use a supported signature kind.


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


## `X07PKG_TREE_DEP_MANIFEST`

Summary: pkg.tree could not read a dependency package manifest.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

A dependency directory is missing `x07-package.json` or the file is unreadable/schema-invalid.

Agent strategy:

- Ensure the dependency directory exists and contains `x07-package.json`.
- For vendored deps under `.x07/deps`, run `x07 pkg lock --project x07.json` to hydrate missing deps.


## `X07PKG_TREE_DEP_PATH`

Summary: pkg.tree could not resolve a dependency path.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

A dependency path in the lockfile could not be resolved relative to the project root/workspace.

Agent strategy:

- Check `dependencies[].path` in `x07.json` and the corresponding lockfile entries.
- If using workspace paths, set `X07_WORKSPACE_ROOT=...` and rerun `x07 pkg lock`.


## `X07PKG_TREE_LOCK_MISSING`

Summary: pkg.tree requires a lockfile.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

`x07 pkg tree` reads `x07.json` + the resolved lockfile (`x07.lock.json`) to report the dependency closure and module roots. Without a lockfile it cannot produce a stable closure graph.

Agent strategy:

- Run `x07 pkg lock --project x07.json`.
- Re-run `x07 pkg tree --project x07.json`.
- In CI, add `x07 pkg lock --check --project x07.json` to fail if the lock is missing/out of date.


## `X07PKG_TREE_LOCK_PARSE`

Summary: pkg.tree lockfile is not valid JSON.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The lockfile could not be parsed as JSON. This usually means the file is corrupted or was edited manually.

Agent strategy:

- Regenerate the lockfile with `x07 pkg lock --project x07.json`.
- Re-run `x07 pkg tree --project x07.json`.


## `X07PKG_TREE_LOCK_READ`

Summary: pkg.tree could not read the lockfile.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The lockfile exists but could not be read (permissions, broken symlink, IO error).

Agent strategy:

- Verify the lockfile path is readable and is a regular file.
- If the file is corrupted, regenerate it with `x07 pkg lock --project x07.json`.


## `X07PKG_TREE_LOCK_VERIFY`

Summary: pkg.tree lockfile does not match the project manifest.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The lockfile contents do not match `x07.json` (deps, paths, or hashes). This can happen after editing dependencies, moving workspaces, or upgrading the toolchain.

Agent strategy:

- Regenerate with `x07 pkg lock --project x07.json`.
- If your project uses workspace-relative paths, set `X07_WORKSPACE_ROOT=...` when locking.


## `X07PKG_TREE_MODULE_ROOTS`

Summary: pkg.tree could not compute resolved module roots.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The dependency closure was loaded, but module root resolution failed (usually due to invalid dependency paths or workspace path resolution).

Agent strategy:

- Validate `module_roots[]` and `dependencies[].path` in `x07.json` are valid relative paths.
- If you use workspace paths, ensure `X07_WORKSPACE_ROOT` is set consistently.
- Re-run `x07 pkg lock --project x07.json`, then `x07 pkg tree`.


## `X07PKG_TREE_PROJECT`

Summary: pkg.tree failed to load the project manifest.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The project manifest (usually `x07.json`) is missing, unreadable, or schema-invalid. `x07 pkg tree` cannot compute a dependency closure without a valid manifest.

Agent strategy:

- Fix JSON parse/schema errors in the project manifest.
- If this is an older manifest line, run `x07 project migrate --project x07.json --write`.
- Re-run `x07 pkg tree --project x07.json`.


## `X07PKG_TREE_REQUIRES_INVALID`

Summary: pkg.tree found an invalid `meta.requires_packages` entry.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

A package manifest contains an invalid `meta.requires_packages` entry. Entries must be strings in `NAME@VERSION` form (for example: `"b@1.0.0"`).

Agent strategy:

- Fix the package `x07-package.json` so `meta.requires_packages` is an array of `NAME@VERSION` strings.
- Re-run `x07 pkg lock` (to refresh closure) and then `x07 pkg tree`.


## `X07PKG_UNSIGNED`

Summary: Package or index is unsigned.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

`x07 pkg verify` could not verify the package because the index does not advertise signing keys or the entry does not contain a signature.

Agent strategy:

- If you require integrity, use a registry/index that publishes signing keys and per-entry signatures.
- If the package is expected to be signed, verify the registry configuration and republish if needed.
- Otherwise, treat the package as unsigned and decide whether it is acceptable for your threat model.


## `X07PKG_X07C_COMPAT_INVALID`

Summary: Package compatibility metadata is invalid.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package manifest declares `meta.x07c_compat`, but it is not a valid semver requirement string.

Agent strategy:

- Fix `x07-package.json` so `meta.x07c_compat` is a semver range (example: `">=0.1.111 <0.2.0"`).
- If you do not control the package, pin a different version that carries valid metadata.
- Re-run `x07 pkg lock`.


## `X07PKG_X07C_INCOMPATIBLE`

Summary: Package is incompatible with the current compiler.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package `meta.x07c_compat` range excludes the running `x07c` version. Select a compatible package version or change toolchain versions to satisfy the declared range.

Agent strategy:

- Check the package `meta.x07c_compat` range and your toolchain version.
- Upgrade/downgrade the toolchain or pin a package version that matches the range.
- Re-run `x07 pkg lock` and tests.


## `X07PKG_YANKED_DEP`

Summary: Package workflow diagnostic `X07PKG_YANKED_DEP`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The package/index/lock workflow needs deterministic command-level remediation (dependency spec, lock refresh, credentials/config, or index selection).

Agent strategy:

- Normalize dependency specs and run `x07 pkg lock`.
- Use `x07 pkg add/remove/versions/login/publish` as needed.
- Re-run the original package command.


## `X07PROOF_ECHECK_FAILED`

Summary: Independent proof check rejected the proof bundle.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proof checker found a digest mismatch or missing bundled artifact while replaying the emitted proof object against its proof-summary, SMT obligation, and solver transcript evidence.

Agent strategy:

- Keep the proof object together with its emitted `verify.proof-summary.json`, `verify.smt2`, and `z3.out.txt` bundle files.
- Re-run `x07 verify --prove --emit-proof <path>` if any bundle artifact was edited or lost.
- Re-run `x07 prove check --proof <path>` and confirm the report returns `result=accepted`.


## `X07PROOF_EIMPORTED_SUMMARY_MISMATCH`

Summary: Semantic proof replay found imported proof-summary drift.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proof checker could not validate the bundled imported proof-summary artifacts against the digests recorded in the proof object or the replayed reachable summary surface.

Agent strategy:

- Keep the bundled imported proof-summary artifacts exactly as emitted by the prove flow.
- Re-run `x07 verify --prove --proof-summary <path> --emit-proof <path>` if any imported proof summary changed.
- Re-run `x07 prove check --proof <path>`.


## `X07PROOF_EOBJECT_INVALID`

Summary: Proof object failed schema or structural validation.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proof checker read a `x07.verify.proof_object@0.2.0` file that was malformed, schema-invalid, or otherwise could not be decoded as a valid proof object.

Agent strategy:

- Re-run `x07 verify --prove --emit-proof <path>` to regenerate the proof object.
- Ensure the proof object file is the exact emitted artifact, not a coverage summary or edited JSON.
- Re-run `x07 prove check --proof <path>`.


## `X07PROOF_EOBLIGATION_MISMATCH`

Summary: Semantic proof replay regenerated a different verification obligation.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proof checker successfully replayed the source surface but the regenerated SMT obligation digest did not match the obligation recorded in the proof object.

Agent strategy:

- Treat this as source/proof drift: re-run `x07 verify --prove --emit-proof <path>` on the current source tree.
- Keep imported proof summaries, prove bounds, and project manifest state aligned with the emitted proof bundle.
- Re-run `x07 prove check --proof <path>` until it returns `result=accepted`.


## `X07PROOF_ESCHEDULER_MODEL_MISMATCH`

Summary: Semantic async proof replay found a scheduler-model mismatch.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proof checker replayed an async proof but the trusted scheduler-model digest did not match the one recorded in the proof object.

Agent strategy:

- Use the canonical checked-in scheduler model from the same toolchain release that emitted the proof object.
- Re-run `x07 verify --prove --emit-proof <path>` for the async symbol.
- Re-run `x07 prove check --proof <path>`.


## `X07PROOF_ESOLVER_REPLAY_FAILED`

Summary: Semantic proof replay did not reproduce the expected solver verdict.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proof checker regenerated the obligation but solver replay failed or returned a different result than the proof object expected.

Agent strategy:

- Ensure `z3` is installed and reachable in the replay environment.
- Re-run `x07 verify --prove --emit-proof <path>` from the current source tree if the obligation or imported proof-summary inputs changed.
- Re-run `x07 prove check --proof <path>` and confirm the report returns `result=accepted`.


## `X07PROOF_ESOURCE_REPLAY_FAILED`

Summary: Semantic proof replay could not reconstruct the proved source surface.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proof checker could not replay the proof against the current project manifest, resolved declaration graph, or bundled proof-summary inputs, so it rejected the proof before obligation comparison.

Agent strategy:

- Run `x07 prove check --proof <path>` from the same project tree that emitted the proof object.
- Keep the emitted proof bundle together with the matching `x07.json` state and bundled proof-summary inputs.
- Re-run `x07 verify --prove --emit-proof <path>` if the source or manifest changed.


## `X07RD_ADVISORY_ALLOWANCE_ENABLED`

Summary: Core lint/schema diagnostic `X07RD_ADVISORY_ALLOWANCE_ENABLED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07RD_ASSUMPTION_SURFACE_WIDEN`

Summary: Review diff gate rejected a wider proof-assumption surface.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a review decision or a narrower assumption surface.

Details:

The candidate introduces new proof assumptions or widens the accepted assumption classes relative to the baseline certificate posture.

Agent strategy:

- Inspect `x07 review diff --json-out ...` and the `assumption_changes` highlights.
- Remove the new assumption surface or intentionally reset the review baseline after review.
- Re-run `x07 review diff --fail-on assumption-surface-widen`.


## `X07RD_ASYNC_PROOF_COVERAGE_DECREASE`

Summary: Review diff gate rejected an async proof coverage regression.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a review decision or proof-surface repair.

Details:

The candidate change reduced proven async coverage or introduced newly uncovered/unsupported async proof surface relative to the baseline.

Agent strategy:

- Inspect `x07 review diff --json-out ...` and the `async_proof_changes` highlight set.
- Restore the lost async proof coverage or intentionally reset the baseline after review.
- Re-run `x07 review diff --fail-on async-proof-coverage-decrease`.


## `X07RD_BOUNDED_PROOF_INTRODUCED`

Summary: Review diff gate rejected bounded proof usage.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires proof-surface repair or an explicit review reset.

Details:

The candidate begins depending on bounded recursion proof evidence in accepted certificate posture.

Agent strategy:

- Inspect `x07 review diff --json-out ...` and the `boundedness_changes` highlights.
- Remove bounded recursion from the accepted proof closure or move that code out of the strong profile.
- Re-run `x07 review diff --fail-on bounded-proof-introduced`.


## `X07RD_CAPSULE_CONTRACT_RELAXATION`

Summary: Review diff gate rejected a capsule contract relaxation.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a baseline review decision or a semantic capsule-contract change.

Details:

The candidate weakened a certified capsule contract relative to the reviewed baseline.

Agent strategy:

- Inspect the `capsule_changes` highlights in the review report.
- Restore the stricter capsule contract or intentionally update the review baseline.
- Re-run `x07 review diff --fail-on capsule-contract-relaxation`.


## `X07RD_CAPSULE_NETWORK_SURFACE_WIDEN`

Summary: Core lint/schema diagnostic `X07RD_CAPSULE_NETWORK_SURFACE_WIDEN`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07RD_CAPSULE_SET_CHANGE`

Summary: Review diff gate rejected a capsule inventory change.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a baseline review decision or capsule inventory change.

Details:

The candidate added, removed, or reordered certified capsule membership relative to the baseline review set.

Agent strategy:

- Inspect the capsule index diff and confirm whether the inventory change is intentional.
- Restore the baseline capsule set or intentionally update the review baseline.
- Re-run `x07 review diff --fail-on capsule-set-change`.


## `X07RD_COVERAGE_SUMMARY_IMPORTED`

Summary: Review diff gate rejected a coverage-only import.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires proof evidence, not a coverage-only summary.

Details:

The candidate started importing a coverage/support summary where proof evidence is required.

Agent strategy:

- Inspect the `assumption_changes` and `summary_changes` highlights in the review report.
- Replace the coverage/support import with a proof-summary import or a local prove run.
- Re-run `x07 review diff --fail-on coverage-summary-imported`.


## `X07RD_DEV_ONLY_ASSUMPTION_INTRODUCED`

Summary: Review diff gate rejected a developer-only proof assumption.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires proof-surface repair, not a mechanical patch.

Details:

The candidate begins depending on a non-certifiable proof assumption such as an imported stub in accepted certificate posture.

Agent strategy:

- Inspect the `assumption_changes` highlights in the review report.
- Remove the developer-only assumption or move the flow out of the strong profile.
- Re-run `x07 review diff --fail-on dev-only-assumption-introduced`.


## `X07RD_LOCKFILE_HASH_CHANGE`

Summary: Core lint/schema diagnostic `X07RD_LOCKFILE_HASH_CHANGE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07RD_NETWORK_ALLOWLIST_WIDEN`

Summary: Core lint/schema diagnostic `X07RD_NETWORK_ALLOWLIST_WIDEN`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07RD_OPERATIONAL_ENTRY_DIVERGES`

Summary: Review diff gate rejected operational-entry divergence.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires an explicit certification-entry decision.

Details:

The candidate changed the certified operational entry or caused `--entry` and `project.operational_entry_symbol` to diverge.

Agent strategy:

- Inspect `x07 review diff --json-out ...` and the `operational_entry_changes` highlights.
- Realign the manifest and certification command to the shipped operational entry.
- Re-run `x07 review diff --fail-on operational-entry-diverges`.


## `X07RD_PACKAGE_SET_CHANGE`

Summary: Core lint/schema diagnostic `X07RD_PACKAGE_SET_CHANGE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07RD_PEER_POLICY_RELAXATION`

Summary: Core lint/schema diagnostic `X07RD_PEER_POLICY_RELAXATION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07RD_RECURSION_PROOF_COVERAGE_DECREASE`

Summary: Review diff gate rejected a recursive proof coverage regression.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a review decision or recursive proof-surface repair.

Details:

The candidate reduced proved recursive coverage or introduced newly unsupported recursive proof surface relative to the reviewed baseline.

Agent strategy:

- Inspect `x07 review diff --json-out ...` and the `recursive_proof_changes` highlight set.
- Restore the lost recursive proof coverage or intentionally reset the review baseline.
- Re-run `x07 review diff --fail-on recursion-proof-coverage-decrease`.


## `X07RD_RUNTIME_ATTEST_REGRESSION`

Summary: Review diff gate rejected a runtime attestation regression.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a baseline review decision or runtime-evidence repair.

Details:

The candidate changed runtime attestation posture relative to the baseline in a way the requested review gate forbids.

Agent strategy:

- Inspect the `runtime_attestation_changes` highlights in the review report.
- Restore the baseline attestation posture or intentionally update the review baseline.
- Re-run `x07 review diff --fail-on runtime-attestation-regression`.


## `X07RD_SANDBOX_POLICY_WIDEN`

Summary: Review diff gate rejected a sandbox policy widening.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a baseline review decision or runtime-policy tightening.

Details:

The candidate widened the sandbox runtime posture, including enabling weaker isolation or otherwise relaxing the reviewed sandbox policy.

Agent strategy:

- Inspect the `sandbox_policy_changes` highlights in the review report.
- Restore the stricter sandbox policy or intentionally update the review baseline.
- Re-run the requested `x07 review diff --fail-on ...` gate.


## `X07RD_SUMMARY_DOWNGRADE`

Summary: Review diff gate rejected a proof-summary downgrade.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a review decision or a stronger summary posture.

Details:

The candidate weakened reachable proof-summary posture relative to the baseline, such as dropping prove-supported status or degrading a reviewed proof status.

Agent strategy:

- Inspect the `summary_changes` highlights in the review report.
- Restore the stronger proof-summary posture or intentionally reset the review baseline.
- Re-run `x07 review diff --fail-on summary-downgrade`.


## `X07REL_EBOUNDED_PROOF`

Summary: Release guard rejected bounded proof usage in the strict fixture.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The strict verified-core fixture accepted a certificate that still depended on bounded proof evidence, which the release guard forbids for the strong internal baseline.

Agent strategy:

- Remove the bounded-proof dependency from the strict fixture certificate path.
- Re-run `scripts/ci/check_verified_core_fixture.sh` and confirm the accepted certificate no longer marks bounded proof.
- Re-run the release-readiness CI gate.


## `X07REL_ECOVERAGE_ONLY_IMPORT`

Summary: Release guard rejected a coverage-only imported summary in the strict fixture.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The strict verified-core fixture imported coverage-only summary evidence where the release guard requires checked proof evidence.

Agent strategy:

- Replace the imported coverage-only summary with per-symbol prove artifacts and checked proof evidence.
- Re-run `scripts/ci/check_verified_core_fixture.sh` and confirm the certificate proof inventory is strict.
- Re-run the release-readiness CI gate.


## `X07REL_EDEV_ONLY_ASSUMPTION`

Summary: Release guard rejected a developer-only proof assumption in the strict fixture.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The strict verified-core fixture accepted certificate evidence that still depended on developer-only assumptions such as imported stubs.

Agent strategy:

- Remove the developer-only assumption from the strict fixture proof surface.
- Re-run `scripts/ci/check_verified_core_fixture.sh` and confirm the accepted certificate has no developer-only assumptions.
- Re-run the release-readiness CI gate.


## `X07REL_ESURROGATE_ENTRY`

Summary: Release guard rejected surrogate-entry certification in the strict fixture.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The strict verified-core fixture accepted certification for an entry that diverged from `project.operational_entry_symbol`, which the release guard forbids.

Agent strategy:

- Keep the certification entry aligned with `project.operational_entry_symbol`.
- Re-run `scripts/ci/check_verified_core_fixture.sh` and confirm surrogate-entry rejection still fires.
- Re-run the release-readiness CI gate.


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


## `X07TC_EARCH_STRICT`

Summary: Strict architecture evidence failed certification.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

Certification requires the arch manifest and arch check result to stay at the verified-core posture.

Agent strategy:

- Run `x07 arch check --manifest arch/manifest.x07arch.json` and inspect the report.
- Apply the deterministic manifest JSON Patch scaffolding to align allowlist mode, cycle/orphan/visibility/world-cap checks, and trust-zone boundary wiring with `verified_core_pure_v1`.
- Re-run `x07 trust certify`.


## `X07TC_EASYNC_PROOF`

Summary: Async proof coverage or async proof obligations failed certification.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires proof-focused contract work or semantic refactoring rather than a deterministic quickfix.

Details:

At least one reachable `defasync` symbol is uncovered, hit an async counterexample, failed a protocol clause, or could not be proven with the trusted scheduler model.

Agent strategy:

- Run `x07 verify --coverage --entry <entry> --project x07.json` and inspect the async summary.
- Re-run `x07 verify --prove --entry <defasync>` for the failing symbol and fix `await_invariant`, `scope_invariant`, or `cancellation_ensures` as needed.
- Re-run `x07 trust certify`.


## `X07TC_EASYNC_PROVE_REPORT_MISSING`

Summary: Required async prove report is missing.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Certification expected a per-symbol async prove report artifact for a reachable `defasync` symbol, but the report file was missing or the proof run did not emit the expected artifact path.

Agent strategy:

- Re-run `x07 verify --prove` for the reported async symbol and keep the emitted prove report under the certificate out-dir.
- Ensure the proof run emits `verify_proof_summary_path` in the prove report.
- Re-run `x07 trust certify`.


## `X07TC_EBOUNDARY_MISSING`

Summary: Boundary declarations are missing or incomplete for certification.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

Public exports must appear in `arch/boundaries/index.x07boundary.json`, and certification requires the boundary index wiring to resolve cleanly.

Agent strategy:

- Run `x07 arch check` and inspect `target/cert/boundaries.report.json`.
- Apply the deterministic boundary-index JSON Patch or use `x07 schema derive --emit-boundary-stub` to scaffold missing records.
- Re-run `x07 trust certify`.


## `X07TC_EBOUNDARY_RELAXED`

Summary: The candidate relaxes a certified boundary contract relative to the baseline.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a baseline review decision or a semantic boundary-contract change.

Details:

Boundary contract review found a trust-surface relaxation in a pinned public boundary.

Agent strategy:

- Run `x07 review diff --fail-on boundary-relaxation` and inspect the highlighted boundary changes.
- Tighten the candidate boundary contract or explicitly reset the baseline after review.
- Re-run `x07 trust certify` with the corrected baseline.


## `X07TC_EBOUNDED_PROOF_FORBIDDEN`

Summary: Bounded recursion proof is forbidden for the active trust profile.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires proof-surface repair or profile scope changes.

Details:

Accepted strong certificates cannot depend on bounded-by-unwind recursion proof evidence.

Agent strategy:

- Inspect the emitted proof summaries and certificate recursive-proof fields.
- Refactor the recursive symbol out of the strong certified closure or move it behind a non-strong flow.
- Re-run `x07 trust certify`.


## `X07TC_ECAPSULE_ATTEST`

Summary: Core lint/schema diagnostic `X07TC_ECAPSULE_ATTEST`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TC_ECAPSULE_NETWORK_ATTEST`

Summary: Core lint/schema diagnostic `X07TC_ECAPSULE_NETWORK_ATTEST`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TC_ECOMPILE_ATTEST`

Summary: Compile attestation failed or could not bind the emitted binary.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Depends on host toolchain state and reproducible-build behavior outside AST quickfixes.

Details:

The bundle step failed, no executable was produced, or the double-build digest did not match.

Agent strategy:

- Run `x07 doctor` and ensure a working C toolchain is available.
- Re-run `x07 bundle --project x07.json --emit-attestation target/cert/compile.attest.json`.
- Fix reproducibility or toolchain drift before certifying again.


## `X07TC_ECOVERAGE_ONLY`

Summary: Coverage/support evidence cannot satisfy certification.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires prove evidence, not a mechanical patch.

Details:

A reachable symbol or imported dependency is covered by support posture only, without the prove artifacts required by the active trust profile.

Agent strategy:

- Run `x07 verify --prove` for the reachable symbol set required by the active profile.
- Replace any imported coverage/support summary with a proof summary emitted by a successful prove run.
- Re-run `x07 trust certify`.


## `X07TC_EDEP_CLOSURE`

Summary: Core lint/schema diagnostic `X07TC_EDEP_CLOSURE`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TC_EDEV_ONLY_ASSUMPTION`

Summary: Developer-only proof assumptions are forbidden for the active trust profile.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires proof-surface repair or dependency changes.

Details:

The accepted proof inventory depends on a non-certifiable assumption such as an `imported_stub` primitive.

Agent strategy:

- Re-run prove without `--allow-imported-stubs`.
- Replace the stubbed dependency with certifiable code or a reviewed proof-summary path.
- Re-run `x07 trust certify`.


## `X07TC_EDIFF_POSTURE`

Summary: Baseline review or trust-posture diff gate failed certification.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a baseline review decision or a semantic change to the candidate.

Details:

The supplied baseline comparison failed, or the candidate introduced a forbidden trust posture delta.

Agent strategy:

- Run `x07 review diff --fail-on proof-coverage-decrease|boundary-relaxation|trusted-subset-expansion`.
- Tighten the candidate change or intentionally update the baseline.
- Re-run `x07 trust certify` with the corrected baseline.


## `X07TC_EEFFECT_LOG`

Summary: Core lint/schema diagnostic `X07TC_EEFFECT_LOG`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TC_ENET_POLICY`

Summary: Network policy posture changed relative to the reviewed baseline.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires a baseline review decision or a semantic policy change.

Details:

Certification rejected the candidate because the review diff reported a network allowlist or policy-surface change that must be re-reviewed before shipping.

Agent strategy:

- Run `x07 review diff --fail-on network-allowlist-widen` and inspect the highlighted policy delta.
- Tighten the candidate policy or intentionally refresh the reviewed baseline.
- Re-run `x07 trust certify` with the corrected baseline.


## `X07TC_ENONDET`

Summary: Trust report detected nondeterminism in the certified closure.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires capability removal or architectural refactoring rather than a deterministic quickfix.

Details:

The trust report found nondeterminism flags or other forbidden runtime effects in the candidate certificate surface.

Agent strategy:

- Run `x07 trust report --project x07.json` and inspect the nondeterminism flags.
- Split the nondeterministic logic out of the certified closure or remove the forbidden capability.
- Re-run `x07 trust certify`.


## `X07TC_EOP_ENTRY_MISMATCH`

Summary: Certification entry does not match the operational entry.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

Strong trust profiles require `--entry` to match `project.operational_entry_symbol` exactly.

Agent strategy:

- Inspect `x07.json` and the `x07 trust certify --entry ...` argument.
- Realign the certification command or manifest so the shipped operational entry is certified directly.
- Re-run `x07 trust certify`.


## `X07TC_EOP_ENTRY_REQUIRED`

Summary: Operational-entry metadata is required for the active trust profile.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

Strong trust profiles require `project.operational_entry_symbol` in `x07.project@0.5.0` manifests (or legacy `0.4.0`) so the shipped entry is explicit.

Agent strategy:

- Add `project.operational_entry_symbol` to `x07.json`.
- If the project still needs a helper certification symbol, keep it separate with `project.certification_entry_symbol` and certify only non-strong flows against it.
- Re-run `x07 trust certify`.


## `X07TC_EPBT`

Summary: Boundary-required property tests are missing, malformed, or failing.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

A required PBT harness is absent from `tests/tests.json`, is not declared with a `pbt` stanza, uses the wrong world, or did not pass.

Agent strategy:

- Add the missing test id under `tests/tests.json` with the required `pbt` stanza using the deterministic JSON Patch scaffolding.
- Keep the test world inside the profile/boundary allowlist.
- Re-run `x07 test --all` and then `x07 trust certify`.


## `X07TC_EPEER_POLICY`

Summary: Core lint/schema diagnostic `X07TC_EPEER_POLICY`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TC_EPROFILE`

Summary: Certification trust profile is missing or invalid.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The profile supplied to `x07 trust certify` could not be parsed or validated.

Agent strategy:

- Validate the profile with `x07 trust profile check` first.
- Fix schema_version, required fields, or file path issues.
- Re-run `x07 trust certify`.


## `X07TC_EPROJECT`

Summary: Certification could not resolve the project manifest or source closure.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The project path is missing, not a project manifest, or failed project-context resolution before evidence collection completed.

Agent strategy:

- Pass `--project x07.json` (or a directory containing it).
- Ensure the project manifest and lockfile resolve cleanly.
- Re-run `x07 trust certify`.


## `X07TC_EPROOF_CHECK_ENGINE_MISMATCH`

Summary: Certification found a proof-check report whose engine did not match the proved symbol metadata.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The proof-check report replayed a different verification engine than the one recorded in the proof-summary evidence for that symbol, so the certification flow rejected it.

Agent strategy:

- Re-run `x07 verify --prove --emit-proof <path>` and `x07 prove check --proof <path>` from the same toolchain build.
- Keep proof summaries, proof objects, and proof-check reports from the same prove run together.
- Re-run `x07 trust certify`.


## `X07TC_EPROOF_CHECK_MISSING`

Summary: Required proof-check report is missing from prove evidence.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires regenerated proof artifacts, not a source-only patch.

Details:

The active trust profile requires an independent proof-check report for each accepted proof object.

Agent strategy:

- Run `x07 prove check --proof <path>` for the missing proof object or rerun the prove flow that emits the check report.
- Ensure the per-symbol prove report references both the proof object and the proof-check report.
- Re-run `x07 trust certify`.


## `X07TC_EPROOF_CHECK_REJECTED`

Summary: Certification rejected a proof whose proof-check report was not accepted.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The loaded proof-check report had `ok=false` or `result!=="accepted"`, so the certification flow failed closed instead of treating the proof as independently replayed evidence.

Agent strategy:

- Inspect the proof-check report diagnostics and repair the source/proof inputs until `x07 prove check` returns `result=accepted`.
- Re-run the prove flow if the source or imported proof-summary surface changed.
- Re-run `x07 trust certify`.


## `X07TC_EPROOF_CHECK_SCHEMA_INVALID`

Summary: Proof-check report failed schema validation during certification.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Certification found a malformed, unreadable, or schema-invalid `x07.verify.proof_check.report@0.2.0` artifact where an accepted proof-check report was required.

Agent strategy:

- Re-run `x07 prove check --proof <path>` to regenerate the proof-check report.
- Ensure the certificate flow is consuming the emitted report rather than an edited JSON file.
- Re-run `x07 trust certify`.


## `X07TC_EPROOF_COVERAGE`

Summary: Reachable proof coverage is incomplete for certification.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires semantic proof work or architectural refactoring, not a mechanical patch.

Details:

Accepted certificates allow only `proven` or `trusted_primitive` statuses in the reachable closure.

Agent strategy:

- Run `x07 verify --coverage --entry <entry> --project x07.json` and inspect `target/cert/verify.coverage.json`.
- Move unsupported logic behind certified boundaries or refactor it into the certifiable subset.
- Re-run `x07 verify --prove` for uncovered symbols before certifying again.


## `X07TC_EPROOF_OBJECT_MISSING`

Summary: Required proof object is missing from prove evidence.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires regenerated proof artifacts, not a source-only patch.

Details:

The active trust profile requires a proof object for each accepted prove inventory item, but a referenced prove report did not bind one.

Agent strategy:

- Re-run `x07 verify --prove --emit-proof <path>` for the missing symbol.
- Run `x07 prove check --proof <path>` so the proof-check report exists alongside the proof object.
- Re-run `x07 trust certify`.


## `X07TC_EPROVE`

Summary: At least one reachable proof obligation failed.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

A reachable symbol failed `x07 verify --prove` or returned a non-`proven` result.

Agent strategy:

- Re-run `x07 verify --prove` for the reported symbol.
- Add missing loop contracts, strengthen `requires[]`/`ensures[]`, or simplify the function into the supported subset.
- Re-run `x07 trust certify`.


## `X07TC_EPROVE_REPORT_MISSING`

Summary: Required per-symbol prove report is missing.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Certification expected a per-symbol prove report artifact for a reachable `defn` symbol, but the report file was missing or the proof run did not emit the expected artifact path.

Agent strategy:

- Re-run `x07 verify --prove` for the reported symbol and keep the emitted prove report under the certificate out-dir.
- Ensure the proof run emits `verify_proof_summary_path` in the prove report.
- Re-run `x07 trust certify`.


## `X07TC_EPROVE_UNSUPPORTED`

Summary: A reachable symbol is outside the supported proof subset.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

At least one reachable symbol uses a construct the prover cannot certify yet (for example unsupported params, unsupported loop bounds, or unresolved non-certified imports).

Agent strategy:

- Re-run `x07 verify --prove` for the reported symbol and inspect the verify diagnostics.
- Add the missing contracts, loop skeleton, or wrapper function needed to stay in the supported subset.
- Re-run `x07 trust certify`.


## `X07TC_ERECURSION_FORBIDDEN`

Summary: Reachable recursion is forbidden by the active trust profile.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires semantic refactoring or an intentional profile change.

Details:

The reachable closure still contains recursive symbols, but the active trust profile sets `language_subset.allow_recursion=false` and therefore fails closed even when recursion would otherwise be supported or imported as reviewed proof.

Agent strategy:

- Refactor recursive helpers out of the certified closure or choose a profile that explicitly allows recursion.
- Re-run `x07 verify --coverage --entry <entry>` and inspect the recursive symbols in the closure.
- Re-run `x07 trust certify` after the certified entry is recursion-free.


## `X07TC_ERUNTIME_ATTEST`

Summary: Core lint/schema diagnostic `X07TC_ERUNTIME_ATTEST`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TC_ERUNTIME_NETWORK_EVIDENCE`

Summary: Core lint/schema diagnostic `X07TC_ERUNTIME_NETWORK_EVIDENCE`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TC_ESANDBOX_PROFILE`

Summary: Sandbox runtime evidence violates the certification profile.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires changing runtime policy or execution posture, not a deterministic AST patch.

Details:

The runtime evidence used for certification reports a sandbox backend, weaker-isolation posture, or network posture that is weaker than the selected trusted-program profile allows.

Agent strategy:

- Keep the project on `run-os-sandboxed` with `sandbox_backend=vm`.
- Disable weaker-isolation opt-ins and keep `policy.net.enabled=false` for the local sandboxed trust profile.
- Re-run the sandbox smoke tests and `x07 trust certify`.


## `X07TC_ESCHEMA_DRIFT`

Summary: Boundary-referenced schema outputs drifted or are missing.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Certification rechecks pinned boundary schemas with `x07 schema derive --check`; missing inputs or drifted generated outputs reject the certificate.

Agent strategy:

- Run `x07 schema derive --input <schema> --out-dir . --write` for each boundary schema.
- If the boundary record is missing, use `x07 schema derive --emit-boundary-stub` to scaffold it.
- Re-run `x07 trust certify`.


## `X07TC_ESURROGATE_ENTRY_FORBIDDEN`

Summary: Surrogate certification entries are forbidden for the active trust profile.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

The selected strong profile requires certifying the shipped operational entry directly, not a separate proof-friendly surrogate entry.

Agent strategy:

- Inspect `project.operational_entry_symbol`, optional `project.certification_entry_symbol`, and the `--entry` argument.
- Certify the operational entry directly or move the surrogate flow to a non-strong developer check.
- Re-run `x07 trust certify`.


## `X07TC_ETESTS`

Summary: Deterministic certification tests are missing or failing.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

The certification profile requires smoke/unit evidence from `tests/tests.json`, but the manifest is missing, miswired, or the report contains failures.

Agent strategy:

- Ensure `tests/tests.json` exists and includes the boundary-required test ids.
- Apply the deterministic manifest JSON Patch when adding missing smoke/unit test declarations.
- Keep the test worlds inside the trust profile allowlist.
- Re-run `x07 test --all --manifest tests/tests.json` before certifying again.


## `X07TC_ETRUST_REPORT`

Summary: Trust report evidence failed certification.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `never`
No quickfix reason: Requires capability or environment changes outside JSON quickfix application.

Details:

The trust report found disallowed nondeterminism/capabilities, or failed to emit the required SBOM evidence.

Agent strategy:

- Run `x07 trust report --project x07.json --out target/trust/trust.json --html-out target/trust/trust.html`.
- Remove the disallowed capability surface or fix the missing SBOM/trust artifact.
- Re-run `x07 trust certify`.


## `X07TC_EUNSUPPORTED_DEFASYNC`

Summary: Reachable `defasync` logic is outside the certifiable subset.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The reachable closure contains runtime-only `defasync` symbols, which are forbidden by `verified_core_pure_v1`.

Agent strategy:

- Split async logic behind a certified boundary and keep the certified entry closure `defn`-only.
- Add a pure wrapper if needed.
- Re-run `x07 verify --coverage` and `x07 trust certify`.


## `X07TC_EUNSUPPORTED_RECURSION`

Summary: Reachable recursion is outside the certifiable subset.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The reachable closure contains recursive logic, which `verified_core_pure_v1` does not certify.

Agent strategy:

- Refactor recursion into a bounded iterative form and add loop contracts where needed.
- Re-run `x07 verify --coverage` and `x07 trust certify`.


## `X07TEST_ASYNC_ENTRY_UNSUPPORTED`

Summary: x07 test could not build or run the selected async entrypoint.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires fixing the selected entrypoint or its supported test return shape.

Details:

The manifest selected an async entry, but the test harness could not lower it into a supported async driver or return-shape combination.

Agent strategy:

- Keep async test entries on the supported `bytes_status_v1` or `result_i32` driver forms.
- Re-run `x07 test --manifest tests/tests.json` after fixing the entry declaration.
- If the entry is not meant to be async, point the manifest at a `defn` wrapper.


## `X07TEST_CAPSULE_EVIDENCE_MISSING`

Summary: A test that requires capsule evidence did not provide it.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires fixing emitted sandbox evidence or the declared capsule inventory.

Details:

The manifest declared required capsules, but the emitted test evidence did not include matching capsule ids or effect-log digests.

Agent strategy:

- Keep the test manifest `required_capsules[]` aligned with the sandboxed boundary surface.
- Ensure the sandbox execution emits capsule/effect-log evidence into the test report.
- Re-run `x07 test`.


## `X07TEST_RUNTIME_ATTEST_REQUIRED`

Summary: A test that requires runtime attestation did not produce it.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The manifest marked the test as requiring runtime attestation, but the run report did not include a runtime attestation reference or the referenced file was missing.

Agent strategy:

- Keep the test on `run-os-sandboxed`.
- Ensure the harness passes `--attest-runtime` through to `x07-os-runner`.
- Re-run `x07 test` and inspect the emitted runtime-attest artifact path.


## `X07TP_ARCH`

Summary: Project architecture posture is weaker than the trust profile requires.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

The arch manifest is missing, on the wrong schema line, or does not enable the strict checks required for certification.

Agent strategy:

- Start from the `verified_core_pure_v1` manifest posture.
- Apply the deterministic manifest JSON Patch scaffolding to enable allowlist mode, cycle/orphan/visibility/world-cap checks, and trust-zone boundary contracts.
- Re-run `x07 trust profile check`.


## `X07TP_ASYNC_PROOF_REQUIRED`

Summary: Sandboxed trusted-program profile is missing async proof coverage.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The sandboxed local trusted-program contract requires async proof coverage for the reviewed closure.

Agent strategy:

- Set `evidence_requirements.require_async_proof_coverage=true`.
- Keep the sandboxed profile aligned with `arch/trust/profiles/trusted_program_sandboxed_local_v1.json`.
- Re-run `x07 trust profile check`.


## `X07TP_BACKEND_NOT_CERTIFIABLE`

Summary: Networked trusted-program profile allows a backend posture that is not certifiable.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

Networked certification requires VM-backed sandboxing, forbids weaker isolation, and rejects project worlds outside the certifiable sandbox line.

Agent strategy:

- Inspect the selected trust profile JSON and backend requirements.
- Restore VM-backed sandboxing and the certifiable world restrictions.
- Re-run `x07 trust profile check`.


## `X07TP_BOUNDARY`

Summary: Project boundary index wiring is missing or incomplete for certification.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`
Quickfix kinds: `json_patch`

Details:

The trust profile requires `contracts_v1.boundaries.index_path`, but the project does not expose a boundary index for the public trust surface.

Agent strategy:

- Add `contracts_v1.boundaries.index_path` to `arch/manifest.x07arch.json` with the deterministic manifest JSON Patch.
- Create `arch/boundaries/index.x07boundary.json` and keep public boundaries there.
- Use `x07 schema derive --emit-boundary-stub` when you need deterministic boundary scaffolding.


## `X07TP_CAPSULES`

Summary: Core lint/schema diagnostic `X07TP_CAPSULES`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TP_CAPSULE_ATTEST_REQUIRED`

Summary: Sandboxed trusted-program profile is missing capsule attestation requirements.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The sandboxed local trusted-program contract must require capsule attestations for effect boundaries.

Agent strategy:

- Set `evidence_requirements.require_capsule_attestations=true`.
- Keep the sandboxed profile aligned with `arch/trust/profiles/trusted_program_sandboxed_local_v1.json`.
- Re-run `x07 trust profile check`.


## `X07TP_DEP_CLOSURE_REQUIRED`

Summary: Core lint/schema diagnostic `X07TP_DEP_CLOSURE_REQUIRED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TP_EFFECT_LOG_REQUIRED`

Summary: Trust profile is missing required effect-log evidence semantics.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The selected trust profile must require effect-log digests for certified capsule or sandboxed trusted-program evidence, but the profile leaves that requirement disabled.

Agent strategy:

- Set `evidence_requirements.require_effect_log_digests=true`.
- Keep the published trust profile aligned with the certified capsule or sandboxed trusted-program contract.
- Re-run `x07 trust profile check`.


## `X07TP_ENTRY_FORBIDDEN`

Summary: Requested entrypoint is not allowed by the trust profile.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The profile's `entrypoints[]` set does not permit the entry you asked to certify.

Agent strategy:

- Either certify one of the profile's declared entrypoints or intentionally widen the profile.
- Keep `entrypoints[]` aligned with the reviewed trust surface.
- Re-run `x07 trust profile check`.


## `X07TP_INVALID`

Summary: Trust profile JSON is missing or invalid.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The trust profile could not be read, parsed, or validated against the certification schema.

Agent strategy:

- Compare the profile against `arch/trust/profiles/verified_core_pure_v1.json`.
- Fix JSON shape, schema_version, and required fields.
- Re-run `x07 trust profile check`.


## `X07TP_LANGUAGE`

Summary: Project uses language or policy features forbidden by the trust profile.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The reachable project surface includes forbidden features such as `defasync`, `extern`, `allow_unsafe`, or `allow_ffi`.

Agent strategy:

- Refactor the certified entry closure into the allowed subset.
- Keep `verified_core_pure_v1` entry closures free of `defasync`, `extern`, `unsafe`, and FFI.
- Re-run `x07 trust profile check`.


## `X07TP_NETWORK_MODE_FORBIDDEN`

Summary: Sandboxed-local trust profile allows networking where it should not.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The sandboxed local trust profile must keep networking disabled for the local certification line.

Agent strategy:

- Inspect the selected trust profile JSON.
- Remove network posture from the local sandbox profile or switch to the networked trust profile intentionally.
- Re-run `x07 trust profile check`.


## `X07TP_NETWORK_PROFILE_REQUIRED`

Summary: Networked trust profile is missing required network certification posture.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The networked trust profile must require allowlist-backed network posture, attested network capsules, and matching runtime network evidence semantics.

Agent strategy:

- Inspect the selected trust profile JSON.
- Restore the required network evidence requirements and sandbox posture.
- Re-run `x07 trust profile check`.


## `X07TP_NOT_CERTIFIABLE`

Summary: Trust profile is weaker than the current certification floor.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The selected trust profile omits required certification posture such as strict operational-entry binding, evidence requirements, or sandbox restrictions needed for accepted source-free review.

Agent strategy:

- Inspect the selected trust profile JSON.
- Restore the missing certification requirements or switch to a non-strong developer/demo flow intentionally.
- Re-run `x07 trust profile check`.


## `X07TP_PEER_POLICY_REQUIRED`

Summary: Core lint/schema diagnostic `X07TP_PEER_POLICY_REQUIRED`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07TP_PROJECT_MISMATCH`

Summary: Project manifest could not be resolved for trust profile validation.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The `--project` path is missing, not a project manifest, or cannot be resolved into a valid X07 project.

Agent strategy:

- Pass `--project x07.json` (or a directory containing it).
- Ensure `x07.json` exists and resolves cleanly.
- Re-run `x07 trust profile check`.


## `X07TP_RUNTIME_ATTEST_REQUIRED`

Summary: Sandboxed trusted-program profile is missing runtime attestation requirements.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The sandboxed local trusted-program contract must require runtime attestation for certified executions.

Agent strategy:

- Set `evidence_requirements.require_runtime_attestation=true`.
- Keep the sandboxed profile aligned with `arch/trust/profiles/trusted_program_sandboxed_local_v1.json`.
- Re-run `x07 trust profile check`.


## `X07TP_SANDBOX_BACKEND_REQUIRED`

Summary: Sandboxed trusted-program profile is missing the VM-only isolation requirement.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The sandboxed local trusted-program contract must require `run-os-sandboxed` with `sandbox_backend=vm` and must forbid weaker isolation.

Agent strategy:

- Keep `worlds_allowed` free of `run-os`.
- Set `sandbox_requirements.sandbox_backend="vm"` and `sandbox_requirements.forbid_weaker_isolation=true`.
- Re-run `x07 trust profile check`.


## `X07TP_WORLD`

Summary: Project world is outside the trust profile allowlist.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The project or selected run profile resolves to a world not listed in `worlds_allowed`.

Agent strategy:

- Move the project to an allowed solve-world (for `verified_core_pure_v1`, `solve-pure`).
- Or select a different trust profile that explicitly permits the current world.
- Re-run `x07 trust profile check`.


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


## `X07V_ASYNC_COUNTEREXAMPLE`

Summary: The async proof found a counterexample.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires understanding and fixing the async proof failure, not applying a deterministic quickfix.

Details:

CBMC produced a concrete async execution trace that violates one of the stated async proof obligations.

Agent strategy:

- Inspect the emitted counterexample JSON and trace.
- Tighten the async protocol clauses or repair the state machine logic.
- Re-run `x07 verify --prove`.


## `X07V_CANCELLATION_ENSURE_FAILED`

Summary: The async cancellation postcondition failed under proof.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires cancellation-path proof work rather than a deterministic quickfix.

Details:

The `cancellation_ensures` clause does not hold on the cancellation path of the proved async function.

Agent strategy:

- Model the cancellation result and cleanup effects explicitly in `cancellation_ensures`.
- Repair any cancellation-path state updates or resource cleanup.
- Re-run `x07 verify --prove`.


## `X07V_COVERAGE_NOT_PROOF`

Summary: Coverage/support summary is not proof evidence.

Origins:
- x07 (stage: lint, severity: warning)

Quickfix support: `sometimes`

Details:

A supplied `x07.verify.summary@0.2.0` artifact describes coverage posture only. It can help review or planning, but it cannot satisfy prove-mode or certification proof requirements.

Agent strategy:

- Replace the coverage/support artifact with `x07.verify.proof_summary@0.2.0` emitted by a successful `x07 verify --prove` run.
- Keep `x07 verify --coverage` outputs for posture review only.
- Re-run the prove flow with `--proof-summary <path>`.


## `X07V_COVERAGE_SUMMARY_FORBIDDEN`

Summary: Coverage/support summaries cannot be imported as proof.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The caller passed `x07.verify.summary@0.2.0` through `--proof-summary`, but prove-mode imports accept only `x07.verify.proof_summary@0.2.0` evidence emitted by successful prove runs.

Agent strategy:

- Remove the coverage/support artifact from `--proof-summary` inputs.
- Re-run the dependency prove flow that emits `verify.proof-summary.json`.
- Retry with the proof-summary artifact instead of the coverage/support summary.


## `X07V_EARGS`

Summary: Core lint/schema diagnostic `X07V_EARGS`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_ECBMC_ERROR`

Summary: Core lint/schema diagnostic `X07V_ECBMC_ERROR`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_ECBMC_FAILURE`

Summary: Core lint/schema diagnostic `X07V_ECBMC_FAILURE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_ECBMC_JSON_PARSE`

Summary: Core lint/schema diagnostic `X07V_ECBMC_JSON_PARSE`.

Origins:
- x07 (stage: parse, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_ECBMC_MISSING`

Summary: Core lint/schema diagnostic `X07V_ECBMC_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_ECBMC_SMT2`

Summary: Core lint/schema diagnostic `X07V_ECBMC_SMT2`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_ECBMC_STDERR`

Summary: Core lint/schema diagnostic `X07V_ECBMC_STDERR`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_EMODULE_ROOTS`

Summary: Core lint/schema diagnostic `X07V_EMODULE_ROOTS`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_EPROJECT`

Summary: Core lint/schema diagnostic `X07V_EPROJECT`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_ETARGET`

Summary: Core lint/schema diagnostic `X07V_ETARGET`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_EZ3_MISSING`

Summary: Core lint/schema diagnostic `X07V_EZ3_MISSING`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_EZ3_RUN`

Summary: Core lint/schema diagnostic `X07V_EZ3_RUN`.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_IMPORTED_STUB_FORBIDDEN`

Summary: Imported stub assumptions are disabled in prove mode.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The requested prove run depends on developer-only `imported_stub` assumptions. Strong certification flows reject those assumptions instead of silently treating them as proof.

Agent strategy:

- Replace the imported stub with a proved or attested implementation on the reachable proof surface.
- If you only need a developer-only exploration run, re-run `x07 verify --prove --allow-imported-stubs` and do not use that result for strong certification.
- Re-run the prove or certification command on the strict surface.


## `X07V_INTERNAL`

Summary: Core lint/schema diagnostic `X07V_INTERNAL`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_NO_CONTRACTS`

Summary: Core lint/schema diagnostic `X07V_NO_CONTRACTS`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_PROOF_SUMMARY_REQUIRED`

Summary: A reachable proof summary dependency is required but missing.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The current prove flow reached a symbol outside the locally loaded graph, and no imported `x07.verify.proof_summary@0.2.0` artifact was supplied for that dependency.

Agent strategy:

- Re-run `x07 verify --prove --entry <sym>` for the reviewed dependency to emit `verify.proof-summary.json`.
- Pass the emitted artifact back via `x07 verify --proof-summary <path>`.
- Re-run the original prove command.


## `X07V_PROVE_UNSUPPORTED`

Summary: Core lint/schema diagnostic `X07V_PROVE_UNSUPPORTED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_RECURSION_TERMINATION_FAILED`

Summary: Recursive proof could not justify the declared termination rank.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The target is directly self-recursive and declares `decreases[]`, but at least one recursive self-call does not obviously decrease the declared rank term in the verifier's certifiable recursive subset.

Agent strategy:

- Keep the first `decreases[].expr` aligned with a recursive parameter.
- Rewrite recursive self-calls so the rank argument decreases by a positive literal step.
- Re-run `x07 verify --prove`.


## `X07V_RECURSIVE_DECREASES_REQUIRED`

Summary: Core lint/schema diagnostic `X07V_RECURSIVE_DECREASES_REQUIRED`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_SCHEDULER_MODEL_UNTRUSTED`

Summary: Async proof cannot proceed without the trusted scheduler model.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The async verifier requires the checked-in deterministic scheduler model to justify the state-machine proof, but that model is missing or could not be loaded.

Agent strategy:

- Restore `catalog/verify_scheduler_model.json` from the canonical toolchain tree.
- Keep async proof runs on the released deterministic scheduler model only.
- Re-run `x07 verify --prove`.


## `X07V_SCOPE_INVARIANT_FAILED`

Summary: An async scope invariant failed under proof.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires contract correction or state-machine changes.

Details:

The `scope_invariant` clause does not hold across the proved async execution.

Agent strategy:

- Re-check the scope state carried across awaits and cancellation.
- Strengthen or correct `scope_invariant` so it matches the actual state machine.
- Re-run `x07 verify --prove`.


## `X07V_SMT_SAT`

Summary: Core lint/schema diagnostic `X07V_SMT_SAT`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_SMT_TIMEOUT`

Summary: SMT solver returned unknown due to a timeout.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The solver returned `unknown` and reported `reason-unknown=timeout` for the SMT query.

Agent strategy:

- Increase solver/verification bounds and re-run `x07 verify`.
- If the target is expected to be hard to prove, keep the default bounded lane and rely on tests for that surface.


## `X07V_SMT_UNKNOWN`

Summary: Core lint/schema diagnostic `X07V_SMT_UNKNOWN`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_SUMMARY_MISMATCH`

Summary: An imported proof summary does not match the current declaration.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `never`
No quickfix reason: Requires aligning reviewed summary artifacts with the current declaration graph.

Details:

The supplied `x07.verify.proof_summary@0.2.0` artifact names a reachable symbol, but its declaration digest does not match the currently loaded source graph.

Agent strategy:

- Regenerate the imported proof summary from the exact reviewed declaration set.
- Keep proof-summary artifacts and source graph revisions aligned.
- Re-run `x07 verify --proof-summary <path>` (or the deprecated `--summary <path>` alias).


## `X07V_SUMMARY_MISSING`

Summary: A reachable imported proof summary dependency is missing.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The requested verify run reached a symbol outside the locally loaded graph and no imported `x07.verify.proof_summary@0.2.0` artifact was supplied for that symbol.

Agent strategy:

- Re-run `x07 verify --prove --entry <sym>` for the reviewed callee to emit a proof summary.
- Pass the emitted artifact back via `x07 verify --proof-summary <path>` (or the deprecated `--summary <path>` alias).
- Re-run the original verify command.


## `X07V_UNSUPPORTED_DEFASYNC_FORM`

Summary: The selected `defasync` target is outside the supported proof subset.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

Async proof currently requires a certifiable state-machine form with supported parameters and a `bytes` or `result_bytes` result type.

Agent strategy:

- Refactor the async entry into the supported subset.
- Keep proof-target `defasync` results in `bytes` or `result_bytes` form.
- Re-run `x07 verify --prove`.


## `X07V_UNSUPPORTED_FOR_BOUNDS`

Summary: Core lint/schema diagnostic `X07V_UNSUPPORTED_FOR_BOUNDS`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_UNSUPPORTED_HEAP_EFFECT`

Summary: The proved core uses heap or pointer effects outside the supported subset.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The verifier rejects direct heap mutation, raw pointer operations, and related memory intrinsics inside the certifiable pure proof subset instead of silently weakening proof posture.

Agent strategy:

- Move heap or pointer effects behind a reviewed capsule boundary.
- Keep proof-target logic on pure value transformations.
- Re-run `x07 verify --prove`.


## `X07V_UNSUPPORTED_MUTUAL_RECURSION`

Summary: Core lint/schema diagnostic `X07V_UNSUPPORTED_MUTUAL_RECURSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_UNSUPPORTED_RECURSION`

Summary: Core lint/schema diagnostic `X07V_UNSUPPORTED_RECURSION`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07V_UNSUPPORTED_RICH_TYPE`

Summary: The target signature is outside the supported richer-data proof subset.

Origins:
- x07 (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The verifier currently supports the reviewed richer-data carriers for certification: unbranded `bytes` / `bytes_view`, `vec_u8`, first-order `option_*` / `result_*`, and branded `bytes_view` carriers whose brand resolves through reachable `meta.brands_v1.validate`. Schema-derived record and tagged-union documents can sit directly on the proof boundary through branded `bytes_view` inputs. Nested result carriers are still rejected explicitly.

Agent strategy:

- Keep proof-target signatures on the supported richer-data carriers.
- Schema-derived record and tagged-union documents can use branded `bytes_view` inputs when the validator is reachable through `meta.brands_v1.validate`.
- Re-run `x07 verify --prove`.


## `X07V_UNWIND_INCOMPLETE`

Summary: Core lint/schema diagnostic `X07V_UNWIND_INCOMPLETE`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X7I0001`

Summary: x07import subset compatibility diagnostic `X7I0001`.

Origins:
- x07import-core (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The source program uses syntax/semantics outside the supported importer subset. Repair is deterministic by rewriting the source into supported constructs.

Agent strategy:

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

- Inspect importer diagnostic context.
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

