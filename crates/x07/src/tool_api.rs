use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Read as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{Context, Result};
use base64::Engine;
use serde::Serialize;
use serde_json::Value;
use x07c::diagnostics;

use crate::reporting;

const TOOL_API_CHILD_ENV: &str = "X07_TOOL_API_CHILD";
const X07_DOC_REPORT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-doc.report.schema.json");
const X07_TOOL_EVENTS_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-tool.events.schema.json");

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

pub(crate) fn maybe_handle(raw_args: &[OsString]) -> Result<Option<std::process::ExitCode>> {
    if std::env::var_os(TOOL_API_CHILD_ENV).is_some() {
        return Ok(None);
    }
    if raw_args.is_empty() {
        return Ok(None);
    }

    let parsed = reporting::parse_machine_flags(&raw_args[1..]);
    if !parsed.saw_any {
        return Ok(None);
    }
    let should_handle = parsed.mode != reporting::JsonMode::Off
        || parsed.json_schema
        || parsed.json_schema_id
        || !parsed.parse_errors.is_empty();
    if !should_handle {
        return Ok(None);
    }

    let started = Instant::now();
    let scope = reporting::detect_scope(&parsed.passthrough);

    let res = std::panic::catch_unwind(|| {
        handle_machine_request(raw_args, &parsed, scope.as_deref(), started)
    });
    match res {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(err)) => {
            let report =
                internal_error_report(raw_args, scope.as_deref(), started, &format!("{err:#}"));
            emit_error_or_report(&parsed, scope.as_deref(), started, &report)?;
            Ok(Some(std::process::ExitCode::from(report.exit_code)))
        }
        Err(panic) => {
            let msg = panic_message(panic);
            let report = internal_error_report(raw_args, scope.as_deref(), started, &msg);
            emit_error_or_report(&parsed, scope.as_deref(), started, &report)?;
            Ok(Some(std::process::ExitCode::from(report.exit_code)))
        }
    }
}

fn handle_machine_request(
    raw_args: &[OsString],
    parsed: &reporting::ParsedMachineFlags,
    scope: Option<&str>,
    started: Instant,
) -> Result<Option<std::process::ExitCode>> {
    let report_schema_version = report_schema_version_for_scope(scope)?;
    if parsed.json_schema || parsed.json_schema_id {
        let code = emit_schema_or_id(parsed, scope, &report_schema_version)?;
        return Ok(Some(code));
    }

    let wants_report = parsed.mode != reporting::JsonMode::Off;
    if !wants_report {
        let mut diags = Vec::new();
        if !parsed.parse_errors.is_empty() {
            for msg in &parsed.parse_errors {
                diags.push(reporting::diag_error(
                    "X07-TOOL-ARGS-0001",
                    diagnostics::Stage::Parse,
                    msg,
                ));
            }
        } else if parsed.report_out.is_some() || parsed.quiet_json {
            let msg = "--report-out/--quiet-json requires --json";
            diags.push(reporting::diag_error(
                "X07-TOOL-ARGS-0001",
                diagnostics::Stage::Parse,
                msg,
            ));
        } else {
            return Ok(None);
        }

        let report = reporting::build_report(
            scope,
            &report_schema_version,
            started,
            raw_args,
            2,
            diags,
            ToolResultPayload {
                stdout: empty_stream_payload(),
                stderr: empty_stream_payload(),
                stdout_json: None,
                stderr_json: None,
            },
            reporting::MetaDelta::default(),
        );
        emit_error_or_report(parsed, scope, started, &report)?;
        return Ok(Some(std::process::ExitCode::from(report.exit_code)));
    }

    if !parsed.parse_errors.is_empty() {
        let mut diags = Vec::new();
        for msg in &parsed.parse_errors {
            diags.push(reporting::diag_error(
                "X07-TOOL-ARGS-0001",
                diagnostics::Stage::Parse,
                msg,
            ));
        }

        let report = reporting::build_report(
            scope,
            &report_schema_version,
            started,
            raw_args,
            2,
            diags,
            ToolResultPayload {
                stdout: empty_stream_payload(),
                stderr: empty_stream_payload(),
                stdout_json: None,
                stderr_json: None,
            },
            reporting::MetaDelta::default(),
        );
        emit_error_or_report(parsed, scope, started, &report)?;
        return Ok(Some(std::process::ExitCode::from(report.exit_code)));
    }

    if parsed.mode == reporting::JsonMode::Jsonl {
        let command_id = reporting::command_id_for_scope(scope);
        let mut emitter = reporting::JsonlEmitter::new(started);
        emitter.emit(&command_id, "start", serde_json::json!({ "scope": scope }))?;

        let out = run_wrapped_command_streaming(&parsed.passthrough, &command_id, &mut emitter)?;
        let report = wrapped_report(raw_args, parsed, scope, started, out)?;

        emitter.emit(&command_id, "final_report", serde_json::to_value(&report)?)?;

        if let Some(path) = parsed.report_out.as_deref() {
            let bytes = reporting::encode_json_bytes(
                &serde_json::to_value(&report)?,
                reporting::JsonMode::Canon,
            )?;
            reporting::write_bytes(path, &bytes)?;
        }

        return Ok(Some(std::process::ExitCode::from(report.exit_code)));
    }

    if parsed.mode != reporting::JsonMode::Off && is_native_json_scope(scope) {
        let out = run_native_json_command(parsed)?;
        return Ok(Some(emit_native_or_fallback(
            raw_args, parsed, scope, started, out,
        )?));
    }

    let out = run_wrapped_command(&parsed.passthrough)?;
    let report = wrapped_report(raw_args, parsed, scope, started, out)?;
    emit_report_json(parsed, &report)?;

    Ok(Some(std::process::ExitCode::from(report.exit_code)))
}

