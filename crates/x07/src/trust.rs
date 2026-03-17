use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use x07_contracts::{
    PROJECT_LOCKFILE_SCHEMA_VERSION, PROJECT_LOCKFILE_SCHEMA_VERSIONS_SUPPORTED,
    X07DIAG_SCHEMA_VERSION, X07_CAPSULE_ATTEST_SCHEMA_VERSION, X07_CAPSULE_CONTRACT_SCHEMA_VERSION,
    X07_DEP_CLOSURE_ATTEST_SCHEMA_VERSION, X07_EFFECT_LOG_SCHEMA_VERSION,
    X07_PEER_POLICY_SCHEMA_VERSION, X07_RUNTIME_ATTEST_SCHEMA_VERSION,
    X07_TRUST_CERTIFICATE_SCHEMA_VERSION, X07_TRUST_PROFILE_SCHEMA_VERSION,
    X07_TRUST_REPORT_SCHEMA_VERSION,
};
use x07_worlds::WorldId;
use x07c::diagnostics;
use x07c::project;

use crate::policy_overrides::{PolicyOverrides, PolicyResolution};
use crate::report_common;
use crate::reporting;
use crate::run;
use crate::util;
use crate::verify;

const X07_TRUST_REPORT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-trust.report.schema.json");
const X07_TRUST_PROFILE_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-trust.profile.schema.json");
const X07_TRUST_CERTIFICATE_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-trust.certificate.schema.json");
const X07_CAPSULE_INDEX_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-capsule.index.schema.json");
const X07_CAPSULE_CONTRACT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-capsule.contract.schema.json");
const X07_CAPSULE_ATTEST_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-capsule.attest.schema.json");
const X07_RUNTIME_ATTEST_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-runtime.attest.schema.json");
const X07_PEER_POLICY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-peer.policy.schema.json");
const X07_DEP_CLOSURE_ATTEST_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-dep.closure.attest.schema.json");
const X07_VERIFY_PRIMITIVES_CATALOG_BYTES: &[u8] =
    include_bytes!("../../../catalog/verify_primitives.json");
const X07_DEPS_CAPABILITY_POLICY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-deps.capability-policy.schema.json");

