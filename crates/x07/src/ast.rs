use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use jsonschema::Draft;
use serde::Serialize;
use serde_json::Value;
use x07_contracts::X07AST_SCHEMA_VERSION;
use x07_worlds::WorldId;
use x07c::diagnostics;
use x07c::json_patch;

use crate::util;

const X07AST_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07ast.schema.json");
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Args)]
#[command(subcommand_required = false)]
pub struct AstArgs {
    #[command(subcommand)]
    pub cmd: Option<AstCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AstCommand {
    /// Emit a minimal x07AST entry/module template.
    Init(AstInitArgs),
    /// Extract a subvalue by JSON Pointer (RFC 6901).
    Get(AstGetArgs),
    /// Apply a JSON patch file to an x07AST JSON input.
    ApplyPatch(AstApplyPatchArgs),
    /// Validate an x07AST file (schema + optional diagnostics catalog).
    Validate(AstValidateArgs),
    /// Canonicalize an x07AST JSON file (JCS ordering).
    Canon(AstCanonArgs),
}

#[derive(Debug, Clone, Args)]
pub struct AstInitArgs {
    #[arg(long, value_enum)]
    pub world: WorldId,

    #[arg(long)]
    pub module: String,

    #[arg(long, value_enum, default_value = "entry")]
    pub kind: AstInitKind,

    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum AstInitKind {
    Entry,
    Module,
}

impl AstInitKind {
    fn as_str(self) -> &'static str {
        match self {
            AstInitKind::Entry => "entry",
            AstInitKind::Module => "module",
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct AstApplyPatchArgs {
    #[arg(long, value_name = "PATH")]
    pub r#in: PathBuf,

    #[arg(long, value_name = "PATH")]
    pub patch: PathBuf,

    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,

    #[arg(long)]
    pub validate: bool,
}

#[derive(Debug, Clone, Args)]
pub struct AstValidateArgs {
    #[arg(long, value_name = "PATH")]
    pub r#in: PathBuf,

    #[arg(long, value_name = "PATH")]
    pub x07diag: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct AstCanonArgs {
    #[arg(long, value_name = "PATH")]
    pub r#in: PathBuf,

    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
}

pub fn cmd_ast(args: AstArgs) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing subcommand (try --help)");
    };

    match cmd {
        AstCommand::Init(args) => cmd_init(args),
        AstCommand::Get(args) => cmd_get(args),
        AstCommand::ApplyPatch(args) => cmd_apply_patch(args),
        AstCommand::Validate(args) => cmd_validate(args),
        AstCommand::Canon(args) => cmd_canon(args),
    }
}

#[derive(Debug, Serialize)]
struct AstInitReport {
    ok: bool,
    out: String,
    schema_version: String,
    template_id: String,
    sha256: String,
}

#[derive(Debug, Clone, Args)]
pub struct AstGetArgs {
    #[arg(long, value_name = "PATH")]
    pub r#in: PathBuf,

