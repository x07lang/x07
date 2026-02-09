use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use jsonschema::Draft;
use serde::Serialize;
use serde_json::Value;
use x07_contracts::{
    X07AST_SCHEMA_VERSION, X07AST_SCHEMA_VERSIONS_SUPPORTED, X07AST_SCHEMA_VERSION_V0_3_0,
    X07AST_SCHEMA_VERSION_V0_4_0, X07AST_SCHEMA_VERSION_V0_5_0,
};
use x07_worlds::WorldId;
use x07c::diagnostics;
use x07c::json_patch;

use crate::util;

const X07AST_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07ast.schema.json");
const X07AST_SCHEMA_V0_3_BYTES: &[u8] = include_bytes!("../../../spec/x07ast.v0.3.0.schema.json");
const X07AST_SCHEMA_V0_4_BYTES: &[u8] = include_bytes!("../../../spec/x07ast.v0.4.0.schema.json");
const X07AST_SCHEMA_V0_5_BYTES: &[u8] = include_bytes!("../../../spec/x07ast.v0.5.0.schema.json");
const X07AST_MIN_GBNF_BYTES: &[u8] = include_bytes!("../../../spec/x07ast.min.gbnf");
const X07AST_PRETTY_GBNF_BYTES: &[u8] = include_bytes!("../../../spec/x07ast.pretty.gbnf");
const X07AST_SEMANTIC_BYTES: &[u8] = include_bytes!("../../../spec/x07ast.semantic.json");
const X07AST_SEMANTIC_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07ast.semantic.schema.json");

const X07_AST_GRAMMAR_BUNDLE_SCHEMA_VERSION: &str = "x07.ast.grammar_bundle@0.1.0";
const X07_AST_SEMANTIC_SCHEMA_VERSION: &str = "x07.x07ast.semantic@0.1.0";
const X07_GENPACK_MANIFEST_SCHEMA_VERSION: &str = "x07.genpack.manifest@0.1.0";
const X07_GENPACK_NAME: &str = "x07-genpack-x07ast";
const X07_GENPACK_VERSION: &str = "0.1.0";

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
    /// Emit the canonical x07AST JSON Schema document.
    Schema(AstSchemaArgs),
    /// Emit the x07AST grammar bundle for constrained decoding runtimes.
    Grammar(AstGrammarArgs),
}

#[derive(Debug, Clone, Args)]
pub struct AstInitArgs {
    #[arg(long, value_enum)]
    pub world: WorldId,

    #[arg(long)]
    pub module: String,

    #[arg(long, value_name = "SCHEMA_VERSION")]
    pub schema_version: Option<String>,

    #[arg(long, value_enum, default_value = "entry")]
    pub kind: AstInitKind,
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
}

#[derive(Debug, Clone, Args)]
pub struct AstSchemaArgs {
    #[arg(long)]
    pub pretty: bool,

    #[arg(long, value_name = "SCHEMA_VERSION")]
    pub schema_version: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct AstGrammarArgs {
    #[arg(long, required = true)]
    pub cfg: bool,

    #[arg(long, value_name = "DIR")]
    pub out_dir: Option<PathBuf>,
}

pub fn cmd_ast(
    machine: &crate::reporting::MachineArgs,
    args: AstArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing subcommand (try --help)");
    };

