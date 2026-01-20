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
    let mut doc: Value = serde_json::from_slice(&project_bytes)
        .with_context(|| format!("parse project JSON: {}", project_path.display()))?;
    let obj = doc
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("project must be a JSON object"))?;

    let (name, version) = parse_pkg_spec(&args.spec)?;
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

    let mut out = serde_json::to_vec_pretty(&doc)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    std::fs::write(&project_path, &out)
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
        let (lock_code, lock_report) = pkg_lock_report(&lock_args)?;
        add_result.lock = lock_report.result;
        if !lock_report.ok {
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
    let manifest =
        project::load_project_manifest(&project_path).context("load project manifest")?;
    let lock_path = project::default_lockfile_path(&project_path, &manifest);

    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut fetched = Vec::new();
    let mut missing = Vec::new();
    for dep in &manifest.dependencies {
        let dep_dir = base.join(&dep.path);
        if !dep_dir.join("x07-package.json").is_file() {
            missing.push(dep);
        }
    }

    let mut index_used: Option<String> = None;

    if !missing.is_empty() {
        if args.offline {
            let report = PkgReport {
                ok: false,
                command: "pkg.lock",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_OFFLINE_MISSING_DEP".to_string(),
                    message: format!("{} missing dependencies (offline mode)", missing.len()),
                }),
            };
            return Ok((std::process::ExitCode::from(20), report));
        }

        let index = match args.index.as_deref() {
            Some(index) => index.to_string(),
            None => DEFAULT_INDEX_URL.to_string(),
        };
        index_used = Some(index.clone());

        let token = x07_pkg::load_token(&index).unwrap_or(None);
        let client = SparseIndexClient::from_index_url(&index, token)?;

        for dep in missing {
            let dep_dir = base.join(&dep.path);
            let entries = client
                .fetch_entries(&dep.name)
                .with_context(|| format!("fetch index entries for {:?}", dep.name))?;
            let Some(entry) = entries
                .into_iter()
                .find(|e| e.name == dep.name && e.version == dep.version && !e.yanked)
            else {
                let report = PkgReport::<LockResult> {
                    ok: false,
                    command: "pkg.lock",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_INDEX_NO_MATCH".to_string(),
                        message: format!(
                            "no non-yanked index entry for {:?}@{:?}",
                            dep.name, dep.version
                        ),
                    }),
                };
                return Ok((std::process::ExitCode::from(20), report));
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
            } else {
                client.download_to_file(&dep.name, &dep.version, &entry.cksum, &archive_path)?;
            }

            let archive_bytes = std::fs::read(&archive_path)
                .with_context(|| format!("read archive for {:?}@{:?}", dep.name, dep.version))?;
            let tmp_dir = temp_unpack_dir(base)?;
            x07_pkg::unpack_tar_bytes(&archive_bytes, &tmp_dir)?;

            let (pkg, _pkg_manifest_path, _pkg_manifest_bytes) =
                project::load_package_manifest(&tmp_dir).with_context(|| {
                    format!("validate unpacked package at {}", tmp_dir.display())
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
    }

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
