//! Shared, version-pinned protocol identifiers.
//!
//! These constants are the single source of truth for schema/version strings that
//! appear in machine-readable I/O. Versioning rules are defined in
//! `docs/versioning-policy.md`.

pub const X07AST_SCHEMA_VERSION: &str = "x07.x07ast@0.3.0";
pub const X07DIAG_SCHEMA_VERSION: &str = "x07.x07diag@0.1.0";
pub const X07TEST_SCHEMA_VERSION: &str = "x07.x07test@0.2.0";

pub const X07C_REPORT_SCHEMA_VERSION: &str = "x07c.report@0.1.0";
pub const X07_HOST_RUNNER_REPORT_SCHEMA_VERSION: &str = "x07-host-runner.report@0.2.0";
pub const X07_OS_RUNNER_REPORT_SCHEMA_VERSION: &str = "x07-os-runner.report@0.2.0";
pub const X07_RUN_REPORT_SCHEMA_VERSION: &str = "x07.run.report@0.1.0";
pub const X07_BUNDLE_REPORT_SCHEMA_VERSION: &str = "x07.bundle.report@0.1.0";

pub const RUN_OS_POLICY_SCHEMA_VERSION: &str = "x07.run-os-policy@0.1.0";
pub const X07_POLICY_INIT_REPORT_SCHEMA_VERSION: &str = "x07.policy.init.report@0.1.0";

pub const NATIVE_BACKENDS_SCHEMA_VERSION: &str = "x07.native-backends@0.1.0";
pub const NATIVE_REQUIRES_SCHEMA_VERSION: &str = "x07.native-requires@0.1.0";

pub const PROJECT_MANIFEST_SCHEMA_VERSION: &str = "x07.project@0.2.0";
pub const PACKAGE_MANIFEST_SCHEMA_VERSION: &str = "x07.package@0.1.0";
pub const PROJECT_LOCKFILE_SCHEMA_VERSION: &str = "x07.lock@0.2.0";
