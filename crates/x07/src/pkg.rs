use std::collections::{BTreeSet, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use x07_contracts::X07_DEP_CLOSURE_ATTEST_SCHEMA_VERSION;
use x07_pkg::SparseIndexClient;
use x07_runner_common::os_paths;
use x07c::builtin_modules;
use x07c::project;

use crate::report_common;
use crate::util;

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(crate) const DEFAULT_INDEX_URL: &str = x07_contracts::X07_PKG_DEFAULT_INDEX_URL;
const PKG_PROVIDES_REPORT_SCHEMA_VERSION: &str = "x07.pkg.provides.report@0.1.0";
const X07_DEP_CLOSURE_ATTEST_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-dep.closure.attest.schema.json");

#[derive(Debug, Clone, Default)]
struct PkgConfig {
    registry: Option<String>,
    offline: Option<bool>,
    #[allow(dead_code)]
    cache_policy: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegistrySource {
    Cli,
    Env,
    Config,
    Default,
}

#[derive(Debug, Clone)]
struct ResolvedRegistry {
    url: String,
    source: RegistrySource,
}

fn load_pkg_config(base: &Path) -> Result<PkgConfig> {
    let mut cfg = PkgConfig::default();

    let paths = [
        base.join(".x07").join("config.json"),
        base.join("x07.config.json"),
    ];
    for path in paths {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
        };
        let doc: Value =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        let Some(obj) = doc.as_object() else {
            anyhow::bail!("config must be a JSON object: {}", path.display());
        };

        if let Some(schema) = obj.get("schema_version").and_then(Value::as_str) {
            let schema = schema.trim();
            if schema == "x07up.config@0.1.0" || schema.starts_with("x07up.") {
                continue;
            }
            if !schema.is_empty() && schema != "x07.config@0.1.0" {
                anyhow::bail!(
                    "unsupported config schema_version {:?} (expected x07.config@0.1.0): {}",
                    schema,
                    path.display()
                );
            }
        }

        let scope = obj.get("pkg").and_then(Value::as_object).unwrap_or(obj);

        let registry = scope
            .get("registry")
            .or_else(|| scope.get("index"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        if registry.is_some() {
            cfg.registry = registry;
        }

        let offline = scope.get("offline").and_then(Value::as_bool);
        if offline.is_some() {
            cfg.offline = offline;
        }

        let cache_policy = scope
            .get("cache_policy")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        if cache_policy.is_some() {
            cfg.cache_policy = cache_policy;
        }
    }

    Ok(cfg)
}

fn env_index_url() -> Option<String> {
    std::env::var("X07_PKG_INDEX_URL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn resolve_registry_url(cli_index: Option<&str>, cfg: &PkgConfig) -> ResolvedRegistry {
    if let Some(cli) = cli_index
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
    {
        return ResolvedRegistry {
            url: cli,
            source: RegistrySource::Cli,
        };
    }
    if let Some(env) = env_index_url() {
        return ResolvedRegistry {
            url: env,
            source: RegistrySource::Env,
        };
    }
    if let Some(config) = cfg
        .registry
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
    {
        return ResolvedRegistry {
            url: config,
            source: RegistrySource::Config,
        };
    }
    ResolvedRegistry {
        url: DEFAULT_INDEX_URL.to_string(),
        source: RegistrySource::Default,
    }
}

fn resolve_pkg_registry_and_offline(
    base: &Path,
    cli_index: Option<&str>,
    cli_offline: bool,
) -> Result<(ResolvedRegistry, bool)> {
    let cfg = load_pkg_config(base)?;
    let registry = resolve_registry_url(cli_index, &cfg);
    let offline = cli_offline || cfg.offline.unwrap_or(false);
    Ok((registry, offline))
}

fn index_url_is_file(index_url: &str) -> bool {
    let raw = index_url.strip_prefix("sparse+").unwrap_or(index_url);
    raw.trim_start().starts_with("file://")
}

fn percent_decode_url_path(raw: &str) -> Result<String> {
    if !raw.as_bytes().contains(&b'%') {
        return Ok(raw.to_string());
    }

    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }

    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        if i + 2 >= bytes.len() {
            anyhow::bail!("invalid percent-encoding in file url path: {:?}", raw);
        }
        let hi = hex(bytes[i + 1]).ok_or_else(|| {
            anyhow::anyhow!("invalid percent-encoding in file url path: {:?}", raw)
        })?;
        let lo = hex(bytes[i + 2]).ok_or_else(|| {
            anyhow::anyhow!("invalid percent-encoding in file url path: {:?}", raw)
        })?;
        out.push((hi << 4) | lo);
        i += 3;
    }

    String::from_utf8(out).context("file url path is not utf-8")
}

fn file_index_url_to_dir(index_url: &str) -> Result<PathBuf> {
    let raw = index_url
        .strip_prefix("sparse+")
        .unwrap_or(index_url)
        .trim();
    let rest = raw
        .strip_prefix("file://")
        .ok_or_else(|| anyhow::anyhow!("expected file:// index url, got {:?}", index_url))?;

    let rest = if let Some(r) = rest.strip_prefix("localhost/") {
        format!("/{r}")
    } else {
        rest.to_string()
    };
    if !rest.starts_with('/') {
        anyhow::bail!(
            "unsupported file index url form {:?} (expected file:///ABS/PATH/)",
            index_url
        );
    }

    let decoded = percent_decode_url_path(&rest)?;
    Ok(PathBuf::from(decoded))
}

#[derive(Debug, Args)]
pub struct PkgArgs {
    #[command(subcommand)]
    pub cmd: Option<PkgCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum PkgCommand {
    /// Add a dependency entry to `x07.json`.
    Add(AddArgs),
    /// Remove a dependency entry from `x07.json`.
    Remove(RemoveArgs),
    /// List available versions of a package from the index.
    Versions(VersionsArgs),
    /// Show package metadata from the index (and local cache when available).
    Info(InfoArgs),
    /// List available packages from a local `file://` sparse index mirror.
    List(ListArgs),
    /// Pack a local package directory into a publishable archive.
    Pack(PackArgs),
    /// Resolve project dependencies and write `x07.lock.json`.
    Lock(LockArgs),
    /// Repair an existing lockfile after a toolchain upgrade.
    Repair(RepairArgs),
    /// Emit a dependency-closure attestation from `x07.json` + `x07.lock.json`.
    AttestClosure(AttestClosureArgs),
    /// Find packages that provide a given module id.
    Provides(ProvidesArgs),
    /// Store a registry token for `pkg publish`.
    Login(LoginArgs),
    /// Publish a package archive to an index.
    Publish(PublishArgs),
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH", default_value = "x07.json")]
    pub project: PathBuf,

    /// After adding the dependency, resolve and update `x07.lock.json`.
    #[arg(long)]
    pub sync: bool,

    /// Sparse index URL used when fetching dependencies (default: official registry).
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: Option<String>,

    /// Override the dependency path stored in `x07.json`.
    #[arg(long, value_name = "PATH")]
    pub path: Option<String>,

    /// Add transitive dependencies from `meta.requires_packages` to x07.json
    /// (without writing the lockfile). Use --sync for full lock+closure.
    #[arg(long)]
    pub with_closure: bool,

    /// Package spec in `NAME` or `NAME@VERSION` form.
    ///
    /// If only `NAME` is provided, this command resolves the latest non-yanked semver version from
    /// the index and writes it into `x07.json`.
    #[arg(value_name = "NAME[@VERSION]")]
    pub spec: String,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH", default_value = "x07.json")]
    pub project: PathBuf,

    /// After removing the dependency, resolve and update `x07.lock.json`.
    #[arg(long)]
    pub sync: bool,

    /// Sparse index URL used when fetching dependencies (default: official registry).
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: Option<String>,

    /// Package name.
    #[arg(value_name = "NAME")]
    pub name: String,
}

#[derive(Debug, Args)]
pub struct VersionsArgs {
    /// Sparse index URL (example: `sparse+https://registry.x07.io/index/`).
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: Option<String>,

    /// Force a fresh sparse-index fetch (cache-busting).
    #[arg(long)]
    pub refresh: bool,

    /// Disallow network access (requires a `file://` registry index).
    #[arg(long)]
    pub offline: bool,

    /// Package name.
    #[arg(value_name = "NAME")]
    pub name: String,
}

#[derive(Debug, Args)]
pub struct InfoArgs {
    /// Sparse index URL (example: `sparse+https://registry.x07.io/index/`).
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: Option<String>,

    /// Disallow network access (requires a `file://` registry index and local package contents).
    #[arg(long)]
    pub offline: bool,

    /// Package spec in `NAME` or `NAME@VERSION` form.
    #[arg(value_name = "NAME[@VERSION]")]
    pub spec: String,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Sparse index URL (example: `sparse+https://registry.x07.io/index/`).
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: Option<String>,

    /// Disallow network access (requires a `file://` registry index).
    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct PackArgs {
    /// Package directory containing `x07-package.json`.
    #[arg(long, value_name = "DIR")]
    pub package: PathBuf,
}

#[derive(Debug, Args)]
pub struct LockArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH", default_value = "x07.json")]
    pub project: PathBuf,

    /// Sparse index URL (example: `sparse+https://registry.x07.io/index/`).
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: Option<String>,

    /// Fail if `x07.lock.json` is out of date.
    #[arg(long)]
    pub check: bool,

    /// Lockfile schema version to write/check (example: `0.4`).
    ///
    /// Use `0.3` only when you must interoperate with an older toolchain that cannot read
    /// `x07.lock@0.4.0`.
    #[arg(long, value_name = "VER", default_value = "0.4")]
    pub lock_version: String,

    /// Disallow network access and reuse existing `.x07/deps` contents.
    #[arg(long)]
    pub offline: bool,

    /// When using `--check`, allow yanked dependencies.
    #[arg(long)]
    pub allow_yanked: bool,

    /// When using `--check`, allow dependencies with active advisories.
    #[arg(long)]
    pub allow_advisories: bool,
}

#[derive(Debug, Args)]
pub struct RepairArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH", default_value = "x07.json")]
    pub project: PathBuf,

    /// Sparse index URL (example: `sparse+https://registry.x07.io/index/`).
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: Option<String>,

    /// Toolchain target to repair against (currently only `current`).
    #[arg(long, value_name = "TOOLCHAIN", default_value = "current")]
    pub toolchain: String,

    /// Disallow network access and prefer already-cached compatible versions.
    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestedLockVersion {
    V0_3_0,
    V0_4_0,
}

impl RequestedLockVersion {
    fn schema_version(self) -> &'static str {
        match self {
            RequestedLockVersion::V0_3_0 => x07_contracts::PROJECT_LOCKFILE_SCHEMA_VERSION_V0_3_0,
            RequestedLockVersion::V0_4_0 => x07_contracts::PROJECT_LOCKFILE_SCHEMA_VERSION_V0_4_0,
        }
    }
}

fn parse_lock_version(raw: &str) -> Result<RequestedLockVersion> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("lock version must be non-empty");
    }
    match raw {
        "0.3" | "0.3.0" => Ok(RequestedLockVersion::V0_3_0),
        "0.4" | "0.4.0" => Ok(RequestedLockVersion::V0_4_0),
        other => anyhow::bail!("unsupported lock version {:?} (expected 0.3 or 0.4)", other),
    }
}

#[derive(Debug, Args)]
pub struct AttestClosureArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH", default_value = "x07.json")]
    pub project: PathBuf,

    /// Output path for the dependency-closure attestation.
    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,

    /// Sparse index URL (example: `sparse+https://registry.x07.io/index/`).
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: Option<String>,

    /// Disallow network access and reuse existing lockfile metadata.
    #[arg(long)]
    pub offline: bool,

    /// Allow yanked dependencies while still recording them in the attestation.
    #[arg(long)]
    pub allow_yanked: bool,

    /// Allow active advisories while still recording them in the attestation.
    #[arg(long)]
    pub allow_advisories: bool,
}

#[derive(Debug, Args)]
pub struct ProvidesArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH", default_value = "x07.json")]
    pub project: PathBuf,

    /// Module id to resolve.
    #[arg(value_name = "MODULE_ID")]
    pub module_id: String,
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Index base URL.
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: String,

    /// Token value.
    #[arg(long, value_name = "TOKEN", conflicts_with = "token_stdin")]
    pub token: Option<String>,

    /// Read token from stdin.
    #[arg(long, conflicts_with = "token")]
    pub token_stdin: bool,
}

#[derive(Debug, Args)]
pub struct PublishArgs {
    /// Package directory containing `x07-package.json`.
    #[arg(long, value_name = "DIR")]
    pub package: PathBuf,

    /// Index base URL.
    #[arg(long, value_name = "URL", alias = "registry")]
    pub index: String,
}