    match cmd {
        AstCommand::Init(args) => cmd_init(machine, args),
        AstCommand::Get(args) => cmd_get(machine, args),
        AstCommand::ApplyPatch(args) => cmd_apply_patch(machine, args),
        AstCommand::Validate(args) => cmd_validate(args),
        AstCommand::Canon(args) => cmd_canon(machine, args),
        AstCommand::Schema(args) => cmd_schema(machine, args),
        AstCommand::Grammar(args) => cmd_grammar(args),
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

fn cmd_init(
    machine: &crate::reporting::MachineArgs,
    args: AstInitArgs,
) -> Result<std::process::ExitCode> {
    let out_path = machine
        .out
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ast init: missing --out <PATH>"))?
        .clone();

    let schema_version = match args.schema_version.as_deref() {
        None => X07AST_SCHEMA_VERSION,
        Some(X07AST_SCHEMA_VERSION_V0_3_0) => X07AST_SCHEMA_VERSION_V0_3_0,
        Some(X07AST_SCHEMA_VERSION_V0_4_0) => X07AST_SCHEMA_VERSION_V0_4_0,
        Some(X07AST_SCHEMA_VERSION_V0_5_0) => X07AST_SCHEMA_VERSION_V0_5_0,
        Some(other) => {
            anyhow::bail!(
                "unsupported schema_version: expected {} got {other:?}",
                X07AST_SCHEMA_VERSIONS_SUPPORTED.join(", ")
            )
        }
    };

    if args.module.is_empty() || args.module.chars().any(|c| c.is_whitespace()) {
        let report = AstInitReport {
            ok: false,
            out: out_path.display().to_string(),
            schema_version: schema_version.to_string(),
            template_id: format!("{}/{}@v1", args.world.as_str(), args.module),
            sha256: String::new(),
        };
        print_json(&report)?;
        return Ok(std::process::ExitCode::from(2));
    }

    let mut doc = serde_json::Map::new();
    doc.insert(
        "schema_version".to_string(),
        Value::String(schema_version.to_string()),
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

    util::write_atomic(&out_path, &out_bytes)
        .with_context(|| format!("write: {}", out_path.display()))?;

    let report = AstInitReport {
        ok: true,
        out: out_path.display().to_string(),
        schema_version: schema_version.to_string(),
        template_id: format!("{}/{}@v1", args.world.as_str(), args.module),
        sha256: sha256_hex(&out_bytes),
    };
    print_json(&report)?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_get(
    machine: &crate::reporting::MachineArgs,
    args: AstGetArgs,
) -> Result<std::process::ExitCode> {
    let out_path = machine.out.as_ref();
    let input_bytes = match std::fs::read(&args.r#in) {
        Ok(bytes) => bytes,
        Err(err) => {
            let report = AstGetReport {
                ok: false,
                r#in: args.r#in.display().to_string(),
                ptr: args.ptr.clone(),
                out: out_path.map(|p| p.display().to_string()),
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
                    out: out_path.map(|p| p.display().to_string()),
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
                out: out_path.map(|p| p.display().to_string()),
                error: Some(msg),
                value: None,
            };
            print_json(&report)?;
            return Ok(std::process::ExitCode::from(20));
        }
    };

    if let Some(out_path) = out_path {
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
        util::write_atomic(out_path, &out_bytes)
            .with_context(|| format!("write: {}", out_path.display()))?;
    }

    let report = AstGetReport {
        ok: true,
        r#in: args.r#in.display().to_string(),
        ptr: args.ptr.clone(),
        out: out_path.map(|p| p.display().to_string()),
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

fn cmd_apply_patch(
    machine: &crate::reporting::MachineArgs,
    args: AstApplyPatchArgs,
) -> Result<std::process::ExitCode> {
    let out_path = machine.out.clone().unwrap_or_else(|| args.r#in.clone());
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

    util::write_atomic(&out_path, &out_bytes)
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
        util::write_atomic(path, serde_json::to_string(&report)?.as_bytes())
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

fn cmd_canon(
    machine: &crate::reporting::MachineArgs,
    args: AstCanonArgs,
) -> Result<std::process::ExitCode> {
    let out_path = machine.out.clone().unwrap_or_else(|| args.r#in.clone());
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

    util::write_atomic(&out_path, &out_bytes)
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

#[derive(Debug, Serialize)]
struct GrammarVariant {
    name: String,
    cfg: String,
}

#[derive(Debug, Serialize)]
struct GrammarSha256 {
    min_cfg: String,
    pretty_cfg: String,
    semantic_supplement: String,
}

#[derive(Debug, Serialize)]
struct GrammarBundle {
    schema_version: String,
    x07ast_schema_version: String,
    format: String,
    variants: Vec<GrammarVariant>,
    semantic_supplement: Value,
    sha256: GrammarSha256,
}

#[derive(Debug, Serialize)]
struct GenpackManifestArtifact {
    name: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct GenpackManifest {
    schema_version: String,
    pack: String,
    pack_version: String,
    x07ast_schema_version: String,
    artifacts: Vec<GenpackManifestArtifact>,
}

fn cmd_schema(
    machine: &crate::reporting::MachineArgs,
    args: AstSchemaArgs,
) -> Result<std::process::ExitCode> {
    let schema_bytes = match args.schema_version.as_deref() {
        None => X07AST_SCHEMA_BYTES,
        Some(X07AST_SCHEMA_VERSION_V0_3_0) => X07AST_SCHEMA_V0_3_BYTES,
        Some(X07AST_SCHEMA_VERSION_V0_4_0) => X07AST_SCHEMA_V0_4_BYTES,
        Some(X07AST_SCHEMA_VERSION_V0_5_0) => X07AST_SCHEMA_V0_5_BYTES,
        Some(other) => {
            anyhow::bail!(
                "unsupported schema_version: expected {} got {other:?}",
                X07AST_SCHEMA_VERSIONS_SUPPORTED.join(", ")
            )
        }
    };

    let out_bytes = if args.pretty {
        let doc: Value =
            serde_json::from_slice(schema_bytes).context("parse spec/x07ast.schema.json")?;
        with_trailing_newline(serde_json::to_string_pretty(&doc)?.into_bytes())
    } else {
        with_trailing_newline(schema_bytes.to_vec())
    };

    if let Some(out_path) = machine.out.as_ref() {
        util::write_atomic(out_path, &out_bytes)
            .with_context(|| format!("write: {}", out_path.display()))?;
    } else {
        write_stdout_bytes(&out_bytes)?;
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_grammar(args: AstGrammarArgs) -> Result<std::process::ExitCode> {
    let min_cfg_bytes = with_trailing_newline(X07AST_MIN_GBNF_BYTES.to_vec());
    let pretty_cfg_bytes = with_trailing_newline(X07AST_PRETTY_GBNF_BYTES.to_vec());
    let semantic_supplement_bytes = with_trailing_newline(X07AST_SEMANTIC_BYTES.to_vec());
    let schema_bytes = with_trailing_newline(X07AST_SCHEMA_BYTES.to_vec());

    let semantic_supplement: Value = serde_json::from_slice(&semantic_supplement_bytes)
        .context("parse spec/x07ast.semantic.json")?;
    validate_semantic_supplement_doc(&semantic_supplement)?;

    let min_cfg = embedded_utf8_string(&min_cfg_bytes, "spec/x07ast.min.gbnf")?;
    let pretty_cfg = embedded_utf8_string(&pretty_cfg_bytes, "spec/x07ast.pretty.gbnf")?;

    let bundle = GrammarBundle {
        schema_version: X07_AST_GRAMMAR_BUNDLE_SCHEMA_VERSION.to_string(),
        x07ast_schema_version: X07AST_SCHEMA_VERSION.to_string(),
        format: "gbnf_v1".to_string(),
        variants: vec![
            GrammarVariant {
                name: "min".to_string(),
                cfg: min_cfg,
            },
            GrammarVariant {
                name: "pretty".to_string(),
                cfg: pretty_cfg,
            },
        ],
        semantic_supplement,
        sha256: GrammarSha256 {
            min_cfg: sha256_hex(&min_cfg_bytes),
            pretty_cfg: sha256_hex(&pretty_cfg_bytes),
            semantic_supplement: sha256_hex(&semantic_supplement_bytes),
        },
    };

    let bundle_bytes = with_trailing_newline(serde_json::to_vec(&bundle)?);
    write_stdout_bytes(&bundle_bytes)?;

    if let Some(out_dir) = &args.out_dir {
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("create out-dir: {}", out_dir.display()))?;

        let schema_path = out_dir.join("x07ast.schema.json");
        let min_cfg_path = out_dir.join("x07ast.min.gbnf");
        let pretty_cfg_path = out_dir.join("x07ast.pretty.gbnf");
        let semantic_path = out_dir.join("x07ast.semantic.json");
        let manifest_path = out_dir.join("manifest.json");

        util::write_atomic(&schema_path, &schema_bytes)
            .with_context(|| format!("write: {}", schema_path.display()))?;
        util::write_atomic(&min_cfg_path, &min_cfg_bytes)
            .with_context(|| format!("write: {}", min_cfg_path.display()))?;
        util::write_atomic(&pretty_cfg_path, &pretty_cfg_bytes)
            .with_context(|| format!("write: {}", pretty_cfg_path.display()))?;
        util::write_atomic(&semantic_path, &semantic_supplement_bytes)
            .with_context(|| format!("write: {}", semantic_path.display()))?;

        let manifest = GenpackManifest {
            schema_version: X07_GENPACK_MANIFEST_SCHEMA_VERSION.to_string(),
            pack: X07_GENPACK_NAME.to_string(),
            pack_version: X07_GENPACK_VERSION.to_string(),
            x07ast_schema_version: X07AST_SCHEMA_VERSION.to_string(),
            artifacts: vec![
                GenpackManifestArtifact {
                    name: "x07ast.schema.json".to_string(),
                    sha256: sha256_hex(&schema_bytes),
                },
                GenpackManifestArtifact {
                    name: "x07ast.min.gbnf".to_string(),
                    sha256: sha256_hex(&min_cfg_bytes),
                },
                GenpackManifestArtifact {
                    name: "x07ast.pretty.gbnf".to_string(),
                    sha256: sha256_hex(&pretty_cfg_bytes),
                },
                GenpackManifestArtifact {
                    name: "x07ast.semantic.json".to_string(),
                    sha256: sha256_hex(&semantic_supplement_bytes),
                },
            ],
        };
        let manifest_bytes = with_trailing_newline(serde_json::to_vec(&manifest)?);
        util::write_atomic(&manifest_path, &manifest_bytes)
            .with_context(|| format!("write: {}", manifest_path.display()))?;
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn write_stdout_bytes(bytes: &[u8]) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(bytes)?;
    stdout.flush()?;
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
    let schema_bytes = match doc.get("schema_version").and_then(|v| v.as_str()) {
        Some(X07AST_SCHEMA_VERSION_V0_3_0) => X07AST_SCHEMA_V0_3_BYTES,
        Some(X07AST_SCHEMA_VERSION_V0_4_0) => X07AST_SCHEMA_V0_4_BYTES,
        Some(X07AST_SCHEMA_VERSION_V0_5_0) => X07AST_SCHEMA_V0_5_BYTES,
        Some(other) => {
            let mut data = BTreeMap::new();
            data.insert("got".to_string(), Value::String(other.to_string()));
            data.insert(
                "supported".to_string(),
                Value::Array(
                    X07AST_SCHEMA_VERSIONS_SUPPORTED
                        .iter()
                        .map(|s| Value::String((*s).to_string()))
                        .collect(),
                ),
            );

            return Ok(vec![diagnostics::Diagnostic {
                code: "X07-SCHEMA-0002".to_string(),
                severity: diagnostics::Severity::Error,
                stage: diagnostics::Stage::Parse,
                message: format!(
                    "unsupported schema_version: {other:?} (supported: {})",
                    X07AST_SCHEMA_VERSIONS_SUPPORTED.join(", ")
                ),
                loc: Some(diagnostics::Location::X07Ast {
                    ptr: "/schema_version".to_string(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data,
                quickfix: Some(diagnostics::Quickfix {
                    kind: diagnostics::QuickfixKind::JsonPatch,
                    patch: vec![diagnostics::PatchOp::Replace {
                        path: "/schema_version".to_string(),
                        value: Value::String(X07AST_SCHEMA_VERSION.to_string()),
                    }],
                    note: Some(format!("Set schema_version to {}", X07AST_SCHEMA_VERSION)),
                }),
            }]);
        }
        // If schema_version is missing or not a string, validate against the currently
        // emitted default schema. The schema validator will report the missing/invalid
        // field deterministically.
        None => X07AST_SCHEMA_BYTES,
    };

    let schema_json: Value =
        serde_json::from_slice(schema_bytes).context("parse x07ast JSON schema")?;
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema_json)
        .context("build x07ast schema validator")?;

    let mut out = Vec::new();
    for error in validator.iter_errors(doc) {
        let mut data = BTreeMap::new();
        data.insert(
            "instance_path".to_string(),
            Value::String(error.instance_path().to_string()),
        );
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

fn validate_semantic_supplement_doc(doc: &Value) -> Result<()> {
    let semantic_schema: Value = serde_json::from_slice(X07AST_SEMANTIC_SCHEMA_BYTES)
        .context("parse spec/x07ast.semantic.schema.json")?;
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&semantic_schema)
        .context("build x07ast semantic supplement schema validator")?;

    let mut errors = validator.iter_errors(doc).peekable();
    if let Some(first) = errors.next() {
        anyhow::bail!(
            "semantic supplement is invalid at {}: {}",
            first.instance_path(),
            first
        );
    }

    let actual_version = doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if actual_version != X07_AST_SEMANTIC_SCHEMA_VERSION {
        anyhow::bail!(
            "semantic supplement schema_version mismatch: expected {}, got {}",
            X07_AST_SEMANTIC_SCHEMA_VERSION,
            actual_version
        );
    }

    Ok(())
}

fn canonicalize_x07ast_bytes_to_value(bytes: &[u8]) -> Result<Value> {
    let mut file = x07c::x07ast::parse_x07ast_json(bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    Ok(x07c::x07ast::x07ast_file_to_value(&file))
}

fn exit_with_error(err: impl std::fmt::Display) -> std::process::ExitCode {
    eprintln!("{err}");
    std::process::ExitCode::from(2)
}

fn embedded_utf8_string(bytes: &[u8], label: &str) -> Result<String> {
    String::from_utf8(bytes.to_vec()).with_context(|| format!("decode UTF-8: {label}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_semantic_supplement_is_schema_valid() {
        let doc: Value =
            serde_json::from_slice(X07AST_SEMANTIC_BYTES).expect("parse spec/x07ast.semantic.json");
        validate_semantic_supplement_doc(&doc).expect("validate semantic supplement");
    }
}
