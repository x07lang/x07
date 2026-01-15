use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "x07import")]
#[command(about = "Deterministic stdlib importer: Rust/C -> X07 modules.", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Import a Rust file into a module.
    Rust {
        #[arg(long)]
        r#in: PathBuf,
        #[arg(long)]
        module_id: String,
        /// Module root directory (writes <out>/<module_id>.x07.json).
        #[arg(long)]
        out: PathBuf,
        /// If set, fail if output differs; do not write.
        #[arg(long, default_value_t = false)]
        check: bool,
    },
    /// Import a C file into a module.
    C {
        #[arg(long)]
        r#in: PathBuf,
        #[arg(long)]
        module_id: String,
        /// Module root directory (writes <out>/<module_id>.x07.json).
        #[arg(long)]
        out: PathBuf,
        /// If set, fail if output differs; do not write.
        #[arg(long, default_value_t = false)]
        check: bool,
    },
    /// Import multiple modules from a manifest.
    Batch {
        #[arg(long)]
        manifest: PathBuf,
        /// If set, fail if any output differs; do not write.
        #[arg(long, default_value_t = false)]
        check: bool,
    },
}

fn main() -> Result<()> {
    try_main().map_err(|err| {
        eprintln!("{err:#}");
        err
    })
}

fn try_main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Rust {
            r#in,
            module_id,
            out,
            check,
        } => run_rust(&r#in, &module_id, &out, check),
        Command::C {
            r#in,
            module_id,
            out,
            check,
        } => run_c(&r#in, &module_id, &out, check),
        Command::Batch { manifest, check } => run_batch(&manifest, check),
    }
}

fn run_rust(
    src_path: &std::path::Path,
    module_id: &str,
    out_root: &std::path::Path,
    check: bool,
) -> Result<()> {
    let src = std::fs::read_to_string(src_path)
        .with_context(|| format!("read Rust source: {}", src_path.display()))?;
    let module = x07import_core::rust::import_rust_file(module_id, src_path, &src)?;
    let out_src = x07import_core::x07_emit::emit_module(&module)?;
    write_module(out_root, module_id, &out_src, check)
}

fn run_c(
    src_path: &std::path::Path,
    module_id: &str,
    out_root: &std::path::Path,
    check: bool,
) -> Result<()> {
    let module = x07import_core::c::import_c_file(module_id, src_path)?;
    let out_src = x07import_core::x07_emit::emit_module(&module)?;
    write_module(out_root, module_id, &out_src, check)
}

#[derive(Debug, serde::Deserialize)]
struct Manifest {
    schema_version: String,
    entries: Vec<ManifestEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct ManifestEntry {
    module_id: String,
    kind: String,
    source: String,
    out_root: String,
}

fn run_batch(manifest_path: &PathBuf, check: bool) -> Result<()> {
    let bytes = std::fs::read(manifest_path)
        .with_context(|| format!("read manifest: {}", manifest_path.display()))?;
    let m: Manifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse manifest JSON: {}", manifest_path.display()))?;
    if m.schema_version.trim() != "x07import.manifest@0.1.0" {
        anyhow::bail!(
            "manifest schema_version mismatch: expected x07import.manifest@0.1.0 got {:?}",
            m.schema_version
        );
    }

    for (idx, e) in m.entries.iter().enumerate() {
        let out_root = PathBuf::from(&e.out_root);
        let src_path = PathBuf::from(&e.source);
        match e.kind.as_str() {
            "rust" => run_rust(&src_path, &e.module_id, &out_root, check)
                .with_context(|| format!("manifest entry[{idx}] rust"))?,
            "c" => run_c(&src_path, &e.module_id, &out_root, check)
                .with_context(|| format!("manifest entry[{idx}] c"))?,
            other => anyhow::bail!(
                "manifest entry[{idx}] has unknown kind {:?} (expected 'rust' or 'c')",
                other
            ),
        }
    }
    Ok(())
}

fn write_module(out_root: &std::path::Path, module_id: &str, src: &str, check: bool) -> Result<()> {
    let mut rel = PathBuf::new();
    for seg in module_id.split('.') {
        rel.push(seg);
    }
    rel.set_extension("x07.json");
    let out_path = out_root.join(rel);

    if check {
        let cur = std::fs::read_to_string(&out_path)
            .with_context(|| format!("read existing output: {}", out_path.display()))?;
        if cur != src {
            anyhow::bail!("generated output differs: {}", out_path.display());
        }
        return Ok(());
    }

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output dir: {}", parent.display()))?;
    }
    std::fs::write(&out_path, src.as_bytes())
        .with_context(|| format!("write output: {}", out_path.display()))?;
    Ok(())
}