    #[arg(long, value_name = "JSON_POINTER")]
    pub ptr: String,

    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct AstGetReport {
    ok: bool,
    r#in: String,
    ptr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    out: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
}

fn cmd_init(args: AstInitArgs) -> Result<std::process::ExitCode> {
    if args.module.is_empty() || args.module.chars().any(|c| c.is_whitespace()) {
        let report = AstInitReport {
            ok: false,
            out: args.out.display().to_string(),
            schema_version: X07AST_SCHEMA_VERSION.to_string(),
            template_id: format!("{}/{}@v1", args.world.as_str(), args.module),
            sha256: String::new(),
        };
        print_json(&report)?;
        return Ok(std::process::ExitCode::from(2));
    }

    let mut doc = serde_json::Map::new();
    doc.insert(
        "schema_version".to_string(),
        Value::String(X07AST_SCHEMA_VERSION.to_string()),
    );
    doc.insert(
        "kind".to_string(),
        Value::String(args.kind.as_str().to_string()),
    );
    doc.insert("module_id".to_string(), Value::String(args.module.clone()));
    doc.insert("imports".to_string(), Value::Array(Vec::new()));
    doc.insert("decls".to_string(), Value::Array(Vec::new()));
    if args.kind == AstInitKind::Entry {
        doc.insert(
            "solve".to_string(),
            Value::Array(vec![
                Value::String("bytes.alloc".to_string()),
                Value::Number(0.into()),
            ]),
        );
    }

    let mut v = Value::Object(doc);
    x07c::x07ast::canon_value_jcs(&mut v);
    let out_bytes = serde_json::to_string(&v)?.into_bytes();
    let out_bytes = with_trailing_newline(out_bytes);

    write_atomic(&args.out, &out_bytes)
        .with_context(|| format!("write: {}", args.out.display()))?;

    let report = AstInitReport {
        ok: true,
        out: args.out.display().to_string(),
        schema_version: X07AST_SCHEMA_VERSION.to_string(),
        template_id: format!("{}/{}@v1", args.world.as_str(), args.module),
        sha256: sha256_hex(&out_bytes),
    };
    print_json(&report)?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_get(args: AstGetArgs) -> Result<std::process::ExitCode> {
    let input_bytes = match std::fs::read(&args.r#in) {
        Ok(bytes) => bytes,
        Err(err) => {
            let report = AstGetReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                ptr: args.ptr.clone(),
                out: args.out.as_ref().map(|p| p.display().to_string()),
                error: Some(err.to_string()),
                value: None,
            };
            print_json(&report)?;
            return Ok(exit_with_error(err));
        }
    };

    let doc: Value = match canonicalize_x07ast_bytes_to_value(&input_bytes) {
        Ok(doc) => doc,
        Err(_) => match serde_json::from_slice(&input_bytes) {
            Ok(doc) => doc,
            Err(err) => {
                let report = AstGetReport {
                    ok: false,
                    r#in: args.r#in.display().to_string(),
                    ptr: args.ptr.clone(),
                    out: args.out.as_ref().map(|p| p.display().to_string()),
                    error: Some(err.to_string()),
                    value: None,
                };
                print_json(&report)?;
                return Ok(exit_with_error(err));
            }
        },
    };

    let value = match json_pointer_get(&doc, &args.ptr) {
        Ok(v) => v.clone(),
        Err(msg) => {
            let report = AstGetReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                ptr: args.ptr.clone(),
                out: args.out.as_ref().map(|p| p.display().to_string()),
                error: Some(msg),
                value: None,
            };
            print_json(&report)?;
            return Ok(std::process::ExitCode::from(20));
        }
    };

    if let Some(out_path) = &args.out {
        if out_path.as_os_str() == "-" {
            let report = AstGetReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                ptr: args.ptr.clone(),
                out: Some(out_path.display().to_string()),
                error: Some(
                    "--out '-' is not supported (stdout is reserved for the report)".to_string(),
                ),
                value: None,
            };
            print_json(&report)?;
            return Ok(std::process::ExitCode::from(20));
        }

        let mut out_bytes = serde_json::to_string_pretty(&value)?.into_bytes();
        out_bytes = with_trailing_newline(out_bytes);
        write_atomic(out_path, &out_bytes)
            .with_context(|| format!("write: {}", out_path.display()))?;
    }

    let report = AstGetReport {
        ok: true,
        r#in: args.r#in.display().to_string(),
        ptr: args.ptr.clone(),
        out: args.out.as_ref().map(|p| p.display().to_string()),
        error: None,
        value: Some(value),
    };
    print_json(&report)?;
    Ok(std::process::ExitCode::SUCCESS)
}

#[derive(Debug, Serialize)]
struct AstApplyPatchReport {
    ok: bool,
    r#in: String,
    out: String,
    sha256: String,
}

fn cmd_apply_patch(args: AstApplyPatchArgs) -> Result<std::process::ExitCode> {
    let out_path = args.out.clone().unwrap_or_else(|| args.r#in.clone());
    let input_bytes = match std::fs::read(&args.r#in) {
        Ok(bytes) => bytes,
        Err(err) => {
            let report = AstApplyPatchReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
            };
            print_json(&report)?;
            return Ok(exit_with_error(err));
        }
    };

    let mut doc: Value = match canonicalize_x07ast_bytes_to_value(&input_bytes) {
        Ok(doc) => doc,
        Err(_) => match serde_json::from_slice(&input_bytes) {
            Ok(doc) => doc,
            Err(err) => {
                let report = AstApplyPatchReport {
                    ok: false,
                    r#in: args.r#in.display().to_string(),
                    out: out_path.display().to_string(),
                    sha256: String::new(),
                };
                print_json(&report)?;
                return Ok(exit_with_error(err));
            }
        },
    };

    let patch_bytes = match std::fs::read(&args.patch) {
        Ok(bytes) => bytes,
        Err(err) => {
            let report = AstApplyPatchReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
            };
            print_json(&report)?;
            return Ok(exit_with_error(err));
        }
    };
    let ops: Vec<diagnostics::PatchOp> = match serde_json::from_slice(&patch_bytes) {
        Ok(ops) => ops,
        Err(err) => {
            let report = AstApplyPatchReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
            };
            print_json(&report)?;
            return Ok(exit_with_error(err));
        }
    };

