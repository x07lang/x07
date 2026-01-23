use std::collections::{BTreeSet, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use serde_json::Value;

use x07_pkg::SparseIndexClient;
use x07c::project;

use crate::util;

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

const DEFAULT_INDEX_URL: &str = "sparse+https://registry.x07.io/index/";

#[derive(Debug, Args)]
pub struct PkgArgs {
    #[command(subcommand)]
    pub cmd: Option<PkgCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum PkgCommand {
    /// Add a dependency entry to `x07.json`.
    Add(AddArgs),
    /// Pack a local package directory into a publishable archive.
    Pack(PackArgs),
    /// Resolve project dependencies and write `x07.lock.json`.
    Lock(LockArgs),
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
    #[arg(long, value_name = "URL")]
    pub index: Option<String>,

    /// Override the dependency path stored in `x07.json`.
    #[arg(long, value_name = "PATH")]
    pub path: Option<String>,

    /// Package spec in `NAME@VERSION` form.
    #[arg(value_name = "NAME@VERSION")]
    pub spec: String,
}

#[derive(Debug, Args)]
pub struct PackArgs {
    /// Package directory containing `x07-package.json`.
    #[arg(long, value_name = "DIR")]
    pub package: PathBuf,

    /// Output archive path.
    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,
}

#[derive(Debug, Args)]
pub struct LockArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH", default_value = "x07.json")]
    pub project: PathBuf,

    /// Sparse index URL (example: `sparse+https://registry.x07.io/index/`).
    #[arg(long, value_name = "URL")]
    pub index: Option<String>,

    /// Fail if `x07.lock.json` is out of date.
    #[arg(long)]
    pub check: bool,

    /// Disallow network access and reuse existing `.x07/deps` contents.
    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Index base URL.
    #[arg(long, value_name = "URL")]
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
    #[arg(long, value_name = "URL")]
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

#[derive(Debug, Serialize)]
struct FetchedDep {
    name: String,
    version: String,
    path: String,
    sha256: String,
}

pub fn cmd_pkg(args: PkgArgs) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing pkg subcommand (try --help)");
    };

    match cmd {
        PkgCommand::Add(args) => cmd_pkg_add(args),
        PkgCommand::Pack(args) => cmd_pkg_pack(args),
        PkgCommand::Lock(args) => cmd_pkg_lock(args),
        PkgCommand::Login(args) => cmd_pkg_login(args),
        PkgCommand::Publish(args) => cmd_pkg_publish(args),
    }
}