#[derive(Debug, Serialize)]
struct PkgError {
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct PkgReport<T> {
    ok: bool,
    command: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<PkgError>,
}

#[derive(Debug, Serialize)]
struct PackResult {
    package_dir: String,
    out: String,
    sha256: String,
    bytes: usize,
}

#[derive(Debug, Serialize)]
struct LockResult {
    project: String,
    index: Option<String>,
    lockfile: String,
    fetched: Vec<FetchedDep>,
}

#[derive(Debug, Serialize)]
struct RepairResult {
    project: String,
    index: Option<String>,
    lockfile: String,
    toolchain: String,
    repaired: Vec<RepairedDep>,
    fetched: Vec<FetchedDep>,
}

#[derive(Debug, Serialize)]
struct RepairedDep {
    name: String,
    from_version: String,
    to_version: String,
}

#[derive(Debug, Serialize)]
struct DepClosureAttestation {
    schema_version: &'static str,
    project_path: String,
    manifest_digest: String,
    lockfile_digest: String,
    package_set_digest: String,
    dependencies: Vec<DepClosureDependency>,
    advisory_check: DepClosureAdvisoryCheck,
}

#[derive(Debug, Serialize)]
struct DepClosureDependency {
    name: String,
    version: String,
    path: String,
    package_manifest_digest: String,
    module_root: String,
    module_root_digest: String,
    modules: Vec<DepClosureModuleDigest>,
    yanked: bool,
    advisories: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DepClosureModuleDigest {
    module_id: String,
    digest: String,
}

#[derive(Debug, Serialize)]
struct DepClosureAdvisoryCheck {
    mode: &'static str,
    ok: bool,
    allow_yanked: bool,
    allow_advisories: bool,
    yanked: Vec<String>,
    advisories: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LoginResult {
    index: String,
    credentials_path: String,
}

#[derive(Debug, Serialize)]
struct AddResult {
    project: String,
    name: String,
    version: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lock: Option<LockResult>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    transitive_added: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PublishResult {
    index: String,
    package_dir: String,
    name: String,
    version: String,
    cksum: String,
    index_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct FetchedDep {
    name: String,
    version: String,
    path: String,
    sha256: String,
}

pub fn cmd_pkg(
    machine: &crate::reporting::MachineArgs,
    args: PkgArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing pkg subcommand (try --help)");
    };

    match cmd {
        PkgCommand::Add(args) => cmd_pkg_add(args),
        PkgCommand::Remove(args) => cmd_pkg_remove(args),
        PkgCommand::Versions(args) => cmd_pkg_versions(args),
        PkgCommand::Info(args) => cmd_pkg_info(args),
        PkgCommand::List(args) => cmd_pkg_list(args),
        PkgCommand::Pack(args) => cmd_pkg_pack(machine, args),
        PkgCommand::Lock(args) => cmd_pkg_lock(args),
        PkgCommand::Repair(args) => cmd_pkg_repair(args),
        PkgCommand::AttestClosure(args) => cmd_pkg_attest_closure(args),
        PkgCommand::Provides(args) => cmd_pkg_provides(args),
        PkgCommand::Login(args) => cmd_pkg_login(args),
        PkgCommand::Publish(args) => cmd_pkg_publish(args),
    }
}

#[derive(Debug, Serialize)]
struct ProvidesProvider {
    kind: &'static str,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    module_root: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProvidesReport {
    schema_version: &'static str,
    ok: bool,
    module_id: String,
    providers: Vec<ProvidesProvider>,
}

fn cmd_pkg_provides(args: ProvidesArgs) -> Result<std::process::ExitCode> {
    let project_path = util::resolve_existing_path_upwards(&args.project);
    let project_root = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let manifest =
        project::load_project_manifest(&project_path).context("load project manifest")?;

    let module_id = args.module_id.trim().to_string();
    if module_id.is_empty() {
        anyhow::bail!("module id must be non-empty");
    }

    let mut providers: Vec<ProvidesProvider> = Vec::new();

    // Built-in modules (bundled into the compiler for solve-* worlds).
    if builtin_modules::builtin_module_source(&module_id).is_some() {
        providers.push(ProvidesProvider {
            kind: "builtin",
            name: module_id.clone(),
            version: None,
            module_root: None,
        });
    }

    // Local module roots (project-owned code).
    let mut rel = PathBuf::new();
    for seg in module_id.split('.') {
        rel.push(seg);
    }
    rel.set_extension("x07.json");
    for root in &manifest.module_roots {
        let abs_root = project::resolve_rel_path_with_workspace(project_root, root)?;
        if abs_root.join(&rel).is_file() {
            providers.push(ProvidesProvider {
                kind: "local",
                name: module_id.clone(),
                version: None,
                module_root: Some(abs_root.display().to_string()),
            });
        }
    }

    // Toolchain OS stdlib module roots (used by run-os / run-os-sandboxed).
    for root in os_paths::default_os_module_roots_best_effort_from_exe(
        std::env::current_exe().ok().as_deref(),
    ) {
        if root.join(&rel).is_file() {
            providers.push(ProvidesProvider {
                kind: "os-stdlib",
                name: module_id.clone(),
                version: None,
                module_root: Some(root.display().to_string()),
            });
        }
    }

    // Locked dependencies (installed deps under .x07/deps).
    let lock_path = project::default_lockfile_path(&project_path, &manifest);
    if lock_path.is_file() {
        let lock_bytes = std::fs::read(&lock_path)
            .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
        let lock: project::Lockfile = serde_json::from_slice(&lock_bytes)
            .with_context(|| format!("parse lockfile: {}", lock_path.display()))?;
        project::verify_lockfile(&project_path, &manifest, &lock)?;

        for dep in &lock.dependencies {
            if dep.modules_sha256.contains_key(&module_id) {
                let dep_dir = project::resolve_rel_path_with_workspace(project_root, &dep.path)?;
                let root = dep_dir.join(&dep.module_root);
                providers.push(ProvidesProvider {
                    kind: "dependency",
                    name: dep.name.clone(),
                    version: Some(dep.version.clone()),
                    module_root: Some(root.display().to_string()),
                });
            }
        }
    }

    // Offline catalog (bundled into the host runner).
    if let Some((name, version)) = x07_host_runner::best_external_package_for_module(&module_id) {
        providers.push(ProvidesProvider {
            kind: "catalog",
            name,
            version: Some(version),
            module_root: None,
        });
    }

    providers.sort_by(|a, b| {
        (
            a.kind,
            a.name.as_str(),
            a.version.as_deref().unwrap_or(""),
            a.module_root.as_deref().unwrap_or(""),
        )
            .cmp(&(
                b.kind,
                b.name.as_str(),
                b.version.as_deref().unwrap_or(""),
                b.module_root.as_deref().unwrap_or(""),
            ))
    });

    let report = ProvidesReport {
        schema_version: PKG_PROVIDES_REPORT_SCHEMA_VERSION,
        ok: !providers.is_empty(),
        module_id,
        providers,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_pkg_add(args: AddArgs) -> Result<std::process::ExitCode> {
    let (code, report) = pkg_add_report(&args)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(code)
}

fn pkg_add_report(args: &AddArgs) -> Result<(std::process::ExitCode, PkgReport<AddResult>)> {
    let project_path = util::resolve_existing_path_upwards(&args.project);
    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    // Validate using the canonical parser for better error messages and stricter checks.
    project::load_project_manifest(&project_path).context("load project manifest")?;

    let project_bytes = std::fs::read(&project_path)
        .with_context(|| format!("read: {}", project_path.display()))?;
    let original_project_bytes = project_bytes.clone();
    let mut doc: Value = serde_json::from_slice(&project_bytes).with_context(|| {
        format!(
            "[X07PROJECT_PARSE] parse project JSON: {}",
            project_path.display()
        )
    })?;
    let obj = doc
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;

    let parsed_spec = match parse_user_pkg_spec(&args.spec) {
        Ok(parsed) => parsed,
        Err(err) => {
            let report = PkgReport::<AddResult> {
                ok: false,
                command: "pkg.add",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_SPEC_INVALID".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };

    let deps_val = obj
        .entry("dependencies".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let deps = deps_val
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("project.dependencies must be an array"))?;

    let (name, version) = match parsed_spec {
        ParsedUserPkgSpec::Pinned { name, version } => {
            if let Some(dep) = deps
                .iter()
                .find(|dep| dep.get("name").and_then(Value::as_str) == Some(name.as_str()))
            {
                let existing_version = dep.get("version").and_then(Value::as_str).unwrap_or("");
                let existing_path = dep.get("path").and_then(Value::as_str).unwrap_or("");

                if existing_version == version {
                    if let Some(requested_path) = args.path.as_deref() {
                        if requested_path != existing_path {
                            let report = PkgReport::<AddResult> {
                                ok: false,
                                command: "pkg.add",
                                result: None,
                                error: Some(PkgError {
                                    code: "X07PKG_DEP_EXISTS".to_string(),
                                    message: format!(
                                        "dependency already exists: {name}@{version} with path {existing_path:?} (requested path {requested_path:?})"
                                    ),
                                }),
                            };
                            return Ok((std::process::ExitCode::from(20), report));
                        }
                    }

                    let dep_path = if existing_path.is_empty() {
                        format!(".x07/deps/{name}/{version}")
                    } else {
                        existing_path.to_string()
                    };

                    let mut add_result = AddResult {
                        project: project_path.display().to_string(),
                        name,
                        version,
                        path: dep_path,
                        lock: None,
                        transitive_added: Vec::new(),
                    };

                    if args.sync {
                        let lock_args = LockArgs {
                            project: project_path.clone(),
                            index: args.index.clone(),
                            check: false,
                            lock_version: "0.4".to_string(),
                            offline: false,
                            allow_yanked: false,
                            allow_advisories: false,
                        };
                        let (lock_code, lock_report) = pkg_lock_report(&lock_args)?;
                        add_result.lock = lock_report.result;
                        if !lock_report.ok {
                            let report = PkgReport::<AddResult> {
                                ok: false,
                                command: "pkg.add",
                                result: Some(add_result),
                                error: lock_report.error,
                            };
                            return Ok((lock_code, report));
                        }
                    }

                    let report = PkgReport {
                        ok: true,
                        command: "pkg.add",
                        result: Some(add_result),
                        error: None,
                    };
                    return Ok((std::process::ExitCode::SUCCESS, report));
                }

                let report = PkgReport::<AddResult> {
                    ok: false,
                    command: "pkg.add",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_DEP_EXISTS".to_string(),
                        message: format!(
                            "dependency already exists: {name}@{existing_version} (requested {name}@{version}); hint: run `x07 pkg remove {name} --sync` then `x07 pkg add {name}@{version} --sync`"
                        ),
                    }),
                };
                return Ok((std::process::ExitCode::from(20), report));
            }

            (name, version)
        }
        ParsedUserPkgSpec::Unpinned { name } => {
            if let Some(dep) = deps
                .iter()
                .find(|dep| dep.get("name").and_then(Value::as_str) == Some(name.as_str()))
            {
                let existing_version = dep.get("version").and_then(Value::as_str).unwrap_or("");
                let existing_path = dep.get("path").and_then(Value::as_str).unwrap_or("");

                if let Some(requested_path) = args.path.as_deref() {
                    if requested_path != existing_path {
                        let report = PkgReport::<AddResult> {
                            ok: false,
                            command: "pkg.add",
                            result: None,
                            error: Some(PkgError {
                                code: "X07PKG_DEP_EXISTS".to_string(),
                                message: format!(
                                    "dependency already exists: {name}@{existing_version} with path {existing_path:?} (requested path {requested_path:?})"
                                ),
                            }),
                        };
                        return Ok((std::process::ExitCode::from(20), report));
                    }
                }

                let dep_path = if existing_path.is_empty() {
                    format!(".x07/deps/{name}/{existing_version}")
                } else {
                    existing_path.to_string()
                };

                let mut add_result = AddResult {
                    project: project_path.display().to_string(),
                    name,
                    version: existing_version.to_string(),
                    path: dep_path,
                    lock: None,
                    transitive_added: Vec::new(),
                };

                if args.sync {
                    let lock_args = LockArgs {
                        project: project_path.clone(),
                        index: args.index.clone(),
                        check: false,
                        lock_version: "0.4".to_string(),
                        offline: false,
                        allow_yanked: false,
                        allow_advisories: false,
                    };
                    let (lock_code, lock_report) = pkg_lock_report(&lock_args)?;
                    add_result.lock = lock_report.result;
                    if !lock_report.ok {
                        let report = PkgReport::<AddResult> {
                            ok: false,
                            command: "pkg.add",
                            result: Some(add_result),
                            error: lock_report.error,
                        };
                        return Ok((lock_code, report));
                    }
                }

                let report = PkgReport {
                    ok: true,
                    command: "pkg.add",
                    result: Some(add_result),
                    error: None,
                };
                return Ok((std::process::ExitCode::SUCCESS, report));
            }

            let cfg = load_pkg_config(base)?;
            let registry = resolve_registry_url(args.index.as_deref(), &cfg);
            let index = registry.url;
            let token = x07_pkg::load_token(&index).unwrap_or(None);
            let client = match SparseIndexClient::from_index_url(&index, token) {
                Ok(c) => c,
                Err(err) => {
                    let report = PkgReport::<AddResult> {
                        ok: false,
                        command: "pkg.add",
                        result: None,
                        error: Some(PkgError {
                            code: "X07PKG_INDEX_CONFIG".to_string(),
                            message: format!("{err:#}"),
                        }),
                    };
                    return Ok((std::process::ExitCode::from(20), report));
                }
            };

            let entries = match client.fetch_entries(&name) {
                Ok(entries) => entries,
                Err(err) => {
                    let report = PkgReport::<AddResult> {
                        ok: false,
                        command: "pkg.add",
                        result: None,
                        error: Some(PkgError {
                            code: "X07PKG_INDEX_FETCH".to_string(),
                            message: format!(
                                "fetch index entries for {:?}: {err:#} (hint: check the package name and index URL)",
                                name
                            ),
                        }),
                    };
                    return Ok((std::process::ExitCode::from(20), report));
                }
            };

            let Some(version) = latest_non_yanked_semver_version(&entries) else {
                let report = PkgReport::<AddResult> {
                    ok: false,
                    command: "pkg.add",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_INDEX_NO_MATCH".to_string(),
                        message: format!("no non-yanked semver versions found for {:?}", name),
                    }),
                };
                return Ok((std::process::ExitCode::from(20), report));
            };

            (name, version)
        }
    };

    let dep_path = args
        .path
        .clone()
        .unwrap_or_else(|| format!(".x07/deps/{name}/{version}"));

    deps.push(Value::Object(
        [
            ("name".to_string(), Value::String(name.clone())),
            ("version".to_string(), Value::String(version.clone())),
            ("path".to_string(), Value::String(dep_path.clone())),
        ]
        .into_iter()
        .collect(),
    ));

    sort_project_deps(deps);
    write_canonical_json_file(&project_path, &doc)
        .with_context(|| format!("write: {}", project_path.display()))?;

    let mut add_result = AddResult {
        project: project_path.display().to_string(),
        name,
        version,
        path: dep_path,
        lock: None,
        transitive_added: Vec::new(),
    };

    if args.with_closure && !args.sync {
        let manifest =
            project::load_project_manifest(&project_path).context("load project manifest")?;
        let base = project_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let lock_args = LockArgs {
            project: project_path.clone(),
            index: args.index.clone(),
            check: false,
            lock_version: "0.4".to_string(),
            offline: false,
            allow_yanked: false,
            allow_advisories: false,
        };
        let cfg = load_pkg_config(base)?;
        let registry = resolve_registry_url(lock_args.index.as_deref(), &cfg);
        let index = registry.url;
        let mut fetched: Vec<FetchedDep> = Vec::new();
        let mut index_used: Option<String> = None;
        let mut client: Option<SparseIndexClient> = None;
        let patch_updates =
            match apply_patch_overrides_to_project_doc_deps(&mut doc, &manifest.patch) {
                Ok(v) => v,
                Err(err) => {
                    if let Err(rollback_err) =
                        std::fs::write(&project_path, &original_project_bytes)
                    {
                        return Err(anyhow::anyhow!(
                            "{err}\nrollback failed ({}): {rollback_err}",
                            project_path.display()
                        ));
                    }
                    return Err(err);
                }
            };
        let mut ctx = TransitiveResolveCtx {
            base,
            args: &lock_args,
            index: index.as_str(),
            patch: &manifest.patch,
            client: &mut client,
            fetched: &mut fetched,
            index_used: &mut index_used,
        };
        let closure_outcome = match resolve_transitive_deps(&mut doc, &project_path, &mut ctx) {
            Ok(outcome) => outcome,
            Err(err) => {
                if let Err(rollback_err) = std::fs::write(&project_path, &original_project_bytes) {
                    return Err(anyhow::anyhow!(
                        "{err}\nrollback failed ({}): {rollback_err}",
                        project_path.display()
                    ));
                }
                return Err(err);
            }
        };
        match closure_outcome {
            TransitiveResolutionOutcome::Ok(resolution) => {
                if resolution.changed || !patch_updates.is_empty() {
                    sort_project_deps(
                        doc.as_object_mut()
                            .and_then(|o| o.get_mut("dependencies"))
                            .and_then(Value::as_array_mut)
                            .expect("dependencies array"),
                    );
                    write_canonical_json_file(&project_path, &doc)
                        .with_context(|| format!("write: {}", project_path.display()))?;
                }
                add_result.transitive_added = resolution.added_specs;
            }
            TransitiveResolutionOutcome::Error(err) => {
                std::fs::write(&project_path, &original_project_bytes)
                    .with_context(|| format!("rollback write: {}", project_path.display()))?;
                let report = PkgReport::<AddResult> {
                    ok: false,
                    command: "pkg.add",
                    result: Some(add_result),
                    error: Some(err),
                };
                return Ok((std::process::ExitCode::from(20), report));
            }
        }
    }

    if args.sync {
        let lock_args = LockArgs {
            project: project_path.clone(),
            index: args.index.clone(),
            check: false,
            lock_version: "0.4".to_string(),
            offline: false,
            allow_yanked: false,
            allow_advisories: false,
        };
        let (lock_code, lock_report) = match pkg_lock_report(&lock_args) {
            Ok(out) => out,
            Err(err) => {
                if let Err(rollback_err) = std::fs::write(&project_path, &original_project_bytes) {
                    return Err(anyhow::anyhow!(
                        "{err}\nrollback failed ({}): {rollback_err}",
                        project_path.display()
                    ));
                }
                return Err(err);
            }
        };
        add_result.lock = lock_report.result;
        if !lock_report.ok {
            std::fs::write(&project_path, &original_project_bytes)
                .with_context(|| format!("rollback write: {}", project_path.display()))?;
            let report = PkgReport::<AddResult> {
                ok: false,
                command: "pkg.add",
                result: Some(add_result),
                error: lock_report.error,
            };
            return Ok((lock_code, report));
        }
    }

    let report = PkgReport {
        ok: true,
        command: "pkg.add",
        result: Some(add_result),
        error: None,
    };
    Ok((std::process::ExitCode::SUCCESS, report))
}

#[derive(Debug, Serialize)]
struct RemoveResult {
    project: String,
    name: String,
    removed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    lock: Option<LockResult>,
}

fn cmd_pkg_remove(args: RemoveArgs) -> Result<std::process::ExitCode> {
    let (code, report) = pkg_remove_report(&args)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(code)
}

fn pkg_remove_report(
    args: &RemoveArgs,
) -> Result<(std::process::ExitCode, PkgReport<RemoveResult>)> {
    let project_path = util::resolve_existing_path_upwards(&args.project);

    // Validate using the canonical parser for better error messages and stricter checks.
    project::load_project_manifest(&project_path).context("load project manifest")?;

    let project_bytes = std::fs::read(&project_path)
        .with_context(|| format!("read: {}", project_path.display()))?;
    let original_project_bytes = project_bytes.clone();
    let mut doc: Value = serde_json::from_slice(&project_bytes).with_context(|| {
        format!(
            "[X07PROJECT_PARSE] parse project JSON: {}",
            project_path.display()
        )
    })?;
    let obj = doc
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;

    let name = match parse_pkg_name(&args.name) {
        Ok(name) => name,
        Err(err) => {
            let report = PkgReport::<RemoveResult> {
                ok: false,
                command: "pkg.remove",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_SPEC_INVALID".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };

    let Some(deps_val) = obj.get_mut("dependencies") else {
        let report = PkgReport::<RemoveResult> {
            ok: false,
            command: "pkg.remove",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_DEP_NOT_FOUND".to_string(),
                message: format!("dependency not found: {name}"),
            }),
        };
        return Ok((std::process::ExitCode::from(20), report));
    };
    let deps = deps_val
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("project.dependencies must be an array"))?;

    let before_len = deps.len();
    deps.retain(|dep| dep.get("name").and_then(Value::as_str) != Some(name.as_str()));
    let removed = before_len.saturating_sub(deps.len());
    if removed == 0 {
        let report = PkgReport::<RemoveResult> {
            ok: false,
            command: "pkg.remove",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_DEP_NOT_FOUND".to_string(),
                message: format!("dependency not found: {name}"),
            }),
        };
        return Ok((std::process::ExitCode::from(20), report));
    }

    sort_project_deps(deps);
    write_canonical_json_file(&project_path, &doc)
        .with_context(|| format!("write: {}", project_path.display()))?;

    let mut result = RemoveResult {
        project: project_path.display().to_string(),
        name,
        removed,
        lock: None,
    };

    if args.sync {
        let lock_args = LockArgs {
            project: project_path.clone(),
            index: args.index.clone(),
            check: false,
            lock_version: "0.4".to_string(),
            offline: false,
            allow_yanked: false,
            allow_advisories: false,
        };
        let (lock_code, lock_report) = match pkg_lock_report(&lock_args) {
            Ok(out) => out,
            Err(err) => {
                if let Err(rollback_err) = std::fs::write(&project_path, &original_project_bytes) {
                    return Err(anyhow::anyhow!(
                        "{err}\nrollback failed ({}): {rollback_err}",
                        project_path.display()
                    ));
                }
                return Err(err);
            }
        };
        result.lock = lock_report.result;
        if !lock_report.ok {
            std::fs::write(&project_path, &original_project_bytes)
                .with_context(|| format!("rollback write: {}", project_path.display()))?;
            let report = PkgReport::<RemoveResult> {
                ok: false,
                command: "pkg.remove",
                result: Some(result),
                error: lock_report.error,
            };
            return Ok((lock_code, report));
        }
    }

