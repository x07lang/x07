use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use walkdir::WalkDir;
use x07_worlds::WorldId;
use x07c::compile;
use x07c::diagnostics;
use x07c::lint;
use x07c::module_source;
use x07c::typecheck;
use x07c::x07ast;

use crate::repair::{RepairArgs, RepairMode};

fn should_walk_dir_entry(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if !entry.file_type().is_dir() {
        return true;
    }
    !matches!(
        name.as_ref(),
        ".git" | ".x07" | "target" | ".agent" | ".claude"
    )
}

fn collect_x07ast_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for input in inputs {
        if input.is_file() {
            if seen.insert(input.clone()) {
                out.push(input.clone());
            }
            continue;
        }
        if input.is_dir() {
            let mut files: Vec<PathBuf> = Vec::new();
            for entry in WalkDir::new(input)
                .follow_links(false)
                .into_iter()
                .filter_entry(should_walk_dir_entry)
                .flatten()
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.into_path();
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(".x07.json"))
                {
                    files.push(path);
                }
            }
            files.sort();
            for file in files {
                if seen.insert(file.clone()) {
                    out.push(file);
                }
            }
            continue;
        }

        anyhow::bail!(
            "--input does not exist or is not a file/dir: {}",
            input.display()
        );
    }

    if out.is_empty() {
        anyhow::bail!("no *.x07.json inputs found");
    }

    Ok(out)
}

#[derive(Debug, Clone, Args)]
pub struct FmtArgs {
    #[arg(long, value_name = "PATH")]
    pub input: Vec<PathBuf>,
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub write: bool,
    /// Deterministic multi-line formatting intended for human editing/review.
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Clone, Args)]
pub struct LintArgs {
    #[arg(long, value_name = "PATH", required = true)]
    pub input: Vec<PathBuf>,
    /// Lint world gating (advanced; the public surface defaults to `run-os`).
    #[arg(long, value_enum, default_value_t = WorldId::RunOs, hide = true)]
    pub world: WorldId,
}

#[derive(Debug, Clone, Args)]
pub struct FixArgs {
    #[arg(long, value_name = "PATH", required_unless_present = "from_pbt")]
    pub input: Vec<PathBuf>,
    /// Convert a PBT repro artifact into a deterministic regression test (writes wrapper module + manifest entry).
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with = "input",
        conflicts_with = "suggest_generics"
    )]
    pub from_pbt: Option<PathBuf>,
    /// Tests manifest to patch in `--from-pbt` mode.
    #[arg(long, value_name = "PATH", default_value = "tests/tests.json")]
    pub tests_manifest: PathBuf,
    /// Output dir for wrapper/repro files in `--from-pbt` mode.
    ///
    /// If relative, it is resolved relative to the tests manifest directory.
    #[arg(long, value_name = "DIR", default_value = "repro/pbt")]
    pub out_dir: PathBuf,
    /// Fix world gating (advanced; the public surface defaults to `run-os`).
    #[arg(long, value_enum, default_value_t = WorldId::RunOs, hide = true)]
    pub world: WorldId,
    #[arg(long)]
    pub write: bool,
    /// Emit a suggested `x07.patchset@0.1.0` that migrates near-identical type-suffixed functions
    /// into a single generic base plus typed wrappers.
    #[arg(long)]
    pub suggest_generics: bool,
}

#[derive(Debug, Clone, Args)]
pub struct MigrateArgs {
    /// Input file or directory.
    ///
    /// If omitted, defaults to the current directory.
    #[arg(long, value_name = "PATH")]
    pub input: Vec<PathBuf>,

    /// Target language/toolchain compatibility version (example: `0.5`).
    #[arg(long, value_name = "COMPAT_VERSION")]
    pub to: String,

    /// Check whether migrations are required (default when neither flag is set).
    #[arg(long)]
    pub check: bool,

    /// Apply migrations in-place.
    #[arg(long)]
    pub write: bool,

    /// Maximum migration iterations.
    #[arg(long, value_name = "N", default_value_t = 3)]
    pub max_iters: u32,
}

#[derive(Debug, Clone, Args)]
pub struct BuildArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: PathBuf,

    /// Override the language/toolchain compatibility mode.
    #[arg(long, value_name = "COMPAT")]
    pub compat: Option<String>,

    /// Emit the runtime C header (requires `emit_main=false`; use `--freestanding` for embedding).
    #[arg(long, value_name = "PATH")]
    pub emit_c_header: Option<PathBuf>,

    /// Also emit a core wasm module (solve_pure ABI), written to this path.
    #[arg(long = "emit-wasm", value_name = "FILE")]
    pub emit_wasm: Option<PathBuf>,

    /// WebAssembly initial memory size in bytes (must be multiple of 65536).
    #[arg(long = "wasm-initial-memory-bytes", value_name = "N")]
    pub wasm_initial_memory_bytes: Option<u64>,

    /// WebAssembly max memory size in bytes (must be multiple of 65536).
    #[arg(long = "wasm-max-memory-bytes", value_name = "N")]
    pub wasm_max_memory_bytes: Option<u64>,

    /// If set, emit a non-growable memory (max == initial).
    #[arg(long = "wasm-no-growable-memory", action = clap::ArgAction::SetTrue)]
    pub wasm_no_growable_memory: bool,

    /// Build in freestanding mode for library embedding (exports `x07_solve_v2`; no `main()`).
    #[arg(long)]
    pub freestanding: bool,

    /// Override the generated C source size budget (in bytes).
    #[arg(long, value_name = "BYTES")]
    pub max_c_bytes: Option<usize>,

    #[command(flatten)]
    pub repair: RepairArgs,
}

