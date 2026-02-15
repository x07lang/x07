use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;
use clap::{Args, Subcommand};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;
use x07_contracts::X07AST_SCHEMA_VERSION;

use crate::util;

#[derive(Debug, Clone, Args)]
pub struct AssetsArgs {
    #[command(subcommand)]
    pub cmd: Option<AssetsCommand>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AssetsCommand {
    /// Embed a directory of files into an X07 module (base64 payloads).
    EmbedDir(EmbedDirArgs),
}

#[derive(Debug, Clone, Args)]
pub struct EmbedDirArgs {
    /// Input directory to embed.
    #[arg(long, value_name = "DIR")]
    pub r#in: PathBuf,

    /// Module id to write (e.g. "my.assets").
    #[arg(long, value_name = "MODULE_ID")]
    pub module_id: String,

    /// Output .x07.json path.
    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,

    /// Optional prefix to strip from embedded paths (defaults to --in).
    #[arg(long, value_name = "PATH")]
    pub strip_prefix: Option<PathBuf>,

    /// Include glob (repeatable).
    #[arg(long, value_name = "GLOB")]
    pub glob_include: Vec<String>,

    /// Exclude glob (repeatable).
    #[arg(long, value_name = "GLOB")]
    pub glob_exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AssetsError {
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct AssetsEmbedDirReport {
    ok: bool,
    command: &'static str,
    r#in: String,
    out: String,
    module_id: String,
    file_count: usize,
    files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<AssetsError>,
}

pub fn cmd_assets(
    _machine: &crate::reporting::MachineArgs,
    args: AssetsArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing assets subcommand (try --help)");
    };
    match cmd {
        AssetsCommand::EmbedDir(args) => cmd_assets_embed_dir(args),
    }
}

fn cmd_assets_embed_dir(args: EmbedDirArgs) -> Result<std::process::ExitCode> {
    let res = embed_dir(&args);
    let report = match res {
        Ok(ok) => ok,
        Err(err) => AssetsEmbedDirReport {
            ok: false,
            command: "assets.embed-dir",
            r#in: args.r#in.display().to_string(),
            out: args.out.display().to_string(),
            module_id: args.module_id,
            file_count: 0,
            files: Vec::new(),
            error: Some(AssetsError {
                message: format!("{err:#}"),
            }),
        },
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(20)
    })
}

fn embed_dir(args: &EmbedDirArgs) -> Result<AssetsEmbedDirReport> {
    let in_abs =
        resolve_abs_dir(&args.r#in).with_context(|| format!("resolve --in {:?}", args.r#in))?;
    if !in_abs.is_dir() {
        anyhow::bail!("--in is not a directory: {}", in_abs.display());
    }

    let strip_prefix = args.strip_prefix.as_ref().unwrap_or(&args.r#in);
    let strip_abs = resolve_abs_dir(strip_prefix)
        .with_context(|| format!("resolve --strip-prefix {:?}", strip_prefix))?;

    let include_globs = if args.glob_include.is_empty() {
        vec!["**/*".to_string()]
    } else {
        args.glob_include.clone()
    };
    let exclude_globs = if args.glob_exclude.is_empty() {
        vec![
            "**/.git/**".to_string(),
            "**/.x07/**".to_string(),
            "**/target/**".to_string(),
            "**/node_modules/**".to_string(),
            "**/dist/**".to_string(),
        ]
    } else {
        args.glob_exclude.clone()
    };

    let include = compile_globset(&include_globs).context("compile --glob-include")?;
    let exclude = compile_globset(&exclude_globs).context("compile --glob-exclude")?;

    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for entry in WalkDir::new(&in_abs)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let abs = entry.into_path();
        let rel = abs.strip_prefix(&strip_abs).with_context(|| {
            format!(
                "strip prefix {} from {}",
                strip_abs.display(),
                abs.display()
            )
        })?;
        let rel_text = rel_path_slash(rel)?;
        if !include.is_match(&rel_text) {
            continue;
        }
        if exclude.is_match(&rel_text) {
            continue;
        }
        found.push((rel_text, abs));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));

    if found.is_empty() {
        anyhow::bail!("no files matched");
    }

    let mut files: Vec<(String, String)> = Vec::with_capacity(found.len());
    for (rel, abs) in &found {
        let bytes = std::fs::read(abs).with_context(|| format!("read {}", abs.display()))?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        files.push((rel.clone(), b64));
    }

    let paths_text = {
        let mut out = String::new();
        for (idx, (rel, _)) in files.iter().enumerate() {
            if idx != 0 {
                out.push('\n');
            }
            out.push_str(rel);
        }
        out
    };

    let module = assets_module_value(&args.module_id, &paths_text, &files)?;
    let mut file = x07c::x07ast::parse_x07ast_json(serde_json::to_vec(&module)?.as_slice())
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("parse generated x07ast")?;
    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    let mut v = x07c::x07ast::x07ast_file_to_value(&file);
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut module_bytes = serde_json::to_string(&v)?.into_bytes();
    module_bytes.push(b'\n');

    util::write_atomic(&args.out, &module_bytes)
        .with_context(|| format!("write formatted x07ast module to {}", args.out.display()))?;

    Ok(AssetsEmbedDirReport {
        ok: true,
        command: "assets.embed-dir",
        r#in: in_abs.display().to_string(),
        out: args.out.display().to_string(),
        module_id: args.module_id.clone(),
        file_count: files.len(),
        files: files.into_iter().map(|(p, _)| p).collect(),
        error: None,
    })
}

fn resolve_abs_dir(p: &Path) -> Result<PathBuf> {
    if p.is_absolute() {
        return Ok(p.to_path_buf());
    }
    Ok(std::env::current_dir().context("get current dir")?.join(p))
}

fn compile_globset(globs: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for g in globs {
        builder.add(Glob::new(g).with_context(|| format!("invalid glob: {g:?}"))?);
    }
    builder.build().context("build globset")
}

fn rel_path_slash(path: &Path) -> Result<String> {
    let mut parts: Vec<&str> = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(s) => {
                let s = s.to_str().context("path contains non-utf8 component")?;
                parts.push(s);
            }
            other => {
                anyhow::bail!("unsupported relative path component: {other:?}");
            }
        }
    }
    Ok(parts.join("/"))
}