    let report = PkgReport {
        ok: true,
        command: "pkg.remove",
        result: Some(result),
        error: None,
    };
    Ok((std::process::ExitCode::SUCCESS, report))
}

#[derive(Debug, Serialize)]
struct VersionsAdvisory {
    id: String,
    kind: String,
    severity: String,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Debug, Serialize)]
struct VersionsRow {
    version: String,
    yanked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    advisories_count: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    advisories: Vec<VersionsAdvisory>,
}

#[derive(Debug, Serialize)]
struct VersionsResult {
    index: String,
    name: String,
    versions: Vec<VersionsRow>,
}

fn cmd_pkg_versions(args: VersionsArgs) -> Result<std::process::ExitCode> {
    let (code, report) = pkg_versions_report(&args)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(code)
}

fn pkg_versions_report(
    args: &VersionsArgs,
) -> Result<(std::process::ExitCode, PkgReport<VersionsResult>)> {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (registry, offline) =
        resolve_pkg_registry_and_offline(&base, args.index.as_deref(), args.offline)?;
    let index = registry.url;
    let name = match parse_pkg_name(&args.name) {
        Ok(name) => name,
        Err(err) => {
            let report = PkgReport::<VersionsResult> {
                ok: false,
                command: "pkg.versions",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_SPEC_INVALID".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };

    if offline {
        if args.refresh {
            let report = PkgReport::<VersionsResult> {
                ok: false,
                command: "pkg.versions",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_OFFLINE_REFRESH".to_string(),
                    message: "--refresh is not supported in offline mode".to_string(),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
        if !index_url_is_file(&index) {
            let report = PkgReport::<VersionsResult> {
                ok: false,
                command: "pkg.versions",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_OFFLINE_INDEX".to_string(),
                    message: format!(
                        "offline mode requires a file:// registry index (got {index:?})"
                    ),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    }

    let token = x07_pkg::load_token(&index).unwrap_or(None);
    let client = match SparseIndexClient::from_index_url(&index, token) {
        Ok(c) => c,
        Err(err) => {
            let report = PkgReport::<VersionsResult> {
                ok: false,
                command: "pkg.versions",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_INDEX_CONFIG".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };

    let entries = match if args.refresh {
        client.fetch_entries_refresh(&name)
    } else {
        client.fetch_entries(&name)
    } {
        Ok(entries) => entries,
        Err(err) => {
            let report = PkgReport::<VersionsResult> {
                ok: false,
                command: "pkg.versions",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_INDEX_FETCH".to_string(),
                    message: format!(
                        "fetch index entries for {:?}: {err:#} (hint: check the package name and index URL)",
                        name
                    ),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };

    let mut versions: Vec<(SemverVersion, VersionsRow)> = Vec::new();
    for entry in entries {
        if entry.name != name {
            continue;
        }
        let Some(semver) = parse_semver_version(&entry.version) else {
            continue;
        };
        let mut advisories: Vec<VersionsAdvisory> = entry
            .advisories
            .into_iter()
            .map(|a| VersionsAdvisory {
                id: a.id,
                kind: a.kind,
                severity: a.severity,
                summary: a.summary,
                url: a.url,
            })
            .collect();
        advisories.sort_by(|a, b| a.id.cmp(&b.id));
        let advisories_count = (!advisories.is_empty()).then_some(advisories.len() as u64);

        versions.push((
            semver,
            VersionsRow {
                version: entry.version,
                yanked: entry.yanked,
                advisories_count,
                advisories,
            },
        ));
    }
    versions.sort_by(|(a, _), (b, _)| a.cmp(b));

    let versions: Vec<VersionsRow> = versions.into_iter().map(|(_, row)| row).collect();

    if versions.is_empty() {
        let report = PkgReport::<VersionsResult> {
            ok: false,
            command: "pkg.versions",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_INDEX_NO_MATCH".to_string(),
                message: format!("no semver versions found for {:?}", name),
            }),
        };
        return Ok((std::process::ExitCode::from(20), report));
    }

    let report = PkgReport {
        ok: true,
        command: "pkg.versions",
        result: Some(VersionsResult {
            index: index.to_string(),
            name,
            versions,
        }),
        error: None,
    };
    Ok((std::process::ExitCode::SUCCESS, report))
}

#[derive(Debug, Serialize)]
struct ListResult {
    index: String,
    packages: Vec<String>,
}

fn cmd_pkg_list(args: ListArgs) -> Result<std::process::ExitCode> {
    let (code, report) = pkg_list_report(&args)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(code)
}

fn pkg_list_report(args: &ListArgs) -> Result<(std::process::ExitCode, PkgReport<ListResult>)> {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (registry, offline) =
        resolve_pkg_registry_and_offline(&base, args.index.as_deref(), args.offline)?;
    let index = registry.url;

    if offline && !index_url_is_file(&index) {
        let report = PkgReport::<ListResult> {
            ok: false,
            command: "pkg.list",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_OFFLINE_INDEX".to_string(),
                message: format!("offline mode requires a file:// registry index (got {index:?})"),
            }),
        };
        return Ok((std::process::ExitCode::from(20), report));
    }
    if !index_url_is_file(&index) {
        let report = PkgReport::<ListResult> {
            ok: false,
            command: "pkg.list",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_LIST_UNSUPPORTED".to_string(),
                message: format!("pkg list requires a file:// sparse index mirror (got {index:?})"),
            }),
        };
        return Ok((std::process::ExitCode::from(20), report));
    }

    let dir = file_index_url_to_dir(&index)?;
    if !dir.is_dir() {
        let report = PkgReport::<ListResult> {
            ok: false,
            command: "pkg.list",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_LIST_INDEX_MISSING".to_string(),
                message: format!("index dir is missing: {}", dir.display()),
            }),
        };
        return Ok((std::process::ExitCode::from(20), report));
    }

    let packages = collect_packages_from_index_dir(&dir)?;
    let report = PkgReport {
        ok: true,
        command: "pkg.list",
        result: Some(ListResult { index, packages }),
        error: None,
    };
    Ok((std::process::ExitCode::SUCCESS, report))
}

fn collect_packages_from_index_dir(index_dir: &Path) -> Result<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    let mut pending: Vec<PathBuf> = vec![index_dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in
            std::fs::read_dir(&dir).with_context(|| format!("read dir: {}", dir.display()))?
        {
            let entry = entry.with_context(|| format!("read dir entry: {}", dir.display()))?;
            let file_type = entry
                .file_type()
                .with_context(|| format!("read file type: {}", entry.path().display()))?;
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "config.json" {
                continue;
            }
            if parse_pkg_name(&name).is_ok() {
                names.push(name);
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

#[derive(Debug, Serialize)]
struct InfoResult {
    index: String,
    name: String,
    version: String,
    sha256: String,
    yanked: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    advisories: Vec<VersionsAdvisory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_manifest: Option<Value>,
}

fn cmd_pkg_info(args: InfoArgs) -> Result<std::process::ExitCode> {
    let (code, report) = pkg_info_report(&args)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(code)
}

fn pkg_info_report(args: &InfoArgs) -> Result<(std::process::ExitCode, PkgReport<InfoResult>)> {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (registry, offline) =
        resolve_pkg_registry_and_offline(&base, args.index.as_deref(), args.offline)?;
    let index = registry.url;

    if offline && !index_url_is_file(&index) {
        let report = PkgReport::<InfoResult> {
            ok: false,
            command: "pkg.info",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_OFFLINE_INDEX".to_string(),
                message: format!("offline mode requires a file:// registry index (got {index:?})"),
            }),
        };
        return Ok((std::process::ExitCode::from(20), report));
    }

    let parsed_spec = match parse_user_pkg_spec(&args.spec) {
        Ok(parsed) => parsed,
        Err(err) => {
            let report = PkgReport::<InfoResult> {
                ok: false,
                command: "pkg.info",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_SPEC_INVALID".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };

    let (name, pinned_version) = match parsed_spec {
        ParsedUserPkgSpec::Pinned { name, version } => (name, Some(version)),
        ParsedUserPkgSpec::Unpinned { name } => (name, None),
    };

    let token = x07_pkg::load_token(&index).unwrap_or(None);
    let client = match SparseIndexClient::from_index_url(&index, token) {
        Ok(c) => c,
        Err(err) => {
            let report = PkgReport::<InfoResult> {
                ok: false,
                command: "pkg.info",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_INDEX_CONFIG".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };
    let entries = match client.fetch_entries(&name) {
        Ok(entries) => entries,
        Err(err) => {
            let report = PkgReport::<InfoResult> {
                ok: false,
                command: "pkg.info",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_INDEX_FETCH".to_string(),
                    message: format!(
                        "fetch index entries for {:?}: {err:#} (hint: check the package name and index URL)",
                        name
                    ),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };

    let version = match pinned_version {
        Some(v) => v,
        None => match latest_non_yanked_semver_version(&entries) {
            Some(v) => v,
            None => {
                let report = PkgReport::<InfoResult> {
                    ok: false,
                    command: "pkg.info",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_INDEX_NO_MATCH".to_string(),
                        message: format!("no non-yanked semver versions found for {:?}", name),
                    }),
                };
                return Ok((std::process::ExitCode::from(20), report));
            }
        },
    };

    let entry = match entries
        .iter()
        .find(|e| e.name == name && e.version == version)
    {
        Some(e) => e,
        None => {
            let report = PkgReport::<InfoResult> {
                ok: false,
                command: "pkg.info",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_INDEX_NO_MATCH".to_string(),
                    message: format!("no index entry for {:?}@{:?}", name, version),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };

    let mut advisories: Vec<VersionsAdvisory> = entry
        .advisories
        .iter()
        .map(|a| VersionsAdvisory {
            id: a.id.clone(),
            kind: a.kind.clone(),
            severity: a.severity.clone(),
            summary: a.summary.clone(),
            url: a.url.clone(),
        })
        .collect();
    advisories.sort_by(|a, b| a.id.cmp(&b.id));

    let (package_dir, package_manifest) = {
        let dep_dir = base.join(".x07").join("deps").join(&name).join(&version);
        let manifest_path = dep_dir.join("x07-package.json");
        if manifest_path.is_file() {
            let (_pkg, _path, bytes) = project::load_package_manifest(&dep_dir)
                .with_context(|| format!("load package manifest in {}", dep_dir.display()))?;
            let doc: Value = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse {}", manifest_path.display()))?;
            (Some(format!(".x07/deps/{name}/{version}")), Some(doc))
        } else if offline {
            let report = PkgReport::<InfoResult> {
                ok: false,
                command: "pkg.info",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_OFFLINE_MISSING_DEP".to_string(),
                    message: format!(
                        "package is not installed locally: {name}@{version} (expected {})",
                        manifest_path.display()
                    ),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        } else {
            (None, None)
        }
    };

    let report = PkgReport {
        ok: true,
        command: "pkg.info",
        result: Some(InfoResult {
            index,
            name,
            version,
            sha256: entry.cksum.clone(),
            yanked: entry.yanked,
            advisories,
            package_dir,
            package_manifest,
        }),
        error: None,
    };
    Ok((std::process::ExitCode::SUCCESS, report))
}

pub(crate) fn pkg_add_sync_quiet(
    project: PathBuf,
    spec: String,
    index: Option<String>,
) -> Result<()> {
    let args = AddArgs {
        project,
        sync: true,
        index,
        path: None,
        with_closure: false,
        spec,
    };

    let (_code, report) = pkg_add_report(&args)?;
    if report.ok {
        return Ok(());
    }

    let err_code = report.error.as_ref().map(|e| e.code.as_str()).unwrap_or("");
    if err_code == "X07PKG_DEP_EXISTS" {
        return Ok(());
    }

    let msg = report
        .error
        .as_ref()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "pkg.add failed".to_string());
    anyhow::bail!("{msg}");
}

pub(crate) fn ensure_project_deps_hydrated_quiet(project: PathBuf) -> Result<bool> {
    let check_args = LockArgs {
        project: project.clone(),
        index: None,
        check: true,
        lock_version: "0.4".to_string(),
        offline: true,
        allow_yanked: false,
        allow_advisories: false,
    };
    let (_code, check_report) = match pkg_lock_report(&check_args) {
        Ok(result) => result,
        Err(_) => return Ok(false),
    };
    if check_report.ok {
        return Ok(false);
    }
    let should_sync = check_report
        .error
        .as_ref()
        .map(|e| e.code.as_str())
        .is_some_and(|code| {
            matches!(
                code,
                "X07PKG_OFFLINE_MISSING_DEP" | "X07PKG_LOCK_MISSING" | "X07PKG_TRANSITIVE_MISSING"
            )
        });
    if !should_sync {
        return Ok(false);
    }

    let sync_args = LockArgs {
        project,
        index: None,
        check: false,
        lock_version: "0.4".to_string(),
        offline: false,
        allow_yanked: false,
        allow_advisories: false,
    };
    let (_code, sync_report) = pkg_lock_report(&sync_args)?;
    if sync_report.ok {
        return Ok(true);
    }

    let msg = sync_report
        .error
        .as_ref()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "pkg.lock failed".to_string());
    anyhow::bail!("{msg}");
}

fn parse_pkg_spec(spec: &str) -> Result<(String, String)> {
    let spec = spec.trim();
    let Some((name, version)) = spec.split_once('@') else {
        anyhow::bail!("expected NAME@VERSION, got {:?}", spec);
    };
    let name = parse_pkg_name(name)?;
    let version = version.trim();
    if version.is_empty() {
        anyhow::bail!("package version must be non-empty");
    }
    if !is_valid_semver_version(version) {
        anyhow::bail!(
            "package version must be semver (MAJOR.MINOR.PATCH), got {:?}",
            version
        );
    }
    Ok((name, version.to_string()))
}

fn parse_pkg_name(raw: &str) -> Result<String> {
    let name = raw.trim();
    if name.is_empty() {
        anyhow::bail!("package name must be non-empty");
    }
    if !name
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_lowercase())
        || name
            .as_bytes()
            .iter()
            .any(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit() && !matches!(b, b'_' | b'-'))
    {
        anyhow::bail!("package name must match ^[a-z][a-z0-9_-]*$: got {:?}", name);
    }
    Ok(name.to_string())
}

#[derive(Debug, Clone)]
enum ParsedUserPkgSpec {
    Pinned { name: String, version: String },
    Unpinned { name: String },
}

fn parse_user_pkg_spec(spec: &str) -> Result<ParsedUserPkgSpec> {
    let spec = spec.trim();
    if spec.is_empty() {
        anyhow::bail!("package spec must be non-empty");
    }
    if let Some((name, version)) = spec.split_once('@') {
        let name = parse_pkg_name(name)?;
        let version = version.trim();
        if version.is_empty() {
            anyhow::bail!("package version must be non-empty");
        }
        if !is_valid_semver_version(version) {
            anyhow::bail!(
                "package version must be semver (MAJOR.MINOR.PATCH), got {:?}",
                version
            );
        }
        return Ok(ParsedUserPkgSpec::Pinned {
            name,
            version: version.to_string(),
        });
    }

    Ok(ParsedUserPkgSpec::Unpinned {
        name: parse_pkg_name(spec)?,
    })
}

fn is_valid_semver_version(version: &str) -> bool {
    let (core_and_pre, build) = match version.split_once('+') {
        Some((a, b)) => (a, Some(b)),
        None => (version, None),
    };

    if let Some(build) = build {
        if build.is_empty() {
            return false;
        }
        if !build.split('.').all(is_valid_semver_build_identifier) {
            return false;
        }
    }

    let (core, pre) = match core_and_pre.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (core_and_pre, None),
    };

    let mut parts = core.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    if !is_valid_semver_numeric_identifier(major) {
        return false;
    }
    if !is_valid_semver_numeric_identifier(minor) {
        return false;
    }
    if !is_valid_semver_numeric_identifier(patch) {
        return false;
    }

    if let Some(pre) = pre {
        if pre.is_empty() {
            return false;
        }
        if !pre.split('.').all(is_valid_semver_prerelease_identifier) {
            return false;
        }
    }

    true
}

fn is_valid_semver_numeric_identifier(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    if !id.as_bytes().iter().all(|b| b.is_ascii_digit()) {
        return false;
    }
    id == "0" || !id.starts_with('0')
}

fn is_valid_semver_prerelease_identifier(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }

    if id.as_bytes().iter().all(|b| b.is_ascii_digit()) {
        return id == "0" || !id.starts_with('0');
    }

    id.as_bytes()
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
}

fn is_valid_semver_build_identifier(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }

    id.as_bytes()
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum SemverPreId {
    Numeric(u64),
    AlphaNum(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SemverVersion {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<SemverPreId>,
}

impl Ord for SemverVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => {
                    let n = std::cmp::min(self.pre.len(), other.pre.len());
                    for i in 0..n {
                        let a = &self.pre[i];
                        let b = &other.pre[i];
                        let c = match (a, b) {
                            (SemverPreId::Numeric(a), SemverPreId::Numeric(b)) => a.cmp(b),
                            (SemverPreId::Numeric(_), SemverPreId::AlphaNum(_)) => {
                                std::cmp::Ordering::Less
                            }
                            (SemverPreId::AlphaNum(_), SemverPreId::Numeric(_)) => {
                                std::cmp::Ordering::Greater
                            }
                            (SemverPreId::AlphaNum(a), SemverPreId::AlphaNum(b)) => a.cmp(b),
                        };
                        if c != std::cmp::Ordering::Equal {
                            return c;
                        }
                    }
                    self.pre.len().cmp(&other.pre.len())
                }
            })
    }
}

impl PartialOrd for SemverVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn parse_semver_version(version: &str) -> Option<SemverVersion> {
    let (core_and_pre, _build) = version.split_once('+').unwrap_or((version, ""));
    let (core, pre) = core_and_pre.split_once('-').unwrap_or((core_and_pre, ""));

    let mut parts = core.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if !is_valid_semver_numeric_identifier(major)
        || !is_valid_semver_numeric_identifier(minor)
        || !is_valid_semver_numeric_identifier(patch)
    {
        return None;
    }
    let major = major.parse::<u64>().ok()?;
    let minor = minor.parse::<u64>().ok()?;
    let patch = patch.parse::<u64>().ok()?;

    let mut pre_ids: Vec<SemverPreId> = Vec::new();
    if !pre.is_empty() {
        for raw in pre.split('.') {
            if raw.is_empty() {
                return None;
            }
            if raw.as_bytes().iter().all(|b| b.is_ascii_digit()) {
                if !is_valid_semver_numeric_identifier(raw) {
                    return None;
                }
                pre_ids.push(SemverPreId::Numeric(raw.parse::<u64>().ok()?));
            } else {
                if !is_valid_semver_prerelease_identifier(raw) {
                    return None;
                }
                pre_ids.push(SemverPreId::AlphaNum(raw.to_string()));
            }
        }
    }

    Some(SemverVersion {
        major,
        minor,
        patch,
        pre: pre_ids,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemverReqOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemverReqClause {
    op: SemverReqOp,
    version: SemverVersion,
}

fn parse_semver_req(raw: &str) -> Result<Vec<SemverReqClause>> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("semver requirement must be non-empty");
    }

    let mut out: Vec<SemverReqClause> = Vec::new();
    for tok in raw.split_whitespace() {
        let tok = tok.trim().trim_end_matches(',');
        if tok.is_empty() {
            continue;
        }

        let (op, version_raw) = if let Some(v) = tok.strip_prefix(">=") {
            (SemverReqOp::Ge, v)
        } else if let Some(v) = tok.strip_prefix("<=") {
            (SemverReqOp::Le, v)
        } else if let Some(v) = tok.strip_prefix('>') {
            (SemverReqOp::Gt, v)
        } else if let Some(v) = tok.strip_prefix('<') {
            (SemverReqOp::Lt, v)
        } else if let Some(v) = tok.strip_prefix('=') {
            (SemverReqOp::Eq, v)
        } else {
            (SemverReqOp::Eq, tok)
        };

        let version_raw = version_raw.trim();
        if version_raw.is_empty() {
            anyhow::bail!("semver requirement token is missing a version: {:?}", tok);
        }
        let Some(version) = parse_semver_version(version_raw) else {
            anyhow::bail!(
                "semver requirement token has invalid version {:?}: {:?}",
                version_raw,
                tok
            );
        };
        out.push(SemverReqClause { op, version });
    }

    if out.is_empty() {
        anyhow::bail!("semver requirement did not contain any clauses: {:?}", raw);
    }

    Ok(out)
}

fn semver_satisfies(version: &SemverVersion, req: &[SemverReqClause]) -> bool {
    req.iter().all(|c| match version.cmp(&c.version) {
        std::cmp::Ordering::Less => matches!(c.op, SemverReqOp::Lt | SemverReqOp::Le),
        std::cmp::Ordering::Equal => {
            matches!(c.op, SemverReqOp::Le | SemverReqOp::Ge | SemverReqOp::Eq)
        }
        std::cmp::Ordering::Greater => matches!(c.op, SemverReqOp::Gt | SemverReqOp::Ge),
    })
}

fn current_x07c_semver() -> Result<SemverVersion> {
    let Some(v) = parse_semver_version(x07c::X07C_VERSION) else {
        anyhow::bail!(
            "internal error: x07c version is not semver: {:?}",
            x07c::X07C_VERSION
        );
    };
    Ok(v)
}

fn check_pkg_x07c_compat(
    pkg_name: &str,
    pkg_version: &str,
    pkg_manifest_path: &Path,
    pkg_manifest_bytes: &[u8],
) -> Result<Option<PkgError>> {
    let doc: Value = match serde_json::from_slice(pkg_manifest_bytes) {
        Ok(v) => v,
        Err(err) => {
            return Ok(Some(PkgError {
                code: "X07PKG_MANIFEST_PARSE".to_string(),
                message: format!(
                    "parse package manifest for {pkg_name}@{pkg_version}: {err} ({})",
                    pkg_manifest_path.display()
                ),
            }));
        }
    };

    let Some(meta) = doc.get("meta").and_then(Value::as_object) else {
        return Ok(None);
    };
    let Some(raw) = meta.get("x07c_compat") else {
        return Ok(None);
    };
    let Some(raw) = raw.as_str() else {
        return Ok(Some(PkgError {
            code: "X07PKG_X07C_COMPAT_INVALID".to_string(),
            message: format!(
                "package meta.x07c_compat must be a string for {pkg_name}@{pkg_version} ({})",
                pkg_manifest_path.display()
            ),
        }));
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }

    let req = match parse_semver_req(raw) {
        Ok(req) => req,
        Err(err) => {
            return Ok(Some(PkgError {
                code: "X07PKG_X07C_COMPAT_INVALID".to_string(),
                message: format!(
                    "invalid package meta.x07c_compat for {pkg_name}@{pkg_version}: {:?} ({err})",
                    raw
                ),
            }));
        }
    };

    let cur = current_x07c_semver()?;
    if semver_satisfies(&cur, &req) {
        return Ok(None);
    }

    Ok(Some(PkgError {
        code: "X07PKG_X07C_INCOMPATIBLE".to_string(),
        message: format!(
            "package {pkg_name}@{pkg_version} is incompatible with x07c {} (meta.x07c_compat = {:?})",
            x07c::X07C_VERSION,
            raw
        ),
    }))
}

fn latest_non_yanked_semver_version(entries: &[x07_pkg::IndexEntry]) -> Option<String> {
    let mut best: Option<(SemverVersion, String)> = None;
    for entry in entries {
        if entry.yanked {
            continue;
        }
        let Some(v) = parse_semver_version(&entry.version) else {
            continue;
        };
        match &best {
            None => best = Some((v, entry.version.clone())),
            Some((cur, _)) if v > *cur => best = Some((v, entry.version.clone())),
            _ => {}
        }
    }
    best.map(|(_, version)| version)
}

fn cmd_pkg_pack(
    machine: &crate::reporting::MachineArgs,
    args: PackArgs,
) -> Result<std::process::ExitCode> {
    let package_dir = util::resolve_existing_path_upwards(&args.package);
    let out_path = machine
        .out
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("pkg pack: missing --out <PATH>"))?
        .clone();

    let (_pkg, tar) = pack_package_to_tar(&package_dir)?;
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output dir: {}", parent.display()))?;
    }
    std::fs::write(&out_path, &tar).with_context(|| format!("write: {}", out_path.display()))?;

    let report = PkgReport {
        ok: true,
        command: "pkg.pack",
        result: Some(PackResult {
            package_dir: package_dir.display().to_string(),
            out: out_path.display().to_string(),
            sha256: x07_pkg::sha256_hex(&tar),
            bytes: tar.len(),
        }),
        error: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_pkg_lock(args: LockArgs) -> Result<std::process::ExitCode> {
    let (code, report) = pkg_lock_report(&args)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(code)
}

fn cmd_pkg_repair(args: RepairArgs) -> Result<std::process::ExitCode> {
    let (code, report) = pkg_repair_report(&args)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(code)
}

fn cmd_pkg_attest_closure(args: AttestClosureArgs) -> Result<std::process::ExitCode> {
    let (attestation, ok) = build_dep_closure_attestation(&args)?;
    let value = serde_json::to_value(&attestation).context("serialize dependency closure")?;
    let schema_diags = report_common::validate_schema(
        X07_DEP_CLOSURE_ATTEST_SCHEMA_BYTES,
        "spec/x07-dep.closure.attest.schema.json",
        &value,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "internal error: dependency closure attestation is not schema-valid: {}",
            schema_diags[0].message
        );
    }
    write_canonical_json_file(&args.out, &value)
        .with_context(|| format!("write: {}", args.out.display()))?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(20)
    })
}

fn build_dep_closure_attestation(
    args: &AttestClosureArgs,
) -> Result<(DepClosureAttestation, bool)> {
    let project_path = util::resolve_existing_path_upwards(&args.project);
    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let project_bytes = std::fs::read(&project_path)
        .with_context(|| format!("read: {}", project_path.display()))?;
    let manifest =
        project::load_project_manifest(&project_path).context("load project manifest")?;
    let lock_path = project::default_lockfile_path(&project_path, &manifest);
    let lock_bytes = std::fs::read(&lock_path).with_context(|| {
        format!(
            "X07DEP_LOCK_CHECK_FAILED: read lockfile: {}",
            lock_path.display()
        )
    })?;
    let existing_lock: project::Lockfile =
        serde_json::from_slice(&lock_bytes).with_context(|| {
            format!(
                "X07DEP_LOCK_CHECK_FAILED: parse lockfile JSON: {}",
                lock_path.display()
            )
        })?;
    project::verify_lockfile(&project_path, &manifest, &existing_lock).with_context(|| {
        format!(
            "X07DEP_LOCK_CHECK_FAILED: verify lockfile against {}",
            project_path.display()
        )
    })?;

    let cfg = load_pkg_config(base)?;
    let registry = resolve_registry_url(args.index.as_deref(), &cfg);
    let index = registry.url;
    let lock_args = LockArgs {
        project: project_path.clone(),
        index: args.index.clone(),
        check: false,
        lock_version: "0.4".to_string(),
        offline: args.offline,
        allow_yanked: args.allow_yanked,
        allow_advisories: args.allow_advisories,
    };
    let mut refreshed_lock = project::compute_lockfile(&project_path, &manifest)?;
    let mut client: Option<SparseIndexClient> = None;
    let mut index_used = None;
    if let Some(err) = apply_lock_overrides_and_metadata(
        &manifest,
        &index,
        &lock_args,
        &mut client,
        &mut index_used,
        Some(&existing_lock),
        &mut refreshed_lock,
    )? {
        anyhow::bail!("X07DEP_LOCK_CHECK_FAILED: {}", err.message);
    }

    let mut dependencies = Vec::with_capacity(refreshed_lock.dependencies.len());
    let mut yanked = Vec::new();
    let mut advisories = Vec::new();

    for dep in &refreshed_lock.dependencies {
        project::resolve_rel_path_with_workspace(base, &dep.path)?;
        let mut modules = dep
            .modules_sha256
            .iter()
            .map(|(module_id, digest)| DepClosureModuleDigest {
                module_id: module_id.clone(),
                digest: prefix_sha256_hex(digest),
            })
            .collect::<Vec<_>>();
        modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));

        let advisory_ids = dep
            .advisories
            .iter()
            .map(|advisory| advisory.id.clone())
            .collect::<Vec<_>>();
        if dep.yanked.unwrap_or(false) {
            yanked.push(format!("{}@{}", dep.name, dep.version));
        }
        if !dep.advisories.is_empty() {
            let rendered = dep
                .advisories
                .iter()
                .map(|advisory| format!("{} ({})", advisory.id, advisory.summary))
                .collect::<Vec<_>>()
                .join(", ");
            advisories.push(format!("{}@{}: {}", dep.name, dep.version, rendered));
        }

        dependencies.push(DepClosureDependency {
            name: dep.name.clone(),
            version: dep.version.clone(),
            path: dep.path.clone(),
            package_manifest_digest: prefix_sha256_hex(&dep.package_manifest_sha256),
            module_root: dep.module_root.clone(),
            module_root_digest: module_root_digest(dep),
            modules,
            yanked: dep.yanked.unwrap_or(false),
            advisories: advisory_ids,
        });
    }

    dependencies.sort_by(|a, b| {
        (
            a.name.as_str(),
            a.version.as_str(),
            a.path.as_str(),
            a.module_root.as_str(),
        )
            .cmp(&(
                b.name.as_str(),
                b.version.as_str(),
                b.path.as_str(),
                b.module_root.as_str(),
            ))
    });
    yanked.sort();
    advisories.sort();

    let package_set_digest = {
        let bytes = serde_json::to_vec(&dependencies).context("serialize package set")?;
        sha256_prefixed(&bytes)
    };
    let advisory_check = DepClosureAdvisoryCheck {
        mode: if args.offline { "offline" } else { "online" },
        ok: (args.allow_yanked || yanked.is_empty())
            && (args.allow_advisories || advisories.is_empty()),
        allow_yanked: args.allow_yanked,
        allow_advisories: args.allow_advisories,
        yanked,
        advisories,
    };
    let attestation = DepClosureAttestation {
        schema_version: X07_DEP_CLOSURE_ATTEST_SCHEMA_VERSION,
        project_path: project_path.display().to_string(),
        manifest_digest: sha256_prefixed(&project_bytes),
        lockfile_digest: sha256_prefixed(&lock_bytes),
        package_set_digest,
        dependencies,
        advisory_check,
    };

    let ok = attestation.advisory_check.ok;
    Ok((attestation, ok))
}

fn prefix_sha256_hex(hex: &str) -> String {
    format!("sha256:{hex}")
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn module_root_digest(dep: &project::LockedDependency) -> String {
    let mut hasher = Sha256::new();
    for (module_id, digest) in &dep.modules_sha256 {
        hasher.update(module_id.as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([b'\n']);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn cached_versions_for_package(
    project_base: &Path,
    name: &str,
) -> Result<Vec<(SemverVersion, String)>> {
    let root = project_base.join(".x07").join("deps").join(name);
    let rd = match std::fs::read_dir(&root) {
        Ok(rd) => rd,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("read {}", root.display())),
    };

    let mut out: Vec<(SemverVersion, String)> = Vec::new();
    for entry in rd {
        let entry = entry.with_context(|| format!("read entry in {}", root.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("file_type for {}", entry.path().display()))?;
        if !file_type.is_dir() {
            continue;
        }
        let raw = entry.file_name().to_string_lossy().to_string();
        let Some(v) = parse_semver_version(&raw) else {
            continue;
        };
        out.push((v, raw));
    }

    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    Ok(out)
}

fn repair_band_matches_strict(cur: &SemverVersion, cand: &SemverVersion) -> bool {
    if cur.major == 0 {
        cand.major == 0 && cand.minor == cur.minor
    } else {
        cand.major == cur.major
    }
}

fn repair_band_matches_fallback(cur: &SemverVersion, cand: &SemverVersion) -> bool {
    if cur.major == 0 {
        cand.major == 0
    } else {
        cand.major == cur.major
    }
}

fn is_repair_ineligible_error(err: &PkgError) -> bool {
    matches!(
        err.code.as_str(),
        "X07PKG_X07C_INCOMPATIBLE" | "X07PKG_X07C_COMPAT_INVALID" | "X07PKG_MANIFEST_PARSE"
    )
}

fn set_patch_version_in_project_doc(doc: &mut Value, name: &str, version: &str) -> Result<()> {
    let obj = doc
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;

    // `project.patch` requires at least `x07.project@0.3.0`, so bump old manifests automatically.
    let schema_version_raw = obj
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if schema_version_raw == x07_contracts::PROJECT_MANIFEST_SCHEMA_VERSION_V0_2_0 {
        obj.insert(
            "schema_version".to_string(),
            Value::String(x07_contracts::PROJECT_MANIFEST_SCHEMA_VERSION.to_string()),
        );
    }

    let patch_val = obj
        .entry("patch".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let patch_obj = patch_val
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("project.patch must be a JSON object"))?;

    let spec_val = patch_obj
        .entry(name.to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let spec_obj = spec_val
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("project.patch[{name:?}] must be a JSON object"))?;

    if let Some(path) = spec_obj.get("path").and_then(Value::as_str) {
        if !path.trim().is_empty() {
            anyhow::bail!(
                "cannot repair {name:?}: project.patch has a local path override ({path:?})"
            );
        }
    }

    spec_obj.insert("version".to_string(), Value::String(version.to_string()));
    Ok(())
}

fn pkg_repair_report(
    args: &RepairArgs,
) -> Result<(std::process::ExitCode, PkgReport<RepairResult>)> {
    let toolchain = args.toolchain.trim();
    if toolchain.is_empty() {
        let report = PkgReport {
            ok: false,
            command: "pkg.repair",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_REPAIR_TOOLCHAIN_INVALID".to_string(),
                message: "toolchain must be non-empty".to_string(),
            }),
        };
        return Ok((std::process::ExitCode::from(20), report));
    }
    if toolchain != "current" {
        let report = PkgReport {
            ok: false,
            command: "pkg.repair",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_REPAIR_TOOLCHAIN_UNSUPPORTED".to_string(),
                message: format!("unsupported toolchain {:?} (expected current)", toolchain),
            }),
        };
        return Ok((std::process::ExitCode::from(20), report));
    }

    let project_path = util::resolve_existing_path_upwards(&args.project);
    let project_bytes = std::fs::read(&project_path)
        .with_context(|| format!("read: {}", project_path.display()))?;
    let original_project_bytes = project_bytes.clone();
    let mut doc: Value = serde_json::from_slice(&project_bytes).with_context(|| {
        format!(
            "[X07PROJECT_PARSE] parse project JSON: {}",
            project_path.display()
        )
    })?;

    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let patch_non_empty = doc
        .get("patch")
        .and_then(Value::as_object)
        .is_some_and(|p| !p.is_empty());
    let schema_version_raw = doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if patch_non_empty
        && schema_version_raw == x07_contracts::PROJECT_MANIFEST_SCHEMA_VERSION_V0_2_0
    {
        let obj = doc
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;
        obj.insert(
            "schema_version".to_string(),
            Value::String(x07_contracts::PROJECT_MANIFEST_SCHEMA_VERSION.to_string()),
        );
    }

    let manifest = {
        let bytes = serde_json::to_vec(&doc)?;
        project::parse_project_manifest_bytes(&bytes, &project_path)
            .context("load project manifest")?
    };
    for (name, spec) in &manifest.patch {
        if !is_valid_semver_version(&spec.version) {
            let report = PkgReport {
                ok: false,
                command: "pkg.repair",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_SPEC_INVALID".to_string(),
                    message: format!(
                        "project.patch[{name:?}].version must be semver (MAJOR.MINOR.PATCH), got {:?}",
                        spec.version
                    ),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    }

    let lock_path = project::default_lockfile_path(&project_path, &manifest);
    let lock_bytes = match std::fs::read(&lock_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let report = PkgReport {
                ok: false,
                command: "pkg.repair",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_REPAIR_LOCK_MISSING".to_string(),
                    message: format!("missing lockfile: {}", lock_path.display()),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
        Err(err) => return Err(err).with_context(|| format!("read {}", lock_path.display())),
    };
    let original_lock_bytes = lock_bytes.clone();
    let existing_lock: project::Lockfile = serde_json::from_slice(&lock_bytes)
        .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;

    let (registry, offline) =
        resolve_pkg_registry_and_offline(base, args.index.as_deref(), args.offline)?;
    let index = registry.url.clone();

    let mut fetched: Vec<FetchedDep> = Vec::new();
    let mut index_used: Option<String> = None;

    let mut repaired: Vec<RepairedDep> = Vec::new();
    let mut repaired_names: HashSet<String> = HashSet::new();

    {
        let lock_args = LockArgs {
            project: project_path.clone(),
            index: args.index.clone(),
            check: false,
            lock_version: "0.4".to_string(),
            offline,
            allow_yanked: false,
            allow_advisories: false,
        };
        let mut client: Option<SparseIndexClient> = None;

        let mut resolve_ctx = TransitiveResolveCtx {
            base,
            args: &lock_args,
            index: index.as_str(),
            patch: &manifest.patch,
            client: &mut client,
            fetched: &mut fetched,
            index_used: &mut index_used,
        };

        let mut locked: Vec<project::DependencySpec> = existing_lock
            .dependencies
            .iter()
            .map(|d| project::DependencySpec {
                name: d.name.clone(),
                version: d.version.clone(),
                path: d.path.clone(),
            })
            .collect();
        locked.sort_by(|a, b| {
            (a.name.as_str(), a.version.as_str(), a.path.as_str()).cmp(&(
                b.name.as_str(),
                b.version.as_str(),
                b.path.as_str(),
            ))
        });

        for dep in locked {
            if repaired_names.contains(&dep.name) {
                continue;
            }

            let err = match ensure_deps_present(std::slice::from_ref(&dep), &mut resolve_ctx)? {
                None => continue,
                Some(err) => err,
            };
            if err.code != "X07PKG_X07C_INCOMPATIBLE" {
                let report = PkgReport {
                    ok: false,
                    command: "pkg.repair",
                    result: None,
                    error: Some(err),
                };
                return Ok((std::process::ExitCode::from(20), report));
            }

            if !project::is_vendored_dep_path(&dep.path) {
                let report = PkgReport {
                    ok: false,
                    command: "pkg.repair",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_REPAIR_LOCAL_INCOMPATIBLE".to_string(),
                        message: format!(
                            "cannot repair incompatible local dependency {}@{} ({})",
                            dep.name, dep.version, dep.path
                        ),
                    }),
                };
                return Ok((std::process::ExitCode::from(20), report));
            }
            if manifest
                .patch
                .get(&dep.name)
                .is_some_and(|s| s.path.is_some())
            {
                let report = PkgReport {
                    ok: false,
                    command: "pkg.repair",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_REPAIR_PATCHED_PATH_INCOMPATIBLE".to_string(),
                        message: format!(
                            "cannot repair incompatible dependency {}@{}: project.patch has a local path override",
                            dep.name, dep.version
                        ),
                    }),
                };
                return Ok((std::process::ExitCode::from(20), report));
            }

            let Some(cur) = parse_semver_version(&dep.version) else {
                let report = PkgReport {
                    ok: false,
                    command: "pkg.repair",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_REPAIR_LOCK_INVALID".to_string(),
                        message: format!(
                            "locked version for {} is not semver: {:?}",
                            dep.name, dep.version
                        ),
                    }),
                };
                return Ok((std::process::ExitCode::from(20), report));
            };

            let mut selected: Option<String> = None;
            if offline {
                let cached = cached_versions_for_package(base, &dep.name)?;
                for (v, ver) in &cached {
                    if !repair_band_matches_strict(&cur, v) {
                        continue;
                    }
                    let dir = base.join(".x07").join("deps").join(&dep.name).join(ver);
                    let (pkg, pkg_manifest_path, pkg_manifest_bytes) =
                        match project::load_package_manifest(&dir) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                    if pkg.name != dep.name || pkg.version != *ver {
                        continue;
                    }
                    if check_pkg_x07c_compat(
                        &pkg.name,
                        &pkg.version,
                        &pkg_manifest_path,
                        &pkg_manifest_bytes,
                    )?
                    .is_none()
                    {
                        selected = Some(ver.clone());
                        break;
                    }
                }
                if selected.is_none() && cur.major == 0 {
                    for (v, ver) in &cached {
                        if !repair_band_matches_fallback(&cur, v) {
                            continue;
                        }
                        let dir = base.join(".x07").join("deps").join(&dep.name).join(ver);
                        let (pkg, pkg_manifest_path, pkg_manifest_bytes) =
                            match project::load_package_manifest(&dir) {
                                Ok(v) => v,
                                Err(_) => continue,
                            };
                        if pkg.name != dep.name || pkg.version != *ver {
                            continue;
                        }
                        if check_pkg_x07c_compat(
                            &pkg.name,
                            &pkg.version,
                            &pkg_manifest_path,
                            &pkg_manifest_bytes,
                        )?
                        .is_none()
                        {
                            selected = Some(ver.clone());
                            break;
                        }
                    }
                }
            } else {
                if let Some(err) =
                    ensure_index_client(&index, resolve_ctx.client, resolve_ctx.index_used)?
                {
                    let report = PkgReport {
                        ok: false,
                        command: "pkg.repair",
                        result: None,
                        error: Some(err),
                    };
                    return Ok((std::process::ExitCode::from(20), report));
                }
                let entries = {
                    let client_ref = resolve_ctx.client.as_ref().expect("client initialized");
                    match client_ref.fetch_entries(&dep.name) {
                        Ok(entries) => entries,
                        Err(err) => {
                            let report = PkgReport {
                                ok: false,
                                command: "pkg.repair",
                                result: None,
                                error: Some(PkgError {
                                    code: "X07PKG_INDEX_FETCH".to_string(),
                                    message: format!(
                                        "fetch index entries for {:?}: {err:#} (hint: check the package name and index URL)",
                                        dep.name
                                    ),
                                }),
                            };
                            return Ok((std::process::ExitCode::from(20), report));
                        }
                    }
                };

                let mut candidates: Vec<(SemverVersion, String)> = Vec::new();
                for entry in &entries {
                    if entry.yanked {
                        continue;
                    }
                    let Some(v) = parse_semver_version(&entry.version) else {
                        continue;
                    };
                    if !repair_band_matches_strict(&cur, &v) {
                        continue;
                    }
                    candidates.push((v, entry.version.clone()));
                }
                candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

                if candidates.is_empty() && cur.major == 0 {
                    for entry in &entries {
                        if entry.yanked {
                            continue;
                        }
                        let Some(v) = parse_semver_version(&entry.version) else {
                            continue;
                        };
                        if !repair_band_matches_fallback(&cur, &v) {
                            continue;
                        }
                        candidates.push((v, entry.version.clone()));
                    }
                    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
                }

                for (_v, ver) in candidates {
                    let spec = project::DependencySpec {
                        name: dep.name.clone(),
                        version: ver.clone(),
                        path: format!(".x07/deps/{}/{ver}", dep.name),
                    };
                    match ensure_deps_present(std::slice::from_ref(&spec), &mut resolve_ctx)? {
                        None => {
                            selected = Some(ver);
                            break;
                        }
                        Some(err) if is_repair_ineligible_error(&err) => continue,
                        Some(err) => {
                            let report = PkgReport {
                                ok: false,
                                command: "pkg.repair",
                                result: None,
                                error: Some(err),
                            };
                            return Ok((std::process::ExitCode::from(20), report));
                        }
                    }
                }
            }

            let Some(to_version) = selected else {
                let report = PkgReport {
                    ok: false,
                    command: "pkg.repair",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_REPAIR_NO_COMPATIBLE_VERSION".to_string(),
                        message: format!(
                            "no compatible versions found for {} (locked {} is incompatible with x07c {})",
                            dep.name,
                            dep.version,
                            x07c::X07C_VERSION
                        ),
                    }),
                };
                return Ok((std::process::ExitCode::from(20), report));
            };

            set_patch_version_in_project_doc(&mut doc, &dep.name, &to_version)?;
            let _ = ensure_dep_entry(
                &mut doc,
                &dep.name,
                &to_version,
                &format!(".x07/deps/{}/{to_version}", dep.name),
                true,
            )?;
            repaired_names.insert(dep.name.clone());
            repaired.push(RepairedDep {
                name: dep.name,
                from_version: dep.version,
                to_version,
            });
        }
    }