    if let Err(err) = json_patch::apply_patch(&mut doc, &ops) {
        let report = AstApplyPatchReport {
            ok: false,
            r#in: args.r#in.display().to_string(),
            out: out_path.display().to_string(),
            sha256: String::new(),
        };
        print_json(&report)?;
        eprintln!("apply patch failed: {err}");
        return Ok(std::process::ExitCode::from(21));
    }

    if args.validate {
        let diags = validate_x07ast_doc(&doc)?;
        if !diags.is_empty() {
            let report = diagnostics::Report::ok().with_diagnostics(diags);
            print_json(&report)?;
            return Ok(std::process::ExitCode::from(20));
        }
    }

    let mut out_doc = match canonicalize_x07ast_bytes_to_value(&serde_json::to_vec(&doc)?) {
        Ok(out_doc) => out_doc,
        Err(_) => doc,
    };

    x07c::x07ast::canon_value_jcs(&mut out_doc);
    let out_bytes = serde_json::to_string(&out_doc)?.into_bytes();
    let out_bytes = with_trailing_newline(out_bytes);

    write_atomic(&out_path, &out_bytes)
        .with_context(|| format!("write: {}", out_path.display()))?;

    let report = AstApplyPatchReport {
        ok: true,
        r#in: args.r#in.display().to_string(),
        out: out_path.display().to_string(),
        sha256: sha256_hex(&out_bytes),
    };
    print_json(&report)?;
    Ok(std::process::ExitCode::SUCCESS)
}

#[derive(Debug, Serialize)]
struct AstValidateReport {
    ok: bool,
    r#in: String,
    x07diag: Option<String>,
    diagnostics_count: usize,
}

fn cmd_validate(args: AstValidateArgs) -> Result<std::process::ExitCode> {
    let input_bytes = match std::fs::read(&args.r#in) {
        Ok(bytes) => bytes,
        Err(err) => {
            let report = AstValidateReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                x07diag: args.x07diag.as_ref().map(|p| p.display().to_string()),
                diagnostics_count: 0,
            };
            print_json(&report)?;
            return Ok(exit_with_error(err));
        }
    };

    let doc: Value = match serde_json::from_slice(&input_bytes) {
        Ok(doc) => doc,
        Err(err) => {
            let report = AstValidateReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                x07diag: args.x07diag.as_ref().map(|p| p.display().to_string()),
                diagnostics_count: 0,
            };
            print_json(&report)?;
            return Ok(exit_with_error(err));
        }
    };

    let diagnostics = validate_x07ast_doc(&doc)?;
    let report = diagnostics::Report::ok().with_diagnostics(diagnostics);
    if let Some(path) = &args.x07diag {
        write_atomic(path, serde_json::to_string(&report)?.as_bytes())
            .with_context(|| format!("write: {}", path.display()))?;
    }

    let out = AstValidateReport {
        ok: report.ok,
        r#in: args.r#in.display().to_string(),
        x07diag: args.x07diag.as_ref().map(|p| p.display().to_string()),
        diagnostics_count: report.diagnostics.len(),
    };
    print_json(&out)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(20)
    })
}

#[derive(Debug, Serialize)]
struct AstCanonReport {
    ok: bool,
    r#in: String,
    out: String,
    sha256: String,
}

fn cmd_canon(args: AstCanonArgs) -> Result<std::process::ExitCode> {
    let out_path = args.out.clone().unwrap_or_else(|| args.r#in.clone());
    let input_bytes = match std::fs::read(&args.r#in) {
        Ok(bytes) => bytes,
        Err(err) => {
            let report = AstCanonReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
            };
            print_json(&report)?;
            return Ok(exit_with_error(err));
        }
    };

    let mut doc: Value = match canonicalize_x07ast_bytes_to_value(&input_bytes) {
        Ok(doc) => doc,
        Err(err) => {
            let report = AstCanonReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
            };
            print_json(&report)?;
            return Ok(exit_with_error(err));
        }
    };

    x07c::x07ast::canon_value_jcs(&mut doc);
    let out_bytes = serde_json::to_string(&doc)?.into_bytes();
    let out_bytes = with_trailing_newline(out_bytes);

    write_atomic(&out_path, &out_bytes)
        .with_context(|| format!("write: {}", out_path.display()))?;

    let report = AstCanonReport {
        ok: true,
        r#in: args.r#in.display().to_string(),
        out: out_path.display().to_string(),
        sha256: sha256_hex(&out_bytes),
    };
    print_json(&report)?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn with_trailing_newline(mut bytes: Vec<u8>) -> Vec<u8> {
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    bytes
}

