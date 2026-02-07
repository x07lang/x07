use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result};
use base64::Engine;
use serde::Serialize;
use serde_json::Value;
use x07c::diagnostics;

use crate::util;

const TOOL_API_CHILD_ENV: &str = "X07_TOOL_API_CHILD";
const X07_TOOL_REPORT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-tool.report.schema.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsonMode {
    Off,
    Canon,
    Pretty,
}

#[derive(Debug, Clone)]
struct ParsedMachineFlags {
    mode: JsonMode,
    jsonl: bool,
    json_schema: bool,
    json_schema_id: bool,
    report_out: Option<PathBuf>,
    quiet_json: bool,
    passthrough: Vec<OsString>,
    parse_errors: Vec<String>,
    saw_any: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ToolReportMeta {
    tool: ToolVersionMeta,
    elapsed_ms: u64,
    cwd: String,
    argv: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolVersionMeta {
    name: &'static str,
    version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    git_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct StreamPayload {
    bytes_len: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base64: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolResultPayload {
    stdout: StreamPayload,
    stderr: StreamPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout_json: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr_json: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolReport {
    schema_version: String,
    command: String,
    ok: bool,
    exit_code: u8,
    diagnostics: Vec<diagnostics::Diagnostic>,
    meta: ToolReportMeta,
    result: ToolResultPayload,
}

pub(crate) fn maybe_handle(raw_args: &[OsString]) -> Result<Option<std::process::ExitCode>> {
    if std::env::var_os(TOOL_API_CHILD_ENV).is_some() {
        return Ok(None);
    }
    if raw_args.is_empty() {
        return Ok(None);
    }

    let parsed = parse_machine_flags(&raw_args[1..]);
    if !parsed.saw_any {
        return Ok(None);
    }

    let scope = detect_scope(&parsed.passthrough).unwrap_or_else(|| "root".to_string());
    let has_native_json = has_native_json_mode(&scope);
    let has_native_schema = has_native_json_schema(&scope);

    let wants_schema = parsed.json_schema || parsed.json_schema_id;
    let wants_json = parsed.mode != JsonMode::Off || parsed.jsonl;
    let needs_wrapper = if wants_schema {
        !has_native_schema
            || parsed.mode != JsonMode::Off
            || parsed.jsonl
            || parsed.quiet_json
            || parsed.report_out.is_some()
    } else if wants_json {
        if parsed.mode == JsonMode::Pretty
            || parsed.jsonl
            || parsed.quiet_json
            || parsed.report_out.is_some()
        {
            true
        } else {
            !has_native_json
        }
    } else {
        parsed.report_out.is_some() || parsed.quiet_json || parsed.jsonl
    };

    if !needs_wrapper {
        return Ok(None);
    }

    let started = Instant::now();

    if parsed.json_schema_id {
        let schema_id = schema_id_for_scope(&scope);
        if parsed.json_schema {
            let schema = command_schema(&scope)?;
            let bytes = encode_json_bytes(&schema, parsed.mode)?;
            if let Some(path) = &parsed.report_out {
                write_bytes(path, &bytes)?;
            }
            if !parsed.quiet_json {
                std::io::Write::write_all(&mut std::io::stdout(), &bytes)
                    .context("write stdout")?;
            }
            return Ok(Some(std::process::ExitCode::SUCCESS));
        }

        let mut out = schema_id.into_bytes();
        out.push(b'\n');
        if let Some(path) = &parsed.report_out {
            write_bytes(path, &out)?;
        }
        if !parsed.quiet_json {
            std::io::Write::write_all(&mut std::io::stdout(), &out).context("write stdout")?;
        }
        return Ok(Some(std::process::ExitCode::SUCCESS));
    }

    if parsed.json_schema {
        let schema = command_schema(&scope)?;
        let bytes = encode_json_bytes(&schema, parsed.mode)?;
        if let Some(path) = &parsed.report_out {
            write_bytes(path, &bytes)?;
        }
        if !parsed.quiet_json {
            std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;
        }
        return Ok(Some(std::process::ExitCode::SUCCESS));
    }

    if parsed.parse_errors.is_empty() {
        let report = run_wrapped_command(raw_args, &parsed, &scope, started)?;
        let code = report.exit_code;
        emit_report(&parsed, &report)?;
        return Ok(Some(std::process::ExitCode::from(code)));
    }

    let mut diags = Vec::new();
    for msg in &parsed.parse_errors {
        diags.push(diag_error(
            "X07-TOOL-ARGS-0001",
            diagnostics::Stage::Parse,
            msg,
        ));
    }
    let report = error_report(raw_args, &scope, started, 2, diags);
    emit_report(&parsed, &report)?;
    Ok(Some(std::process::ExitCode::from(2)))
}

fn run_wrapped_command(
    raw_args: &[OsString],
    parsed: &ParsedMachineFlags,
    scope: &str,
    started: Instant,
) -> Result<ToolReport> {
    let exe = std::env::current_exe().context("resolve current executable")?;
    let output = Command::new(exe)
        .args(&parsed.passthrough)
        .env(TOOL_API_CHILD_ENV, "1")
        .output()
        .context("run wrapped command")?;

    let exit_code = output.status.code().unwrap_or(3).clamp(0, 255) as u8;
    let ok = exit_code == 0;

    let stdout_payload = stream_payload(&output.stdout);
    let stderr_payload = stream_payload(&output.stderr);
    let stdout_json = parse_json_bytes(&output.stdout);
    let stderr_json = parse_json_bytes(&output.stderr);

    let mut diagnostics = extract_diagnostics(stdout_json.as_ref())
        .or_else(|| extract_diagnostics(stderr_json.as_ref()));
    if !ok && diagnostics.is_none() {
        let msg = if let Some(text) = stderr_payload.text.as_deref() {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                format!("wrapped command failed with exit code {exit_code}")
            } else {
                trimmed.to_string()
            }
        } else {
            format!("wrapped command failed with exit code {exit_code}")
        };
        diagnostics = Some(vec![diag_error(
            "X07-TOOL-EXEC-0001",
            diagnostics::Stage::Run,
            &msg,
        )]);
    }

    Ok(ToolReport {
        schema_version: schema_id_for_scope(scope),
        command: command_id_for_scope(scope),
        ok,
        exit_code,
        diagnostics: diagnostics.unwrap_or_default(),
        meta: ToolReportMeta {
            tool: ToolVersionMeta {
                name: "x07",
                version: env!("CARGO_PKG_VERSION"),
                git_sha: std::env::var("X07_GIT_SHA").ok(),
            },
            elapsed_ms: started.elapsed().as_millis() as u64,
            cwd: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .display()
                .to_string(),
            argv: raw_args.iter().map(os_to_string).collect(),
        },
        result: ToolResultPayload {
            stdout: stdout_payload,
            stderr: stderr_payload,
            stdout_json,
            stderr_json,
        },
    })
}

fn emit_report(parsed: &ParsedMachineFlags, report: &ToolReport) -> Result<()> {
    let report_value = serde_json::to_value(report)?;
    let out = if parsed.jsonl {
        let mut bytes = util::canonical_jcs_bytes(&report_value)?;
        if bytes.last() != Some(&b'\n') {
            bytes.push(b'\n');
        }
        bytes
    } else {
        encode_json_bytes(&report_value, parsed.mode)?
    };

    if let Some(path) = &parsed.report_out {
        write_bytes(path, &out)?;
    }
    if !parsed.quiet_json {
        std::io::Write::write_all(&mut std::io::stdout(), &out).context("write stdout")?;
    }
    Ok(())
}

fn command_schema(scope: &str) -> Result<Value> {
    let mut schema: Value =
        serde_json::from_slice(X07_TOOL_REPORT_SCHEMA_BYTES).context("parse tool report schema")?;
    let schema_id = schema_id_for_scope(scope);
    let command_id = command_id_for_scope(scope);
    let schema_url = schema_url_for_scope(scope);

    let obj = schema
        .as_object_mut()
        .context("tool report schema must be an object")?;
    obj.insert("$id".to_string(), Value::String(schema_url));
    obj.insert("title".to_string(), Value::String(schema_id.clone()));

    if let Some(props) = obj.get_mut("properties").and_then(Value::as_object_mut) {
        if let Some(v) = props
            .get_mut("schema_version")
            .and_then(Value::as_object_mut)
        {
            v.insert("const".to_string(), Value::String(schema_id));
        }
        if let Some(v) = props.get_mut("command").and_then(Value::as_object_mut) {
            v.insert("const".to_string(), Value::String(command_id));
        }
    }

    Ok(schema)
}

fn parse_machine_flags(tokens: &[OsString]) -> ParsedMachineFlags {
    let mut mode = JsonMode::Off;
    let mut jsonl = false;
    let mut json_schema = false;
    let mut json_schema_id = false;
    let mut report_out = None;
    let mut quiet_json = false;
    let mut parse_errors = Vec::new();
    let mut passthrough = Vec::new();
    let mut saw_any = false;

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

        passthrough.push(token.clone());
        i += 1;
    }

    if quiet_json && report_out.is_none() {
        parse_errors.push("--quiet-json requires --report-out <PATH>".to_string());
    }
    if mode != JsonMode::Off && jsonl {
        parse_errors.push("--jsonl conflicts with --json=<mode>".to_string());
    }

    ParsedMachineFlags {
        mode,
        jsonl,
        json_schema,
        json_schema_id,
        report_out,
        quiet_json,
        passthrough,
        parse_errors,
        saw_any,
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

fn detect_scope(tokens: &[OsString]) -> Option<String> {
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
            | "run"
            | "bundle"
            | "guide"
            | "doctor"
            | "diag"
            | "policy"
            | "ast"
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
    )
}

fn nested_commands(scope: &str) -> &'static [&'static str] {
    match scope {
        "arch" => &["check"],
        "ast" => &[
            "init",
            "get",
            "apply-patch",
            "validate",
            "canon",
            "schema",
            "grammar",
        ],
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

fn has_native_json_mode(scope: &str) -> bool {
    matches!(
        scope,
        "diag.explain"
            | "doc"
            | "fmt"
            | "lint"
            | "fix"
            | "pkg.provides"
            | "schema.derive"
            | "sm.check"
            | "test"
            | "patch.apply"
    )
}

fn has_native_json_schema(scope: &str) -> bool {
    matches!(scope, "ast.schema")
}

fn schema_id_for_scope(scope: &str) -> String {
    if scope == "root" {
        return "x07.root.report@0.1.0".to_string();
    }
    format!("x07.{scope}.report@0.1.0")
}

fn command_id_for_scope(scope: &str) -> String {
    if scope == "root" {
        return "x07".to_string();
    }
    format!("x07.{scope}")
}

fn schema_url_for_scope(scope: &str) -> String {
    let suffix = if scope == "root" {
        "x07-root.report.schema.json".to_string()
    } else {
        format!("x07-{}.report.schema.json", scope.replace('.', "-"))
    };
    format!("https://x07.io/spec/{suffix}")
}

fn parse_json_bytes(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}

fn extract_diagnostics(doc: Option<&Value>) -> Option<Vec<diagnostics::Diagnostic>> {
    let doc = doc?;
    let obj = doc.as_object()?;
    for key in ["diagnostics", "diags"] {
        let Some(v) = obj.get(key) else { continue };
        let Some(arr) = v.as_array() else { continue };
        if arr.is_empty() {
            return Some(Vec::new());
        }
        let mut diags = Vec::new();
        for entry in arr {
            let Some(obj) = entry.as_object() else {
                continue;
            };
            let code = obj
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("X07-TOOL-EXEC-0001");
            let message = obj
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("wrapped command diagnostic");
            let stage = match obj.get("stage").and_then(Value::as_str) {
                Some("parse") => diagnostics::Stage::Parse,
                Some("lint") => diagnostics::Stage::Lint,
                Some("rewrite") => diagnostics::Stage::Rewrite,
                Some("type") => diagnostics::Stage::Type,
                Some("lower") => diagnostics::Stage::Lower,
                Some("codegen") => diagnostics::Stage::Codegen,
                Some("link") => diagnostics::Stage::Link,
                _ => diagnostics::Stage::Run,
            };
            let severity = match obj.get("severity").and_then(Value::as_str) {
                Some("warning") => diagnostics::Severity::Warning,
                Some("info") => diagnostics::Severity::Info,
                Some("hint") => diagnostics::Severity::Hint,
                _ => diagnostics::Severity::Error,
            };
            diags.push(diagnostics::Diagnostic {
                code: code.to_string(),
                severity,
                stage,
                message: message.to_string(),
                loc: None,
                notes: Vec::new(),
                related: Vec::new(),
                data: BTreeMap::new(),
                quickfix: None,
            });
        }
        if !diags.is_empty() {
            return Some(diags);
        }
    }
    None
}

fn stream_payload(bytes: &[u8]) -> StreamPayload {
    let text = String::from_utf8(bytes.to_vec()).ok();
    let base64 = if text.is_none() && !bytes.is_empty() {
        Some(base64::engine::general_purpose::STANDARD.encode(bytes))
    } else {
        None
    };
    StreamPayload {
        bytes_len: bytes.len(),
        text,
        base64,
    }
}

fn encode_json_bytes(value: &Value, mode: JsonMode) -> Result<Vec<u8>> {
    match mode {
        JsonMode::Pretty => {
            let mut v = value.clone();
            x07c::x07ast::canon_value_jcs(&mut v);
            let mut out = serde_json::to_vec_pretty(&v)?;
            if out.last() != Some(&b'\n') {
                out.push(b'\n');
            }
            Ok(out)
        }
        JsonMode::Off | JsonMode::Canon => {
            let mut out = util::canonical_jcs_bytes(value)?;
            if out.last() != Some(&b'\n') {
                out.push(b'\n');
            }
            Ok(out)
        }
    }
}

fn write_bytes(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create report-out dir: {}", parent.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("write report-out: {}", path.display()))
}

fn error_report(
    raw_args: &[OsString],
    scope: &str,
    started: Instant,
    exit_code: u8,
    diagnostics: Vec<diagnostics::Diagnostic>,
) -> ToolReport {
    ToolReport {
        schema_version: schema_id_for_scope(scope),
        command: command_id_for_scope(scope),
        ok: false,
        exit_code,
        diagnostics,
        meta: ToolReportMeta {
            tool: ToolVersionMeta {
                name: "x07",
                version: env!("CARGO_PKG_VERSION"),
                git_sha: std::env::var("X07_GIT_SHA").ok(),
            },
            elapsed_ms: started.elapsed().as_millis() as u64,
            cwd: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .display()
                .to_string(),
            argv: raw_args.iter().map(os_to_string).collect(),
        },
        result: ToolResultPayload {
            stdout: StreamPayload {
                bytes_len: 0,
                text: None,
                base64: None,
            },
            stderr: StreamPayload {
                bytes_len: 0,
                text: None,
                base64: None,
            },
            stdout_json: None,
            stderr_json: None,
        },
    }
}

fn diag_error(code: &str, stage: diagnostics::Stage, message: &str) -> diagnostics::Diagnostic {
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

fn os_to_string(v: &OsString) -> String {
    match v.to_string_lossy() {
        Cow::Borrowed(s) => s.to_string(),
        Cow::Owned(s) => s,
    }
}