#[derive(Debug, Clone, Args)]
pub struct CheckArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: PathBuf,

    /// Override the language/toolchain compatibility mode.
    #[arg(long, value_name = "COMPAT")]
    pub compat: Option<String>,
}

pub fn cmd_fmt(
    _machine: &crate::reporting::MachineArgs,
    args: FmtArgs,
) -> Result<std::process::ExitCode> {
    if args.check == args.write {
        anyhow::bail!("set exactly one of --check or --write");
    }

    let mut raw_inputs = args.input;
    raw_inputs.extend(args.paths);
    if raw_inputs.is_empty() {
        anyhow::bail!("missing input (use --input <PATH> or pass PATH... as positional args)");
    }

    let inputs = collect_x07ast_inputs(&raw_inputs).context("collect inputs")?;
    let mut not_formatted: Vec<PathBuf> = Vec::new();

    for input in &inputs {
        let bytes =
            std::fs::read(input).with_context(|| format!("read input: {}", input.display()))?;

        let mut file = match x07ast::parse_x07ast_json(&bytes) {
            Ok(file) => file,
            Err(err) => {
                return Err(anyhow::anyhow!("{err}"))
                    .with_context(|| format!("parse x07ast JSON: {}", input.display()));
            }
        };

        x07ast::canonicalize_x07ast_file(&mut file);
        let mut v = x07ast::x07ast_file_to_value(&file);
        x07ast::canon_value_jcs(&mut v);
        let formatted = if args.pretty {
            serde_json::to_string_pretty(&v)? + "\n"
        } else {
            serde_json::to_string(&v)? + "\n"
        };

        if args.check && bytes != formatted.as_bytes() {
            not_formatted.push(input.clone());
            continue;
        }

        if args.write && bytes != formatted.as_bytes() {
            std::fs::write(input, formatted.as_bytes())
                .with_context(|| format!("write: {}", input.display()))?;
        }
    }

    if !not_formatted.is_empty() {
        for p in not_formatted {
            eprintln!("file is not formatted: {}", p.display());
        }
        return Ok(std::process::ExitCode::from(1));
    }

    Ok(std::process::ExitCode::SUCCESS)
}

pub fn cmd_lint(
    machine: &crate::reporting::MachineArgs,
    args: LintArgs,
) -> Result<std::process::ExitCode> {
    let inputs = collect_x07ast_inputs(&args.input).context("collect inputs")?;

    let lint_options = x07c::world_config::lint_options_for_world(args.world);
    let mut all_diags: Vec<diagnostics::Diagnostic> = Vec::new();
    let mut ok = true;

    for input in &inputs {
        let bytes =
            std::fs::read(input).with_context(|| format!("read input: {}", input.display()))?;
        let mut file = match x07ast::parse_x07ast_json(&bytes) {
            Ok(file) => file,
            Err(err) => {
                return Err(anyhow::anyhow!("{err}"))
                    .with_context(|| format!("parse x07ast JSON: {}", input.display()));
            }
        };

        x07ast::canonicalize_x07ast_file(&mut file);
        let report = lint::lint_file(&file, lint_options);
        if !report.ok {
            ok = false;
        }
        for mut d in report.diagnostics {
            d.data.insert(
                "file".to_string(),
                serde_json::Value::String(input.display().to_string()),
            );
            all_diags.push(d);
        }
    }

    all_diags.sort_by(|a, b| {
        let ap = a.data.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let bp = b.data.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let a_ptr = a
            .loc
            .as_ref()
            .and_then(|l| match l {
                diagnostics::Location::X07Ast { ptr } => Some(ptr.as_str()),
                diagnostics::Location::Text { .. } => None,
            })
            .unwrap_or("");
        let b_ptr = b
            .loc
            .as_ref()
            .and_then(|l| match l {
                diagnostics::Location::X07Ast { ptr } => Some(ptr.as_str()),
                diagnostics::Location::Text { .. } => None,
            })
            .unwrap_or("");
        ap.cmp(bp)
            .then_with(|| a_ptr.cmp(b_ptr))
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.message.cmp(&b.message))
    });

    let mut report = diagnostics::Report::ok();
    report.ok = ok;
    report.diagnostics = all_diags;
    report.meta.insert(
        "inputs".to_string(),
        serde_json::Value::Array(
            inputs
                .iter()
                .map(|p| serde_json::Value::String(p.display().to_string()))
                .collect(),
        ),
    );

    let out = serde_json::to_string(&report)? + "\n";
    if let Some(path) = machine.out.as_deref() {
        crate::reporting::write_bytes(path, out.as_bytes())?;
    } else {
        print!("{out}");
    }

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