fn wrapped_report(
    raw_args: &[OsString],
    parsed: &reporting::ParsedMachineFlags,
    scope: Option<&str>,
    started: Instant,
    out: ChildOutput,
) -> Result<reporting::ToolReport<ToolResultPayload>> {
    let stdout_json = parse_json_bytes(&out.stdout);
    let stderr_json = parse_json_bytes(&out.stderr);

    let mut diagnostics = extract_diagnostics(stdout_json.as_ref())
        .or_else(|| extract_diagnostics(stderr_json.as_ref()))
        .unwrap_or_default();

    let stdout_stream = stream_payload(&out.stdout);
    let stderr_stream = stream_payload(&out.stderr);

    let mut exit_code = out.exit_code;
    if out.internal_failure {
        exit_code = 3;
        diagnostics.clear();
        let msg = "internal tool failure";
        let mut diag = reporting::diag_error("X07-INTERNAL-0001", diagnostics::Stage::Run, msg);
        if let Ok(text) = String::from_utf8(out.stderr.clone()) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                diag.data.insert(
                    "panic_stderr".to_string(),
                    Value::String(trimmed.to_string()),
                );
            }
        }
        diagnostics.push(diag);
    }

    if exit_code != 0
        && diagnostics
            .iter()
            .all(|d| d.severity != diagnostics::Severity::Error)
    {
        let msg = extract_child_error_message_any(stdout_json.as_ref(), stderr_json.as_ref())
            .or_else(|| {
                stderr_stream
                    .text
                    .as_deref()
                    .and_then(non_empty_trimmed)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("wrapped command failed with exit code {exit_code}"));
        diagnostics.push(reporting::diag_error(
            "X07-TOOL-EXEC-0001",
            diagnostics::Stage::Run,
            msg.trim(),
        ));
    }

    let result = ToolResultPayload {
        stdout: stdout_stream,
        stderr: stderr_stream,
        stdout_json,
        stderr_json,
    };

    let mut input_paths: BTreeSet<PathBuf> = BTreeSet::new();
    let mut output_paths: BTreeSet<PathBuf> = BTreeSet::new();

    if let Some(out) = parsed.out.as_ref() {
        output_paths.insert(out.clone());
    }
    if let Some(path) = parsed.report_out.as_ref() {
        output_paths.insert(path.clone());
    }
    if let Some(doc) = result.stdout_json.as_ref() {
        collect_meta_paths_from_child(doc, &mut input_paths, &mut output_paths);
    }
    if let Some(doc) = result.stderr_json.as_ref() {
        collect_meta_paths_from_child(doc, &mut input_paths, &mut output_paths);
    }

    let meta = reporting::MetaDelta {
        inputs: input_paths.into_iter().collect(),
        outputs: output_paths.into_iter().collect(),
        nondeterminism: infer_nondeterminism(scope, &parsed.passthrough),
    };

    Ok(reporting::build_report(
        scope,
        &report_schema_version_for_scope(scope)?,
        started,
        raw_args,
        exit_code,
        diagnostics,
        result,
        meta,
    ))
}

