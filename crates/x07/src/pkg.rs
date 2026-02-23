use std::collections::{BTreeSet, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use serde_json::Value;

use x07_pkg::SparseIndexClient;
use x07_runner_common::os_paths;
use x07c::builtin_modules;
use x07c::project;

use crate::util;

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(crate) const DEFAULT_INDEX_URL: &str = "sparse+https://registry.x07.io/index/";
const PKG_PROVIDES_REPORT_SCHEMA_VERSION: &str = "x07.pkg.provides.report@0.1.0";

fn default_index_url() -> String {
    match std::env::var("X07_PKG_INDEX_URL") {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                DEFAULT_INDEX_URL.to_string()
            } else {
                trimmed.to_string()
            }
        }
        Err(_) => DEFAULT_INDEX_URL.to_string(),
    }
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
    /// Pack a local package directory into a publishable archive.
    Pack(PackArgs),
    /// Resolve project dependencies and write `x07.lock.json`.
    Lock(LockArgs),
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
    #[arg(long, value_name = "URL")]
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
    #[arg(long, value_name = "URL")]
    pub index: Option<String>,

    /// Package name.
    #[arg(value_name = "NAME")]
    pub name: String,
}

#[derive(Debug, Args)]
pub struct VersionsArgs {
    /// Sparse index URL (example: `sparse+https://registry.x07.io/index/`).
    #[arg(long, value_name = "URL")]
    pub index: Option<String>,

    /// Package name.
    #[arg(value_name = "NAME")]
    pub name: String,
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
    #[arg(long, value_name = "URL")]
    pub index: Option<String>,

    /// Fail if `x07.lock.json` is out of date.
    #[arg(long)]
    pub check: bool,

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

#[derive(Debug, Serialize)]
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
        PkgCommand::Pack(args) => cmd_pkg_pack(machine, args),
        PkgCommand::Lock(args) => cmd_pkg_lock(args),
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

            let index = args.index.clone().unwrap_or_else(default_index_url);
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
            offline: false,
            allow_yanked: false,
            allow_advisories: false,
        };
        let index = match lock_args.index.as_deref() {
            Some(index) => index.to_string(),
            None => default_index_url(),
        };
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
    let index = args.index.clone().unwrap_or_else(default_index_url);
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

    let entries = match client.fetch_entries(&name) {
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

    let index = match args.index.as_deref() {
        Some(index) => index.to_string(),
        None => default_index_url(),
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

    let mut updated_specs = apply_patch_overrides_to_project_doc_deps(&mut doc, &manifest.patch)?;

    let mut fetched = Vec::new();
    let mut index_used: Option<String> = None;
    let mut client: Option<SparseIndexClient> = None;

    let mut ctx = TransitiveResolveCtx {
        base,
        args,
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
        args,
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
                index: index_used.clone().or(args.index.clone()),
                lockfile: lock_path.display().to_string(),
                fetched,
            }),
            error: Some(err),
        };
        return Ok((std::process::ExitCode::from(20), report));
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

        let metadata_online = !args.offline && (args.index.is_some() || index_used.is_some());

        if metadata_online {
            if !args.allow_yanked {
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
                            index: index_used.clone().or(args.index.clone()),
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

            if !args.allow_advisories {
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
                            index: index_used.clone().or(args.index.clone()),
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

fn lockfiles_equal_core(a: &project::Lockfile, b: &project::Lockfile) -> bool {
    if a.schema_version.trim() != b.schema_version.trim() {
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
            dep.yanked = *yanked;
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
        .any(|d| d.path.starts_with(".x07/deps/"));
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
        if !dep.path.starts_with(".x07/deps/") {
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
        if cand.is_dir() {
            return Some(cand);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        for anc in exe.ancestors() {
            let cand = anc.join("packages").join("ext");
            if cand.is_dir() {
                return Some(cand);
            }
        }
    }

    None
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
        if !dep_dir.join("x07-package.json").is_file() {
            if ctx.patch.get(&dep.name).is_some_and(|s| s.path.is_some()) {
                missing_local_override.push(format!(
                    "{}@{} ({})",
                    dep.name,
                    dep.version,
                    dep_dir.display()
                ));
            } else if !dep.path.starts_with(".x07/deps/") {
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
        let tmp_dir = temp_unpack_dir(ctx.base)?;
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
                                "warning: sparse index reads (x07 pkg versions) may be cached; retry API verification via GET /v1/packages/<name>."
                            );
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "warning: publish verified upload ok, but failed to parse registry API response at {}: {err}",
                            detail_url.as_str()
                        );
                        eprintln!(
                            "warning: sparse index reads (x07 pkg versions) may be cached; retry API verification via GET /v1/packages/<name>."
                        );
                    }
                },
                Err(err) => {
                    eprintln!(
                        "warning: publish verified upload ok, but failed to fetch registry API {}: {err:#}",
                        detail_url.as_str()
                    );
                    eprintln!(
                        "warning: sparse index reads (x07 pkg versions) may be cached; retry API verification via GET /v1/packages/<name>."
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