pub fn cmd_fix(
    machine: &crate::reporting::MachineArgs,
    args: FixArgs,
) -> Result<std::process::ExitCode> {
    if args.suggest_generics && args.write {
        anyhow::bail!("--suggest-generics cannot be combined with --write");
    }

    if args.from_pbt.is_some() {
        let repro_path = args.from_pbt.as_ref().context("from_pbt")?;

        let out_dir = if args.out_dir.is_absolute() {
            args.out_dir.clone()
        } else {
            let base = args
                .tests_manifest
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            base.join(&args.out_dir)
        };

        let (report_bytes, exit_code) = match crate::pbt_fix::cmd_fix_from_pbt(
            repro_path,
            &args.tests_manifest,
            &out_dir,
            args.write,
        ) {
            Ok(outcome) => (
                crate::pbt_fix::fix_from_pbt_report_bytes(&outcome).context("encode report")?,
                0u8,
            ),
            Err(err) => {
                if let Some(known) = err.downcast_ref::<crate::pbt_fix::FixFromPbtError>() {
                    (
                        crate::pbt_fix::fix_from_pbt_error_report_bytes(repro_path, known)
                            .context("encode error report")?,
                        known.exit_code(),
                    )
                } else {
                    return Err(err).context("fix-from-pbt");
                }
            }
        };

        if let Some(path) = machine.out.as_deref() {
            crate::reporting::write_bytes(path, &report_bytes)?;
        } else {
            std::io::Write::write_all(&mut std::io::stdout(), &report_bytes)
                .context("write stdout")?;
        }

        return Ok(std::process::ExitCode::from(exit_code));
    }

    if args.write && machine.out.is_some() {
        anyhow::bail!("--out cannot be combined with --write");
    }

    let inputs = collect_x07ast_inputs(&args.input).context("collect inputs")?;

    if args.suggest_generics {
        if inputs.len() != 1 {
            anyhow::bail!("--suggest-generics expects exactly one input file");
        }

        let input = inputs.first().context("input")?;
        let bytes =
            std::fs::read(input).with_context(|| format!("read input: {}", input.display()))?;

        let patchset = crate::fix_suggest::suggest_generics_patchset(input, &bytes)
            .context("suggest-generics")?;
        let mut out = crate::util::canonical_jcs_bytes(&patchset)?;
        if out.last() != Some(&b'\n') {
            out.push(b'\n');
        }

        match machine.out.as_deref() {
            Some(path) => crate::reporting::write_bytes(path, &out)?,
            None => {
                std::io::Write::write_all(&mut std::io::stdout(), &out).context("write stdout")?
            }
        }

        return Ok(std::process::ExitCode::SUCCESS);
    }

    if !args.write && inputs.len() != 1 {
        anyhow::bail!("multiple inputs require --write");
    }

    let repair_mode = if args.write {
        RepairMode::Write
    } else {
        RepairMode::Memory
    };
    let mut ok = true;

    for input in &inputs {
        let bytes =
            std::fs::read(input).with_context(|| format!("read input: {}", input.display()))?;
        let mut doc: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(doc) => doc,
            Err(err) => {
                return Err(err).with_context(|| format!("parse JSON: {}", input.display()));
            }
        };

        let repair_result =
            crate::repair::repair_x07ast_file_doc(&mut doc, args.world, 5, repair_mode)
                .with_context(|| format!("fix: {}", input.display()))?;
        let formatted = repair_result.formatted;
        let final_report = repair_result.final_report;

        let remaining_errors: usize = final_report
            .diagnostics
            .iter()
            .filter(|d| d.severity == diagnostics::Severity::Error)
            .count();
        if remaining_errors > 0 {
            ok = false;
            if let Some(next) = final_report
                .diagnostics
                .iter()
                .find(|d| d.severity == diagnostics::Severity::Error)
            {
                let loc = match &next.loc {
                    Some(diagnostics::Location::X07Ast { ptr }) => format!(" ({ptr})"),
                    Some(diagnostics::Location::Text { span, .. }) => match &span.file {
                        Some(file) => format!(" ({}:{}:{})", file, span.start.line, span.start.col),
                        None => format!(" ({}:{})", span.start.line, span.start.col),
                    },
                    None => String::new(),
                };
                eprintln!("x07 fix: next error: {}: {}{loc}", next.code, next.message);
            }
            eprintln!(
                "x07 fix: {remaining_errors} error(s) remain after auto-fix for {}. \
                 Run `x07 lint --input {}` to see the remaining diagnostics.",
                input.display(),
                input.display()
            );
        }

        if args.write {
            std::fs::write(input, formatted.as_bytes())
                .with_context(|| format!("write: {}", input.display()))?;
        } else {
            match machine.out.as_deref() {
                Some(path) => crate::reporting::write_bytes(path, formatted.as_bytes())?,
                None => print!("{formatted}"),
            }
        }
    }

    Ok(if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

#[derive(Debug, Serialize)]
struct MigrateFileReport {
    path: String,
    changed: bool,
    applied_ops_count: usize,
    iterations: u32,
    report: diagnostics::Report,
}

#[derive(Debug, Serialize)]
struct MigrateReport {
    ok: bool,
    command: &'static str,
    to: String,
    write: bool,
    files: Vec<MigrateFileReport>,
}

pub fn cmd_migrate(
    machine: &crate::reporting::MachineArgs,
    mut args: MigrateArgs,
) -> Result<std::process::ExitCode> {
    if args.check && args.write {
        anyhow::bail!("--check cannot be combined with --write");
    }
    if !args.check && !args.write {
        args.check = true;
    }

    let to = x07c::compat::CompatVersion::parse(&args.to).context("parse --to")?;
    if to != x07c::compat::Compat::CURRENT_VERSION {
        anyhow::bail!(
            "unsupported migration target {:?} (supported: {:?})",
            to.as_str(),
            x07c::compat::Compat::CURRENT_VERSION.as_str()
        );
    }
    let compat = x07c::compat::Compat {
        version: to,
        strict: true,
    };

    let mut raw_inputs = args.input;
    if raw_inputs.is_empty() {
        raw_inputs.push(PathBuf::from("."));
    }
    let inputs = collect_x07ast_inputs(&raw_inputs).context("collect inputs")?;

    let mut files: Vec<MigrateFileReport> = Vec::with_capacity(inputs.len());
    let mut ok = true;

    for input in &inputs {
        let bytes =
            std::fs::read(input).with_context(|| format!("read input: {}", input.display()))?;
        let mut doc: serde_json::Value = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse JSON: {}", input.display()))?;

        let report = if args.write {
            let r = crate::repair::migrate_x07ast_file_doc(
                &mut doc,
                WorldId::RunOs,
                args.max_iters,
                RepairMode::Write,
                compat,
            )
            .with_context(|| format!("migrate: {}", input.display()))?;
            let changed = bytes != r.formatted.as_bytes();
            if changed {
                std::fs::write(input, r.formatted.as_bytes())
                    .with_context(|| format!("write: {}", input.display()))?;
            }
            if !r.final_report.ok {
                ok = false;
            }
            files.push(MigrateFileReport {
                path: input.display().to_string(),
                changed,
                applied_ops_count: r.summary.applied_ops_count,
                iterations: r.summary.iterations,
                report: r.final_report,
            });
            continue;
        } else {
            let doc_bytes = serde_json::to_vec(&doc)?;
            let mut file = x07ast::parse_x07ast_json(&doc_bytes)
                .map_err(|e| anyhow::anyhow!("{e}"))
                .with_context(|| format!("parse x07AST: {}", input.display()))?;
            x07ast::canonicalize_x07ast_file(&mut file);
            let mut lint_options = x07c::world_config::lint_options_for_world(WorldId::RunOs);
            lint_options.compat = compat;
            let report = lint::lint_file_migration(&file, lint_options);
            if !report.ok {
                ok = false;
            }
            report
        };

        files.push(MigrateFileReport {
            path: input.display().to_string(),
            changed: false,
            applied_ops_count: 0,
            iterations: 1,
            report,
        });
    }

    let report = MigrateReport {
        ok,
        command: "migrate",
        to: to.as_str(),
        write: args.write,
        files,
    };

    let out = serde_json::to_string(&report)? + "\n";
    if let Some(path) = machine.out.as_deref() {
        crate::reporting::write_bytes(path, out.as_bytes())?;
    } else {
        print!("{out}");
    }

    Ok(if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

pub fn cmd_build(
    machine: &crate::reporting::MachineArgs,
    args: BuildArgs,
) -> Result<std::process::ExitCode> {
    if let Some(max_c_bytes) = args.max_c_bytes {
        std::env::set_var("X07_MAX_C_BYTES", max_c_bytes.to_string());
    }

    let ctx = crate::project_ctx::load_project_ctx(&args.project, true).context("load project")?;
    let crate::project_ctx::ProjectCtx {
        base,
        manifest,
        program_path,
        module_roots,
        world,
        ..
    } = ctx;
    let compat = crate::util::resolve_compat(args.compat.as_deref(), manifest.compat.as_deref())
        .context("resolve compat")?;

    let repair_result = crate::repair::maybe_repair_x07ast_file(&program_path, world, &args.repair)
        .with_context(|| format!("repair entry: {}", program_path.display()))?;
    let program_bytes = if let Some(r) = repair_result {
        r.formatted.into_bytes()
    } else {
        std::fs::read(&program_path)
            .with_context(|| format!("read entry: {}", program_path.display()))?
    };

    let mut options = x07c::world_config::compile_options_for_world(world, module_roots);
    options.compat = compat;
    options.arch_root = Some(base);
    if args.freestanding {
        options.emit_main = false;
        options.freestanding = true;
    } else if args.emit_c_header.is_some() {
        options.emit_main = false;
    }

    let c = x07c::compile::compile_program_to_c(&program_bytes, &options)
        .map_err(|e| anyhow::anyhow!("compile failed: {:?}: {}", e.kind, e.message))?;
    match machine.out.as_ref() {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create output dir: {}", parent.display()))?;
            }
            std::fs::write(path, c.as_bytes())
                .with_context(|| format!("write: {}", path.display()))?;
        }
        None => {
            print!("{c}");
        }
    }

    if let Some(path) = args.emit_c_header {
        let h = x07c::c_emit::emit_c_header(&options)
            .map_err(|e| anyhow::anyhow!("emit header failed: {:?}: {}", e.kind, e.message))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create output dir: {}", parent.display()))?;
        }
        std::fs::write(&path, h.as_bytes())
            .with_context(|| format!("write: {}", path.display()))?;
    }

    if let Some(path) = args.emit_wasm {
        let default_initial = 16 * 1024 * 1024u64;
        let initial = args.wasm_initial_memory_bytes.unwrap_or(default_initial);
        let mut max = args.wasm_max_memory_bytes.unwrap_or(initial);
        let no_growable = if args.wasm_no_growable_memory {
            true
        } else {
            // Default for Phase 7 is "no growable memory" unless max>initial is explicitly set.
            max == initial
        };
        if no_growable {
            max = initial;
        }

        let wasm_opts = x07c::wasm_emit::WasmEmitOptions {
            mem: x07c::wasm_emit::WasmMemLimits {
                initial_memory_bytes: initial,
                max_memory_bytes: max,
                no_growable_memory: no_growable,
            },
            features: x07c::wasm_emit::features::supported_features_v1(),
        };

        let prepared =
            x07c::compile::compile_program_to_program_with_meta(&program_bytes, &options)
                .map_err(|e| anyhow::anyhow!("compile failed: {:?}: {}", e.kind, e.message))?;
        let wasm =
            x07c::compile::compile_program_to_wasm_v1(&prepared.program, &options, &wasm_opts)
                .map_err(|e| anyhow::anyhow!("wasm emit failed: {:?}: {}", e.kind, e.message))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create output dir: {}", parent.display()))?;
        }
        std::fs::write(&path, wasm).with_context(|| format!("write: {}", path.display()))?;
    }

    Ok(std::process::ExitCode::SUCCESS)
}