fn collect_meta_paths_from_child(
    doc: &Value,
    inputs: &mut BTreeSet<PathBuf>,
    outputs: &mut BTreeSet<PathBuf>,
) {
    let meta_obj = doc.get("meta").and_then(Value::as_object);
    let digests_obj = doc.get("digests").and_then(Value::as_object);

    let meta_inputs = meta_obj.and_then(|m| m.get("inputs").and_then(Value::as_array));
    if let Some(arr) = meta_inputs {
        for v in arr {
            if let Some(s) = v.as_str() {
                inputs.insert(PathBuf::from(s));
            }
        }
    } else if let Some(arr) = digests_obj.and_then(|d| d.get("inputs").and_then(Value::as_array)) {
        for v in arr {
            if let Some(p) = v.get("path").and_then(Value::as_str) {
                inputs.insert(PathBuf::from(p));
            }
        }
    }

    let meta_outputs = meta_obj.and_then(|m| m.get("outputs").and_then(Value::as_array));
    if let Some(arr) = meta_outputs {
        for v in arr {
            if let Some(s) = v.as_str() {
                outputs.insert(PathBuf::from(s));
            }
        }
    } else if let Some(arr) = digests_obj.and_then(|d| d.get("outputs").and_then(Value::as_array)) {
        for v in arr {
            if let Some(p) = v.get("path").and_then(Value::as_str) {
                outputs.insert(PathBuf::from(p));
            }
        }
    }
}

