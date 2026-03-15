# Diagnostic codes catalog

This file is generated from `catalog/diagnostics.json` using `x07 diag catalog`.

- total codes: 400
- quickfix support (`sometimes` or `always`): 370
- quickfix coverage: 92.50%

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
| `E_SBOM_GENERATION_FAILED` | x07 / lint / error | sometimes | Diagnostic code `E_SBOM_GENERATION_FAILED`. |
| `W_ARCH_CONTRACTS_LOCK_MISSING` | x07 / lint / warning | sometimes | Architecture contract diagnostic `W_ARCH_CONTRACTS_LOCK_MISSING`. |
| `W_ARCH_CONTRACT_OPAQUE_USAGE` | x07 / lint / warning | sometimes | Architecture contract diagnostic `W_ARCH_CONTRACT_OPAQUE_USAGE`. |
| `W_DEPS_CAP_POLICY_MISSING` | x07 / lint / warning | sometimes | Diagnostic code `W_DEPS_CAP_POLICY_MISSING`. |
| `X07-ARITY-0000` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-0000`. |
| `X07-ARITY-BEGIN-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-BEGIN-0001`. |
| `X07-ARITY-FOR-0001` | x07c / lint / error | sometimes | `for` has invalid arity. |
| `X07-ARITY-IF-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-IF-0001`. |
| `X07-ARITY-LET-0001` | x07c / lint / error | sometimes | `let`/`set` has invalid arity. |
| `X07-ARITY-RETURN-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-RETURN-0001`. |
| `X07-ARITY-UNSAFE-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-ARITY-UNSAFE-0001`. |
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
| `X07-GENERICS-0001` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-GENERICS-0001`. |
| `X07-GENERICS-0002` | x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-GENERICS-0002`. |
| `X07-INTERNAL-0001` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07-INTERNAL-0001`. |
| `X07-IO-READ-0001` | x07 / lint / error<br/>x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-IO-READ-0001`. |
| `X07-IO-WRITE-0001` | x07 / lint / error<br/>x07c / lint / error | sometimes | Core lint/schema diagnostic `X07-IO-WRITE-0001`. |
| `X07-JSON-PARSE-0001` | x07c / parse / error | sometimes | Core lint/schema diagnostic `X07-JSON-PARSE-0001`. |
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
| `X07-TYPE-CALL-0001` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-CALL-0001`. |
| `X07-TYPE-CALL-0002` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-CALL-0002`. |
| `X07-TYPE-CALL-0003` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-CALL-0003`. |
| `X07-TYPE-IF-0001` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-IF-0001`. |
| `X07-TYPE-IF-0002` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-IF-0002`. |
| `X07-TYPE-RET-0001` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-RET-0001`. |
| `X07-TYPE-SET-0001` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-SET-0001`. |
| `X07-TYPE-SET-0002` | x07c / type / error | sometimes | Core lint/schema diagnostic `X07-TYPE-SET-0002`. |
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
| `X07PKG_API_URL` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_API_URL`. |
| `X07PKG_DEP_EXISTS` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_DEP_EXISTS`. |
| `X07PKG_DEP_NOT_FOUND` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_DEP_NOT_FOUND`. |
| `X07PKG_DOWNLOAD_FAILED` | x07 / lint / error | never | Diagnostic code `X07PKG_DOWNLOAD_FAILED`. |
| `X07PKG_INDEX_CONFIG` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_INDEX_CONFIG`. |
| `X07PKG_INDEX_FETCH` | x07 / lint / error | never | Diagnostic code `X07PKG_INDEX_FETCH`. |
| `X07PKG_INDEX_NO_MATCH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_INDEX_NO_MATCH`. |
| `X07PKG_LOCAL_MISSING_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOCAL_MISSING_DEP`. |
| `X07PKG_LOCK_MISMATCH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOCK_MISMATCH`. |
| `X07PKG_LOCK_MISSING` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOCK_MISSING`. |
| `X07PKG_LOGIN_FAILED` | x07 / lint / error | never | Diagnostic code `X07PKG_LOGIN_FAILED`. |
| `X07PKG_LOGIN_TOKEN` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_LOGIN_TOKEN`. |
| `X07PKG_OFFLINE_MISSING_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_OFFLINE_MISSING_DEP`. |
| `X07PKG_PATCH_MISSING_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_PATCH_MISSING_DEP`. |
| `X07PKG_PUBLISH_FAILED` | x07 / lint / error | never | Diagnostic code `X07PKG_PUBLISH_FAILED`. |
| `X07PKG_PUBLISH_RESPONSE` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_PUBLISH_RESPONSE`. |
| `X07PKG_PUBLISH_RESPONSE_MISMATCH` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_PUBLISH_RESPONSE_MISMATCH`. |
| `X07PKG_SPEC_INVALID` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_SPEC_INVALID`. |
| `X07PKG_TRANSITIVE_MISSING` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_TRANSITIVE_MISSING`. |
| `X07PKG_YANKED_DEP` | x07 / lint / error | sometimes | Package workflow diagnostic `X07PKG_YANKED_DEP`. |
| `X07RD_ADVISORY_ALLOWANCE_ENABLED` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_ADVISORY_ALLOWANCE_ENABLED`. |
| `X07RD_ASYNC_PROOF_COVERAGE_DECREASE` | x07 / lint / error | never | Review diff gate rejected an async proof coverage regression. |
| `X07RD_CAPSULE_CONTRACT_RELAXATION` | x07 / lint / error | never | Review diff gate rejected a capsule contract relaxation. |
| `X07RD_CAPSULE_NETWORK_SURFACE_WIDEN` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_CAPSULE_NETWORK_SURFACE_WIDEN`. |
| `X07RD_CAPSULE_SET_CHANGE` | x07 / lint / error | never | Review diff gate rejected a capsule inventory change. |
| `X07RD_LOCKFILE_HASH_CHANGE` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_LOCKFILE_HASH_CHANGE`. |
| `X07RD_NETWORK_ALLOWLIST_WIDEN` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_NETWORK_ALLOWLIST_WIDEN`. |
| `X07RD_PACKAGE_SET_CHANGE` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_PACKAGE_SET_CHANGE`. |
| `X07RD_PEER_POLICY_RELAXATION` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07RD_PEER_POLICY_RELAXATION`. |
| `X07RD_RECURSION_PROOF_COVERAGE_DECREASE` | x07 / lint / error | never | Review diff gate rejected a recursive proof coverage regression. |
| `X07RD_RUNTIME_ATTEST_REGRESSION` | x07 / run / error | never | Review diff gate rejected a runtime attestation regression. |
| `X07RD_SANDBOX_POLICY_WIDEN` | x07 / lint / error | never | Review diff gate rejected a sandbox policy widening. |
| `X07RD_SUMMARY_DOWNGRADE` | x07 / lint / error | never | Review diff gate rejected a proof-summary downgrade. |
| `X07RR_ENTRY_EXISTS` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_ENTRY_EXISTS`. |
| `X07RR_HTTP` | x07 / run / error | never | Diagnostic code `X07RR_HTTP`. |
| `X07RR_KEY_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_KEY_EMPTY`. |
| `X07RR_KIND_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_KIND_EMPTY`. |
| `X07RR_LATENCY_OUT_OF_RANGE` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_LATENCY_OUT_OF_RANGE`. |
| `X07RR_OP_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_OP_EMPTY`. |
| `X07RR_URL_EMPTY` | x07 / run / error | sometimes | Record/replay fixture diagnostic `X07RR_URL_EMPTY`. |
| `X07TC_EARCH_STRICT` | x07 / run / error | sometimes | Strict architecture evidence failed certification. |
| `X07TC_EASYNC_PROOF` | x07 / run / error | never | Async proof coverage or async proof obligations failed certification. |
| `X07TC_EBOUNDARY_MISSING` | x07 / run / error | sometimes | Boundary declarations are missing or incomplete for certification. |
| `X07TC_EBOUNDARY_RELAXED` | x07 / run / error | never | The candidate relaxes a certified boundary contract relative to the baseline. |
| `X07TC_ECAPSULE_ATTEST` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_ECAPSULE_ATTEST`. |
| `X07TC_ECAPSULE_NETWORK_ATTEST` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_ECAPSULE_NETWORK_ATTEST`. |
| `X07TC_ECOMPILE_ATTEST` | x07 / run / error | never | Compile attestation failed or could not bind the emitted binary. |
| `X07TC_EDEP_CLOSURE` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_EDEP_CLOSURE`. |
| `X07TC_EDIFF_POSTURE` | x07 / run / error | never | Baseline review or trust-posture diff gate failed certification. |
| `X07TC_EEFFECT_LOG` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_EEFFECT_LOG`. |
| `X07TC_ENET_POLICY` | x07 / run / error | never | Network policy posture changed relative to the reviewed baseline. |
| `X07TC_ENONDET` | x07 / run / error | never | Trust report detected nondeterminism in the certified closure. |
| `X07TC_EPBT` | x07 / run / error | sometimes | Boundary-required property tests are missing, malformed, or failing. |
| `X07TC_EPEER_POLICY` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_EPEER_POLICY`. |
| `X07TC_EPROFILE` | x07 / run / error | sometimes | Certification trust profile is missing or invalid. |
| `X07TC_EPROJECT` | x07 / run / error | sometimes | Certification could not resolve the project manifest or source closure. |
| `X07TC_EPROOF_COVERAGE` | x07 / run / error | never | Reachable proof coverage is incomplete for certification. |
| `X07TC_ERUNTIME_ATTEST` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_ERUNTIME_ATTEST`. |
| `X07TC_ERUNTIME_NETWORK_EVIDENCE` | x07 / run / error | sometimes | Core lint/schema diagnostic `X07TC_ERUNTIME_NETWORK_EVIDENCE`. |
| `X07TC_ESANDBOX_PROFILE` | x07 / run / error | never | Sandbox runtime evidence violates the certification profile. |
| `X07TC_ESCHEMA_DRIFT` | x07 / run / error | sometimes | Boundary-referenced schema outputs drifted or are missing. |
| `X07TC_ETESTS` | x07 / run / error | sometimes | Deterministic certification tests are missing or failing. |
| `X07TC_ETRUST_REPORT` | x07 / run / error | never | Trust report evidence failed certification. |
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
| `X07TP_NETWORK_MODE_FORBIDDEN` | x07 / run / error | sometimes | Sandboxed local trusted-program profile allows networking where it should not. |
| `X07TP_NETWORK_PROFILE_REQUIRED` | x07 / run / error | sometimes | Networked trusted-program profile is missing required network certification posture. |
| `X07TP_NOT_CERTIFIABLE` | x07 / run / error | sometimes | Trust profile is weaker than the Milestone A certification floor. |
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
| `X07V_INTERNAL` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_INTERNAL`. |
| `X07V_SCOPE_INVARIANT_FAILED` | x07 / lint / error | never | An async scope invariant failed under proof. |
| `X07V_SMT_SAT` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_SMT_SAT`. |
| `X07V_SMT_UNKNOWN` | x07 / lint / error | sometimes | Core lint/schema diagnostic `X07V_SMT_UNKNOWN`. |
| `X07V_SUMMARY_MISMATCH` | x07 / lint / error | never | An imported proof summary does not match the current declaration. |
| `X07V_SUMMARY_MISSING` | x07 / lint / error | sometimes | A reachable proof summary dependency is missing. |
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

