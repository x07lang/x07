//! Shared, version-pinned protocol identifiers.
//!
//! These constants are the single source of truth for schema/version strings that
//! appear in machine-readable I/O. Versioning rules are defined in
//! `docs/versioning-policy.md`.

pub const X07AST_SCHEMA_VERSION: &str = "x07.x07ast@0.3.0";
pub const X07DIAG_SCHEMA_VERSION: &str = "x07.x07diag@0.1.0";
pub const X07TEST_SCHEMA_VERSION: &str = "x07.x07test@0.2.0";

pub const X07C_REPORT_SCHEMA_VERSION: &str = "x07c.report@0.1.0";
pub const X07_HOST_RUNNER_REPORT_SCHEMA_VERSION: &str = "x07-host-runner.report@0.3.0";
pub const X07_OS_RUNNER_REPORT_SCHEMA_VERSION: &str = "x07-os-runner.report@0.3.0";
pub const X07_RUN_REPORT_SCHEMA_VERSION: &str = "x07.run.report@0.1.0";
pub const X07_BUNDLE_REPORT_SCHEMA_VERSION: &str = "x07.bundle.report@0.1.0";

pub const RUN_OS_POLICY_SCHEMA_VERSION: &str = "x07.run-os-policy@0.1.0";
pub const X07_POLICY_INIT_REPORT_SCHEMA_VERSION: &str = "x07.policy.init.report@0.1.0";

pub const NATIVE_BACKENDS_SCHEMA_VERSION: &str = "x07.native-backends@0.1.0";
pub const NATIVE_REQUIRES_SCHEMA_VERSION: &str = "x07.native-requires@0.1.0";

pub const X07_ARCH_MANIFEST_SCHEMA_VERSION: &str = "x07.arch.manifest@0.1.0";
pub const X07_ARCH_MANIFEST_LOCK_SCHEMA_VERSION: &str = "x07.arch.manifest.lock@0.1.0";
pub const X07_ARCH_REPORT_SCHEMA_VERSION: &str = "x07.arch.report@0.1.0";
pub const X07_ARCH_PATCHSET_SCHEMA_VERSION: &str = "x07.arch.patchset@0.1.0";
pub const X07_ARCH_CONTRACTS_LOCK_SCHEMA_VERSION: &str = "x07.arch.contracts.lock@0.1.0";
pub const X07_ARCH_RR_INDEX_SCHEMA_VERSION: &str = "x07.arch.rr.index@0.1.0";
pub const X07_ARCH_RR_POLICY_SCHEMA_VERSION: &str = "x07.arch.rr.policy@0.1.0";
pub const X07_ARCH_RR_SANITIZE_SCHEMA_VERSION: &str = "x07.arch.rr.sanitize@0.1.0";
pub const X07_ARCH_SM_INDEX_SCHEMA_VERSION: &str = "x07.arch.sm.index@0.1.0";
pub const X07_ARCH_BUDGETS_INDEX_SCHEMA_VERSION: &str = "x07.arch.budgets.index@0.1.0";
pub const X07_BUDGET_PROFILE_SCHEMA_VERSION: &str = "x07.budget.profile@0.1.0";
pub const X07_SM_SPEC_SCHEMA_VERSION: &str = "x07.sm.spec@0.1.0";

pub const PROJECT_MANIFEST_SCHEMA_VERSION: &str = "x07.project@0.2.0";
pub const PACKAGE_MANIFEST_SCHEMA_VERSION: &str = "x07.package@0.1.0";
pub const PROJECT_LOCKFILE_SCHEMA_VERSION: &str = "x07.lock@0.2.0";