const DEFAULT_SOLVE_FUEL: u64 = 50_000_000;
const DEFAULT_MAX_MEMORY_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Args)]
#[command(subcommand_required = false)]
pub struct TrustArgs {
    #[command(subcommand)]
    pub cmd: Option<TrustCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TrustCommand {
    /// Emit a CI trust report artifact (budgets/caps, capabilities, nondeterminism, SBOM).
    Report(TrustReportArgs),
    /// Validate trust profiles and project compatibility.
    Profile(TrustProfileArgs),
    /// Validate or attest certified capsules.
    Capsule(TrustCapsuleArgs),
    /// Emit a certificate bundle for a certifiable entrypoint.
    Certify(TrustCertifyArgs),
}

#[derive(Debug, Clone, Args)]
#[command(subcommand_required = false)]
pub struct TrustProfileArgs {
    #[command(subcommand)]
    pub cmd: Option<TrustProfileCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TrustProfileCommand {
    /// Validate a trust profile and optional project compatibility.
    Check(TrustProfileCheckArgs),
}

#[derive(Debug, Clone, Args)]
#[command(subcommand_required = false)]
pub struct TrustCapsuleArgs {
    #[command(subcommand)]
    pub cmd: Option<TrustCapsuleCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TrustCapsuleCommand {
    /// Validate a capsule index plus referenced contracts and attestations.
    Check(TrustCapsuleCheckArgs),
    /// Emit a capsule attestation from a contract + digest inputs.
    Attest(TrustCapsuleAttestArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum TrustFailOn {
    AllowUnsafe,
    AllowFfi,
    NetEnabled,
    ProcessEnabled,
    Nondeterminism,
    SbomMissing,
    DepsCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum SbomFormat {
    None,
    Cyclonedx,
    Spdx,
}

#[derive(Debug, Clone, Args)]
pub struct TrustReportArgs {
    /// Project manifest path (`x07.json` or `*.x07project.json`).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Run profile name (project-defined).
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Optional output path for the HTML trust summary.
    #[arg(long, value_name = "PATH")]
    pub html_out: Option<PathBuf>,

    /// Optional x07 run wrapper reports to merge observed usage.
    #[arg(long, value_name = "PATH")]
    pub run_report: Vec<PathBuf>,

    /// Optional x07 bundle reports to merge policy materialization info.
    #[arg(long, value_name = "PATH")]
    pub bundle_report: Vec<PathBuf>,

    /// Optional x07test reports to merge observed stats (best-effort).
    #[arg(long, value_name = "PATH")]
    pub x07test: Vec<PathBuf>,

    /// SBOM format to generate (deterministic).
    #[arg(long, value_enum, default_value_t = SbomFormat::Cyclonedx)]
    pub sbom_format: SbomFormat,

    /// Optional dependency capability policy path (safe relative path).
    ///
    /// If omitted: attempts to load `x07.deps.capability-policy.json` from the project root.
    #[arg(long, value_name = "PATH")]
    pub deps_cap_policy: Option<String>,

    /// If set: missing policy/lock/schema mismatch becomes a hard error.
    #[arg(long)]
    pub strict: bool,

    /// CI gating: fail if any matching condition is true.
    #[arg(long, value_enum)]
    pub fail_on: Vec<TrustFailOn>,
}

#[derive(Debug, Clone, Args)]
pub struct TrustProfileCheckArgs {
    /// Trust profile JSON path.
    #[arg(long, value_name = "PATH")]
    pub profile: PathBuf,

    /// Optional project manifest path (`x07.json`) or directory containing it.
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Optional entry symbol to validate against the profile entry allowlist.
    #[arg(long, value_name = "SYM")]
    pub entry: Option<String>,

    /// Treat advisories as hard errors.
    #[arg(long)]
    pub strict: bool,
}

#[derive(Debug, Clone, Args)]
pub struct TrustCapsuleCheckArgs {
    /// Capsule index JSON path.
    #[arg(
        long,
        value_name = "PATH",
        default_value = "arch/capsules/index.x07capsule.json"
    )]
    pub index: PathBuf,

    /// Optional project manifest path (`x07.json`) or directory containing it.
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct TrustCapsuleAttestArgs {
    /// Capsule contract JSON path.
    #[arg(long, value_name = "PATH")]
    pub contract: PathBuf,

    /// Module or package files to hash into the attestation.
    #[arg(long, value_name = "PATH")]
    pub module: Vec<PathBuf>,

    /// Lockfile to hash into the attestation.
    #[arg(long, value_name = "PATH")]
    pub lockfile: PathBuf,

    /// Conformance report to hash into the attestation.
    #[arg(long, value_name = "PATH")]
    pub conformance_report: PathBuf,

    /// Output path for the capsule attestation.
    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct TrustCertifyArgs {
    /// Project manifest path (`x07.json`) or directory containing it.
    #[arg(long, value_name = "PATH")]
    pub project: PathBuf,

    /// Trust profile JSON path.
    #[arg(long, value_name = "PATH")]
    pub profile: PathBuf,

    /// Fully-qualified entry symbol to certify.
    #[arg(long, value_name = "SYM")]
    pub entry: String,

    /// Output directory for the certificate bundle.
    #[arg(long, value_name = "DIR")]
    pub out_dir: PathBuf,

    /// Tests manifest path.
    #[arg(long, value_name = "PATH", default_value = "tests/tests.json")]
    pub tests_manifest: PathBuf,

    /// Optional review baseline path for `x07 review diff`.
    #[arg(long, value_name = "PATH")]
    pub baseline: Option<PathBuf>,

    /// Optional output path for the bundled native executable.
    #[arg(long, value_name = "PATH")]
    pub bundle_out: Option<PathBuf>,

    /// Keep intermediate work directories.
    #[arg(long)]
    pub keep_workdir: bool,

    /// Do not write summary.html.
    #[arg(long)]
    pub no_html: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustProfile {
    schema_version: String,
    id: String,
    claims: Vec<String>,
    entrypoints: Vec<String>,
    worlds_allowed: Vec<String>,
    language_subset: TrustLanguageSubset,
    arch_requirements: TrustArchRequirements,
    evidence_requirements: TrustEvidenceRequirements,
    sandbox_requirements: TrustSandboxRequirements,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustLanguageSubset {
    allow_defasync: bool,
    allow_recursion: bool,
    allow_extern: bool,
    allow_unsafe: bool,
    allow_ffi: bool,
    allow_dynamic_dispatch: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustArchRequirements {
    manifest_min_version: String,
    require_allowlist_mode: bool,
    require_deny_cycles: bool,
    require_deny_orphans: bool,
    require_visibility: bool,
    require_world_caps: bool,
    require_brand_boundaries: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustEvidenceRequirements {
    require_boundary_index: bool,
    require_schema_derive_check: bool,
    require_smoke_harnesses: bool,
    require_unit_tests: bool,
    require_pbt: String,
    require_proof_mode: String,
    require_proof_coverage: String,
    require_async_proof_coverage: bool,
    require_per_symbol_prove_reports_defn: bool,
    require_per_symbol_prove_reports_async: bool,
    allow_coverage_summary_imports: bool,
    require_capsule_attestations: bool,
    require_runtime_attestation: bool,
    require_effect_log_digests: bool,
    require_peer_policies: bool,
    require_network_capsules: bool,
    require_dependency_closure_attestation: bool,
    require_compile_attestation: bool,
    require_trust_report_clean: bool,
    require_sbom: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustSandboxRequirements {
    sandbox_backend: String,
    forbid_weaker_isolation: bool,
    network_mode: String,
    network_enforcement: String,
}

#[derive(Debug, Clone, Serialize)]
struct TrustProfileCheckReport {
    schema_version: &'static str,
    ok: bool,
    profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry: Option<String>,
    diagnostics: Vec<diagnostics::Diagnostic>,
    exit_code: u8,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificate {
    schema_version: &'static str,
    verdict: String,
    profile: String,
    entry: String,
    operational_entry_symbol: String,
    out_dir: String,
    claims: Vec<String>,
    formal_verification_scope: String,
    proved_symbol_count: u64,
    proved_defn_count: u64,
    proved_defasync_count: u64,
    entry_body_formally_proved: bool,
    #[serde(default)]
    operational_entry_proof_inventory_refs: Vec<EvidenceRef>,
    capsule_boundary_only_symbol_count: u64,
    runtime_evidence_only_symbol_count: u64,
    async_proof: TrustCertificateAsyncProof,
    #[serde(default)]
    proof_inventory: Vec<TrustCertificateProofInventoryItem>,
    #[serde(default)]
    proof_assumptions: Vec<TrustCertificateProofAssumption>,
    recursive_proof_summary: TrustCertificateRecursiveProofSummary,
    #[serde(default)]
    imported_summary_inventory: Vec<TrustCertificateImportedSummary>,
    accepted_depends_on_bounded_proof: bool,
    accepted_depends_on_dev_only_assumption: bool,
    capsules: TrustCertificateCapsules,
    network_capsules: TrustCertificateNetworkCapsules,
    runtime: Option<TrustCertificateRuntime>,
    package_set_digest: Option<String>,
    dependency_closure: Option<TrustCertificateDependencyClosure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    effect_logs: Vec<EvidenceRef>,
    tcb: TrustCertificateTcb,
    evidence: TrustCertificateEvidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<diagnostics::Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateAsyncProof {
    reachable: u64,
    proved: u64,
    model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateRecursiveProofSummary {
    reachable_recursive_defn: u64,
    accepted_recursive_defn: u64,
    bounded_recursive_defn: u64,
    unbounded_recursive_defn: u64,
    imported_proof_summary_defn: u64,
    rejected_recursive_defn: u64,
    accepted_depends_on_bounded_proof: bool,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateImportedSummary {
    path: String,
    sha256_hex: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateProofInventoryItem {
    symbol: String,
    kind: String,
    result_kind: String,
    verify_report: EvidenceRef,
    proof_summary: EvidenceRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_object: Option<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_check_report: Option<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_check_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_check_checker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_object_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct TrustCertificateProofAssumption {
    kind: String,
    subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
    certifiable: bool,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateCapsules {
    count: u64,
    ids: Vec<String>,
    #[serde(default)]
    attestations: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateNetworkCapsules {
    count: u64,
    ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateRuntime {
    backend: String,
    network_mode: String,
    network_enforcement: String,
    weaker_isolation: bool,
    #[serde(default)]
    effective_allow_hosts: Vec<TrustCertificateNetHost>,
    policy_digest_bound: bool,
    guest_image_digest_bound: bool,
    attestation: Option<EvidenceRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TrustCertificateNetHost {
    host: String,
    ports: Vec<u16>,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateDependencyClosure {
    manifest_digest: String,
    lockfile_digest: String,
    packages: Vec<String>,
    advisory_check_ok: bool,
    attestation: Option<EvidenceRef>,
}

#[derive(Debug, Clone)]
struct FormalVerificationScopeSummary {
    formal_verification_scope: String,
    proved_symbol_count: u64,
    proved_defn_count: u64,
    proved_defasync_count: u64,
    entry_body_formally_proved: bool,
    operational_entry_proof_inventory_refs: Vec<EvidenceRef>,
    capsule_boundary_only_symbol_count: u64,
    runtime_evidence_only_symbol_count: u64,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateTcb {
    x07_version: String,
    host_compiler: String,
    trusted_primitive_manifest_digest: String,
}

#[derive(Debug, Clone, Serialize)]
struct TrustCertificateEvidence {
    boundaries_report: EvidenceRef,
    coverage_report: EvidenceRef,
    verify_summary_report: Option<EvidenceRef>,
    #[serde(default)]
    schema_derive_reports: Vec<EvidenceRef>,
    #[serde(default)]
    prove_reports: Vec<EvidenceRef>,
    tests_report: EvidenceRef,
    trust_report: EvidenceRef,
    compile_attestation: EvidenceRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_attestation: Option<EvidenceRef>,
    #[serde(default)]
    peer_policy_files: Vec<EvidenceRef>,
    #[serde(default)]
    capsule_attestations: Vec<EvidenceRef>,
    #[serde(default)]
    effect_logs: Vec<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_diff: Option<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependency_closure_attestation: Option<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct EvidenceRef {
    path: String,
    sha256_hex: String,
}

#[derive(Debug, Clone, Default)]
struct BoundaryEvidenceRequirements {
    schema_paths: BTreeSet<String>,
    required_tests: BTreeMap<String, BoundaryTestRequirement>,
}

#[derive(Debug, Clone)]
struct CapsuleArtifacts {
    capsules: TrustCertificateCapsules,
    network_capsules: TrustCertificateNetworkCapsules,
    capsule_attestations: Vec<EvidenceRef>,
    effect_logs: Vec<EvidenceRef>,
    peer_policies: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Default)]
struct BoundaryTestRequirement {
    expects_pbt: bool,
    boundary_ids: BTreeSet<String>,
    worlds_allowed: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct ManifestTestRequirement {
    world: String,
    has_pbt: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapsuleIndex {
    schema_version: String,
    #[serde(default)]
    capsules: Vec<CapsuleRef>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapsuleRef {
    id: String,
    #[serde(default)]
    worlds_allowed: Vec<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    contract_path: String,
    attestation_path: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapsuleContract {
    schema_version: String,
    id: String,
    #[serde(default)]
    worlds_allowed: Vec<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    language: CapsuleLanguage,
    input: Value,
    output: Value,
    #[serde(default)]
    error_spaces: Vec<String>,
    effect_log: CapsuleEffectLog,
    replay: CapsuleReplay,
    conformance: CapsuleConformance,
    #[serde(default)]
    network: Option<CapsuleNetworkContract>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapsuleLanguage {
    allow_unsafe: bool,
    allow_ffi: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapsuleEffectLog {
    schema_path: String,
    redaction: String,
    replay_safe: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapsuleReplay {
    mode: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapsuleConformance {
    #[serde(default)]
    tests: Vec<String>,
    #[serde(default)]
    report_path: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapsuleNetworkContract {
    peer_policy_paths: Vec<String>,
    #[serde(default)]
    request_contract_path: Option<String>,
    #[serde(default)]
    response_contract_path: Option<String>,
    #[serde(default)]
    conformance_tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CapsuleAttestationDoc {
    schema_version: &'static str,
    capsule_id: String,
    contract_digest: String,
    module_digests: Vec<CapsuleModuleDigest>,
    lockfile_digest: String,
    conformance_report_digest: String,
    peer_policy_digests: Vec<CapsuleModuleDigest>,
    request_contract_digest: Option<String>,
    response_contract_digest: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PeerPolicyDoc {
    schema_version: String,
    policy_id: String,
    role: String,
    host: String,
    ports: Vec<u16>,
    transport: String,
    tls_mode: String,
    #[serde(default)]
    sni: Option<String>,
    #[serde(default)]
    alpn: Vec<String>,
    #[serde(default)]
    ca_paths: Vec<String>,
    #[serde(default)]
    spki_sha256: Vec<String>,
    #[serde(default)]
    mtls_alias: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeAttestationDoc {
    schema_version: String,
    world: String,
    sandbox_backend: String,
    weaker_isolation: bool,
    artifact_path: String,
    #[serde(default)]
    policy_path: Option<String>,
    #[serde(default)]
    input_len_bytes: u64,
    #[serde(default)]
    run_dir: Option<String>,
    #[serde(default)]
    guest_image_digest: Option<String>,
    #[serde(default)]
    effective_policy_digest: Option<String>,
    network_mode: String,
    network_enforcement: String,
    #[serde(default)]
    allow_dns: bool,
    #[serde(default)]
    allow_tcp: bool,
    #[serde(default)]
    allow_udp: bool,
    #[serde(default)]
    effective_allow_hosts: Vec<TrustCertificateNetHost>,
    #[serde(default)]
    effective_deny_hosts: Vec<String>,
    #[serde(default)]
    bundled_binary_digest: String,
    #[serde(default)]
    compile_attestation_digest: Option<String>,
    #[serde(default)]
    capsule_attestation_digests: Vec<String>,
    #[serde(default)]
    effect_log_digests: Vec<String>,
    outcome: RuntimeAttestationOutcomeDoc,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeAttestationOutcomeDoc {
    ok: bool,
    exit_status: i32,
    #[serde(default)]
    trap: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DepClosureAttestationDoc {
    schema_version: String,
    project_path: String,
    manifest_digest: String,
    lockfile_digest: String,
    package_set_digest: String,
    #[serde(default)]
    dependencies: Vec<DepClosureDependencyDoc>,
    advisory_check: DepClosureAdvisoryCheckDoc,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DepClosureDependencyDoc {
    name: String,
    version: String,
    path: String,
    package_manifest_digest: String,
    module_root: String,
    module_root_digest: String,
    #[serde(default)]
    modules: Vec<DepClosureModuleDigestDoc>,
    yanked: bool,
    #[serde(default)]
    advisories: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DepClosureModuleDigestDoc {
    module_id: String,
    digest: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DepClosureAdvisoryCheckDoc {
    mode: String,
    ok: bool,
    allow_yanked: bool,
    allow_advisories: bool,
    #[serde(default)]
    yanked: Vec<String>,
    #[serde(default)]
    advisories: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CapsuleModuleDigest {
    path: String,
    digest: String,
}

fn path_components_for_compare(path: &Path) -> Vec<String> {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect()
}

fn attested_path_matches_declared_path(
    attested_path: &str,
    declared_path: &str,
    root: &Path,
) -> bool {
    let attested = Path::new(attested_path);
    let declared = Path::new(declared_path);
    if attested == declared {
        return true;
    }
    if let Ok(rel) = attested.strip_prefix(root) {
        if rel == declared {
            return true;
        }
    }
    let attested_components = path_components_for_compare(attested);
    let declared_components = path_components_for_compare(declared);
    attested_components.ends_with(&declared_components)
}

#[derive(Debug, Clone, Serialize)]
struct TrustReport {
    schema_version: &'static str,
    tool: ToolInfo,
    invocation: Invocation,
    project: ProjectInfo,
    budgets: Budgets,
    capabilities: Capabilities,
    nondeterminism: Nondeterminism,
    sbom: Sbom,
}

#[derive(Debug, Clone, Serialize)]
struct ToolInfo {
    name: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct Invocation {
    argv: Vec<String>,
    cwd: String,
    started_at_unix_ms: u64,
    project_path: Option<String>,
    profile: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectInfo {
    root: String,
    world: String,
    runner: String,
    module_roots: Vec<String>,
    profile: Option<String>,
    manifest_path: Option<String>,
    lockfile_path: Option<String>,
    stdlib_lock_path: Option<String>,
    arch_root: Option<String>,
    arch_manifest_path: Option<String>,
    policy_base_path: Option<String>,
    policy_effective_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct Budgets {
    caps: BudgetCaps,
    scopes: Vec<BudgetScope>,
    arch_profiles: Vec<ArchBudgetProfileRef>,
    observed: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct BudgetCaps {
    run_profile: RunCaps,
    policy_limits: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct RunCaps {
    solve_fuel: u64,
    max_memory_bytes: u64,
    max_output_bytes: Option<u64>,
    cpu_time_limit_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct BudgetScope {
    kind: String,
    module_id: String,
    #[serde(rename = "fn")]
    fn_name: String,
    ptr: String,
    label: Option<String>,
    mode: Option<String>,
    limits: BTreeMap<String, Option<u64>>,
    arch_profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ArchBudgetProfileRef {
    id: String,
    enforce: String,
    worlds_allowed: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct Capabilities {
    world: String,
    declared: DeclaredCaps,
    used: UsedCaps,
    observed: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct DeclaredCaps {
    policy: Option<Value>,
    arch_world_assignments: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct UsedCaps {
    namespaces: Vec<String>,
    details: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
struct Nondeterminism {
    flags: Vec<NondetFlag>,
}

#[derive(Debug, Clone, Serialize)]
struct NondetFlag {
    kind: String,
    severity: String,
    summary: String,
    details: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
struct Sbom {
    format: String,
    generated: bool,
    path: Option<String>,
    cyclonedx: Option<Value>,
    spdx: Option<Value>,
    components: Vec<SbomComponent>,
}

#[derive(Debug, Clone, Serialize)]
struct SbomComponent {
    kind: String,
    name: String,
    version: Option<String>,
    source: Option<String>,
    purl: Option<String>,
    license: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ObservedBudget {
    fuel_used: Option<u64>,
    heap_used: Option<u64>,
    mem_stats: Option<Value>,
}

#[derive(Debug, Clone)]
struct ProjectContext {
    project_path: Option<PathBuf>,
    root: PathBuf,
    world: WorldId,
    runner: String,
    module_roots: Vec<PathBuf>,
    profile: Option<String>,
    declared_dependencies: Vec<project::DependencySpec>,
    lockfile_path: Option<PathBuf>,
    stdlib_lock_path: Option<PathBuf>,
    arch_root: Option<PathBuf>,
    arch_manifest_path: Option<PathBuf>,
    policy_base_path: Option<PathBuf>,
    policy_effective_path: Option<PathBuf>,
    policy_doc: Option<Value>,
    run_caps: RunCaps,
    arch_world_assignments: Vec<Value>,
    arch_budget_profiles: Vec<ArchBudgetProfileRef>,
    lockfile: Option<project::Lockfile>,
}

#[derive(Debug, Clone, Default)]
struct StaticScan {
    namespaces: BTreeSet<String>,
    op_counts: BTreeMap<String, u64>,
    scopes: Vec<BudgetScope>,
    uses_os_time: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct DepsCapabilityPolicyDefault {
    deny_sensitive_namespaces: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DepsCapabilityPolicyPackage {
    name: String,
    #[serde(default)]
    allow_sensitive_namespaces: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DepsCapabilityPolicy {
    policy_id: String,
    #[serde(default)]
    default: DepsCapabilityPolicyDefault,
    #[serde(default)]
    packages: Vec<DepsCapabilityPolicyPackage>,
}

pub fn cmd_trust(
    machine: &crate::reporting::MachineArgs,
    args: TrustArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing trust subcommand (try --help)");
    };

    match cmd {
        TrustCommand::Report(args) => cmd_trust_report(machine, args),
        TrustCommand::Profile(args) => cmd_trust_profile(machine, args),
        TrustCommand::Capsule(args) => cmd_trust_capsule(machine, args),
        TrustCommand::Certify(args) => cmd_trust_certify(machine, args),
    }
}

fn cmd_trust_profile(
    machine: &crate::reporting::MachineArgs,
    args: TrustProfileArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing trust profile subcommand (try --help)");
    };
    match cmd {
        TrustProfileCommand::Check(args) => cmd_trust_profile_check(machine, args),
    }
}

fn cmd_trust_capsule(
    machine: &crate::reporting::MachineArgs,
    args: TrustCapsuleArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing trust capsule subcommand (try --help)");
    };
    match cmd {
        TrustCapsuleCommand::Check(args) => cmd_trust_capsule_check(machine, args),
        TrustCapsuleCommand::Attest(args) => cmd_trust_capsule_attest(machine, args),
    }
}

fn cmd_trust_capsule_check(
    machine: &crate::reporting::MachineArgs,
    args: TrustCapsuleCheckArgs,
) -> Result<std::process::ExitCode> {
    let project_path = match args.project.as_deref() {
        Some(path) => Some(resolve_project_manifest_arg(path)?),
        None => None,
    };
    let base_dir = project_path
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."));
    let index_path = if args.index.is_absolute() {
        args.index.clone()
    } else {
        base_dir.join(&args.index)
    };

    let mut diagnostics = Vec::new();
    let mut checked = 0u64;
    let index = match load_capsule_index(&index_path) {
        Ok(index) => Some(index),
        Err(err) => {
            diagnostics.push(trust_diag_with_path(
                "X07CAP_INDEX_INVALID",
                format!("{err:#}"),
                &index_path,
            ));
            None
        }
    };

    if let Some(index) = &index {
        let index_root = index_path.parent().unwrap_or_else(|| Path::new("."));
        for capsule in &index.capsules {
            checked += 1;
            let contract_path = index_root.join(&capsule.contract_path);
            let contract = match load_capsule_contract(&contract_path) {
                Ok(contract) => contract,
                Err(err) => {
                    diagnostics.push(trust_diag_with_path(
                        "X07CAP_CONTRACT_INVALID",
                        format!("{err:#}"),
                        &contract_path,
                    ));
                    continue;
                }
            };
            if contract.id != capsule.id {
                diagnostics.push(trust_diag_with_path(
                    "X07CAP_CONTRACT_INVALID",
                    format!(
                        "capsule id mismatch: index has {:?}, contract has {:?}",
                        capsule.id, contract.id
                    ),
                    &contract_path,
                ));
            }
            if contract.effect_log.schema_path.trim().is_empty() {
                diagnostics.push(trust_diag_with_path(
                    "X07CAP_EFFECT_LOG_REQUIRED",
                    "capsule contract is missing effect_log.schema_path",
                    &contract_path,
                ));
            }
            if contract.conformance.tests.is_empty() {
                diagnostics.push(trust_diag_with_path(
                    "X07CAP_CONFORMANCE_REQUIRED",
                    "capsule contract must declare at least one conformance test id",
                    &contract_path,
                ));
            }
            if let Some(network) = &contract.network {
                if network.peer_policy_paths.is_empty() {
                    diagnostics.push(trust_diag_with_path(
                        "X07CAP_PEER_POLICY_REQUIRED",
                        "network capsule contracts must declare at least one peer policy",
                        &contract_path,
                    ));
                }
                if network.conformance_tests.is_empty() {
                    diagnostics.push(trust_diag_with_path(
                        "X07CAP_CONFORMANCE_MISSING",
                        "network capsule contracts must declare conformance_tests",
                        &contract_path,
                    ));
                }
                for peer_policy_path in &network.peer_policy_paths {
                    let peer_policy_abs = index_root.join(peer_policy_path);
                    match load_peer_policy(&peer_policy_abs) {
                        Ok(policy) => {
                            if policy.tls_mode != "none"
                                && policy.ca_paths.is_empty()
                                && policy.spki_sha256.is_empty()
                            {
                                diagnostics.push(trust_diag_with_path(
                                    "X07CAP_TLS_POLICY_INCOMPLETE",
                                    format!(
                                        "peer policy {:?} enables TLS but does not declare trust roots or SPKI pins",
                                        policy.policy_id
                                    ),
                                    &peer_policy_abs,
                                ));
                            }
                        }
                        Err(err) => diagnostics.push(trust_diag_with_path(
                            "X07CAP_PEER_POLICY_REQUIRED",
                            format!("{err:#}"),
                            &peer_policy_abs,
                        )),
                    }
                }
            }
            let attest_path = index_root.join(&capsule.attestation_path);
            match report_common::read_json_file(&attest_path) {
                Ok(doc) => {
                    let schema_diags = report_common::validate_schema(
                        X07_CAPSULE_ATTEST_SCHEMA_BYTES,
                        "spec/x07-capsule.attest.schema.json",
                        &doc,
                    )?;
                    if !schema_diags.is_empty() {
                        diagnostics.push(trust_diag_with_path(
                            "X07CAP_ATTEST_DIGEST_MISMATCH",
                            format!("attestation schema invalid: {}", schema_diags[0].message),
                            &attest_path,
                        ));
                    } else {
                        let actual = format!("sha256:{}", sha256_hex_for_path(&contract_path)?);
                        let got = doc
                            .get("contract_digest")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if got != actual {
                            diagnostics.push(trust_diag_with_path(
                                "X07CAP_ATTEST_DIGEST_MISMATCH",
                                format!(
                                    "contract digest mismatch: expected {:?} got {:?}",
                                    actual, got
                                ),
                                &attest_path,
                            ));
                        }
                        if let Some(network) = &contract.network {
                            let expected_peer_policy_digests = network
                                .peer_policy_paths
                                .iter()
                                .filter_map(|path| {
                                    let abs = index_root.join(path);
                                    if !abs.is_file() {
                                        return None;
                                    }
                                    Some((
                                        path.clone(),
                                        format!("sha256:{}", sha256_hex_for_path(&abs).ok()?),
                                    ))
                                })
                                .collect::<Vec<_>>();
                            let mut got_peer_policy_digests = doc
                                .get("peer_policy_digests")
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default();
                            let peer_policy_match = expected_peer_policy_digests.iter().all(
                                |(expected_path, expected_digest)| {
                                    let maybe_idx =
                                        got_peer_policy_digests.iter().position(|value| {
                                            let got_path = value
                                                .get("path")
                                                .and_then(Value::as_str)
                                                .unwrap_or("");
                                            let got_digest = value
                                                .get("digest")
                                                .and_then(Value::as_str)
                                                .unwrap_or("");
                                            got_digest == expected_digest
                                                && attested_path_matches_declared_path(
                                                    got_path,
                                                    expected_path,
                                                    index_root,
                                                )
                                        });
                                    if let Some(idx) = maybe_idx {
                                        got_peer_policy_digests.remove(idx);
                                        true
                                    } else {
                                        false
                                    }
                                },
                            );
                            if !peer_policy_match || !got_peer_policy_digests.is_empty() {
                                diagnostics.push(trust_diag_with_path(
                                    "X07CAP_PEER_POLICY_REQUIRED",
                                    "capsule attestation peer-policy digests do not match the contract peer-policy set",
                                    &attest_path,
                                ));
                            }
                        }
                    }
                }
                Err(err) => diagnostics.push(trust_diag_with_path(
                    "X07CAP_ATTEST_DIGEST_MISMATCH",
                    format!("{err:#}"),
                    &attest_path,
                )),
            }
        }
    }

    let exit_code = if diagnostics.is_empty() { 0 } else { 20 };
    let value = json!({
        "kind": "x07.trust.capsule.check",
        "ok": diagnostics.is_empty(),
        "index": index_path.display().to_string(),
        "checked_capsules": checked,
        "diagnostics": diagnostics,
        "exit_code": exit_code
    });
    write_machine_json(
        machine,
        &value,
        exit_code,
        &format!(
            "trust capsule check: ok={} checked={checked}",
            diagnostics.is_empty()
        ),
    )
}

pub(crate) fn emit_capsule_attestation(args: &TrustCapsuleAttestArgs) -> Result<Value> {
    let contract_path = util::resolve_existing_path_upwards(&args.contract);
    let contract = load_capsule_contract(&contract_path)?;
    let contract_root = contract_path.parent().unwrap_or_else(|| Path::new("."));
    let mut module_digests = Vec::new();
    for module in &args.module {
        let path = util::resolve_existing_path_upwards(module);
        module_digests.push(CapsuleModuleDigest {
            path: if module.is_absolute() {
                path.display().to_string()
            } else {
                module.display().to_string()
            },
            digest: format!("sha256:{}", sha256_hex_for_path(&path)?),
        });
    }
    module_digests.sort_by(|a, b| a.path.cmp(&b.path));
    let mut peer_policy_digests = Vec::new();
    let (request_contract_digest, response_contract_digest) =
        if let Some(network) = &contract.network {
            for peer_policy_path in &network.peer_policy_paths {
                let path = contract_root.join(peer_policy_path);
                peer_policy_digests.push(CapsuleModuleDigest {
                    path: peer_policy_path.clone(),
                    digest: format!("sha256:{}", sha256_hex_for_path(&path)?),
                });
            }
            peer_policy_digests.sort_by(|a, b| a.path.cmp(&b.path));
            let request_digest = network
                .request_contract_path
                .as_deref()
                .map(|path| contract_root.join(path))
                .map(|path| sha256_hex_for_path(&path).map(|digest| format!("sha256:{digest}")))
                .transpose()?;
            let response_digest = network
                .response_contract_path
                .as_deref()
                .map(|path| contract_root.join(path))
                .map(|path| sha256_hex_for_path(&path).map(|digest| format!("sha256:{digest}")))
                .transpose()?;
            (request_digest, response_digest)
        } else {
            (None, None)
        };

    let doc = CapsuleAttestationDoc {
        schema_version: X07_CAPSULE_ATTEST_SCHEMA_VERSION,
        capsule_id: contract.id,
        contract_digest: format!("sha256:{}", sha256_hex_for_path(&contract_path)?),
        module_digests,
        lockfile_digest: format!(
            "sha256:{}",
            sha256_hex_for_path(&util::resolve_existing_path_upwards(&args.lockfile))?
        ),
        conformance_report_digest: format!(
            "sha256:{}",
            sha256_hex_for_path(&util::resolve_existing_path_upwards(
                &args.conformance_report
            ))?
        ),
        peer_policy_digests,
        request_contract_digest,
        response_contract_digest,
    };
    let value = serde_json::to_value(&doc).context("serialize capsule attestation")?;
    let schema_diags = report_common::validate_schema(
        X07_CAPSULE_ATTEST_SCHEMA_BYTES,
        "spec/x07-capsule.attest.schema.json",
        &value,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "internal error: capsule attestation is not schema-valid: {}",
            schema_diags[0].message
        );
    }
    let bytes = report_common::canonical_pretty_json_bytes(&value)?;
    util::write_atomic(&args.out, &bytes)
        .with_context(|| format!("write capsule attestation: {}", args.out.display()))?;
    Ok(value)
}

fn cmd_trust_capsule_attest(
    machine: &crate::reporting::MachineArgs,
    args: TrustCapsuleAttestArgs,
) -> Result<std::process::ExitCode> {
    let value = emit_capsule_attestation(&args)?;
    write_machine_json(machine, &value, 0, "trust capsule attest: ok")
}

fn cmd_trust_report(
    machine: &crate::reporting::MachineArgs,
    args: TrustReportArgs,
) -> Result<std::process::ExitCode> {
    let out_path = machine
        .out
        .as_ref()
        .context("missing --out <PATH> for trust report")?;
    let cwd = std::env::current_dir().context("get cwd")?;
    let project_path = match args.project.as_deref() {
        Some(p) => Some(util::resolve_existing_path_upwards(p)),
        None => run::discover_project_manifest(&cwd)?,
    };

    let started_at_unix_ms = now_unix_ms();
    let invocation = Invocation {
        argv: std::env::args().collect(),
        cwd: cwd.display().to_string(),
        started_at_unix_ms,
        project_path: project_path.as_ref().map(|p| p.display().to_string()),
        profile: args.profile.clone(),
    };

    let mut strict_issues: Vec<String> = Vec::new();
    let mut ctx = resolve_project_context(project_path.as_deref(), args.profile.as_deref())?;

    if args.strict && ctx.project_path.is_none() {
        strict_issues.push("strict mode requires a project manifest".to_string());
    }

    if args.strict && ctx.world == WorldId::RunOsSandboxed && ctx.policy_effective_path.is_none() {
        strict_issues
            .push("strict mode: run-os-sandboxed requires a resolved policy file".to_string());
    }

    if args.strict
        && ctx.project_path.is_some()
        && ctx.lockfile_path.is_none()
        && ctx
            .lockfile
            .as_ref()
            .is_none_or(|lock| lock.dependencies.is_empty())
    {
        strict_issues.push("strict mode: lockfile missing".to_string());
    }

    let static_scan = scan_module_roots(&ctx.module_roots);

    let fail_on_deps_capability = args.fail_on.contains(&TrustFailOn::DepsCapability);
    let mut deps_cap_diags: Vec<diagnostics::Diagnostic> = Vec::new();
    let mut deps_cap_policy_schema_invalid = false;
    let mut deps_cap_policy_missing = false;
    let mut deps_cap_policy: Option<DepsCapabilityPolicy> = None;

    let deps_declared = !ctx.declared_dependencies.is_empty();
    if ctx.project_path.is_some() && deps_declared {
        let default_policy_path = ctx.root.join("x07.deps.capability-policy.json");
        let policy_path = if let Some(raw) = args.deps_cap_policy.as_deref() {
            if !util::is_safe_rel_path(raw) {
                anyhow::bail!(
                    "--deps-cap-policy must be a safe relative path (no '..', no absolute paths): {:?}",
                    raw
                );
            }
            ctx.root.join(raw)
        } else {
            default_policy_path
        };

        if policy_path.is_file() {
            let doc = report_common::read_json_file(&policy_path)?;
            let schema_diags = report_common::validate_schema(
                X07_DEPS_CAPABILITY_POLICY_SCHEMA_BYTES,
                "spec/x07-deps.capability-policy.schema.json",
                &doc,
            )?;
            if !schema_diags.is_empty() {
                deps_cap_policy_schema_invalid = true;
                deps_cap_diags.extend(schema_diags);
            } else {
                deps_cap_policy = match serde_json::from_value::<DepsCapabilityPolicy>(doc.clone())
                {
                    Ok(parsed) => Some(parsed),
                    Err(err) => {
                        deps_cap_policy_schema_invalid = true;
                        deps_cap_diags.push(reporting::diag_error(
                            "X07-TOOL-EXEC-0001",
                            diagnostics::Stage::Run,
                            &format!("parse deps capability policy JSON: {err}"),
                        ));
                        None
                    }
                };
            }
        } else {
            deps_cap_policy_missing = true;
            let mut diag = reporting::diag_error(
                "W_DEPS_CAP_POLICY_MISSING",
                diagnostics::Stage::Lint,
                "dependency capability policy missing",
            );
            diag.severity = if fail_on_deps_capability {
                diagnostics::Severity::Error
            } else {
                diagnostics::Severity::Warning
            };
            diag.data.insert(
                "expected_path".to_string(),
                Value::String(policy_path.display().to_string()),
            );
            deps_cap_diags.push(diag);
        }

        if let (Some(lock), Some(policy)) = (ctx.lockfile.as_ref(), deps_cap_policy.as_ref()) {
            let deny = normalize_sensitive_namespace_set(&policy.default.deny_sensitive_namespaces);
            for dep in &lock.dependencies {
                let dep_dir = project::resolve_rel_path_with_workspace(&ctx.root, &dep.path)?;
                let module_root = dep_dir.join(&dep.module_root);
                let scan = scan_module_roots(&[module_root]);
                if scan.namespaces.is_empty() {
                    continue;
                }

                let allow = deps_cap_allowlist(policy, &dep.name);
                let denied_effective: BTreeSet<String> = deny.difference(&allow).cloned().collect();
                let offending: BTreeSet<String> = scan
                    .namespaces
                    .intersection(&denied_effective)
                    .cloned()
                    .collect();
                if offending.is_empty() {
                    continue;
                }

                let mut diag = reporting::diag_error(
                    "E_DEPS_CAP_POLICY_DENY",
                    diagnostics::Stage::Lint,
                    &format!(
                        "dependency {:?} uses denied sensitive namespaces",
                        dep.name.as_str()
                    ),
                );
                diag.data.insert(
                    "package".to_string(),
                    json!({
                        "name": dep.name.as_str(),
                        "version": dep.version.as_str(),
                        "path": dep.path.as_str(),
                    }),
                );
                diag.data.insert(
                    "offending_namespaces".to_string(),
                    Value::Array(offending.into_iter().map(Value::String).collect()),
                );
                diag.data.insert(
                    "policy".to_string(),
                    json!({
                        "path": policy_path.display().to_string(),
                        "policy_id": policy.policy_id.as_str(),
                        "rule_ptr": "/default/deny_sensitive_namespaces"
                    }),
                );
                deps_cap_diags.push(diag);
            }
        }
    }
    if !deps_cap_diags.is_empty() {
        for diag in &deps_cap_diags {
            eprintln!("{}: {}", diag.code, diag.message);
        }
    }

    let mut observed_budget = ObservedBudget::default();
    let mut observed_caps = serde_json::Map::new();

    for path in &args.run_report {
        let abs = util::resolve_existing_path_upwards(path);
        let doc = report_common::read_json_file(&abs)
            .with_context(|| format!("load --run-report {}", abs.display()))?;
        merge_observed_from_report(&doc, &mut observed_budget, &mut observed_caps);
    }

    for path in &args.bundle_report {
        let abs = util::resolve_existing_path_upwards(path);
        let doc = report_common::read_json_file(&abs)
            .with_context(|| format!("load --bundle-report {}", abs.display()))?;
        if let Some(policy) = doc.get("bundle").and_then(|b| b.get("policy")) {
            if let Some(base) = policy.get("base_policy").and_then(Value::as_str) {
                ctx.policy_base_path = Some(PathBuf::from(base));
            }
            if let Some(effective) = policy.get("effective_policy").and_then(Value::as_str) {
                ctx.policy_effective_path = Some(PathBuf::from(effective));
            }
            if let Some(keys) = policy.get("embedded_env_keys").and_then(Value::as_array) {
                observed_caps.insert("embedded_env_keys".to_string(), Value::Array(keys.clone()));
            }
        }
        merge_observed_from_report(&doc, &mut observed_budget, &mut observed_caps);
    }

    for path in &args.x07test {
        let abs = util::resolve_existing_path_upwards(path);
        let doc = report_common::read_json_file(&abs)
            .with_context(|| format!("load --x07test {}", abs.display()))?;
        merge_observed_from_x07test(&doc, &mut observed_budget, &mut observed_caps);
    }

    let declared_policy = ctx.policy_doc.clone().map(policy_subset_for_report);

    let mut used_namespaces: Vec<String> = static_scan.namespaces.into_iter().collect();
    used_namespaces.sort();

    let used_details = static_scan
        .op_counts
        .into_iter()
        .map(|(k, v)| (k, Value::from(v)))
        .collect();

    let mut caps_observed = if observed_caps.is_empty() {
        None
    } else {
        Some(Value::Object(observed_caps))
    };

    if caps_observed.is_none() && ctx.world.is_eval_world() {
        caps_observed = Some(json!({"mode":"deterministic-eval"}));
    }

    let mut flags = Vec::new();
    if !ctx.world.is_eval_world() {
        flags.push(NondetFlag {
            kind: "world_non_deterministic".to_string(),
            severity: "high".to_string(),
            summary: format!("world {} can observe host OS state", ctx.world.as_str()),
            details: BTreeMap::new(),
        });
    }

    if let Some(policy) = &ctx.policy_doc {
        if policy
            .pointer("/language/allow_unsafe")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            flags.push(NondetFlag {
                kind: "allow_unsafe".to_string(),
                severity: "high".to_string(),
                summary: "policy enables language.allow_unsafe".to_string(),
                details: BTreeMap::new(),
            });
        }
        if policy
            .pointer("/language/allow_ffi")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            flags.push(NondetFlag {
                kind: "allow_ffi".to_string(),
                severity: "high".to_string(),
                summary: "policy enables language.allow_ffi".to_string(),
                details: BTreeMap::new(),
            });
        }
        if policy
            .pointer("/net/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            flags.push(NondetFlag {
                kind: "net_enabled".to_string(),
                severity: "warn".to_string(),
                summary: "policy enables network access".to_string(),
                details: BTreeMap::new(),
            });
        }
        if policy
            .pointer("/process/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            flags.push(NondetFlag {
                kind: "process_enabled".to_string(),
                severity: "warn".to_string(),
                summary: "policy enables process spawning".to_string(),
                details: BTreeMap::new(),
            });
        }
        if policy
            .pointer("/env/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            flags.push(NondetFlag {
                kind: "os_env".to_string(),
                severity: "warn".to_string(),
                summary: "policy enables environment access".to_string(),
                details: BTreeMap::new(),
            });
        }
        let allow_wall_clock = policy
            .pointer("/time/allow_wall_clock")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if allow_wall_clock && static_scan.uses_os_time {
            flags.push(NondetFlag {
                kind: "os_time".to_string(),
                severity: "warn".to_string(),
                summary: "code calls std.os.time.* while wall clock is allowed".to_string(),
                details: BTreeMap::new(),
            });
        }
    }

    flags.sort_by(|a, b| a.kind.cmp(&b.kind));

    let observed_budget_json = observed_budget_to_value(&observed_budget);

    let mut sbom_components = Vec::new();
    sbom_components.push(SbomComponent {
        kind: "toolchain".to_string(),
        name: "x07".to_string(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        source: None,
        purl: None,
        license: None,
    });

    if let Some(lock) = &ctx.lockfile {
        for dep in &lock.dependencies {
            sbom_components.push(SbomComponent {
                kind: "package".to_string(),
                name: dep.name.clone(),
                version: Some(dep.version.clone()),
                source: Some(dep.path.clone()),
                purl: None,
                license: None,
            });
        }
    }
    if let Some(stdlib_lock_path) = ctx.stdlib_lock_path.as_deref() {
        sbom_components.extend(stdlib_sbom_components(stdlib_lock_path));
    }
    sbom_components.sort_by(|a, b| {
        (
            a.kind.as_str(),
            a.name.as_str(),
            a.version.as_deref().unwrap_or(""),
        )
            .cmp(&(
                b.kind.as_str(),
                b.name.as_str(),
                b.version.as_deref().unwrap_or(""),
            ))
    });

    let mut sbom_diags: Vec<diagnostics::Diagnostic> = Vec::new();
    let sbom = build_sbom(&args, out_path, sbom_components, &mut sbom_diags);
    if !sbom_diags.is_empty() {
        for diag in &sbom_diags {
            eprintln!("{}: {}", diag.code, diag.message);
        }
    }

    let report = TrustReport {
        schema_version: X07_TRUST_REPORT_SCHEMA_VERSION,
        tool: ToolInfo {
            name: "x07".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            build: None,
        },
        invocation,
        project: ProjectInfo {
            root: ctx.root.display().to_string(),
            world: ctx.world.as_str().to_string(),
            runner: ctx.runner,
            module_roots: ctx
                .module_roots
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            profile: ctx.profile,
            manifest_path: ctx.project_path.as_ref().map(|p| p.display().to_string()),
            lockfile_path: ctx.lockfile_path.as_ref().map(|p| p.display().to_string()),
            stdlib_lock_path: ctx
                .stdlib_lock_path
                .as_ref()
                .map(|p| p.display().to_string()),
            arch_root: ctx.arch_root.as_ref().map(|p| p.display().to_string()),
            arch_manifest_path: ctx
                .arch_manifest_path
                .as_ref()
                .map(|p| p.display().to_string()),
            policy_base_path: ctx
                .policy_base_path
                .as_ref()
                .map(|p| p.display().to_string()),
            policy_effective_path: ctx
                .policy_effective_path
                .as_ref()
                .map(|p| p.display().to_string()),
        },
        budgets: Budgets {
            caps: BudgetCaps {
                run_profile: ctx.run_caps,
                policy_limits: ctx
                    .policy_doc
                    .as_ref()
                    .and_then(policy_limits_subset_for_report),
            },
            scopes: static_scan.scopes,
            arch_profiles: ctx.arch_budget_profiles,
            observed: observed_budget_json,
        },
        capabilities: Capabilities {
            world: ctx.world.as_str().to_string(),
            declared: DeclaredCaps {
                policy: declared_policy,
                arch_world_assignments: ctx.arch_world_assignments,
            },
            used: UsedCaps {
                namespaces: used_namespaces,
                details: used_details,
            },
            observed: caps_observed,
        },
        nondeterminism: Nondeterminism { flags },
        sbom,
    };

    let report_value = serde_json::to_value(&report)?;
    let schema_diags = report_common::validate_schema(
        X07_TRUST_REPORT_SCHEMA_BYTES,
        "spec/x07-trust.report.schema.json",
        &report_value,
    )?;

    if !schema_diags.is_empty() {
        strict_issues.push("generated report is not schema-valid".to_string());
    }

    let fail_on_triggered = trust_fail_on_triggered(&report, &args.fail_on);
    let deps_cap_violation = deps_cap_diags
        .iter()
        .any(|d| d.code == "E_DEPS_CAP_POLICY_DENY");
    let deps_cap_strict_triggered =
        args.strict && (deps_cap_policy_schema_invalid || deps_cap_violation);
    let deps_cap_fail_on_triggered = fail_on_deps_capability
        && (deps_cap_policy_schema_invalid || deps_cap_violation || deps_cap_policy_missing);

    if args.strict && deps_cap_policy_schema_invalid {
        strict_issues.push("strict mode: deps capability policy schema invalid".to_string());
    }

    let json_bytes = report_common::canonical_pretty_json_bytes(&report_value)?;
    util::write_atomic(out_path, &json_bytes)
        .with_context(|| format!("write trust report: {}", out_path.display()))?;

    if let Some(html_out) = &args.html_out {
        let html = render_trust_html(&report, &strict_issues, &schema_diags, &deps_cap_diags);
        util::write_atomic(html_out, html.as_bytes())
            .with_context(|| format!("write trust html: {}", html_out.display()))?;
    }

    if !schema_diags.is_empty()
        || fail_on_triggered
        || deps_cap_fail_on_triggered
        || deps_cap_strict_triggered
        || (args.strict && !strict_issues.is_empty())
    {
        for issue in &strict_issues {
            eprintln!("x07 trust: {issue}");
        }
        for diag in &schema_diags {
            eprintln!("{}: {}", diag.code, diag.message);
        }

        if std::env::var_os("X07_TOOL_API_CHILD").is_some() {
            let mut diags = Vec::new();
            for issue in &strict_issues {
                diags.push(reporting::diag_error(
                    "X07-TOOL-EXEC-0001",
                    diagnostics::Stage::Lint,
                    issue,
                ));
            }
            diags.extend(schema_diags.clone());
            diags.extend(deps_cap_diags.clone());
            diags.extend(sbom_diags);

            let report = diagnostics::Report {
                schema_version: X07DIAG_SCHEMA_VERSION.to_string(),
                ok: false,
                diagnostics: diags,
                meta: BTreeMap::new(),
            };
            let doc = serde_json::to_value(&report)?;
            let bytes = report_common::canonical_pretty_json_bytes(&doc)?;
            std::io::stdout().write_all(&bytes)?;
        }
        return Ok(std::process::ExitCode::from(20));
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn resolve_project_context(
    project_path: Option<&Path>,
    profile: Option<&str>,
) -> Result<ProjectContext> {
    if let Some(project_path) = project_path {
        let project_path = project_path.to_path_buf();
        let manifest = project::load_project_manifest(&project_path)
            .with_context(|| format!("load project: {}", project_path.display()))?;
        let root = project_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        let profiles_file = run::load_project_profiles(&project_path)?;
        let selected_profile =
            run::resolve_selected_profile(Some(&project_path), Some(&profiles_file), profile)?;

        let world = if let Some(sel) = &selected_profile {
            sel.world
        } else {
            x07c::world_config::parse_world_id(&manifest.world)
                .with_context(|| format!("invalid project world {:?}", manifest.world))?
        };
        let runner = if world.is_eval_world() { "host" } else { "os" }.to_string();

        let run_caps = RunCaps {
            solve_fuel: selected_profile
                .as_ref()
                .and_then(|p| p.solve_fuel)
                .unwrap_or(DEFAULT_SOLVE_FUEL),
            max_memory_bytes: selected_profile
                .as_ref()
                .and_then(|p| p.max_memory_bytes)
                .map(|v| v as u64)
                .unwrap_or(DEFAULT_MAX_MEMORY_BYTES),
            max_output_bytes: selected_profile
                .as_ref()
                .and_then(|p| p.max_output_bytes)
                .map(|v| v as u64),
            cpu_time_limit_seconds: selected_profile
                .as_ref()
                .and_then(|p| p.cpu_time_limit_seconds),
        };

        let lockfile_path = project::default_lockfile_path(&project_path, &manifest);
        let lockfile = if lockfile_path.is_file() {
            let bytes = std::fs::read(&lockfile_path)
                .with_context(|| format!("read lockfile: {}", lockfile_path.display()))?;
            let lock: project::Lockfile = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse lockfile JSON: {}", lockfile_path.display()))?;
            if !PROJECT_LOCKFILE_SCHEMA_VERSIONS_SUPPORTED
                .iter()
                .any(|v| *v == lock.schema_version.trim())
            {
                anyhow::bail!(
                    "lockfile schema_version mismatch: expected one of {:?} got {:?}",
                    PROJECT_LOCKFILE_SCHEMA_VERSIONS_SUPPORTED,
                    lock.schema_version
                );
            }
            Some(lock)
        } else if manifest.dependencies.is_empty() {
            Some(project::Lockfile {
                schema_version: PROJECT_LOCKFILE_SCHEMA_VERSION.to_string(),
                dependencies: Vec::new(),
            })
        } else {
            None
        };

        let module_roots = if let Some(lock) = &lockfile {
            project::collect_module_roots(&project_path, &manifest, lock)
                .context("collect module roots")?
        } else {
            manifest
                .module_roots
                .iter()
                .map(|r| root.join(r))
                .collect::<Vec<PathBuf>>()
        };

        let stdlib_lock_path = {
            let p = root.join("stdlib.lock");
            if p.is_file() {
                Some(p)
            } else {
                None
            }
        };

        let arch_root = {
            let p = root.join("arch");
            if p.is_dir() {
                Some(p)
            } else {
                None
            }
        };
        let arch_manifest_path = arch_root
            .as_ref()
            .map(|p| p.join("manifest.x07arch.json"))
            .filter(|p| p.is_file());

        let mut arch_world_assignments = Vec::new();
        if let Some(path) = &arch_manifest_path {
            if let Ok(doc) = report_common::read_json_file(path) {
                if let Some(nodes) = doc.get("nodes").and_then(Value::as_array) {
                    for node in nodes {
                        let Some(node_id) = node.get("id").and_then(Value::as_str) else {
                            continue;
                        };
                        let Some(node_world) = node.get("world").and_then(Value::as_str) else {
                            continue;
                        };
                        arch_world_assignments.push(json!({
                            "node_id": node_id,
                            "world": node_world
                        }));
                    }
                }
            }
        }
        arch_world_assignments.sort_by(|a, b| {
            a.get("node_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .cmp(b.get("node_id").and_then(Value::as_str).unwrap_or(""))
        });

        let mut arch_budget_profiles = Vec::new();
        if let Some(arch_root) = &arch_root {
            let p = arch_root.join("budgets/index.x07budgets.json");
            if p.is_file() {
                if let Ok(doc) = report_common::read_json_file(&p) {
                    if let Some(profiles) = doc.get("profiles").and_then(Value::as_array) {
                        for profile in profiles {
                            let Some(id) = profile.get("id").and_then(Value::as_str) else {
                                continue;
                            };
                            let enforce = profile
                                .get("enforce")
                                .and_then(Value::as_str)
                                .unwrap_or("off")
                                .to_string();
                            let worlds_allowed = profile
                                .get("worlds_allowed")
                                .and_then(Value::as_array)
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(Value::as_str)
                                        .map(str::to_string)
                                        .collect::<Vec<String>>()
                                })
                                .unwrap_or_default();
                            arch_budget_profiles.push(ArchBudgetProfileRef {
                                id: id.to_string(),
                                enforce,
                                worlds_allowed,
                            });
                        }
                    }
                }
            }
        }
        arch_budget_profiles.sort_by(|a, b| a.id.cmp(&b.id));

        let mut policy_base_path = None;
        let mut policy_effective_path = None;
        let mut policy_doc = None;

        if world == WorldId::RunOsSandboxed {
            let profile_policy = selected_profile.as_ref().and_then(|p| p.policy.clone());
            let resolution = crate::policy_overrides::resolve_policy_for_world(
                world,
                &root,
                None,
                profile_policy,
                &PolicyOverrides::default(),
            )?;

            match resolution {
                PolicyResolution::None => {}
                PolicyResolution::Base(base) => {
                    policy_base_path = Some(base.clone());
                    policy_effective_path = Some(base.clone());
                    policy_doc = Some(report_common::read_json_file(&base)?);
                }
                PolicyResolution::Derived { derived } => {
                    policy_base_path = Some(derived.clone());
                    policy_effective_path = Some(derived.clone());
                    policy_doc = Some(report_common::read_json_file(&derived)?);
                }
                PolicyResolution::SchemaInvalid(errors) => {
                    anyhow::bail!("invalid sandbox policy schema: {}", errors.join("; "));
                }
            }
        }

        Ok(ProjectContext {
            project_path: Some(project_path),
            root,
            world,
            runner,
            module_roots,
            profile: profile.map(str::to_string),
            declared_dependencies: manifest.dependencies.clone(),
            lockfile_path: if lockfile_path.is_file() {
                Some(lockfile_path)
            } else {
                None
            },
            stdlib_lock_path,
            arch_root,
            arch_manifest_path,
            policy_base_path,
            policy_effective_path,
            policy_doc,
            run_caps,
            arch_world_assignments,
            arch_budget_profiles,
            lockfile,
        })
    } else {
        Ok(ProjectContext {
            project_path: None,
            root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            world: WorldId::SolvePure,
            runner: "host".to_string(),
            module_roots: Vec::new(),
            profile: profile.map(str::to_string),
            declared_dependencies: Vec::new(),
            lockfile_path: None,
            stdlib_lock_path: None,
            arch_root: None,
            arch_manifest_path: None,
            policy_base_path: None,
            policy_effective_path: None,
            policy_doc: None,
            run_caps: RunCaps {
                solve_fuel: DEFAULT_SOLVE_FUEL,
                max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES,
                max_output_bytes: None,
                cpu_time_limit_seconds: None,
            },
            arch_world_assignments: Vec::new(),
            arch_budget_profiles: Vec::new(),
            lockfile: None,
        })
    }
}

fn scan_module_roots(module_roots: &[PathBuf]) -> StaticScan {
    let mut out = StaticScan::default();

    for root in module_roots {
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if !path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".x07.json"))
            {
                continue;
            }
            let Ok(doc) = report_common::read_json_file(path) else {
                continue;
            };
            let module_id = doc
                .get("module_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let doc_kind = doc
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            let Some(decls) = doc.get("decls").and_then(Value::as_array) else {
                if doc_kind == "entry" {
                    if let Some(solve) = doc.get("solve") {
                        let scan = report_common::scan_sensitive(solve);
                        out.uses_os_time = out.uses_os_time || scan.uses_os_time;
                        for ns in scan.namespaces {
                            out.namespaces.insert(ns);
                        }
                        for (op, count) in scan.op_counts {
                            *out.op_counts.entry(op).or_insert(0) += count;
                        }
                        for hit in scan.budget_scopes {
                            out.scopes.push(BudgetScope {
                                kind: hit.kind,
                                module_id: module_id.clone(),
                                fn_name: "solve".to_string(),
                                ptr: format!("/solve{}", hit.ptr),
                                label: hit.label,
                                mode: hit.mode,
                                limits: hit.limits,
                                arch_profile_id: hit.arch_profile_id,
                            });
                        }
                    }
                }
                continue;
            };

            for (didx, decl) in decls.iter().enumerate() {
                let Some(kind) = decl.get("kind").and_then(Value::as_str) else {
                    continue;
                };
                if kind != "defn" && kind != "defasync" {
                    continue;
                }
                let fn_name = decl
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let Some(body) = decl.get("body") else {
                    continue;
                };

                let scan = report_common::scan_sensitive(body);
                out.uses_os_time = out.uses_os_time || scan.uses_os_time;
                for ns in scan.namespaces {
                    out.namespaces.insert(ns);
                }
                for (op, count) in scan.op_counts {
                    *out.op_counts.entry(op).or_insert(0) += count;
                }

                for hit in scan.budget_scopes {
                    out.scopes.push(BudgetScope {
                        kind: hit.kind,
                        module_id: module_id.clone(),
                        fn_name: fn_name.clone(),
                        ptr: if hit.ptr.is_empty() {
                            format!("/decls/{didx}/body")
                        } else {
                            format!("/decls/{didx}/body{}", hit.ptr)
                        },
                        label: hit.label,
                        mode: hit.mode,
                        limits: hit.limits,
                        arch_profile_id: hit.arch_profile_id,
                    });
                }
            }

            if doc_kind == "entry" {
                if let Some(solve) = doc.get("solve") {
                    let scan = report_common::scan_sensitive(solve);
                    out.uses_os_time = out.uses_os_time || scan.uses_os_time;
                    for ns in scan.namespaces {
                        out.namespaces.insert(ns);
                    }
                    for (op, count) in scan.op_counts {
                        *out.op_counts.entry(op).or_insert(0) += count;
                    }
                    for hit in scan.budget_scopes {
                        out.scopes.push(BudgetScope {
                            kind: hit.kind,
                            module_id: module_id.clone(),
                            fn_name: "solve".to_string(),
                            ptr: format!("/solve{}", hit.ptr),
                            label: hit.label,
                            mode: hit.mode,
                            limits: hit.limits,
                            arch_profile_id: hit.arch_profile_id,
                        });
                    }
                }
            }
        }
    }

    out.scopes.sort_by(|a, b| {
        (a.module_id.as_str(), a.fn_name.as_str(), a.ptr.as_str()).cmp(&(
            b.module_id.as_str(),
            b.fn_name.as_str(),
            b.ptr.as_str(),
        ))
    });

    out
}

fn merge_observed_from_report(
    doc: &Value,
    observed_budget: &mut ObservedBudget,
    observed_caps: &mut serde_json::Map<String, Value>,
) {
    let mut candidate = doc;
    if let Some(inner) = doc.get("report") {
        candidate = inner;
    }

    if let Some(solve) = candidate.get("solve") {
        merge_solve_section(solve, observed_budget, observed_caps);
    } else {
        merge_solve_section(candidate, observed_budget, observed_caps);
    }
}

fn merge_observed_from_x07test(
    doc: &Value,
    observed_budget: &mut ObservedBudget,
    observed_caps: &mut serde_json::Map<String, Value>,
) {
    if let Some(tests) = doc.get("tests").and_then(Value::as_array) {
        let mut worlds: BTreeSet<String> = BTreeSet::new();
        for test in tests {
            if let Some(world) = test.get("world").and_then(Value::as_str) {
                worlds.insert(world.to_string());
            }
            if let Some(run) = test.get("run") {
                if let Some(fuel) = run.get("fuel_used").and_then(Value::as_u64) {
                    observed_budget.fuel_used =
                        Some(observed_budget.fuel_used.unwrap_or(0).max(fuel));
                }
                if let Some(mem_stats) = run.get("mem_stats") {
                    observed_budget.mem_stats = Some(mem_stats.clone());
                }
            }
        }
        if !worlds.is_empty() {
            observed_caps.insert(
                "x07test_worlds".to_string(),
                Value::Array(worlds.into_iter().map(Value::String).collect()),
            );
        }
    }
}

fn merge_solve_section(
    solve: &Value,
    observed_budget: &mut ObservedBudget,
    observed_caps: &mut serde_json::Map<String, Value>,
) {
    if let Some(fuel) = solve.get("fuel_used").and_then(Value::as_u64) {
        observed_budget.fuel_used = Some(observed_budget.fuel_used.unwrap_or(0).max(fuel));
    }
    if let Some(heap) = solve.get("heap_used").and_then(Value::as_u64) {
        observed_budget.heap_used = Some(observed_budget.heap_used.unwrap_or(0).max(heap));
    }
    if let Some(mem_stats) = solve.get("mem_stats") {
        observed_budget.mem_stats = Some(mem_stats.clone());
    }

    for key in [
        "fs_read_file_calls",
        "fs_list_dir_calls",
        "rr_open_calls",
        "rr_close_calls",
        "rr_stats_calls",
        "rr_next_calls",
        "rr_next_miss_calls",
        "rr_append_calls",
        "kv_get_calls",
        "kv_set_calls",
    ] {
        if let Some(v) = solve.get(key).and_then(Value::as_u64) {
            observed_caps.insert(key.to_string(), Value::from(v));
        }
    }
}

fn observed_budget_to_value(observed: &ObservedBudget) -> Option<Value> {
    let (Some(fuel_used), Some(heap_used), Some(mem_stats)) = (
        observed.fuel_used,
        observed.heap_used,
        observed.mem_stats.as_ref(),
    ) else {
        return None;
    };

    Some(json!({
        "fuel_used": fuel_used,
        "heap_used": heap_used,
        "mem_stats": mem_stats
    }))
}

fn policy_subset_for_report(policy: Value) -> Value {
    json!({
        "fs": {
            "enabled": policy.pointer("/fs/enabled").and_then(Value::as_bool).unwrap_or(false),
            "read_roots": policy.pointer("/fs/read_roots").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
            "write_roots": policy.pointer("/fs/write_roots").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        },
        "net": {
            "enabled": policy.pointer("/net/enabled").and_then(Value::as_bool).unwrap_or(false),
            "allow_dns": policy.pointer("/net/allow_dns").cloned().unwrap_or(Value::Null),
            "allow_tcp": policy.pointer("/net/allow_tcp").cloned().unwrap_or(Value::Null),
            "allow_udp": policy.pointer("/net/allow_udp").cloned().unwrap_or(Value::Null),
            "allow_hosts": policy.pointer("/net/allow_hosts").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        },
        "env": {
            "enabled": policy.pointer("/env/enabled").and_then(Value::as_bool).unwrap_or(false),
            "allow_keys": policy.pointer("/env/allow_keys").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
            "deny_keys": policy.pointer("/env/deny_keys").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        },
        "time": {
            "enabled": policy.pointer("/time/enabled").and_then(Value::as_bool).unwrap_or(false),
            "allow_monotonic": policy.pointer("/time/allow_monotonic").cloned().unwrap_or(Value::Null),
            "allow_wall_clock": policy.pointer("/time/allow_wall_clock").cloned().unwrap_or(Value::Null),
            "allow_sleep": policy.pointer("/time/allow_sleep").cloned().unwrap_or(Value::Null),
            "max_sleep_ms": policy.pointer("/time/max_sleep_ms").cloned().unwrap_or(Value::Null),
            "allow_local_tzid": policy.pointer("/time/allow_local_tzid").cloned().unwrap_or(Value::Null),
        },
        "process": {
            "enabled": policy.pointer("/process/enabled").and_then(Value::as_bool).unwrap_or(false),
            "allow_spawn": policy.pointer("/process/allow_spawn").cloned().unwrap_or(Value::Null),
            "allow_exec": policy.pointer("/process/allow_exec").cloned().unwrap_or(Value::Null),
            "allow_exit": policy.pointer("/process/allow_exit").cloned().unwrap_or(Value::Null),
            "allow_execs": policy.pointer("/process/allow_execs").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
            "allow_exec_prefixes": policy
                .pointer("/process/allow_exec_prefixes")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
        },
        "language": {
            "allow_unsafe": policy.pointer("/language/allow_unsafe").cloned().unwrap_or(Value::Null),
            "allow_ffi": policy.pointer("/language/allow_ffi").cloned().unwrap_or(Value::Null),
        }
    })
}

fn policy_limits_subset_for_report(policy: &Value) -> Option<Value> {
    let limits = policy.pointer("/limits")?;
    Some(json!({
        "cpu_ms": limits.get("cpu_ms").cloned().unwrap_or(Value::Null),
        "wall_ms": limits.get("wall_ms").cloned().unwrap_or(Value::Null),
        "mem_bytes": limits.get("mem_bytes").cloned().unwrap_or(Value::Null),
        "fds": limits.get("fds").cloned().unwrap_or(Value::Null),
        "procs": limits.get("procs").cloned().unwrap_or(Value::Null)
    }))
}

fn trust_fail_on_triggered(report: &TrustReport, fail_on: &[TrustFailOn]) -> bool {
    for flag in fail_on {
        match flag {
            TrustFailOn::AllowUnsafe => {
                if report
                    .nondeterminism
                    .flags
                    .iter()
                    .any(|f| f.kind == "allow_unsafe")
                {
                    return true;
                }
            }
            TrustFailOn::AllowFfi => {
                if report
                    .nondeterminism
                    .flags
                    .iter()
                    .any(|f| f.kind == "allow_ffi")
                {
                    return true;
                }
            }
            TrustFailOn::NetEnabled => {
                if report
                    .nondeterminism
                    .flags
                    .iter()
                    .any(|f| f.kind == "net_enabled")
                {
                    return true;
                }
            }
            TrustFailOn::ProcessEnabled => {
                if report
                    .nondeterminism
                    .flags
                    .iter()
                    .any(|f| f.kind == "process_enabled")
                {
                    return true;
                }
            }
            TrustFailOn::Nondeterminism => {
                if !report.nondeterminism.flags.is_empty() {
                    return true;
                }
            }
            TrustFailOn::SbomMissing => {
                if !report.sbom.generated || report.sbom.path.is_none() {
                    return true;
                }
            }
            TrustFailOn::DepsCapability => {}
        }
    }
    false
}

fn render_trust_html(
    report: &TrustReport,
    strict_issues: &[String],
    schema_diags: &[diagnostics::Diagnostic],
    deps_cap_diags: &[diagnostics::Diagnostic],
) -> String {
    let mut s = String::new();
    s.push_str("<!doctype html>\n<html><head><meta charset=\"utf-8\">");
    s.push_str("<title>x07 trust report</title>");
    s.push_str("<style>body{font-family:system-ui,Segoe UI,Helvetica,Arial,sans-serif;margin:24px;line-height:1.45}code,pre{background:#f6f8fa;padding:2px 4px;border-radius:4px}pre{padding:12px;overflow:auto}details{margin:12px 0}table{border-collapse:collapse}td,th{padding:6px 8px;border:1px solid #ddd}h2{margin-top:28px}</style>");
    s.push_str("</head><body>");
    s.push_str("<h1>x07 trust report</h1>");
    s.push_str("<p><b>world:</b> <code>");
    s.push_str(&report_common::html_escape(&report.project.world));
    s.push_str("</code> <b>runner:</b> <code>");
    s.push_str(&report_common::html_escape(&report.project.runner));
    s.push_str("</code></p>");

    s.push_str("<h2>Budget Caps</h2><pre>");
    let caps = serde_json::to_value(&report.budgets.caps).unwrap_or(Value::Null);
    let caps_bytes =
        report_common::canonical_pretty_json_bytes(&caps).unwrap_or_else(|_| b"{}\n".to_vec());
    s.push_str(&report_common::html_escape(
        String::from_utf8_lossy(&caps_bytes).as_ref(),
    ));
    s.push_str("</pre>");

    s.push_str("<h2>Capabilities</h2><pre>");
    let caps = serde_json::to_value(&report.capabilities).unwrap_or(Value::Null);
    let caps_bytes =
        report_common::canonical_pretty_json_bytes(&caps).unwrap_or_else(|_| b"{}\n".to_vec());
    s.push_str(&report_common::html_escape(
        String::from_utf8_lossy(&caps_bytes).as_ref(),
    ));
    s.push_str("</pre>");

    s.push_str("<h2>Nondeterminism Flags</h2><pre>");
    let flags = serde_json::to_value(&report.nondeterminism).unwrap_or(Value::Null);
    let flags_bytes =
        report_common::canonical_pretty_json_bytes(&flags).unwrap_or_else(|_| b"{}\n".to_vec());
    s.push_str(&report_common::html_escape(
        String::from_utf8_lossy(&flags_bytes).as_ref(),
    ));
    s.push_str("</pre>");

    s.push_str("<h2>SBOM</h2>");
    if report.sbom.generated {
        if let Some(path) = report.sbom.path.as_deref() {
            s.push_str("<p><b>generated:</b> <code>");
            s.push_str(&report_common::html_escape(path));
            s.push_str("</code></p>");
        }
    }
    s.push_str("<pre>");
    let sbom = serde_json::to_value(&report.sbom).unwrap_or(Value::Null);
    let sbom_bytes =
        report_common::canonical_pretty_json_bytes(&sbom).unwrap_or_else(|_| b"{}\n".to_vec());
    s.push_str(&report_common::html_escape(
        String::from_utf8_lossy(&sbom_bytes).as_ref(),
    ));
    s.push_str("</pre>");

    if !strict_issues.is_empty() {
        s.push_str("<h2>Strict Issues</h2><ul>");
        for issue in strict_issues {
            s.push_str("<li>");
            s.push_str(&report_common::html_escape(issue));
            s.push_str("</li>");
        }
        s.push_str("</ul>");
    }

    if !schema_diags.is_empty() {
        s.push_str("<h2>Schema Diagnostics</h2><ul>");
        for diag in schema_diags {
            s.push_str("<li><code>");
            s.push_str(&report_common::html_escape(&diag.code));
            s.push_str("</code>: ");
            s.push_str(&report_common::html_escape(&diag.message));
            s.push_str("</li>");
        }
        s.push_str("</ul>");
    }

    if !deps_cap_diags.is_empty() {
        s.push_str("<h2>Dependency Capability Policy Diagnostics</h2><ul>");
        for diag in deps_cap_diags {
            s.push_str("<li><code>");
            s.push_str(&report_common::html_escape(&diag.code));
            s.push_str("</code>: ");
            s.push_str(&report_common::html_escape(&diag.message));
            s.push_str("</li>");
        }
        s.push_str("</ul>");
    }

    s.push_str("<details><summary>Raw JSON</summary><pre>");
    let raw = serde_json::to_value(report).unwrap_or(Value::Null);
    let raw_bytes =
        report_common::canonical_pretty_json_bytes(&raw).unwrap_or_else(|_| b"{}\n".to_vec());
    s.push_str(&report_common::html_escape(
        String::from_utf8_lossy(&raw_bytes).as_ref(),
    ));
    s.push_str("</pre></details>");

    s.push_str("</body></html>\n");
    s
}

#[derive(Debug, Clone, Default)]
struct LanguageFeatureScan {
    has_defasync: bool,
    has_extern: bool,
}

#[derive(Debug)]
struct ToolRunOutcome {
    exit_code: i32,
    stderr: Vec<u8>,
}

fn cmd_trust_profile_check(
    machine: &crate::reporting::MachineArgs,
    args: TrustProfileCheckArgs,
) -> Result<std::process::ExitCode> {
    let profile_path = util::resolve_existing_path_upwards(&args.profile);
    let mut diagnostics = Vec::new();

    let profile = match load_trust_profile(&profile_path) {
        Ok(profile) => Some(profile),
        Err(err) => {
            diagnostics.push(trust_diag_with_path(
                "X07TP_INVALID",
                format!("{err:#}"),
                &profile_path,
            ));
            None
        }
    };

    let project_path = match args.project.as_deref() {
        Some(path) => match resolve_project_manifest_arg(path) {
            Ok(path) => Some(path),
            Err(err) => {
                diagnostics.push(trust_diag_with_path(
                    "X07TP_PROJECT_MISMATCH",
                    format!("{err:#}"),
                    path,
                ));
                None
            }
        },
        None => None,
    };

    if let Some(profile) = &profile {
        diagnostics.extend(validate_trust_profile_strength(profile));
        if let Some(entry) = args.entry.as_deref() {
            if !profile
                .entrypoints
                .iter()
                .any(|candidate| candidate == entry)
            {
                diagnostics.push(trust_diag(
                    "X07TP_ENTRY_FORBIDDEN",
                    format!(
                        "entry {entry:?} is not allowed by trust profile {}",
                        profile.id
                    ),
                ));
            }
        }

        if let Some(project_path) = project_path.as_deref() {
            let ctx = resolve_project_context(Some(project_path), None)?;
            diagnostics.extend(validate_profile_against_context(
                profile,
                project_path,
                &ctx,
                args.entry.as_deref(),
            )?);
        }
    }

    let exit_code = if diagnostics.is_empty() { 0 } else { 20 };
    let report = TrustProfileCheckReport {
        schema_version: "x07.trust.profile.check@0.1.0",
        ok: diagnostics.is_empty(),
        profile: profile
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| profile_path.display().to_string()),
        project: project_path.as_ref().map(|p| p.display().to_string()),
        entry: args.entry,
        diagnostics,
        exit_code,
    };

    let value = serde_json::to_value(&report).context("serialize trust profile check report")?;
    write_machine_json(
        machine,
        &value,
        exit_code,
        &format!(
            "trust profile check: ok={} diagnostics={}",
            report.ok,
            report.diagnostics.len()
        ),
    )
}

fn cmd_trust_certify(
    machine: &crate::reporting::MachineArgs,
    args: TrustCertifyArgs,
) -> Result<std::process::ExitCode> {
    let cwd = std::env::current_dir().context("resolve current working directory")?;
    let mut diagnostics = Vec::new();
    let project_path = match resolve_project_manifest_arg(&args.project) {
        Ok(path) => Some(path),
        Err(err) => {
            diagnostics.push(trust_diag_with_path(
                "X07TC_EPROJECT",
                format!("{err:#}"),
                &args.project,
            ));
            None
        }
    };
    let project_root = project_path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cwd.clone());
    let out_dir = if args.out_dir.is_absolute() {
        args.out_dir.clone()
    } else {
        cwd.join(&args.out_dir)
    };
    let bundle_dir = out_dir.join("bundle");
    let profile_path = util::resolve_existing_path_upwards(&args.profile);
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("create certificate dir: {}", out_dir.display()))?;
    std::fs::create_dir_all(&bundle_dir)
        .with_context(|| format!("create bundle dir: {}", bundle_dir.display()))?;

    let profile = match load_trust_profile(&profile_path) {
        Ok(profile) => Some(profile),
        Err(err) => {
            diagnostics.push(trust_diag_with_path(
                "X07TC_EPROFILE",
                format!("{err:#}"),
                &profile_path,
            ));
            None
        }
    };

    let ctx = match project_path.as_deref() {
        Some(project_path) => match resolve_project_context(Some(project_path), None) {
            Ok(ctx) => Some(ctx),
            Err(err) => {
                diagnostics.push(trust_diag(
                    "X07TC_EPROJECT",
                    format!("resolve project context: {err:#}"),
                ));
                None
            }
        },
        None => None,
    };
    if let (Some(profile), Some(project_path), Some(ctx)) =
        (&profile, project_path.as_deref(), &ctx)
    {
        diagnostics.extend(validate_trust_profile_strength(profile));
        diagnostics.extend(validate_profile_against_context(
            profile,
            project_path,
            ctx,
            Some(&args.entry),
        )?);
    }

    let tests_manifest = if args.tests_manifest.is_absolute() {
        args.tests_manifest.clone()
    } else {
        project_root.join(&args.tests_manifest)
    };
    if let Some(baseline) = args.baseline.as_deref() {
        let resolved = util::resolve_existing_path_upwards(baseline);
        if !resolved.exists() {
            diagnostics.push(trust_diag_with_path(
                "X07TC_EDIFF_POSTURE",
                "review baseline path is missing",
                &resolved,
            ));
        }
    }
    let default_bundle_out = bundle_dir.join(util::safe_artifact_dir_name(&args.entry));
    let mut operational_entry_symbol = args.entry.clone();
    let (
        boundaries_ref,
        coverage_ref,
        verify_summary_ref,
        imported_summary_inventory,
        schema_derive_refs,
        proof_inventory,
        tests_ref,
        trust_report_ref,
        compile_attestation_ref,
        review_diff_ref,
        dependency_closure,
        dependency_closure_ref,
        package_set_digest,
        bundle_path,
    ) = if let Some(project_path) = project_path.as_deref() {
        if let Ok(manifest) = project::load_project_manifest(project_path) {
            if let Some(symbol) = manifest.operational_entry_symbol.clone() {
                operational_entry_symbol = symbol.clone();
                if profile.as_ref().is_some_and(is_strong_trust_profile) && args.entry != symbol {
                    diagnostics.push(trust_diag(
                        "X07TC_EOP_ENTRY_MISMATCH",
                        format!(
                            "strong trust profiles require --entry to match project.operational_entry_symbol ({symbol:?})"
                        ),
                    ));
                }
            } else if profile.as_ref().is_some_and(is_strong_trust_profile) {
                diagnostics.push(trust_diag(
                    "X07TC_EOP_ENTRY_REQUIRED",
                    "strong trust profiles require project.operational_entry_symbol",
                ));
            }
            if profile.as_ref().is_some_and(is_strong_trust_profile)
                && manifest
                    .certification_entry_symbol
                    .as_ref()
                    .is_some_and(|symbol| {
                        manifest
                            .operational_entry_symbol
                            .as_ref()
                            .is_some_and(|op| symbol != op)
                    })
            {
                diagnostics.push(trust_diag(
                    "X07TC_ESURROGATE_ENTRY_FORBIDDEN",
                    "strong trust profiles reject developer-only certification surrogate entries",
                ));
            }
        }
        let (boundaries_ref, boundaries_doc) =
            build_boundaries_evidence(&project_root, &out_dir, &mut diagnostics)?;
        if let Some(profile) = &profile {
            if profile.evidence_requirements.require_smoke_harnesses
                && boundaries_doc
                    .pointer("/summary/missing_smoke")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    > 0
            {
                diagnostics.push(trust_diag(
                    "X07TC_ETESTS",
                    "boundary coverage is missing required smoke test declarations",
                ));
            }
            if profile.evidence_requirements.require_pbt.trim() != "none"
                && boundaries_doc
                    .pointer("/summary/missing_pbt")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    > 0
            {
                diagnostics.push(trust_diag(
                    "X07TC_EPBT",
                    "boundary coverage is missing required property-test declarations",
                ));
            }
        }
        let boundary_requirements = match load_boundary_requirements(&project_root) {
            Ok(requirements) => requirements,
            Err(err) => {
                diagnostics.push(trust_diag(
                    "X07TC_EBOUNDARY_MISSING",
                    format!("load boundary requirements: {err:#}"),
                ));
                BoundaryEvidenceRequirements::default()
            }
        };

        let (
            coverage_ref,
            coverage_doc,
            prove_targets,
            verify_summary_ref,
            imported_summary_inventory,
        ) = build_coverage_evidence(
            project_path,
            &args.entry,
            &project_root,
            profile.as_ref(),
            &out_dir,
            &mut diagnostics,
        )?;
        add_coverage_diagnostics(
            &coverage_doc,
            &project_root,
            profile.as_ref(),
            &mut diagnostics,
        );
        let schema_derive_refs = if profile
            .as_ref()
            .is_some_and(|p| p.evidence_requirements.require_schema_derive_check)
        {
            build_schema_derive_evidence(
                &project_root,
                &boundary_requirements,
                &out_dir,
                &mut diagnostics,
            )?
        } else {
            Vec::new()
        };
        let proof_inventory =
            build_prove_evidence(project_path, &prove_targets, &out_dir, &mut diagnostics)?;
        let tests_ref = build_tests_evidence(
            &project_root,
            &tests_manifest,
            profile.as_ref(),
            &boundary_requirements,
            &out_dir,
            &mut diagnostics,
        )?;
        let trust_report_ref = build_trust_report_evidence(
            project_path,
            profile.as_ref(),
            &out_dir,
            &mut diagnostics,
        )?;
        let (compile_attestation_ref, bundle_path) = build_bundle_evidence(
            project_path,
            &args.entry,
            Some(
                args.bundle_out
                    .as_deref()
                    .unwrap_or(default_bundle_out.as_path()),
            ),
            &out_dir,
            &mut diagnostics,
        )?;
        let review_diff_ref = build_review_evidence(
            args.baseline.as_deref(),
            &project_root,
            &out_dir,
            &mut diagnostics,
        )?;
        let (dependency_closure, dependency_closure_ref, package_set_digest) =
            build_dependency_closure_evidence(
                project_path,
                profile.as_ref(),
                &out_dir,
                &mut diagnostics,
            )?;

        if let Some(profile) = &profile {
            if profile.evidence_requirements.require_compile_attestation
                && bundle_path.as_ref().is_none_or(|path| !path.is_file())
            {
                diagnostics.push(trust_diag(
                    "X07TC_ECOMPILE_ATTEST",
                    "compile attestation bundle step did not produce a native executable",
                ));
            }
        }

        (
            boundaries_ref,
            coverage_ref,
            verify_summary_ref,
            imported_summary_inventory,
            schema_derive_refs,
            proof_inventory,
            tests_ref,
            trust_report_ref,
            compile_attestation_ref,
            review_diff_ref,
            dependency_closure,
            dependency_closure_ref,
            package_set_digest,
            bundle_path,
        )
    } else {
        (
            write_stub_artifact(
                &out_dir.join("boundaries.report.json"),
                "boundaries_report",
                "project resolution failed before evidence collection",
            )?,
            write_stub_artifact(
                &out_dir.join("verify.coverage.json"),
                "coverage_report",
                "project resolution failed before evidence collection",
            )?,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            write_stub_artifact(
                &out_dir.join("tests.report.json"),
                "tests_report",
                "project resolution failed before evidence collection",
            )?,
            write_stub_artifact(
                &out_dir.join("trust.report.json"),
                "trust_report",
                "project resolution failed before evidence collection",
            )?,
            write_stub_artifact(
                &out_dir.join("compile.attest.json"),
                "compile_attestation",
                "project resolution failed before evidence collection",
            )?,
            if args.baseline.is_some() {
                Some(write_stub_artifact(
                    &out_dir.join("review.diff.json"),
                    "review_diff",
                    "project resolution failed before evidence collection",
                )?)
            } else {
                None
            },
            None,
            None,
            None,
            None,
        )
    };

    let async_proof = collect_async_proof_summary(
        &proof_inventory,
        Path::new(&coverage_ref.path),
        &project_root,
        profile.as_ref(),
    )?;
    let recursive_proof_summary =
        collect_recursive_proof_summary(&proof_inventory, Path::new(&coverage_ref.path))?;
    let proof_assumptions = collect_proof_assumptions(&proof_inventory)?;
    let accepted_depends_on_bounded_proof =
        recursive_proof_summary.accepted_depends_on_bounded_proof;
    let accepted_depends_on_dev_only_assumption = proof_assumptions
        .iter()
        .any(|assumption| !assumption.certifiable);
    if profile
        .as_ref()
        .is_some_and(|p| !p.evidence_requirements.allow_coverage_summary_imports)
        && !imported_summary_inventory.is_empty()
    {
        diagnostics.push(trust_diag(
            "X07TC_ECOVERAGE_ONLY",
            "strong trust profiles reject imported summaries on the coverage-only evidence path",
        ));
    }
    if profile
        .as_ref()
        .is_some_and(|p| p.evidence_requirements.require_async_proof_coverage)
        && async_proof.proved < async_proof.reachable
    {
        diagnostics.push(trust_diag(
            "X07TC_EASYNC_PROOF",
            format!(
                "async proof coverage is incomplete: proved {} of {} reachable defasync symbol(s)",
                async_proof.proved, async_proof.reachable
            ),
        ));
    }
    if profile.as_ref().is_some_and(is_strong_trust_profile) && accepted_depends_on_bounded_proof {
        diagnostics.push(trust_diag(
            "X07TC_EBOUNDED_PROOF_FORBIDDEN",
            "strong trust profiles reject accepted proofs that depend on bounded recursion",
        ));
    }
    if profile.as_ref().is_some_and(is_strong_trust_profile)
        && accepted_depends_on_dev_only_assumption
    {
        diagnostics.push(trust_diag(
            "X07TC_EDEV_ONLY_ASSUMPTION",
            "strong trust profiles reject developer-only proof assumptions",
        ));
    }
    let capsules = collect_capsule_artifacts(&project_root, profile.as_ref(), &mut diagnostics)?;
    let (runtime, runtime_attestation) = collect_runtime_attestation(
        Path::new(&tests_ref.path),
        &project_root,
        ctx.as_ref(),
        profile.as_ref(),
        &mut diagnostics,
    )?;
    let formal_verification_scope = collect_formal_verification_scope_summary(
        &proof_inventory,
        Path::new(&coverage_ref.path),
        &operational_entry_symbol,
        runtime.as_ref(),
    )?;
    let verdict_accepted = diagnostics.is_empty();

    let certificate = TrustCertificate {
        schema_version: X07_TRUST_CERTIFICATE_SCHEMA_VERSION,
        verdict: if verdict_accepted {
            "accepted".to_string()
        } else {
            "rejected".to_string()
        },
        profile: profile
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| profile_path.display().to_string()),
        entry: args.entry,
        operational_entry_symbol,
        out_dir: out_dir.display().to_string(),
        claims: realized_certificate_claims(
            profile.as_ref().map(|p| p.claims.as_slice()).unwrap_or(&[]),
            verdict_accepted,
            formal_verification_scope.proved_symbol_count,
            formal_verification_scope.entry_body_formally_proved,
        ),
        formal_verification_scope: formal_verification_scope.formal_verification_scope,
        proved_symbol_count: formal_verification_scope.proved_symbol_count,
        proved_defn_count: formal_verification_scope.proved_defn_count,
        proved_defasync_count: formal_verification_scope.proved_defasync_count,
        entry_body_formally_proved: formal_verification_scope.entry_body_formally_proved,
        operational_entry_proof_inventory_refs: formal_verification_scope
            .operational_entry_proof_inventory_refs,
        capsule_boundary_only_symbol_count: formal_verification_scope
            .capsule_boundary_only_symbol_count,
        runtime_evidence_only_symbol_count: formal_verification_scope
            .runtime_evidence_only_symbol_count,
        async_proof,
        proof_inventory: proof_inventory.clone(),
        proof_assumptions: proof_assumptions.clone(),
        recursive_proof_summary,
        imported_summary_inventory,
        accepted_depends_on_bounded_proof,
        accepted_depends_on_dev_only_assumption,
        capsules: capsules.capsules,
        network_capsules: capsules.network_capsules,
        runtime,
        package_set_digest,
        dependency_closure,
        effect_logs: capsules.effect_logs.clone(),
        tcb: build_certificate_tcb(),
        evidence: TrustCertificateEvidence {
            boundaries_report: boundaries_ref,
            coverage_report: coverage_ref,
            verify_summary_report: verify_summary_ref,
            schema_derive_reports: schema_derive_refs,
            prove_reports: proof_inventory
                .iter()
                .map(|item| item.verify_report.clone())
                .collect(),
            tests_report: tests_ref,
            trust_report: trust_report_ref,
            compile_attestation: compile_attestation_ref,
            runtime_attestation,
            peer_policy_files: capsules.peer_policies,
            capsule_attestations: capsules.capsule_attestations,
            effect_logs: capsules.effect_logs,
            review_diff: review_diff_ref,
            dependency_closure_attestation: dependency_closure_ref,
            bundle_path: bundle_path.map(|path| path.display().to_string()),
        },
        diagnostics,
    };

    write_certificate_bundle(&out_dir, &certificate, !args.no_html)?;
    let value = serde_json::to_value(&certificate).context("serialize trust certificate JSON")?;
    write_machine_json(
        machine,
        &value,
        if certificate.verdict == "accepted" {
            0
        } else {
            20
        },
        &format!(
            "trust certify: verdict={} diagnostics={}",
            certificate.verdict,
            certificate.diagnostics.len()
        ),
    )
}

fn write_certificate_bundle(
    out_dir: &Path,
    certificate: &TrustCertificate,
    emit_html: bool,
) -> Result<()> {
    let certificate_path = out_dir.join("certificate.json");
    let summary_path = out_dir.join("summary.html");
    let value = serde_json::to_value(certificate).context("serialize trust certificate JSON")?;
    let schema_diags = report_common::validate_schema(
        X07_TRUST_CERTIFICATE_SCHEMA_BYTES,
        "spec/x07-trust.certificate.schema.json",
        &value,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "internal error: trust certificate JSON is not schema-valid: {}",
            schema_diags[0].message
        );
    }
    let bytes = report_common::canonical_pretty_json_bytes(&value)?;
    util::write_atomic(&certificate_path, &bytes)
        .with_context(|| format!("write certificate: {}", certificate_path.display()))?;
    if emit_html {
        let html = render_certificate_summary(certificate);
        util::write_atomic(&summary_path, html.as_bytes())
            .with_context(|| format!("write certificate summary: {}", summary_path.display()))?;
    }
    Ok(())
}

fn write_machine_json(
    machine: &crate::reporting::MachineArgs,
    value: &Value,
    exit_code: u8,
    text_fallback: &str,
) -> Result<std::process::ExitCode> {
    let bytes = report_common::canonical_pretty_json_bytes(value)?;
    if let Some(path) = machine.out.as_deref() {
        util::write_atomic(path, &bytes)
            .with_context(|| format!("write output: {}", path.display()))?;
    }
    if let Some(path) = machine.report_out.as_deref() {
        reporting::write_bytes(path, &bytes)?;
    }
    if machine.quiet_json {
        return Ok(std::process::ExitCode::from(exit_code));
    }
    if matches!(machine.json, Some(crate::reporting::JsonArg::Off)) {
        println!("{text_fallback}");
    } else {
        std::io::stdout()
            .write_all(&bytes)
            .context("write stdout")?;
    }
    Ok(std::process::ExitCode::from(exit_code))
}

fn render_certificate_summary(certificate: &TrustCertificate) -> String {
    let mut s = String::new();
    s.push_str("<!doctype html>\n<html><head><meta charset=\"utf-8\">");
    s.push_str("<title>x07 trust certificate</title>");
    s.push_str("<style>body{font-family:system-ui,Segoe UI,Helvetica,Arial,sans-serif;margin:24px;line-height:1.45}code,pre{background:#f6f8fa;padding:2px 4px;border-radius:4px}pre{padding:12px;overflow:auto}table{border-collapse:collapse}td,th{padding:6px 8px;border:1px solid #ddd}h2{margin-top:28px}</style>");
    s.push_str("</head><body>");
    s.push_str("<h1>x07 trust certificate</h1>");
    s.push_str("<p><b>verdict:</b> <code>");
    s.push_str(&report_common::html_escape(&certificate.verdict));
    s.push_str("</code></p>");
    s.push_str("<p><b>profile:</b> <code>");
    s.push_str(&report_common::html_escape(&certificate.profile));
    s.push_str("</code> <b>entry:</b> <code>");
    s.push_str(&report_common::html_escape(&certificate.entry));
    s.push_str("</code></p>");

    s.push_str("<h2>Formal Verification Scope</h2><table><thead><tr><th>Field</th><th>Value</th></tr></thead><tbody>");
    for (label, value) in [
        (
            "formal verification scope",
            certificate.formal_verification_scope.as_str(),
        ),
        (
            "entry body formally proved",
            if certificate.entry_body_formally_proved {
                "true"
            } else {
                "false"
            },
        ),
    ] {
        s.push_str("<tr><td>");
        s.push_str(label);
        s.push_str("</td><td><code>");
        s.push_str(&report_common::html_escape(value));
        s.push_str("</code></td></tr>");
    }
    s.push_str("<tr><td>proved symbols</td><td><code>");
    s.push_str(&report_common::html_escape(format!(
        "total={} defn={} defasync={}",
        certificate.proved_symbol_count,
        certificate.proved_defn_count,
        certificate.proved_defasync_count
    )));
    s.push_str("</code></td></tr>");
    s.push_str("<tr><td>capsule-boundary-only symbols</td><td><code>");
    s.push_str(&report_common::html_escape(
        certificate.capsule_boundary_only_symbol_count.to_string(),
    ));
    s.push_str("</code></td></tr>");
    s.push_str("<tr><td>runtime-evidence-only symbols</td><td><code>");
    s.push_str(&report_common::html_escape(
        certificate.runtime_evidence_only_symbol_count.to_string(),
    ));
    s.push_str("</code></td></tr>");
    s.push_str("<tr><td>operational entry proof refs</td><td><code>");
    s.push_str(&report_common::html_escape(
        certificate
            .operational_entry_proof_inventory_refs
            .iter()
            .map(|evidence| evidence.path.clone())
            .collect::<Vec<_>>()
            .join(", "),
    ));
    s.push_str("</code></td></tr>");
    s.push_str("</tbody></table>");

    s.push_str("<h2>TCB</h2><table><thead><tr><th>Field</th><th>Value</th></tr></thead><tbody>");
    for (label, value) in [
        ("x07 version", certificate.tcb.x07_version.as_str()),
        ("host compiler", certificate.tcb.host_compiler.as_str()),
        (
            "trusted primitive manifest digest",
            certificate.tcb.trusted_primitive_manifest_digest.as_str(),
        ),
    ] {
        s.push_str("<tr><td>");
        s.push_str(label);
        s.push_str("</td><td><code>");
        s.push_str(&report_common::html_escape(value));
        s.push_str("</code></td></tr>");
    }
    s.push_str("</tbody></table>");

    s.push_str(
        "<h2>Assurance</h2><table><thead><tr><th>Area</th><th>Value</th></tr></thead><tbody>",
    );
    s.push_str("<tr><td>async proof</td><td><code>");
    s.push_str(&report_common::html_escape(format!(
        "{}/{} reachable",
        certificate.async_proof.proved, certificate.async_proof.reachable
    )));
    s.push_str("</code></td></tr>");
    s.push_str("<tr><td>capsules</td><td><code>");
    s.push_str(&report_common::html_escape(format!(
        "{} [{}]",
        certificate.capsules.count,
        certificate.capsules.ids.join(", ")
    )));
    s.push_str("</code></td></tr>");
    s.push_str("<tr><td>network capsules</td><td><code>");
    s.push_str(&report_common::html_escape(format!(
        "{} [{}]",
        certificate.network_capsules.count,
        certificate.network_capsules.ids.join(", ")
    )));
    s.push_str("</code></td></tr>");
    s.push_str("<tr><td>recursive proof</td><td><code>");
    s.push_str(&report_common::html_escape(format!(
        "reachable={} accepted={} bounded={} unbounded={} imported={} rejected={}",
        certificate.recursive_proof_summary.reachable_recursive_defn,
        certificate.recursive_proof_summary.accepted_recursive_defn,
        certificate.recursive_proof_summary.bounded_recursive_defn,
        certificate.recursive_proof_summary.unbounded_recursive_defn,
        certificate
            .recursive_proof_summary
            .imported_proof_summary_defn,
        certificate.recursive_proof_summary.rejected_recursive_defn
    )));
    s.push_str("</code></td></tr>");
    if let Some(runtime) = &certificate.runtime {
        s.push_str("<tr><td>runtime</td><td><code>");
        s.push_str(&report_common::html_escape(format!(
            "backend={} network_mode={} network_enforcement={} weaker_isolation={}",
            runtime.backend,
            runtime.network_mode,
            runtime.network_enforcement,
            runtime.weaker_isolation
        )));
        s.push_str("</code></td></tr>");
    }
    if let Some(package_set_digest) = &certificate.package_set_digest {
        s.push_str("<tr><td>package set digest</td><td><code>");
        s.push_str(&report_common::html_escape(package_set_digest));
        s.push_str("</code></td></tr>");
    }
    if let Some(dependency_closure) = &certificate.dependency_closure {
        s.push_str("<tr><td>dependency closure</td><td><code>");
        s.push_str(&report_common::html_escape(format!(
            "packages={} advisory_check_ok={}",
            dependency_closure.packages.join(", "),
            dependency_closure.advisory_check_ok
        )));
        s.push_str("</code></td></tr>");
    }
    if !certificate.imported_summary_inventory.is_empty() {
        s.push_str("<tr><td>imported summaries</td><td><code>");
        s.push_str(&report_common::html_escape(format!(
            "{} artifact(s)",
            certificate.imported_summary_inventory.len()
        )));
        s.push_str("</code></td></tr>");
    }
    s.push_str("</tbody></table>");

    s.push_str("<h2>Evidence</h2><table><thead><tr><th>Artifact</th><th>Path</th><th>sha256</th></tr></thead><tbody>");
    for (label, evidence) in [
        ("boundaries", &certificate.evidence.boundaries_report),
        ("coverage", &certificate.evidence.coverage_report),
        ("tests", &certificate.evidence.tests_report),
        ("trust report", &certificate.evidence.trust_report),
        (
            "compile attestation",
            &certificate.evidence.compile_attestation,
        ),
    ] {
        s.push_str("<tr><td>");
        s.push_str(label);
        s.push_str("</td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    if let Some(evidence) = &certificate.evidence.verify_summary_report {
        s.push_str("<tr><td>verify summary</td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    for evidence in &certificate.evidence.schema_derive_reports {
        s.push_str("<tr><td>schema derive</td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    for evidence in &certificate.evidence.prove_reports {
        s.push_str("<tr><td>prove</td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    if let Some(review) = &certificate.evidence.review_diff {
        s.push_str("<tr><td>review diff</td><td><code>");
        s.push_str(&report_common::html_escape(&review.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&review.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    if let Some(runtime_attestation) = &certificate.evidence.runtime_attestation {
        s.push_str("<tr><td>runtime attestation</td><td><code>");
        s.push_str(&report_common::html_escape(&runtime_attestation.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&runtime_attestation.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    for evidence in &certificate.evidence.peer_policy_files {
        s.push_str("<tr><td>peer policy</td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    for evidence in &certificate.evidence.capsule_attestations {
        s.push_str("<tr><td>capsule attestation</td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    for evidence in &certificate.evidence.effect_logs {
        s.push_str("<tr><td>effect log</td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    if let Some(evidence) = &certificate.evidence.dependency_closure_attestation {
        s.push_str("<tr><td>dependency closure</td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.path));
        s.push_str("</code></td><td><code>");
        s.push_str(&report_common::html_escape(&evidence.sha256_hex));
        s.push_str("</code></td></tr>");
    }
    s.push_str("</tbody></table>");

    if !certificate.imported_summary_inventory.is_empty() {
        s.push_str("<h2>Imported Summary Inventory</h2><table><thead><tr><th>Path</th><th>sha256</th><th>Symbols</th></tr></thead><tbody>");
        for entry in &certificate.imported_summary_inventory {
            s.push_str("<tr><td><code>");
            s.push_str(&report_common::html_escape(&entry.path));
            s.push_str("</code></td><td><code>");
            s.push_str(&report_common::html_escape(&entry.sha256_hex));
            s.push_str("</code></td><td><code>");
            s.push_str(&report_common::html_escape(entry.symbols.join(", ")));
            s.push_str("</code></td></tr>");
        }
        s.push_str("</tbody></table>");
    }

    if !certificate.diagnostics.is_empty() {
        s.push_str("<h2>Diagnostics</h2><ul>");
        for diag in &certificate.diagnostics {
            s.push_str("<li><code>");
            s.push_str(&report_common::html_escape(&diag.code));
            s.push_str("</code>: ");
            s.push_str(&report_common::html_escape(&diag.message));
            s.push_str("</li>");
        }
        s.push_str("</ul>");
    }

    s.push_str("</body></html>\n");
    s
}

fn load_trust_profile(path: &Path) -> Result<TrustProfile> {
    let doc = report_common::read_json_file(path)?;
    let schema_diags = report_common::validate_schema(
        X07_TRUST_PROFILE_SCHEMA_BYTES,
        "spec/x07-trust.profile.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!("trust profile schema invalid: {}", schema_diags[0].message);
    }
    let profile: TrustProfile =
        serde_json::from_value(doc).with_context(|| format!("parse {}", path.display()))?;
    if profile.schema_version.trim() != X07_TRUST_PROFILE_SCHEMA_VERSION {
        anyhow::bail!(
            "trust profile schema_version mismatch: expected {:?} got {:?}",
            X07_TRUST_PROFILE_SCHEMA_VERSION,
            profile.schema_version
        );
    }
    Ok(profile)
}

fn load_capsule_index(path: &Path) -> Result<CapsuleIndex> {
    let doc = report_common::read_json_file(path)?;
    let schema_diags = report_common::validate_schema(
        X07_CAPSULE_INDEX_SCHEMA_BYTES,
        "spec/x07-capsule.index.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!("capsule index schema invalid: {}", schema_diags[0].message);
    }
    let index: CapsuleIndex =
        serde_json::from_value(doc).with_context(|| format!("parse {}", path.display()))?;
    if index.schema_version.trim() != "x07.capsule.index@0.1.0" {
        anyhow::bail!(
            "capsule index schema_version mismatch: expected {:?} got {:?}",
            "x07.capsule.index@0.1.0",
            index.schema_version
        );
    }
    Ok(index)
}

fn load_capsule_contract(path: &Path) -> Result<CapsuleContract> {
    let doc = report_common::read_json_file(path)?;
    let schema_diags = report_common::validate_schema(
        X07_CAPSULE_CONTRACT_SCHEMA_BYTES,
        "spec/x07-capsule.contract.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "capsule contract schema invalid: {}",
            schema_diags[0].message
        );
    }
    let contract: CapsuleContract =
        serde_json::from_value(doc).with_context(|| format!("parse {}", path.display()))?;
    if contract.schema_version.trim() != X07_CAPSULE_CONTRACT_SCHEMA_VERSION {
        anyhow::bail!(
            "capsule contract schema_version mismatch: expected {:?} got {:?}",
            X07_CAPSULE_CONTRACT_SCHEMA_VERSION,
            contract.schema_version
        );
    }
    Ok(contract)
}

fn load_peer_policy(path: &Path) -> Result<PeerPolicyDoc> {
    let doc = report_common::read_json_file(path)?;
    let schema_diags = report_common::validate_schema(
        X07_PEER_POLICY_SCHEMA_BYTES,
        "spec/x07-peer.policy.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!("peer policy schema invalid: {}", schema_diags[0].message);
    }
    let policy: PeerPolicyDoc =
        serde_json::from_value(doc).with_context(|| format!("parse {}", path.display()))?;
    if policy.schema_version.trim() != X07_PEER_POLICY_SCHEMA_VERSION {
        anyhow::bail!(
            "peer policy schema_version mismatch: expected {:?} got {:?}",
            X07_PEER_POLICY_SCHEMA_VERSION,
            policy.schema_version
        );
    }
    Ok(policy)
}

fn load_dep_closure_attestation(path: &Path) -> Result<DepClosureAttestationDoc> {
    let doc = report_common::read_json_file(path)?;
    let schema_diags = report_common::validate_schema(
        X07_DEP_CLOSURE_ATTEST_SCHEMA_BYTES,
        "spec/x07-dep.closure.attest.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "dependency closure attestation schema invalid: {}",
            schema_diags[0].message
        );
    }
    let attestation: DepClosureAttestationDoc =
        serde_json::from_value(doc).with_context(|| format!("parse {}", path.display()))?;
    if attestation.schema_version.trim() != X07_DEP_CLOSURE_ATTEST_SCHEMA_VERSION {
        anyhow::bail!(
            "dependency closure schema_version mismatch: expected {:?} got {:?}",
            X07_DEP_CLOSURE_ATTEST_SCHEMA_VERSION,
            attestation.schema_version
        );
    }
    Ok(attestation)
}

fn load_runtime_attestation_doc(path: &Path) -> Result<RuntimeAttestationDoc> {
    let doc = report_common::read_json_file(path)?;
    let schema_diags = report_common::validate_schema(
        X07_RUNTIME_ATTEST_SCHEMA_BYTES,
        "spec/x07-runtime.attest.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "runtime attestation schema invalid: {}",
            schema_diags[0].message
        );
    }
    let attestation: RuntimeAttestationDoc =
        serde_json::from_value(doc).with_context(|| format!("parse {}", path.display()))?;
    if attestation.schema_version.trim() != X07_RUNTIME_ATTEST_SCHEMA_VERSION {
        anyhow::bail!(
            "runtime attestation schema_version mismatch: expected {:?} got {:?}",
            X07_RUNTIME_ATTEST_SCHEMA_VERSION,
            attestation.schema_version
        );
    }
    Ok(attestation)
}

fn default_capsule_index_path(project_root: &Path) -> PathBuf {
    project_root.join("arch/capsules/index.x07capsule.json")
}

fn resolve_report_artifact_path(report_path: &Path, project_root: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        return candidate;
    }
    let report_dir = report_path.parent().unwrap_or_else(|| Path::new("."));
    let report_relative = report_dir.join(&candidate);
    if report_relative.exists() {
        return report_relative;
    }
    project_root.join(candidate)
}

fn policy_allow_hosts_from_doc(doc: Option<&Value>) -> Vec<TrustCertificateNetHost> {
    let Some(hosts) = doc
        .and_then(|doc| doc.pointer("/net/allow_hosts"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut out = hosts
        .iter()
        .filter_map(|host| {
            let host_name = host.get("host").and_then(Value::as_str)?.to_string();
            let ports = host
                .get("ports")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_u64)
                .filter_map(|port| u16::try_from(port).ok())
                .collect::<Vec<_>>();
            if host_name.is_empty() || ports.is_empty() {
                return None;
            }
            Some(TrustCertificateNetHost {
                host: host_name,
                ports,
            })
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| {
        (a.host.as_str(), a.ports.as_slice()).cmp(&(b.host.as_str(), b.ports.as_slice()))
    });
    out
}

fn collect_async_proof_summary(
    proof_inventory: &[TrustCertificateProofInventoryItem],
    coverage_path: &Path,
    project_root: &Path,
    profile: Option<&TrustProfile>,
) -> Result<TrustCertificateAsyncProof> {
    if !coverage_path.is_file() {
        return Ok(TrustCertificateAsyncProof {
            reachable: 0,
            proved: 0,
            model: None,
        });
    }

    let doc = report_common::read_json_file(coverage_path)?;
    let mut reachable = 0u64;
    let user_only = profile_requires_user_proof_scope(profile);
    if let Some(functions) = doc.get("functions").and_then(Value::as_array) {
        for function in functions {
            if function.get("kind").and_then(Value::as_str) != Some("defasync") {
                continue;
            }
            if function.get("status").and_then(Value::as_str) != Some("supported_async") {
                continue;
            }
            if !coverage_function_in_proof_scope(function, project_root, user_only) {
                continue;
            }
            reachable += 1;
        }
    }

    Ok(TrustCertificateAsyncProof {
        reachable,
        proved: proof_inventory
            .iter()
            .filter(|item| item.kind == "defasync" && item.result_kind == "proven_async")
            .count() as u64,
        model: doc
            .pointer("/summary/async_model")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn collect_recursive_proof_summary(
    proof_inventory: &[TrustCertificateProofInventoryItem],
    coverage_path: &Path,
) -> Result<TrustCertificateRecursiveProofSummary> {
    if !coverage_path.is_file() {
        return Ok(TrustCertificateRecursiveProofSummary {
            reachable_recursive_defn: 0,
            accepted_recursive_defn: 0,
            bounded_recursive_defn: 0,
            unbounded_recursive_defn: 0,
            imported_proof_summary_defn: 0,
            rejected_recursive_defn: 0,
            accepted_depends_on_bounded_proof: false,
        });
    }

    let doc = report_common::read_json_file(coverage_path)?;
    let mut bounded_recursive_defn = 0u64;
    let mut unbounded_recursive_defn = 0u64;
    let mut accepted_recursive_defn = 0u64;
    for item in proof_inventory {
        let proof_summary_doc = report_common::read_json_file(Path::new(&item.proof_summary.path))?;
        let recursion_kind = proof_summary_doc
            .get("recursion_kind")
            .and_then(Value::as_str)
            .unwrap_or("none");
        if recursion_kind == "none" {
            continue;
        }
        accepted_recursive_defn += 1;
        match proof_summary_doc
            .get("recursion_bound_kind")
            .and_then(Value::as_str)
            .unwrap_or("none")
        {
            "unbounded" => unbounded_recursive_defn += 1,
            "bounded_by_unwind" => bounded_recursive_defn += 1,
            _ => {}
        }
    }
    Ok(TrustCertificateRecursiveProofSummary {
        reachable_recursive_defn: doc
            .pointer("/summary/recursive_defn")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        accepted_recursive_defn,
        bounded_recursive_defn,
        unbounded_recursive_defn,
        imported_proof_summary_defn: doc
            .pointer("/summary/imported_proof_summary_defn")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        rejected_recursive_defn: doc
            .pointer("/summary/unsupported_recursive_defn")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        accepted_depends_on_bounded_proof: bounded_recursive_defn > 0,
    })
}

fn collect_proof_assumptions(
    proof_inventory: &[TrustCertificateProofInventoryItem],
) -> Result<Vec<TrustCertificateProofAssumption>> {
    let mut assumptions = BTreeSet::new();
    for item in proof_inventory {
        let doc = report_common::read_json_file(Path::new(&item.proof_summary.path))?;
        for assumption in doc
            .get("assumptions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(kind) = assumption.get("kind").and_then(Value::as_str) else {
                continue;
            };
            let Some(subject) = assumption.get("subject").and_then(Value::as_str) else {
                continue;
            };
            assumptions.insert(TrustCertificateProofAssumption {
                kind: kind.to_string(),
                subject: subject.to_string(),
                digest: assumption
                    .get("digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                certifiable: assumption
                    .get("certifiable")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            });
        }
    }
    Ok(assumptions.into_iter().collect())
}

fn collect_formal_verification_scope_summary(
    proof_inventory: &[TrustCertificateProofInventoryItem],
    coverage_path: &Path,
    operational_entry_symbol: &str,
    runtime: Option<&TrustCertificateRuntime>,
) -> Result<FormalVerificationScopeSummary> {
    let proved_symbol_count = proof_inventory.len() as u64;
    let proved_defn_count = proof_inventory
        .iter()
        .filter(|item| item.kind == "defn")
        .count() as u64;
    let proved_defasync_count = proof_inventory
        .iter()
        .filter(|item| item.kind == "defasync")
        .count() as u64;
    let entry_body_formally_proved = proof_inventory.iter().any(|item| {
        item.symbol == operational_entry_symbol
            && item
                .proof_check_result
                .as_deref()
                .is_some_and(|result| result == "accepted")
    });
    let operational_entry_proof_inventory_refs = proof_inventory
        .iter()
        .filter(|item| item.symbol == operational_entry_symbol)
        .filter_map(|item| item.proof_object.clone())
        .collect::<Vec<_>>();

    if !coverage_path.is_file() {
        return Ok(FormalVerificationScopeSummary {
            formal_verification_scope: if entry_body_formally_proved {
                "entry_body".to_string()
            } else if proved_symbol_count > 0 {
                "partial".to_string()
            } else {
                "none".to_string()
            },
            proved_symbol_count,
            proved_defn_count,
            proved_defasync_count,
            entry_body_formally_proved,
            operational_entry_proof_inventory_refs,
            capsule_boundary_only_symbol_count: 0,
            runtime_evidence_only_symbol_count: if !entry_body_formally_proved && runtime.is_some()
            {
                1
            } else {
                0
            },
        });
    }

    let doc = report_common::read_json_file(coverage_path)?;
    let functions = doc
        .get("functions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let capsule_boundary_only_symbol_count = functions
        .iter()
        .filter(|function| {
            function.get("status").and_then(Value::as_str) == Some("capsule_boundary")
        })
        .count() as u64;
    let certifiable_graph_symbol_count = functions
        .iter()
        .filter(|function| {
            matches!(
                function.get("status").and_then(Value::as_str),
                Some("supported") | Some("supported_recursive") | Some("supported_async")
            )
        })
        .count() as u64;
    let formal_verification_scope = if proved_symbol_count == 0 {
        "none"
    } else if certifiable_graph_symbol_count > 0
        && proved_symbol_count >= certifiable_graph_symbol_count
    {
        "whole_certifiable_graph"
    } else if entry_body_formally_proved {
        "entry_body"
    } else {
        "partial"
    };
    Ok(FormalVerificationScopeSummary {
        formal_verification_scope: formal_verification_scope.to_string(),
        proved_symbol_count,
        proved_defn_count,
        proved_defasync_count,
        entry_body_formally_proved,
        operational_entry_proof_inventory_refs,
        capsule_boundary_only_symbol_count,
        runtime_evidence_only_symbol_count: if !entry_body_formally_proved && runtime.is_some() {
            1
        } else {
            0
        },
    })
}

fn realized_certificate_claims(
    profile_claims: &[String],
    verdict_accepted: bool,
    proved_symbol_count: u64,
    entry_body_formally_proved: bool,
) -> Vec<String> {
    let mut claims = Vec::new();
    for claim in profile_claims {
        let allow = match claim.as_str() {
            "human_can_review_certificate_not_source" => verdict_accepted,
            "certificate_includes_formal_proof" => verdict_accepted && proved_symbol_count > 0,
            "operational_entry_formally_proved" => verdict_accepted && entry_body_formally_proved,
            _ => true,
        };
        if allow {
            claims.push(claim.clone());
        }
    }
    if verdict_accepted
        && proved_symbol_count > 0
        && !claims
            .iter()
            .any(|claim| claim == "certificate_includes_formal_proof")
    {
        claims.push("certificate_includes_formal_proof".to_string());
    }
    if verdict_accepted
        && entry_body_formally_proved
        && !claims
            .iter()
            .any(|claim| claim == "operational_entry_formally_proved")
    {
        claims.push("operational_entry_formally_proved".to_string());
    }
    claims
}

fn collect_imported_summary_inventory(doc: &Value) -> Vec<TrustCertificateImportedSummary> {
    let mut out = doc
        .get("imported_summaries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.get("path").and_then(Value::as_str)?.to_string();
            let sha256_hex = entry.get("sha256_hex").and_then(Value::as_str)?.to_string();
            let mut symbols = entry
                .get("symbols")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            symbols.sort();
            symbols.dedup();
            Some(TrustCertificateImportedSummary {
                path,
                sha256_hex,
                symbols,
            })
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| {
        (a.path.as_str(), a.sha256_hex.as_str()).cmp(&(b.path.as_str(), b.sha256_hex.as_str()))
    });
    out
}

fn collect_capsule_artifacts(
    project_root: &Path,
    profile: Option<&TrustProfile>,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<CapsuleArtifacts> {
    let empty = || CapsuleArtifacts {
        capsules: TrustCertificateCapsules {
            count: 0,
            ids: Vec::new(),
            attestations: Vec::new(),
        },
        network_capsules: TrustCertificateNetworkCapsules {
            count: 0,
            ids: Vec::new(),
        },
        capsule_attestations: Vec::new(),
        effect_logs: Vec::new(),
        peer_policies: Vec::new(),
    };
    let index_path = default_capsule_index_path(project_root);
    if !index_path.is_file() {
        if profile.is_some_and(|p| p.evidence_requirements.require_capsule_attestations) {
            diagnostics.push(trust_diag_with_path(
                "X07TC_ECAPSULE_ATTEST",
                "trust profile requires arch/capsules/index.x07capsule.json",
                &index_path,
            ));
        }
        if profile.is_some_and(|p| p.evidence_requirements.require_effect_log_digests) {
            diagnostics.push(trust_diag_with_path(
                "X07TC_EEFFECT_LOG",
                "trust profile requires effect-log evidence, but no capsule index is present",
                &index_path,
            ));
        }
        if profile.is_some_and(|p| p.evidence_requirements.require_network_capsules) {
            diagnostics.push(trust_diag_with_path(
                "X07TC_ECAPSULE_NETWORK_ATTEST",
                "trust profile requires network capsule evidence, but no capsule index is present",
                &index_path,
            ));
        }
        if profile.is_some_and(|p| p.evidence_requirements.require_peer_policies) {
            diagnostics.push(trust_diag_with_path(
                "X07TC_EPEER_POLICY",
                "trust profile requires peer-policy evidence, but no capsule index is present",
                &index_path,
            ));
        }
        return Ok(empty());
    }

    let index = match load_capsule_index(&index_path) {
        Ok(index) => index,
        Err(err) => {
            diagnostics.push(trust_diag_with_path(
                "X07TC_ECAPSULE_ATTEST",
                format!("{err:#}"),
                &index_path,
            ));
            return Ok(empty());
        }
    };

    let index_root = index_path.parent().unwrap_or_else(|| Path::new("."));
    let mut capsule_ids = Vec::new();
    let mut network_capsule_ids = Vec::new();
    let mut capsule_attestations = Vec::new();
    let mut effect_logs = Vec::new();
    let mut peer_policies = Vec::new();

    for capsule in &index.capsules {
        capsule_ids.push(capsule.id.clone());

        let contract_path = index_root.join(&capsule.contract_path);
        match load_capsule_contract(&contract_path) {
            Ok(contract) => {
                let effect_log_schema = index_root.join(&contract.effect_log.schema_path);
                if !effect_log_schema.is_file() {
                    diagnostics.push(trust_diag_with_path(
                        "X07TC_EEFFECT_LOG",
                        format!(
                            "capsule {:?} references missing effect-log schema {:?}",
                            capsule.id, contract.effect_log.schema_path
                        ),
                        &effect_log_schema,
                    ));
                } else {
                    let effect_log_doc = report_common::read_json_file(&effect_log_schema)?;
                    if effect_log_doc
                        .get("schema_version")
                        .and_then(Value::as_str)
                        .is_some_and(|version| version != X07_EFFECT_LOG_SCHEMA_VERSION)
                    {
                        diagnostics.push(trust_diag_with_path(
                            "X07TC_EEFFECT_LOG",
                            format!(
                                "capsule {:?} effect-log schema must use {}",
                                capsule.id, X07_EFFECT_LOG_SCHEMA_VERSION
                            ),
                            &effect_log_schema,
                        ));
                    }
                    effect_logs.push(evidence_ref_for_path(&effect_log_schema)?);
                }
                if let Some(network) = &contract.network {
                    network_capsule_ids.push(capsule.id.clone());
                    if network.conformance_tests.is_empty() {
                        diagnostics.push(trust_diag_with_path(
                            "X07TC_ECAPSULE_NETWORK_ATTEST",
                            format!(
                                "network capsule {:?} must declare conformance_tests",
                                capsule.id
                            ),
                            &contract_path,
                        ));
                    }
                    for peer_policy_path in &network.peer_policy_paths {
                        let peer_policy_abs = index_root.join(peer_policy_path);
                        if !peer_policy_abs.is_file() {
                            diagnostics.push(trust_diag_with_path(
                                "X07TC_EPEER_POLICY",
                                format!(
                                    "network capsule {:?} references missing peer policy {:?}",
                                    capsule.id, peer_policy_path
                                ),
                                &peer_policy_abs,
                            ));
                            continue;
                        }
                        let _policy = load_peer_policy(&peer_policy_abs).map_err(|err| {
                            diagnostics.push(trust_diag_with_path(
                                "X07TC_EPEER_POLICY",
                                format!("{err:#}"),
                                &peer_policy_abs,
                            ));
                            err
                        });
                        if peer_policy_abs.is_file() {
                            peer_policies.push(evidence_ref_for_path(&peer_policy_abs)?);
                        }
                    }
                }
            }
            Err(err) => diagnostics.push(trust_diag_with_path(
                "X07TC_ECAPSULE_ATTEST",
                format!("{err:#}"),
                &contract_path,
            )),
        }

        let attestation_path = index_root.join(&capsule.attestation_path);
        if attestation_path.is_file() {
            capsule_attestations.push(evidence_ref_for_path(&attestation_path)?);
        } else {
            diagnostics.push(trust_diag_with_path(
                "X07TC_ECAPSULE_ATTEST",
                format!(
                    "capsule {:?} is missing attestation {:?}",
                    capsule.id, capsule.attestation_path
                ),
                &attestation_path,
            ));
        }
    }

    capsule_ids.sort();
    network_capsule_ids.sort();
    capsule_attestations.sort_by(|a, b| a.path.cmp(&b.path));
    effect_logs.sort_by(|a, b| a.path.cmp(&b.path));
    peer_policies.sort_by(|a, b| a.path.cmp(&b.path));
    peer_policies.dedup_by(|a, b| a.path == b.path);

    if profile.is_some_and(|p| p.evidence_requirements.require_capsule_attestations)
        && capsule_attestations.len() < capsule_ids.len()
    {
        diagnostics.push(trust_diag(
            "X07TC_ECAPSULE_ATTEST",
            "trust profile requires attested capsules, but one or more attestations are missing",
        ));
    }
    if profile.is_some_and(|p| p.evidence_requirements.require_effect_log_digests)
        && effect_logs.is_empty()
    {
        diagnostics.push(trust_diag(
            "X07TC_EEFFECT_LOG",
            "trust profile requires effect-log digests, but none were collected",
        ));
    }
    if profile.is_some_and(|p| p.evidence_requirements.require_network_capsules)
        && network_capsule_ids.is_empty()
    {
        diagnostics.push(trust_diag(
            "X07TC_ECAPSULE_NETWORK_ATTEST",
            "trust profile requires network capsule evidence, but no network capsule contracts were collected",
        ));
    }
    if profile.is_some_and(|p| p.evidence_requirements.require_peer_policies)
        && peer_policies.is_empty()
    {
        diagnostics.push(trust_diag(
            "X07TC_EPEER_POLICY",
            "trust profile requires peer-policy evidence, but none were collected",
        ));
    }

    Ok(CapsuleArtifacts {
        capsules: TrustCertificateCapsules {
            count: capsule_ids.len() as u64,
            ids: capsule_ids,
            attestations: capsule_attestations.clone(),
        },
        network_capsules: TrustCertificateNetworkCapsules {
            count: network_capsule_ids.len() as u64,
            ids: network_capsule_ids,
        },
        capsule_attestations,
        effect_logs,
        peer_policies,
    })
}

fn collect_runtime_attestation(
    tests_report_path: &Path,
    project_root: &Path,
    ctx: Option<&ProjectContext>,
    profile: Option<&TrustProfile>,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<(Option<TrustCertificateRuntime>, Option<EvidenceRef>)> {
    let mut backend = None;
    let mut weaker_isolation = false;
    let mut runtime_attestation = None;
    let mut network_mode = match ctx
        .and_then(|project| project.policy_doc.as_ref())
        .and_then(|doc| doc.pointer("/net/enabled"))
        .and_then(Value::as_bool)
    {
        Some(true)
            if !policy_allow_hosts_from_doc(
                ctx.and_then(|project| project.policy_doc.as_ref()),
            )
            .is_empty() =>
        {
            "allowlist".to_string()
        }
        Some(false) => "none".to_string(),
        _ => "disabled".to_string(),
    };
    let mut network_enforcement = if network_mode == "allowlist" {
        "unsupported".to_string()
    } else {
        "none".to_string()
    };
    let mut effective_allow_hosts =
        policy_allow_hosts_from_doc(ctx.and_then(|project| project.policy_doc.as_ref()));
    let mut policy_digest_bound = false;
    let mut guest_image_digest_bound = false;

    if tests_report_path.is_file() {
        let report_doc = report_common::read_json_file(tests_report_path)?;
        if let Some(tests) = report_doc.get("tests").and_then(Value::as_array) {
            for test in tests {
                let Some(run) = test.get("run") else {
                    continue;
                };
                if backend.is_none() {
                    backend = run
                        .get("sandbox_backend")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                let Some(attestation_ref) = run.get("runtime_attestation") else {
                    continue;
                };
                if backend.is_none() {
                    backend = attestation_ref
                        .get("sandbox_backend")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                weaker_isolation = attestation_ref
                    .get("weaker_isolation")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if let Some(raw_path) = attestation_ref.get("path").and_then(Value::as_str) {
                    let resolved =
                        resolve_report_artifact_path(tests_report_path, project_root, raw_path);
                    if resolved.is_file() {
                        match load_runtime_attestation_doc(&resolved) {
                            Ok(doc) => {
                                backend = Some(doc.sandbox_backend.clone());
                                weaker_isolation = doc.weaker_isolation;
                                network_mode = doc.network_mode;
                                network_enforcement = doc.network_enforcement;
                                effective_allow_hosts = doc.effective_allow_hosts;
                                policy_digest_bound = doc.effective_policy_digest.is_some();
                                guest_image_digest_bound = doc.guest_image_digest.is_some();
                                runtime_attestation = Some(evidence_ref_for_path(&resolved)?);
                            }
                            Err(err) => diagnostics.push(trust_diag_with_path(
                                "X07TC_ERUNTIME_ATTEST",
                                format!("{err:#}"),
                                &resolved,
                            )),
                        }
                    } else {
                        diagnostics.push(trust_diag_with_path(
                            "X07TC_ERUNTIME_ATTEST",
                            format!(
                                "runtime attestation referenced by x07 test is missing: {raw_path}"
                            ),
                            &resolved,
                        ));
                    }
                    break;
                }
            }
        }
    }

    if profile.is_some_and(|p| p.evidence_requirements.require_runtime_attestation)
        && runtime_attestation.is_none()
    {
        diagnostics.push(trust_diag(
            "X07TC_ERUNTIME_ATTEST",
            "trust profile requires runtime attestation, but x07 test did not produce one",
        ));
    }
    if profile.is_some_and(|p| p.sandbox_requirements.sandbox_backend == "vm")
        && backend.as_deref().is_some_and(|value| value != "vm")
    {
        diagnostics.push(trust_diag(
            "X07TC_ESANDBOX_PROFILE",
            format!(
                "runtime attestation reports sandbox_backend={:?}, but the trust profile requires vm",
                backend.as_deref().unwrap_or("none")
            ),
        ));
    }
    if profile.is_some_and(|p| p.sandbox_requirements.forbid_weaker_isolation) && weaker_isolation {
        diagnostics.push(trust_diag(
            "X07TC_ESANDBOX_PROFILE",
            "runtime attestation reports weaker isolation, which the trust profile forbids",
        ));
    }

    let runtime = if ctx.is_some_and(|project| project.world == WorldId::RunOsSandboxed)
        || runtime_attestation.is_some()
        || backend.is_some()
    {
        Some(TrustCertificateRuntime {
            backend: backend.unwrap_or_else(|| "none".to_string()),
            network_mode: network_mode.clone(),
            network_enforcement: network_enforcement.clone(),
            weaker_isolation,
            effective_allow_hosts: effective_allow_hosts.clone(),
            policy_digest_bound,
            guest_image_digest_bound,
            attestation: runtime_attestation.clone(),
        })
    } else {
        None
    };

    if let Some(profile) = profile {
        if profile.sandbox_requirements.network_mode == "none" && network_mode != "none" {
            diagnostics.push(trust_diag(
                "X07TC_ESANDBOX_PROFILE",
                format!(
                    "runtime policy reports network_mode={network_mode:?}, but the trust profile requires none"
                ),
            ));
        }
        if profile.sandbox_requirements.network_mode == "allowlist" {
            if network_mode != "allowlist" {
                diagnostics.push(trust_diag(
                    "X07TC_ERUNTIME_NETWORK_EVIDENCE",
                    format!(
                        "runtime attestation must report network_mode=\"allowlist\", got {:?}",
                        network_mode
                    ),
                ));
            }
            if effective_allow_hosts.is_empty() {
                diagnostics.push(trust_diag(
                    "X07TC_ERUNTIME_NETWORK_EVIDENCE",
                    "runtime attestation must bind a non-empty effective allowlist for the networked sandbox profile",
                ));
            }
            if !policy_digest_bound {
                diagnostics.push(trust_diag(
                    "X07TC_ERUNTIME_NETWORK_EVIDENCE",
                    "runtime attestation is missing the effective policy digest required for the networked sandbox profile",
                ));
            }
            if profile.sandbox_requirements.sandbox_backend == "vm" && !guest_image_digest_bound {
                diagnostics.push(trust_diag(
                    "X07TC_ERUNTIME_NETWORK_EVIDENCE",
                    "runtime attestation is missing the VM guest image digest required for the networked sandbox profile",
                ));
            }
            if profile.sandbox_requirements.network_enforcement != "any"
                && network_enforcement != profile.sandbox_requirements.network_enforcement
            {
                diagnostics.push(trust_diag(
                    "X07TC_ERUNTIME_NETWORK_EVIDENCE",
                    format!(
                        "runtime attestation reports network_enforcement={:?}, but the trust profile requires {:?}",
                        network_enforcement, profile.sandbox_requirements.network_enforcement
                    ),
                ));
            }
        }
    }

    Ok((runtime, runtime_attestation))
}

fn validate_trust_profile_strength(profile: &TrustProfile) -> Vec<diagnostics::Diagnostic> {
    let mut diags = Vec::new();
    if profile.id == "trusted_program_sandboxed_local_v1" {
        if profile
            .worlds_allowed
            .iter()
            .any(|world| world == WorldId::RunOs.as_str())
        {
            diags.push(trust_diag(
                "X07TP_SANDBOX_BACKEND_REQUIRED",
                "sandboxed trusted-program profiles must not allow run-os",
            ));
        }
        if !profile.evidence_requirements.require_async_proof_coverage {
            diags.push(trust_diag(
                "X07TP_ASYNC_PROOF_REQUIRED",
                "sandboxed trusted-program profiles must require async proof coverage",
            ));
        }
        if !profile
            .evidence_requirements
            .require_per_symbol_prove_reports_defn
            || !profile
                .evidence_requirements
                .require_per_symbol_prove_reports_async
        {
            diags.push(trust_diag(
                "X07TP_ASYNC_PROOF_REQUIRED",
                "sandboxed trusted-program profiles must require per-symbol prove reports",
            ));
        }
        if profile.evidence_requirements.allow_coverage_summary_imports {
            diags.push(trust_diag(
                "X07TP_ASYNC_PROOF_REQUIRED",
                "sandboxed trusted-program profiles must reject coverage-only imported summaries",
            ));
        }
        if !profile.evidence_requirements.require_capsule_attestations {
            diags.push(trust_diag(
                "X07TP_CAPSULE_ATTEST_REQUIRED",
                "sandboxed trusted-program profiles must require capsule attestations",
            ));
        }
        if !profile.evidence_requirements.require_runtime_attestation {
            diags.push(trust_diag(
                "X07TP_RUNTIME_ATTEST_REQUIRED",
                "sandboxed trusted-program profiles must require runtime attestation",
            ));
        }
        if profile.sandbox_requirements.sandbox_backend != "vm" {
            diags.push(trust_diag(
                "X07TP_SANDBOX_BACKEND_REQUIRED",
                "sandboxed trusted-program profiles must require sandbox_backend=vm",
            ));
        }
        if !profile.sandbox_requirements.forbid_weaker_isolation {
            diags.push(trust_diag(
                "X07TP_SANDBOX_BACKEND_REQUIRED",
                "sandboxed trusted-program profiles must forbid weaker isolation",
            ));
        }
        if profile.sandbox_requirements.network_mode != "none" {
            diags.push(trust_diag(
                "X07TP_NETWORK_MODE_FORBIDDEN",
                "sandboxed local trusted-program profiles must keep network_mode=none",
            ));
        }
        if profile.sandbox_requirements.network_enforcement != "none" {
            diags.push(trust_diag(
                "X07TP_NETWORK_MODE_FORBIDDEN",
                "sandboxed local trusted-program profiles must keep network_enforcement=none",
            ));
        }
    } else if profile.id == "trusted_program_sandboxed_net_v1" {
        if profile
            .worlds_allowed
            .iter()
            .any(|world| world == WorldId::RunOs.as_str())
        {
            diags.push(trust_diag(
                "X07TP_BACKEND_NOT_CERTIFIABLE",
                "networked sandboxed trusted-program profiles must not allow run-os",
            ));
        }
        if !profile.evidence_requirements.require_async_proof_coverage {
            diags.push(trust_diag(
                "X07TP_ASYNC_PROOF_REQUIRED",
                "networked sandboxed trusted-program profiles must require async proof coverage",
            ));
        }
        if !profile
            .evidence_requirements
            .require_per_symbol_prove_reports_defn
            || !profile
                .evidence_requirements
                .require_per_symbol_prove_reports_async
        {
            diags.push(trust_diag(
                "X07TP_ASYNC_PROOF_REQUIRED",
                "networked sandboxed trusted-program profiles must require per-symbol prove reports",
            ));
        }
        if profile.evidence_requirements.allow_coverage_summary_imports {
            diags.push(trust_diag(
                "X07TP_ASYNC_PROOF_REQUIRED",
                "networked sandboxed trusted-program profiles must reject coverage-only imported summaries",
            ));
        }
        if !profile.evidence_requirements.require_capsule_attestations {
            diags.push(trust_diag(
                "X07TP_CAPSULE_ATTEST_REQUIRED",
                "networked sandboxed trusted-program profiles must require capsule attestations",
            ));
        }
        if !profile.evidence_requirements.require_runtime_attestation {
            diags.push(trust_diag(
                "X07TP_RUNTIME_ATTEST_REQUIRED",
                "networked sandboxed trusted-program profiles must require runtime attestation",
            ));
        }
        if !profile.evidence_requirements.require_effect_log_digests {
            diags.push(trust_diag(
                "X07TP_EFFECT_LOG_REQUIRED",
                "networked sandboxed trusted-program profiles must require effect-log digests",
            ));
        }
        if !profile.evidence_requirements.require_peer_policies {
            diags.push(trust_diag(
                "X07TP_PEER_POLICY_REQUIRED",
                "networked sandboxed trusted-program profiles must require peer-policy evidence",
            ));
        }
        if !profile.evidence_requirements.require_network_capsules {
            diags.push(trust_diag(
                "X07TP_NETWORK_PROFILE_REQUIRED",
                "networked sandboxed trusted-program profiles must require network capsule evidence",
            ));
        }
        if !profile
            .evidence_requirements
            .require_dependency_closure_attestation
        {
            diags.push(trust_diag(
                "X07TP_DEP_CLOSURE_REQUIRED",
                "networked sandboxed trusted-program profiles must require dependency-closure attestations",
            ));
        }
        if profile.sandbox_requirements.sandbox_backend != "vm" {
            diags.push(trust_diag(
                "X07TP_BACKEND_NOT_CERTIFIABLE",
                "networked sandboxed trusted-program profiles must require sandbox_backend=vm",
            ));
        }
        if !profile.sandbox_requirements.forbid_weaker_isolation {
            diags.push(trust_diag(
                "X07TP_BACKEND_NOT_CERTIFIABLE",
                "networked sandboxed trusted-program profiles must forbid weaker isolation",
            ));
        }
        if profile.sandbox_requirements.network_mode != "allowlist" {
            diags.push(trust_diag(
                "X07TP_NETWORK_PROFILE_REQUIRED",
                "networked sandboxed trusted-program profiles must require network_mode=allowlist",
            ));
        }
        if profile.sandbox_requirements.network_enforcement != "vm_boundary_allowlist" {
            diags.push(trust_diag(
                "X07TP_NETWORK_PROFILE_REQUIRED",
                "networked sandboxed trusted-program profiles must require network_enforcement=vm_boundary_allowlist",
            ));
        }
    } else if profile.id == "certified_capsule_v1" {
        if profile
            .worlds_allowed
            .iter()
            .any(|world| world == WorldId::RunOs.as_str())
        {
            diags.push(trust_diag(
                "X07TP_SANDBOX_BACKEND_REQUIRED",
                "certified capsule profiles must not allow run-os",
            ));
        }
        if !profile.evidence_requirements.require_capsule_attestations {
            diags.push(trust_diag(
                "X07TP_CAPSULE_ATTEST_REQUIRED",
                "certified capsule profiles must require capsule attestations",
            ));
        }
        if !profile
            .evidence_requirements
            .require_per_symbol_prove_reports_defn
            || profile
                .evidence_requirements
                .require_per_symbol_prove_reports_async
        {
            diags.push(trust_diag(
                "X07TP_CAPSULE_ATTEST_REQUIRED",
                "certified capsule profiles must require per-symbol defn prove reports only",
            ));
        }
        if profile.evidence_requirements.allow_coverage_summary_imports {
            diags.push(trust_diag(
                "X07TP_CAPSULE_ATTEST_REQUIRED",
                "certified capsule profiles must reject coverage-only imported summaries",
            ));
        }
        if !profile.evidence_requirements.require_effect_log_digests {
            diags.push(trust_diag(
                "X07TP_EFFECT_LOG_REQUIRED",
                "certified capsule profiles must require effect-log digests",
            ));
        }
        if profile.sandbox_requirements.sandbox_backend != "vm" {
            diags.push(trust_diag(
                "X07TP_SANDBOX_BACKEND_REQUIRED",
                "certified capsule profiles must require sandbox_backend=vm",
            ));
        }
        if !profile.sandbox_requirements.forbid_weaker_isolation {
            diags.push(trust_diag(
                "X07TP_SANDBOX_BACKEND_REQUIRED",
                "certified capsule profiles must forbid weaker isolation",
            ));
        }
        if profile.sandbox_requirements.network_mode != "none" {
            diags.push(trust_diag(
                "X07TP_NETWORK_MODE_FORBIDDEN",
                "certified capsule profiles must keep network_mode=none",
            ));
        }
        if profile.sandbox_requirements.network_enforcement != "none" {
            diags.push(trust_diag(
                "X07TP_NETWORK_MODE_FORBIDDEN",
                "certified capsule profiles must keep network_enforcement=none",
            ));
        }
    } else if profile.language_subset.allow_defasync
        || profile.language_subset.allow_recursion
        || profile.language_subset.allow_extern
        || profile.language_subset.allow_unsafe
        || profile.language_subset.allow_ffi
        || profile.language_subset.allow_dynamic_dispatch
        || profile.arch_requirements.manifest_min_version != "x07.arch.manifest@0.3.0"
        || !profile.arch_requirements.require_allowlist_mode
        || !profile.arch_requirements.require_deny_cycles
        || !profile.arch_requirements.require_deny_orphans
        || !profile.arch_requirements.require_visibility
        || !profile.arch_requirements.require_world_caps
        || !profile.arch_requirements.require_brand_boundaries
        || !profile.evidence_requirements.require_boundary_index
        || !profile.evidence_requirements.require_schema_derive_check
        || !profile.evidence_requirements.require_smoke_harnesses
        || !profile.evidence_requirements.require_unit_tests
        || profile.evidence_requirements.require_pbt == "none"
        || profile.evidence_requirements.require_proof_mode != "prove"
        || !profile
            .evidence_requirements
            .require_proof_coverage
            .starts_with("all_reachable_")
        || profile.evidence_requirements.require_async_proof_coverage
        || !profile
            .evidence_requirements
            .require_per_symbol_prove_reports_defn
        || profile
            .evidence_requirements
            .require_per_symbol_prove_reports_async
        || profile.evidence_requirements.allow_coverage_summary_imports
        || profile.evidence_requirements.require_capsule_attestations
        || profile.evidence_requirements.require_runtime_attestation
        || profile.evidence_requirements.require_effect_log_digests
        || profile.evidence_requirements.require_peer_policies
        || profile.evidence_requirements.require_network_capsules
        || profile
            .evidence_requirements
            .require_dependency_closure_attestation
        || profile.sandbox_requirements.sandbox_backend != "any"
        || profile.sandbox_requirements.forbid_weaker_isolation
        || profile.sandbox_requirements.network_mode != "any"
        || profile.sandbox_requirements.network_enforcement != "any"
        || !profile.evidence_requirements.require_compile_attestation
        || !profile.evidence_requirements.require_trust_report_clean
        || !profile.evidence_requirements.require_sbom
    {
        diags.push(trust_diag(
            "X07TP_NOT_CERTIFIABLE",
            "trust profile is weaker than the Milestone A certification floor",
        ));
    }
    diags
}

fn validate_profile_against_context(
    profile: &TrustProfile,
    project_path: &Path,
    ctx: &ProjectContext,
    entry: Option<&str>,
) -> Result<Vec<diagnostics::Diagnostic>> {
    let mut diags = Vec::new();
    if let Some(entry) = entry {
        if !profile
            .entrypoints
            .iter()
            .any(|candidate| candidate == entry)
        {
            diags.push(trust_diag(
                "X07TP_ENTRY_FORBIDDEN",
                format!(
                    "entry {entry:?} is not allowed by trust profile {}",
                    profile.id
                ),
            ));
        }
    }

    if !profile
        .worlds_allowed
        .iter()
        .any(|world| world == ctx.world.as_str())
    {
        diags.push(trust_diag(
            "X07TP_WORLD",
            format!(
                "project world {:?} is not allowed by trust profile {}",
                ctx.world.as_str(),
                profile.id
            ),
        ));
    }
    if profile.sandbox_requirements.sandbox_backend == "vm" && ctx.world != WorldId::RunOsSandboxed
    {
        diags.push(trust_diag(
            if profile.id == "trusted_program_sandboxed_net_v1" {
                "X07TP_BACKEND_NOT_CERTIFIABLE"
            } else {
                "X07TP_SANDBOX_BACKEND_REQUIRED"
            },
            if profile.id == "trusted_program_sandboxed_net_v1" {
                "networked sandboxed trusted-program profiles require run-os-sandboxed project worlds"
            } else {
                "sandboxed trusted-program profiles require run-os-sandboxed project worlds"
            },
        ));
    }
    if profile.sandbox_requirements.network_mode == "none"
        && ctx
            .policy_doc
            .as_ref()
            .and_then(|doc| doc.pointer("/net/enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        diags.push(trust_diag(
            "X07TP_NETWORK_MODE_FORBIDDEN",
            "sandboxed local trusted-program profiles require policy.net.enabled=false",
        ));
    }
    if profile.sandbox_requirements.network_mode == "allowlist" {
        let allow_hosts = policy_allow_hosts_from_doc(ctx.policy_doc.as_ref());
        if ctx
            .policy_doc
            .as_ref()
            .and_then(|doc| doc.pointer("/net/enabled"))
            .and_then(Value::as_bool)
            != Some(true)
        {
            diags.push(trust_diag(
                "X07TP_NETWORK_PROFILE_REQUIRED",
                "networked sandboxed trusted-program profiles require policy.net.enabled=true",
            ));
        }
        if allow_hosts.is_empty() {
            diags.push(trust_diag(
                "X07TP_NETWORK_PROFILE_REQUIRED",
                "networked sandboxed trusted-program profiles require a non-empty policy.net.allow_hosts allowlist",
            ));
        }
    }

    let features = scan_language_features(&ctx.module_roots);
    if !profile.language_subset.allow_defasync && features.has_defasync {
        diags.push(trust_diag(
            "X07TP_LANGUAGE",
            "project contains defasync declarations but the trust profile forbids them",
        ));
    }
    if !profile.language_subset.allow_extern && features.has_extern {
        diags.push(trust_diag(
            "X07TP_LANGUAGE",
            "project contains extern declarations but the trust profile forbids them",
        ));
    }
    if !profile.language_subset.allow_unsafe
        && ctx
            .policy_doc
            .as_ref()
            .and_then(|doc| doc.pointer("/language/allow_unsafe"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        diags.push(trust_diag(
            "X07TP_LANGUAGE",
            "project policy enables allow_unsafe but the trust profile forbids it",
        ));
    }
    if !profile.language_subset.allow_ffi
        && ctx
            .policy_doc
            .as_ref()
            .and_then(|doc| doc.pointer("/language/allow_ffi"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        diags.push(trust_diag(
            "X07TP_LANGUAGE",
            "project policy enables allow_ffi but the trust profile forbids it",
        ));
    }

    let Some(arch_manifest_path) = ctx.arch_manifest_path.as_deref() else {
        diags.push(trust_diag(
            "X07TP_ARCH",
            format!(
                "project {:?} does not expose arch/manifest.x07arch.json required by trust profile",
                project_path.display()
            ),
        ));
        return Ok(diags);
    };

    let arch_doc = report_common::read_json_file(arch_manifest_path)?;
    let schema_version = arch_doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if schema_version != profile.arch_requirements.manifest_min_version {
        diags.push(trust_diag_with_path(
            "X07TP_ARCH",
            format!(
                "arch manifest schema_version mismatch: expected {:?} got {:?}",
                profile.arch_requirements.manifest_min_version, schema_version
            ),
            arch_manifest_path,
        ));
    }

    if profile.arch_requirements.require_allowlist_mode
        && !arch_doc
            .pointer("/checks/allowlist_mode/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        diags.push(trust_diag_with_path(
            "X07TP_ARCH",
            "arch checks.allowlist_mode.enabled must be true for this trust profile",
            arch_manifest_path,
        ));
    }
    if profile.arch_requirements.require_deny_cycles
        && !arch_doc
            .pointer("/checks/deny_cycles")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        diags.push(trust_diag_with_path(
            "X07TP_ARCH",
            "arch checks.deny_cycles must be true for this trust profile",
            arch_manifest_path,
        ));
    }
    if profile.arch_requirements.require_deny_orphans
        && !arch_doc
            .pointer("/checks/deny_orphans")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        diags.push(trust_diag_with_path(
            "X07TP_ARCH",
            "arch checks.deny_orphans must be true for this trust profile",
            arch_manifest_path,
        ));
    }
    if profile.arch_requirements.require_visibility
        && !arch_doc
            .pointer("/checks/enforce_visibility")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        diags.push(trust_diag_with_path(
            "X07TP_ARCH",
            "arch checks.enforce_visibility must be true for this trust profile",
            arch_manifest_path,
        ));
    }
    if profile.arch_requirements.require_world_caps
        && !arch_doc
            .pointer("/checks/enforce_world_caps")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        diags.push(trust_diag_with_path(
            "X07TP_ARCH",
            "arch checks.enforce_world_caps must be true for this trust profile",
            arch_manifest_path,
        ));
    }
    if profile.arch_requirements.require_brand_boundaries
        && !arch_doc
            .pointer("/checks/brand_boundary_v1/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        diags.push(trust_diag_with_path(
            "X07TP_ARCH",
            "arch checks.brand_boundary_v1.enabled must be true for this trust profile",
            arch_manifest_path,
        ));
    }
    if profile.evidence_requirements.require_boundary_index
        && arch_doc
            .pointer("/contracts_v1/boundaries/index_path")
            .and_then(Value::as_str)
            .is_none()
    {
        diags.push(trust_diag_with_path(
            "X07TP_BOUNDARY",
            "contracts_v1.boundaries.index_path is required by this trust profile",
            arch_manifest_path,
        ));
    }
    if profile.evidence_requirements.require_capsule_attestations {
        let capsule_index_path = default_capsule_index_path(&ctx.root);
        if !capsule_index_path.is_file() {
            diags.push(trust_diag_with_path(
                "X07TP_CAPSULES",
                "arch/capsules/index.x07capsule.json is required by this trust profile",
                &capsule_index_path,
            ));
        }
    }

    Ok(diags)
}

fn is_strong_trust_profile(profile: &TrustProfile) -> bool {
    profile.evidence_requirements.require_proof_mode == "prove"
        && profile
            .evidence_requirements
            .require_per_symbol_prove_reports_defn
        && !profile.evidence_requirements.allow_coverage_summary_imports
        && (!profile.language_subset.allow_defasync
            || profile
                .evidence_requirements
                .require_per_symbol_prove_reports_async)
}

fn add_coverage_diagnostics(
    coverage_doc: &Value,
    project_root: &Path,
    profile: Option<&TrustProfile>,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let Some(functions) = coverage_doc.get("functions").and_then(Value::as_array) else {
        return;
    };
    let user_only = profile_requires_user_proof_scope(profile);
    let recursion_forbidden = profile.is_some_and(|p| !p.language_subset.allow_recursion);
    let mut issues: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut forbidden_recursive_symbols = Vec::new();
    for function in functions {
        if !coverage_function_in_proof_scope(function, project_root, user_only) {
            continue;
        }
        let symbol = function
            .get("symbol")
            .and_then(Value::as_str)
            .unwrap_or("unknown_symbol")
            .to_string();
        if recursion_forbidden && coverage_function_is_recursive(function) {
            forbidden_recursive_symbols.push(symbol);
            continue;
        }
        if matches!(
            function.get("status").and_then(Value::as_str),
            Some(
                "supported"
                    | "supported_recursive"
                    | "supported_async"
                    | "imported_proof_summary"
                    | "trusted_primitive"
                    | "trusted_scheduler_model"
                    | "capsule_boundary"
            )
        ) {
            continue;
        }
        let code = coverage_issue_code(function);
        issues.entry(code).or_default().push(symbol);
    }
    if !forbidden_recursive_symbols.is_empty() {
        diagnostics.push(trust_diag(
            "X07TC_ERECURSION_FORBIDDEN",
            format!(
                "coverage report includes reachable recursive symbols under a trust profile that forbids recursion: {}",
                summarize_symbol_list(&forbidden_recursive_symbols)
            ),
        ));
    }
    for (code, symbols) in issues {
        diagnostics.push(trust_diag(code, coverage_issue_message(code, &symbols)));
    }
}

fn profile_requires_user_proof_scope(profile: Option<&TrustProfile>) -> bool {
    profile.is_some_and(|p| {
        p.evidence_requirements.require_proof_coverage == "all_reachable_user_defn"
    })
}

fn coverage_function_in_proof_scope(
    function: &Value,
    project_root: &Path,
    user_only: bool,
) -> bool {
    if !user_only {
        return true;
    }

    let Some(raw_source_path) = function.get("source_path").and_then(Value::as_str) else {
        return false;
    };
    let source_path = PathBuf::from(raw_source_path);
    let source_path = if source_path.is_absolute() {
        source_path
    } else {
        project_root.join(source_path)
    };
    if !source_path.is_file() {
        return false;
    }
    if !source_path.starts_with(project_root) {
        return false;
    }
    !source_path.starts_with(project_root.join(".x07").join("deps"))
}

fn coverage_issue_code(function: &Value) -> &'static str {
    let kind = function.get("kind").and_then(Value::as_str).unwrap_or("");
    let status = function.get("status").and_then(Value::as_str).unwrap_or("");
    let details = function
        .get("details")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let recursion_kind = coverage_function_recursion_kind(function);
    if kind == "defasync" && status == "unsupported" {
        return "X07TC_EUNSUPPORTED_DEFASYNC";
    }
    if kind == "defasync" && status == "uncovered" {
        return "X07TC_EASYNC_PROOF";
    }
    if status == "uncovered" {
        return "X07TC_EPROOF_COVERAGE";
    }
    if status == "unsupported"
        && (details.contains("recursive") || matches!(recursion_kind, "self_recursive" | "mutual"))
    {
        return "X07TC_EUNSUPPORTED_RECURSION";
    }
    if status == "unsupported" {
        return "X07TC_EPROVE_UNSUPPORTED";
    }
    "X07TC_EPROOF_COVERAGE"
}

fn coverage_function_recursion_kind(function: &Value) -> &str {
    function
        .pointer("/support_summary/recursion_kind")
        .or_else(|| function.pointer("/proof_summary/recursion_kind"))
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn coverage_function_is_recursive(function: &Value) -> bool {
    matches!(
        coverage_function_recursion_kind(function),
        "self_recursive" | "mutual"
    )
}

fn coverage_issue_message(code: &str, symbols: &[String]) -> String {
    let listed = summarize_symbol_list(symbols);
    match code {
        "X07TC_EASYNC_PROOF" => format!(
            "coverage report includes reachable defasync symbols without complete async proof coverage: {listed}"
        ),
        "X07TC_EUNSUPPORTED_DEFASYNC" => format!(
            "coverage report includes reachable defasync symbols outside the certifiable subset: {listed}"
        ),
        "X07TC_EUNSUPPORTED_RECURSION" => format!(
            "coverage report includes reachable recursive symbols outside the certifiable subset: {listed}"
        ),
        "X07TC_EPROVE_UNSUPPORTED" => format!(
            "coverage report includes reachable symbols outside the supported proof subset: {listed}"
        ),
        _ => format!(
            "coverage report includes reachable symbols without complete proof coverage: {listed}"
        ),
    }
}

fn summarize_symbol_list(symbols: &[String]) -> String {
    let preview = symbols.iter().take(3).cloned().collect::<Vec<_>>();
    let head = preview.join(", ");
    if symbols.len() > preview.len() {
        format!("{head} (+{} more)", symbols.len() - preview.len())
    } else {
        head
    }
}

fn prove_issue_code(report_doc: &Value) -> &'static str {
    let diagnostics = report_doc
        .get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    for diag in diagnostics {
        match diag.get("code").and_then(Value::as_str).unwrap_or("") {
            "X07V_UNSUPPORTED_DEFASYNC_FORM" => return "X07TC_EUNSUPPORTED_DEFASYNC",
            "X07V_ASYNC_COUNTEREXAMPLE"
            | "X07V_SCOPE_INVARIANT_FAILED"
            | "X07V_CANCELLATION_ENSURE_FAILED"
            | "X07V_SCHEDULER_MODEL_UNTRUSTED" => return "X07TC_EASYNC_PROOF",
            "X07V_UNSUPPORTED_RECURSION"
            | "X07V_RECURSIVE_DECREASES_REQUIRED"
            | "X07V_UNSUPPORTED_MUTUAL_RECURSION" => return "X07TC_EUNSUPPORTED_RECURSION",
            _ => {}
        }
    }
    if report_doc.pointer("/result/kind").and_then(Value::as_str) == Some("unsupported") {
        return "X07TC_EPROVE_UNSUPPORTED";
    }
    "X07TC_EPROVE"
}

fn add_review_diff_diagnostics(review_doc: &Value, diagnostics: &mut Vec<diagnostics::Diagnostic>) {
    let proof_changes = review_highlight_count(review_doc, "/highlights/proof_changes");
    let recursive_proof_changes =
        review_highlight_count(review_doc, "/highlights/recursive_proof_changes");
    let boundary_changes = review_highlight_count(review_doc, "/highlights/boundary_changes");
    let subset_changes = review_highlight_count(review_doc, "/highlights/subset_changes");
    let summary_changes = review_highlight_count(review_doc, "/highlights/summary_changes");
    let network_policy_changes =
        review_highlight_count(review_doc, "/highlights/network_policy_changes");
    let peer_policy_changes = review_highlight_count(review_doc, "/highlights/peer_policy_changes");
    let capsule_network_changes =
        review_highlight_count(review_doc, "/highlights/capsule_network_changes");
    let dependency_closure_changes =
        review_highlight_count(review_doc, "/highlights/dependency_closure_changes");

    if proof_changes > 0 || recursive_proof_changes > 0 || subset_changes > 0 || summary_changes > 0
    {
        diagnostics.push(trust_diag(
            "X07TC_EDIFF_POSTURE",
            format!(
                "review diff detected forbidden trust posture changes (proof_changes={proof_changes}, recursive_proof_changes={recursive_proof_changes}, subset_changes={subset_changes}, summary_changes={summary_changes})"
            ),
        ));
    }
    if boundary_changes > 0 {
        diagnostics.push(trust_diag(
            "X07TC_EBOUNDARY_RELAXED",
            format!(
                "review diff detected {boundary_changes} boundary contract relaxation(s) relative to the baseline"
            ),
        ));
    }
    if network_policy_changes > 0 {
        diagnostics.push(trust_diag(
            "X07TC_ENET_POLICY",
            format!(
                "review diff detected {network_policy_changes} network-policy change(s) relative to the baseline"
            ),
        ));
    }
    if peer_policy_changes > 0 {
        diagnostics.push(trust_diag(
            "X07TC_EPEER_POLICY",
            format!(
                "review diff detected {peer_policy_changes} peer-policy change(s) relative to the baseline"
            ),
        ));
    }
    if capsule_network_changes > 0 {
        diagnostics.push(trust_diag(
            "X07TC_ECAPSULE_NETWORK_ATTEST",
            format!(
                "review diff detected {capsule_network_changes} network capsule contract change(s) relative to the baseline"
            ),
        ));
    }
    if dependency_closure_changes > 0 {
        diagnostics.push(trust_diag(
            "X07TC_EDEP_CLOSURE",
            format!(
                "review diff detected {dependency_closure_changes} dependency-closure change(s) relative to the baseline"
            ),
        ));
    }
}

fn review_highlight_count(review_doc: &Value, ptr: &str) -> usize {
    review_doc
        .pointer(ptr)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn scan_language_features(module_roots: &[PathBuf]) -> LanguageFeatureScan {
    let mut scan = LanguageFeatureScan::default();
    for root in module_roots {
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".x07.json"))
            {
                continue;
            }
            let Ok(doc) = report_common::read_json_file(path) else {
                continue;
            };
            if let Some(decls) = doc.get("decls").and_then(Value::as_array) {
                for decl in decls {
                    match decl.get("kind").and_then(Value::as_str).unwrap_or("") {
                        "defasync" => scan.has_defasync = true,
                        "extern" => scan.has_extern = true,
                        _ => {}
                    }
                }
            }
            if scan.has_defasync && scan.has_extern {
                return scan;
            }
        }
    }
    scan
}

fn resolve_project_manifest_arg(path: &Path) -> Result<PathBuf> {
    let resolved = util::resolve_existing_path_upwards(path);
    if resolved.is_dir() {
        let manifest = resolved.join("x07.json");
        if manifest.is_file() {
            return Ok(manifest);
        }
        anyhow::bail!(
            "project dir does not contain x07.json: {}",
            resolved.display()
        );
    }
    if resolved.is_file() {
        return Ok(resolved);
    }
    anyhow::bail!("project path not found: {}", resolved.display())
}

fn build_boundaries_evidence(
    project_root: &Path,
    out_dir: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<(EvidenceRef, Value)> {
    let manifest_path = project_root.join("arch/manifest.x07arch.json");
    let boundaries_path = out_dir.join("boundaries.report.json");
    if !manifest_path.is_file() {
        diagnostics.push(trust_diag(
            "X07TC_EARCH_STRICT",
            "arch/manifest.x07arch.json is missing",
        ));
        let evidence = write_stub_artifact(
            &boundaries_path,
            "boundaries_report",
            "arch manifest is missing",
        )?;
        return Ok((evidence, json!({})));
    }

    let arch_report_path = out_dir.join("arch.report.json");
    let args = vec![
        "--out".to_string(),
        arch_report_path.display().to_string(),
        "arch".to_string(),
        "check".to_string(),
        "--repo-root".to_string(),
        project_root.display().to_string(),
        "--manifest".to_string(),
        manifest_path.display().to_string(),
    ];
    let run = run_self_command(project_root, &args)?;
    if run.exit_code != 0 {
        diagnostics.push(trust_diag(
            "X07TC_EARCH_STRICT",
            format!(
                "x07 arch check exited with {}: {}",
                run.exit_code,
                stderr_summary(&run.stderr)
            ),
        ));
    }
    let doc = if arch_report_path.is_file() {
        report_common::read_json_file(&arch_report_path)?
    } else {
        json!({})
    };
    let boundaries_doc = doc.get("boundaries_report").cloned().unwrap_or_else(|| {
        diagnostics.push(trust_diag(
            "X07TC_EBOUNDARY_MISSING",
            "arch report did not include boundaries_report",
        ));
        json!({"ok": false, "message": "boundaries_report missing from arch report"})
    });
    write_json_artifact(&boundaries_path, &boundaries_doc)?;
    Ok((evidence_ref_for_path(&boundaries_path)?, boundaries_doc))
}

type CoverageEvidence = (
    EvidenceRef,
    Value,
    Vec<(String, String)>,
    Option<EvidenceRef>,
    Vec<TrustCertificateImportedSummary>,
);

fn build_coverage_evidence(
    project_path: &Path,
    entry: &str,
    project_root: &Path,
    profile: Option<&TrustProfile>,
    out_dir: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<CoverageEvidence> {
    let verify_report_path = out_dir.join("verify.coverage.report.json");
    let coverage_path = out_dir.join("verify.coverage.json");
    let cwd = project_path.parent().unwrap_or_else(|| Path::new("."));
    let args = vec![
        "verify".to_string(),
        "--coverage".to_string(),
        "--entry".to_string(),
        entry.to_string(),
        "--project".to_string(),
        project_path.display().to_string(),
        "--report-out".to_string(),
        verify_report_path.display().to_string(),
        "--quiet-json".to_string(),
    ];
    let run = run_self_command(cwd, &args)?;
    if run.exit_code != 0 {
        diagnostics.push(trust_diag(
            "X07TC_EPROOF_COVERAGE",
            format!(
                "x07 verify --coverage exited with {}: {}",
                run.exit_code,
                stderr_summary(&run.stderr)
            ),
        ));
    }

    let report_doc = if verify_report_path.is_file() {
        report_common::read_json_file(&verify_report_path)?
    } else {
        json!({})
    };
    let coverage_doc = report_doc
        .get("coverage")
        .cloned()
        .unwrap_or_else(|| json!({"summary": {}, "functions": []}));
    write_json_artifact(&coverage_path, &coverage_doc)?;

    if report_doc.pointer("/result/kind").and_then(Value::as_str) != Some("coverage_report") {
        diagnostics.push(trust_diag(
            "X07TC_EPROOF_COVERAGE",
            "verify coverage report did not report result.kind = coverage_report",
        ));
    }

    let mut summary_ref = None;
    let mut imported_summary_inventory = Vec::new();
    if let Some(raw_summary_path) = report_doc
        .pointer("/artifacts/verify_coverage_summary_path")
        .and_then(Value::as_str)
    {
        let summary_src_path =
            resolve_report_artifact_path(&verify_report_path, cwd, raw_summary_path);
        if summary_src_path.is_file() {
            let summary_doc = report_common::read_json_file(&summary_src_path)?;
            let summary_path = out_dir.join("verify.summary.json");
            write_json_artifact(&summary_path, &summary_doc)?;
            summary_ref = Some(evidence_ref_for_path(&summary_path)?);
            imported_summary_inventory = collect_imported_summary_inventory(&summary_doc);
        } else {
            diagnostics.push(trust_diag(
                "X07TC_EPROOF_COVERAGE",
                format!(
                    "verify coverage report referenced missing verify.summary artifact: {}",
                    summary_src_path.display()
                ),
            ));
        }
    }

    let mut prove_targets = Vec::new();
    let user_only = profile_requires_user_proof_scope(profile);
    if let Some(functions) = coverage_doc.get("functions").and_then(Value::as_array) {
        for function in functions {
            if !coverage_function_in_proof_scope(function, project_root, user_only) {
                continue;
            }
            let Some(kind) = function.get("kind").and_then(Value::as_str) else {
                continue;
            };
            let should_prove = matches!(
                (kind, function.get("status").and_then(Value::as_str)),
                ("defn", Some("supported" | "supported_recursive"))
                    | ("defasync", Some("supported_async"))
            );
            if should_prove {
                if let Some(symbol) = function.get("symbol").and_then(Value::as_str) {
                    prove_targets.push((symbol.to_string(), kind.to_string()));
                }
            }
        }
    }

    Ok((
        evidence_ref_for_path(&coverage_path)?,
        coverage_doc,
        prove_targets,
        summary_ref,
        imported_summary_inventory,
    ))
}

fn build_prove_evidence(
    project_path: &Path,
    prove_targets: &[(String, String)],
    out_dir: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<Vec<TrustCertificateProofInventoryItem>> {
    let cwd = project_path.parent().unwrap_or_else(|| Path::new("."));
    let prove_dir = out_dir.join("prove");
    std::fs::create_dir_all(&prove_dir)
        .with_context(|| format!("create prove dir: {}", prove_dir.display()))?;

    let mut inventory = Vec::new();
    for (symbol, kind) in prove_targets {
        let item_dir = prove_dir.join(util::safe_artifact_dir_name(symbol));
        std::fs::create_dir_all(&item_dir)
            .with_context(|| format!("create prove item dir: {}", item_dir.display()))?;
        let report_path = item_dir.join("verify.prove.report.json");
        let proof_path = item_dir.join("proof.json");
        let args = vec![
            "verify".to_string(),
            "--prove".to_string(),
            "--entry".to_string(),
            symbol.clone(),
            "--project".to_string(),
            project_path.display().to_string(),
            "--emit-proof".to_string(),
            proof_path.display().to_string(),
            "--report-out".to_string(),
            report_path.display().to_string(),
            "--quiet-json".to_string(),
        ];
        let run = run_self_command(cwd, &args)?;
        let report_doc = if report_path.is_file() {
            report_common::read_json_file(&report_path)?
        } else {
            json!({})
        };
        if run.exit_code != 0 {
            diagnostics.push(trust_diag(
                prove_issue_code(&report_doc),
                format!(
                    "x07 verify --prove for {:?} exited with {}: {}",
                    symbol,
                    run.exit_code,
                    stderr_summary(&run.stderr)
                ),
            ));
        }
        if report_doc.pointer("/result/kind").and_then(Value::as_str) != Some("proven") {
            diagnostics.push(trust_diag(
                prove_issue_code(&report_doc),
                format!(
                    "proof report for {:?} did not return result.kind = proven",
                    symbol
                ),
            ));
        }
        let verify_report = if report_path.is_file() {
            evidence_ref_for_path(&report_path)?
        } else {
            diagnostics.push(trust_diag(
                if kind == "defasync" {
                    "X07TC_EASYNC_PROVE_REPORT_MISSING"
                } else {
                    "X07TC_EPROVE_REPORT_MISSING"
                },
                format!("missing proof report for {symbol}"),
            ));
            write_stub_artifact(
                &report_path,
                "prove_report",
                &format!("missing proof report for {symbol}"),
            )?
        };

        let proof_summary_path = report_doc
            .pointer("/artifacts/verify_proof_summary_path")
            .and_then(Value::as_str)
            .map(|raw| resolve_report_artifact_path(&report_path, cwd, raw))
            .filter(|path| path.is_file());
        if proof_summary_path.is_none() {
            diagnostics.push(trust_diag(
                if kind == "defasync" {
                    "X07TC_EASYNC_PROVE_REPORT_MISSING"
                } else {
                    "X07TC_EPROVE_REPORT_MISSING"
                },
                format!("proof report for {symbol:?} is missing verify_proof_summary_path"),
            ));
        }
        let proof_summary_ref = if let Some(path) = proof_summary_path.as_ref() {
            evidence_ref_for_path(path)?
        } else {
            write_stub_artifact(
                &item_dir.join("verify.proof-summary.json"),
                "proof_summary",
                &format!("missing proof summary for {symbol}"),
            )?
        };

        let proof_object_path = report_doc
            .pointer("/artifacts/proof_object_path")
            .and_then(Value::as_str)
            .map(|raw| resolve_report_artifact_path(&report_path, cwd, raw))
            .filter(|path| path.is_file())
            .map(|path| path.to_path_buf());
        let proof_object_ref = proof_object_path
            .as_ref()
            .map(|path| evidence_ref_for_path(&path))
            .transpose()?;
        if proof_object_ref.is_none() {
            diagnostics.push(trust_diag(
                "X07TC_EPROOF_OBJECT_MISSING",
                format!("proof report for {symbol:?} is missing proof_object_path"),
            ));
        }

        let proof_check_report_path = report_doc
            .pointer("/artifacts/proof_check_report_path")
            .and_then(Value::as_str)
            .map(|raw| resolve_report_artifact_path(&report_path, cwd, raw))
            .filter(|path| path.is_file())
            .map(|path| path.to_path_buf());
        let proof_check_report_ref = proof_check_report_path
            .as_ref()
            .map(|path| evidence_ref_for_path(&path))
            .transpose()?;
        if proof_check_report_ref.is_none() {
            diagnostics.push(trust_diag(
                "X07TC_EPROOF_CHECK_MISSING",
                format!("proof report for {symbol:?} is missing proof_check_report_path"),
            ));
        }

        let proof_summary_doc = proof_summary_path
            .as_ref()
            .and_then(|path| report_common::read_json_file(path).ok());
        let proof_summary_engine = proof_summary_doc
            .as_ref()
            .and_then(|doc| doc.get("engine"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let result_kind = proof_summary_doc
            .as_ref()
            .and_then(|doc| {
                doc.get("result_kind")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| {
                if kind == "defasync" {
                    "proven_async".to_string()
                } else {
                    "proven".to_string()
                }
            });
        let mut proof_check_result = None;
        let mut proof_check_checker = None;
        let mut proof_object_digest = None;
        if let Some(path) = proof_check_report_path.as_ref() {
            match verify::load_proof_check_report_path(path) {
                Ok(report) => {
                    proof_object_digest = Some(report.proof_object_digest.clone());
                    proof_check_result = Some(report.result.clone());
                    proof_check_checker = Some(report.checker.clone());
                    if !proof_summary_engine.is_empty()
                        && report.verify_engine != proof_summary_engine
                    {
                        diagnostics.push(trust_diag(
                            "X07TC_EPROOF_CHECK_ENGINE_MISMATCH",
                            format!(
                                "proof-check report for {symbol:?} used engine {:?}, expected {:?}",
                                report.verify_engine, proof_summary_engine
                            ),
                        ));
                    }
                    if !report.ok || report.result != "accepted" {
                        diagnostics.push(trust_diag(
                            "X07TC_EPROOF_CHECK_REJECTED",
                            format!(
                                "proof-check report for {symbol:?} is not accepted (ok={}, result={})",
                                report.ok, report.result
                            ),
                        ));
                    }
                }
                Err(err) => {
                    diagnostics.push(trust_diag(
                        "X07TC_EPROOF_CHECK_SCHEMA_INVALID",
                        format!(
                            "proof-check report for {symbol:?} is schema-invalid or unreadable: {err:#}"
                        ),
                    ));
                }
            }
        }

        inventory.push(TrustCertificateProofInventoryItem {
            symbol: symbol.clone(),
            kind: kind.clone(),
            result_kind,
            verify_report,
            proof_summary: proof_summary_ref,
            proof_object: proof_object_ref,
            proof_check_report: proof_check_report_ref,
            proof_check_result,
            proof_check_checker,
            proof_object_digest,
        });
    }
    Ok(inventory)
}

fn build_tests_evidence(
    project_root: &Path,
    tests_manifest: &Path,
    profile: Option<&TrustProfile>,
    boundary_requirements: &BoundaryEvidenceRequirements,
    out_dir: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<EvidenceRef> {
    let tests_report_path = out_dir.join("tests.report.json");
    let require_tests = profile.is_some_and(|p| {
        p.evidence_requirements.require_unit_tests
            || p.evidence_requirements.require_smoke_harnesses
            || p.evidence_requirements.require_pbt.trim() != "none"
    });

    if !tests_manifest.is_file() {
        if require_tests {
            diagnostics.push(trust_diag(
                "X07TC_ETESTS",
                "tests/tests.json is missing but the trust profile requires tests",
            ));
        }
        return write_stub_artifact(&tests_report_path, "tests_report", "tests manifest missing");
    }
    if let Err(err) = validate_boundary_tests_against_manifest(
        tests_manifest,
        profile,
        boundary_requirements,
        diagnostics,
    ) {
        diagnostics.push(trust_diag(
            "X07TC_ETESTS",
            format!("validate boundary test requirements: {err:#}"),
        ));
    }

    let mut args = vec![
        "test".to_string(),
        "--manifest".to_string(),
        tests_manifest.display().to_string(),
        "--report-out".to_string(),
        tests_report_path.display().to_string(),
        "--quiet-json".to_string(),
    ];
    if profile.is_some_and(|p| p.evidence_requirements.require_pbt.trim() != "none") {
        args.push("--all".to_string());
    }
    let run = run_self_command(project_root, &args)?;
    if run.exit_code != 0 {
        diagnostics.push(trust_diag(
            "X07TC_ETESTS",
            format!(
                "x07 test exited with {}: {}",
                run.exit_code,
                stderr_summary(&run.stderr)
            ),
        ));
    }
    if tests_report_path.is_file() {
        let report_doc = report_common::read_json_file(&tests_report_path)?;
        let failed = report_doc
            .pointer("/summary/failed")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let errors = report_doc
            .pointer("/summary/errors")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if failed > 0 || errors > 0 {
            diagnostics.push(trust_diag(
                "X07TC_ETESTS",
                "x07 test report indicates failing tests",
            ));
        }
        validate_boundary_tests_against_report(&report_doc, boundary_requirements, diagnostics);
        evidence_ref_for_path(&tests_report_path)
    } else {
        write_stub_artifact(
            &tests_report_path,
            "tests_report",
            "x07 test did not emit a report",
        )
    }
}

fn build_trust_report_evidence(
    project_path: &Path,
    profile: Option<&TrustProfile>,
    out_dir: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<EvidenceRef> {
    let trust_report_path = out_dir.join("trust.report.json");
    let trust_report_html = out_dir.join("trust.report.html");
    let cwd = project_path.parent().unwrap_or_else(|| Path::new("."));
    let args = vec![
        "--out".to_string(),
        trust_report_path.display().to_string(),
        "trust".to_string(),
        "report".to_string(),
        "--project".to_string(),
        project_path.display().to_string(),
        "--html-out".to_string(),
        trust_report_html.display().to_string(),
    ];
    let run = run_self_command(cwd, &args)?;
    if run.exit_code != 0 {
        diagnostics.push(trust_diag(
            "X07TC_ETRUST_REPORT",
            format!(
                "x07 trust report exited with {}: {}",
                run.exit_code,
                stderr_summary(&run.stderr)
            ),
        ));
    }

    if trust_report_path.is_file() {
        let report_doc = report_common::read_json_file(&trust_report_path)?;
        if profile.is_some_and(|p| p.evidence_requirements.require_trust_report_clean)
            && trust_report_has_blocking_nondeterminism(profile, &report_doc)
        {
            diagnostics.push(trust_diag(
                "X07TC_ENONDET",
                "trust report contains nondeterminism flags",
            ));
        }
        if profile.is_some_and(|p| p.evidence_requirements.require_sbom)
            && !report_doc
                .pointer("/sbom/generated")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            diagnostics.push(trust_diag(
                "X07TC_ETRUST_REPORT",
                "trust report did not generate an SBOM artifact",
            ));
        }
        evidence_ref_for_path(&trust_report_path)
    } else {
        write_stub_artifact(
            &trust_report_path,
            "trust_report",
            "x07 trust report did not emit a report",
        )
    }
}

fn trust_report_has_blocking_nondeterminism(
    profile: Option<&TrustProfile>,
    report_doc: &Value,
) -> bool {
    let Some(flags) = report_doc
        .pointer("/nondeterminism/flags")
        .and_then(Value::as_array)
    else {
        return false;
    };

    flags.iter().filter_map(Value::as_object).any(|flag| {
        let kind = flag.get("kind").and_then(Value::as_str).unwrap_or("");
        if matches!(kind, "world_non_deterministic" | "process_enabled")
            && profile.is_some_and(|p| p.sandbox_requirements.sandbox_backend == "vm")
        {
            return false;
        }
        true
    })
}

fn build_bundle_evidence(
    project_path: &Path,
    entry: &str,
    bundle_out: Option<&Path>,
    out_dir: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<(EvidenceRef, Option<PathBuf>)> {
    let bundle_path = bundle_out
        .map(PathBuf::from)
        .unwrap_or_else(|| out_dir.join(util::safe_artifact_dir_name(entry)));
    let attestation_path = out_dir.join("compile.attest.json");
    let cwd = project_path.parent().unwrap_or_else(|| Path::new("."));
    let args = vec![
        "--out".to_string(),
        bundle_path.display().to_string(),
        "bundle".to_string(),
        "--project".to_string(),
        project_path.display().to_string(),
        "--emit-attestation".to_string(),
        attestation_path.display().to_string(),
    ];
    let run = run_self_command(cwd, &args)?;
    if run.exit_code != 0 {
        diagnostics.push(trust_diag(
            "X07TC_ECOMPILE_ATTEST",
            format!(
                "x07 bundle exited with {}: {}",
                run.exit_code,
                stderr_summary(&run.stderr)
            ),
        ));
    }

    let attestation_ref = if attestation_path.is_file() {
        let doc = report_common::read_json_file(&attestation_path)?;
        if !doc
            .pointer("/rebuild/deterministic_match")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            diagnostics.push(trust_diag(
                "X07TC_ECOMPILE_ATTEST",
                "compile attestation does not report a deterministic rebuild match",
            ));
        }
        evidence_ref_for_path(&attestation_path)?
    } else {
        write_stub_artifact(
            &attestation_path,
            "compile_attestation",
            "bundle step did not emit compile attestation",
        )?
    };

    let bundle_path = if bundle_path.is_file() {
        Some(bundle_path)
    } else {
        diagnostics.push(trust_diag(
            "X07TC_ECOMPILE_ATTEST",
            "bundle step did not produce an output executable",
        ));
        None
    };

    Ok((attestation_ref, bundle_path))
}

fn build_dependency_closure_evidence(
    project_path: &Path,
    profile: Option<&TrustProfile>,
    out_dir: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<(
    Option<TrustCertificateDependencyClosure>,
    Option<EvidenceRef>,
    Option<String>,
)> {
    let attestation_path = out_dir.join("dep.closure.attest.json");
    let cwd = project_path.parent().unwrap_or_else(|| Path::new("."));
    let args = vec![
        "pkg".to_string(),
        "attest-closure".to_string(),
        "--project".to_string(),
        project_path.display().to_string(),
        "--out".to_string(),
        attestation_path.display().to_string(),
    ];
    let run = run_self_command(cwd, &args)?;

    if !attestation_path.is_file() {
        if profile.is_some_and(|p| {
            p.evidence_requirements
                .require_dependency_closure_attestation
        }) {
            diagnostics.push(trust_diag(
                "X07TC_EDEP_CLOSURE",
                format!(
                    "x07 pkg attest-closure did not emit an attestation: {}",
                    stderr_summary(&run.stderr)
                ),
            ));
        }
        return Ok((None, None, None));
    }

    let doc = match load_dep_closure_attestation(&attestation_path) {
        Ok(doc) => doc,
        Err(err) => {
            diagnostics.push(trust_diag_with_path(
                "X07TC_EDEP_CLOSURE",
                format!("{err:#}"),
                &attestation_path,
            ));
            return Ok((None, None, None));
        }
    };
    let attestation_ref = evidence_ref_for_path(&attestation_path)?;
    let mut packages = doc
        .dependencies
        .iter()
        .map(|dep| format!("{}@{}", dep.name, dep.version))
        .collect::<Vec<_>>();
    packages.sort();

    if !doc.advisory_check.ok {
        diagnostics.push(trust_diag(
            "X07TC_EDEP_CLOSURE",
            "dependency closure attestation reports disallowed yanked dependencies or active advisories",
        ));
    } else if run.exit_code != 0 {
        diagnostics.push(trust_diag(
            "X07TC_EDEP_CLOSURE",
            format!(
                "x07 pkg attest-closure exited with {}: {}",
                run.exit_code,
                stderr_summary(&run.stderr)
            ),
        ));
    }

    Ok((
        Some(TrustCertificateDependencyClosure {
            manifest_digest: doc.manifest_digest.clone(),
            lockfile_digest: doc.lockfile_digest.clone(),
            packages,
            advisory_check_ok: doc.advisory_check.ok,
            attestation: Some(attestation_ref.clone()),
        }),
        Some(attestation_ref),
        Some(doc.package_set_digest),
    ))
}

fn build_review_evidence(
    baseline: Option<&Path>,
    project_root: &Path,
    out_dir: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<Option<EvidenceRef>> {
    let Some(baseline) = baseline else {
        return Ok(None);
    };
    let baseline = util::resolve_existing_path_upwards(baseline);
    let review_json = out_dir.join("review.diff.json");
    let review_html = out_dir.join("review.diff.html");
    let args = vec![
        "review".to_string(),
        "diff".to_string(),
        "--from".to_string(),
        baseline.display().to_string(),
        "--to".to_string(),
        project_root.display().to_string(),
        "--mode".to_string(),
        "project".to_string(),
        "--fail-on".to_string(),
        "proof-coverage-decrease".to_string(),
        "--fail-on".to_string(),
        "boundary-relaxation".to_string(),
        "--fail-on".to_string(),
        "trusted-subset-expansion".to_string(),
        "--json-out".to_string(),
        review_json.display().to_string(),
        "--html-out".to_string(),
        review_html.display().to_string(),
    ];
    let run = run_self_command(project_root, &args)?;
    if run.exit_code != 0 {
        diagnostics.push(trust_diag(
            "X07TC_EDIFF_POSTURE",
            format!(
                "x07 review diff exited with {}: {}",
                run.exit_code,
                stderr_summary(&run.stderr)
            ),
        ));
    }
    if review_json.is_file() {
        let review_doc = report_common::read_json_file(&review_json)?;
        add_review_diff_diagnostics(&review_doc, diagnostics);
        Ok(Some(evidence_ref_for_path(&review_json)?))
    } else {
        Ok(Some(write_stub_artifact(
            &review_json,
            "review_diff",
            "review diff did not emit a JSON report",
        )?))
    }
}

fn load_boundary_requirements(project_root: &Path) -> Result<BoundaryEvidenceRequirements> {
    let manifest_path = project_root.join("arch/manifest.x07arch.json");
    if !manifest_path.is_file() {
        return Ok(BoundaryEvidenceRequirements::default());
    }
    let manifest_doc = report_common::read_json_file(&manifest_path)?;
    let Some(index_path) = manifest_doc
        .pointer("/contracts_v1/boundaries/index_path")
        .and_then(Value::as_str)
    else {
        return Ok(BoundaryEvidenceRequirements::default());
    };
    let index_path = project_root.join(index_path);
    if !index_path.is_file() {
        return Ok(BoundaryEvidenceRequirements::default());
    }
    let index_doc = report_common::read_json_file(&index_path)?;
    let mut requirements = BoundaryEvidenceRequirements::default();
    if let Some(boundaries) = index_doc.get("boundaries").and_then(Value::as_array) {
        for boundary in boundaries {
            let boundary_id = boundary
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("unknown_boundary");
            let worlds_allowed = boundary
                .get("worlds_allowed")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if let Some(params) = boundary.pointer("/input/params").and_then(Value::as_array) {
                for param in params {
                    if let Some(schema_path) = param.get("schema").and_then(Value::as_str) {
                        requirements.schema_paths.insert(schema_path.to_string());
                    }
                }
            }
            if let Some(schema_path) = boundary.pointer("/output/schema").and_then(Value::as_str) {
                requirements.schema_paths.insert(schema_path.to_string());
            }
            if let Some(tests) = boundary.pointer("/smoke/tests").and_then(Value::as_array) {
                for test_id in tests.iter().filter_map(Value::as_str) {
                    add_boundary_test_requirement(
                        &mut requirements,
                        test_id,
                        false,
                        boundary_id,
                        &worlds_allowed,
                    );
                }
            }
            if boundary
                .pointer("/pbt/required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                if let Some(tests) = boundary.pointer("/pbt/tests").and_then(Value::as_array) {
                    for test_id in tests.iter().filter_map(Value::as_str) {
                        add_boundary_test_requirement(
                            &mut requirements,
                            test_id,
                            true,
                            boundary_id,
                            &worlds_allowed,
                        );
                    }
                }
            }
        }
    }
    Ok(requirements)
}

fn add_boundary_test_requirement(
    requirements: &mut BoundaryEvidenceRequirements,
    test_id: &str,
    expects_pbt: bool,
    boundary_id: &str,
    worlds_allowed: &[String],
) {
    let entry = requirements
        .required_tests
        .entry(test_id.to_string())
        .or_default();
    entry.expects_pbt |= expects_pbt;
    entry.boundary_ids.insert(boundary_id.to_string());
    entry.worlds_allowed.extend(worlds_allowed.iter().cloned());
}

fn load_tests_manifest_requirements(
    tests_manifest: &Path,
) -> Result<BTreeMap<String, ManifestTestRequirement>> {
    let manifest_doc = report_common::read_json_file(tests_manifest)?;
    let mut tests = BTreeMap::new();
    if let Some(entries) = manifest_doc.get("tests").and_then(Value::as_array) {
        for entry in entries {
            let Some(id) = entry.get("id").and_then(Value::as_str) else {
                continue;
            };
            let world = entry
                .get("world")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let has_pbt = entry.get("pbt").is_some();
            tests.insert(id.to_string(), ManifestTestRequirement { world, has_pbt });
        }
    }
    Ok(tests)
}

fn validate_boundary_tests_against_manifest(
    tests_manifest: &Path,
    profile: Option<&TrustProfile>,
    boundary_requirements: &BoundaryEvidenceRequirements,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<()> {
    let manifest_tests = load_tests_manifest_requirements(tests_manifest)?;
    if let Some(profile) = profile {
        for (test_id, info) in &manifest_tests {
            if !profile
                .worlds_allowed
                .iter()
                .any(|world| world == &info.world)
            {
                diagnostics.push(trust_diag(
                    "X07TC_ETESTS",
                    format!(
                        "test {:?} uses world {:?} outside trust profile {}",
                        test_id, info.world, profile.id
                    ),
                ));
            }
        }
    }
    for (test_id, requirement) in &boundary_requirements.required_tests {
        let code = if requirement.expects_pbt {
            "X07TC_EPBT"
        } else {
            "X07TC_ETESTS"
        };
        let Some(info) = manifest_tests.get(test_id) else {
            diagnostics.push(trust_diag(
                code,
                format!(
                    "boundary-required test {:?} is missing from {}",
                    test_id,
                    tests_manifest.display()
                ),
            ));
            continue;
        };
        if requirement.expects_pbt && !info.has_pbt {
            diagnostics.push(trust_diag(
                "X07TC_EPBT",
                format!(
                    "boundary-required property test {:?} is not declared as a pbt test",
                    test_id
                ),
            ));
        }
        if !requirement.worlds_allowed.is_empty()
            && !requirement.worlds_allowed.contains(&info.world)
        {
            diagnostics.push(trust_diag(
                code,
                format!(
                    "boundary-required test {:?} uses world {:?}, expected one of {:?}",
                    test_id, info.world, requirement.worlds_allowed
                ),
            ));
        }
    }
    Ok(())
}

fn validate_boundary_tests_against_report(
    report_doc: &Value,
    boundary_requirements: &BoundaryEvidenceRequirements,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let mut statuses = BTreeMap::new();
    if let Some(entries) = report_doc.get("tests").and_then(Value::as_array) {
        for entry in entries {
            let Some(id) = entry.get("id").and_then(Value::as_str) else {
                continue;
            };
            let status = entry
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            statuses.insert(id.to_string(), status);
        }
    }

    for (test_id, requirement) in &boundary_requirements.required_tests {
        let code = if requirement.expects_pbt {
            "X07TC_EPBT"
        } else {
            "X07TC_ETESTS"
        };
        match statuses.get(test_id).map(String::as_str) {
            Some("pass") => {}
            Some(status) => diagnostics.push(trust_diag(
                code,
                format!(
                    "boundary-required test {:?} did not pass (status = {:?})",
                    test_id, status
                ),
            )),
            None => diagnostics.push(trust_diag(
                code,
                format!(
                    "boundary-required test {:?} was not present in tests.report.json",
                    test_id
                ),
            )),
        }
    }
}

fn build_schema_derive_evidence(
    project_root: &Path,
    boundary_requirements: &BoundaryEvidenceRequirements,
    out_dir: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Result<Vec<EvidenceRef>> {
    let reports_dir = out_dir.join("schema-derive");
    std::fs::create_dir_all(&reports_dir)
        .with_context(|| format!("create schema derive dir: {}", reports_dir.display()))?;

    let mut refs = Vec::new();
    for schema_path in &boundary_requirements.schema_paths {
        let report_path = reports_dir.join(format!(
            "{}.json",
            util::safe_artifact_dir_name(schema_path)
        ));
        let input_path = project_root.join(schema_path);
        if !input_path.is_file() {
            diagnostics.push(trust_diag(
                "X07TC_ESCHEMA_DRIFT",
                format!(
                    "boundary-referenced schema is missing: {}",
                    input_path.display()
                ),
            ));
            refs.push(write_stub_artifact(
                &report_path,
                "schema_derive",
                &format!("missing schema input {}", input_path.display()),
            )?);
            continue;
        }

        let args = vec![
            "schema".to_string(),
            "derive".to_string(),
            "--input".to_string(),
            schema_path.clone(),
            "--out-dir".to_string(),
            ".".to_string(),
            "--check".to_string(),
            "--report-out".to_string(),
            report_path.display().to_string(),
            "--quiet-json".to_string(),
        ];
        let run = run_self_command(project_root, &args)?;
        if run.exit_code != 0 {
            diagnostics.push(trust_diag(
                "X07TC_ESCHEMA_DRIFT",
                format!(
                    "x07 schema derive --check failed for {:?}: {}",
                    schema_path,
                    stderr_summary(&run.stderr)
                ),
            ));
        }
        refs.push(if report_path.is_file() {
            evidence_ref_for_path(&report_path)?
        } else {
            write_stub_artifact(
                &report_path,
                "schema_derive",
                &format!("missing schema derive report for {schema_path}"),
            )?
        });
    }
    Ok(refs)
}

fn run_self_command(cwd: &Path, args: &[String]) -> Result<ToolRunOutcome> {
    let exe = std::env::current_exe().context("resolve current x07 executable")?;
    let out = Command::new(exe)
        .current_dir(cwd)
        .args(args)
        .output()
        .with_context(|| format!("run x07 command in {}", cwd.display()))?;
    Ok(ToolRunOutcome {
        exit_code: out.status.code().unwrap_or(-1),
        stderr: out.stderr,
    })
}

fn write_json_artifact(path: &Path, value: &Value) -> Result<()> {
    let bytes = report_common::canonical_pretty_json_bytes(value)?;
    util::write_atomic(path, &bytes).with_context(|| format!("write artifact: {}", path.display()))
}

fn write_stub_artifact(path: &Path, artifact: &str, message: &str) -> Result<EvidenceRef> {
    let value = json!({
        "ok": false,
        "artifact": artifact,
        "message": message,
    });
    write_json_artifact(path, &value)?;
    evidence_ref_for_path(path)
}

fn evidence_ref_for_path(path: &Path) -> Result<EvidenceRef> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read artifact: {}", path.display()))?;
    Ok(EvidenceRef {
        path: path.display().to_string(),
        sha256_hex: util::sha256_hex(&bytes),
    })
}

fn sha256_hex_for_path(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read artifact: {}", path.display()))?;
    Ok(util::sha256_hex(&bytes))
}

fn stderr_summary(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr).trim().to_string();
    if text.is_empty() {
        "no stderr output".to_string()
    } else {
        text
    }
}

fn trust_diag(code: &str, message: impl Into<String>) -> diagnostics::Diagnostic {
    reporting::diag_error(code, diagnostics::Stage::Run, &message.into())
}

fn trust_diag_with_path(
    code: &str,
    message: impl Into<String>,
    path: &Path,
) -> diagnostics::Diagnostic {
    let mut diag = trust_diag(code, message);
    diag.data.insert(
        "path".to_string(),
        Value::String(path.display().to_string()),
    );
    diag
}

fn build_certificate_tcb() -> TrustCertificateTcb {
    TrustCertificateTcb {
        x07_version: env!("CARGO_PKG_VERSION").to_string(),
        host_compiler: host_compiler_identity(),
        trusted_primitive_manifest_digest: format!(
            "sha256:{}",
            util::sha256_hex(X07_VERIFY_PRIMITIVES_CATALOG_BYTES)
        ),
    }
}

fn host_compiler_identity() -> String {
    let cc = std::env::var_os("X07_CC").unwrap_or_else(|| "cc".into());
    let cc_str = cc.to_string_lossy().trim().to_string();
    let fallback = if cc_str.is_empty() {
        "cc".to_string()
    } else {
        cc_str.clone()
    };
    let Ok(out) = Command::new(&cc).arg("--version").output() else {
        return fallback;
    };
    let version_text = String::from_utf8_lossy(&out.stdout);
    let first_line = version_text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    if first_line.is_empty() || fallback == first_line {
        fallback
    } else {
        format!("{fallback} ({first_line})")
    }
}

fn normalize_sensitive_namespace_set(items: &[String]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for raw in items {
        let mut s = raw.trim().to_string();
        if s.is_empty() {
            continue;
        }
        if !s.ends_with('.') {
            s.push('.');
        }
        out.insert(s);
    }
    out
}

fn deps_cap_allowlist(policy: &DepsCapabilityPolicy, pkg_name: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for pkg in &policy.packages {
        if pkg.name == pkg_name {
            out.extend(normalize_sensitive_namespace_set(
                &pkg.allow_sensitive_namespaces,
            ));
        }
    }
    out
}

fn build_sbom(
    args: &TrustReportArgs,
    out_path: &Path,
    sbom_components: Vec<SbomComponent>,
    sbom_diags: &mut Vec<diagnostics::Diagnostic>,
) -> Sbom {
    let format = match args.sbom_format {
        SbomFormat::None => "none",
        SbomFormat::Cyclonedx => "cyclonedx",
        SbomFormat::Spdx => "spdx",
    }
    .to_string();

    let (cyclonedx, spdx) = match args.sbom_format {
        SbomFormat::Cyclonedx => (Some(json!({ "spec_version": "1.5" })), None),
        SbomFormat::Spdx => (None, Some(json!({ "spec_version": "2.3" }))),
        SbomFormat::None => (None, None),
    };

    let mut generated = false;
    let mut path = None;

    let sbom_out_path = match args.sbom_format {
        SbomFormat::Cyclonedx => Some(out_path.with_extension("sbom.cdx.json")),
        SbomFormat::Spdx => Some(out_path.with_extension("sbom.spdx.json")),
        SbomFormat::None => None,
    };

    if let Some(sbom_out_path) = sbom_out_path {
        let doc = match args.sbom_format {
            SbomFormat::Cyclonedx => build_cyclonedx_sbom(&sbom_components),
            SbomFormat::Spdx => build_spdx_sbom(&sbom_components),
            SbomFormat::None => Value::Null,
        };

        match report_common::canonical_pretty_json_bytes(&doc) {
            Ok(bytes) => match util::write_atomic(&sbom_out_path, &bytes) {
                Ok(()) => {
                    generated = true;
                    path = Some(sbom_out_path.display().to_string());
                }
                Err(err) => {
                    let mut diag = reporting::diag_error(
                        "E_SBOM_GENERATION_FAILED",
                        diagnostics::Stage::Lint,
                        "write SBOM artifact failed",
                    );
                    diag.data.insert(
                        "path".to_string(),
                        Value::String(sbom_out_path.display().to_string()),
                    );
                    diag.data
                        .insert("error".to_string(), Value::String(err.to_string()));
                    sbom_diags.push(diag);
                }
            },
            Err(err) => {
                let mut diag = reporting::diag_error(
                    "E_SBOM_GENERATION_FAILED",
                    diagnostics::Stage::Lint,
                    "generate SBOM artifact failed",
                );
                diag.data.insert(
                    "path".to_string(),
                    Value::String(sbom_out_path.display().to_string()),
                );
                diag.data
                    .insert("error".to_string(), Value::String(err.to_string()));
                sbom_diags.push(diag);
            }
        }
    } else if args.fail_on.contains(&TrustFailOn::SbomMissing) {
        sbom_diags.push(reporting::diag_error(
            "E_SBOM_GENERATION_FAILED",
            diagnostics::Stage::Lint,
            "SBOM generation disabled (--sbom-format none)",
        ));
    }

    Sbom {
        format,
        generated,
        path,
        cyclonedx,
        spdx,
        components: sbom_components,
    }
}

fn build_cyclonedx_sbom(sbom_components: &[SbomComponent]) -> Value {
    let components: Vec<Value> = sbom_components
        .iter()
        .map(|c| {
            let id = format!(
                "{}\t{}\t{}",
                c.kind,
                c.name,
                c.version.as_deref().unwrap_or("")
            );
            let bom_ref = format!("x07:{}", util::sha256_hex(id.as_bytes()));
            let component_type = if c.kind == "toolchain" {
                "application"
            } else {
                "library"
            };

            let mut obj = serde_json::Map::new();
            obj.insert(
                "type".to_string(),
                Value::String(component_type.to_string()),
            );
            obj.insert("name".to_string(), Value::String(c.name.clone()));
            if let Some(version) = c.version.as_deref() {
                obj.insert("version".to_string(), Value::String(version.to_string()));
            }
            obj.insert("bom-ref".to_string(), Value::String(bom_ref));
            Value::Object(obj)
        })
        .collect();

    json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "tools": {
                "components": [
                    {
                        "type": "application",
                        "name": "x07",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                ]
            }
        },
        "components": components,
    })
}

fn build_spdx_sbom(sbom_components: &[SbomComponent]) -> Value {
    let mut lines: Vec<String> = Vec::new();
    for c in sbom_components {
        lines.push(format!(
            "{}\t{}\t{}",
            c.kind,
            c.name,
            c.version.as_deref().unwrap_or("")
        ));
    }
    let hash = util::sha256_hex(lines.join("\n").as_bytes());

    let mut packages = Vec::new();
    for (idx, c) in sbom_components.iter().enumerate() {
        let spdx_id = format!("SPDXRef-Package-{}", idx + 1);
        let mut pkg = serde_json::Map::new();
        pkg.insert("SPDXID".to_string(), Value::String(spdx_id));
        pkg.insert("name".to_string(), Value::String(c.name.clone()));
        pkg.insert(
            "downloadLocation".to_string(),
            Value::String("NOASSERTION".to_string()),
        );
        if let Some(version) = c.version.as_deref() {
            pkg.insert(
                "versionInfo".to_string(),
                Value::String(version.to_string()),
            );
        }
        packages.push(Value::Object(pkg));
    }

    json!({
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": "x07 trust SBOM",
        "documentNamespace": format!("https://x07.io/spdx/{}", hash),
        "creationInfo": {
            "created": "2000-01-01T00:00:00Z",
            "creators": [
                format!("Tool: x07-{}", env!("CARGO_PKG_VERSION"))
            ]
        },
        "packages": packages
    })
}

fn now_unix_ms() -> u64 {
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return 0;
    };
    now.as_millis() as u64
}

fn stdlib_sbom_components(path: &Path) -> Vec<SbomComponent> {
    let Ok(doc) = report_common::read_json_file(path) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let Some(packages) = doc.get("packages").and_then(Value::as_array) else {
        return out;
    };
    for pkg in packages {
        let Some(name) = pkg.get("name").and_then(Value::as_str) else {
            continue;
        };
        let version = pkg
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let key = (name.to_string(), version);
        if !seen.insert(key.clone()) {
            continue;
        }
        out.push(SbomComponent {
            kind: "stdlib".to_string(),
            name: key.0,
            version: if key.1.is_empty() { None } else { Some(key.1) },
            source: Some(path.display().to_string()),
            purl: None,
            license: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "x07_trust_{prefix}_{}_{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn evidence_ref(path: &str, ch: char) -> EvidenceRef {
        EvidenceRef {
            path: path.to_string(),
            sha256_hex: ch.to_string().repeat(64),
        }
    }

    fn proof_inventory_item(
        symbol: &str,
        kind: &str,
        proof_check_result: Option<&str>,
    ) -> TrustCertificateProofInventoryItem {
        TrustCertificateProofInventoryItem {
            symbol: symbol.to_string(),
            kind: kind.to_string(),
            result_kind: if kind == "defasync" {
                "proven_async".to_string()
            } else {
                "proven".to_string()
            },
            verify_report: evidence_ref("verify.prove.report.json", '1'),
            proof_summary: evidence_ref("verify.proof-summary.json", '2'),
            proof_object: Some(evidence_ref("proof.json", '3')),
            proof_check_report: Some(evidence_ref("proof.check.json", '4')),
            proof_check_result: proof_check_result.map(str::to_string),
            proof_check_checker: Some("x07.proof_replay_checker".to_string()),
            proof_object_digest: Some(format!("sha256:{}", "5".repeat(64))),
        }
    }

    #[test]
    fn trust_certificate_serialization_keeps_empty_prove_reports() {
        let certificate = TrustCertificate {
            schema_version: X07_TRUST_CERTIFICATE_SCHEMA_VERSION,
            verdict: "rejected".to_string(),
            profile: "verified_core_pure_v1".to_string(),
            entry: "app.main".to_string(),
            operational_entry_symbol: "app.main".to_string(),
            out_dir: "target/cert".to_string(),
            claims: vec!["human_can_review_certificate_not_source".to_string()],
            formal_verification_scope: "none".to_string(),
            proved_symbol_count: 0,
            proved_defn_count: 0,
            proved_defasync_count: 0,
            entry_body_formally_proved: false,
            operational_entry_proof_inventory_refs: Vec::new(),
            capsule_boundary_only_symbol_count: 0,
            runtime_evidence_only_symbol_count: 0,
            async_proof: TrustCertificateAsyncProof {
                reachable: 0,
                proved: 0,
                model: None,
            },
            proof_inventory: Vec::new(),
            proof_assumptions: Vec::new(),
            recursive_proof_summary: TrustCertificateRecursiveProofSummary {
                reachable_recursive_defn: 0,
                accepted_recursive_defn: 0,
                bounded_recursive_defn: 0,
                unbounded_recursive_defn: 0,
                imported_proof_summary_defn: 0,
                rejected_recursive_defn: 0,
                accepted_depends_on_bounded_proof: false,
            },
            imported_summary_inventory: Vec::new(),
            accepted_depends_on_bounded_proof: false,
            accepted_depends_on_dev_only_assumption: false,
            capsules: TrustCertificateCapsules {
                count: 0,
                ids: Vec::new(),
                attestations: Vec::new(),
            },
            network_capsules: TrustCertificateNetworkCapsules {
                count: 0,
                ids: Vec::new(),
            },
            runtime: None,
            package_set_digest: None,
            dependency_closure: None,
            effect_logs: Vec::new(),
            tcb: TrustCertificateTcb {
                x07_version: "0.1.78".to_string(),
                host_compiler: "cc (clang 18.1.0)".to_string(),
                trusted_primitive_manifest_digest: format!("sha256:{}", "f".repeat(64)),
            },
            evidence: TrustCertificateEvidence {
                boundaries_report: EvidenceRef {
                    path: "boundaries.report.json".to_string(),
                    sha256_hex: "0".repeat(64),
                },
                coverage_report: EvidenceRef {
                    path: "verify.coverage.json".to_string(),
                    sha256_hex: "1".repeat(64),
                },
                verify_summary_report: None,
                schema_derive_reports: Vec::new(),
                prove_reports: Vec::new(),
                tests_report: EvidenceRef {
                    path: "tests.report.json".to_string(),
                    sha256_hex: "2".repeat(64),
                },
                trust_report: EvidenceRef {
                    path: "trust.report.json".to_string(),
                    sha256_hex: "3".repeat(64),
                },
                compile_attestation: EvidenceRef {
                    path: "compile.attest.json".to_string(),
                    sha256_hex: "4".repeat(64),
                },
                runtime_attestation: None,
                peer_policy_files: Vec::new(),
                capsule_attestations: Vec::new(),
                effect_logs: Vec::new(),
                review_diff: None,
                dependency_closure_attestation: None,
                bundle_path: None,
            },
            diagnostics: Vec::new(),
        };

        let value = serde_json::to_value(&certificate).expect("serialize certificate");
        let prove_reports = value
            .pointer("/evidence/prove_reports")
            .and_then(Value::as_array)
            .expect("prove_reports array");
        assert!(prove_reports.is_empty());
        assert!(value
            .get("async_proof")
            .and_then(|v| v.get("model"))
            .is_some());
        assert!(value
            .pointer("/async_proof/model")
            .is_some_and(Value::is_null));
        assert_eq!(
            value
                .pointer("/tcb/trusted_primitive_manifest_digest")
                .and_then(Value::as_str),
            Some(format!("sha256:{}", "f".repeat(64)).as_str())
        );

        let schema_diags = report_common::validate_schema(
            X07_TRUST_CERTIFICATE_SCHEMA_BYTES,
            "spec/x07-trust.certificate.schema.json",
            &value,
        )
        .expect("validate certificate schema");
        assert!(schema_diags.is_empty(), "schema diags: {schema_diags:?}");
    }

    #[test]
    fn trust_certificate_runtime_serialization_keeps_null_attestation() {
        let runtime = TrustCertificateRuntime {
            backend: "vm".to_string(),
            network_mode: "none".to_string(),
            network_enforcement: "none".to_string(),
            weaker_isolation: false,
            effective_allow_hosts: Vec::new(),
            policy_digest_bound: false,
            guest_image_digest_bound: false,
            attestation: None,
        };
        let value = serde_json::to_value(runtime).expect("serialize runtime");
        assert!(value.get("attestation").is_some());
        assert!(value.get("attestation").is_some_and(Value::is_null));
    }

    #[test]
    fn collect_formal_verification_scope_summary_reports_entry_body_truth() {
        let dir = temp_test_dir("formal_verification_scope_entry");
        let coverage_path = dir.join("verify.coverage.json");
        std::fs::write(
            &coverage_path,
            serde_json::to_vec_pretty(&json!({
                "functions": [
                    { "symbol": "example.main", "status": "supported_async" },
                    { "symbol": "capsule.main", "status": "capsule_boundary" }
                ]
            }))
            .expect("serialize coverage"),
        )
        .expect("write coverage");

        let runtime = TrustCertificateRuntime {
            backend: "vm".to_string(),
            network_mode: "local_only".to_string(),
            network_enforcement: "vm".to_string(),
            weaker_isolation: false,
            effective_allow_hosts: Vec::new(),
            policy_digest_bound: true,
            guest_image_digest_bound: true,
            attestation: Some(evidence_ref("runtime.attest.json", '6')),
        };
        let summary = collect_formal_verification_scope_summary(
            &[proof_inventory_item(
                "example.main",
                "defasync",
                Some("accepted"),
            )],
            &coverage_path,
            "example.main",
            Some(&runtime),
        )
        .expect("collect scope");

        assert_eq!(summary.formal_verification_scope, "whole_certifiable_graph");
        assert_eq!(summary.proved_symbol_count, 1);
        assert_eq!(summary.proved_defn_count, 0);
        assert_eq!(summary.proved_defasync_count, 1);
        assert!(summary.entry_body_formally_proved);
        assert_eq!(summary.operational_entry_proof_inventory_refs.len(), 1);
        assert_eq!(summary.capsule_boundary_only_symbol_count, 1);
        assert_eq!(summary.runtime_evidence_only_symbol_count, 0);

        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn collect_formal_verification_scope_summary_reports_runtime_only_scope() {
        let dir = temp_test_dir("formal_verification_scope_runtime");
        let coverage_path = dir.join("missing.verify.coverage.json");
        let runtime = TrustCertificateRuntime {
            backend: "vm".to_string(),
            network_mode: "local_only".to_string(),
            network_enforcement: "vm".to_string(),
            weaker_isolation: false,
            effective_allow_hosts: Vec::new(),
            policy_digest_bound: true,
            guest_image_digest_bound: true,
            attestation: Some(evidence_ref("runtime.attest.json", '7')),
        };

        let summary = collect_formal_verification_scope_summary(
            &[],
            &coverage_path,
            "example.main",
            Some(&runtime),
        )
        .expect("collect scope");

        assert_eq!(summary.formal_verification_scope, "none");
        assert_eq!(summary.proved_symbol_count, 0);
        assert!(!summary.entry_body_formally_proved);
        assert!(summary.operational_entry_proof_inventory_refs.is_empty());
        assert_eq!(summary.capsule_boundary_only_symbol_count, 0);
        assert_eq!(summary.runtime_evidence_only_symbol_count, 1);

        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn realized_certificate_claims_only_emit_true_formal_claims() {
        let profile_claims = vec![
            "human_can_review_certificate_not_source".to_string(),
            "certificate_includes_formal_proof".to_string(),
            "operational_entry_formally_proved".to_string(),
        ];

        assert!(realized_certificate_claims(&profile_claims, false, 1, true).is_empty());

        let proof_only = realized_certificate_claims(&profile_claims, true, 1, false);
        assert_eq!(
            proof_only,
            vec![
                "human_can_review_certificate_not_source".to_string(),
                "certificate_includes_formal_proof".to_string(),
            ]
        );

        let entry_proved = realized_certificate_claims(&profile_claims, true, 1, true);
        assert_eq!(
            entry_proved,
            vec![
                "human_can_review_certificate_not_source".to_string(),
                "certificate_includes_formal_proof".to_string(),
                "operational_entry_formally_proved".to_string(),
            ]
        );
    }

    #[test]
    fn load_boundary_requirements_collects_schema_paths_and_required_tests() {
        let dir = temp_test_dir("boundary_requirements");
        std::fs::create_dir_all(dir.join("arch/boundaries")).expect("create boundaries dir");
        std::fs::write(
            dir.join("arch/manifest.x07arch.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": "x07.arch.manifest@0.3.0",
                "repo": {"id": "fixture", "root": "."},
                "nodes": [],
                "rules": [],
                "checks": {
                    "deny_cycles": true,
                    "deny_orphans": true,
                    "enforce_visibility": true,
                    "enforce_world_caps": true
                },
                "contracts_v1": {
                    "boundaries": {
                        "index_path": "arch/boundaries/index.x07boundary.json",
                        "enforce": "error"
                    }
                }
            }))
            .expect("serialize manifest"),
        )
        .expect("write manifest");
        std::fs::write(
            dir.join("arch/boundaries/index.x07boundary.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": "x07.arch.boundaries.index@0.1.0",
                "boundaries": [
                    {
                        "id": "app.main_v1",
                        "symbol": "app.main_v1",
                        "node_id": "app_core",
                        "kind": "public_function",
                        "from_zone": "verified_core",
                        "to_zone": "verified_core",
                        "worlds_allowed": ["solve-pure"],
                        "input": {
                            "params": [
                                {
                                    "name": "req",
                                    "ty": "bytes_view",
                                    "schema": "arch/schemas/request.x07schema.json"
                                }
                            ]
                        },
                        "output": {
                            "ty": "result_bytes",
                            "schema": "arch/schemas/response.x07schema.json",
                            "error_space": "app.main_errors_v1"
                        },
                        "smoke": {
                            "entry": "tests.core.smoke_main_v1",
                            "tests": ["smoke/main"]
                        },
                        "pbt": {
                            "required": true,
                            "tests": ["pbt/main"]
                        },
                        "verify": {
                            "required": true,
                            "mode": "prove"
                        }
                    }
                ]
            }))
            .expect("serialize boundaries"),
        )
        .expect("write boundaries");

        let requirements = load_boundary_requirements(&dir).expect("load boundary requirements");
        assert!(requirements
            .schema_paths
            .contains("arch/schemas/request.x07schema.json"));
        assert!(requirements
            .schema_paths
            .contains("arch/schemas/response.x07schema.json"));
        assert!(requirements
            .required_tests
            .get("smoke/main")
            .is_some_and(|req| !req.expects_pbt));
        assert!(requirements
            .required_tests
            .get("pbt/main")
            .is_some_and(|req| req.expects_pbt));
        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn validate_boundary_tests_against_manifest_requires_pbt_entries() {
        let dir = temp_test_dir("boundary_test_manifest");
        let tests_manifest = dir.join("tests.json");
        std::fs::write(
            &tests_manifest,
            serde_json::to_vec_pretty(&json!({
                "schema_version": "x07.tests_manifest@0.2.0",
                "tests": [
                    {
                        "id": "smoke/main",
                        "entry": "tests.core.smoke_main_v1",
                        "expect": "pass",
                        "world": "solve-pure"
                    },
                    {
                        "id": "pbt/main",
                        "entry": "tests.core.pbt_main_v1",
                        "expect": "pass",
                        "world": "solve-pure"
                    }
                ]
            }))
            .expect("serialize tests manifest"),
        )
        .expect("write tests manifest");

        let mut diagnostics = Vec::new();
        let mut requirements = BoundaryEvidenceRequirements::default();
        add_boundary_test_requirement(
            &mut requirements,
            "smoke/main",
            false,
            "app.main_v1",
            &["solve-pure".to_string()],
        );
        add_boundary_test_requirement(
            &mut requirements,
            "pbt/main",
            true,
            "app.main_v1",
            &["solve-pure".to_string()],
        );

        validate_boundary_tests_against_manifest(
            &tests_manifest,
            None,
            &requirements,
            &mut diagnostics,
        )
        .expect("validate manifest requirements");

        assert!(diagnostics.iter().any(|diag| {
            diag.code == "X07TC_EPBT"
                && diag.message.contains(
                    "boundary-required property test \"pbt/main\" is not declared as a pbt test",
                )
        }));
        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn add_coverage_diagnostics_splits_certification_failures() {
        let mut diagnostics = Vec::new();
        add_coverage_diagnostics(
            &json!({
                "functions": [
                    {
                        "symbol": "example.async_main",
                        "kind": "defasync",
                        "status": "unsupported"
                    },
                    {
                        "symbol": "example.async_worker",
                        "kind": "defasync",
                        "status": "uncovered"
                    },
                    {
                        "symbol": "example.loop_missing_contracts",
                        "kind": "defn",
                        "status": "uncovered"
                    },
                    {
                        "symbol": "example.recursive_main",
                        "kind": "defn",
                        "status": "unsupported",
                        "proof_summary": {
                            "recursion_kind": "self_recursive",
                            "has_decreases": false,
                            "decreases_count": 0,
                            "prove_supported": false
                        }
                    },
                    {
                        "symbol": "example.unsupported_param",
                        "kind": "defn",
                        "status": "unsupported",
                        "details": "unsupported verify param type"
                    }
                ]
            }),
            Path::new("."),
            None,
            &mut diagnostics,
        );

        let codes = diagnostics
            .iter()
            .map(|diag| diag.code.as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("X07TC_EASYNC_PROOF"));
        assert!(codes.contains("X07TC_EUNSUPPORTED_DEFASYNC"));
        assert!(codes.contains("X07TC_EPROOF_COVERAGE"));
        assert!(codes.contains("X07TC_EUNSUPPORTED_RECURSION"));
        assert!(codes.contains("X07TC_EPROVE_UNSUPPORTED"));
    }

    #[test]
    fn add_coverage_diagnostics_rejects_recursive_symbols_when_profile_forbids_recursion() {
        let profile = TrustProfile {
            schema_version: "x07.trust.profile@0.4.0".to_string(),
            id: "verified_core_pure_v1".to_string(),
            claims: Vec::new(),
            entrypoints: Vec::new(),
            worlds_allowed: vec!["solve-pure".to_string()],
            language_subset: TrustLanguageSubset {
                allow_defasync: false,
                allow_recursion: false,
                allow_extern: false,
                allow_unsafe: false,
                allow_ffi: false,
                allow_dynamic_dispatch: false,
            },
            arch_requirements: TrustArchRequirements {
                manifest_min_version: "x07.arch.manifest@0.3.0".to_string(),
                require_allowlist_mode: true,
                require_deny_cycles: true,
                require_deny_orphans: true,
                require_visibility: true,
                require_world_caps: true,
                require_brand_boundaries: true,
            },
            evidence_requirements: TrustEvidenceRequirements {
                require_boundary_index: true,
                require_schema_derive_check: true,
                require_smoke_harnesses: true,
                require_unit_tests: true,
                require_pbt: "required".to_string(),
                require_proof_mode: "prove".to_string(),
                require_proof_coverage: "all_reachable_symbols".to_string(),
                require_async_proof_coverage: false,
                require_per_symbol_prove_reports_defn: true,
                require_per_symbol_prove_reports_async: false,
                allow_coverage_summary_imports: false,
                require_capsule_attestations: false,
                require_runtime_attestation: false,
                require_effect_log_digests: false,
                require_peer_policies: false,
                require_network_capsules: false,
                require_dependency_closure_attestation: false,
                require_compile_attestation: true,
                require_trust_report_clean: true,
                require_sbom: true,
            },
            sandbox_requirements: TrustSandboxRequirements {
                sandbox_backend: "any".to_string(),
                forbid_weaker_isolation: false,
                network_mode: "any".to_string(),
                network_enforcement: "any".to_string(),
            },
        };
        let mut diagnostics = Vec::new();
        add_coverage_diagnostics(
            &json!({
                "functions": [
                    {
                        "symbol": "example.recursive_supported",
                        "kind": "defn",
                        "status": "supported_recursive",
                        "support_summary": {
                            "recursion_kind": "self_recursive"
                        }
                    },
                    {
                        "symbol": "example.recursive_imported",
                        "kind": "defn",
                        "status": "imported_proof_summary",
                        "proof_summary": {
                            "recursion_kind": "self_recursive"
                        }
                    }
                ]
            }),
            Path::new("."),
            Some(&profile),
            &mut diagnostics,
        );

        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "X07TC_ERECURSION_FORBIDDEN"));
    }

    #[test]
    fn add_review_diff_diagnostics_splits_boundary_and_posture_failures() {
        let mut diagnostics = Vec::new();
        add_review_diff_diagnostics(
            &json!({
                "highlights": {
                    "proof_changes": [{"subject": "proof coverage summary"}],
                    "boundary_changes": [{"subject": "example.api.sum_v1"}],
                    "subset_changes": [{"subject": "trust profile relaxation"}],
                    "summary_changes": [{"subject": "trust summary regression"}],
                    "network_policy_changes": [{"subject": "policy/run-os.json"}],
                    "peer_policy_changes": [{"subject": "arch/capsules/upstream.peer_policy.json"}],
                    "capsule_network_changes": [{"subject": "capsule.echo_v1"}],
                    "dependency_closure_changes": [{"subject": "x07.lock.json"}]
                }
            }),
            &mut diagnostics,
        );

        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "X07TC_EDIFF_POSTURE"));
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "X07TC_EBOUNDARY_RELAXED"));
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "X07TC_ENET_POLICY"));
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "X07TC_EPEER_POLICY"));
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "X07TC_ECAPSULE_NETWORK_ATTEST"));
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "X07TC_EDEP_CLOSURE"));
    }

    #[test]
    fn prove_issue_code_maps_async_verify_failures() {
        assert_eq!(
            prove_issue_code(&json!({
                "diagnostics": [
                    { "code": "X07V_SCOPE_INVARIANT_FAILED" }
                ]
            })),
            "X07TC_EASYNC_PROOF"
        );
        assert_eq!(
            prove_issue_code(&json!({
                "diagnostics": [
                    { "code": "X07V_UNSUPPORTED_DEFASYNC_FORM" }
                ]
            })),
            "X07TC_EUNSUPPORTED_DEFASYNC"
        );
        assert_eq!(
            prove_issue_code(&json!({
                "diagnostics": [
                    { "code": "X07V_RECURSIVE_DECREASES_REQUIRED" }
                ]
            })),
            "X07TC_EUNSUPPORTED_RECURSION"
        );
        assert_eq!(
            prove_issue_code(&json!({
                "diagnostics": [
                    { "code": "X07V_UNSUPPORTED_MUTUAL_RECURSION" }
                ]
            })),
            "X07TC_EUNSUPPORTED_RECURSION"
        );
    }

    #[test]
    fn collect_runtime_attestation_reports_sandbox_profile_mismatch() {
        let dir = temp_test_dir("runtime_attestation_profile_mismatch");
        let tests_report = dir.join("tests.report.json");
        std::fs::write(
            &tests_report,
            serde_json::to_vec_pretty(&json!({
                "tests": [
                    {
                        "id": "smoke/runtime",
                        "run": {
                            "sandbox_backend": "os"
                        }
                    }
                ]
            }))
            .expect("serialize tests report"),
        )
        .expect("write tests report");

        let profile = TrustProfile {
            schema_version: "x07.trust.profile@0.4.0".to_string(),
            id: "trusted_program_sandboxed_local_v1".to_string(),
            claims: Vec::new(),
            entrypoints: Vec::new(),
            worlds_allowed: vec!["run-os-sandboxed".to_string()],
            language_subset: TrustLanguageSubset {
                allow_defasync: true,
                allow_recursion: false,
                allow_extern: false,
                allow_unsafe: false,
                allow_ffi: false,
                allow_dynamic_dispatch: false,
            },
            arch_requirements: TrustArchRequirements {
                manifest_min_version: "x07.arch.manifest@0.3.0".to_string(),
                require_allowlist_mode: true,
                require_deny_cycles: true,
                require_deny_orphans: true,
                require_visibility: true,
                require_world_caps: true,
                require_brand_boundaries: true,
            },
            evidence_requirements: TrustEvidenceRequirements {
                require_boundary_index: true,
                require_schema_derive_check: true,
                require_smoke_harnesses: true,
                require_unit_tests: true,
                require_pbt: "required".to_string(),
                require_proof_mode: "prove".to_string(),
                require_proof_coverage: "all_reachable_symbols".to_string(),
                require_async_proof_coverage: true,
                require_per_symbol_prove_reports_defn: true,
                require_per_symbol_prove_reports_async: true,
                allow_coverage_summary_imports: false,
                require_capsule_attestations: true,
                require_runtime_attestation: false,
                require_effect_log_digests: true,
                require_peer_policies: false,
                require_network_capsules: false,
                require_dependency_closure_attestation: false,
                require_compile_attestation: true,
                require_trust_report_clean: true,
                require_sbom: true,
            },
            sandbox_requirements: TrustSandboxRequirements {
                sandbox_backend: "vm".to_string(),
                forbid_weaker_isolation: true,
                network_mode: "none".to_string(),
                network_enforcement: "none".to_string(),
            },
        };

        let mut diagnostics = Vec::new();
        let _ = collect_runtime_attestation(
            &tests_report,
            &dir,
            None,
            Some(&profile),
            &mut diagnostics,
        )
        .expect("collect runtime attestation");

        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "X07TC_ESANDBOX_PROFILE"));
        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }
}