The enclosing `defn` declares `decreases[]`, but the function is not a directly self-recursive proof target or it lacks the surrounding contract clauses needed for certification.

Agent strategy:

- Keep `decreases[]` only on directly self-recursive `defn` targets.
- Add at least one `requires`, `ensures`, or `invariant` clause when the function is meant to be proved.
- Remove `decreases[]` from non-recursive helpers.
- Re-run `x07 lint` or `x07 check`.


## `X07-CONTRACT-0011`

Summary: Recursive self-call is missing decreases evidence.

Origins:
- x07c (stage: lint, severity: error)

Quickfix support: `sometimes`

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

Summary: Core lint/schema diagnostic `X07-INTERNAL-0001`.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


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

Summary: Core lint/schema diagnostic `X07-TYPE-CALL-0001`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


## `X07-TYPE-CALL-0002`

Summary: Core lint/schema diagnostic `X07-TYPE-CALL-0002`.

Origins:
- x07c (stage: type, severity: error)

Quickfix support: `sometimes`

Details:

The issue is in x07AST shape, world capability use, or policy/schema constraints and is typically repairable with deterministic AST/config edits.

Agent strategy:

- Run `x07 fmt`, `x07 lint`, and `x07 fix`.
- Apply deterministic AST/config edits.
- Re-run compile/test.


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