fn emit_schema_or_id(
    parsed: &reporting::ParsedMachineFlags,
    scope: Option<&str>,
    report_schema_version: &str,
) -> Result<std::process::ExitCode> {
    if parsed.json_schema {
        let bytes = if parsed.mode == reporting::JsonMode::Jsonl {
            X07_TOOL_EVENTS_SCHEMA_BYTES
        } else {
            schema_bytes_for_scope(scope)?
        };
        reporting::emit_report_bytes(bytes, parsed.report_out.as_deref(), parsed.quiet_json)?;
        return Ok(std::process::ExitCode::SUCCESS);
    }

    if parsed.json_schema_id {
        let id = if parsed.mode == reporting::JsonMode::Jsonl {
            reporting::TOOL_EVENTS_SCHEMA_VERSION.to_string()
        } else {
            report_schema_version.to_string()
        };
        let mut bytes = id.into_bytes();
        bytes.push(b'\n');
        reporting::emit_report_bytes(&bytes, parsed.report_out.as_deref(), parsed.quiet_json)?;
        return Ok(std::process::ExitCode::SUCCESS);
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn emit_error_or_report(
    parsed: &reporting::ParsedMachineFlags,
    scope: Option<&str>,
    started: Instant,
    report: &reporting::ToolReport<ToolResultPayload>,
) -> Result<()> {
    if parsed.mode == reporting::JsonMode::Jsonl {
        let command_id = reporting::command_id_for_scope(scope);
        let mut emitter = reporting::JsonlEmitter::new(started);
        emitter.emit(&command_id, "start", serde_json::json!({ "scope": scope }))?;
        emitter.emit(&command_id, "final_report", serde_json::to_value(report)?)?;

        if let Some(path) = parsed.report_out.as_deref() {
            let bytes = reporting::encode_json_bytes(
                &serde_json::to_value(report)?,
                reporting::JsonMode::Canon,
            )?;
            reporting::write_bytes(path, &bytes)?;
        }
        return Ok(());
    }

    emit_report_json(parsed, report)
}

fn emit_report_json(
    parsed: &reporting::ParsedMachineFlags,
    report: &reporting::ToolReport<ToolResultPayload>,
) -> Result<()> {
    let report_value = serde_json::to_value(report)?;
    let bytes = reporting::encode_json_bytes(&report_value, parsed.mode)?;
    reporting::emit_report_bytes(&bytes, parsed.report_out.as_deref(), parsed.quiet_json)
}

fn schema_bytes_for_scope(scope: Option<&str>) -> Result<&'static [u8]> {
    match scope {
        Some("doc") => Ok(X07_DOC_REPORT_SCHEMA_BYTES),
        _ => crate::tool_report_schemas::tool_report_schema_bytes(scope.map(std::ffi::OsStr::new))
            .context("missing embedded tool report schema for scope"),
    }
}

fn schema_version_from_bytes(schema_bytes: &[u8]) -> Result<String> {
    let doc: Value = serde_json::from_slice(schema_bytes).context("parse embedded tool schema")?;
    doc.pointer("/properties/schema_version/const")
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("embedded tool schema missing properties.schema_version.const")
}

fn report_schema_version_for_scope(scope: Option<&str>) -> Result<String> {
    match scope {
        Some("doc") => Ok(x07_contracts::X07_DOC_REPORT_SCHEMA_VERSION.to_string()),
        _ => schema_version_from_bytes(schema_bytes_for_scope(scope)?),
    }
}

fn fallback_report_schema_version_for_scope(scope: Option<&str>) -> String {
    report_schema_version_for_scope(scope)
        .unwrap_or_else(|_| reporting::schema_id_for_scope(scope, "0.1.0"))
}

fn is_native_json_scope(scope: Option<&str>) -> bool {
    matches!(scope, Some("doc") | Some("wasm"))
}

fn run_native_json_command(parsed: &reporting::ParsedMachineFlags) -> Result<ChildOutput> {
    let mut args = parsed.passthrough.clone();
    args.push(OsString::from("--json"));
    run_wrapped_command(&args)
}

fn emit_native_or_fallback(
    raw_args: &[OsString],
    parsed: &reporting::ParsedMachineFlags,
    scope: Option<&str>,
    started: Instant,
    out: ChildOutput,
) -> Result<std::process::ExitCode> {
    if out.internal_failure {
        let report = wrapped_report(raw_args, parsed, scope, started, out)?;
        emit_report_json(parsed, &report)?;
        return Ok(std::process::ExitCode::from(report.exit_code));
    }

    let doc: Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(_) => {
            let report = wrapped_report(raw_args, parsed, scope, started, out)?;
            emit_report_json(parsed, &report)?;
            return Ok(std::process::ExitCode::from(report.exit_code));
        }
    };

    let bytes = reporting::encode_json_bytes(&doc, parsed.mode)?;
    reporting::emit_report_bytes(&bytes, parsed.report_out.as_deref(), parsed.quiet_json)?;
    Ok(std::process::ExitCode::from(out.exit_code))
}

