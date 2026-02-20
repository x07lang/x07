//! Shared, version-pinned protocol identifiers.
//!
//! These constants are the single source of truth for schema/version strings that
//! appear in machine-readable I/O. Versioning rules are defined in
//! `docs/versioning-policy.md`.

pub const X07AST_SCHEMA_VERSION_V0_3_0: &str = "x07.x07ast@0.3.0";
pub const X07AST_SCHEMA_VERSION_V0_4_0: &str = "x07.x07ast@0.4.0";
pub const X07AST_SCHEMA_VERSION_V0_5_0: &str = "x07.x07ast@0.5.0";

/// The default x07AST schema version emitted by current tooling.
pub const X07AST_SCHEMA_VERSION: &str = X07AST_SCHEMA_VERSION_V0_5_0;

pub const X07AST_SCHEMA_VERSIONS_SUPPORTED: &[&str] = &[
    X07AST_SCHEMA_VERSION_V0_3_0,
    X07AST_SCHEMA_VERSION_V0_4_0,
    X07AST_SCHEMA_VERSION_V0_5_0,
];
pub const X07_MONO_MAP_SCHEMA_VERSION: &str = "x07.mono.map@0.1.0";
pub const X07_PBT_REPRO_SCHEMA_VERSION: &str = "x07.pbt.repro@0.1.0";
pub const X07_CONTRACT_REPRO_SCHEMA_VERSION: &str = "x07.contract.repro@0.1.0";
pub const X07DIAG_SCHEMA_VERSION: &str = "x07.x07diag@0.1.0";
pub const X07_AGENT_CONTEXT_SCHEMA_VERSION: &str = "x07.agent.context@0.1.0";
pub const X07TEST_SCHEMA_VERSION: &str = "x07.x07test@0.3.0";
pub const X07_DIAG_CATALOG_SCHEMA_VERSION: &str = "x07.diag.catalog@0.1.0";
pub const X07_DIAG_COVERAGE_SCHEMA_VERSION: &str = "x07.diag.coverage@0.1.0";

pub const X07C_REPORT_SCHEMA_VERSION: &str = "x07c.report@0.1.0";
pub const X07_HOST_RUNNER_REPORT_SCHEMA_VERSION: &str = "x07-host-runner.report@0.3.0";
pub const X07_OS_RUNNER_REPORT_SCHEMA_VERSION: &str = "x07-os-runner.report@0.3.0";
pub const X07_RUN_REPORT_SCHEMA_VERSION: &str = "x07.run.report@0.1.0";
pub const X07_BUNDLE_REPORT_SCHEMA_VERSION: &str = "x07.bundle.report@0.2.0";
pub const X07_DOC_REPORT_SCHEMA_VERSION: &str = "x07.doc.report@0.1.0";
pub const X07_VERIFY_REPORT_SCHEMA_VERSION: &str = "x07.verify.report@0.1.0";
pub const X07_VERIFY_CEX_SCHEMA_VERSION: &str = "x07.verify.cex@0.1.0";
pub const X07_BENCH_SUITE_SCHEMA_VERSION: &str = "x07.bench.suite@0.1.0";
pub const X07_BENCH_INSTANCE_SCHEMA_VERSION: &str = "x07.bench.instance@0.1.0";
pub const X07_BENCH_REPORT_SCHEMA_VERSION: &str = "x07.bench.report@0.1.0";
pub const X07_REVIEW_DIFF_SCHEMA_VERSION: &str = "x07.review.diff@0.1.0";
pub const X07_TRUST_REPORT_SCHEMA_VERSION: &str = "x07.trust.report@0.1.0";
pub const X07_DEPS_CAPABILITY_POLICY_SCHEMA_VERSION: &str = "x07.deps.capability_policy@0.1.0";
pub const X07_TOOL_REPORT_SCHEMA_VERSION: &str = "x07.tool.report@0.1.0";

pub const RUN_OS_POLICY_SCHEMA_VERSION: &str = "x07.run-os-policy@0.1.0";
pub const X07_POLICY_INIT_REPORT_SCHEMA_VERSION: &str = "x07.policy.init.report@0.1.0";

pub const NATIVE_BACKENDS_SCHEMA_VERSION: &str = "x07.native-backends@0.1.0";
pub const NATIVE_REQUIRES_SCHEMA_VERSION: &str = "x07.native-requires@0.1.0";

pub const X07_ARCH_MANIFEST_SCHEMA_VERSION: &str = "x07.arch.manifest@0.1.0";
pub const X07_ARCH_MANIFEST_LOCK_SCHEMA_VERSION: &str = "x07.arch.manifest.lock@0.1.0";
pub const X07_ARCH_REPORT_SCHEMA_VERSION: &str = "x07.arch.report@0.1.0";
pub const X07_ARCH_PATCHSET_SCHEMA_VERSION: &str = "x07.arch.patchset@0.1.0";
pub const X07_PATCHSET_SCHEMA_VERSION: &str = "x07.patchset@0.1.0";
pub const X07_ARCH_CONTRACTS_LOCK_SCHEMA_VERSION: &str = "x07.arch.contracts.lock@0.1.0";
pub const X07_ARCH_RR_INDEX_SCHEMA_VERSION: &str = "x07.arch.rr.index@0.1.0";
pub const X07_ARCH_RR_POLICY_SCHEMA_VERSION: &str = "x07.arch.rr.policy@0.1.0";
pub const X07_ARCH_RR_SANITIZE_SCHEMA_VERSION: &str = "x07.arch.rr.sanitize@0.1.0";
pub const X07_ARCH_SM_INDEX_SCHEMA_VERSION: &str = "x07.arch.sm.index@0.1.0";
pub const X07_ARCH_BUDGETS_INDEX_SCHEMA_VERSION: &str = "x07.arch.budgets.index@0.1.0";
pub const X07_ARCH_STREAM_PLUGINS_INDEX_SCHEMA_VERSION: &str =
    "x07.arch.stream.plugins.index@0.1.0";