Milestone C networked certification requires VM-backed sandboxing, forbids weaker isolation, and rejects project worlds outside the certifiable sandbox line.

Agent strategy:

- Keep `worlds_allowed` free of `run-os`.
- Set `sandbox_requirements.sandbox_backend="vm"` and `sandbox_requirements.forbid_weaker_isolation=true`.
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

Summary: Sandboxed local trusted-program profile allows networking where it should not.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The Milestone B local sandbox profile keeps networking disabled until VM-boundary allowlist enforcement is fully supported across backends.

Agent strategy:

- Keep `sandbox_requirements.network_mode="none"`.
- Ensure the selected sandbox policy sets `net.enabled=false`.
- Re-run `x07 trust profile check`.


## `X07TP_NETWORK_PROFILE_REQUIRED`

Summary: Networked trusted-program profile is missing required network certification posture.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The Milestone C networked trust profile must require the allowlist-backed network posture, attested network capsules, and matching runtime network evidence semantics.

Agent strategy:

- Keep `sandbox_requirements.network_mode="allowlist"` and `sandbox_requirements.network_enforcement="vm_boundary_allowlist"`.
- Set `evidence_requirements.require_network_capsules=true`.
- Re-run `x07 trust profile check`.


## `X07TP_NOT_CERTIFIABLE`

Summary: Trust profile is weaker than the Milestone A certification floor.

Origins:
- x07 (stage: run, severity: error)

Quickfix support: `sometimes`

Details:

The profile relaxes one or more requirements needed for `verified_core_pure_v1` certification.

Agent strategy:

- Tighten the profile to the verified-core floor instead of weakening the certificate contract.
- Keep the profile immutable once it is published.
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

The supplied `x07.verify.summary@0.1.0` artifact names a reachable symbol, but its declaration digest does not match the currently loaded source graph.

Agent strategy:

- Regenerate the imported summary from the exact reviewed declaration set.
- Keep summary artifacts and source graph revisions aligned.
- Re-run `x07 verify --summary <path>`.


## `X07V_SUMMARY_MISSING`

Summary: A reachable proof summary dependency is missing.

Origins:
- x07 (stage: lint, severity: error)

Quickfix support: `sometimes`

Details:

The requested verify run reached a symbol outside the locally loaded graph and no imported `x07.verify.summary@0.1.0` artifact was supplied for that symbol.

Agent strategy:

- Re-run `x07 verify --coverage --entry <sym>` for the reviewed callee to emit `verify.summary.json`.
- Pass the emitted artifact back via `x07 verify --summary <path>`.
- Re-run the original verify command.


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