#[derive(Debug)]
struct ChildOutput {
    exit_code: u8,
    internal_failure: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_wrapped_command(args: &[OsString]) -> Result<ChildOutput> {
    let exe = std::env::current_exe().context("resolve current executable")?;
    let output = Command::new(exe)
        .args(args)
        .env(TOOL_API_CHILD_ENV, "1")
        .output()
        .context("run wrapped command")?;

    let raw = output.status.code();
    let internal_failure = raw.is_none() || raw == Some(101);
    let exit_code = raw.unwrap_or(3).clamp(0, 255) as u8;

    Ok(ChildOutput {
        exit_code,
        internal_failure,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

#[derive(Debug)]
enum StreamKind {
    Stdout,
    Stderr,
}

#[derive(Debug)]
struct StreamMsg {
    kind: StreamKind,
    bytes: Vec<u8>,
}

fn run_wrapped_command_streaming(
    args: &[OsString],
    command_id: &str,
    emitter: &mut reporting::JsonlEmitter,
) -> Result<ChildOutput> {
    let exe = std::env::current_exe().context("resolve current executable")?;
    let mut child = Command::new(exe)
        .args(args)
        .env(TOOL_API_CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn wrapped command")?;

    let mut stdout = child.stdout.take().context("child stdout")?;
    let mut stderr = child.stderr.take().context("child stderr")?;

    let (tx, rx) = mpsc::channel::<StreamMsg>();

    let tx_out = tx.clone();
    let out_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 8 * 1024];
        while let Ok(n) = stdout.read(&mut buf) {
            if n == 0 {
                break;
            }
            let _ = tx_out.send(StreamMsg {
                kind: StreamKind::Stdout,
                bytes: buf[..n].to_vec(),
            });
        }
    });

    let tx_err = tx;
    let err_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 8 * 1024];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            let _ = tx_err.send(StreamMsg {
                kind: StreamKind::Stderr,
                bytes: buf[..n].to_vec(),
            });
        }
    });

    let mut out_bytes = Vec::new();
    let mut err_bytes = Vec::new();

    while let Ok(msg) = rx.recv() {
        match msg.kind {
            StreamKind::Stdout => {
                out_bytes.extend_from_slice(&msg.bytes);
                emitter.emit(
                    command_id,
                    "stdout_chunk",
                    serde_json::to_value(stream_payload(&msg.bytes))?,
                )?;
            }
            StreamKind::Stderr => {
                err_bytes.extend_from_slice(&msg.bytes);
                emitter.emit(
                    command_id,
                    "stderr_chunk",
                    serde_json::to_value(stream_payload(&msg.bytes))?,
                )?;
            }
        }
    }

    let _ = out_thread.join();
    let _ = err_thread.join();

    let status = child.wait().context("wait wrapped command")?;

    let raw = status.code();
    let internal_failure = raw.is_none() || raw == Some(101);
    let exit_code = raw.unwrap_or(3).clamp(0, 255) as u8;

    Ok(ChildOutput {
        exit_code,
        internal_failure,
        stdout: out_bytes,
        stderr: err_bytes,
    })
}

fn parse_json_bytes(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}