    if !repaired.is_empty() {
        let _ = {
            let bytes = serde_json::to_vec(&doc)?;
            project::parse_project_manifest_bytes(&bytes, &project_path)
                .context("validate updated project manifest")?
        };
        write_canonical_json_file(&project_path, &doc)
            .with_context(|| format!("write: {}", project_path.display()))?;
    }

    let lock_args = LockArgs {
        project: project_path.clone(),
        index: args.index.clone(),
        check: false,
        lock_version: "0.4".to_string(),
        offline: args.offline,
        allow_yanked: false,
        allow_advisories: false,
    };
    let (lock_code, lock_report) = pkg_lock_report(&lock_args)?;

    let mut all_fetched = fetched;
    if let Some(lock_res) = lock_report.result.as_ref() {
        all_fetched.extend(lock_res.fetched.iter().cloned());
    }
    all_fetched.sort_by(|a, b| {
        (
            a.name.as_str(),
            a.version.as_str(),
            a.path.as_str(),
            a.sha256.as_str(),
        )
            .cmp(&(
                b.name.as_str(),
                b.version.as_str(),
                b.path.as_str(),
                b.sha256.as_str(),
            ))
    });
    all_fetched.dedup_by(|a, b| {
        a.name == b.name && a.version == b.version && a.path == b.path && a.sha256 == b.sha256
    });

