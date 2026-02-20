use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use x07c::diagnostics;

use crate::util;

pub const TOOL_EVENTS_SCHEMA_VERSION: &str = "x07.tool.events@0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonMode {
    Off,
    Canon,
    Pretty,
    Jsonl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum JsonArg {
    #[clap(alias = "true")]
    Canon,
    Pretty,
    #[clap(alias = "false")]
    Off,
}

impl JsonArg {
    pub fn to_mode(self) -> JsonMode {
        match self {
            JsonArg::Canon => JsonMode::Canon,
            JsonArg::Pretty => JsonMode::Pretty,
            JsonArg::Off => JsonMode::Off,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct MachineArgs {
    /// Redirect the command's primary output to a file so stdout is available for `--json`.
    #[arg(long, global = true, value_name = "PATH")]
    pub out: Option<PathBuf>,

    /// Emit machine-readable JSON report to stdout.
    ///
    /// Supported modes:
    /// - `--json` / `--json=canon` (canonical JSON; default)
    /// - `--json=pretty` (pretty-printed)
    /// - `--json=off` (disable; accepts `--json=false` as an alias)
    #[arg(
        long,
        global = true,
        num_args(0..=1),
        default_missing_value = "canon",
        value_enum,
        value_name = "MODE"
    )]
    pub json: Option<JsonArg>,

    /// Emit newline-delimited JSON events (streaming).
    #[arg(long, global = true, conflicts_with = "json")]
    pub jsonl: bool,

    /// Print the JSON Schema for the selected scope and exit 0.
    #[arg(long, global = true)]
    pub json_schema: bool,

    /// Print the schema id/version string for the selected scope and exit 0.
    #[arg(long, global = true)]
    pub json_schema_id: bool,

    /// Write the JSON report to a file (in addition to stdout unless `--quiet-json` is set).
    #[arg(long, global = true, value_name = "PATH")]
    pub report_out: Option<PathBuf>,

    /// In `--json` mode, suppress writing the report to stdout and only write to `--report-out`.
    #[arg(long, global = true, requires = "report_out", conflicts_with = "jsonl")]
    pub quiet_json: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedMachineFlags {
    pub mode: JsonMode,
    pub json_schema: bool,
    pub json_schema_id: bool,
    pub report_out: Option<PathBuf>,
    pub quiet_json: bool,
    pub out: Option<PathBuf>,
    pub passthrough: Vec<OsString>,
    pub saw_any: bool,
    pub parse_errors: Vec<String>,
}

pub fn parse_machine_flags(tokens: &[OsString]) -> ParsedMachineFlags {
    let mut mode = JsonMode::Off;
    let mut jsonl = false;
    let mut json_schema = false;
    let mut json_schema_id = false;
    let mut report_out = None;
    let mut quiet_json = false;
    let mut out = None;
    let mut passthrough = Vec::new();
    let mut saw_any = false;
    let mut parse_errors = Vec::new();

    let mut i = 0usize;
    while i < tokens.len() {
        let token = &tokens[i];
        let s = token.to_string_lossy();

        if s == "--json" {
            saw_any = true;
            if let Some(next) = tokens.get(i + 1).map(|v| v.to_string_lossy()) {
                if !next.starts_with('-') {
                    match parse_json_mode_value(next.as_ref()) {
                        Ok(m) => {
                            mode = m;
                            i += 2;
                            continue;
                        }
                        Err(_) => {
                            mode = JsonMode::Canon;
                            i += 1;
                            continue;
                        }
                    }
                }
            }
            mode = JsonMode::Canon;
            i += 1;
            continue;
        }
        if let Some(raw) = s.strip_prefix("--json=") {
            saw_any = true;
            match parse_json_mode_value(raw) {
                Ok(m) => mode = m,
                Err(err) => parse_errors.push(err),
            }
            i += 1;
            continue;
        }
        if s == "--report-json" {
            saw_any = true;
            if let Some(next) = tokens.get(i + 1).map(|v| v.to_string_lossy()) {
                if !next.starts_with('-') {
                    match next.trim() {
                        "" | "true" => {
                            mode = JsonMode::Canon;
                            i += 2;
                            continue;
                        }
                        "false" => {
                            mode = JsonMode::Off;
                            i += 2;
                            continue;
                        }
                        _ => {}
                    }
                }
            }
            mode = JsonMode::Canon;
            i += 1;
            continue;
        }
        if let Some(raw) = s.strip_prefix("--report-json=") {
            saw_any = true;
            match raw.trim() {
                "" | "true" => mode = JsonMode::Canon,
                "false" => mode = JsonMode::Off,
                other => parse_errors.push(format!(
                    "unsupported --report-json value {other:?}; expected one of: true,false"
                )),
            }
            i += 1;
            continue;
        }
        if s == "--jsonl" {
            saw_any = true;
            jsonl = true;
            i += 1;
            continue;
        }
        if s == "--json-schema" {
            saw_any = true;
            json_schema = true;
            i += 1;
            continue;
        }
        if s == "--json-schema-id" {
            saw_any = true;
            json_schema_id = true;
            i += 1;
            continue;
        }
        if s == "--quiet-json" {
            saw_any = true;
            quiet_json = true;
            i += 1;
            continue;
        }
        if s == "--report-out" {
            saw_any = true;
            if let Some(path) = tokens.get(i + 1) {
                report_out = Some(PathBuf::from(path));
                i += 2;
                continue;
            }
            parse_errors.push("--report-out requires a path".to_string());
            i += 1;
            continue;
        }
        if let Some(path) = s.strip_prefix("--report-out=") {
            saw_any = true;
            report_out = Some(PathBuf::from(path));
            i += 1;
            continue;
        }

        if s == "--out" {
            if let Some(path) = tokens.get(i + 1) {
                out = Some(PathBuf::from(path));
                passthrough.push(token.clone());
                passthrough.push(path.clone());
                i += 2;
                continue;
            }
            parse_errors.push("--out requires a path".to_string());
            passthrough.push(token.clone());
            i += 1;
            continue;
        }
        if let Some(path) = s.strip_prefix("--out=") {
            out = Some(PathBuf::from(path));
            passthrough.push(token.clone());
            i += 1;
            continue;
        }

        passthrough.push(token.clone());
        i += 1;
    }

    if quiet_json && report_out.is_none() {
        parse_errors.push("--quiet-json requires --report-out <PATH>".to_string());
    }
    if mode != JsonMode::Off && jsonl {
        parse_errors.push("--jsonl conflicts with --json=<mode>".to_string());
    }

    if jsonl {
        mode = JsonMode::Jsonl;
    }

    ParsedMachineFlags {
        mode,
        json_schema,
        json_schema_id,
        report_out,
        quiet_json,
        out,
        passthrough,
        saw_any,
        parse_errors,
    }
}

fn parse_json_mode_value(raw: &str) -> Result<JsonMode, String> {
    match raw.trim() {
        "" | "true" | "canon" => Ok(JsonMode::Canon),
        "false" | "off" => Ok(JsonMode::Off),
        "pretty" => Ok(JsonMode::Pretty),
        other => Err(format!(
            "unsupported --json value {other:?}; expected one of: true,false,canon,pretty,off"
        )),
    }
}

pub fn detect_scope(tokens: &[OsString]) -> Option<String> {
    let top = first_positional(tokens)?;
    let top_cmd = top.to_string();
    if !is_top_level_command(&top_cmd) {
        return None;
    }
    let mut scope = top_cmd;

    if let Some(next) = next_positional(tokens, &scope, 1) {
        if nested_commands(&scope).contains(&next.as_str()) {
            scope = format!("{scope}.{next}");
            if let Some(third) = next_positional(tokens, &scope, 2) {
                if nested_commands(&scope).contains(&third.as_str()) {
                    scope = format!("{scope}.{third}");
                }
            }
        }
    }

    Some(scope)
}

fn first_positional(tokens: &[OsString]) -> Option<String> {
    let mut idx = 0usize;
    while idx < tokens.len() {
        let s = tokens[idx].to_string_lossy();
        if !s.starts_with('-') {
            return Some(s.to_string());
        }
        idx += 1;
    }
    None
}

fn next_positional(
    tokens: &[OsString],
    _scope: &str,
    ordinal_after_first: usize,
) -> Option<String> {
    let mut seen = 0usize;
    for tok in tokens {
        let s = tok.to_string_lossy();
        if s.starts_with('-') {
            continue;
        }
        if seen == 0 {
            seen += 1;
            continue;
        }
        if seen == ordinal_after_first {
            return Some(s.to_string());
        }
        seen += 1;
    }
    None
}

fn is_top_level_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "init"
            | "test"
            | "bench"
            | "arch"
            | "assets"
            | "run"
            | "bundle"
            | "guide"
            | "doctor"
            | "diag"
            | "policy"
            | "ast"
            | "agent"
            | "fmt"
            | "lint"
            | "fix"
            | "build"
            | "cli"
            | "pkg"
            | "review"
            | "trust"
            | "doc"
            | "schema"
            | "sm"
            | "rr"
            | "patch"
            | "verify"
            | "mcp"
    )
}