fn cmd_pkg_add(args: AddArgs) -> Result<std::process::ExitCode> {
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

    let (name, version) = match parse_pkg_spec(&args.spec) {
        Ok(out) => out,
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
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };
    let dep_path = args
        .path
        .unwrap_or_else(|| format!(".x07/deps/{name}/{version}"));

    let deps_val = obj
        .entry("dependencies".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let deps = deps_val
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("project.dependencies must be an array"))?;

    for dep in deps.iter() {
        if dep.get("name").and_then(Value::as_str) == Some(name.as_str()) {
            let report = PkgReport::<AddResult> {
                ok: false,
                command: "pkg.add",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_DEP_EXISTS".to_string(),
                    message: format!("dependency already exists: {name}"),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    }

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
    };

    if args.sync {
        let lock_args = LockArgs {
            project: project_path.clone(),
            index: args.index.clone(),
            check: false,
            offline: false,
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
            println!("{}", serde_json::to_string(&report)?);
            return Ok(lock_code);
        }
    }

    let report = PkgReport {
        ok: true,
        command: "pkg.add",
        result: Some(add_result),
        error: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::SUCCESS)
}

fn parse_pkg_spec(spec: &str) -> Result<(String, String)> {
    let spec = spec.trim();
    let Some((name, version)) = spec.split_once('@') else {
        anyhow::bail!("expected NAME@VERSION, got {:?}", spec);
    };
    let name = name.trim();
    let version = version.trim();
    if name.is_empty() {
        anyhow::bail!("package name must be non-empty");
    }
    if version.is_empty() {
        anyhow::bail!("package version must be non-empty");
    }
    if !is_valid_semver_version(version) {
        anyhow::bail!(
            "package version must be semver (MAJOR.MINOR.PATCH), got {:?}",
            version
        );
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
    Ok((name.to_string(), version.to_string()))
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

fn cmd_pkg_pack(args: PackArgs) -> Result<std::process::ExitCode> {
    let package_dir = util::resolve_existing_path_upwards(&args.package);

    let (_pkg, tar) = pack_package_to_tar(&package_dir)?;
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output dir: {}", parent.display()))?;
    }
    std::fs::write(&args.out, &tar).with_context(|| format!("write: {}", args.out.display()))?;

    let report = PkgReport {
        ok: true,
        command: "pkg.pack",
        result: Some(PackResult {
            package_dir: package_dir.display().to_string(),
            out: args.out.display().to_string(),
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

    // Validate using the canonical parser for better error messages and stricter checks.
    let mut manifest =
        project::load_project_manifest(&project_path).context("load project manifest")?;

    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut fetched = Vec::new();
    let mut index_used: Option<String> = None;

    let transitive = match resolve_transitive_deps(
        &mut doc,
        &project_path,
        &manifest,
        base,
        args,
        &mut fetched,
        &mut index_used,
    )? {
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
    if transitive.changed {
        if args.check {
            let report = PkgReport {
                ok: false,
                command: "pkg.lock",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_TRANSITIVE_MISSING".to_string(),
                    message: format!(
                        "project is missing transitive dependencies (run `x07 pkg lock` to update x07.json): {}",
                        transitive.added_specs.join(", ")
                    ),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }
        write_canonical_json_file(&project_path, &doc)
            .with_context(|| format!("write: {}", project_path.display()))?;
        manifest = project::load_project_manifest(&project_path).context("reload project")?;
    }

    let lock_path = project::default_lockfile_path(&project_path, &manifest);

    let lock = project::compute_lockfile(&project_path, &manifest)?;

    if args.check {
        let existing_bytes = match std::fs::read(&lock_path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
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
            Err(err) => {
                return Err(err).with_context(|| format!("read lockfile: {}", lock_path.display()))
            }
        };
        let existing: project::Lockfile = serde_json::from_slice(&existing_bytes)
            .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;
        if existing != lock {
            let report = PkgReport {
                ok: false,
                command: "pkg.lock",
                result: Some(LockResult {
                    project: project_path.display().to_string(),
                    index: index_used.clone().or(args.index.clone()),
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
                index: index_used.clone().or(args.index.clone()),
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
            index: index_used.or(args.index.clone()),
            lockfile: lock_path.display().to_string(),
            fetched,
        }),
        error: None,
    };
    Ok((std::process::ExitCode::SUCCESS, report))
}

#[derive(Debug, Clone)]
struct TransitiveResolution {
    changed: bool,
    added_specs: Vec<String>,
}

enum TransitiveResolutionOutcome {
    Ok(TransitiveResolution),
    Error(PkgError),
}

fn resolve_transitive_deps(
    doc: &mut Value,
    project_path: &Path,
    _manifest: &project::ProjectManifest,
    base: &Path,
    args: &LockArgs,
    fetched: &mut Vec<FetchedDep>,
    index_used: &mut Option<String>,
) -> Result<TransitiveResolutionOutcome> {
    let mut scanned: HashSet<(String, String)> = HashSet::new();
    let mut added: BTreeSet<String> = BTreeSet::new();
    let mut changed = false;

    let index = match args.index.as_deref() {
        Some(index) => index.to_string(),
        None => DEFAULT_INDEX_URL.to_string(),
    };

    let mut client: Option<SparseIndexClient> = None;

    loop {
        let deps = deps_from_project_doc(doc, project_path)?;
        if let Some(err) =
            ensure_deps_present(&deps, base, args, &index, &mut client, fetched, index_used)?
        {
            return Ok(TransitiveResolutionOutcome::Error(err));
        }

        let mut round_added = false;
        for dep in deps {
            let key = (dep.name.clone(), dep.version.clone());
            if !scanned.insert(key) {
                continue;
            }
            let dep_dir = base.join(&dep.path);
            let reqs = requires_packages_from_manifest(&dep_dir)?;
            for spec in reqs {
                let (name, version) = parse_pkg_spec(&spec)?;
                let path = format!(".x07/deps/{name}/{version}");
                match ensure_dep_entry(doc, &name, &version, &path)? {
                    EnsureDepOutcome::Added => {
                        changed = true;
                        round_added = true;
                        added.insert(format!("{name}@{version}"));
                    }
                    EnsureDepOutcome::AlreadyPresentSameVersion => {}
                    EnsureDepOutcome::AlreadyPresentDifferentVersion { existing_version } => {
                        anyhow::bail!(
                            "dependency version conflict: project has {name}@{existing_version}, but {spec:?} is required by a dependency"
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
    }))
}

#[derive(Debug, Clone)]
enum EnsureDepOutcome {
    Added,
    AlreadyPresentSameVersion,
    AlreadyPresentDifferentVersion { existing_version: String },
}

fn ensure_dep_entry(
    doc: &mut Value,
    name: &str,
    version: &str,
    path: &str,
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

    for dep in deps.iter() {
        let dep_name = dep.get("name").and_then(Value::as_str).unwrap_or("");
        if dep_name != name {
            continue;
        }
        let dep_ver = dep.get("version").and_then(Value::as_str).unwrap_or("");
        if dep_ver == version {
            return Ok(EnsureDepOutcome::AlreadyPresentSameVersion);
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

fn ensure_deps_present(
    deps: &[project::DependencySpec],
    base: &Path,
    args: &LockArgs,
    index: &str,
    client: &mut Option<SparseIndexClient>,
    fetched: &mut Vec<FetchedDep>,
    index_used: &mut Option<String>,
) -> Result<Option<PkgError>> {
    let mut missing: Vec<&project::DependencySpec> = Vec::new();
    for dep in deps {
        let dep_dir = base.join(&dep.path);
        if !dep_dir.join("x07-package.json").is_file() {
            missing.push(dep);
        }
    }

    if missing.is_empty() {
        return Ok(None);
    }

    if args.offline {
        return Ok(Some(PkgError {
            code: "X07PKG_OFFLINE_MISSING_DEP".to_string(),
            message: format!("{} missing dependencies (offline mode)", missing.len()),
        }));
    }

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
    let client = client.as_ref().expect("client initialized");
    if index_used.is_none() {
        *index_used = Some(index.to_string());
    }

    for dep in missing {
        let dep_dir = base.join(&dep.path);
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
            .find(|e| e.name == dep.name && e.version == dep.version && !e.yanked)
        else {
            return Ok(Some(PkgError {
                code: "X07PKG_INDEX_NO_MATCH".to_string(),
                message: format!(
                    "no non-yanked index entry for {:?}@{:?}",
                    dep.name, dep.version
                ),
            }));
        };

        let cache_dir = base.join(".x07").join("cache").join("sha256");
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
        let tmp_dir = temp_unpack_dir(base)?;
        x07_pkg::unpack_tar_bytes(&archive_bytes, &tmp_dir)?;

        let (pkg, _pkg_manifest_path, _pkg_manifest_bytes) =
            project::load_package_manifest(&tmp_dir)
                .with_context(|| format!("validate unpacked package at {}", tmp_dir.display()))?;
        if pkg.name != dep.name || pkg.version != dep.version {
            anyhow::bail!(
                "unpacked package identity mismatch: expected {:?}@{:?} got {:?}@{:?}",
                dep.name,
                dep.version,
                pkg.name,
                pkg.version
            );
        }

        if dep_dir.exists() {
            std::fs::remove_dir_all(&dep_dir)
                .with_context(|| format!("remove existing dep dir: {}", dep_dir.display()))?;
        }
        if let Some(parent) = dep_dir.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dep parent: {}", parent.display()))?;
        }
        std::fs::rename(&tmp_dir, &dep_dir).with_context(|| {
            format!(
                "move unpacked package into place: {} -> {}",
                tmp_dir.display(),
                dep_dir.display()
            )
        })?;

        fetched.push(FetchedDep {
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