    let result = RepairResult {
        project: project_path.display().to_string(),
        index: index_used.clone().or_else(|| Some(index.clone())),
        lockfile: lock_path.display().to_string(),
        toolchain: toolchain.to_string(),
        repaired,
        fetched: all_fetched,
    };

    if !lock_report.ok {
        if let Err(err) = std::fs::write(&project_path, &original_project_bytes) {
            return Err(err).with_context(|| format!("rollback {}", project_path.display()));
        }
        if let Err(err) = std::fs::write(&lock_path, &original_lock_bytes) {
            return Err(err).with_context(|| format!("rollback {}", lock_path.display()));
        }

        let report = PkgReport {
            ok: false,
            command: "pkg.repair",
            result: Some(result),
            error: lock_report.error,
        };
        return Ok((lock_code, report));
    }

    let report = PkgReport {
        ok: true,
        command: "pkg.repair",
        result: Some(result),
        error: None,
    };
    Ok((std::process::ExitCode::SUCCESS, report))
}

fn pkg_lock_report(args: &LockArgs) -> Result<(std::process::ExitCode, PkgReport<LockResult>)> {
    let project_path = util::resolve_existing_path_upwards(&args.project);
    let project_bytes = std::fs::read(&project_path)
        .with_context(|| format!("read: {}", project_path.display()))?;
    let mut doc: Value = serde_json::from_slice(&project_bytes).with_context(|| {
        format!(
            "[X07PROJECT_PARSE] parse project JSON: {}",
            project_path.display()
        )
    })?;

    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let (registry, offline) =
        resolve_pkg_registry_and_offline(base, args.index.as_deref(), args.offline)?;
    let index = registry.url.clone();

    let mut effective_args = LockArgs {
        project: args.project.clone(),
        index: args.index.clone(),
        check: args.check,
        lock_version: args.lock_version.clone(),
        offline,
        allow_yanked: args.allow_yanked,
        allow_advisories: args.allow_advisories,
    };
    if effective_args.index.is_none() && registry.source == RegistrySource::Config {
        effective_args.index = Some(index.clone());
    }

    let requested_lock_version = match parse_lock_version(&effective_args.lock_version) {
        Ok(v) => v,
        Err(err) => {
            let report = PkgReport {
                ok: false,
                command: "pkg.lock",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_LOCK_VERSION_INVALID".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };

    let patch_non_empty = doc
        .get("patch")
        .and_then(Value::as_object)
        .is_some_and(|p| !p.is_empty());
    let schema_version_raw = doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let mut schema_bumped = false;
    if patch_non_empty
        && schema_version_raw == x07_contracts::PROJECT_MANIFEST_SCHEMA_VERSION_V0_2_0
    {
        if args.check {
            let report = PkgReport {
                ok: false,
                command: "pkg.lock",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_TRANSITIVE_MISSING".to_string(),
                    message: format!(
                        "project.patch requires schema_version {} (run `x07 pkg lock` to update x07.json)",
                        x07_contracts::PROJECT_MANIFEST_SCHEMA_VERSION
                    ),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }

        let obj = doc
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;
        obj.insert(
            "schema_version".to_string(),
            Value::String(x07_contracts::PROJECT_MANIFEST_SCHEMA_VERSION.to_string()),
        );
        schema_bumped = true;
    }

    let mut normalized_specs = normalize_project_doc_deps(&mut doc)?;

    let manifest = {
        let bytes = serde_json::to_vec(&doc)?;
        project::parse_project_manifest_bytes(&bytes, &project_path)
            .context("load project manifest")?
    };
    for (name, spec) in &manifest.patch {
        if !is_valid_semver_version(&spec.version) {
            let report = PkgReport {
                ok: false,
                command: "pkg.lock",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_SPEC_INVALID".to_string(),
                    message: format!(
                        "project.patch[{name:?}].version must be semver (MAJOR.MINOR.PATCH), got {:?}",
                        spec.version
                    ),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    }

    let mut updated_specs = std::mem::take(&mut normalized_specs);
    updated_specs.extend(apply_patch_overrides_to_project_doc_deps(
        &mut doc,
        &manifest.patch,
    )?);

    let mut fetched = Vec::new();
    let mut index_used: Option<String> = None;
    let mut client: Option<SparseIndexClient> = None;

    let mut ctx = TransitiveResolveCtx {
        base,
        args: &effective_args,
        index: index.as_str(),
        patch: &manifest.patch,
        client: &mut client,
        fetched: &mut fetched,
        index_used: &mut index_used,
    };
    let transitive = match resolve_transitive_deps(&mut doc, &project_path, &mut ctx)? {
        TransitiveResolutionOutcome::Ok(res) => res,
        TransitiveResolutionOutcome::Error(err) => {
            let report = PkgReport {
                ok: false,
                command: "pkg.lock",
                result: None,
                error: Some(err),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
    };
    updated_specs.extend(transitive.updated_specs);
    updated_specs.sort();
    updated_specs.dedup();
    let mut added_specs = transitive.added_specs;
    added_specs.sort();
    added_specs.dedup();

    let project_changed = schema_bumped || transitive.changed || !updated_specs.is_empty();
    if project_changed {
        if args.check {
            let mut parts: Vec<String> = Vec::new();
            if schema_bumped {
                parts.push(format!(
                    "schema_version -> {}",
                    x07_contracts::PROJECT_MANIFEST_SCHEMA_VERSION
                ));
            }
            if !updated_specs.is_empty() {
                parts.push(format!("patched: {}", updated_specs.join(", ")));
            }
            if !added_specs.is_empty() {
                parts.push(format!("missing: {}", added_specs.join(", ")));
            }
            let extra = if parts.is_empty() {
                String::new()
            } else {
                format!(" ({})", parts.join("; "))
            };
            let report = PkgReport {
                ok: false,
                command: "pkg.lock",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_TRANSITIVE_MISSING".to_string(),
                    message: format!(
                        "project dependencies are out of date (run `x07 pkg lock` to update x07.json){}",
                        extra
                    ),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }

        // Validate the updated project document before writing it.
        let _ = {
            let bytes = serde_json::to_vec(&doc)?;
            project::parse_project_manifest_bytes(&bytes, &project_path)
                .context("validate updated project manifest")?
        };
        write_canonical_json_file(&project_path, &doc)
            .with_context(|| format!("write: {}", project_path.display()))?;
    }

    let manifest = {
        let bytes = serde_json::to_vec(&doc)?;
        project::parse_project_manifest_bytes(&bytes, &project_path)
            .context("load project manifest")?
    };

    let lock_path = project::default_lockfile_path(&project_path, &manifest);

    let mut lock = project::compute_lockfile(&project_path, &manifest)?;

    let existing_lock_bytes = match std::fs::read(&lock_path) {
        Ok(bytes) => Some(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(err).with_context(|| format!("read lockfile: {}", lock_path.display()))
        }
    };
    let existing_lock: Option<project::Lockfile> = match existing_lock_bytes.as_deref() {
        Some(bytes) => Some(
            serde_json::from_slice(bytes)
                .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?,
        ),
        None => None,
    };

    if let Some(err) = apply_lock_overrides_and_metadata(
        &manifest,
        &index,
        &effective_args,
        &mut client,
        &mut index_used,
        existing_lock.as_ref(),
        &mut lock,
    )? {
        let report = PkgReport {
            ok: false,
            command: "pkg.lock",
            result: Some(LockResult {
                project: project_path.display().to_string(),
                index: index_used.clone().or(effective_args.index.clone()),
                lockfile: lock_path.display().to_string(),
                fetched,
            }),
            error: Some(err),
        };
        return Ok((std::process::ExitCode::from(20), report));
    }

    lock.schema_version = requested_lock_version.schema_version().to_string();
    match requested_lock_version {
        RequestedLockVersion::V0_3_0 => {
            lock.toolchain = None;
            lock.registry = None;
        }
        RequestedLockVersion::V0_4_0 => {
            let compat = manifest
                .compat
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("current");
            lock.toolchain = Some(project::LockfileToolchain {
                x07_version: env!("CARGO_PKG_VERSION").to_string(),
                x07c_version: x07c::X07C_VERSION.to_string(),
                lang_id: x07c::language::LANG_ID.to_string(),
                compat: compat.to_string(),
            });
            let mut registry = project::LockfileRegistry {
                index_url: index_used.clone().unwrap_or_else(|| index.clone()),
                snapshot_hash: None,
            };
            // `--check` is often used with an index mirror solely for hydration.
            // Preserve the lockfile's declared registry so `--check` stays stable
            // across environments.
            if effective_args.check {
                if let Some(existing) = existing_lock.as_ref() {
                    if let Some(existing_registry) = existing.registry.clone() {
                        registry = existing_registry;
                    }
                }
            }
            lock.registry = Some(registry);
        }
    }

    if args.check {
        let existing = match existing_lock {
            Some(lock) => lock,
            None => {
                let report = PkgReport {
                    ok: false,
                    command: "pkg.lock",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_LOCK_MISSING".to_string(),
                        message: format!("missing lockfile: {}", lock_path.display()),
                    }),
                };
                return Ok((std::process::ExitCode::from(20), report));
            }
        };

        let metadata_online =
            !effective_args.offline && (effective_args.index.is_some() || index_used.is_some());

        if metadata_online {
            if !effective_args.allow_yanked {
                let yanked: Vec<String> = lock
                    .dependencies
                    .iter()
                    .filter(|d| d.yanked == Some(true))
                    .map(|d| format!("{}@{}", d.name, d.version))
                    .collect();
                if !yanked.is_empty() {
                    let report = PkgReport {
                        ok: false,
                        command: "pkg.lock",
                        result: Some(LockResult {
                            project: project_path.display().to_string(),
                            index: index_used.clone().or(effective_args.index.clone()),
                            lockfile: lock_path.display().to_string(),
                            fetched,
                        }),
                        error: Some(PkgError {
                            code: "X07PKG_YANKED_DEP".to_string(),
                            message: format!(
                                "lockfile contains yanked dependencies: {} (hint: use project.patch or bump; allow with --allow-yanked)",
                                yanked.join(", ")
                            ),
                        }),
                    };
                    return Ok((std::process::ExitCode::from(20), report));
                }
            }

            if !effective_args.allow_advisories {
                let mut advised: Vec<String> = Vec::new();
                for dep in &lock.dependencies {
                    if dep.advisories.is_empty() {
                        continue;
                    }
                    let mut ids: Vec<String> = dep
                        .advisories
                        .iter()
                        .map(|a| format!("{} ({})", a.id, a.summary))
                        .collect();
                    ids.sort();
                    advised.push(format!("{}@{}: {}", dep.name, dep.version, ids.join(", ")));
                }
                if !advised.is_empty() {
                    let report = PkgReport {
                        ok: false,
                        command: "pkg.lock",
                        result: Some(LockResult {
                            project: project_path.display().to_string(),
                            index: index_used.clone().or(effective_args.index.clone()),
                            lockfile: lock_path.display().to_string(),
                            fetched,
                        }),
                        error: Some(PkgError {
                            code: "X07PKG_ADVISED_DEP".to_string(),
                            message: format!(
                                "lockfile contains dependencies with active advisories: {} (hint: use project.patch or bump; allow with --allow-advisories)",
                                advised.join("; ")
                            ),
                        }),
                    };
                    return Ok((std::process::ExitCode::from(20), report));
                }
            }
        }

        let mismatch = if metadata_online {
            existing != lock
        } else {
            !lockfiles_equal_core(&existing, &lock)
        };
        if mismatch {
            let report = PkgReport {
                ok: false,
                command: "pkg.lock",
                result: Some(LockResult {
                    project: project_path.display().to_string(),
                    index: index_used.clone().or(effective_args.index.clone()),
                    lockfile: lock_path.display().to_string(),
                    fetched,
                }),
                error: Some(PkgError {
                    code: "X07PKG_LOCK_MISMATCH".to_string(),
                    message: format!("{} would change", lock_path.display()),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }

        let report = PkgReport {
            ok: true,
            command: "pkg.lock",
            result: Some(LockResult {
                project: project_path.display().to_string(),
                index: index_used.clone().or(effective_args.index.clone()),
                lockfile: lock_path.display().to_string(),
                fetched,
            }),
            error: None,
        };
        return Ok((std::process::ExitCode::SUCCESS, report));
    }

    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create lockfile dir: {}", parent.display()))?;
    }
    let mut out = serde_json::to_vec_pretty(&lock)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    std::fs::write(&lock_path, &out)
        .with_context(|| format!("write lockfile: {}", lock_path.display()))?;

    let report = PkgReport {
        ok: true,
        command: "pkg.lock",
        result: Some(LockResult {
            project: project_path.display().to_string(),
            index: index_used.or(effective_args.index.clone()),
            lockfile: lock_path.display().to_string(),
            fetched,
        }),
        error: None,
    };
    Ok((std::process::ExitCode::SUCCESS, report))
}

pub(crate) fn pkg_lock_for_init(
    args: &LockArgs,
) -> Result<(std::process::ExitCode, Option<String>)> {
    let (code, report) = pkg_lock_report(args)?;
    if report.ok {
        return Ok((code, None));
    }
    let msg = report
        .error
        .as_ref()
        .map(|e| format!("{}: {}", e.code, e.message))
        .unwrap_or_else(|| "unknown error".to_string());
    Ok((code, Some(msg)))
}

#[derive(Debug, Clone)]
struct TransitiveResolution {
    changed: bool,
    added_specs: Vec<String>,
    updated_specs: Vec<String>,
}

enum TransitiveResolutionOutcome {
    Ok(TransitiveResolution),
    Error(PkgError),
}

struct TransitiveResolveCtx<'a> {
    base: &'a Path,
    args: &'a LockArgs,
    index: &'a str,
    patch: &'a std::collections::BTreeMap<String, project::PatchSpec>,
    client: &'a mut Option<SparseIndexClient>,
    fetched: &'a mut Vec<FetchedDep>,
    index_used: &'a mut Option<String>,
}

fn resolve_transitive_deps(
    doc: &mut Value,
    project_path: &Path,
    ctx: &mut TransitiveResolveCtx<'_>,
) -> Result<TransitiveResolutionOutcome> {
    let mut scanned: HashSet<(String, String)> = HashSet::new();
    let mut added: BTreeSet<String> = BTreeSet::new();
    let mut updated: BTreeSet<String> = BTreeSet::new();
    let mut changed = false;

    loop {
        let deps = deps_from_project_doc(doc, project_path)?;
        if let Some(err) = ensure_deps_present(deps.as_slice(), ctx)? {
            return Ok(TransitiveResolutionOutcome::Error(err));
        }

        let mut round_added = false;
        for dep in deps {
            let key = (dep.name.clone(), dep.version.clone());
            if !scanned.insert(key) {
                continue;
            }
            let dep_dir = project::resolve_rel_path_with_workspace(ctx.base, &dep.path)?;
            let reqs = requires_packages_from_manifest(&dep_dir)?;
            for spec in reqs {
                let (name, version) = parse_pkg_spec(&spec)?;
                let (version, path) = apply_patch_override(&name, &version, ctx.patch);
                let allow_update = ctx.patch.contains_key(&name);
                match ensure_dep_entry(doc, &name, &version, &path, allow_update)? {
                    EnsureDepOutcome::Added => {
                        changed = true;
                        round_added = true;
                        added.insert(format!("{name}@{version}"));
                    }
                    EnsureDepOutcome::AlreadyPresentSameVersion => {}
                    EnsureDepOutcome::UpdatedExisting { existing_version } => {
                        changed = true;
                        round_added = true;
                        updated.insert(format!("{name}@{version}"));
                        if existing_version != version {
                            scanned.remove(&(name.clone(), existing_version));
                        }
                    }
                    EnsureDepOutcome::AlreadyPresentDifferentVersion { existing_version } => {
                        anyhow::bail!(
                            "dependency version conflict: project has {name}@{existing_version}, but {spec:?} is required by {}@{} (hint: use project.patch to override {name})",
                            dep.name,
                            dep.version
                        );
                    }
                }
            }
        }

        if !round_added {
            break;
        }
    }

    Ok(TransitiveResolutionOutcome::Ok(TransitiveResolution {
        changed,
        added_specs: added.into_iter().collect(),
        updated_specs: updated.into_iter().collect(),
    }))
}

#[derive(Debug, Clone)]
enum EnsureDepOutcome {
    Added,
    AlreadyPresentSameVersion,
    UpdatedExisting { existing_version: String },
    AlreadyPresentDifferentVersion { existing_version: String },
}

fn ensure_dep_entry(
    doc: &mut Value,
    name: &str,
    version: &str,
    path: &str,
    allow_update: bool,
) -> Result<EnsureDepOutcome> {
    let obj = doc
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;
    let deps_val = obj
        .entry("dependencies".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let deps = deps_val
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("project.dependencies must be an array"))?;

    let mut found_idx: Option<usize> = None;
    for (idx, dep) in deps.iter().enumerate() {
        let dep_name = dep.get("name").and_then(Value::as_str).unwrap_or("");
        if dep_name == name {
            found_idx = Some(idx);
            break;
        }
    }
    if let Some(idx) = found_idx {
        let dep_ver = deps[idx]
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("");
        let dep_path = deps[idx].get("path").and_then(Value::as_str).unwrap_or("");
        if dep_ver == version && (!allow_update || dep_path == path) {
            return Ok(EnsureDepOutcome::AlreadyPresentSameVersion);
        }
        if allow_update {
            let existing_version = dep_ver.to_string();
            {
                let dep = &mut deps[idx];
                if let Some(obj) = dep.as_object_mut() {
                    obj.insert("version".to_string(), Value::String(version.to_string()));
                    obj.insert("path".to_string(), Value::String(path.to_string()));
                }
            }
            sort_project_deps(deps);
            return Ok(EnsureDepOutcome::UpdatedExisting { existing_version });
        }
        return Ok(EnsureDepOutcome::AlreadyPresentDifferentVersion {
            existing_version: dep_ver.to_string(),
        });
    }

    deps.push(Value::Object(
        [
            ("name".to_string(), Value::String(name.to_string())),
            ("version".to_string(), Value::String(version.to_string())),
            ("path".to_string(), Value::String(path.to_string())),
        ]
        .into_iter()
        .collect(),
    ));
    sort_project_deps(deps);
    Ok(EnsureDepOutcome::Added)
}

fn sort_project_deps(deps: &mut [Value]) {
    deps.sort_by(|a, b| {
        let an = a.get("name").and_then(Value::as_str).unwrap_or("");
        let bn = b.get("name").and_then(Value::as_str).unwrap_or("");
        let c = an.cmp(bn);
        if c != std::cmp::Ordering::Equal {
            return c;
        }
        let av = a.get("version").and_then(Value::as_str).unwrap_or("");
        let bv = b.get("version").and_then(Value::as_str).unwrap_or("");
        av.cmp(bv)
    });
}

fn ensure_index_client(
    index: &str,
    client: &mut Option<SparseIndexClient>,
    index_used: &mut Option<String>,
) -> Result<Option<PkgError>> {
    if client.is_none() {
        let token = x07_pkg::load_token(index).unwrap_or(None);
        *client = match SparseIndexClient::from_index_url(index, token) {
            Ok(c) => Some(c),
            Err(err) => {
                return Ok(Some(PkgError {
                    code: "X07PKG_INDEX_CONFIG".to_string(),
                    message: format!("{err:#}"),
                }))
            }
        };
    }
    if index_used.is_none() {
        *index_used = Some(index.to_string());
    }
    Ok(None)
}

fn apply_patch_override(
    name: &str,
    version: &str,
    patch: &std::collections::BTreeMap<String, project::PatchSpec>,
) -> (String, String) {
    match patch.get(name) {
        Some(spec) => {
            let version = spec.version.clone();
            let path = spec
                .path
                .clone()
                .unwrap_or_else(|| format!(".x07/deps/{name}/{version}"));
            (version, path)
        }
        None => (version.to_string(), format!(".x07/deps/{name}/{version}")),
    }
}

fn apply_patch_overrides_to_project_doc_deps(
    doc: &mut Value,
    patch: &std::collections::BTreeMap<String, project::PatchSpec>,
) -> Result<Vec<String>> {
    if patch.is_empty() {
        return Ok(Vec::new());
    }

    let obj = doc
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;
    let Some(deps_val) = obj.get_mut("dependencies") else {
        return Ok(Vec::new());
    };
    let Some(deps) = deps_val.as_array_mut() else {
        anyhow::bail!("project.dependencies must be an array");
    };

    let mut updated: Vec<String> = Vec::new();
    for dep in deps.iter_mut() {
        let Some(dep_obj) = dep.as_object_mut() else {
            continue;
        };
        let name = dep_obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        if !patch.contains_key(&name) {
            continue;
        }
        let current_version = dep_obj
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let current_path = dep_obj
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();

        let (desired_version, desired_path) = apply_patch_override(&name, current_version, patch);
        if current_version != desired_version || current_path != desired_path {
            dep_obj.insert("name".to_string(), Value::String(name.clone()));
            dep_obj.insert(
                "version".to_string(),
                Value::String(desired_version.clone()),
            );
            dep_obj.insert("path".to_string(), Value::String(desired_path));
            updated.push(format!("{name}@{desired_version}"));
        }
    }

    if !updated.is_empty() {
        sort_project_deps(deps);
    }

    Ok(updated)
}

fn normalize_project_doc_deps(doc: &mut Value) -> Result<Vec<String>> {
    let obj = doc
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;
    let Some(deps_val) = obj.get_mut("dependencies") else {
        return Ok(Vec::new());
    };
    let Some(deps) = deps_val.as_array_mut() else {
        anyhow::bail!("project.dependencies must be an array");
    };

    let mut normalized: Vec<String> = Vec::new();
    for dep in deps.iter_mut() {
        let Some(dep_obj) = dep.as_object_mut() else {
            continue;
        };
        let name = dep_obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let version = dep_obj
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() || version.is_empty() {
            continue;
        }
        let path = dep_obj
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !path.is_empty() {
            continue;
        }
        dep_obj.insert(
            "path".to_string(),
            Value::String(format!(".x07/deps/{name}/{version}")),
        );
        normalized.push(format!("{name}@{version}"));
    }

    if !normalized.is_empty() {
        sort_project_deps(deps);
    }

    Ok(normalized)
}

fn lockfiles_equal_core(a: &project::Lockfile, b: &project::Lockfile) -> bool {
    if a.schema_version.trim() != b.schema_version.trim() {
        return false;
    }
    if a.toolchain != b.toolchain {
        return false;
    }
    if a.registry != b.registry {
        return false;
    }
    if a.dependencies.len() != b.dependencies.len() {
        return false;
    }
    for (da, db) in a.dependencies.iter().zip(b.dependencies.iter()) {
        if da.name != db.name || da.version != db.version || da.path != db.path {
            return false;
        }
        if da.package_manifest_sha256 != db.package_manifest_sha256 {
            return false;
        }
        if da.module_root != db.module_root {
            return false;
        }
        if da.modules_sha256 != db.modules_sha256 {
            return false;
        }
        if da.overridden_by != db.overridden_by {
            return false;
        }
        if da.yanked != db.yanked {
            return false;
        }
    }
    true
}

type LockMetadataKey = (String, String, String, String);
type LockMetadataValue = (Option<bool>, Vec<project::LockAdvisory>);
type LockMetadataMap = std::collections::HashMap<LockMetadataKey, LockMetadataValue>;

fn preserve_lock_metadata(existing: &project::Lockfile, lock: &mut project::Lockfile) {
    let mut map: LockMetadataMap = LockMetadataMap::new();
    for dep in &existing.dependencies {
        map.insert(
            (
                dep.name.clone(),
                dep.version.clone(),
                dep.path.clone(),
                dep.package_manifest_sha256.clone(),
            ),
            (dep.yanked, dep.advisories.clone()),
        );
    }
    for dep in &mut lock.dependencies {
        if let Some((yanked, advisories)) = map.get(&(
            dep.name.clone(),
            dep.version.clone(),
            dep.path.clone(),
            dep.package_manifest_sha256.clone(),
        )) {
            if yanked.is_some() {
                dep.yanked = *yanked;
            }
            dep.advisories = advisories.clone();
        }
    }
}

fn apply_lock_overrides_and_metadata(
    manifest: &project::ProjectManifest,
    index: &str,
    args: &LockArgs,
    client: &mut Option<SparseIndexClient>,
    index_used: &mut Option<String>,
    existing_lock: Option<&project::Lockfile>,
    lock: &mut project::Lockfile,
) -> Result<Option<PkgError>> {
    for dep in &mut lock.dependencies {
        if manifest.patch.contains_key(&dep.name) {
            dep.overridden_by = Some(dep.name.clone());
        } else {
            dep.overridden_by = None;
        }
    }

    if args.offline {
        if let Some(existing) = existing_lock {
            preserve_lock_metadata(existing, lock);
        }
        return Ok(None);
    }

    let metadata_online = args.index.is_some()
        || std::env::var("X07_PKG_INDEX_URL")
            .ok()
            .is_some_and(|v| !v.trim().is_empty())
        || index_used.is_some();
    if !metadata_online {
        if let Some(existing) = existing_lock {
            preserve_lock_metadata(existing, lock);
        }
        return Ok(None);
    }

    let needs_index = lock
        .dependencies
        .iter()
        .any(|d| project::is_vendored_dep_path(&d.path));
    if !needs_index {
        return Ok(None);
    }
    if let Some(err) = ensure_index_client(index, client, index_used)? {
        return Ok(Some(err));
    }
    let client = client.as_ref().expect("client initialized");

    let mut cache: std::collections::HashMap<String, Vec<x07_pkg::IndexEntry>> =
        std::collections::HashMap::new();

    for dep in &mut lock.dependencies {
        if !project::is_vendored_dep_path(&dep.path) {
            continue;
        }

        let entries = match cache.get(&dep.name) {
            Some(entries) => entries,
            None => {
                let entries = match client.fetch_entries(&dep.name) {
                    Ok(entries) => entries,
                    Err(err) => {
                        return Ok(Some(PkgError {
                            code: "X07PKG_INDEX_FETCH".to_string(),
                            message: format!(
                                "fetch index entries for {:?}: {err:#} (hint: check the package name and index URL)",
                                dep.name
                            ),
                        }))
                    }
                };
                cache.insert(dep.name.clone(), entries);
                cache.get(&dep.name).expect("cache insert")
            }
        };

        let Some(entry) = entries
            .iter()
            .find(|e| e.name == dep.name && e.version == dep.version)
        else {
            return Ok(Some(PkgError {
                code: "X07PKG_INDEX_NO_MATCH".to_string(),
                message: format!(
                    "no index entry for {:?}@{:?} (hint: run `x07 pkg versions {}`)",
                    dep.name, dep.version, dep.name
                ),
            }));
        };

        dep.yanked = Some(entry.yanked);
        dep.advisories = entry
            .advisories
            .iter()
            .map(|a| project::LockAdvisory {
                schema_version: a.schema_version.clone(),
                id: a.id.clone(),
                package: a.package.clone(),
                version: a.version.clone(),
                kind: a.kind.clone(),
                severity: a.severity.clone(),
                summary: a.summary.clone(),
                url: a.url.clone(),
                details: a.details.clone(),
                created_at_utc: a.created_at_utc.clone(),
                withdrawn_at_utc: a.withdrawn_at_utc.clone(),
            })
            .collect();
        dep.advisories.sort_by(|a, b| a.id.cmp(&b.id));
    }

    Ok(None)
}

fn write_canonical_json_file(path: &Path, doc: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }
    let mut out = serde_json::to_vec_pretty(doc)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    std::fs::write(path, &out).with_context(|| format!("write: {}", path.display()))?;
    Ok(())
}

fn deps_from_project_doc(doc: &Value, project_path: &Path) -> Result<Vec<project::DependencySpec>> {
    let obj = doc
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;
    let Some(deps_val) = obj.get("dependencies") else {
        return Ok(Vec::new());
    };
    let deps = deps_val
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("project.dependencies must be an array"))?;
    let mut out = Vec::with_capacity(deps.len());
    for (idx, dep) in deps.iter().enumerate() {
        let name = dep
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("project.dependencies[{idx}].name must be a string"))?
            .trim()
            .to_string();
        let version = dep
            .get("version")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("project.dependencies[{idx}].version must be a string"))?
            .trim()
            .to_string();
        let path = dep
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("project.dependencies[{idx}].path must be a string"))?
            .trim()
            .to_string();
        if name.is_empty() || version.is_empty() || path.is_empty() {
            anyhow::bail!(
                "invalid dependency entry in {} at index {idx}",
                project_path.display()
            );
        }
        out.push(project::DependencySpec {
            name,
            version,
            path,
        });
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix time")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("x07_pkg_{prefix}_{}_{}", std::process::id(), stamp));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn temp_unpack_dir_cleanup_is_best_effort_and_does_not_leak() {
        let dir = temp_dir("unpack_cleanup");
        let unpack_path = {
            let guard = TempUnpackDir::create(&dir).expect("create temp unpack dir");
            let p = guard.path().to_path_buf();
            std::fs::write(p.join("fixture.txt"), b"ok").expect("write fixture");
            assert!(p.is_dir());
            p
        };
        assert!(!unpack_path.exists());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn temp_unpack_dir_can_be_persisted_without_being_deleted() {
        let dir = temp_dir("unpack_persist");
        let dest = dir.join(".x07").join("deps").join("persisted");
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).expect("create dest parent");
        }

        let mut guard = TempUnpackDir::create(&dir).expect("create temp unpack dir");
        let unpack_path = guard.path().to_path_buf();
        std::fs::write(unpack_path.join("fixture.txt"), b"ok").expect("write fixture");
        guard.persist_to(&dest).expect("persist");
        drop(guard);

        assert!(!unpack_path.exists());
        assert!(dest.is_dir());
        assert_eq!(
            std::fs::read(dest.join("fixture.txt")).expect("read"),
            b"ok"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn pkg_repair_offline_upgrades_incompatible_locked_dependency() {
        let dir = temp_dir("repair_offline");
        std::fs::create_dir_all(dir.join("src")).expect("create src dir");

        let dep_name = "dep-repair";
        let v_bad = "1.0.0";
        let v_ok = "1.0.1";

        for (ver, compat) in [(v_bad, "<0.0.1"), (v_ok, ">=0.0.1")] {
            let dep_dir = dir.join(".x07/deps").join(dep_name).join(ver);
            std::fs::create_dir_all(dep_dir.join("src/dep")).expect("create dep module dir");
            std::fs::write(
                dep_dir.join("x07-package.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "schema_version": "x07.package@0.1.0",
                    "name": dep_name,
                    "version": ver,
                    "module_root": "src",
                    "modules": ["dep.lib"],
                    "meta": { "x07c_compat": compat }
                }))
                .expect("serialize package manifest"),
            )
            .expect("write package manifest");
            std::fs::write(
                dep_dir.join("src/dep/lib.x07.json"),
                br#"{"schema_version":"x07.x07ast@0.8.0","decls":[]}"#,
            )
            .expect("write module");
        }

        let project_path = dir.join("x07.json");
        std::fs::write(
            &project_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": "x07.project@0.5.0",
                "world": "solve-pure",
                "entry": "src/main.x07.json",
                "module_roots": ["src"],
                "dependencies": [
                    { "name": dep_name, "version": v_bad, "path": format!(".x07/deps/{dep_name}/{v_bad}") }
                ]
            }))
            .expect("serialize project"),
        )
        .expect("write project");
        std::fs::write(
            dir.join("src/main.x07.json"),
            br#"{"schema_version":"x07.x07ast@0.8.0","decls":[]}"#,
        )
        .expect("write entry");

        let manifest = project::load_project_manifest(&project_path).expect("load project");
        let lock = project::compute_lockfile(&project_path, &manifest).expect("compute lock");
        let lock_path = project::default_lockfile_path(&project_path, &manifest);
        std::fs::write(
            &lock_path,
            serde_json::to_vec_pretty(&lock).expect("serialize lock"),
        )
        .expect("write lock");

        let args = RepairArgs {
            project: project_path.clone(),
            index: None,
            toolchain: "current".to_string(),
            offline: true,
        };
        let (code, report) = pkg_repair_report(&args).expect("repair report");
        assert_eq!(code, std::process::ExitCode::SUCCESS);
        assert!(report.ok);

        let repaired = report.result.expect("result").repaired;
        assert_eq!(repaired.len(), 1);
        assert_eq!(repaired[0].name, dep_name);
        assert_eq!(repaired[0].from_version, v_bad);
        assert_eq!(repaired[0].to_version, v_ok);

        let updated_project: Value =
            serde_json::from_slice(&std::fs::read(&project_path).expect("read project"))
                .expect("parse project");
        assert_eq!(
            updated_project
                .pointer("/dependencies/0/version")
                .and_then(Value::as_str),
            Some(v_ok)
        );
        assert_eq!(
            updated_project
                .pointer(&format!("/patch/{dep_name}/version"))
                .and_then(Value::as_str),
            Some(v_ok)
        );

        let updated_lock: project::Lockfile =
            serde_json::from_slice(&std::fs::read(&lock_path).expect("read lock"))
                .expect("parse lock");
        assert_eq!(updated_lock.dependencies.len(), 1);
        assert_eq!(updated_lock.dependencies[0].name, dep_name);
        assert_eq!(updated_lock.dependencies[0].version, v_ok);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    fn write_fixture_project(root: &Path) -> (PathBuf, PathBuf) {
        let dep_dir = root.join("deps/dep_local");
        std::fs::create_dir_all(dep_dir.join("src/dep")).expect("create dep module dir");
        std::fs::write(
            dep_dir.join("x07-package.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": "x07.package@0.1.0",
                "name": "dep-local",
                "version": "1.2.3",
                "module_root": "src",
                "modules": ["dep.lib"]
            }))
            .expect("serialize package manifest"),
        )
        .expect("write package manifest");
        std::fs::write(
            dep_dir.join("src/dep/lib.x07.json"),
            br#"{"schema_version":"x07.x07ast@0.8.0","decls":[]}"#,
        )
        .expect("write module");

        let project_path = root.join("x07.json");
        std::fs::write(
            &project_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": "x07.project@0.3.0",
                "world": "solve-pure",
                "entry": "src/main.x07.json",
                "module_roots": ["src"],
                "dependencies": [
                    {
                        "name": "dep-local",
                        "version": "1.2.3",
                        "path": "deps/dep_local"
                    }
                ]
            }))
            .expect("serialize project"),
        )
        .expect("write project");
        std::fs::create_dir_all(root.join("src")).expect("create src dir");
        std::fs::write(
            root.join("src/main.x07.json"),
            br#"{"schema_version":"x07.x07ast@0.8.0","decls":[]}"#,
        )
        .expect("write entry");

        let manifest = project::load_project_manifest(&project_path).expect("load project");
        let lock = project::compute_lockfile(&project_path, &manifest).expect("compute lock");
        let lock_path = project::default_lockfile_path(&project_path, &manifest);
        std::fs::write(
            &lock_path,
            serde_json::to_vec_pretty(&lock).expect("serialize lock"),
        )
        .expect("write lock");
        (project_path, lock_path)
    }

    #[test]
    fn build_dep_closure_attestation_offline_records_dependency_inventory() {
        let dir = temp_dir("attest_closure_ok");
        let (project_path, _lock_path) = write_fixture_project(&dir);

        let args = AttestClosureArgs {
            project: project_path.clone(),
            out: dir.join("target/dep.closure.attest.json"),
            index: None,
            offline: true,
            allow_yanked: false,
            allow_advisories: false,
        };
        let (attestation, ok) =
            build_dep_closure_attestation(&args).expect("build dependency closure attestation");

        assert!(ok);
        assert_eq!(
            attestation.schema_version,
            X07_DEP_CLOSURE_ATTEST_SCHEMA_VERSION
        );
        assert_eq!(attestation.dependencies.len(), 1);
        assert_eq!(attestation.dependencies[0].name, "dep-local");
        assert!(attestation.package_set_digest.starts_with("sha256:"));
        assert!(attestation.advisory_check.ok);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn build_dep_closure_attestation_preserves_offline_lock_metadata() {
        let dir = temp_dir("attest_closure_metadata");
        let (project_path, lock_path) = write_fixture_project(&dir);

        let mut lock_doc: Value =
            serde_json::from_slice(&std::fs::read(&lock_path).expect("read lock for mutation"))
                .expect("parse lock doc");
        let dep = lock_doc
            .pointer_mut("/dependencies/0")
            .expect("dependency entry");
        dep.as_object_mut()
            .expect("dep object")
            .insert("yanked".to_string(), Value::Bool(true));
        dep.as_object_mut().expect("dep object").insert(
            "advisories".to_string(),
            serde_json::json!([{
                "schema_version": "x07.pkg.advisory@0.1.0",
                "id": "X07-2026-0001",
                "package": "dep-local",
                "version": "1.2.3",
                "kind": "security",
                "severity": "high",
                "summary": "fixture advisory",
                "created_at_utc": "2026-03-15T00:00:00Z"
            }]),
        );
        std::fs::write(
            &lock_path,
            serde_json::to_vec_pretty(&lock_doc).expect("serialize mutated lock"),
        )
        .expect("write mutated lock");

        let args = AttestClosureArgs {
            project: project_path,
            out: dir.join("target/dep.closure.attest.json"),
            index: None,
            offline: true,
            allow_yanked: false,
            allow_advisories: false,
        };
        let (attestation, ok) =
            build_dep_closure_attestation(&args).expect("build dependency closure attestation");

        assert!(!ok);
        assert_eq!(
            attestation.advisory_check.yanked,
            vec!["dep-local@1.2.3".to_string()]
        );
        assert_eq!(attestation.advisory_check.advisories.len(), 1);
        assert_eq!(
            attestation.dependencies[0].advisories,
            vec!["X07-2026-0001".to_string()]
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}

fn requires_packages_from_manifest(dep_dir: &Path) -> Result<Vec<String>> {
    let path = dep_dir.join("x07-package.json");
    let bytes =
        std::fs::read(&path).with_context(|| format!("read package: {}", path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse package JSON: {}", path.display()))?;
    let Some(meta) = doc.get("meta").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    let Some(reqs) = meta.get("requires_packages") else {
        return Ok(Vec::new());
    };
    let Some(reqs) = reqs.as_array() else {
        anyhow::bail!(
            "package meta.requires_packages must be an array: {}",
            path.display()
        );
    };
    let mut out = Vec::new();
    for (idx, raw) in reqs.iter().enumerate() {
        let Some(spec) = raw.as_str() else {
            anyhow::bail!(
                "package meta.requires_packages[{idx}] must be a string: {}",
                path.display()
            );
        };
        let spec = spec.trim();
        if spec.is_empty() {
            continue;
        }
        out.push(spec.to_string());
    }
    out.sort();
    out.dedup();
    Ok(out)
}

pub(crate) fn official_ext_packages_dir() -> Option<PathBuf> {
    if let Ok(v) = std::env::var("X07_REPO_ROOT") {
        let root = PathBuf::from(v);
        let cand = root.join("packages").join("ext");
        if std::fs::read_dir(&cand).is_ok() {
            return Some(cand);
        }
    }

    for anc in Path::new(env!("CARGO_MANIFEST_DIR")).ancestors() {
        let cand = anc.join("packages").join("ext");
        if std::fs::read_dir(&cand).is_ok() {
            return Some(cand);
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(toolchain_root) = util::detect_toolchain_root_best_effort(&cwd) {
            let cand = toolchain_root.join("packages").join("ext");
            if std::fs::read_dir(&cand).is_ok() {
                return Some(cand);
            }
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        for anc in exe.ancestors() {
            let cand = anc.join("packages").join("ext");
            if std::fs::read_dir(&cand).is_ok() {
                return Some(cand);
            }
        }
    }

    None
}

pub(crate) fn official_ext_packages_dir_required() -> Result<PathBuf> {
    let mut tried: Vec<(PathBuf, std::io::Error)> = Vec::new();

    if let Ok(v) = std::env::var("X07_REPO_ROOT") {
        let root = PathBuf::from(v);
        let cand = root.join("packages").join("ext");
        match std::fs::read_dir(&cand) {
            Ok(_) => return Ok(cand),
            Err(err) => tried.push((cand, err)),
        }
    }

    for anc in Path::new(env!("CARGO_MANIFEST_DIR")).ancestors() {
        let cand = anc.join("packages").join("ext");
        match std::fs::read_dir(&cand) {
            Ok(_) => return Ok(cand),
            Err(err) => tried.push((cand, err)),
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(toolchain_root) = util::detect_toolchain_root_best_effort(&cwd) {
            let cand = toolchain_root.join("packages").join("ext");
            match std::fs::read_dir(&cand) {
                Ok(_) => return Ok(cand),
                Err(err) => tried.push((cand, err)),
            }
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        for anc in exe.ancestors() {
            let cand = anc.join("packages").join("ext");
            match std::fs::read_dir(&cand) {
                Ok(_) => return Ok(cand),
                Err(err) => tried.push((cand, err)),
            }
        }
    }

    tried.sort_by(|(a, _), (b, _)| a.cmp(b));
    tried.dedup_by(|(a, _), (b, _)| a == b);

    let mut msg = String::from("could not find toolchain packages/ext directory");
    if !tried.is_empty() {
        msg.push_str("\ntried:");
        for (path, err) in tried {
            msg.push_str(&format!("\n- {}: {}", path.display(), err));
        }
        msg.push('\n');
    }
    anyhow::bail!(msg);
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("create dir: {}", dst.display()))?;

    for entry in std::fs::read_dir(src).with_context(|| format!("read dir: {}", src.display()))? {
        let entry = entry.with_context(|| format!("read dir entry: {}", src.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("read file type: {}", entry.path().display()))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
            continue;
        }

        if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "copy file: {} -> {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
            continue;
        }

        anyhow::bail!(
            "unsupported file type in {}: {}",
            src.display(),
            src_path.display()
        );
    }

    Ok(())
}

fn try_copy_official_dep(
    official_ext: &Path,
    dep: &project::DependencySpec,
    base: &Path,
) -> Result<bool> {
    let src = official_ext
        .join(format!("x07-{}", dep.name))
        .join(&dep.version);
    if !src.join("x07-package.json").is_file() {
        return Ok(false);
    }

    let dst = project::resolve_rel_path_with_workspace(base, &dep.path)?;
    if dst.exists() {
        if dst.is_dir() {
            std::fs::remove_dir_all(&dst)
                .with_context(|| format!("remove existing dep dir: {}", dst.display()))?;
        } else {
            anyhow::bail!(
                "dependency path exists but is not a directory: {}",
                dst.display()
            );
        }
    }

    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dep parent dir: {}", parent.display()))?;
    }
    copy_dir_recursive(&src, &dst).with_context(|| {
        format!(
            "copy official package {}@{} from {}",
            dep.name,
            dep.version,
            src.display()
        )
    })?;

    Ok(true)
}

fn ensure_deps_present(
    deps: &[project::DependencySpec],
    ctx: &mut TransitiveResolveCtx<'_>,
) -> Result<Option<PkgError>> {
    let mut missing_local_override: Vec<String> = Vec::new();
    let mut missing_local: Vec<String> = Vec::new();
    let mut missing: Vec<&project::DependencySpec> = Vec::new();
    for dep in deps {
        let dep_dir = project::resolve_rel_path_with_workspace(ctx.base, &dep.path)?;
        let pkg_manifest_path = dep_dir.join("x07-package.json");
        if pkg_manifest_path.is_file() {
            let bytes = std::fs::read(&pkg_manifest_path).with_context(|| {
                format!("read package manifest: {}", pkg_manifest_path.display())
            })?;
            if let Some(err) =
                check_pkg_x07c_compat(&dep.name, &dep.version, &pkg_manifest_path, &bytes)?
            {
                return Ok(Some(err));
            }
            continue;
        }
        let patched_by_path = ctx.patch.get(&dep.name).is_some_and(|s| s.path.is_some());
        if patched_by_path && !project::is_vendored_dep_path(&dep.path) {
            missing_local_override.push(format!(
                "{}@{} ({})",
                dep.name,
                dep.version,
                dep_dir.display()
            ));
        } else if !project::is_vendored_dep_path(&dep.path) {
            missing_local.push(format!(
                "{}@{} ({})",
                dep.name,
                dep.version,
                dep_dir.display()
            ));
        } else {
            missing.push(dep);
        }
    }

    if !missing_local_override.is_empty() {
        missing_local_override.sort();
        missing_local_override.dedup();
        return Ok(Some(PkgError {
            code: "X07PKG_PATCH_MISSING_DEP".to_string(),
            message: format!(
                "patched dependencies are missing on disk: {}",
                missing_local_override.join(", ")
            ),
        }));
    }

    if !missing_local.is_empty() {
        missing_local.sort();
        missing_local.dedup();
        return Ok(Some(PkgError {
            code: "X07PKG_LOCAL_MISSING_DEP".to_string(),
            message: format!(
                "local dependencies are missing on disk: {}",
                missing_local.join(", ")
            ),
        }));
    }

    if missing.is_empty() {
        return Ok(None);
    }

    if let Some(official_ext) = official_ext_packages_dir() {
        let mut still_missing: Vec<&project::DependencySpec> = Vec::new();
        for dep in missing {
            if try_copy_official_dep(&official_ext, dep, ctx.base)? {
                let dep_dir = project::resolve_rel_path_with_workspace(ctx.base, &dep.path)?;
                let pkg_manifest_path = dep_dir.join("x07-package.json");
                let bytes = std::fs::read(&pkg_manifest_path).with_context(|| {
                    format!("read package manifest: {}", pkg_manifest_path.display())
                })?;
                if let Some(err) =
                    check_pkg_x07c_compat(&dep.name, &dep.version, &pkg_manifest_path, &bytes)?
                {
                    return Ok(Some(err));
                }
                continue;
            }
            still_missing.push(dep);
        }

        if still_missing.is_empty() {
            return Ok(None);
        }
        missing = still_missing;
    }

    if ctx.args.offline {
        return Ok(Some(PkgError {
            code: "X07PKG_OFFLINE_MISSING_DEP".to_string(),
            message: format!("{} missing dependencies (offline mode)", missing.len()),
        }));
    }

    if let Some(err) = ensure_index_client(ctx.index, ctx.client, ctx.index_used)? {
        return Ok(Some(err));
    }
    let client = ctx.client.as_ref().expect("client initialized");

    for dep in missing {
        let dep_dir = project::resolve_rel_path_with_workspace(ctx.base, &dep.path)?;
        let entries = match client.fetch_entries(&dep.name) {
            Ok(entries) => entries,
            Err(err) => {
                return Ok(Some(PkgError {
                    code: "X07PKG_INDEX_FETCH".to_string(),
                    message: format!(
                        "fetch index entries for {:?}: {err:#} (hint: check the package name and index URL)",
                        dep.name
                    ),
                }))
            }
        };
        let Some(entry) = entries
            .into_iter()
            .find(|e| e.name == dep.name && e.version == dep.version)
        else {
            return Ok(Some(PkgError {
                code: "X07PKG_INDEX_NO_MATCH".to_string(),
                message: format!(
                    "no index entry for {:?}@{:?} (hint: run `x07 pkg versions {}`)",
                    dep.name, dep.version, dep.name
                ),
            }));
        };

        let cache_dir = ctx.base.join(".x07").join("cache").join("sha256");
        let archive_path = cache_dir.join(format!("{}.x07pkg", entry.cksum));
        if archive_path.is_file() {
            let bytes = std::fs::read(&archive_path)
                .with_context(|| format!("read cached archive: {}", archive_path.display()))?;
            let actual = x07_pkg::sha256_hex(&bytes);
            if actual != entry.cksum {
                anyhow::bail!(
                    "cached archive sha256 mismatch: expected {} got {} ({})",
                    entry.cksum,
                    actual,
                    archive_path.display()
                );
            }
        } else if let Err(err) =
            client.download_to_file(&dep.name, &dep.version, &entry.cksum, &archive_path)
        {
            return Ok(Some(PkgError {
                code: "X07PKG_DOWNLOAD_FAILED".to_string(),
                message: format!(
                    "download {:?}@{:?}: {err:#} (hint: check network access and index URL)",
                    dep.name, dep.version
                ),
            }));
        }

        let archive_bytes = std::fs::read(&archive_path)
            .with_context(|| format!("read archive for {:?}@{:?}", dep.name, dep.version))?;
        let mut tmp_dir = TempUnpackDir::create(ctx.base)?;
        x07_pkg::unpack_tar_bytes(&archive_bytes, tmp_dir.path())?;

        let (pkg, pkg_manifest_path, pkg_manifest_bytes) =
            project::load_package_manifest(tmp_dir.path()).with_context(|| {
                format!("validate unpacked package at {}", tmp_dir.path().display())
            })?;
        if pkg.name != dep.name || pkg.version != dep.version {
            anyhow::bail!(
                "unpacked package identity mismatch: expected {:?}@{:?} got {:?}@{:?}",
                dep.name,
                dep.version,
                pkg.name,
                pkg.version
            );
        }
        if let Some(err) = check_pkg_x07c_compat(
            &pkg.name,
            &pkg.version,
            &pkg_manifest_path,
            &pkg_manifest_bytes,
        )? {
            return Ok(Some(err));
        }

        if dep_dir.exists() {
            std::fs::remove_dir_all(&dep_dir)
                .with_context(|| format!("remove existing dep dir: {}", dep_dir.display()))?;
        }
        if let Some(parent) = dep_dir.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dep parent: {}", parent.display()))?;
        }
        tmp_dir.persist_to(&dep_dir).with_context(|| {
            format!(
                "move unpacked package into place: {} -> {}",
                tmp_dir.path().display(),
                dep_dir.display()
            )
        })?;

        ctx.fetched.push(FetchedDep {
            name: dep.name.clone(),
            version: dep.version.clone(),
            path: dep.path.clone(),
            sha256: entry.cksum,
        });
    }
    Ok(None)
}

fn cmd_pkg_login(args: LoginArgs) -> Result<std::process::ExitCode> {
    let token = match (args.token, args.token_stdin) {
        (Some(token), false) => token,
        (None, true) => {
            let mut buf = String::new();
            if let Err(err) = std::io::stdin().read_to_string(&mut buf) {
                let report = PkgReport::<LoginResult> {
                    ok: false,
                    command: "pkg.login",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_LOGIN_TOKEN".to_string(),
                        message: format!("read token from stdin: {err}"),
                    }),
                };
                println!("{}", serde_json::to_string(&report)?);
                return Ok(std::process::ExitCode::from(20));
            }
            buf
        }
        (None, false) => match rpassword::prompt_password("Token: ") {
            Ok(token) => token,
            Err(err) => {
                let report = PkgReport::<LoginResult> {
                    ok: false,
                    command: "pkg.login",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_LOGIN_TOKEN".to_string(),
                        message: format!("{err}"),
                    }),
                };
                println!("{}", serde_json::to_string(&report)?);
                return Ok(std::process::ExitCode::from(20));
            }
        },
        (Some(_), true) => unreachable!("clap enforces token/token-stdin mutual exclusion"),
    };

    if let Err(err) = x07_pkg::store_token(&args.index, &token) {
        let report = PkgReport::<LoginResult> {
            ok: false,
            command: "pkg.login",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_LOGIN_FAILED".to_string(),
                message: format!("{err:#}"),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }
    let report = PkgReport {
        ok: true,
        command: "pkg.login",
        result: Some(LoginResult {
            index: args.index,
            credentials_path: x07_pkg::credentials_path()?.display().to_string(),
        }),
        error: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_pkg_publish(args: PublishArgs) -> Result<std::process::ExitCode> {
    let package_dir = util::resolve_existing_path_upwards(&args.package);
    let (pkg, tar) = pack_package_to_tar(&package_dir)?;

    let token = x07_pkg::load_token(&args.index).unwrap_or(None);
    let client = match SparseIndexClient::from_index_url(&args.index, token.clone()) {
        Ok(c) => c,
        Err(err) => {
            let report = PkgReport::<PublishResult> {
                ok: false,
                command: "pkg.publish",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_INDEX_CONFIG".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };

    let publish_url = match client.api_root().join("packages/publish") {
        Ok(u) => u,
        Err(err) => {
            let report = PkgReport::<PublishResult> {
                ok: false,
                command: "pkg.publish",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_API_URL".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };

    let resp_bytes = match x07_pkg::http_post_bytes(&publish_url, token.as_deref(), &tar) {
        Ok(bytes) => bytes,
        Err(err) => {
            let report = PkgReport::<PublishResult> {
                ok: false,
                command: "pkg.publish",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_PUBLISH_FAILED".to_string(),
                    message: format!("{err:#}"),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };

    let resp_json: serde_json::Value = match serde_json::from_slice(&resp_bytes) {
        Ok(v) => v,
        Err(err) => {
            let report = PkgReport::<PublishResult> {
                ok: false,
                command: "pkg.publish",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_PUBLISH_RESPONSE".to_string(),
                    message: format!("parse publish response: {err}"),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };
    let name = resp_json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(&pkg.name)
        .to_string();
    let version = resp_json
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or(&pkg.version)
        .to_string();
    let cksum = resp_json
        .get("cksum")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let index_path = resp_json
        .get("index_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if name != pkg.name || version != pkg.version || cksum != x07_pkg::sha256_hex(&tar) {
        let report = PkgReport::<PublishResult> {
            ok: false,
            command: "pkg.publish",
            result: None,
            error: Some(PkgError {
                code: "X07PKG_PUBLISH_RESPONSE_MISMATCH".to_string(),
                message: "publish response did not match the uploaded archive".to_string(),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    // Best-effort publish verification: check the registry API for the just-published version.
    //
    // This is intentionally non-fatal because the sparse index (and sometimes API frontends)
    // may take a short time to reflect the latest publish.
    if matches!(client.api_root().scheme(), "http" | "https") {
        if let Ok(detail_url) = client.api_root().join(&format!("packages/{name}")) {
            match x07_pkg::http_get_bytes(&detail_url, token.as_deref()) {
                Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                    Ok(v) => {
                        let has_version = v
                            .get("versions")
                            .and_then(|vv| vv.as_array())
                            .is_some_and(|vs| {
                                vs.iter().any(|row| {
                                    row.get("version")
                                        .and_then(|s| s.as_str())
                                        .is_some_and(|s| s == version)
                                })
                            });
                        if !has_version {
                            eprintln!(
                                "warning: publish verified upload ok, but registry API did not yet list {name}@{version} at {}",
                                detail_url.as_str()
                            );
                            eprintln!(
                                "warning: sparse index reads (x07 pkg versions) may be cached; try `x07 pkg versions --refresh {name}` or retry API verification via GET /v1/packages/<name>."
                            );
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "warning: publish verified upload ok, but failed to parse registry API response at {}: {err}",
                            detail_url.as_str()
                        );
                        eprintln!(
                            "warning: sparse index reads (x07 pkg versions) may be cached; try `x07 pkg versions --refresh {name}` or retry API verification via GET /v1/packages/<name>."
                        );
                    }
                },
                Err(err) => {
                    eprintln!(
                        "warning: publish verified upload ok, but failed to fetch registry API {}: {err:#}",
                        detail_url.as_str()
                    );
                    eprintln!(
                        "warning: sparse index reads (x07 pkg versions) may be cached; try `x07 pkg versions --refresh {name}` or retry API verification via GET /v1/packages/<name>."
                    );
                }
            }
        }
    }

    let report = PkgReport {
        ok: true,
        command: "pkg.publish",
        result: Some(PublishResult {
            index: args.index,
            package_dir: package_dir.display().to_string(),
            name,
            version,
            cksum,
            index_path,
        }),
        error: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::SUCCESS)
}

fn pack_package_to_tar(package_dir: &Path) -> Result<(project::PackageManifest, Vec<u8>)> {
    let (pkg, _pkg_manifest_path, pkg_manifest_bytes) = project::load_package_manifest(package_dir)
        .with_context(|| format!("load package manifest in {}", package_dir.display()))?;

    let mut entries: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    entries.push((PathBuf::from("x07-package.json"), pkg_manifest_bytes));
    for module_id in &pkg.modules {
        let rel = format!("{}.x07.json", module_id.replace('.', "/"));
        let disk_path = package_dir.join(&pkg.module_root).join(&rel);
        let bytes = std::fs::read(&disk_path)
            .with_context(|| format!("read module {module_id:?}: {}", disk_path.display()))?;
        entries.push((PathBuf::from(&pkg.module_root).join(rel), bytes));
    }

    let ffi_dir = package_dir.join("ffi");
    if ffi_dir.is_dir() {
        let mut pending: Vec<PathBuf> = vec![ffi_dir];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir)
                .with_context(|| format!("read ffi dir: {}", dir.display()))?
            {
                let entry =
                    entry.with_context(|| format!("read ffi dir entry in {}", dir.display()))?;
                let disk_path = entry.path();
                let file_type = entry
                    .file_type()
                    .with_context(|| format!("file_type for {}", disk_path.display()))?;
                if file_type.is_dir() {
                    pending.push(disk_path);
                    continue;
                }
                if !file_type.is_file() {
                    anyhow::bail!("unsupported ffi entry type: {}", disk_path.display());
                }
                let rel = disk_path
                    .strip_prefix(package_dir)
                    .with_context(|| format!("strip prefix: {}", disk_path.display()))?;
                let bytes = std::fs::read(&disk_path)
                    .with_context(|| format!("read ffi file: {}", disk_path.display()))?;
                entries.push((rel.to_path_buf(), bytes));
            }
        }
    }

    let tar = x07_pkg::build_tar_bytes(&entries)?;
    Ok((pkg, tar))
}

fn temp_unpack_dir(base: &Path) -> Result<PathBuf> {
    let tmp_root = base.join(".x07").join("tmp");
    std::fs::create_dir_all(&tmp_root)
        .with_context(|| format!("create tmp root: {}", tmp_root.display()))?;
    let pid = std::process::id();
    for _ in 0..10_000 {
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let p = tmp_root.join(format!("unpack_{pid}_{n}"));
        if std::fs::create_dir(&p).is_ok() {
            return Ok(p);
        }
    }
    anyhow::bail!("failed to create temp dir under {}", tmp_root.display());
}

struct TempUnpackDir {
    path: PathBuf,
    persisted: bool,
}

impl TempUnpackDir {
    fn create(base: &Path) -> Result<Self> {
        Ok(Self {
            path: temp_unpack_dir(base)?,
            persisted: false,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn persist_to(&mut self, dst: &Path) -> Result<()> {
        std::fs::rename(&self.path, dst)?;
        self.persisted = true;
        Ok(())
    }
}

impl Drop for TempUnpackDir {
    fn drop(&mut self) {
        if self.persisted {
            return;
        }
        if let Err(err) = std::fs::remove_dir_all(&self.path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                // Best-effort cleanup; leaking temp dirs is worse than masking cleanup errors.
            }
        }
    }
}