fn non_empty_trimmed(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn extract_child_error_message(doc: &Value) -> Option<String> {
    let obj = doc.as_object()?;

    if let Some(err) = obj.get("error") {
        match err {
            Value::Object(eobj) => {
                let code = eobj
                    .get("code")
                    .and_then(Value::as_str)
                    .and_then(non_empty_trimmed);
                let message = eobj
                    .get("message")
                    .and_then(Value::as_str)
                    .and_then(non_empty_trimmed)
                    .or_else(|| {
                        eobj.get("msg")
                            .and_then(Value::as_str)
                            .and_then(non_empty_trimmed)
                    })
                    .or_else(|| {
                        eobj.get("error")
                            .and_then(Value::as_str)
                            .and_then(non_empty_trimmed)
                    });

                return match (code, message) {
                    (Some(c), Some(m)) => Some(format!("{c}: {m}")),
                    (None, Some(m)) => Some(m.to_string()),
                    (Some(c), None) => Some(c.to_string()),
                    (None, None) => None,
                };
            }
            Value::String(s) => {
                return non_empty_trimmed(s).map(str::to_string);
            }
            _ => {}
        }
    }

    if let Some(m) = obj
        .get("message")
        .and_then(Value::as_str)
        .and_then(non_empty_trimmed)
    {
        return Some(m.to_string());
    }

    None
}

fn extract_child_error_message_any(
    stdout_json: Option<&Value>,
    stderr_json: Option<&Value>,
) -> Option<String> {
    stdout_json
        .and_then(extract_child_error_message)
        .or_else(|| stderr_json.and_then(extract_child_error_message))
}

fn infer_nondeterminism(scope: Option<&str>, argv: &[OsString]) -> reporting::Nondeterminism {
    let mut nd = reporting::Nondeterminism::default();
    let Some(scope) = scope else {
        return nd;
    };

    if scope == "pkg" || scope.starts_with("pkg.") {
        let mut offline = false;
        let mut file_registry = false;

        let mut i = 0usize;
        while i < argv.len() {
            let tok = argv[i].to_string_lossy();

            if tok == "--offline" {
                offline = true;
            }

            if let Some(v) = tok
                .strip_prefix("--index=")
                .or_else(|| tok.strip_prefix("--registry="))
            {
                if v.contains("file://") {
                    file_registry = true;
                }
            }

            if (tok == "--index" || tok == "--registry") && i + 1 < argv.len() {
                let v = argv[i + 1].to_string_lossy();
                if v.contains("file://") {
                    file_registry = true;
                }
            }

            i += 1;
        }

        nd.uses_network = !(offline || file_registry);
    }

    if scope == "init" || scope.starts_with("init.") {
        let mut template: Option<String> = None;

        let mut i = 0usize;
        while i < argv.len() {
            let tok = argv[i].to_string_lossy();
            if tok == "--template" {
                if let Some(v) = argv.get(i + 1) {
                    template = Some(v.to_string_lossy().to_string());
                }
            } else if let Some(v) = tok.strip_prefix("--template=") {
                template = Some(v.to_string());
            }
            i += 1;
        }

        let runs_pkg_lock = template.is_some_and(|t| {
            !matches!(
                t.as_str(),
                "verified-core-pure"
                    | "trusted-sandbox-program"
                    | "trusted-network-service"
                    | "certified-capsule"
                    | "certified-network-capsule"
                    | "mcp-server"
                    | "mcp-server-stdio"
                    | "mcp-server-http"
                    | "mcp-server-http-tasks"
            )
        });

        if runs_pkg_lock {
            let index = std::env::var("X07_PKG_INDEX_URL").ok().unwrap_or_default();
            let raw = index.strip_prefix("sparse+").unwrap_or(index.as_str());
            nd.uses_network = !raw.trim_start().starts_with("file://");
        }
    }

    nd
}

fn extract_diagnostics(doc: Option<&Value>) -> Option<Vec<diagnostics::Diagnostic>> {
    let doc = doc?;
    let obj = doc.as_object()?;

    for key in ["diagnostics", "diags"] {
        let Some(v) = obj.get(key) else {
            continue;
        };

        let Some(arr) = v.as_array() else {
            continue;
        };
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

            let loc = obj
                .get("loc")
                .and_then(|v| serde_json::from_value::<diagnostics::Location>(v.clone()).ok());

            let notes: Vec<String> = obj
                .get("notes")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();

            let related: Vec<diagnostics::Location> = obj
                .get("related")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            serde_json::from_value::<diagnostics::Location>(v.clone()).ok()
                        })
                        .collect()
                })
                .unwrap_or_default();

            let data: BTreeMap<String, Value> = obj
                .get("data")
                .and_then(Value::as_object)
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();

            let quickfix = obj
                .get("quickfix")
                .and_then(|v| serde_json::from_value::<diagnostics::Quickfix>(v.clone()).ok());

            diags.push(diagnostics::Diagnostic {
                code: code.to_string(),
                severity,
                stage,
                message: message.to_string(),
                loc,
                notes,
                related,
                data,
                quickfix,
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

fn empty_stream_payload() -> StreamPayload {
    StreamPayload {
        bytes_len: 0,
        text: None,
        base64: None,
    }
}

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic".to_string()
    }
}

