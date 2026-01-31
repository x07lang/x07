use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use globset::{Glob, GlobSet, GlobSetBuilder};
use jsonschema::Draft;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;
use x07_contracts::{
    X07_ARCH_MANIFEST_LOCK_SCHEMA_VERSION, X07_ARCH_MANIFEST_SCHEMA_VERSION,
    X07_ARCH_PATCHSET_SCHEMA_VERSION, X07_ARCH_REPORT_SCHEMA_VERSION,
};
use x07_worlds::WorldId;
use x07c::diagnostics;
use x07c::json_patch;

use crate::util;

const X07_ARCH_MANIFEST_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-arch.manifest.schema.json");
const X07_ARCH_MANIFEST_LOCK_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-arch.manifest.lock.schema.json");

const DEFAULT_MODULE_SCAN_INCLUDE: &[&str] = &["**/*.x07.json"];
const DEFAULT_MODULE_SCAN_EXCLUDE: &[&str] = &[
    "**/.git/**",
    "**/dist/**",
    "**/gen/**",
    "**/node_modules/**",
    "**/target/**",
    "**/tmp/**",
];

#[derive(Debug, Args)]
pub struct ArchArgs {
    #[command(subcommand)]
    pub cmd: Option<ArchCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum ArchCommand {
    /// Check repo architecture against an architecture manifest.
    Check(ArchCheckArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum ArchFormat {
    Json,
    Text,
}

#[derive(Debug, Args)]
pub struct ArchCheckArgs {
    #[arg(
        long,
        value_name = "PATH",
        default_value = "arch/manifest.x07arch.json"
    )]
    pub manifest: PathBuf,

    /// Optional manifest lock file.
    ///
    /// If omitted, `arch/manifest.lock.json` is used only when it exists under --repo-root.
    #[arg(long, value_name = "PATH")]
    pub lock: Option<PathBuf>,

    /// Update the lock file deterministically.
    #[arg(long)]
    pub write_lock: bool,

    /// Repository root directory.
    #[arg(long, value_name = "DIR", default_value = ".")]
    pub repo_root: PathBuf,

    #[arg(long, value_enum, default_value_t = ArchFormat::Json)]
    pub format: ArchFormat,

    /// Write the report to a file (stdout when omitted).
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,

    /// Emit suggested patches (multi-file JSON Patch set).
    #[arg(long, value_name = "PATH")]
    pub emit_patch: Option<PathBuf>,

    /// Apply suggested patches.
    #[arg(long)]
    pub write: bool,

    #[arg(long, value_name = "N")]
    pub max_modules: Option<usize>,

    #[arg(long, value_name = "N")]
    pub max_edges: Option<usize>,

    #[arg(long, value_name = "N")]
    pub max_diags: Option<usize>,

    #[arg(long, value_name = "N")]
    pub max_bytes_in: Option<u64>,
}

pub fn cmd_arch(args: ArchArgs) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing arch subcommand (try --help)");
    };
    match cmd {
        ArchCommand::Check(args) => cmd_arch_check(args),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchManifest {
    schema_version: String,
    repo: ArchRepo,
    externals: ArchExternals,
    nodes: Vec<ArchNode>,
    #[serde(default)]
    rules: Vec<ArchRule>,
    checks: ArchChecks,
    tool_budgets: ArchToolBudgets,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchRepo {
    id: String,
    root: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchExternals {
    #[serde(default)]
    allowed_import_prefixes: Vec<String>,
    #[serde(default)]
    allowed_exact: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchNode {
    id: String,
    #[serde(rename = "match")]
    matcher: ArchNodeMatch,
    world: String,
    visibility: ArchNodeVisibility,
    imports: ArchNodeImports,
    #[serde(default)]
    contracts: Option<ArchNodeContracts>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchNodeMatch {
    #[serde(default)]
    module_prefixes: Vec<String>,
    #[serde(default)]
    path_globs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchNodeVisibility {
    mode: String,
    #[serde(default)]
    visible_to: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchNodeImports {
    #[serde(default)]
    deny_prefixes: Vec<String>,
    #[serde(default)]
    allow_prefixes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchNodeContracts {
    smoke_entry: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "kind")]
enum ArchRule {
    #[serde(rename = "deps_v1")]
    DepsV1 {
        id: String,
        from: String,
        to: Vec<String>,
        mode: String,
    },
    #[serde(rename = "layers_v1")]
    LayersV1 {
        id: String,
        layers: Vec<String>,
        direction: String,
    },
    #[serde(rename = "deny_cycles_v1")]
    DenyCyclesV1 { id: String, scope: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchChecks {
    deny_cycles: bool,
    deny_orphans: bool,
    enforce_visibility: bool,
    enforce_world_caps: bool,
    #[serde(default)]
    allowlist_mode: Option<ArchAllowlistMode>,
    #[serde(default)]
    brand_boundary_v1: Option<ArchCheckEnabled>,
    #[serde(default)]
    world_of_imported_v1: Option<ArchCheckEnabled>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchAllowlistMode {
    enabled: bool,
    default_allow_external: bool,
    default_allow_internal: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchCheckEnabled {
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchToolBudgets {
    max_modules: usize,
    max_edges: usize,
    max_diags: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArchManifestLock {
    schema_version: String,
    manifest_path: String,
    jcs_sha256_hex: String,
    module_scan: ArchModuleScan,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArchModuleScan {
    include_globs: Vec<String>,
    exclude_globs: Vec<String>,
}

#[derive(Debug, Clone)]
struct ArchBudgets {
    max_modules: usize,
    max_edges: usize,
    max_diags: usize,
    max_bytes_in: Option<u64>,
}

#[derive(Debug, Clone)]
struct ModuleInfo {
    rel_path: String,
    imports: Vec<String>,
    parsed: x07c::x07ast::X07AstFile,
}

#[derive(Debug, Clone)]
struct NodeMatcher {
    id: String,
    module_prefixes: Vec<String>,
    path_globs: GlobSet,
    world: WorldId,
    visibility: ArchNodeVisibility,
    imports: ArchNodeImports,
    smoke_entry: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ArchReport {
    schema_version: &'static str,
    manifest: ArchReportManifest,
    stats: ArchReportStats,
    diags: Vec<diagnostics::Diagnostic>,
    suggested_patches: Vec<ArchPatchTarget>,
}

#[derive(Debug, Clone, Serialize)]
struct ArchReportManifest {
    path: String,
    jcs_sha256_hex: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ArchReportStats {
    modules: usize,
    nodes: usize,
    module_edges: usize,
    node_edges: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ArchPatchTarget {
    path: String,
    patch: Vec<diagnostics::PatchOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ArchPatchSet {
    schema_version: &'static str,
    patches: Vec<ArchPatchTarget>,
}

#[derive(Debug, Clone)]
struct EdgeEvidence {
    module_path: String,
    module_id: String,
    import: String,
}

struct DiagSink {
    max_diags: usize,
    diags: Vec<diagnostics::Diagnostic>,
    diags_overflowed: bool,
    tool_budget_exceeded: bool,
}

impl DiagSink {
    fn new(max_diags: usize) -> Self {
        Self {
            max_diags,
            diags: Vec::new(),
            diags_overflowed: false,
            tool_budget_exceeded: false,
        }
    }

    fn push(&mut self, diag: diagnostics::Diagnostic) {
        if self.diags.len() < self.max_diags {
            self.diags.push(diag);
            return;
        }
        self.diags_overflowed = true;
    }

    fn budget_exceeded(&mut self, message: &str, data: BTreeMap<String, Value>) {
        self.tool_budget_exceeded = true;
        let budget_diag = diagnostics::Diagnostic {
            code: "E_ARCH_TOOL_BUDGET_EXCEEDED".to_string(),
            severity: diagnostics::Severity::Error,
            stage: diagnostics::Stage::Lint,
            message: message.to_string(),
            loc: None,
            notes: Vec::new(),
            related: Vec::new(),
            data,
            quickfix: None,
        };

        if self.max_diags == 0 {
            return;
        }
        if self.diags.len() < self.max_diags {
            self.diags.push(budget_diag);
        } else if let Some(last) = self.diags.last_mut() {
            *last = budget_diag;
        }
    }
}

fn cmd_arch_check(args: ArchCheckArgs) -> Result<std::process::ExitCode> {
    if args.out.as_ref().is_some_and(|p| p.as_os_str() == "-") {
        anyhow::bail!("--out '-' is not supported (stdout is reserved for the report)");
    }
    if args
        .emit_patch
        .as_ref()
        .is_some_and(|p| p.as_os_str() == "-")
    {
        anyhow::bail!("--emit-patch '-' is not supported");
    }

    let repo_root = util::resolve_existing_path_upwards(&args.repo_root);
    if !repo_root.is_dir() {
        anyhow::bail!("repo root is not a directory: {}", repo_root.display());
    }

    let manifest_path = resolve_path_under_root(&repo_root, &args.manifest);
    let mut lock_path = resolve_lock_path(&repo_root, args.lock.as_ref());
    if args.write_lock && lock_path.is_none() {
        lock_path = Some(repo_root.join("arch/manifest.lock.json"));
    }

    if args.write {
        let first = arch_check_once(
            &repo_root,
            &manifest_path,
            lock_path.as_deref(),
            &args,
            args.write_lock,
        )?;

        if let Some(path) = &args.emit_patch {
            write_patchset(&repo_root, path, &first.suggested_patches)?;
        }

        if !first.suggested_patches.is_empty() {
            apply_patchset(&repo_root, &first.suggested_patches)?;
        }

        let final_out = arch_check_once(
            &repo_root,
            &manifest_path,
            lock_path.as_deref(),
            &args,
            args.write_lock,
        )?;
        emit_report(&args, &final_out.report)?;
        return Ok(final_out.exit_code);
    }

    let out = arch_check_once(
        &repo_root,
        &manifest_path,
        lock_path.as_deref(),
        &args,
        args.write_lock,
    )?;
    if let Some(path) = &args.emit_patch {
        write_patchset(&repo_root, path, &out.suggested_patches)?;
    }
    emit_report(&args, &out.report)?;
    Ok(out.exit_code)
}

struct ArchCheckOutcome {
    report: ArchReport,
    suggested_patches: Vec<ArchPatchTarget>,
    exit_code: std::process::ExitCode,
}

fn arch_check_once(
    repo_root: &Path,
    manifest_path: &Path,
    lock_path: Option<&Path>,
    args: &ArchCheckArgs,
    write_lock: bool,
) -> Result<ArchCheckOutcome> {
    let manifest_path_s = display_relpath(repo_root, manifest_path);

    let mut bytes_in_total: u64 = 0;

    let manifest_bytes = match std::fs::read(manifest_path) {
        Ok(b) => b,
        Err(err) => {
            let report = ArchReport {
                schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                manifest: ArchReportManifest {
                    path: manifest_path_s.clone(),
                    jcs_sha256_hex: None,
                },
                stats: ArchReportStats {
                    modules: 0,
                    nodes: 0,
                    module_edges: 0,
                    node_edges: 0,
                },
                diags: vec![diag_parse_error(
                    "E_ARCH_MANIFEST_READ",
                    &format!("read manifest: {err}"),
                    Some(&manifest_path_s),
                )],
                suggested_patches: Vec::new(),
            };
            return Ok(ArchCheckOutcome {
                suggested_patches: Vec::new(),
                report,
                exit_code: std::process::ExitCode::from(3),
            });
        }
    };
    bytes_in_total = bytes_in_total.saturating_add(manifest_bytes.len() as u64);
    if let Some(max) = args.max_bytes_in {
        if bytes_in_total > max {
            let report = ArchReport {
                schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                manifest: ArchReportManifest {
                    path: manifest_path_s.clone(),
                    jcs_sha256_hex: None,
                },
                stats: ArchReportStats {
                    modules: 0,
                    nodes: 0,
                    module_edges: 0,
                    node_edges: 0,
                },
                diags: vec![diag_budget_exceeded(
                    &format!(
                        "max_bytes_in exceeded while reading manifest ({bytes_in_total} > {max})"
                    ),
                    "max_bytes_in",
                )],
                suggested_patches: Vec::new(),
            };
            return Ok(ArchCheckOutcome {
                suggested_patches: Vec::new(),
                report,
                exit_code: std::process::ExitCode::from(4),
            });
        }
    }

    let manifest_value: Value = match serde_json::from_slice(&manifest_bytes) {
        Ok(v) => v,
        Err(err) => {
            let report = ArchReport {
                schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                manifest: ArchReportManifest {
                    path: manifest_path_s.clone(),
                    jcs_sha256_hex: None,
                },
                stats: ArchReportStats {
                    modules: 0,
                    nodes: 0,
                    module_edges: 0,
                    node_edges: 0,
                },
                diags: vec![diag_parse_error(
                    "E_ARCH_MANIFEST_JSON_PARSE",
                    &format!("parse manifest JSON: {err}"),
                    Some(&manifest_path_s),
                )],
                suggested_patches: Vec::new(),
            };
            return Ok(ArchCheckOutcome {
                suggested_patches: Vec::new(),
                report,
                exit_code: std::process::ExitCode::from(3),
            });
        }
    };

    let manifest_jcs_sha256_hex = util::sha256_hex(&canonical_jcs_bytes(&manifest_value)?);

    let schema_diags = validate_schema(
        "E_ARCH_MANIFEST_SCHEMA_INVALID",
        X07_ARCH_MANIFEST_SCHEMA_BYTES,
        &manifest_value,
    )?;
    if !schema_diags.is_empty() {
        let mut diags = schema_diags;
        sort_diags(&mut diags);
        let report = ArchReport {
            schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
            manifest: ArchReportManifest {
                path: manifest_path_s.clone(),
                jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
            },
            stats: ArchReportStats {
                modules: 0,
                nodes: 0,
                module_edges: 0,
                node_edges: 0,
            },
            diags,
            suggested_patches: Vec::new(),
        };
        return Ok(ArchCheckOutcome {
            suggested_patches: Vec::new(),
            report,
            exit_code: std::process::ExitCode::from(3),
        });
    }

    let manifest: ArchManifest = match serde_json::from_value(manifest_value.clone()) {
        Ok(v) => v,
        Err(err) => {
            let report = ArchReport {
                schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                manifest: ArchReportManifest {
                    path: manifest_path_s.clone(),
                    jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                },
                stats: ArchReportStats {
                    modules: 0,
                    nodes: 0,
                    module_edges: 0,
                    node_edges: 0,
                },
                diags: vec![diag_parse_error(
                    "E_ARCH_MANIFEST_INVALID",
                    &format!("parse manifest: {err}"),
                    Some(&manifest_path_s),
                )],
                suggested_patches: Vec::new(),
            };
            return Ok(ArchCheckOutcome {
                suggested_patches: Vec::new(),
                report,
                exit_code: std::process::ExitCode::from(3),
            });
        }
    };
    if manifest.schema_version != X07_ARCH_MANIFEST_SCHEMA_VERSION {
        anyhow::bail!(
            "internal error: manifest schema_version mismatch (expected {} got {:?})",
            X07_ARCH_MANIFEST_SCHEMA_VERSION,
            manifest.schema_version
        );
    }
    let _ = manifest.repo.id.as_str();
    let _ = manifest.repo.root.as_str();

    let mut budgets = ArchBudgets {
        max_modules: manifest.tool_budgets.max_modules,
        max_edges: manifest.tool_budgets.max_edges,
        max_diags: manifest.tool_budgets.max_diags,
        max_bytes_in: args.max_bytes_in,
    };
    if let Some(v) = args.max_modules {
        budgets.max_modules = v;
    }
    if let Some(v) = args.max_edges {
        budgets.max_edges = v;
    }
    if let Some(v) = args.max_diags {
        budgets.max_diags = v;
    }

    let lock = if let Some(lock_path) = lock_path {
        let lock_path_s = display_relpath(repo_root, lock_path);
        let lock_bytes = match std::fs::read(lock_path) {
            Ok(b) => Some(b),
            Err(err) => {
                if write_lock && err.kind() == std::io::ErrorKind::NotFound {
                    None
                } else {
                    let report = ArchReport {
                        schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                        manifest: ArchReportManifest {
                            path: manifest_path_s.clone(),
                            jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                        },
                        stats: ArchReportStats {
                            modules: 0,
                            nodes: manifest.nodes.len(),
                            module_edges: 0,
                            node_edges: 0,
                        },
                        diags: vec![diag_parse_error(
                            "E_ARCH_LOCK_READ",
                            &format!("read lock: {err}"),
                            Some(&lock_path_s),
                        )],
                        suggested_patches: Vec::new(),
                    };
                    return Ok(ArchCheckOutcome {
                        suggested_patches: Vec::new(),
                        report,
                        exit_code: std::process::ExitCode::from(3),
                    });
                }
            }
        };

        match lock_bytes {
            None => None,
            Some(lock_bytes) => {
                bytes_in_total = bytes_in_total.saturating_add(lock_bytes.len() as u64);
                if let Some(max) = budgets.max_bytes_in {
                    if bytes_in_total > max {
                        let report = ArchReport {
                            schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                            manifest: ArchReportManifest {
                                path: manifest_path_s.clone(),
                                jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                            },
                            stats: ArchReportStats {
                                modules: 0,
                                nodes: manifest.nodes.len(),
                                module_edges: 0,
                                node_edges: 0,
                            },
                            diags: vec![diag_budget_exceeded(
                                &format!(
                                    "max_bytes_in exceeded while reading lock ({bytes_in_total} > {max})"
                                ),
                                "max_bytes_in",
                            )],
                            suggested_patches: Vec::new(),
                        };
                        return Ok(ArchCheckOutcome {
                            suggested_patches: Vec::new(),
                            report,
                            exit_code: std::process::ExitCode::from(4),
                        });
                    }
                }

                let lock_value: Value = match serde_json::from_slice(&lock_bytes) {
                    Ok(v) => v,
                    Err(err) => {
                        let report = ArchReport {
                            schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                            manifest: ArchReportManifest {
                                path: manifest_path_s.clone(),
                                jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                            },
                            stats: ArchReportStats {
                                modules: 0,
                                nodes: manifest.nodes.len(),
                                module_edges: 0,
                                node_edges: 0,
                            },
                            diags: vec![diag_parse_error(
                                "E_ARCH_LOCK_JSON_PARSE",
                                &format!("parse lock JSON: {err}"),
                                Some(&lock_path_s),
                            )],
                            suggested_patches: Vec::new(),
                        };
                        return Ok(ArchCheckOutcome {
                            suggested_patches: Vec::new(),
                            report,
                            exit_code: std::process::ExitCode::from(3),
                        });
                    }
                };

                let schema_diags = validate_schema(
                    "E_ARCH_LOCK_SCHEMA_INVALID",
                    X07_ARCH_MANIFEST_LOCK_SCHEMA_BYTES,
                    &lock_value,
                )?;
                if !schema_diags.is_empty() {
                    let mut diags = schema_diags;
                    sort_diags(&mut diags);
                    let report = ArchReport {
                        schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                        manifest: ArchReportManifest {
                            path: manifest_path_s.clone(),
                            jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                        },
                        stats: ArchReportStats {
                            modules: 0,
                            nodes: manifest.nodes.len(),
                            module_edges: 0,
                            node_edges: 0,
                        },
                        diags,
                        suggested_patches: Vec::new(),
                    };
                    return Ok(ArchCheckOutcome {
                        suggested_patches: Vec::new(),
                        report,
                        exit_code: std::process::ExitCode::from(3),
                    });
                }

                let lock: ArchManifestLock = match serde_json::from_value(lock_value) {
                    Ok(v) => v,
                    Err(err) => {
                        let report = ArchReport {
                            schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                            manifest: ArchReportManifest {
                                path: manifest_path_s.clone(),
                                jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                            },
                            stats: ArchReportStats {
                                modules: 0,
                                nodes: manifest.nodes.len(),
                                module_edges: 0,
                                node_edges: 0,
                            },
                            diags: vec![diag_parse_error(
                                "E_ARCH_LOCK_INVALID",
                                &format!("parse lock: {err}"),
                                Some(&lock_path_s),
                            )],
                            suggested_patches: Vec::new(),
                        };
                        return Ok(ArchCheckOutcome {
                            suggested_patches: Vec::new(),
                            report,
                            exit_code: std::process::ExitCode::from(3),
                        });
                    }
                };
                if lock.schema_version != X07_ARCH_MANIFEST_LOCK_SCHEMA_VERSION {
                    anyhow::bail!(
                        "internal error: lock schema_version mismatch (expected {} got {:?})",
                        X07_ARCH_MANIFEST_LOCK_SCHEMA_VERSION,
                        lock.schema_version
                    );
                }
                Some(lock)
            }
        }
    } else {
        None
    };

    let (scan_include, scan_exclude) = match &lock {
        Some(lock) => (
            lock.module_scan.include_globs.clone(),
            lock.module_scan.exclude_globs.clone(),
        ),
        None => (
            DEFAULT_MODULE_SCAN_INCLUDE
                .iter()
                .map(|s| s.to_string())
                .collect(),
            DEFAULT_MODULE_SCAN_EXCLUDE
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
    };

    let node_ids: BTreeSet<String> = manifest.nodes.iter().map(|n| n.id.clone()).collect();
    if node_ids.len() != manifest.nodes.len() {
        let report = ArchReport {
            schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
            manifest: ArchReportManifest {
                path: manifest_path_s.clone(),
                jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
            },
            stats: ArchReportStats {
                modules: 0,
                nodes: manifest.nodes.len(),
                module_edges: 0,
                node_edges: 0,
            },
            diags: vec![diag_parse_error(
                "E_ARCH_MANIFEST_INVALID",
                "duplicate node id in manifest.nodes",
                None,
            )],
            suggested_patches: Vec::new(),
        };
        return Ok(ArchCheckOutcome {
            suggested_patches: Vec::new(),
            report,
            exit_code: std::process::ExitCode::from(3),
        });
    }

    let mut node_matchers = Vec::new();
    for node in &manifest.nodes {
        let world = match WorldId::parse(&node.world) {
            Some(w) => w,
            None => {
                let report = ArchReport {
                    schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                    manifest: ArchReportManifest {
                        path: manifest_path_s.clone(),
                        jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                    },
                    stats: ArchReportStats {
                        modules: 0,
                        nodes: manifest.nodes.len(),
                        module_edges: 0,
                        node_edges: 0,
                    },
                    diags: vec![diag_parse_error(
                        "E_ARCH_MANIFEST_INVALID",
                        &format!(
                            "unsupported node world {:?} (node={:?})",
                            node.world, node.id
                        ),
                        Some(&manifest_path_s),
                    )],
                    suggested_patches: Vec::new(),
                };
                return Ok(ArchCheckOutcome {
                    suggested_patches: Vec::new(),
                    report,
                    exit_code: std::process::ExitCode::from(3),
                });
            }
        };
        let path_globs = match compile_globset(&node.matcher.path_globs) {
            Ok(v) => v,
            Err(err) => {
                let report = ArchReport {
                    schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                    manifest: ArchReportManifest {
                        path: manifest_path_s.clone(),
                        jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                    },
                    stats: ArchReportStats {
                        modules: 0,
                        nodes: manifest.nodes.len(),
                        module_edges: 0,
                        node_edges: 0,
                    },
                    diags: vec![diag_parse_error(
                        "E_ARCH_MANIFEST_INVALID",
                        &format!("invalid node match glob (node={}): {err}", node.id),
                        Some(&manifest_path_s),
                    )],
                    suggested_patches: Vec::new(),
                };
                return Ok(ArchCheckOutcome {
                    suggested_patches: Vec::new(),
                    report,
                    exit_code: std::process::ExitCode::from(3),
                });
            }
        };
        node_matchers.push(NodeMatcher {
            id: node.id.clone(),
            module_prefixes: node.matcher.module_prefixes.clone(),
            path_globs,
            world,
            visibility: node.visibility.clone(),
            imports: node.imports.clone(),
            smoke_entry: node.contracts.as_ref().map(|c| c.smoke_entry.clone()),
        });
    }

    let mut diags = DiagSink::new(budgets.max_diags);

    // Lock drift checks (non-fatal).
    if let Some(lock) = &lock {
        let expected_manifest_rel = match manifest_relpath_for_lock(repo_root, manifest_path) {
            Ok(v) => v,
            Err(err) => {
                let report = ArchReport {
                    schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                    manifest: ArchReportManifest {
                        path: manifest_path_s.clone(),
                        jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                    },
                    stats: ArchReportStats {
                        modules: 0,
                        nodes: manifest.nodes.len(),
                        module_edges: 0,
                        node_edges: 0,
                    },
                    diags: vec![diag_parse_error(
                        "E_ARCH_LOCK_INVALID",
                        &format!("lock unusable: {err}"),
                        None,
                    )],
                    suggested_patches: Vec::new(),
                };
                return Ok(ArchCheckOutcome {
                    suggested_patches: Vec::new(),
                    report,
                    exit_code: std::process::ExitCode::from(3),
                });
            }
        };
        if lock.manifest_path != expected_manifest_rel {
            diags.push(diag_lint_error(
                "E_ARCH_LOCK_MISMATCH",
                &format!(
                    "lock manifest_path mismatch: expected {:?} got {:?}",
                    expected_manifest_rel, lock.manifest_path
                ),
                None,
                BTreeMap::new(),
            ));
        }
        if lock.jcs_sha256_hex != manifest_jcs_sha256_hex && !write_lock {
            let mut data = BTreeMap::new();
            data.insert(
                "expected_jcs_sha256_hex".to_string(),
                Value::String(manifest_jcs_sha256_hex.clone()),
            );
            data.insert(
                "lock_jcs_sha256_hex".to_string(),
                Value::String(lock.jcs_sha256_hex.clone()),
            );
            diags.push(diag_lint_error(
                "E_ARCH_LOCK_MISMATCH",
                "manifest hash does not match lock (use --write-lock)",
                None,
                data,
            ));
        }
    }

    let include_set = match compile_globset(&scan_include) {
        Ok(v) => v,
        Err(err) => {
            let report = ArchReport {
                schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                manifest: ArchReportManifest {
                    path: manifest_path_s.clone(),
                    jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                },
                stats: ArchReportStats {
                    modules: 0,
                    nodes: manifest.nodes.len(),
                    module_edges: 0,
                    node_edges: 0,
                },
                diags: vec![diag_parse_error(
                    "E_ARCH_LOCK_INVALID",
                    &format!("invalid module_scan.include_globs: {err}"),
                    None,
                )],
                suggested_patches: Vec::new(),
            };
            return Ok(ArchCheckOutcome {
                suggested_patches: Vec::new(),
                report,
                exit_code: std::process::ExitCode::from(3),
            });
        }
    };
    let exclude_set = match compile_globset(&scan_exclude) {
        Ok(v) => v,
        Err(err) => {
            let report = ArchReport {
                schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
                manifest: ArchReportManifest {
                    path: manifest_path_s.clone(),
                    jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
                },
                stats: ArchReportStats {
                    modules: 0,
                    nodes: manifest.nodes.len(),
                    module_edges: 0,
                    node_edges: 0,
                },
                diags: vec![diag_parse_error(
                    "E_ARCH_LOCK_INVALID",
                    &format!("invalid module_scan.exclude_globs: {err}"),
                    None,
                )],
                suggested_patches: Vec::new(),
            };
            return Ok(ArchCheckOutcome {
                suggested_patches: Vec::new(),
                report,
                exit_code: std::process::ExitCode::from(3),
            });
        }
    };

    let mut module_paths: Vec<(String, PathBuf)> = Vec::new();
    for entry in WalkDir::new(repo_root).follow_links(false) {
        let entry = entry.with_context(|| format!("walk repo: {}", repo_root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(repo_root).unwrap_or(entry.path());
        let rel_posix = rel.to_string_lossy().replace('\\', "/");

        if rel_posix.ends_with("/.DS_Store") || rel_posix.ends_with(".DS_Store") {
            continue;
        }
        if rel_posix
            .split('/')
            .any(|p| p == ".DS_Store" || p.starts_with("._"))
        {
            continue;
        }

        if !include_set.is_match(rel) {
            continue;
        }
        if exclude_set.is_match(rel) {
            continue;
        }
        if !rel_posix.ends_with(".x07.json") {
            continue;
        }

        module_paths.push((rel_posix, entry.path().to_path_buf()));
    }
    module_paths.sort_by(|a, b| a.0.cmp(&b.0));

    if module_paths.len() > budgets.max_modules {
        let mut data = BTreeMap::new();
        data.insert(
            "budget".to_string(),
            Value::String("tool_budgets.max_modules".to_string()),
        );
        data.insert(
            "max_modules".to_string(),
            Value::Number((budgets.max_modules as u64).into()),
        );
        data.insert(
            "modules_found".to_string(),
            Value::Number((module_paths.len() as u64).into()),
        );
        diags.budget_exceeded("too many modules", data);
        sort_diags(&mut diags.diags);
        let report = ArchReport {
            schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
            manifest: ArchReportManifest {
                path: manifest_path_s.clone(),
                jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
            },
            stats: ArchReportStats {
                modules: module_paths.len(),
                nodes: manifest.nodes.len(),
                module_edges: 0,
                node_edges: 0,
            },
            diags: diags.diags,
            suggested_patches: Vec::new(),
        };
        return Ok(ArchCheckOutcome {
            suggested_patches: Vec::new(),
            report,
            exit_code: std::process::ExitCode::from(4),
        });
    }

    let mut modules_by_id: BTreeMap<String, ModuleInfo> = BTreeMap::new();
    for (rel_path, abs_path) in &module_paths {
        let bytes =
            std::fs::read(abs_path).with_context(|| format!("read: {}", abs_path.display()))?;
        bytes_in_total = bytes_in_total.saturating_add(bytes.len() as u64);
        if let Some(max) = budgets.max_bytes_in {
            if bytes_in_total > max {
                let mut data = BTreeMap::new();
                data.insert(
                    "budget".to_string(),
                    Value::String("max_bytes_in".to_string()),
                );
                data.insert("max_bytes_in".to_string(), Value::Number(max.into()));
                data.insert(
                    "bytes_in_total".to_string(),
                    Value::Number(bytes_in_total.into()),
                );
                diags.budget_exceeded("max_bytes_in exceeded during module scan", data);
                break;
            }
        }

        let parsed = match x07c::x07ast::parse_x07ast_json(&bytes) {
            Ok(v) => v,
            Err(err) => {
                let mut data = BTreeMap::new();
                data.insert("module_path".to_string(), Value::String(rel_path.clone()));
                diags.push(diagnostics::Diagnostic {
                    code: "E_ARCH_MODULE_PARSE".to_string(),
                    severity: diagnostics::Severity::Error,
                    stage: diagnostics::Stage::Parse,
                    message: err.message,
                    loc: None,
                    notes: Vec::new(),
                    related: Vec::new(),
                    data,
                    quickfix: None,
                });
                continue;
            }
        };
        let imports: Vec<String> = parsed.imports.iter().cloned().collect();

        if let Some(existing) = modules_by_id.get(&parsed.module_id) {
            let mut data = BTreeMap::new();
            data.insert(
                "module_id".to_string(),
                Value::String(parsed.module_id.clone()),
            );
            data.insert("module_path".to_string(), Value::String(rel_path.clone()));
            data.insert(
                "module_path_existing".to_string(),
                Value::String(existing.rel_path.clone()),
            );
            diags.push(diag_lint_error(
                "E_ARCH_DUPLICATE_MODULE_ID",
                "duplicate module_id across scanned files",
                None,
                data,
            ));
            continue;
        }

        modules_by_id.insert(
            parsed.module_id.clone(),
            ModuleInfo {
                rel_path: rel_path.clone(),
                imports,
                parsed,
            },
        );
    }

    if diags.tool_budget_exceeded {
        sort_diags(&mut diags.diags);
        let report = ArchReport {
            schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
            manifest: ArchReportManifest {
                path: manifest_path_s.clone(),
                jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
            },
            stats: ArchReportStats {
                modules: modules_by_id.len(),
                nodes: manifest.nodes.len(),
                module_edges: 0,
                node_edges: 0,
            },
            diags: diags.diags,
            suggested_patches: Vec::new(),
        };
        return Ok(ArchCheckOutcome {
            suggested_patches: Vec::new(),
            report,
            exit_code: std::process::ExitCode::from(4),
        });
    }

    let mut module_to_node: BTreeMap<String, String> = BTreeMap::new();
    let mut node_to_modules: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let mut orphan_modules: Vec<(String, String)> = Vec::new(); // (module_id, module_path)

    for (module_id, m) in &modules_by_id {
        let mut matches: Vec<&NodeMatcher> = Vec::new();
        for node in &node_matchers {
            let mut hit = false;
            for pfx in &node.module_prefixes {
                if module_id.starts_with(pfx) {
                    hit = true;
                    break;
                }
            }
            if !hit && node.path_globs.is_match(Path::new(&m.rel_path)) {
                hit = true;
            }
            if hit {
                matches.push(node);
            }
        }

        if matches.is_empty() {
            orphan_modules.push((module_id.clone(), m.rel_path.clone()));
            if manifest.checks.deny_orphans {
                let mut data = BTreeMap::new();
                data.insert("module_id".to_string(), Value::String(module_id.clone()));
                data.insert("module_path".to_string(), Value::String(m.rel_path.clone()));
                diags.push(diag_lint_error(
                    "E_ARCH_NODE_ORPHAN_MODULE",
                    "module matched no node",
                    Some(&m.rel_path),
                    data,
                ));
            }
            continue;
        }
        if matches.len() > 1 {
            let mut data = BTreeMap::new();
            data.insert("module_id".to_string(), Value::String(module_id.clone()));
            data.insert("module_path".to_string(), Value::String(m.rel_path.clone()));
            data.insert(
                "matched_nodes".to_string(),
                Value::Array(
                    matches
                        .iter()
                        .map(|n| Value::String(n.id.clone()))
                        .collect(),
                ),
            );
            diags.push(diag_lint_error(
                "E_ARCH_NODE_OVERLAP_MODULE",
                "module matched multiple nodes",
                Some(&m.rel_path),
                data,
            ));
            continue;
        }

        let node_id = matches[0].id.clone();
        module_to_node.insert(module_id.clone(), node_id.clone());
        node_to_modules
            .entry(node_id)
            .or_default()
            .push(module_id.clone());
    }

    for mods in node_to_modules.values_mut() {
        mods.sort();
        mods.dedup();
    }

    let mut node_by_id: BTreeMap<String, NodeMatcher> = BTreeMap::new();
    for node in node_matchers {
        node_by_id.insert(node.id.clone(), node);
    }

    let allowlist_mode = manifest
        .checks
        .allowlist_mode
        .as_ref()
        .filter(|m| m.enabled);

    let brand_boundary_enabled = manifest
        .checks
        .brand_boundary_v1
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(true);

    let world_of_imported_enabled = manifest
        .checks
        .world_of_imported_v1
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(true);

    let externals_allowed_exact: BTreeSet<String> =
        manifest.externals.allowed_exact.iter().cloned().collect();

    let externals_allowed_prefixes: Vec<String> =
        if allowlist_mode.is_some_and(|m| !m.default_allow_external) {
            Vec::new()
        } else {
            manifest.externals.allowed_import_prefixes.clone()
        };

    let mut module_edges: BTreeSet<(String, String)> = BTreeSet::new();
    let mut node_edges: BTreeSet<(String, String)> = BTreeSet::new();
    let mut node_edge_evidence: BTreeMap<(String, String), EdgeEvidence> = BTreeMap::new();

    let mut external_imports_not_allowed: BTreeSet<String> = BTreeSet::new();
    let mut imports_to_remove_by_module: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut missing_allow_edges: Vec<(String, String)> = Vec::new();

    for (module_id, m) in &modules_by_id {
        for imp in &m.imports {
            let is_internal = modules_by_id.contains_key(imp);
            if !is_internal {
                let allowed = externals_allowed_exact.contains(imp)
                    || externals_allowed_prefixes
                        .iter()
                        .any(|p| imp.starts_with(p));
                if !allowed {
                    let mut data = BTreeMap::new();
                    data.insert("module_id".to_string(), Value::String(module_id.clone()));
                    data.insert("module_path".to_string(), Value::String(m.rel_path.clone()));
                    data.insert("import".to_string(), Value::String(imp.clone()));
                    diags.push(diag_lint_error(
                        "E_ARCH_EXTERNAL_IMPORT_NOT_ALLOWED",
                        "external import is not allowed by externals policy",
                        Some(&m.rel_path),
                        data,
                    ));
                    external_imports_not_allowed.insert(imp.clone());
                }
            }

            let Some(from_node_id) = module_to_node.get(module_id) else {
                continue;
            };
            let Some(from_node) = node_by_id.get(from_node_id) else {
                continue;
            };

            if manifest.checks.enforce_world_caps {
                if from_node
                    .imports
                    .deny_prefixes
                    .iter()
                    .any(|p| imp.starts_with(p))
                {
                    imports_to_remove_by_module
                        .entry(module_id.clone())
                        .or_default()
                        .insert(imp.clone());
                    let denied_prefix = from_node
                        .imports
                        .deny_prefixes
                        .iter()
                        .find(|p| imp.starts_with(*p))
                        .cloned()
                        .unwrap_or_default();
                    let mut data = BTreeMap::new();
                    data.insert("node".to_string(), Value::String(from_node_id.clone()));
                    data.insert("module_id".to_string(), Value::String(module_id.clone()));
                    data.insert("module_path".to_string(), Value::String(m.rel_path.clone()));
                    data.insert("import".to_string(), Value::String(imp.clone()));
                    data.insert("denied_prefix".to_string(), Value::String(denied_prefix));
                    diags.push(diag_lint_error(
                        "E_ARCH_IMPORT_PREFIX_DENIED",
                        "node import policy denies this prefix",
                        Some(&m.rel_path),
                        data,
                    ));
                } else if !from_node.imports.allow_prefixes.is_empty()
                    && !from_node
                        .imports
                        .allow_prefixes
                        .iter()
                        .any(|p| imp.starts_with(p))
                {
                    imports_to_remove_by_module
                        .entry(module_id.clone())
                        .or_default()
                        .insert(imp.clone());
                    let mut data = BTreeMap::new();
                    data.insert("node".to_string(), Value::String(from_node_id.clone()));
                    data.insert("module_id".to_string(), Value::String(module_id.clone()));
                    data.insert("module_path".to_string(), Value::String(m.rel_path.clone()));
                    data.insert("import".to_string(), Value::String(imp.clone()));
                    diags.push(diag_lint_error(
                        "E_ARCH_IMPORT_PREFIX_NOT_ALLOWED",
                        "node import policy does not allow this import prefix",
                        Some(&m.rel_path),
                        data,
                    ));
                }
            }

            if is_internal {
                let Some(to_node_id) = module_to_node.get(imp) else {
                    continue;
                };
                module_edges.insert((module_id.clone(), imp.clone()));
                node_edges.insert((from_node_id.clone(), to_node_id.clone()));
                node_edge_evidence
                    .entry((from_node_id.clone(), to_node_id.clone()))
                    .or_insert_with(|| EdgeEvidence {
                        module_path: m.rel_path.clone(),
                        module_id: module_id.clone(),
                        import: imp.clone(),
                    });
            }
        }
    }

    if module_edges.len() > budgets.max_edges {
        let mut data = BTreeMap::new();
        data.insert(
            "budget".to_string(),
            Value::String("tool_budgets.max_edges".to_string()),
        );
        data.insert(
            "max_edges".to_string(),
            Value::Number((budgets.max_edges as u64).into()),
        );
        data.insert(
            "module_edges".to_string(),
            Value::Number((module_edges.len() as u64).into()),
        );
        diags.budget_exceeded("too many module edges", data);
        sort_diags(&mut diags.diags);
        let report = ArchReport {
            schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
            manifest: ArchReportManifest {
                path: manifest_path_s.clone(),
                jcs_sha256_hex: Some(manifest_jcs_sha256_hex),
            },
            stats: ArchReportStats {
                modules: modules_by_id.len(),
                nodes: manifest.nodes.len(),
                module_edges: module_edges.len(),
                node_edges: node_edges.len(),
            },
            diags: diags.diags,
            suggested_patches: Vec::new(),
        };
        return Ok(ArchCheckOutcome {
            suggested_patches: Vec::new(),
            report,
            exit_code: std::process::ExitCode::from(4),
        });
    }

    // Visibility enforcement.
    if manifest.checks.enforce_visibility {
        for (from, to) in &node_edges {
            let Some(to_node) = node_by_id.get(to) else {
                continue;
            };
            if to_node.visibility.mode.trim() != "restricted" {
                continue;
            }
            if to_node.visibility.visible_to.iter().any(|n| n == from) {
                continue;
            }
            let mut data = BTreeMap::new();
            data.insert("node_from".to_string(), Value::String(from.clone()));
            data.insert("node_to".to_string(), Value::String(to.clone()));
            if let Some(ev) = node_edge_evidence.get(&(from.clone(), to.clone())) {
                data.insert(
                    "module_path".to_string(),
                    Value::String(ev.module_path.clone()),
                );
                data.insert("module_id".to_string(), Value::String(ev.module_id.clone()));
                data.insert("import".to_string(), Value::String(ev.import.clone()));
            }
            diags.push(diag_lint_error(
                "E_ARCH_VISIBILITY",
                "target node is not visible to importing node",
                None,
                data,
            ));
        }
    }

    // Rule checks.
    for rule in &manifest.rules {
        match rule {
            ArchRule::DepsV1 { id, from, to, mode } => {
                if mode.trim() != "deny" {
                    continue;
                }
                for (edge_from, edge_to) in &node_edges {
                    if edge_from != from {
                        continue;
                    }
                    if !to.iter().any(|t| t == edge_to) {
                        continue;
                    }
                    let mut data = BTreeMap::new();
                    data.insert("rule_id".to_string(), Value::String(id.clone()));
                    data.insert("node_from".to_string(), Value::String(edge_from.clone()));
                    data.insert("node_to".to_string(), Value::String(edge_to.clone()));
                    if let Some(ev) = node_edge_evidence.get(&(edge_from.clone(), edge_to.clone()))
                    {
                        data.insert(
                            "module_path".to_string(),
                            Value::String(ev.module_path.clone()),
                        );
                        data.insert("module_id".to_string(), Value::String(ev.module_id.clone()));
                        data.insert("import".to_string(), Value::String(ev.import.clone()));
                    }
                    diags.push(diag_lint_error(
                        "E_ARCH_DEPS_DENY",
                        "dependency is denied by deps_v1 rule",
                        None,
                        data,
                    ));
                }
            }
            ArchRule::LayersV1 {
                id,
                layers,
                direction,
            } => {
                if direction.trim() != "down" {
                    continue;
                }
                let mut idx: BTreeMap<&str, usize> = BTreeMap::new();
                for (i, n) in layers.iter().enumerate() {
                    idx.insert(n.as_str(), i);
                }
                for (edge_from, edge_to) in &node_edges {
                    let Some(&i_from) = idx.get(edge_from.as_str()) else {
                        continue;
                    };
                    let Some(&i_to) = idx.get(edge_to.as_str()) else {
                        continue;
                    };
                    if i_to <= i_from {
                        continue;
                    }
                    let mut data = BTreeMap::new();
                    data.insert("rule_id".to_string(), Value::String(id.clone()));
                    data.insert("node_from".to_string(), Value::String(edge_from.clone()));
                    data.insert("node_to".to_string(), Value::String(edge_to.clone()));
                    if let Some(ev) = node_edge_evidence.get(&(edge_from.clone(), edge_to.clone()))
                    {
                        data.insert(
                            "module_path".to_string(),
                            Value::String(ev.module_path.clone()),
                        );
                        data.insert("module_id".to_string(), Value::String(ev.module_id.clone()));
                        data.insert("import".to_string(), Value::String(ev.import.clone()));
                    }
                    diags.push(diag_lint_error(
                        "E_ARCH_LAYERS_VIOLATION",
                        "dependency violates layers_v1 direction=down",
                        None,
                        data,
                    ));
                }
            }
            ArchRule::DenyCyclesV1 { id, scope } => {
                if scope.trim() != "nodes" || !manifest.checks.deny_cycles {
                    continue;
                }
                let cycles = find_cycles(&node_edges);
                for scc in cycles {
                    let mut data = BTreeMap::new();
                    data.insert("rule_id".to_string(), Value::String(id.clone()));
                    data.insert(
                        "cycle_nodes".to_string(),
                        Value::Array(scc.iter().cloned().map(Value::String).collect()),
                    );
                    let mut evidence: Vec<Value> = Vec::new();
                    for (a, b) in node_edges
                        .iter()
                        .filter(|(a, b)| scc.contains(a) && scc.contains(b))
                    {
                        if let Some(ev) = node_edge_evidence.get(&(a.clone(), b.clone())) {
                            let mut e = serde_json::Map::new();
                            e.insert(
                                "module_path".to_string(),
                                Value::String(ev.module_path.clone()),
                            );
                            e.insert("module_id".to_string(), Value::String(ev.module_id.clone()));
                            e.insert("import".to_string(), Value::String(ev.import.clone()));
                            evidence.push(Value::Object(e));
                        }
                    }
                    evidence.sort_by(|a, b| {
                        let ap = a.get("module_path").and_then(Value::as_str).unwrap_or("");
                        let bp = b.get("module_path").and_then(Value::as_str).unwrap_or("");
                        ap.cmp(bp)
                    });
                    if !evidence.is_empty() {
                        data.insert("evidence".to_string(), Value::Array(evidence));
                    }
                    diags.push(diag_lint_error(
                        "E_ARCH_CYCLE",
                        "cyclic dependency between nodes is forbidden",
                        None,
                        data,
                    ));
                }
            }
        }
    }

    // allowlist_mode (v1.1)
    if let Some(allowlist) = allowlist_mode {
        if !allowlist.default_allow_internal {
            for (from, to) in &node_edges {
                if edge_is_denied_by_deps_rules(&manifest.rules, from, to) {
                    continue;
                }
                if edge_violates_layers_rules(&manifest.rules, from, to) {
                    continue;
                }
                if edge_is_allowed_by_rules(&manifest.rules, from, to) {
                    continue;
                }
                let mut data = BTreeMap::new();
                data.insert("node_from".to_string(), Value::String(from.clone()));
                data.insert("node_to".to_string(), Value::String(to.clone()));
                if let Some(ev) = node_edge_evidence.get(&(from.clone(), to.clone())) {
                    data.insert(
                        "module_path".to_string(),
                        Value::String(ev.module_path.clone()),
                    );
                    data.insert("module_id".to_string(), Value::String(ev.module_id.clone()));
                    data.insert("import".to_string(), Value::String(ev.import.clone()));
                }
                missing_allow_edges.push((from.clone(), to.clone()));
                diags.push(diag_lint_error(
                    "E_ARCH_EDGE_NOT_ALLOWED",
                    "internal edge is not allowed by layers_v1 or deps_v1 allow rules",
                    None,
                    data,
                ));
            }
        }
    }

    // world-of-imported enforcement (v1.1)
    if manifest.checks.enforce_world_caps && world_of_imported_enabled {
        for (from, to) in &node_edges {
            let Some(from_node) = node_by_id.get(from) else {
                continue;
            };
            let Some(to_node) = node_by_id.get(to) else {
                continue;
            };
            if from_node.world.is_eval_world() && to_node.world.is_standalone_only() {
                let mut data = BTreeMap::new();
                data.insert("node_from".to_string(), Value::String(from.clone()));
                data.insert("node_to".to_string(), Value::String(to.clone()));
                data.insert(
                    "world_from".to_string(),
                    Value::String(from_node.world.as_str().to_string()),
                );
                data.insert(
                    "world_to".to_string(),
                    Value::String(to_node.world.as_str().to_string()),
                );
                if let Some(ev) = node_edge_evidence.get(&(from.clone(), to.clone())) {
                    data.insert(
                        "module_path".to_string(),
                        Value::String(ev.module_path.clone()),
                    );
                    data.insert("module_id".to_string(), Value::String(ev.module_id.clone()));
                    data.insert("import".to_string(), Value::String(ev.import.clone()));
                }
                diags.push(diag_lint_error(
                    "E_ARCH_WORLD_EDGE_FORBIDDEN",
                    "solve-* nodes must not depend on run-os* nodes",
                    None,
                    data,
                ));
            }
        }
    }

    // smoke_entry contract (v1.1)
    for node in node_by_id.values() {
        let Some(smoke_entry) = &node.smoke_entry else {
            continue;
        };
        let Some(mods) = node_to_modules.get(&node.id) else {
            continue;
        };
        let mut found = false;
        for module_id in mods {
            let Some(m) = modules_by_id.get(module_id) else {
                continue;
            };
            if m.parsed.exports.contains(smoke_entry) {
                found = true;
                break;
            }
        }
        if !found {
            let mut data = BTreeMap::new();
            data.insert("node".to_string(), Value::String(node.id.clone()));
            data.insert(
                "smoke_entry".to_string(),
                Value::String(smoke_entry.clone()),
            );
            diags.push(diag_lint_error(
                "E_ARCH_SMOKE_MISSING",
                "node is missing contracts.smoke_entry export",
                None,
                data,
            ));
        }
    }

    // brand boundary checks (v1.1)
    if brand_boundary_enabled {
        for node in node_by_id.values() {
            if node.visibility.mode.trim() != "public" {
                continue;
            }
            let Some(mods) = node_to_modules.get(&node.id) else {
                continue;
            };
            for module_id in mods {
                let Some(m) = modules_by_id.get(module_id) else {
                    continue;
                };
                check_public_module_brands(node, m, &mut diags);
            }
        }
    }

    // Suggested patches.
    let mut suggested: BTreeMap<String, ArchPatchTarget> = BTreeMap::new();

    if !external_imports_not_allowed.is_empty() {
        let mut patch = Vec::new();
        for imp in external_imports_not_allowed.iter() {
            if externals_allowed_exact.contains(imp) {
                continue;
            }
            patch.push(diagnostics::PatchOp::Add {
                path: "/externals/allowed_exact/-".to_string(),
                value: Value::String(imp.clone()),
            });
        }
        if !patch.is_empty() {
            let path = display_relpath(repo_root, manifest_path);
            suggested.insert(
                path.clone(),
                ArchPatchTarget {
                    path,
                    patch,
                    note: Some("Allow external imports explicitly (exact).".to_string()),
                },
            );
        }
    }

    for (module_id, imports_to_remove) in &imports_to_remove_by_module {
        let Some(m) = modules_by_id.get(module_id) else {
            continue;
        };
        let mut new_imports: Vec<String> = Vec::new();
        for imp in &m.parsed.imports {
            if imports_to_remove.contains(imp) {
                continue;
            }
            new_imports.push(imp.clone());
        }
        let path = m.rel_path.clone();
        suggested.insert(
            path.clone(),
            ArchPatchTarget {
                path,
                patch: vec![diagnostics::PatchOp::Replace {
                    path: "/imports".to_string(),
                    value: Value::Array(new_imports.into_iter().map(Value::String).collect()),
                }],
                note: Some("Remove forbidden imports.".to_string()),
            },
        );
    }

    if manifest.checks.deny_orphans {
        let manifest_rel = display_relpath(repo_root, manifest_path);
        let mut ops = Vec::new();
        for (module_id, module_path) in orphan_modules {
            let node_value = orphan_node_value(&module_id, &module_path);
            ops.push(diagnostics::PatchOp::Add {
                path: "/nodes/-".to_string(),
                value: node_value,
            });
        }
        if !ops.is_empty() {
            let entry = suggested
                .entry(manifest_rel.clone())
                .or_insert(ArchPatchTarget {
                    path: manifest_rel,
                    patch: Vec::new(),
                    note: Some("Add nodes for orphan modules.".to_string()),
                });
            entry.patch.extend(ops);
        }
    }

    if let Some(allowlist) = allowlist_mode {
        if !allowlist.default_allow_internal && !missing_allow_edges.is_empty() {
            let manifest_rel = display_relpath(repo_root, manifest_path);
            let entry = suggested
                .entry(manifest_rel.clone())
                .or_insert(ArchPatchTarget {
                    path: manifest_rel,
                    patch: Vec::new(),
                    note: Some("Allow internal edges explicitly.".to_string()),
                });
            for (from, to) in &missing_allow_edges {
                let id = allow_deps_rule_id(from, to);
                entry.patch.push(diagnostics::PatchOp::Add {
                    path: "/rules/-".to_string(),
                    value: serde_json::json!({
                      "kind": "deps_v1",
                      "id": id,
                      "from": from,
                      "to": [to],
                      "mode": "allow"
                    }),
                });
            }
        }
    }

    let mut suggested_patches: Vec<ArchPatchTarget> = suggested.into_values().collect();
    suggested_patches.sort_by(|a, b| a.path.cmp(&b.path));

    if diags.diags_overflowed && !diags.tool_budget_exceeded {
        let mut data = BTreeMap::new();
        data.insert(
            "budget".to_string(),
            Value::String("tool_budgets.max_diags".to_string()),
        );
        data.insert(
            "max_diags".to_string(),
            Value::Number((diags.max_diags as u64).into()),
        );
        diags.budget_exceeded("too many diagnostics", data);
    }

    let mut out_diags = diags.diags;
    sort_diags(&mut out_diags);

    let report = ArchReport {
        schema_version: X07_ARCH_REPORT_SCHEMA_VERSION,
        manifest: ArchReportManifest {
            path: manifest_path_s.clone(),
            jcs_sha256_hex: Some(manifest_jcs_sha256_hex.clone()),
        },
        stats: ArchReportStats {
            modules: modules_by_id.len(),
            nodes: manifest.nodes.len(),
            module_edges: module_edges.len(),
            node_edges: node_edges.len(),
        },
        diags: out_diags.clone(),
        suggested_patches: suggested_patches.clone(),
    };

    if write_lock {
        if let Some(lock_path) = lock_path {
            let lock_doc = ArchManifestLock {
                schema_version: X07_ARCH_MANIFEST_LOCK_SCHEMA_VERSION.to_string(),
                manifest_path: manifest_relpath_for_lock(repo_root, manifest_path)?,
                jcs_sha256_hex: manifest_jcs_sha256_hex.clone(),
                module_scan: ArchModuleScan {
                    include_globs: scan_include,
                    exclude_globs: scan_exclude,
                },
            };
            let bytes = canonical_pretty_json_bytes(&serde_json::to_value(lock_doc)?)?;
            util::write_atomic(lock_path, &bytes)
                .with_context(|| format!("write lock: {}", lock_path.display()))?;
        }
    }

    let has_error = report
        .diags
        .iter()
        .any(|d| d.severity == diagnostics::Severity::Error);
    let exit_code = if diags.tool_budget_exceeded {
        std::process::ExitCode::from(4)
    } else if has_error {
        std::process::ExitCode::from(2)
    } else {
        std::process::ExitCode::SUCCESS
    };

    Ok(ArchCheckOutcome {
        report,
        suggested_patches,
        exit_code,
    })
}

fn resolve_path_under_root(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    repo_root.join(path)
}

fn resolve_lock_path(repo_root: &Path, lock_arg: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(p) = lock_arg {
        return Some(resolve_path_under_root(repo_root, p));
    }
    let default = repo_root.join("arch/manifest.lock.json");
    if default.is_file() {
        return Some(default);
    }
    None
}

fn display_relpath(repo_root: &Path, path: &Path) -> String {
    match path.strip_prefix(repo_root) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => path.display().to_string(),
    }
}

fn manifest_relpath_for_lock(repo_root: &Path, manifest_path: &Path) -> Result<String> {
    let rel = manifest_path.strip_prefix(repo_root).with_context(|| {
        format!(
            "manifest path is not under repo root: {}",
            manifest_path.display()
        )
    })?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn compile_globset(globs: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for g in globs {
        builder.add(Glob::new(g).with_context(|| format!("invalid glob: {g:?}"))?);
    }
    Ok(builder.build()?)
}

fn validate_schema(
    code: &str,
    schema_bytes: &[u8],
    doc: &Value,
) -> Result<Vec<diagnostics::Diagnostic>> {
    let schema_json: Value = serde_json::from_slice(schema_bytes).context("parse JSON schema")?;
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema_json)
        .context("build schema validator")?;
    let mut out = Vec::new();
    for err in validator.iter_errors(doc) {
        let mut data = BTreeMap::new();
        data.insert(
            "instance_path".to_string(),
            Value::String(err.instance_path().to_string()),
        );
        data.insert(
            "schema_path".to_string(),
            Value::String(err.schema_path().to_string()),
        );
        out.push(diag_parse_error(code, &err.to_string(), None).with_data(data));
    }
    Ok(out)
}

fn diag_parse_error(code: &str, message: &str, file: Option<&str>) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic {
        code: code.to_string(),
        severity: diagnostics::Severity::Error,
        stage: diagnostics::Stage::Parse,
        message: message.to_string(),
        loc: file.map(|f| diagnostics::Location::Text {
            span: diagnostics::Span {
                start: diagnostics::Position {
                    line: 1,
                    col: 1,
                    offset: None,
                },
                end: diagnostics::Position {
                    line: 1,
                    col: 1,
                    offset: None,
                },
                file: Some(f.to_string()),
            },
            snippet: None,
        }),
        notes: Vec::new(),
        related: Vec::new(),
        data: BTreeMap::new(),
        quickfix: None,
    }
}

fn diag_lint_error(
    code: &str,
    message: &str,
    file: Option<&str>,
    data: BTreeMap<String, Value>,
) -> diagnostics::Diagnostic {
    let mut d = diagnostics::Diagnostic {
        code: code.to_string(),
        severity: diagnostics::Severity::Error,
        stage: diagnostics::Stage::Lint,
        message: message.to_string(),
        loc: file.map(|f| diagnostics::Location::Text {
            span: diagnostics::Span {
                start: diagnostics::Position {
                    line: 1,
                    col: 1,
                    offset: None,
                },
                end: diagnostics::Position {
                    line: 1,
                    col: 1,
                    offset: None,
                },
                file: Some(f.to_string()),
            },
            snippet: None,
        }),
        notes: Vec::new(),
        related: Vec::new(),
        data: BTreeMap::new(),
        quickfix: None,
    };
    d.data = data;
    d
}

trait DiagnosticExt {
    fn with_data(self, data: BTreeMap<String, Value>) -> Self;
}

impl DiagnosticExt for diagnostics::Diagnostic {
    fn with_data(mut self, data: BTreeMap<String, Value>) -> Self {
        self.data = data;
        self
    }
}

fn diag_budget_exceeded(message: &str, budget: &str) -> diagnostics::Diagnostic {
    let mut data = BTreeMap::new();
    data.insert("budget".to_string(), Value::String(budget.to_string()));
    diagnostics::Diagnostic {
        code: "E_ARCH_TOOL_BUDGET_EXCEEDED".to_string(),
        severity: diagnostics::Severity::Error,
        stage: diagnostics::Stage::Lint,
        message: message.to_string(),
        loc: None,
        notes: Vec::new(),
        related: Vec::new(),
        data,
        quickfix: None,
    }
}

fn canonical_jcs_bytes(v: &Value) -> Result<Vec<u8>> {
    let mut v = v.clone();
    x07c::x07ast::canon_value_jcs(&mut v);
    Ok(serde_json::to_vec(&v)?)
}

fn canonical_pretty_json_bytes(v: &Value) -> Result<Vec<u8>> {
    let mut v = v.clone();
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_vec_pretty(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn sort_diags(diags: &mut [diagnostics::Diagnostic]) {
    diags.sort_by(|a, b| {
        (
            severity_rank(a.severity),
            a.code.as_str(),
            data_str(a, "node_from"),
            data_str(a, "node_to"),
            data_str(a, "module_path"),
            data_str(a, "import"),
            a.message.as_str(),
        )
            .cmp(&(
                severity_rank(b.severity),
                b.code.as_str(),
                data_str(b, "node_from"),
                data_str(b, "node_to"),
                data_str(b, "module_path"),
                data_str(b, "import"),
                b.message.as_str(),
            ))
    });
}

fn severity_rank(s: diagnostics::Severity) -> i32 {
    match s {
        diagnostics::Severity::Error => 0,
        diagnostics::Severity::Warning => 1,
        diagnostics::Severity::Info => 2,
        diagnostics::Severity::Hint => 3,
    }
}

fn data_str<'a>(d: &'a diagnostics::Diagnostic, key: &str) -> &'a str {
    d.data.get(key).and_then(Value::as_str).unwrap_or("")
}

fn find_cycles(edges: &BTreeSet<(String, String)>) -> Vec<Vec<String>> {
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    let mut adj: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (a, b) in edges {
        nodes.insert(a.clone());
        nodes.insert(b.clone());
        adj.entry(a.clone()).or_default().insert(b.clone());
    }

    let mut index: u32 = 0;
    let mut stack: Vec<String> = Vec::new();
    let mut on_stack: BTreeSet<String> = BTreeSet::new();
    let mut idx: BTreeMap<String, u32> = BTreeMap::new();
    let mut low: BTreeMap<String, u32> = BTreeMap::new();
    let mut out: Vec<Vec<String>> = Vec::new();

    #[allow(clippy::too_many_arguments)]
    fn strongconnect(
        v: &str,
        index: &mut u32,
        stack: &mut Vec<String>,
        on_stack: &mut BTreeSet<String>,
        idx: &mut BTreeMap<String, u32>,
        low: &mut BTreeMap<String, u32>,
        adj: &BTreeMap<String, BTreeSet<String>>,
        out: &mut Vec<Vec<String>>,
    ) {
        let v_idx = *index;
        *index += 1;
        idx.insert(v.to_string(), v_idx);
        low.insert(v.to_string(), v_idx);
        stack.push(v.to_string());
        on_stack.insert(v.to_string());

        if let Some(neigh) = adj.get(v) {
            for w in neigh {
                if !idx.contains_key(w) {
                    strongconnect(w, index, stack, on_stack, idx, low, adj, out);
                    let lw = *low.get(w).unwrap_or(&v_idx);
                    let lv = *low.get(v).unwrap_or(&v_idx);
                    low.insert(v.to_string(), lv.min(lw));
                } else if on_stack.contains(w) {
                    let iw = *idx.get(w).unwrap_or(&v_idx);
                    let lv = *low.get(v).unwrap_or(&v_idx);
                    low.insert(v.to_string(), lv.min(iw));
                }
            }
        }

        let lv = *low.get(v).unwrap_or(&v_idx);
        if lv == v_idx {
            let mut scc: Vec<String> = Vec::new();
            while let Some(w) = stack.pop() {
                on_stack.remove(&w);
                scc.push(w.clone());
                if w == v {
                    break;
                }
            }
            if scc.len() > 1 {
                scc.sort();
                out.push(scc);
            }
        }
    }

    for v in nodes.iter() {
        if !idx.contains_key(v) {
            strongconnect(
                v,
                &mut index,
                &mut stack,
                &mut on_stack,
                &mut idx,
                &mut low,
                &adj,
                &mut out,
            );
        }
    }

    out.sort();
    out
}

fn edge_is_denied_by_deps_rules(rules: &[ArchRule], from: &str, to: &str) -> bool {
    rules.iter().any(|r| match r {
        ArchRule::DepsV1 {
            from: rf,
            to: rt,
            mode,
            ..
        } => mode.trim() == "deny" && rf == from && rt.iter().any(|t| t == to),
        _ => false,
    })
}

fn edge_is_allowed_by_rules(rules: &[ArchRule], from: &str, to: &str) -> bool {
    rules.iter().any(|r| match r {
        ArchRule::DepsV1 {
            from: rf,
            to: rt,
            mode,
            ..
        } => mode.trim() == "allow" && rf == from && rt.iter().any(|t| t == to),
        ArchRule::LayersV1 {
            layers, direction, ..
        } => {
            if direction.trim() != "down" {
                return false;
            }
            let mut idx: BTreeMap<&str, usize> = BTreeMap::new();
            for (i, n) in layers.iter().enumerate() {
                idx.insert(n.as_str(), i);
            }
            let Some(&i_from) = idx.get(from) else {
                return false;
            };
            let Some(&i_to) = idx.get(to) else {
                return false;
            };
            i_to <= i_from
        }
        _ => false,
    })
}

fn edge_violates_layers_rules(rules: &[ArchRule], from: &str, to: &str) -> bool {
    rules.iter().any(|r| match r {
        ArchRule::LayersV1 {
            layers, direction, ..
        } => {
            if direction.trim() != "down" {
                return false;
            }
            let mut idx: BTreeMap<&str, usize> = BTreeMap::new();
            for (i, n) in layers.iter().enumerate() {
                idx.insert(n.as_str(), i);
            }
            let Some(&i_from) = idx.get(from) else {
                return false;
            };
            let Some(&i_to) = idx.get(to) else {
                return false;
            };
            i_to > i_from
        }
        _ => false,
    })
}

fn check_public_module_brands(node: &NodeMatcher, m: &ModuleInfo, diags: &mut DiagSink) {
    for f in &m.parsed.functions {
        if !m.parsed.exports.contains(&f.name) {
            continue;
        }
        for p in &f.params {
            if is_bytesish(p.ty) && p.brand.is_none() {
                let mut data = BTreeMap::new();
                data.insert("node".to_string(), Value::String(node.id.clone()));
                data.insert("module_path".to_string(), Value::String(m.rel_path.clone()));
                data.insert("symbol".to_string(), Value::String(f.name.clone()));
                data.insert("param".to_string(), Value::String(p.name.clone()));
                data.insert("ty".to_string(), Value::String(format!("{:?}", p.ty)));
                diags.push(diag_lint_error(
                    "E_ARCH_PUBLIC_BYTES_UNBRANDED",
                    "public exported bytes param is missing a brand",
                    Some(&m.rel_path),
                    data,
                ));
            }
        }
        if is_bytesish(f.ret_ty) && f.ret_brand.is_none() {
            let mut data = BTreeMap::new();
            data.insert("node".to_string(), Value::String(node.id.clone()));
            data.insert("module_path".to_string(), Value::String(m.rel_path.clone()));
            data.insert("symbol".to_string(), Value::String(f.name.clone()));
            data.insert("ty".to_string(), Value::String(format!("{:?}", f.ret_ty)));
            diags.push(diag_lint_error(
                "E_ARCH_PUBLIC_BYTES_UNBRANDED",
                "public exported bytes result is missing a brand",
                Some(&m.rel_path),
                data,
            ));
        }
    }

    for f in &m.parsed.async_functions {
        if !m.parsed.exports.contains(&f.name) {
            continue;
        }
        for p in &f.params {
            if is_bytesish(p.ty) && p.brand.is_none() {
                let mut data = BTreeMap::new();
                data.insert("node".to_string(), Value::String(node.id.clone()));
                data.insert("module_path".to_string(), Value::String(m.rel_path.clone()));
                data.insert("symbol".to_string(), Value::String(f.name.clone()));
                data.insert("param".to_string(), Value::String(p.name.clone()));
                data.insert("ty".to_string(), Value::String(format!("{:?}", p.ty)));
                diags.push(diag_lint_error(
                    "E_ARCH_PUBLIC_BYTES_UNBRANDED",
                    "public exported bytes param is missing a brand",
                    Some(&m.rel_path),
                    data,
                ));
            }
        }
        if is_bytesish(f.ret_ty) && f.ret_brand.is_none() {
            let mut data = BTreeMap::new();
            data.insert("node".to_string(), Value::String(node.id.clone()));
            data.insert("module_path".to_string(), Value::String(m.rel_path.clone()));
            data.insert("symbol".to_string(), Value::String(f.name.clone()));
            data.insert("ty".to_string(), Value::String(format!("{:?}", f.ret_ty)));
            diags.push(diag_lint_error(
                "E_ARCH_PUBLIC_BYTES_UNBRANDED",
                "public exported bytes result is missing a brand",
                Some(&m.rel_path),
                data,
            ));
        }
    }
}

fn is_bytesish(ty: x07c::types::Ty) -> bool {
    matches!(
        ty,
        x07c::types::Ty::Bytes
            | x07c::types::Ty::BytesView
            | x07c::types::Ty::OptionBytes
            | x07c::types::Ty::OptionBytesView
            | x07c::types::Ty::ResultBytes
            | x07c::types::Ty::ResultBytesView
            | x07c::types::Ty::ResultResultBytes
    )
}

fn orphan_node_value(module_id: &str, module_path: &str) -> Value {
    let mut node_id = format!("orphan.{module_id}");
    if node_id.len() > 128 {
        let h = util::sha256_hex(node_id.as_bytes());
        node_id = format!("orphan.{}", &h[..32]);
    }

    let first_seg = module_id.split('.').next().unwrap_or("").trim();
    let allow_prefixes = if first_seg.is_empty() {
        vec!["std.".to_string(), "ext.".to_string()]
    } else {
        vec![
            "std.".to_string(),
            "ext.".to_string(),
            format!("{first_seg}."),
        ]
    };

    let mut m = serde_json::Map::new();
    m.insert("id".to_string(), Value::String(node_id));

    let mut match_obj = serde_json::Map::new();
    match_obj.insert("module_prefixes".to_string(), Value::Array(Vec::new()));
    match_obj.insert(
        "path_globs".to_string(),
        Value::Array(vec![Value::String(module_path.to_string())]),
    );
    m.insert("match".to_string(), Value::Object(match_obj));

    m.insert("world".to_string(), Value::String("solve-pure".to_string()));

    let mut vis = serde_json::Map::new();
    vis.insert("mode".to_string(), Value::String("restricted".to_string()));
    vis.insert("visible_to".to_string(), Value::Array(Vec::new()));
    m.insert("visibility".to_string(), Value::Object(vis));

    let mut imports = serde_json::Map::new();
    imports.insert("deny_prefixes".to_string(), Value::Array(Vec::new()));
    imports.insert(
        "allow_prefixes".to_string(),
        Value::Array(allow_prefixes.into_iter().map(Value::String).collect()),
    );
    m.insert("imports".to_string(), Value::Object(imports));

    Value::Object(m)
}

fn allow_deps_rule_id(from: &str, to: &str) -> String {
    let raw = format!("allow:{from}->{to}");
    if raw.len() <= 128 {
        return raw;
    }
    let h = util::sha256_hex(raw.as_bytes());
    format!("allow:sha256:{h}")
}

fn emit_report(args: &ArchCheckArgs, report: &ArchReport) -> Result<()> {
    match args.format {
        ArchFormat::Json => {
            let bytes = canonical_pretty_json_bytes(&serde_json::to_value(report)?)?;
            if let Some(path) = &args.out {
                util::write_atomic(path, &bytes)
                    .with_context(|| format!("write report: {}", path.display()))?;
            } else {
                std::io::Write::write_all(&mut std::io::stdout(), &bytes)
                    .context("write stdout")?;
            }
        }
        ArchFormat::Text => {
            let mut out = String::new();
            let errors = report
                .diags
                .iter()
                .filter(|d| d.severity == diagnostics::Severity::Error)
                .count();
            out.push_str(&format!(
                "ok: {}\n",
                if errors == 0 { "true" } else { "false" }
            ));
            out.push_str(&format!("manifest: {}\n", report.manifest.path));
            out.push_str(&format!(
                "stats: modules={} nodes={} module_edges={} node_edges={}\n",
                report.stats.modules,
                report.stats.nodes,
                report.stats.module_edges,
                report.stats.node_edges
            ));
            for d in &report.diags {
                out.push_str(&format!(
                    "{} {}: {}\n",
                    match d.severity {
                        diagnostics::Severity::Error => "error",
                        diagnostics::Severity::Warning => "warning",
                        diagnostics::Severity::Info => "info",
                        diagnostics::Severity::Hint => "hint",
                    },
                    d.code,
                    d.message
                ));
            }
            if let Some(path) = &args.out {
                util::write_atomic(path, out.as_bytes())
                    .with_context(|| format!("write report: {}", path.display()))?;
            } else {
                print!("{out}");
            }
        }
    }
    Ok(())
}

fn write_patchset(repo_root: &Path, out_path: &Path, patches: &[ArchPatchTarget]) -> Result<()> {
    let patchset = ArchPatchSet {
        schema_version: X07_ARCH_PATCHSET_SCHEMA_VERSION,
        patches: patches.to_vec(),
    };
    let bytes = canonical_pretty_json_bytes(&serde_json::to_value(patchset)?)?;
    let out_path = resolve_path_under_root(repo_root, out_path);
    util::write_atomic(&out_path, &bytes)
        .with_context(|| format!("write patchset: {}", out_path.display()))
}

fn apply_patchset(repo_root: &Path, patches: &[ArchPatchTarget]) -> Result<()> {
    for target in patches {
        let path = resolve_path_under_root(repo_root, Path::new(&target.path));
        let bytes = std::fs::read(&path).with_context(|| format!("read: {}", path.display()))?;
        let mut doc: Value = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse JSON: {}", path.display()))?;
        json_patch::apply_patch(&mut doc, &target.patch)
            .with_context(|| format!("apply patch: {}", path.display()))?;

        let out_bytes = if target.path.ends_with(".x07.json") {
            let mut file = x07c::x07ast::parse_x07ast_json(&serde_json::to_vec(&doc)?)
                .map_err(|e| anyhow::anyhow!("x07ast parse after patch: {e}"))?;
            x07c::x07ast::canonicalize_x07ast_file(&mut file);
            let mut v = x07c::x07ast::x07ast_file_to_value(&file);
            x07c::x07ast::canon_value_jcs(&mut v);
            let mut out = serde_json::to_vec_pretty(&v)?;
            if out.last() != Some(&b'\n') {
                out.push(b'\n');
            }
            out
        } else {
            canonical_pretty_json_bytes(&doc)?
        };

        util::write_atomic(&path, &out_bytes)
            .with_context(|| format!("write patched: {}", path.display()))?;
    }
    Ok(())
}