#[derive(Debug, Clone)]
struct LoadedModuleFile {
    file: x07ast::X07AstFile,
    path: Option<PathBuf>,
    is_builtin: bool,
}

fn load_module_recursive(
    module_id: &str,
    world: WorldId,
    module_roots: &[PathBuf],
    out: &mut BTreeMap<String, LoadedModuleFile>,
    visiting: &mut BTreeSet<String>,
) -> Result<(), compile::CompilerError> {
    if out.contains_key(module_id) {
        return Ok(());
    }
    if !visiting.insert(module_id.to_string()) {
        return Err(compile::CompilerError::new(
            compile::CompileErrorKind::Parse,
            format!("cyclic import detected at module {module_id:?}"),
        ));
    }

    let source = module_source::load_module_source(module_id, world, module_roots)?;
    if !source.src.trim_start().starts_with('{') {
        return Err(compile::CompilerError::new(
            compile::CompileErrorKind::Parse,
            format!(
                "{module_id:?}: module source must be x07AST JSON (*.x07.json); legacy S-expr is not supported"
            ),
        ));
    }

    let mut file = x07ast::parse_x07ast_json(source.src.as_bytes()).map_err(|e| {
        compile::CompilerError::new(
            compile::CompileErrorKind::Parse,
            format!("{module_id:?}: {e}"),
        )
    })?;
    x07ast::canonicalize_x07ast_file(&mut file);

    for dep in file.imports.clone() {
        load_module_recursive(&dep, world, module_roots, out, visiting)?;
    }

    out.insert(
        module_id.to_string(),
        LoadedModuleFile {
            file,
            path: source.path,
            is_builtin: source.is_builtin,
        },
    );
    let _ = visiting.remove(module_id);
    Ok(())
}

