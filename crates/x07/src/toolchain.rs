use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use walkdir::WalkDir;
use x07_worlds::WorldId;
use x07c::diagnostics;
use x07c::lint;
use x07c::project;
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
    #[arg(long, value_name = "PATH", required = true)]
    pub input: Vec<PathBuf>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub write: bool,
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
pub struct BuildArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: PathBuf,

    /// Emit the runtime C header (requires `emit_main=false`; use `--freestanding` for embedding).
    #[arg(long, value_name = "PATH")]
    pub emit_c_header: Option<PathBuf>,

    /// Build in freestanding mode for library embedding (exports `x07_solve_v2`; no `main()`).
    #[arg(long)]
    pub freestanding: bool,

    /// Override the generated C source size budget (in bytes).
    #[arg(long, value_name = "BYTES")]
    pub max_c_bytes: Option<usize>,

    #[command(flatten)]
    pub repair: RepairArgs,
}

pub fn cmd_fmt(
    _machine: &crate::reporting::MachineArgs,
    args: FmtArgs,
) -> Result<std::process::ExitCode> {
    if args.check == args.write {
        anyhow::bail!("set exactly one of --check or --write");
    }

    let inputs = collect_x07ast_inputs(&args.input).context("collect inputs")?;
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
        let formatted = serde_json::to_string(&v)? + "\n";

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
            eprintln!(
                "x07 fix: {remaining_errors} error(s) remain after auto-fix for {}. \
                 Run `x07 build` to see codegen-stage errors.",
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

pub fn cmd_build(
    machine: &crate::reporting::MachineArgs,
    args: BuildArgs,
) -> Result<std::process::ExitCode> {
    if let Some(max_c_bytes) = args.max_c_bytes {
        std::env::set_var("X07_MAX_C_BYTES", max_c_bytes.to_string());
    }

    let manifest = project::load_project_manifest(&args.project)?;
    let lock_path = project::default_lockfile_path(&args.project, &manifest);
    let lock_bytes = std::fs::read(&lock_path)
        .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
    let lock: project::Lockfile = serde_json::from_slice(&lock_bytes)
        .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;

    project::verify_lockfile(&args.project, &manifest, &lock)?;

    let base = args
        .project
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let program_path = base.join(&manifest.entry);

    let module_roots = project::collect_module_roots(&args.project, &manifest, &lock)?;
    let world = x07c::world_config::parse_world_id(&manifest.world)
        .with_context(|| format!("invalid project world {:?}", manifest.world))?;

    let repair_result = crate::repair::maybe_repair_x07ast_file(&program_path, world, &args.repair)
        .with_context(|| format!("repair entry: {}", program_path.display()))?;
    let program_bytes = if let Some(r) = repair_result {
        r.formatted.into_bytes()
    } else {
        std::fs::read(&program_path)
            .with_context(|| format!("read entry: {}", program_path.display()))?
    };

    let mut options = x07c::world_config::compile_options_for_world(world, module_roots);
    options.arch_root = Some(base.to_path_buf());
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

    Ok(std::process::ExitCode::SUCCESS)
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