fn sha256_hex(bytes: &[u8]) -> String {
    util::sha256_hex(bytes)
}

fn json_pointer_get<'a>(doc: &'a Value, ptr: &str) -> Result<&'a Value, String> {
    let ptr = ptr.trim();
    if ptr.is_empty() {
        return Ok(doc);
    }
    if !ptr.starts_with('/') {
        return Err(format!(
            "invalid JSON Pointer (expected leading '/'): {ptr:?}"
        ));
    }

    let mut cur = doc;
    for raw in ptr.split('/').skip(1) {
        let token = unescape_json_pointer_token(raw)?;
        match cur {
            Value::Object(map) => {
                cur = map
                    .get(&token)
                    .ok_or_else(|| format!("JSON Pointer not found at object key: {token:?}"))?;
            }
            Value::Array(arr) => {
                let idx: usize = token.parse().map_err(|_| {
                    format!("JSON Pointer array index is not an integer: {token:?}")
                })?;
                cur = arr
                    .get(idx)
                    .ok_or_else(|| format!("JSON Pointer array index out of bounds: {idx}"))?;
            }
            _ => {
                return Err(format!(
                    "JSON Pointer traversal hit non-container value at token: {token:?}"
                ));
            }
        }
    }
    Ok(cur)
}

fn unescape_json_pointer_token(token: &str) -> Result<String, String> {
    if !token.contains('~') {
        return Ok(token.to_string());
    }

    let mut out = String::with_capacity(token.len());
    let mut chars = token.chars();
    while let Some(ch) = chars.next() {
        if ch != '~' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('0') => out.push('~'),
            Some('1') => out.push('/'),
            Some(other) => {
                return Err(format!(
                    "invalid JSON Pointer escape sequence: ~{other} in {token:?}"
                ))
            }
            None => {
                return Err(format!(
                    "invalid JSON Pointer escape sequence: '~' at end of {token:?}"
                ))
            }
        }
    }
    Ok(out)
}

fn validate_x07ast_doc(doc: &Value) -> Result<Vec<diagnostics::Diagnostic>> {
    let schema_json: Value =
        serde_json::from_slice(X07AST_SCHEMA_BYTES).context("parse spec/x07ast.schema.json")?;
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema_json)
        .context("build x07ast schema validator")?;

    let mut out = Vec::new();
    for error in validator.iter_errors(doc) {
        let mut data = BTreeMap::new();
        data.insert(
            "schema_path".to_string(),
            Value::String(error.schema_path().to_string()),
        );
        out.push(diagnostics::Diagnostic {
            code: "X07-SCHEMA-0001".to_string(),
            severity: diagnostics::Severity::Error,
            stage: diagnostics::Stage::Parse,
            message: error.to_string(),
            loc: Some(diagnostics::Location::X07Ast {
                ptr: error.instance_path().to_string(),
            }),
            notes: Vec::new(),
            related: Vec::new(),
            data,
            quickfix: None,
        });
    }

    if let Err(err) = x07c::x07ast::parse_x07ast_json(&serde_json::to_vec(doc)?) {
        out.push(diagnostics::Diagnostic {
            code: "X07-X07AST-PARSE-0001".to_string(),
            severity: diagnostics::Severity::Error,
            stage: diagnostics::Stage::Parse,
            message: err.message,
            loc: Some(diagnostics::Location::X07Ast { ptr: err.ptr }),
            notes: Vec::new(),
            related: Vec::new(),
            data: BTreeMap::new(),
            quickfix: None,
        });
    }

    Ok(out)
}

fn canonicalize_x07ast_bytes_to_value(bytes: &[u8]) -> Result<Value> {
    let mut file = x07c::x07ast::parse_x07ast_json(bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    Ok(x07c::x07ast::x07ast_file_to_value(&file))
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output dir: {}", parent.display()))?;
    }

    let tmp = temp_path_next_to(path);
    std::fs::write(&tmp, contents).with_context(|| format!("write temp: {}", tmp.display()))?;

    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp, path).with_context(|| format!("rename: {}", path.display()))?;
            Ok(())
        }
    }
}

fn temp_path_next_to(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{file_name}.{pid}.{n}.tmp"))
}

fn exit_with_error(err: impl std::fmt::Display) -> std::process::ExitCode {
    eprintln!("{err}");
    std::process::ExitCode::from(2)
}