fn internal_error_report(
    raw_args: &[OsString],
    scope: Option<&str>,
    started: Instant,
    message: &str,
) -> reporting::ToolReport<ToolResultPayload> {
    let msg = "internal tool failure";
    let mut diag = reporting::diag_error("X07-INTERNAL-0001", diagnostics::Stage::Run, msg);
    if !message.trim().is_empty() {
        diag.data.insert(
            "panic".to_string(),
            Value::String(message.trim().to_string()),
        );
    }

    reporting::build_report(
        scope,
        &fallback_report_schema_version_for_scope(scope),
        started,
        raw_args,
        3,
        vec![diag],
        ToolResultPayload {
            stdout: empty_stream_payload(),
            stderr: empty_stream_payload(),
            stdout_json: None,
            stderr_json: None,
        },
        reporting::MetaDelta::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvVarGuard {
        key: &'static str,
        old: Option<OsString>,
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.old.take() {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn with_env_var<T>(key: &'static str, value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let old = std::env::var_os(key);
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        let env_guard = EnvVarGuard { key, old };
        let out = f();
        drop(env_guard);
        out
    }

    fn parsed_with_passthrough(passthrough: &[&str]) -> reporting::ParsedMachineFlags {
        reporting::ParsedMachineFlags {
            mode: reporting::JsonMode::Off,
            json_schema: false,
            json_schema_id: false,
            report_out: None,
            quiet_json: false,
            out: None,
            passthrough: passthrough.iter().map(OsString::from).collect(),
            saw_any: false,
            parse_errors: Vec::new(),
        }
    }

    fn tool_exec_diag(
        report: &reporting::ToolReport<ToolResultPayload>,
    ) -> &diagnostics::Diagnostic {
        report
            .diagnostics
            .iter()
            .find(|d| d.code == "X07-TOOL-EXEC-0001")
            .expect("missing X07-TOOL-EXEC-0001")
    }

    #[test]
    fn wrapped_report_uses_stdout_json_error_message_when_stderr_empty() {
        let raw_args = [
            OsString::from("x07"),
            OsString::from("pkg"),
            OsString::from("versions"),
        ];
        let parsed = parsed_with_passthrough(&["pkg", "versions", "std"]);
        let out = ChildOutput {
            exit_code: 20,
            internal_failure: false,
            stdout: br#"{"ok":false,"error":{"code":"X07PKG_INDEX_CONFIG","message":"fetch index failed"}}"#.to_vec(),
            stderr: Vec::new(),
        };

        let report = wrapped_report(
            &raw_args,
            &parsed,
            Some("pkg.versions"),
            Instant::now(),
            out,
        )
        .expect("wrapped_report");

        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(
            tool_exec_diag(&report).message,
            "X07PKG_INDEX_CONFIG: fetch index failed"
        );
    }

    #[test]
    fn wrapped_report_falls_back_to_exit_code_message_if_no_stderr_and_no_json_error() {
        let raw_args = [
            OsString::from("x07"),
            OsString::from("pkg"),
            OsString::from("versions"),
        ];
        let parsed = parsed_with_passthrough(&["pkg", "versions", "std"]);
        let out = ChildOutput {
            exit_code: 7,
            internal_failure: false,
            stdout: b"not json".to_vec(),
            stderr: Vec::new(),
        };

        let report = wrapped_report(
            &raw_args,
            &parsed,
            Some("pkg.versions"),
            Instant::now(),
            out,
        )
        .expect("wrapped_report");

        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(
            tool_exec_diag(&report).message,
            "wrapped command failed with exit code 7"
        );
    }

    #[test]
    fn wrapped_report_infers_uses_network_for_pkg_scope() {
        let raw_args = [
            OsString::from("x07"),
            OsString::from("pkg"),
            OsString::from("versions"),
        ];
        let parsed = parsed_with_passthrough(&["pkg", "versions", "std"]);
        let out = ChildOutput {
            exit_code: 0,
            internal_failure: false,
            stdout: br#"{"ok":true}"#.to_vec(),
            stderr: Vec::new(),
        };

        let report = wrapped_report(
            &raw_args,
            &parsed,
            Some("pkg.versions"),
            Instant::now(),
            out,
        )
        .expect("wrapped_report");

        assert!(report.meta.nondeterminism.uses_network);
    }

    #[test]
    fn wrapped_report_offline_pkg_scope_does_not_use_network() {
        let raw_args = [
            OsString::from("x07"),
            OsString::from("pkg"),
            OsString::from("versions"),
        ];
        let parsed = parsed_with_passthrough(&["pkg", "versions", "std", "--offline"]);
        let out = ChildOutput {
            exit_code: 0,
            internal_failure: false,
            stdout: br#"{"ok":true}"#.to_vec(),
            stderr: Vec::new(),
        };

        let report = wrapped_report(
            &raw_args,
            &parsed,
            Some("pkg.versions"),
            Instant::now(),
            out,
        )
        .expect("wrapped_report");

        assert!(!report.meta.nondeterminism.uses_network);
    }

    #[test]
    fn wrapped_report_infers_uses_network_for_init_default_no_template() {
        let raw_args = [OsString::from("x07"), OsString::from("init")];
        let parsed = parsed_with_passthrough(&["init"]);
        let out = ChildOutput {
            exit_code: 0,
            internal_failure: false,
            stdout: br#"{"ok":true}"#.to_vec(),
            stderr: Vec::new(),
        };

        let report = wrapped_report(&raw_args, &parsed, Some("init"), Instant::now(), out)
            .expect("wrapped_report");

        assert!(!report.meta.nondeterminism.uses_network);
    }

    #[test]
    fn wrapped_report_infers_uses_network_for_init_static_template() {
        let raw_args = [
            OsString::from("x07"),
            OsString::from("init"),
            OsString::from("--template"),
            OsString::from("verified-core-pure"),
        ];
        let parsed = parsed_with_passthrough(&["init", "--template", "verified-core-pure"]);
        let out = ChildOutput {
            exit_code: 0,
            internal_failure: false,
            stdout: br#"{"ok":true}"#.to_vec(),
            stderr: Vec::new(),
        };

        let report = wrapped_report(&raw_args, &parsed, Some("init"), Instant::now(), out)
            .expect("wrapped_report");

        assert!(!report.meta.nondeterminism.uses_network);
    }

    #[test]
    fn wrapped_report_infers_uses_network_for_init_template_locking() {
        with_env_var("X07_PKG_INDEX_URL", None, || {
            let raw_args = [
                OsString::from("x07"),
                OsString::from("init"),
                OsString::from("--template"),
                OsString::from("cli"),
            ];
            let parsed = parsed_with_passthrough(&["init", "--template", "cli"]);
            let out = ChildOutput {
                exit_code: 0,
                internal_failure: false,
                stdout: br#"{"ok":true}"#.to_vec(),
                stderr: Vec::new(),
            };

            let report = wrapped_report(&raw_args, &parsed, Some("init"), Instant::now(), out)
                .expect("wrapped_report");

            assert!(report.meta.nondeterminism.uses_network);
        })
    }

    #[test]
    fn wrapped_report_infers_uses_network_for_init_template_file_index() {
        with_env_var("X07_PKG_INDEX_URL", Some("file:///tmp/index/"), || {
            let raw_args = [
                OsString::from("x07"),
                OsString::from("init"),
                OsString::from("--template"),
                OsString::from("cli"),
            ];
            let parsed = parsed_with_passthrough(&["init", "--template", "cli"]);
            let out = ChildOutput {
                exit_code: 0,
                internal_failure: false,
                stdout: br#"{"ok":true}"#.to_vec(),
                stderr: Vec::new(),
            };

            let report = wrapped_report(&raw_args, &parsed, Some("init"), Instant::now(), out)
                .expect("wrapped_report");

            assert!(!report.meta.nondeterminism.uses_network);
        })
    }

    #[test]
    fn wrapped_report_infers_uses_network_for_init_package() {
        let raw_args = [
            OsString::from("x07"),
            OsString::from("init"),
            OsString::from("--package"),
        ];
        let parsed = parsed_with_passthrough(&["init", "--package"]);
        let out = ChildOutput {
            exit_code: 0,
            internal_failure: false,
            stdout: br#"{"ok":true}"#.to_vec(),
            stderr: Vec::new(),
        };

        let report = wrapped_report(&raw_args, &parsed, Some("init"), Instant::now(), out)
            .expect("wrapped_report");

        assert!(!report.meta.nondeterminism.uses_network);
    }
}
