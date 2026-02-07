use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use serde_json::{json, Value};
use x07_contracts::X07_REVIEW_DIFF_SCHEMA_VERSION;
use x07c::diagnostics;

use crate::report_common;
use crate::util;

const X07_REVIEW_DIFF_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-review.diff.schema.json");

#[derive(Debug, Clone, Args)]
#[command(subcommand_required = false)]
pub struct ReviewArgs {
    #[command(subcommand)]
    pub cmd: Option<ReviewCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ReviewCommand {
    /// Produce a semantic (x07AST-level) diff viewer report (HTML + optional JSON backing model).
    Diff(ReviewDiffArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum ReviewMode {
    Project,
    AstOnly,
}

impl ReviewMode {
    fn as_str(self) -> &'static str {
        match self {
            ReviewMode::Project => "project",
            ReviewMode::AstOnly => "ast-only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum ReviewFailOn {
    WorldCapability,
    BudgetIncrease,
    AllowUnsafe,
    AllowFfi,
}

#[derive(Debug, Clone, Args)]
pub struct ReviewDiffArgs {
    /// Baseline snapshot (file or directory).
    #[arg(long, value_name = "PATH")]
    pub from: PathBuf,

    /// Candidate snapshot (file or directory).
    #[arg(long, value_name = "PATH")]
    pub to: PathBuf,

    /// Output path for the HTML diff viewer (self-contained).
    #[arg(long, value_name = "PATH")]
    pub html_out: PathBuf,

    /// Optional output path for the JSON backing model (schema-valid).
    #[arg(long, value_name = "PATH")]
    pub json_out: Option<PathBuf>,

    /// Diff mode.
    #[arg(long, value_enum, default_value_t = ReviewMode::Project)]
    pub mode: ReviewMode,

    /// Include glob patterns (repeatable). If empty, defaults are used.
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    /// Exclude glob patterns (repeatable). If empty, defaults are used.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Hard cap to keep report bounded (emits truncation markers).
    #[arg(long, value_name = "N")]
    pub max_changes: Option<usize>,

    /// CI gating: fail if any matching change occurs.
    #[arg(long, value_enum)]
    pub fail_on: Vec<ReviewFailOn>,
}

#[derive(Debug, Clone, Serialize)]
struct ReviewDiffReport {
    schema_version: &'static str,
    mode: &'static str,
    from: Snapshot,
    to: Snapshot,
    summary: Summary,
    highlights: Highlights,
    files: Vec<FileDiff>,
    diags: Vec<diagnostics::Diagnostic>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    meta: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
struct Snapshot {
    kind: &'static str,
    path: String,
    sha256_hex: Option<String>,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Clone, Serialize)]
struct Summary {
    files_total: u64,
    files_changed: u64,
    modules_changed: u64,
    decls_added: u64,
    decls_removed: u64,
    decls_changed: u64,
    high_severity_changes: u64,
    truncated: bool,
}

impl Summary {
    fn empty() -> Self {
        Self {
            files_total: 0,
            files_changed: 0,
            modules_changed: 0,
            decls_added: 0,
            decls_removed: 0,
            decls_changed: 0,
            high_severity_changes: 0,
            truncated: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Highlights {
    world_changes: Vec<Value>,
    budget_changes: Vec<Value>,
    policy_changes: Vec<Value>,
    capability_changes: Vec<Value>,
}

impl Highlights {
    fn empty() -> Self {
        Self {
            world_changes: Vec::new(),
            budget_changes: Vec::new(),
            policy_changes: Vec::new(),
            capability_changes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct FileDiff {
    path: String,
    kind: String,
    status: String,
    before_sha256_hex: Option<String>,
    after_sha256_hex: Option<String>,
    changes: Vec<Value>,
    module: Option<Value>,
    truncated: bool,
}

#[derive(Debug, Clone)]
struct BuildCtx {
    limit: Option<usize>,
    emitted: usize,
    truncated: bool,
    high_severity_changes: u64,
}

impl BuildCtx {
    fn new(limit: Option<usize>) -> Self {
        Self {
            limit,
            emitted: 0,
            truncated: false,
            high_severity_changes: 0,
        }
    }

    fn push(&mut self, out: &mut Vec<Value>, value: Value) {
        if self.limit.is_some_and(|max| self.emitted >= max) {
            self.truncated = true;
            return;
        }
        if value
            .get("severity")
            .and_then(Value::as_str)
            .is_some_and(|s| s == "high")
        {
            self.high_severity_changes += 1;
        }
        out.push(value);
        self.emitted += 1;
    }
}

#[derive(Debug, Clone)]
struct AstDiffOutcome {
    changes: Vec<Value>,
    module: Option<Value>,
    decls_added: u64,
    decls_removed: u64,
    decls_changed: u64,
    modules_changed: u64,
    capability_added: BTreeSet<String>,
    capability_removed: BTreeSet<String>,
    budget_scope_changed: bool,
}

impl AstDiffOutcome {
    fn empty() -> Self {
        Self {
            changes: Vec::new(),
            module: None,
            decls_added: 0,
            decls_removed: 0,
            decls_changed: 0,
            modules_changed: 0,
            capability_added: BTreeSet::new(),
            capability_removed: BTreeSet::new(),
            budget_scope_changed: false,
        }
    }
}

#[derive(Debug, Clone)]
struct DeclView {
    kind: String,
    name: String,
    ptr: String,
    sig: Option<Value>,
    body: Option<Value>,
}

pub fn cmd_review(
    _machine: &crate::reporting::MachineArgs,
    args: ReviewArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing review subcommand (try --help)");
    };

    match cmd {
        ReviewCommand::Diff(args) => cmd_review_diff(args),
    }
}

fn cmd_review_diff(args: ReviewDiffArgs) -> Result<std::process::ExitCode> {
    let from = util::resolve_existing_path_upwards(&args.from);
    let to = util::resolve_existing_path_upwards(&args.to);

    if !from.exists() {
        anyhow::bail!("missing --from path: {}", from.display());
    }
    if !to.exists() {
        anyhow::bail!("missing --to path: {}", to.display());
    }

    let from_is_dir = from.is_dir();
    let to_is_dir = to.is_dir();
    if from_is_dir != to_is_dir {
        anyhow::bail!(
            "--from and --to must both be files or both be directories (got {} vs {})",
            path_kind(&from),
            path_kind(&to)
        );
    }

    let (include_set, exclude_set) = build_globsets(&args)?;
    let mut ctx = BuildCtx::new(args.max_changes);

    let mut report = ReviewDiffReport {
        schema_version: X07_REVIEW_DIFF_SCHEMA_VERSION,
        mode: args.mode.as_str(),
        from: Snapshot::from_path(&from),
        to: Snapshot::from_path(&to),
        summary: Summary::empty(),
        highlights: Highlights::empty(),
        files: Vec::new(),
        diags: Vec::new(),
        meta: BTreeMap::new(),
    };

    let rel_paths = collect_rel_paths(&from, &to, from_is_dir, &include_set, &exclude_set)?;

    for rel in rel_paths {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let before_path = if from_is_dir {
            Some(from.join(&rel))
        } else {
            Some(from.clone())
        };
        let after_path = if to_is_dir {
            Some(to.join(&rel))
        } else {
            Some(to.clone())
        };

        let before_bytes = read_if_file(before_path.as_deref().unwrap_or(&from));
        let after_bytes = read_if_file(after_path.as_deref().unwrap_or(&to));

        let before_exists = before_bytes.is_some();
        let after_exists = after_bytes.is_some();

        let status = match (before_exists, after_exists) {
            (true, true) => {
                if before_bytes == after_bytes {
                    "unchanged"
                } else {
                    "changed"
                }
            }
            (false, true) => "added",
            (true, false) => "removed",
            (false, false) => continue,
        }
        .to_string();

        let before_json = before_bytes
            .as_ref()
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok());
        let after_json = after_bytes
            .as_ref()
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok());

        let kind = classify_file(&rel_str, before_json.as_ref(), after_json.as_ref()).to_string();

        let mut file_diff = FileDiff {
            path: rel_str.clone(),
            kind,
            status: status.clone(),
            before_sha256_hex: before_bytes.as_deref().map(util::sha256_hex),
            after_sha256_hex: after_bytes.as_deref().map(util::sha256_hex),
            changes: Vec::new(),
            module: None,
            truncated: false,
        };

        report.summary.files_total += 1;
        if status != "unchanged" {
            report.summary.files_changed += 1;
        }

        if status != "unchanged" {
            match file_diff.kind.as_str() {
                "x07ast" => {
                    let ast = diff_x07ast(
                        &rel_str,
                        before_json.as_ref(),
                        after_json.as_ref(),
                        &mut ctx,
                        &mut report.highlights,
                    );
                    file_diff.changes = ast.changes;
                    file_diff.module = ast.module;
                    report.summary.decls_added += ast.decls_added;
                    report.summary.decls_removed += ast.decls_removed;
                    report.summary.decls_changed += ast.decls_changed;
                    report.summary.modules_changed += ast.modules_changed;

                    if !ast.capability_added.is_empty() || !ast.capability_removed.is_empty() {
                        let mut added: Vec<String> = ast.capability_added.into_iter().collect();
                        let mut removed: Vec<String> = ast.capability_removed.into_iter().collect();
                        added.sort();
                        removed.sort();
                        let sev = if added.iter().any(|s| s.starts_with("std.os.")) {
                            "high"
                        } else {
                            "warn"
                        };
                        let value = json!({
                            "kind": "used_namespace",
                            "severity": sev,
                            "subject": "x07AST capability namespaces",
                            "path": rel_str,
                            "added": added,
                            "removed": removed,
                            "notes": []
                        });
                        ctx.push(&mut report.highlights.capability_changes, value);
                    }

                    if ast.budget_scope_changed {
                        let value = json!({
                            "kind": "scope",
                            "severity": "warn",
                            "subject": "budget scopes changed",
                            "path": rel_str,
                            "ptr_before": null,
                            "ptr_after": null,
                            "before": null,
                            "after": null,
                            "notes": ["budget.scope_v1 or budget.scope_from_arch_v1 changed"]
                        });
                        ctx.push(&mut report.highlights.budget_changes, value);
                    }
                }
                "project" => diff_project(
                    &rel_str,
                    before_json.as_ref(),
                    after_json.as_ref(),
                    &mut file_diff,
                    &mut ctx,
                    &mut report.highlights,
                ),
                "arch" => diff_arch(
                    &rel_str,
                    before_json.as_ref(),
                    after_json.as_ref(),
                    &mut file_diff,
                    &mut ctx,
                    &mut report.highlights,
                ),
                "policy" => diff_policy(
                    &rel_str,
                    before_json.as_ref(),
                    after_json.as_ref(),
                    &mut file_diff,
                    &mut ctx,
                    &mut report.highlights,
                ),
                "json" => push_simple_file_change(
                    &mut file_diff,
                    &mut ctx,
                    "json_changed",
                    "info",
                    "JSON file changed",
                ),
                _ => push_simple_file_change(
                    &mut file_diff,
                    &mut ctx,
                    "text_changed",
                    "info",
                    "text file changed",
                ),
            }
        }

        file_diff.truncated = ctx.truncated;
        report.files.push(file_diff);
    }

    report.summary.high_severity_changes = ctx.high_severity_changes;
    report.summary.truncated = ctx.truncated;

    let mut report_value = serde_json::to_value(&report)?;
    let mut diags = report_common::validate_schema(
        X07_REVIEW_DIFF_SCHEMA_BYTES,
        "spec/x07-review.diff.schema.json",
        &report_value,
    )?;

    if !diags.is_empty() {
        report.diags.append(&mut diags);
        report_value = serde_json::to_value(&report)?;
    }

    if let Some(out) = &args.json_out {
        let bytes = report_common::canonical_pretty_json_bytes(&report_value)?;
        util::write_atomic(out, &bytes)
            .with_context(|| format!("write json report: {}", out.display()))?;
    }

    let html = render_html(&report, &from, &to, &args);
    util::write_atomic(&args.html_out, html.as_bytes())
        .with_context(|| format!("write html: {}", args.html_out.display()))?;

    let fail_on_triggered = fail_on_triggered(&report, &args.fail_on);
    if !report.diags.is_empty() || fail_on_triggered {
        return Ok(std::process::ExitCode::from(20));
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn fail_on_triggered(report: &ReviewDiffReport, fail_on: &[ReviewFailOn]) -> bool {
    for flag in fail_on {
        match flag {
            ReviewFailOn::WorldCapability => {
                if !report.highlights.world_changes.is_empty()
                    || !report.highlights.capability_changes.is_empty()
                {
                    return true;
                }
            }
            ReviewFailOn::BudgetIncrease => {
                if report
                    .highlights
                    .budget_changes
                    .iter()
                    .any(budget_change_is_increase)
                {
                    return true;
                }
            }
            ReviewFailOn::AllowUnsafe => {
                if report
                    .highlights
                    .policy_changes
                    .iter()
                    .any(|v| language_toggle_enabled(v, "allow_unsafe"))
                {
                    return true;
                }
            }
            ReviewFailOn::AllowFfi => {
                if report
                    .highlights
                    .policy_changes
                    .iter()
                    .any(|v| language_toggle_enabled(v, "allow_ffi"))
                {
                    return true;
                }
            }
        }
    }
    false
}

fn budget_change_is_increase(v: &Value) -> bool {
    let before = v.get("before");
    let after = v.get("after");
    match (
        before.and_then(Value::as_u64),
        after.and_then(Value::as_u64),
    ) {
        (Some(b), Some(a)) => a > b,
        _ => false,
    }
}

fn language_toggle_enabled(v: &Value, needle: &str) -> bool {
    v.get("kind")
        .and_then(Value::as_str)
        .is_some_and(|k| k == "policy_language_toggle")
        && v.get("subject")
            .and_then(Value::as_str)
            .is_some_and(|s| s.contains(needle))
        && v.get("after").and_then(Value::as_bool).unwrap_or(false)
}

fn path_kind(path: &Path) -> &'static str {
    if path.is_dir() {
        "directory"
    } else if path.is_file() {
        "file"
    } else {
        "path"
    }
}

fn push_simple_file_change(
    file_diff: &mut FileDiff,
    ctx: &mut BuildCtx,
    kind: &str,
    severity: &str,
    title: &str,
) {
    let value = json!({
        "kind": kind,
        "severity": severity,
        "title": title,
        "entity": {},
        "ptr_before": null,
        "ptr_after": null,
        "before": null,
        "after": null,
        "ops": [],
        "notes": [],
        "meta": {}
    });
    ctx.push(&mut file_diff.changes, value);
}

fn classify_file(rel: &str, before: Option<&Value>, after: Option<&Value>) -> &'static str {
    let lower = rel.to_ascii_lowercase();
    if lower.ends_with(".x07.json") {
        return "x07ast";
    }
    if lower == "x07.json" || lower.ends_with(".x07project.json") {
        return "project";
    }
    if lower.contains("/arch/") || lower.starts_with("arch/") {
        return "arch";
    }
    if lower.ends_with(".policy.json") || lower.contains("/.x07/policies/") {
        return "policy";
    }

    if before.is_some() || after.is_some() {
        return "json";
    }
    "text"
}

fn build_globsets(args: &ReviewDiffArgs) -> Result<(GlobSet, GlobSet)> {
    let mut incb = GlobSetBuilder::new();
    let mut excb = GlobSetBuilder::new();

    let default_includes: &[&str] = match args.mode {
        ReviewMode::AstOnly => &["**/*.x07.json"],
        ReviewMode::Project => &[
            "**/*.x07.json",
            "x07.json",
            "**/*.x07project.json",
            "arch/**",
            ".x07/policies/**",
            "**/*.policy.json",
        ],
    };

    if args.include.is_empty() {
        for pat in default_includes {
            incb.add(Glob::new(pat).with_context(|| format!("invalid include glob: {pat:?}"))?);
        }
    } else {
        for pat in &args.include {
            incb.add(Glob::new(pat).with_context(|| format!("invalid include glob: {pat:?}"))?);
        }
    }

    let default_excludes: &[&str] = &[
        "**/.git/**",
        "**/.x07/**/cache/**",
        "**/dist/**",
        "**/gen/**",
        "**/node_modules/**",
        "**/target/**",
        "**/tmp/**",
    ];

    if args.exclude.is_empty() {
        for pat in default_excludes {
            excb.add(Glob::new(pat).with_context(|| format!("invalid exclude glob: {pat:?}"))?);
        }
    } else {
        for pat in &args.exclude {
            excb.add(Glob::new(pat).with_context(|| format!("invalid exclude glob: {pat:?}"))?);
        }
    }

    let include = incb.build().context("build include globset")?;
    let exclude = excb.build().context("build exclude globset")?;
    Ok((include, exclude))
}

fn collect_rel_paths(
    from: &Path,
    to: &Path,
    dirs: bool,
    include: &GlobSet,
    exclude: &GlobSet,
) -> Result<Vec<PathBuf>> {
    if !dirs {
        let name = from
            .file_name()
            .or_else(|| to.file_name())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("file"));
        return Ok(vec![name]);
    }

    let mut out: BTreeSet<PathBuf> = BTreeSet::new();
    for root in [from, to] {
        for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            let rel = p
                .strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            if include.is_match(&rel) && !exclude.is_match(&rel) {
                out.insert(PathBuf::from(rel));
            }
        }
    }
    Ok(out.into_iter().collect())
}

fn read_if_file(path: &Path) -> Option<Vec<u8>> {
    if !path.is_file() {
        return None;
    }
    std::fs::read(path).ok()
}

impl Snapshot {
    fn from_path(path: &Path) -> Self {
        let kind = if path.is_dir() { "dir" } else { "file" };
        let sha256_hex = sha256_path_best_effort(path).ok();
        Self {
            kind,
            path: path.display().to_string(),
            sha256_hex,
            note: String::new(),
        }
    }
}

fn sha256_path_best_effort(path: &Path) -> Result<String> {
    if path.is_file() {
        let bytes = std::fs::read(path).with_context(|| format!("read: {}", path.display()))?;
        return Ok(util::sha256_hex(&bytes));
    }

    let mut entries: Vec<(String, String)> = Vec::new();
    for entry in walkdir::WalkDir::new(path).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let rel = p.strip_prefix(path).unwrap_or(p).display().to_string();
        let bytes = std::fs::read(p).with_context(|| format!("read: {}", p.display()))?;
        entries.push((rel, util::sha256_hex(&bytes)));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut joined: Vec<u8> = Vec::new();
    for (rel, h) in entries {
        joined.extend_from_slice(rel.as_bytes());
        joined.push(0);
        joined.extend_from_slice(h.as_bytes());
        joined.push(b'\n');
    }
    Ok(util::sha256_hex(&joined))
}

fn diff_x07ast(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    ctx: &mut BuildCtx,
    highlights: &mut Highlights,
) -> AstDiffOutcome {
    let mut out = AstDiffOutcome::empty();

    let before_module_id = before
        .and_then(|v| v.get("module_id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let after_module_id = after
        .and_then(|v| v.get("module_id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let module_id = if !after_module_id.is_empty() {
        after_module_id.clone()
    } else {
        before_module_id.clone()
    };

    let before_decls = collect_decls(before);
    let after_decls = collect_decls(after);

    let mut keys: BTreeSet<(String, String)> = BTreeSet::new();
    keys.extend(before_decls.keys().cloned());
    keys.extend(after_decls.keys().cloned());

    let mut decls_added = Vec::new();
    let mut decls_removed = Vec::new();
    let mut decls_changed = Vec::new();

    let mut before_caps_all: BTreeSet<String> = BTreeSet::new();
    let mut after_caps_all: BTreeSet<String> = BTreeSet::new();
    let mut before_scope_count = 0usize;
    let mut after_scope_count = 0usize;

    for decl in before_decls.values() {
        if let Some(body) = &decl.body {
            let scan = report_common::scan_sensitive(body);
            before_caps_all.extend(scan.namespaces);
            before_scope_count += scan.budget_scopes.len();
        }
    }
    for decl in after_decls.values() {
        if let Some(body) = &decl.body {
            let scan = report_common::scan_sensitive(body);
            after_caps_all.extend(scan.namespaces);
            after_scope_count += scan.budget_scopes.len();
        }
    }

    out.capability_added = after_caps_all
        .difference(&before_caps_all)
        .cloned()
        .collect();
    out.capability_removed = before_caps_all
        .difference(&after_caps_all)
        .cloned()
        .collect();
    out.budget_scope_changed = before_scope_count != after_scope_count;

    for key in keys {
        let b = before_decls.get(&key);
        let a = after_decls.get(&key);
        match (b, a) {
            (None, Some(added)) => {
                out.decls_added += 1;
                decls_added.push(json!({"kind": added.kind, "name": added.name}));
                ctx.push(
                    &mut out.changes,
                    json!({
                        "kind": "decl_added",
                        "severity": "info",
                        "title": format!("added {} {}", added.kind, added.name),
                        "entity": {
                            "module_id": module_id,
                            "decl_kind": added.kind,
                            "decl_name": added.name
                        },
                        "ptr_before": null,
                        "ptr_after": added.ptr,
                        "before": null,
                        "after": {
                            "kind": added.kind,
                            "name": added.name,
                            "sig": added.sig
                        },
                        "ops": [{"op":"insert","summary":"declaration inserted","before_ptr":null,"after_ptr":added.ptr,"meta":{}}],
                        "notes": [],
                        "meta": {}
                    }),
                );
            }
            (Some(removed), None) => {
                out.decls_removed += 1;
                decls_removed.push(json!({"kind": removed.kind, "name": removed.name}));
                ctx.push(
                    &mut out.changes,
                    json!({
                        "kind": "decl_removed",
                        "severity": "warn",
                        "title": format!("removed {} {}", removed.kind, removed.name),
                        "entity": {
                            "module_id": module_id,
                            "decl_kind": removed.kind,
                            "decl_name": removed.name
                        },
                        "ptr_before": removed.ptr,
                        "ptr_after": null,
                        "before": {
                            "kind": removed.kind,
                            "name": removed.name,
                            "sig": removed.sig
                        },
                        "after": null,
                        "ops": [{"op":"delete","summary":"declaration removed","before_ptr":removed.ptr,"after_ptr":null,"meta":{}}],
                        "notes": [],
                        "meta": {}
                    }),
                );
            }
            (Some(before_decl), Some(after_decl)) => {
                let sig_changed = before_decl.sig != after_decl.sig;
                let body_changed = before_decl.body != after_decl.body;
                let moved = before_decl.ptr != after_decl.ptr;
                if !sig_changed && !body_changed && !moved {
                    continue;
                }

                out.decls_changed += 1;

                let before_scan = before_decl
                    .body
                    .as_ref()
                    .map(report_common::scan_sensitive)
                    .unwrap_or_default();
                let after_scan = after_decl
                    .body
                    .as_ref()
                    .map(report_common::scan_sensitive)
                    .unwrap_or_default();

                let mut sig_lens = Vec::new();
                if sig_changed {
                    sig_lens.push("signature changed".to_string());
                }
                if moved {
                    sig_lens.push(format!(
                        "declaration moved: {} -> {}",
                        before_decl.ptr, after_decl.ptr
                    ));
                }

                let before_caps: BTreeSet<String> = before_scan.namespaces;
                let after_caps: BTreeSet<String> = after_scan.namespaces;
                let added_caps: Vec<String> = after_caps
                    .difference(&before_caps)
                    .cloned()
                    .collect::<Vec<String>>();
                let removed_caps: Vec<String> = before_caps
                    .difference(&after_caps)
                    .cloned()
                    .collect::<Vec<String>>();

                let mut effects = Vec::new();
                if !added_caps.is_empty() {
                    effects.push(format!(
                        "new sensitive namespaces: {}",
                        added_caps.join(", ")
                    ));
                }
                if !removed_caps.is_empty() {
                    effects.push(format!(
                        "removed sensitive namespaces: {}",
                        removed_caps.join(", ")
                    ));
                }

                let mut budgets = Vec::new();
                let before_budget = before_scan.budget_scopes.len();
                let after_budget = after_scan.budget_scopes.len();
                if before_budget != after_budget {
                    budgets.push(format!(
                        "budget scopes changed: {} -> {}",
                        before_budget, after_budget
                    ));
                }
                if moved {
                    budgets
                        .push("declaration moved (scope binding context may differ)".to_string());
                }

                let mut caps = Vec::new();
                if !added_caps.is_empty() {
                    caps.push(format!("added: {}", added_caps.join(", ")));
                }
                if !removed_caps.is_empty() {
                    caps.push(format!("removed: {}", removed_caps.join(", ")));
                }

                let severity = if added_caps.iter().any(|s| s.starts_with("std.os.")) {
                    "high"
                } else if moved || !added_caps.is_empty() || !budgets.is_empty() || sig_changed {
                    "warn"
                } else {
                    "info"
                };

                let mut ops = Vec::new();
                if moved {
                    ops.push(json!({
                        "op": "move",
                        "summary": "declaration moved",
                        "before_ptr": before_decl.ptr,
                        "after_ptr": after_decl.ptr,
                        "meta": {}
                    }));
                }
                if sig_changed {
                    ops.push(json!({
                        "op": "update",
                        "summary": "signature changed",
                        "before_ptr": before_decl.ptr,
                        "after_ptr": after_decl.ptr,
                        "meta": {}
                    }));
                }
                if body_changed {
                    ops.push(json!({
                        "op": "update",
                        "summary": "function body changed",
                        "before_ptr": format!("{}/body", before_decl.ptr),
                        "after_ptr": format!("{}/body", after_decl.ptr),
                        "meta": {}
                    }));
                }

                decls_changed.push(json!({
                    "kind": after_decl.kind,
                    "name": after_decl.name,
                    "ptr_before": before_decl.ptr,
                    "ptr_after": after_decl.ptr,
                    "sig": {
                        "before": before_decl.sig,
                        "after": after_decl.sig
                    },
                    "body": {
                        "changed": body_changed,
                        "ops": ops,
                        "truncated": false
                    },
                    "lenses": {
                        "signature": sig_lens,
                        "effects": effects,
                        "budgets": budgets,
                        "capabilities": caps
                    }
                }));

                ctx.push(
                    &mut out.changes,
                    json!({
                        "kind": "decl_changed",
                        "severity": severity,
                        "title": format!("changed {} {}", after_decl.kind, after_decl.name),
                        "entity": {
                            "module_id": module_id,
                            "decl_kind": after_decl.kind,
                            "decl_name": after_decl.name
                        },
                        "ptr_before": before_decl.ptr,
                        "ptr_after": after_decl.ptr,
                        "before": {
                            "sig": before_decl.sig
                        },
                        "after": {
                            "sig": after_decl.sig
                        },
                        "ops": ops,
                        "notes": [],
                        "meta": {}
                    }),
                );
            }
            (None, None) => {}
        }
    }

    let status = if before.is_none() {
        "added"
    } else if after.is_none() {
        "removed"
    } else if out.decls_added + out.decls_removed + out.decls_changed > 0 {
        "changed"
    } else {
        "unchanged"
    };

    out.module = Some(json!({
        "module_id": module_id,
        "status": status,
        "decls_added": decls_added,
        "decls_removed": decls_removed,
        "decls_changed": decls_changed,
    }));

    if status != "unchanged" {
        out.modules_changed = 1;
    }

    if before_scope_count != after_scope_count {
        ctx.push(
            &mut highlights.budget_changes,
            json!({
                "kind": "scope",
                "severity": "warn",
                "subject": "x07AST scope count",
                "path": path,
                "ptr_before": null,
                "ptr_after": null,
                "before": before_scope_count,
                "after": after_scope_count,
                "notes": []
            }),
        );
    }

    out
}

fn collect_decls(module: Option<&Value>) -> BTreeMap<(String, String), DeclView> {
    let mut out = BTreeMap::new();
    let Some(module) = module else {
        return out;
    };
    let Some(decls) = module.get("decls").and_then(Value::as_array) else {
        return out;
    };

    for (idx, decl) in decls.iter().enumerate() {
        let Some(obj) = decl.as_object() else {
            continue;
        };
        let Some(kind) = obj.get("kind").and_then(Value::as_str) else {
            continue;
        };

        let base_ptr = format!("/decls/{idx}");

        if kind == "export" {
            if let Some(names) = obj.get("names").and_then(Value::as_array) {
                for (j, name) in names.iter().enumerate() {
                    let Some(name) = name.as_str() else {
                        continue;
                    };
                    let view = DeclView {
                        kind: "export".to_string(),
                        name: name.to_string(),
                        ptr: format!("{base_ptr}/names/{j}"),
                        sig: None,
                        body: None,
                    };
                    out.insert((view.kind.clone(), view.name.clone()), view);
                }
            }
            continue;
        }

        let Some(name) = obj.get("name").and_then(Value::as_str) else {
            continue;
        };

        let sig = if kind == "defn" || kind == "defasync" || kind == "extern" {
            Some(json!({
                "params": obj.get("params").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
                "result": obj.get("result").cloned().unwrap_or(Value::String(String::new()))
            }))
        } else {
            None
        };

        let body = obj.get("body").cloned();
        let view = DeclView {
            kind: kind.to_string(),
            name: name.to_string(),
            ptr: base_ptr,
            sig,
            body,
        };
        out.insert((view.kind.clone(), view.name.clone()), view);
    }

    out
}

fn diff_project(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    file_diff: &mut FileDiff,
    ctx: &mut BuildCtx,
    highlights: &mut Highlights,
) {
    let before_worlds = project_world_map(before);
    let after_worlds = project_world_map(after);

    let mut keys: BTreeSet<String> = BTreeSet::new();
    keys.extend(before_worlds.keys().cloned());
    keys.extend(after_worlds.keys().cloned());

    for profile in keys {
        let b = before_worlds.get(&profile).cloned();
        let a = after_worlds.get(&profile).cloned();
        if b == a {
            continue;
        }

        let severity = match a.as_deref() {
            Some("run-os") | Some("run-os-sandboxed") => "high",
            _ => "warn",
        };

        let world_change = json!({
            "kind": "project_profile_world",
            "severity": severity,
            "subject": profile,
            "path": path,
            "before": b,
            "after": a,
            "notes": []
        });
        ctx.push(&mut highlights.world_changes, world_change.clone());

        ctx.push(
            &mut file_diff.changes,
            json!({
                "kind": "project_world_changed",
                "severity": severity,
                "title": format!("profile {} world changed", profile),
                "entity": {"profile": profile},
                "ptr_before": null,
                "ptr_after": null,
                "before": b,
                "after": a,
                "ops": [],
                "notes": [],
                "meta": {}
            }),
        );
    }

    for field in [
        "solve_fuel",
        "max_memory_bytes",
        "max_output_bytes",
        "cpu_time_limit_seconds",
    ] {
        let before_caps = project_numeric_caps(before, field);
        let after_caps = project_numeric_caps(after, field);
        let mut cap_keys: BTreeSet<String> = BTreeSet::new();
        cap_keys.extend(before_caps.keys().cloned());
        cap_keys.extend(after_caps.keys().cloned());

        for profile in cap_keys {
            let b = before_caps.get(&profile).cloned();
            let a = after_caps.get(&profile).cloned();
            if b == a {
                continue;
            }
            let severity = match (b, a) {
                (Some(bv), Some(av)) if av > bv => "high",
                _ => "info",
            };
            let change = json!({
                "kind": "run_cap",
                "severity": severity,
                "subject": format!("profiles.{}.{}", profile, field),
                "path": path,
                "ptr_before": null,
                "ptr_after": null,
                "before": b,
                "after": a,
                "notes": []
            });
            ctx.push(&mut highlights.budget_changes, change);
        }
    }
}

fn project_world_map(v: Option<&Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(v) = v else {
        return out;
    };

    if let Some(world) = v.get("world").and_then(Value::as_str) {
        out.insert("_project".to_string(), world.to_string());
    }

    if let Some(profiles) = v.get("profiles").and_then(Value::as_object) {
        for (name, profile) in profiles {
            if let Some(world) = profile.get("world").and_then(Value::as_str) {
                out.insert(name.clone(), world.to_string());
            }
        }
    }

    out
}

fn project_numeric_caps(v: Option<&Value>, field: &str) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let Some(v) = v else {
        return out;
    };

    if let Some(raw) = v.get(field).and_then(Value::as_u64) {
        out.insert("_project".to_string(), raw);
    }

    if let Some(profiles) = v.get("profiles").and_then(Value::as_object) {
        for (name, profile) in profiles {
            if let Some(raw) = profile.get(field).and_then(Value::as_u64) {
                out.insert(name.clone(), raw);
            }
        }
    }

    out
}

fn diff_arch(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    file_diff: &mut FileDiff,
    ctx: &mut BuildCtx,
    highlights: &mut Highlights,
) {
    if path.ends_with("manifest.x07arch.json") {
        let before_nodes = arch_node_worlds(before);
        let after_nodes = arch_node_worlds(after);
        let mut keys: BTreeSet<String> = BTreeSet::new();
        keys.extend(before_nodes.keys().cloned());
        keys.extend(after_nodes.keys().cloned());

        for node in keys {
            let b = before_nodes.get(&node).cloned();
            let a = after_nodes.get(&node).cloned();
            if b == a {
                continue;
            }
            let severity = match a.as_deref() {
                Some("run-os") | Some("run-os-sandboxed") => "high",
                _ => "warn",
            };
            let change = json!({
                "kind": "arch_node_world",
                "severity": severity,
                "subject": node,
                "path": path,
                "before": b,
                "after": a,
                "notes": []
            });
            ctx.push(&mut highlights.world_changes, change.clone());
            ctx.push(
                &mut file_diff.changes,
                json!({
                    "kind": "arch_node_world_changed",
                    "severity": severity,
                    "title": "arch node world changed",
                    "entity": {"arch_node_id": node},
                    "ptr_before": null,
                    "ptr_after": null,
                    "before": b,
                    "after": a,
                    "ops": [],
                    "notes": [],
                    "meta": {}
                }),
            );
        }
        return;
    }

    if path.ends_with("index.x07budgets.json") {
        let before_profiles = arch_budget_profiles(before);
        let after_profiles = arch_budget_profiles(after);
        let mut keys: BTreeSet<String> = BTreeSet::new();
        keys.extend(before_profiles.keys().cloned());
        keys.extend(after_profiles.keys().cloned());

        for id in keys {
            let b = before_profiles.get(&id).cloned();
            let a = after_profiles.get(&id).cloned();
            if b == a {
                continue;
            }
            let change = json!({
                "kind": "arch_profile",
                "severity": "warn",
                "subject": id,
                "path": path,
                "ptr_before": null,
                "ptr_after": null,
                "before": b,
                "after": a,
                "notes": []
            });
            ctx.push(&mut highlights.budget_changes, change);
        }
        return;
    }

    push_simple_file_change(
        file_diff,
        ctx,
        "arch_changed",
        "info",
        "architecture JSON changed",
    );
}

fn arch_node_worlds(v: Option<&Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(v) = v else {
        return out;
    };
    let Some(nodes) = v.get("nodes").and_then(Value::as_array) else {
        return out;
    };

    for node in nodes {
        let Some(node_id) = node.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(world) = node.get("world").and_then(Value::as_str) else {
            continue;
        };
        out.insert(node_id.to_string(), world.to_string());
    }

    out
}

fn arch_budget_profiles(v: Option<&Value>) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let Some(v) = v else {
        return out;
    };
    let Some(profiles) = v.get("profiles").and_then(Value::as_array) else {
        return out;
    };

    for profile in profiles {
        let Some(id) = profile.get("id").and_then(Value::as_str) else {
            continue;
        };
        out.insert(
            id.to_string(),
            json!({
                "enforce": profile.get("enforce").cloned().unwrap_or(Value::Null),
                "worlds_allowed": profile
                    .get("worlds_allowed")
                    .cloned()
                    .unwrap_or_else(|| Value::Array(Vec::new()))
            }),
        );
    }
    out
}

fn diff_policy(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    file_diff: &mut FileDiff,
    ctx: &mut BuildCtx,
    highlights: &mut Highlights,
) {
    {
        let mut sinks = PolicyDiffSinks {
            ctx,
            highlights,
            file_diff,
        };
        compare_policy_bool(
            path,
            before,
            after,
            "/fs/enabled",
            "policy_field",
            "policy.fs.enabled",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/language/allow_unsafe",
            "policy_language_toggle",
            "policy.language.allow_unsafe",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/language/allow_ffi",
            "policy_language_toggle",
            "policy.language.allow_ffi",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/net/enabled",
            "policy_field",
            "policy.net.enabled",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/net/allow_dns",
            "policy_field",
            "policy.net.allow_dns",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/net/allow_tcp",
            "policy_field",
            "policy.net.allow_tcp",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/net/allow_udp",
            "policy_field",
            "policy.net.allow_udp",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/env/enabled",
            "policy_field",
            "policy.env.enabled",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/process/enabled",
            "policy_field",
            "policy.process.enabled",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/process/allow_spawn",
            "policy_field",
            "policy.process.allow_spawn",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/process/allow_exec",
            "policy_field",
            "policy.process.allow_exec",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/process/allow_exit",
            "policy_field",
            "policy.process.allow_exit",
            &mut sinks,
        );
        compare_policy_bool(
            path,
            before,
            after,
            "/time/allow_wall_clock",
            "policy_field",
            "policy.time.allow_wall_clock",
            &mut sinks,
        );
        compare_policy_allowlist(
            path,
            before,
            after,
            "/fs/read_roots",
            "policy.fs.read_roots",
            &mut sinks,
        );
        compare_policy_allowlist(
            path,
            before,
            after,
            "/fs/write_roots",
            "policy.fs.write_roots",
            &mut sinks,
        );
        compare_policy_allowlist(
            path,
            before,
            after,
            "/env/allow_keys",
            "policy.env.allow_keys",
            &mut sinks,
        );
        compare_policy_allowlist(
            path,
            before,
            after,
            "/process/allow_execs",
            "policy.process.allow_execs",
            &mut sinks,
        );
        compare_policy_allowlist(
            path,
            before,
            after,
            "/process/allow_exec_prefixes",
            "policy.process.allow_exec_prefixes",
            &mut sinks,
        );
        compare_policy_allowlist(
            path,
            before,
            after,
            "/net/allow_hosts",
            "policy.net.allow_hosts",
            &mut sinks,
        );
    }

    for field in ["cpu_ms", "wall_ms", "mem_bytes", "fds", "procs"] {
        let ptr = format!("/limits/{field}");
        let b = json_pointer_u64(before, &ptr);
        let a = json_pointer_u64(after, &ptr);
        if b == a {
            continue;
        }
        let severity = match (b, a) {
            (Some(bv), Some(av)) if av > bv => "high",
            _ => "warn",
        };
        let change = json!({
            "kind": "policy_limit",
            "severity": severity,
            "subject": format!("policy.limits.{field}"),
            "path": path,
            "ptr_before": ptr,
            "ptr_after": ptr,
            "before": b,
            "after": a,
            "notes": []
        });
        ctx.push(&mut highlights.policy_changes, change.clone());

        let budget = json!({
            "kind": "policy_limit",
            "severity": severity,
            "subject": format!("policy.limits.{field}"),
            "path": path,
            "ptr_before": ptr,
            "ptr_after": ptr,
            "before": b,
            "after": a,
            "notes": []
        });
        ctx.push(&mut highlights.budget_changes, budget);
    }

    let before_declared = declared_policy_caps(before);
    let after_declared = declared_policy_caps(after);
    let added: Vec<String> = after_declared
        .difference(&before_declared)
        .cloned()
        .collect();
    let removed: Vec<String> = before_declared
        .difference(&after_declared)
        .cloned()
        .collect();
    if !added.is_empty() || !removed.is_empty() {
        let severity = if added
            .iter()
            .any(|s| s == "allow_unsafe" || s == "allow_ffi")
        {
            "high"
        } else {
            "warn"
        };
        let value = json!({
            "kind": "declared_policy",
            "severity": severity,
            "subject": "declared policy capabilities",
            "path": path,
            "added": added,
            "removed": removed,
            "notes": []
        });
        ctx.push(&mut highlights.capability_changes, value);
    }

    if file_diff.changes.is_empty() {
        push_simple_file_change(file_diff, ctx, "policy_changed", "warn", "policy changed");
    }
}

struct PolicyDiffSinks<'a> {
    ctx: &'a mut BuildCtx,
    highlights: &'a mut Highlights,
    file_diff: &'a mut FileDiff,
}

fn compare_policy_allowlist(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    ptr: &str,
    subject: &str,
    sinks: &mut PolicyDiffSinks<'_>,
) {
    let b = before
        .and_then(|v| v.pointer(ptr))
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let a = after
        .and_then(|v| v.pointer(ptr))
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    if b == a {
        return;
    }

    let (added, removed) = allowlist_delta(&b, &a);
    let widened = !added.is_empty();
    let severity = if widened
        && matches!(
            subject,
            "policy.net.allow_hosts"
                | "policy.process.allow_execs"
                | "policy.process.allow_exec_prefixes"
                | "policy.fs.write_roots"
        ) {
        "high"
    } else {
        "warn"
    };

    let notes = vec![
        format!("added entries: {}", added.len()),
        format!("removed entries: {}", removed.len()),
    ];

    let value = json!({
        "kind": "policy_allowlist",
        "severity": severity,
        "subject": subject,
        "path": path,
        "ptr_before": ptr,
        "ptr_after": ptr,
        "before": b,
        "after": a,
        "notes": notes
    });
    sinks
        .ctx
        .push(&mut sinks.highlights.policy_changes, value.clone());

    sinks.ctx.push(
        &mut sinks.file_diff.changes,
        json!({
            "kind": "policy_allowlist_changed",
            "severity": severity,
            "title": format!("{} changed", subject),
            "entity": {},
            "ptr_before": ptr,
            "ptr_after": ptr,
            "before": value.get("before").cloned().unwrap_or(Value::Null),
            "after": value.get("after").cloned().unwrap_or(Value::Null),
            "ops": [],
            "notes": value.get("notes").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
            "meta": {}
        }),
    );
}

fn allowlist_delta(before: &Value, after: &Value) -> (Vec<String>, Vec<String>) {
    let before_set = allowlist_entries(before);
    let after_set = allowlist_entries(after);
    let mut added: Vec<String> = after_set.difference(&before_set).cloned().collect();
    let mut removed: Vec<String> = before_set.difference(&after_set).cloned().collect();
    added.sort();
    removed.sort();
    (added, removed)
}

fn allowlist_entries(v: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(items) = v.as_array() else {
        return out;
    };
    for item in items {
        out.insert(canonical_json_compact(item));
    }
    out
}

fn canonical_json_compact(v: &Value) -> String {
    let mut clone = v.clone();
    x07c::x07ast::canon_value_jcs(&mut clone);
    serde_json::to_string(&clone).unwrap_or_else(|_| "null".to_string())
}

fn compare_policy_bool(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    ptr: &str,
    kind: &str,
    subject: &str,
    sinks: &mut PolicyDiffSinks<'_>,
) {
    let b = json_pointer_bool(before, ptr);
    let a = json_pointer_bool(after, ptr);
    if b == a {
        return;
    }

    let high_on_enable = matches!(
        subject,
        "policy.language.allow_unsafe"
            | "policy.language.allow_ffi"
            | "policy.net.enabled"
            | "policy.process.enabled"
    );
    let severity = if a == Some(true) && (kind == "policy_language_toggle" || high_on_enable) {
        "high"
    } else {
        "warn"
    };

    let value = json!({
        "kind": kind,
        "severity": severity,
        "subject": subject,
        "path": path,
        "ptr_before": ptr,
        "ptr_after": ptr,
        "before": b,
        "after": a,
        "notes": []
    });
    sinks
        .ctx
        .push(&mut sinks.highlights.policy_changes, value.clone());

    sinks.ctx.push(
        &mut sinks.file_diff.changes,
        json!({
            "kind": "policy_bool_changed",
            "severity": severity,
            "title": format!("{} changed", subject),
            "entity": {},
            "ptr_before": ptr,
            "ptr_after": ptr,
            "before": b,
            "after": a,
            "ops": [],
            "notes": [],
            "meta": {}
        }),
    );
}

fn json_pointer_bool(v: Option<&Value>, ptr: &str) -> Option<bool> {
    v.and_then(|v| v.pointer(ptr)).and_then(Value::as_bool)
}

fn json_pointer_u64(v: Option<&Value>, ptr: &str) -> Option<u64> {
    v.and_then(|v| v.pointer(ptr)).and_then(Value::as_u64)
}

fn declared_policy_caps(v: Option<&Value>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(v) = v else {
        return out;
    };

    let checks = [
        ("/fs/enabled", "fs"),
        ("/net/enabled", "net"),
        ("/env/enabled", "env"),
        ("/time/enabled", "time"),
        ("/process/enabled", "process"),
        ("/language/allow_unsafe", "allow_unsafe"),
        ("/language/allow_ffi", "allow_ffi"),
    ];

    for (ptr, name) in checks {
        if v.pointer(ptr).and_then(Value::as_bool).unwrap_or(false) {
            out.insert(name.to_string());
        }
    }

    out
}

fn render_html(report: &ReviewDiffReport, from: &Path, to: &Path, args: &ReviewDiffArgs) -> String {
    let mut s = String::new();
    let (risk_label, risk_bg, risk_fg) = review_risk(report);
    s.push_str("<!doctype html>\n<html><head><meta charset=\"utf-8\">");
    s.push_str("<title>x07 review diff</title>");
    s.push_str("<style>body{font-family:system-ui,Segoe UI,Helvetica,Arial,sans-serif;margin:24px;line-height:1.45}code,pre{background:#f6f8fa;padding:2px 4px;border-radius:4px}pre{padding:12px;overflow:auto}details{margin:12px 0}table{border-collapse:collapse}td,th{padding:6px 8px;border:1px solid #ddd}h2{margin-top:28px}.risk{padding:10px 12px;border-radius:8px;font-weight:700;display:inline-block;margin:6px 0 16px}</style>");
    s.push_str("</head><body>");
    s.push_str("<h1>x07 review diff</h1>");
    s.push_str("<p><b>tool:</b> <code>x07 ");
    s.push_str(env!("CARGO_PKG_VERSION"));
    s.push_str("</code> <b>mode:</b> ");
    s.push_str(report.mode);
    s.push_str("</p>");
    s.push_str("<p><b>from:</b> <code>");
    s.push_str(&report_common::html_escape(from.display().to_string()));
    s.push_str("</code><br><b>to:</b> <code>");
    s.push_str(&report_common::html_escape(to.display().to_string()));
    s.push_str("</code></p>");
    s.push_str("<div class=\"risk\" style=\"background:");
    s.push_str(risk_bg);
    s.push_str(";color:");
    s.push_str(risk_fg);
    s.push_str("\">risk summary: ");
    s.push_str(risk_label);
    s.push_str("</div>");

    s.push_str("<h2>Summary</h2><table>");
    for (k, v) in [
        ("files_total", report.summary.files_total.to_string()),
        ("files_changed", report.summary.files_changed.to_string()),
        (
            "modules_changed",
            report.summary.modules_changed.to_string(),
        ),
        ("decls_added", report.summary.decls_added.to_string()),
        ("decls_removed", report.summary.decls_removed.to_string()),
        ("decls_changed", report.summary.decls_changed.to_string()),
        (
            "high_severity_changes",
            report.summary.high_severity_changes.to_string(),
        ),
        ("truncated", report.summary.truncated.to_string()),
    ] {
        s.push_str("<tr><th>");
        s.push_str(k);
        s.push_str("</th><td>");
        s.push_str(&report_common::html_escape(v));
        s.push_str("</td></tr>");
    }
    s.push_str("</table>");

    s.push_str("<h2>High-Signal Deltas</h2>");
    render_change_list(&mut s, "World Changes", &report.highlights.world_changes);
    render_change_list(&mut s, "Budget Changes", &report.highlights.budget_changes);
    render_change_list(&mut s, "Policy Changes", &report.highlights.policy_changes);
    render_change_list(
        &mut s,
        "Capability Changes",
        &report.highlights.capability_changes,
    );

    s.push_str("<h2>File Diffs</h2>");
    for file in &report.files {
        s.push_str("<details>");
        s.push_str("<summary><code>");
        s.push_str(&report_common::html_escape(&file.path));
        s.push_str("</code> [");
        s.push_str(&report_common::html_escape(&file.kind));
        s.push_str("] ");
        s.push_str(&report_common::html_escape(&file.status));
        s.push_str("</summary>");

        if !file.changes.is_empty() {
            s.push_str("<pre>");
            let bytes =
                report_common::canonical_pretty_json_bytes(&Value::Array(file.changes.clone()))
                    .unwrap_or_else(|_| b"[]\n".to_vec());
            s.push_str(&report_common::html_escape(
                String::from_utf8_lossy(&bytes).as_ref(),
            ));
            s.push_str("</pre>");
        }

        if let Some(module) = &file.module {
            let lens_lines = module_lens_lines(module);
            if !lens_lines.is_empty() {
                s.push_str("<h3>Intent lenses</h3><pre>");
                s.push_str(&report_common::html_escape(lens_lines.join("\n")));
                s.push_str("</pre>");
            }
            s.push_str("<pre>");
            let bytes = report_common::canonical_pretty_json_bytes(module)
                .unwrap_or_else(|_| b"{}\n".to_vec());
            s.push_str(&report_common::html_escape(
                String::from_utf8_lossy(&bytes).as_ref(),
            ));
            s.push_str("</pre>");
        }

        s.push_str("</details>");
    }

    if !report.diags.is_empty() {
        s.push_str("<h2>Schema Validation Diagnostics</h2><ul>");
        for d in &report.diags {
            s.push_str("<li><code>");
            s.push_str(&report_common::html_escape(&d.code));
            s.push_str("</code>: ");
            s.push_str(&report_common::html_escape(&d.message));
            s.push_str("</li>");
        }
        s.push_str("</ul>");
    }

    s.push_str("<h2>CLI Options</h2><pre>");
    let options = json!({
        "include": args.include,
        "exclude": args.exclude,
        "max_changes": args.max_changes,
        "fail_on": args
            .fail_on
            .iter()
            .map(|f| match f {
                ReviewFailOn::WorldCapability => "world-capability",
                ReviewFailOn::BudgetIncrease => "budget-increase",
                ReviewFailOn::AllowUnsafe => "allow-unsafe",
                ReviewFailOn::AllowFfi => "allow-ffi",
            })
            .collect::<Vec<&str>>()
    });
    let bytes =
        report_common::canonical_pretty_json_bytes(&options).unwrap_or_else(|_| b"{}\n".to_vec());
    s.push_str(&report_common::html_escape(
        String::from_utf8_lossy(&bytes).as_ref(),
    ));
    s.push_str("</pre>");

    s.push_str("</body></html>\n");
    s
}

fn render_change_list(out: &mut String, title: &str, changes: &[Value]) {
    out.push_str("<details>");
    out.push_str("<summary>");
    out.push_str(&report_common::html_escape(format!(
        "{} ({})",
        title,
        changes.len()
    )));
    out.push_str("</summary>");
    out.push_str("<pre>");
    let bytes = report_common::canonical_pretty_json_bytes(&Value::Array(changes.to_vec()))
        .unwrap_or_else(|_| b"[]\n".to_vec());
    out.push_str(&report_common::html_escape(
        String::from_utf8_lossy(&bytes).as_ref(),
    ));
    out.push_str("</pre></details>");
}

fn review_risk(report: &ReviewDiffReport) -> (&'static str, &'static str, &'static str) {
    if report.summary.high_severity_changes > 0 {
        return ("high", "#fee2e2", "#991b1b");
    }
    let has_warn = report
        .highlights
        .world_changes
        .iter()
        .chain(report.highlights.budget_changes.iter())
        .chain(report.highlights.policy_changes.iter())
        .chain(report.highlights.capability_changes.iter())
        .any(|v| {
            v.get("severity")
                .and_then(Value::as_str)
                .is_some_and(|sev| sev == "warn" || sev == "high")
        });
    if has_warn {
        ("medium", "#fef3c7", "#92400e")
    } else {
        ("low", "#dcfce7", "#166534")
    }
}

fn module_lens_lines(module: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    let Some(changed) = module.get("decls_changed").and_then(Value::as_array) else {
        return lines;
    };
    for decl in changed {
        let name = decl
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let Some(lenses) = decl.get("lenses") else {
            continue;
        };
        let mut decl_lines = Vec::new();
        for key in ["signature", "effects", "budgets", "capabilities"] {
            let entries = lenses
                .get(key)
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();
            if !entries.is_empty() {
                decl_lines.push(format!("  {key}: {}", entries.join("; ")));
            }
        }
        if !decl_lines.is_empty() {
            lines.push(format!("{name}:"));
            lines.extend(decl_lines);
        }
    }
    lines
}