pub const X07_ARCH_STREAM_PLUGIN_SCHEMA_VERSION: &str = "x07.arch.stream.plugin@0.1.0";
pub const X07_BUDGET_PROFILE_SCHEMA_VERSION: &str = "x07.budget.profile@0.1.0";
pub const X07_SM_SPEC_SCHEMA_VERSION: &str = "x07.sm.spec@0.1.0";

pub const X07_ARCH_WEB_INDEX_SCHEMA_VERSION: &str = "x07.arch.web.index@0.1.0";
pub const X07_ARCH_WEB_API_SCHEMA_VERSION: &str = "x07.arch.web.api@0.1.0";
pub const X07_ARCH_WEB_OPENAPI_PROFILE_SCHEMA_VERSION: &str = "x07.arch.web.openapi_profile@0.1.0";

pub const X07_ARCH_CRAWL_INDEX_SCHEMA_VERSION: &str = "x07.arch.crawl.index@0.1.0";
pub const X07_ARCH_CRAWL_POLICY_SCHEMA_VERSION: &str = "x07.arch.crawl.policy@0.1.0";

pub const X07_ARCH_MSG_INDEX_SCHEMA_VERSION: &str = "x07.arch.msg.index@0.1.0";
pub const X07_ARCH_MSG_KAFKA_INDEX_SCHEMA_VERSION: &str = "x07.arch.msg.kafka.index@0.1.0";
pub const X07_ARCH_MSG_KAFKA_PROFILE_SCHEMA_VERSION: &str = "x07.arch.msg.kafka.profile@0.1.0";
pub const X07_ARCH_MSG_AMQP_INDEX_SCHEMA_VERSION: &str = "x07.arch.msg.amqp.index@0.1.0";
pub const X07_ARCH_MSG_AMQP_PROFILE_SCHEMA_VERSION: &str = "x07.arch.msg.amqp.profile@0.1.0";
pub const X07_ARCH_MSG_AMQP_TOPOLOGY_SCHEMA_VERSION: &str = "x07.arch.msg.amqp.topology@0.1.0";

pub const X07_ARCH_CLI_INDEX_SCHEMA_VERSION: &str = "x07.arch.cli.index@0.1.0";
pub const X07_ARCH_CLI_PROFILE_SCHEMA_VERSION: &str = "x07.arch.cli.profile@0.1.0";

pub const X07_ARCH_ARCHIVE_INDEX_SCHEMA_VERSION: &str = "x07.arch.archive.index@0.1.0";
pub const X07_ARCH_ARCHIVE_PROFILE_SCHEMA_VERSION: &str = "x07.arch.archive.profile@0.1.0";
pub const X07_ARCH_DB_INDEX_SCHEMA_VERSION: &str = "x07.arch.db.index@0.1.0";
pub const X07_DB_MIGRATE_PLAN_SCHEMA_VERSION: &str = "x07.db.migrate.plan@0.1.0";
pub const X07_ARCH_DB_QUERIES_SCHEMA_VERSION: &str = "x07.arch.db.queries@0.1.0";
pub const X07_ARCH_OBS_INDEX_SCHEMA_VERSION: &str = "x07.arch.obs.index@0.1.0";
pub const X07_OBS_METRICS_REGISTRY_SCHEMA_VERSION: &str = "x07.obs.metrics.registry@0.1.0";
pub const X07_OBS_EXPORTER_PROFILE_SCHEMA_VERSION: &str = "x07.obs.exporter.profile@0.1.0";
pub const X07_ARCH_NET_INDEX_SCHEMA_VERSION: &str = "x07.arch.net.index@0.1.0";
pub const X07_ARCH_NET_GRPC_SERVICES_SCHEMA_VERSION: &str = "x07.arch.net.grpc.services@0.1.0";
pub const X07_ARCH_CRYPTO_INDEX_SCHEMA_VERSION: &str = "x07.arch.crypto.index@0.1.0";
pub const X07_ARCH_CRYPTO_JWT_PROFILES_SCHEMA_VERSION: &str = "x07.arch.crypto.jwt_profiles@0.1.0";

pub const PROJECT_MANIFEST_SCHEMA_VERSION: &str = "x07.project@0.2.0";
pub const PACKAGE_MANIFEST_SCHEMA_VERSION: &str = "x07.package@0.1.0";
pub const PROJECT_LOCKFILE_SCHEMA_VERSION: &str = "x07.lock@0.2.0";
