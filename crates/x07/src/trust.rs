use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::{json, Value};
use x07_contracts::{PROJECT_LOCKFILE_SCHEMA_VERSION, X07_TRUST_REPORT_SCHEMA_VERSION};
use x07_worlds::WorldId;
use x07c::diagnostics;
use x07c::project;

use crate::policy_overrides::{PolicyOverrides, PolicyResolution};
use crate::report_common;
use crate::run;
use crate::util;

const X07_TRUST_REPORT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-trust.report.schema.json");

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
    /// Emit a CI trust report artifact (budgets/caps, capabilities, nondeterminism, SBOM placeholders).
    Report(TrustReportArgs),
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

    /// If set: missing policy/lock/schema mismatch becomes a hard error.
    #[arg(long)]
    pub strict: bool,

    /// CI gating: fail if any matching condition is true.
    #[arg(long, value_enum)]
    pub fail_on: Vec<TrustFailOn>,
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

pub fn cmd_trust(
    machine: &crate::reporting::MachineArgs,
    args: TrustArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing trust subcommand (try --help)");
    };

    match cmd {
        TrustCommand::Report(args) => cmd_trust_report(machine, args),
    }
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
        sbom: Sbom {
            format: "none".to_string(),
            generated: false,
            path: None,
            cyclonedx: None,
            spdx: None,
            components: sbom_components,
        },
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

    let json_bytes = report_common::canonical_pretty_json_bytes(&report_value)?;
    util::write_atomic(out_path, &json_bytes)
        .with_context(|| format!("write trust report: {}", out_path.display()))?;

    if let Some(html_out) = &args.html_out {
        let html = render_trust_html(&report, &strict_issues, &schema_diags);
        util::write_atomic(html_out, html.as_bytes())
            .with_context(|| format!("write trust html: {}", html_out.display()))?;
    }

    if !schema_diags.is_empty() || fail_on_triggered || (args.strict && !strict_issues.is_empty()) {
        for issue in &strict_issues {
            eprintln!("x07 trust: {issue}");
        }
        for diag in &schema_diags {
            eprintln!("{}: {}", diag.code, diag.message);
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
            if lock.schema_version != PROJECT_LOCKFILE_SCHEMA_VERSION {
                anyhow::bail!(
                    "lockfile schema_version mismatch: expected {} got {:?}",
                    PROJECT_LOCKFILE_SCHEMA_VERSION,
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
                if report.sbom.format == "none" || report.sbom.components.is_empty() {
                    return true;
                }
            }
        }
    }
    false
}

fn render_trust_html(
    report: &TrustReport,
    strict_issues: &[String],
    schema_diags: &[diagnostics::Diagnostic],
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

    s.push_str("<h2>SBOM Placeholder</h2><pre>");
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
