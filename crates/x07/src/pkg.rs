use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use x07_pkg::SparseIndexClient;
use x07c::project;

use crate::util;

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Args)]
pub struct PkgArgs {
    #[command(subcommand)]
    pub cmd: Option<PkgCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum PkgCommand {
    Pack(PackArgs),
    Lock(LockArgs),
    Login(LoginArgs),
    Publish(PublishArgs),
}

#[derive(Debug, Args)]
pub struct PackArgs {
    #[arg(long, value_name = "DIR")]
    pub package: PathBuf,

    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,
}

#[derive(Debug, Args)]
pub struct LockArgs {
    #[arg(long, value_name = "PATH", default_value = "x07.json")]
    pub project: PathBuf,

    #[arg(long, value_name = "URL")]
    pub index: Option<String>,

    #[arg(long)]
    pub check: bool,

    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    #[arg(long, value_name = "URL")]
    pub index: String,

    #[arg(long, value_name = "TOKEN")]
    pub token: String,
}

#[derive(Debug, Args)]
pub struct PublishArgs {
    #[arg(long, value_name = "DIR")]
    pub package: PathBuf,

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
        PkgCommand::Pack(args) => cmd_pkg_pack(args),
        PkgCommand::Lock(args) => cmd_pkg_lock(args),
        PkgCommand::Login(args) => cmd_pkg_login(args),
        PkgCommand::Publish(args) => cmd_pkg_publish(args),
    }
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
    let project_path = util::resolve_existing_path_upwards(&args.project);
    let manifest =
        project::load_project_manifest(&project_path).context("load project manifest")?;
    let lock_path = project::default_lockfile_path(&project_path, &manifest);

    let base = project_path.parent().unwrap_or_else(|| Path::new("."));

    let mut fetched = Vec::new();
    let mut missing = Vec::new();
    for dep in &manifest.dependencies {
        let dep_dir = base.join(&dep.path);
        if !dep_dir.join("x07-package.json").is_file() {
            missing.push(dep);
        }
    }

    if !missing.is_empty() {
        if args.offline {
            let report = PkgReport::<LockResult> {
                ok: false,
                command: "pkg.lock",
                result: None,
                error: Some(PkgError {
                    code: "X07PKG_OFFLINE_MISSING_DEP".to_string(),
                    message: format!("{} missing dependencies (offline mode)", missing.len()),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }

        let index = match args.index.as_deref() {
            Some(index) => index,
            None => {
                let report = PkgReport::<LockResult> {
                    ok: false,
                    command: "pkg.lock",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_INDEX_REQUIRED".to_string(),
                        message: "--index is required when dependencies need fetching".to_string(),
                    }),
                };
                println!("{}", serde_json::to_string(&report)?);
                return Ok(std::process::ExitCode::from(20));
            }
        };

        let token = x07_pkg::load_token(index).unwrap_or(None);
        let client = SparseIndexClient::from_index_url(index, token)?;

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
                println!("{}", serde_json::to_string(&report)?);
                return Ok(std::process::ExitCode::from(20));
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
                let report = PkgReport::<LockResult> {
                    ok: false,
                    command: "pkg.lock",
                    result: None,
                    error: Some(PkgError {
                        code: "X07PKG_LOCK_MISSING".to_string(),
                        message: format!("missing lockfile: {}", lock_path.display()),
                    }),
                };
                println!("{}", serde_json::to_string(&report)?);
                return Ok(std::process::ExitCode::from(20));
            }
            Err(err) => {
                return Err(err).with_context(|| format!("read lockfile: {}", lock_path.display()))
            }
        };
        let existing: project::Lockfile = serde_json::from_slice(&existing_bytes)
            .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;
        if existing != lock {
            let report = PkgReport::<LockResult> {
                ok: false,
                command: "pkg.lock",
                result: Some(LockResult {
                    project: project_path.display().to_string(),
                    index: args.index.clone(),
                    lockfile: lock_path.display().to_string(),
                    fetched,
                }),
                error: Some(PkgError {
                    code: "X07PKG_LOCK_MISMATCH".to_string(),
                    message: format!("{} would change", lock_path.display()),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }

        let report = PkgReport {
            ok: true,
            command: "pkg.lock",
            result: Some(LockResult {
                project: project_path.display().to_string(),
                index: args.index.clone(),
                lockfile: lock_path.display().to_string(),
                fetched,
            }),
            error: None,
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::SUCCESS);
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
            index: args.index.clone(),
            lockfile: lock_path.display().to_string(),
            fetched,
        }),
        error: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_pkg_login(args: LoginArgs) -> Result<std::process::ExitCode> {
    if let Err(err) = x07_pkg::store_token(&args.index, &args.token) {
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