fn assets_module_value(
    module_id: &str,
    paths_text: &str,
    files: &[(String, String)],
) -> Result<Value> {
    let f_paths = format!("{module_id}.paths_text_v1");
    let f_get = format!("{module_id}.get_b64_v1");

    let get_body = get_b64_body(files);
    Ok(serde_json::json!({
      "schema_version": X07AST_SCHEMA_VERSION,
      "kind":"module",
      "module_id": module_id,
      "imports": ["std.bytes"],
      "decls": [
        {"kind":"export","names":[f_paths, f_get]},
        {"kind":"defn","name":f_paths,"params":[],"result":"bytes","body":["bytes.lit", paths_text]},
        {"kind":"defn","name":f_get,"params":[{"name":"path","ty":"bytes_view"}],"result":"option_bytes","body": get_body}
      ]
    }))
}

fn get_b64_body(files: &[(String, String)]) -> Value {
    let mut begin: Vec<Value> = Vec::with_capacity(1 + (files.len() * 2) + 1);
    begin.push(Value::String("begin".to_string()));

    for (idx, (path, b64)) in files.iter().enumerate() {
        let p_var = format!("_p_{idx}");
        begin.push(serde_json::json!(["let", p_var, ["bytes.lit", path]]));
        begin.push(serde_json::json!([
            "if",
            ["=", ["std.bytes.eq", "path", ["bytes.view", p_var]], 1],
            ["return", ["option_bytes.some", ["bytes.lit", b64]]],
            0
        ]));
    }

    begin.push(serde_json::json!(["option_bytes.none"]));
    Value::Array(begin)
}