fn nested_commands(scope: &str) -> &'static [&'static str] {
    match scope {
        "arch" => &["check"],
        "assets" => &["embed-dir"],
        "ast" => &[
            "init",
            "get",
            "slice",
            "apply-patch",
            "validate",
            "canon",
            "schema",
            "grammar",
        ],
        "agent" => &["context"],
        "bench" => &["list", "validate", "eval"],
        "cli" => &["spec"],
        "cli.spec" => &["fmt", "check", "compile"],
        "diag" => &[
            "catalog",
            "init-catalog",
            "explain",
            "check",
            "coverage",
            "sarif",
        ],
        "pkg" => &[
            "add", "remove", "versions", "pack", "lock", "provides", "login", "publish",
        ],
        "policy" => &["init"],
        "review" => &["diff"],
        "trust" => &["report"],
        "schema" => &["derive"],
        "sm" => &["check", "gen"],
        "rr" => &["record"],
        "patch" => &["apply"],
        _ => &[],
    }
}

pub fn schema_id_for_scope(scope: Option<&str>, report_semver: &str) -> String {
    match scope {
        None => format!("x07.tool.root.report@{report_semver}"),
        Some(scope) => format!("x07.tool.{scope}.report@{report_semver}"),
    }
}

pub fn command_id_for_scope(scope: Option<&str>) -> String {
    match scope {
        None => "x07".to_string(),
        Some(scope) => format!("x07.{scope}"),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolMeta {
    pub tool: ToolVersionMeta,
    pub elapsed_ms: u64,
    pub cwd: String,
    pub argv: Vec<String>,
    pub inputs: Vec<FileDigest>,
    pub outputs: Vec<FileDigest>,
    pub nondeterminism: Nondeterminism,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolVersionMeta {
    pub name: &'static str,
    pub version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rustc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileDigest {
    pub path: String,
    pub sha256: String,
    pub bytes_len: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Nondeterminism {
    pub uses_os_time: bool,
    pub uses_network: bool,
    pub uses_process: bool,
}

pub fn file_digest(path: &Path) -> Result<FileDigest> {
    let mut f = std::fs::File::open(path).with_context(|| format!("open: {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = std::io::Read::read(&mut f, &mut buf)
            .with_context(|| format!("read: {}", path.display()))?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        hasher.update(&buf[..n]);
    }
    Ok(FileDigest {
        path: path.display().to_string(),
        sha256: util::hex_lower(&hasher.finalize()),
        bytes_len: total,
    })
}

pub fn tool_meta(started: Instant, raw_argv: &[OsString]) -> ToolMeta {
    ToolMeta {
        tool: ToolVersionMeta {
            name: "x07",
            version: env!("CARGO_PKG_VERSION"),
            git_sha: std::env::var("X07_GIT_SHA").ok(),
            rustc: std::env::var("X07_RUSTC_VERSION").ok(),
        },
        elapsed_ms: started.elapsed().as_millis() as u64,
        cwd: std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .display()
            .to_string(),
        argv: raw_argv.iter().map(os_to_string).collect(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nondeterminism: Nondeterminism::default(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolReport<T> {
    pub schema_version: String,
    pub command: String,
    pub ok: bool,
    pub exit_code: u8,
    pub diagnostics: Vec<diagnostics::Diagnostic>,
    pub meta: ToolMeta,
    pub result: T,
}

pub fn diag_error(code: &str, stage: diagnostics::Stage, message: &str) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic {
        code: code.to_string(),
        severity: diagnostics::Severity::Error,
        stage,
        message: message.to_string(),
        loc: None,
        notes: Vec::new(),
        related: Vec::new(),
        data: BTreeMap::new(),
        quickfix: None,
    }
}

#[derive(Debug, Clone, Default)]
pub struct MetaDelta {
    pub inputs: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
    pub nondeterminism: Nondeterminism,
}

#[allow(clippy::too_many_arguments)]
pub fn build_report<T: Serialize>(
    scope: Option<&str>,
    report_semver: &str,
    started: Instant,
    raw_argv: &[OsString],
    exit_code: u8,
    diagnostics: Vec<diagnostics::Diagnostic>,
    result: T,
    meta_delta: MetaDelta,
) -> ToolReport<T> {
    let ok = exit_code == 0
        && diagnostics
            .iter()
            .all(|d| d.severity != diagnostics::Severity::Error);

    let mut meta = tool_meta(started, raw_argv);
    meta.nondeterminism = meta_delta.nondeterminism;
    for p in meta_delta.inputs {
        if let Ok(d) = file_digest(&p) {
            meta.inputs.push(d);
        }
    }
    for p in meta_delta.outputs {
        if let Ok(d) = file_digest(&p) {
            meta.outputs.push(d);
        }
    }

    ToolReport {
        schema_version: schema_id_for_scope(scope, report_semver),
        command: command_id_for_scope(scope),
        ok,
        exit_code,
        diagnostics,
        meta,
        result,
    }
}

pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut out = util::canonical_jcs_bytes(value)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

pub fn canonical_pretty_json_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut v = value.clone();
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_vec_pretty(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

pub fn encode_json_bytes(value: &Value, mode: JsonMode) -> Result<Vec<u8>> {
    match mode {
        JsonMode::Pretty => canonical_pretty_json_bytes(value),
        JsonMode::Off | JsonMode::Canon | JsonMode::Jsonl => canonical_json_bytes(value),
    }
}

pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("write: {}", path.display()))
}

pub fn emit_report_bytes(
    report_bytes: &[u8],
    report_out: Option<&Path>,
    quiet_json: bool,
) -> Result<()> {
    if let Some(path) = report_out {
        write_bytes(path, report_bytes)?;
    }
    if !quiet_json {
        std::io::Write::write_all(&mut std::io::stdout(), report_bytes).context("write stdout")?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolEvent {
    pub schema_version: &'static str,
    pub command: String,
    pub seq: u64,
    pub elapsed_ms: u64,
    pub event: String,
    pub data: Value,
}

pub struct JsonlEmitter {
    started: Instant,
    seq: u64,
    stdout: std::io::Stdout,
}

impl JsonlEmitter {
    pub fn new(started: Instant) -> Self {
        Self {
            started,
            seq: 0,
            stdout: std::io::stdout(),
        }
    }

    pub fn emit(&mut self, command: &str, event: &str, data: Value) -> Result<()> {
        let ev = ToolEvent {
            schema_version: TOOL_EVENTS_SCHEMA_VERSION,
            command: command.to_string(),
            seq: self.seq,
            elapsed_ms: self.started.elapsed().as_millis() as u64,
            event: event.to_string(),
            data,
        };
        self.seq = self.seq.saturating_add(1);

        let bytes = canonical_json_bytes(&serde_json::to_value(ev)?)?;
        let mut lock = self.stdout.lock();
        lock.write_all(&bytes).context("write stdout")?;
        lock.flush().ok();
        Ok(())
    }
}

fn os_to_string(v: &OsString) -> String {
    match v.to_string_lossy() {
        Cow::Borrowed(s) => s.to_string(),
        Cow::Owned(s) => s,
    }
}