fn parse_fn_and_ptr_suffix(message: &str) -> (Option<String>, Option<String>) {
    let Some(start) = message.rfind("(fn=") else {
        return (None, None);
    };
    let suffix = &message[start + 1..];
    let Some(end) = suffix.find(')') else {
        return (None, None);
    };
    let inner = &suffix[..end];
    let mut fn_name = None;
    let mut ptr = None;
    for part in inner.split_whitespace() {
        if let Some(v) = part.strip_prefix("fn=") {
            fn_name = Some(v.to_string());
        } else if let Some(v) = part.strip_prefix("ptr=") {
            ptr = Some(v.to_string());
        }
    }
    (fn_name, ptr)
}

pub fn cmd_check(
    machine: &crate::reporting::MachineArgs,
    args: CheckArgs,
) -> Result<std::process::ExitCode> {
    let mut diags: Vec<diagnostics::Diagnostic> = Vec::new();

    let ctx = match crate::project_ctx::load_project_ctx(&args.project, false) {
        Ok(ctx) => ctx,
        Err(err) => {
            diags.push(crate::reporting::diag_error(
                "X07-IO-READ-0001",
                diagnostics::Stage::Parse,
                &format!("{err:#}"),
            ));
            let mut report = diagnostics::Report::ok();
            report.ok = false;
            report.diagnostics = diags;
            let out = serde_json::to_string(&report)? + "\n";
            if let Some(path) = machine.out.as_deref() {
                crate::reporting::write_bytes(path, out.as_bytes())?;
            } else {
                print!("{out}");
            }
            return Ok(std::process::ExitCode::from(1));
        }
    };

    let crate::project_ctx::ProjectCtx {
        base,
        manifest,
        lock,
        lock_path,
        program_path,
        module_roots,
        world,
    } = ctx;
    let compat = crate::util::resolve_compat(args.compat.as_deref(), manifest.compat.as_deref())
        .context("resolve compat")?;

    let program_bytes = match std::fs::read(&program_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            diags.push(crate::reporting::diag_error(
                "X07-IO-READ-0001",
                diagnostics::Stage::Parse,
                &format!("read entry {}: {err}", program_path.display()),
            ));
            let mut report = diagnostics::Report::ok();
            report.ok = false;
            report.diagnostics = diags;
            report.meta.insert(
                "inputs".to_string(),
                serde_json::Value::Array(vec![
                    serde_json::Value::String(args.project.display().to_string()),
                    serde_json::Value::String(lock_path.display().to_string()),
                    serde_json::Value::String(program_path.display().to_string()),
                ]),
            );
            let out = serde_json::to_string(&report)? + "\n";
            if let Some(path) = machine.out.as_deref() {
                crate::reporting::write_bytes(path, out.as_bytes())?;
            } else {
                print!("{out}");
            }
            return Ok(std::process::ExitCode::from(1));
        }
    };

    let mut entry_file = match x07ast::parse_x07ast_json(&program_bytes) {
        Ok(file) => file,
        Err(err) => {
            let mut d = diagnostics::Diagnostic {
                code: "X07-X07AST-PARSE-0001".to_string(),
                severity: diagnostics::Severity::Error,
                stage: diagnostics::Stage::Parse,
                message: err.message,
                loc: Some(diagnostics::Location::X07Ast { ptr: err.ptr }),
                notes: Vec::new(),
                related: Vec::new(),
                data: Default::default(),
                quickfix: None,
            };
            d.data.insert(
                "file".to_string(),
                serde_json::Value::String(program_path.display().to_string()),
            );
            diags.push(d);
            let mut report = diagnostics::Report::ok();
            report.ok = false;
            report.diagnostics = diags;
            report.meta.insert(
                "inputs".to_string(),
                serde_json::Value::Array(vec![
                    serde_json::Value::String(args.project.display().to_string()),
                    serde_json::Value::String(lock_path.display().to_string()),
                    serde_json::Value::String(program_path.display().to_string()),
                ]),
            );
            let out = serde_json::to_string(&report)? + "\n";
            if let Some(path) = machine.out.as_deref() {
                crate::reporting::write_bytes(path, out.as_bytes())?;
            } else {
                print!("{out}");
            }
            return Ok(std::process::ExitCode::from(1));
        }
    };
    x07ast::canonicalize_x07ast_file(&mut entry_file);

    let mut modules: BTreeMap<String, LoadedModuleFile> = BTreeMap::new();
    let mut visiting: BTreeSet<String> = BTreeSet::new();
    for module_id in entry_file.imports.clone() {
        if let Err(err) = load_module_recursive(
            &module_id,
            world,
            &module_roots,
            &mut modules,
            &mut visiting,
        ) {
            let mut d = crate::reporting::diag_error(
                "X07-X07AST-PARSE-0001",
                diagnostics::Stage::Parse,
                &format!("{:?}: {}", err.kind, err.message),
            );
            d.data.insert(
                "module_id".to_string(),
                serde_json::Value::String(module_id.clone()),
            );
            diags.push(d);
            let mut inputs: BTreeSet<String> = BTreeSet::new();
            inputs.insert(args.project.display().to_string());
            inputs.insert(lock_path.display().to_string());
            inputs.insert(program_path.display().to_string());
            for m in modules.values() {
                if m.is_builtin {
                    continue;
                }
                if let Some(p) = m.path.as_ref() {
                    inputs.insert(p.display().to_string());
                }
            }

            let mut report = diagnostics::Report::ok();
            report.ok = false;
            report.diagnostics = diags;
            report.meta.insert(
                "inputs".to_string(),
                serde_json::Value::Array(
                    inputs.into_iter().map(serde_json::Value::String).collect(),
                ),
            );

            let out = serde_json::to_string(&report)? + "\n";
            if let Some(path) = machine.out.as_deref() {
                crate::reporting::write_bytes(path, out.as_bytes())?;
            } else {
                print!("{out}");
            }

            return Ok(std::process::ExitCode::from(1));
        }
    }

    let mut lint_opts = x07c::world_config::lint_options_for_world(world);
    lint_opts.compat = compat;

    let mut all_diags: Vec<diagnostics::Diagnostic> = Vec::new();
    let mut has_error = false;

    #[derive(Debug, Clone)]
    struct CheckedFile {
        module_id: String,
        path: PathBuf,
        file: x07ast::X07AstFile,
    }

    fn canonicalize_best_effort(p: &Path) -> PathBuf {
        p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
    }

    #[derive(Debug, Clone)]
    struct DepRootInfo {
        name: String,
        version: String,
        root_canon: PathBuf,
    }

    let mut dep_roots: Vec<DepRootInfo> = Vec::new();
    for dep in &lock.dependencies {
        let dep_dir = x07c::project::resolve_rel_path_with_workspace(&base, &dep.path)
            .with_context(|| format!("resolve dep path: {:?}", dep.path))?;
        let root = dep_dir.join(&dep.module_root);
        dep_roots.push(DepRootInfo {
            name: dep.name.clone(),
            version: dep.version.clone(),
            root_canon: canonicalize_best_effort(&root),
        });
    }
    dep_roots.sort_by(|a, b| {
        b.root_canon
            .to_string_lossy()
            .len()
            .cmp(&a.root_canon.to_string_lossy().len())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.version.cmp(&b.version))
    });

    let mut imports_by_module: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    imports_by_module.insert(
        "main".to_string(),
        entry_file.imports.iter().cloned().collect(),
    );
    for (module_id, m) in &modules {
        imports_by_module.insert(module_id.clone(), m.file.imports.iter().cloned().collect());
    }

    use std::collections::VecDeque;
    let mut prev: BTreeMap<String, String> = BTreeMap::new();
    let mut q: VecDeque<String> = VecDeque::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert("main".to_string());
    q.push_back("main".to_string());
    while let Some(cur) = q.pop_front() {
        let Some(imports) = imports_by_module.get(&cur) else {
            continue;
        };
        for next in imports {
            if seen.insert(next.clone()) {
                prev.insert(next.clone(), cur.clone());
                q.push_back(next.clone());
            }
        }
    }

    let mut chain_by_module: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for module_id in imports_by_module.keys() {
        if module_id == "main" {
            chain_by_module.insert(module_id.clone(), vec!["main".to_string()]);
            continue;
        }
        let mut chain_rev: Vec<String> = vec![module_id.clone()];
        let mut cur = module_id.as_str();
        while let Some(p) = prev.get(cur) {
            chain_rev.push(p.clone());
            cur = p.as_str();
            if cur == "main" {
                break;
            }
        }
        chain_rev.reverse();
        if chain_rev.first().map(|s| s.as_str()) == Some("main") {
            chain_by_module.insert(module_id.clone(), chain_rev);
        }
    }

    let mut package_by_module: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (module_id, m) in &modules {
        let Some(path) = m.path.as_ref() else {
            continue;
        };
        let path_canon = canonicalize_best_effort(path);
        for dep in &dep_roots {
            if path_canon.starts_with(&dep.root_canon) {
                package_by_module
                    .insert(module_id.clone(), (dep.name.clone(), dep.version.clone()));
                break;
            }
        }
    }

    let enrich_diag = |d: &mut diagnostics::Diagnostic, module_id: &str| {
        d.data.insert(
            "module_id".to_string(),
            serde_json::Value::String(module_id.to_string()),
        );

        if let Some((name, version)) = package_by_module.get(module_id) {
            d.data.insert(
                "package".to_string(),
                serde_json::json!({ "name": name, "version": version }),
            );
            d.notes.push(format!("from dependency: {name}@{version}"));
        }

        if let Some(chain) = chain_by_module.get(module_id) {
            d.data.insert(
                "dependency_chain".to_string(),
                serde_json::Value::Array(
                    chain
                        .iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
            if package_by_module.contains_key(module_id) {
                d.notes
                    .push(format!("dependency chain: {}", chain.join(" -> ")));
            }
        }
    };

    let mut file_set: Vec<CheckedFile> = Vec::new();
    file_set.push(CheckedFile {
        module_id: "main".to_string(),
        path: program_path.clone(),
        file: entry_file.clone(),
    });
    for (module_id, m) in &modules {
        if m.is_builtin {
            continue;
        }
        let Some(path) = m.path.clone() else {
            continue;
        };
        file_set.push(CheckedFile {
            module_id: module_id.clone(),
            path,
            file: m.file.clone(),
        });
    }
    file_set.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.module_id.cmp(&b.module_id))
    });

    for item in &file_set {
        let report = lint::lint_file_no_typecheck(&item.file, lint_opts);
        if !report.ok {
            has_error = true;
        }
        for mut d in report.diagnostics {
            d.data.insert(
                "file".to_string(),
                serde_json::Value::String(item.path.display().to_string()),
            );
            enrich_diag(&mut d, &item.module_id);
            all_diags.push(d);
        }
    }

    let mut sigs = typecheck::TypecheckSigs::new();
    for item in &file_set {
        sigs.add_file(&item.file);
    }
    sigs.add_builtins();

    for item in &file_set {
        let report = typecheck::typecheck_file_with_sigs(
            &item.file,
            &sigs,
            &typecheck::TypecheckOptions {
                compat,
                ..Default::default()
            },
        );
        for mut d in report.diagnostics {
            if d.severity == diagnostics::Severity::Error {
                has_error = true;
            }
            d.data.insert(
                "file".to_string(),
                serde_json::Value::String(item.path.display().to_string()),
            );
            enrich_diag(&mut d, &item.module_id);
            all_diags.push(d);
        }
    }

    if !has_error {
        let mut options = x07c::world_config::compile_options_for_world(world, module_roots);
        options.compat = compat;
        options.arch_root = Some(base);
        if let Err(err) = compile::check_program(&program_bytes, &options) {
            let (fn_name, ptr) = parse_fn_and_ptr_suffix(&err.message);
            let mut code = "X07-INTERNAL-0001";
            if err.message.contains("use after move") {
                code = "X07-MOVE-0901";
            } else if err.message.contains("borrow") {
                code = "X07-MOVE-0902";
            }
            let mut d = diagnostics::Diagnostic {
                code: code.to_string(),
                severity: diagnostics::Severity::Error,
                stage: diagnostics::Stage::Codegen,
                message: err.message.to_string(),
                loc: ptr
                    .as_ref()
                    .map(|p| diagnostics::Location::X07Ast { ptr: p.clone() }),
                notes: Vec::new(),
                related: Vec::new(),
                data: Default::default(),
                quickfix: None,
            };
            d.data.insert(
                "compiler_error_kind".to_string(),
                serde_json::Value::String(format!("{:?}", err.kind)),
            );
            let mut diag_path: Option<PathBuf> = None;
            let mut diag_module_id: Option<String> = None;
            if let Some(fn_name) = fn_name.as_deref() {
                d.data.insert(
                    "fn".to_string(),
                    serde_json::Value::String(fn_name.to_string()),
                );
                if fn_name == "solve" || fn_name.starts_with("main.") {
                    diag_module_id = Some("main".to_string());
                    d.data.insert(
                        "file".to_string(),
                        serde_json::Value::String(program_path.display().to_string()),
                    );
                    diag_path = Some(program_path.clone());
                } else if let Some((mod_id, _)) = fn_name.rsplit_once('.') {
                    diag_module_id = Some(mod_id.to_string());
                    if let Some(m) = modules.get(mod_id) {
                        if let Some(p) = m.path.as_ref() {
                            d.data.insert(
                                "file".to_string(),
                                serde_json::Value::String(p.display().to_string()),
                            );
                            diag_path = Some(p.clone());
                        }
                    }
                }
            }
            if !d.data.contains_key("file") {
                d.data.insert(
                    "file".to_string(),
                    serde_json::Value::String(program_path.display().to_string()),
                );
                diag_path = Some(program_path.clone());
            }

            enrich_diag(&mut d, diag_module_id.as_deref().unwrap_or("main"));

            if d.code == "X07-MOVE-0901" {
                if let (Some(path), Some(moved_ptr)) = (
                    diag_path.as_ref(),
                    crate::run::first_pointer_for_compile_error(&d.message, "moved_ptr="),
                ) {
                    if let Ok(bytes) = std::fs::read(path) {
                        if let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            if let Some(serde_json::Value::String(name)) = doc.pointer(&moved_ptr) {
                                d.quickfix = Some(diagnostics::Quickfix {
                                    kind: diagnostics::QuickfixKind::JsonPatch,
                                    patch: vec![diagnostics::PatchOp::Replace {
                                        path: moved_ptr,
                                        value: serde_json::json!([
                                            "view.to_bytes",
                                            ["bytes.view", name]
                                        ]),
                                    }],
                                    note: Some("Copy before move".to_string()),
                                });
                            }
                        }
                    }
                }
            }
            all_diags.push(d);
        }
    }

    all_diags.sort_by(|a, b| {
        let ap = a.data.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let bp = b.data.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let a_ptr = a
            .loc
            .as_ref()
            .and_then(|l| match l {
                diagnostics::Location::X07Ast { ptr } => Some(ptr.as_str()),
                diagnostics::Location::Text { .. } => None,
            })
            .unwrap_or("");
        let b_ptr = b
            .loc
            .as_ref()
            .and_then(|l| match l {
                diagnostics::Location::X07Ast { ptr } => Some(ptr.as_str()),
                diagnostics::Location::Text { .. } => None,
            })
            .unwrap_or("");
        ap.cmp(bp)
            .then_with(|| a_ptr.cmp(b_ptr))
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.message.cmp(&b.message))
    });

    let ok = all_diags
        .iter()
        .all(|d| d.severity != diagnostics::Severity::Error);

    let mut inputs: BTreeSet<String> = BTreeSet::new();
    inputs.insert(args.project.display().to_string());
    inputs.insert(lock_path.display().to_string());
    inputs.insert(program_path.display().to_string());
    for m in modules.values() {
        if m.is_builtin {
            continue;
        }
        if let Some(p) = m.path.as_ref() {
            inputs.insert(p.display().to_string());
        }
    }

    let mut report = diagnostics::Report::ok();
    report.ok = ok;
    report.diagnostics = all_diags;
    report.meta.insert(
        "inputs".to_string(),
        serde_json::Value::Array(inputs.into_iter().map(serde_json::Value::String).collect()),
    );

    let out = serde_json::to_string(&report)? + "\n";
    if let Some(path) = machine.out.as_deref() {
        crate::reporting::write_bytes(path, out.as_bytes())?;
    } else {
        print!("{out}");
    }

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::collect_x07ast_inputs;

    static TMP_N: AtomicUsize = AtomicUsize::new(0);

    fn tmp_root(prefix: &str) -> PathBuf {
        let pid = std::process::id();
        let n = TMP_N.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("x07_{prefix}_{pid}_{n}"))
    }

    fn write_text(path: &std::path::Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents.as_bytes()).unwrap();
    }

    #[test]
    fn collect_x07ast_inputs_skips_common_dirs_and_sorts() {
        let root = tmp_root("collect_inputs");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let a = root.join("src/a.x07.json");
        let b = root.join("src/b.x07.json");
        let skipped_target = root.join("target/c.x07.json");
        let skipped_dot_x07 = root.join(".x07/d.x07.json");

        let minimal = "{\"schema_version\":\"x07.x07ast@0.5.0\",\"kind\":\"module\",\"module_id\":\"m\",\"imports\":[],\"decls\":[]}\n";
        write_text(&a, minimal);
        write_text(&b, minimal);
        write_text(&skipped_target, minimal);
        write_text(&skipped_dot_x07, minimal);

        let got = collect_x07ast_inputs(std::slice::from_ref(&root)).unwrap();
        assert_eq!(got, vec![a.clone(), b.clone()]);

        let got2 = collect_x07ast_inputs(&[root.clone(), a.clone()]).unwrap();
        assert_eq!(got2, vec![a, b]);

        let _ = std::fs::remove_dir_all(&root);
    }
}
