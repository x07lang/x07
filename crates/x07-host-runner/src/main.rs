use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::Engine;
use clap::Parser;
use x07_contracts::X07_HOST_RUNNER_REPORT_SCHEMA_VERSION;
use x07_host_runner::{
    apply_cc_profile, compile_program_with_options, run_artifact_file, CcProfile, RunnerConfig,
};
use x07_worlds::WorldId;
use x07c::project;

#[derive(Parser)]
#[command(name = "x07-host-runner")]
#[command(about = "Deterministic native runner (C backend).", long_about = None)]
struct Cli {
    #[arg(long)]
    cli_specrows: bool,

    #[arg(long, value_enum, default_value_t = CcProfile::Default)]
    cc_profile: CcProfile,

    #[arg(long)]
    artifact: Option<PathBuf>,

    #[arg(long)]
    program: Option<PathBuf>,

    #[arg(long)]
    project: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = WorldId::SolvePure)]
    world: WorldId,

    #[arg(long, value_name = "BYTES")]
    max_c_bytes: Option<usize>,

    #[arg(long)]
    module_root: Vec<PathBuf>,

    #[arg(long)]
    fixture_fs_dir: Option<PathBuf>,

    #[arg(long)]
    fixture_fs_root: Option<PathBuf>,

    #[arg(long)]
    fixture_fs_latency_index: Option<PathBuf>,

    #[arg(long)]
    fixture_rr_dir: Option<PathBuf>,

    #[arg(long)]
    fixture_rr_index: Option<PathBuf>,

    #[arg(long)]
    fixture_kv_dir: Option<PathBuf>,

    #[arg(long)]
    fixture_kv_seed: Option<PathBuf>,

    #[arg(long)]
    input: Option<PathBuf>,

    #[arg(long, default_value_t = 50_000_000)]
    solve_fuel: u64,

    #[arg(long, default_value_t = 64 * 1024 * 1024)]
    max_memory_bytes: usize,

    #[arg(long)]
    max_output_bytes: Option<usize>,

    #[arg(long, default_value_t = 5)]
    cpu_time_limit_seconds: u64,

    #[arg(long)]
    debug_borrow_checks: bool,

    #[arg(long)]
    compiled_out: Option<PathBuf>,

    #[arg(long)]
    compile_only: bool,
}

fn main() -> std::process::ExitCode {
    // Windows defaults to a 1MiB stack, which is not enough for our current compiler recursion depth
    // (for example, some solve-full benchmark programs). Run the real entrypoint on a larger-stack
    // thread to keep behavior consistent across platforms.
    let handle = std::thread::Builder::new()
        .name("x07-host-runner".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(run);

    match handle {
        Ok(handle) => match handle.join() {
            Ok(code) => code,
            Err(panic) => {
                if let Some(message) = panic.downcast_ref::<&str>() {
                    eprintln!("x07-host-runner panicked: {message}");
                } else if let Some(message) = panic.downcast_ref::<String>() {
                    eprintln!("x07-host-runner panicked: {message}");
                } else {
                    eprintln!("x07-host-runner panicked");
                }
                std::process::ExitCode::from(2)
            }
        },
        Err(err) => {
            eprintln!("failed to spawn x07-host-runner thread: {err}");
            run()
        }
    }
}

fn run() -> std::process::ExitCode {
    match try_main() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err:#}");
            std::process::ExitCode::from(2)
        }
    }
}

fn try_main() -> Result<std::process::ExitCode> {
    let cli = Cli::parse();

    if cli.cli_specrows {
        use clap::CommandFactory as _;
        let cmd = Cli::command();
        let doc = x07c::cli_specrows::command_to_specrows(&cmd);
        println!("{}", serde_json::to_string(&doc)?);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    apply_cc_profile(cli.cc_profile);

    if let Some(max_c_bytes) = cli.max_c_bytes {
        std::env::set_var("X07_MAX_C_BYTES", max_c_bytes.to_string());
    }

    let input = match &cli.input {
        Some(path) => {
            std::fs::read(path).with_context(|| format!("read input: {}", path.display()))?
        }
        None => Vec::new(),
    };

    let b64 = base64::engine::general_purpose::STANDARD;

    match (&cli.artifact, &cli.program, &cli.project) {
        (Some(_), Some(_), _)
        | (Some(_), _, Some(_))
        | (_, Some(_), Some(_))
        | (None, None, None) => {
            anyhow::bail!("set exactly one of --artifact, --program, or --project")
        }

        (Some(artifact), None, None) => {
            if !cli.module_root.is_empty() {
                anyhow::bail!("--module-root is only valid with --program");
            }
            if cli.compile_only {
                anyhow::bail!("--compile-only is only valid with --program or --project");
            }
            let world = cli.world;
            if !world.is_eval_world() {
                anyhow::bail!(
                    "x07-host-runner supports only deterministic solve worlds, got {}",
                    world.as_str()
                );
            }
            match world {
                WorldId::SolvePure => {}
                WorldId::SolveFs => {
                    if cli.fixture_fs_dir.is_none() {
                        anyhow::bail!("set --fixture-fs-dir for --world solve-fs");
                    }
                }
                WorldId::SolveRr => {
                    if cli.fixture_rr_dir.is_none() {
                        anyhow::bail!("set --fixture-rr-dir for --world solve-rr");
                    }
                }
                WorldId::SolveKv => {
                    if cli.fixture_kv_dir.is_none() {
                        anyhow::bail!("set --fixture-kv-dir for --world solve-kv");
                    }
                }
                WorldId::SolveFull => {
                    if cli.fixture_fs_dir.is_none() {
                        anyhow::bail!("set --fixture-fs-dir for --world solve-full");
                    }
                    if cli.fixture_rr_dir.is_none() {
                        anyhow::bail!("set --fixture-rr-dir for --world solve-full");
                    }
                    if cli.fixture_kv_dir.is_none() {
                        anyhow::bail!("set --fixture-kv-dir for --world solve-full");
                    }
                }
                _ => anyhow::bail!(
                    "x07-host-runner supports only deterministic solve worlds, got {}",
                    world.as_str()
                ),
            }
            let config = RunnerConfig {
                world,
                fixture_fs_dir: cli.fixture_fs_dir.clone(),
                fixture_fs_root: cli.fixture_fs_root.clone(),
                fixture_fs_latency_index: cli.fixture_fs_latency_index.clone(),
                fixture_rr_dir: cli.fixture_rr_dir.clone(),
                fixture_rr_index: cli.fixture_rr_index.clone(),
                fixture_kv_dir: cli.fixture_kv_dir.clone(),
                fixture_kv_seed: cli.fixture_kv_seed.clone(),
                solve_fuel: cli.solve_fuel,
                max_memory_bytes: cli.max_memory_bytes,
                max_output_bytes: cli.max_output_bytes.unwrap_or(1024 * 1024),
                cpu_time_limit_seconds: cli.cpu_time_limit_seconds,
                debug_borrow_checks: cli.debug_borrow_checks,
            };

            let result = x07_host_runner::run_artifact_file(&config, artifact, &input)?;
            let exit_code: u8 = if result.ok && result.exit_status == 0 {
                0
            } else {
                1
            };
            let json = serde_json::json!({
                "schema_version": X07_HOST_RUNNER_REPORT_SCHEMA_VERSION,
                "mode": "solve",
                "ok": result.ok,
                "exit_code": exit_code,
                "exit_status": result.exit_status,
                "solve_output_b64": b64.encode(&result.solve_output),
                "stdout_b64": b64.encode(&result.stdout),
                "stderr_b64": b64.encode(&result.stderr),
                "fuel_used": result.fuel_used,
                "heap_used": result.heap_used,
                "fs_read_file_calls": result.fs_read_file_calls,
                "fs_list_dir_calls": result.fs_list_dir_calls,
                "rr_send_calls": result.rr_send_calls,
                "rr_request_calls": result.rr_request_calls,
                "rr_last_request_sha256": result.rr_last_request_sha256,
                "kv_get_calls": result.kv_get_calls,
                "kv_set_calls": result.kv_set_calls,
                "sched_stats": result.sched_stats,
                "mem_stats": result.mem_stats,
                "debug_stats": result.debug_stats,
                "trap": result.trap,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);

            Ok(std::process::ExitCode::from(exit_code))
        }

        (None, Some(program_path), None) => {
            let world = cli.world;
            if !world.is_eval_world() {
                anyhow::bail!(
                    "x07-host-runner supports only deterministic solve worlds, got {}",
                    world.as_str()
                );
            }
            match world {
                WorldId::SolvePure => {}
                WorldId::SolveFs => {
                    if cli.fixture_fs_dir.is_none() {
                        anyhow::bail!("set --fixture-fs-dir for --world solve-fs");
                    }
                }
                WorldId::SolveRr => {
                    if cli.fixture_rr_dir.is_none() {
                        anyhow::bail!("set --fixture-rr-dir for --world solve-rr");
                    }
                }
                WorldId::SolveKv => {
                    if cli.fixture_kv_dir.is_none() {
                        anyhow::bail!("set --fixture-kv-dir for --world solve-kv");
                    }
                }
                WorldId::SolveFull => {
                    if cli.fixture_fs_dir.is_none() {
                        anyhow::bail!("set --fixture-fs-dir for --world solve-full");
                    }
                    if cli.fixture_rr_dir.is_none() {
                        anyhow::bail!("set --fixture-rr-dir for --world solve-full");
                    }
                    if cli.fixture_kv_dir.is_none() {
                        anyhow::bail!("set --fixture-kv-dir for --world solve-full");
                    }
                }
                _ => anyhow::bail!(
                    "x07-host-runner supports only deterministic solve worlds, got {}",
                    world.as_str()
                ),
            }
            let config = RunnerConfig {
                world,
                fixture_fs_dir: cli.fixture_fs_dir.clone(),
                fixture_fs_root: cli.fixture_fs_root.clone(),
                fixture_fs_latency_index: cli.fixture_fs_latency_index.clone(),
                fixture_rr_dir: cli.fixture_rr_dir.clone(),
                fixture_rr_index: cli.fixture_rr_index.clone(),
                fixture_kv_dir: cli.fixture_kv_dir.clone(),
                fixture_kv_seed: cli.fixture_kv_seed.clone(),
                solve_fuel: cli.solve_fuel,
                max_memory_bytes: cli.max_memory_bytes,
                max_output_bytes: cli.max_output_bytes.unwrap_or(1024 * 1024),
                cpu_time_limit_seconds: cli.cpu_time_limit_seconds,
                debug_borrow_checks: cli.debug_borrow_checks,
            };

            if !program_path
                .as_os_str()
                .to_string_lossy()
                .ends_with(".x07.json")
            {
                anyhow::bail!(
                    "--program must be an x07AST JSON file (*.x07.json), got {}",
                    program_path.display()
                );
            }
            let program = std::fs::read(program_path)
                .with_context(|| format!("read program: {}", program_path.display()))?;

            let compile_options =
                x07_host_runner::compile_options_for_world(world, cli.module_root.clone())?;

            if cli.compile_only {
                let compile = compile_program_with_options(
                    &program,
                    &config,
                    cli.compiled_out.as_deref(),
                    &compile_options,
                    &[],
                )?;
                let exit_code: u8 = if compile.ok { 0 } else { 1 };
                let json = serde_json::json!({
                    "schema_version": X07_HOST_RUNNER_REPORT_SCHEMA_VERSION,
                    "mode": "compile",
                    "exit_code": exit_code,
                    "compile": {
                        "ok": compile.ok,
                        "exit_status": compile.exit_status,
                        "lang_id": compile.lang_id,
                        "guide_md": compile.guide_md,
                        "native_requires": compile.native_requires,
                        "c_source_size": compile.c_source_size,
                        "compiled_exe": compile.compiled_exe.as_ref().map(|p| p.display().to_string()),
                        "compiled_exe_size": compile.compiled_exe_size,
                        "compile_error": compile.compile_error,
                        "stdout_b64": b64.encode(&compile.stdout),
                        "stderr_b64": b64.encode(&compile.stderr),
                        "fuel_used": compile.fuel_used,
                        "trap": compile.trap,
                    },
                    "solve": serde_json::Value::Null,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);

                return Ok(std::process::ExitCode::from(exit_code));
            }

            let result = x07_host_runner::compile_and_run_with_options(
                &program,
                &config,
                &input,
                cli.compiled_out.as_deref(),
                &compile_options,
            )?;

            let solve_json = match &result.solve {
                Some(solve) => serde_json::json!({
                    "ok": solve.ok,
                    "exit_status": solve.exit_status,
                    "solve_output_b64": b64.encode(&solve.solve_output),
                    "stdout_b64": b64.encode(&solve.stdout),
                    "stderr_b64": b64.encode(&solve.stderr),
                    "fuel_used": solve.fuel_used,
                    "heap_used": solve.heap_used,
                    "fs_read_file_calls": solve.fs_read_file_calls,
                    "fs_list_dir_calls": solve.fs_list_dir_calls,
                    "rr_send_calls": solve.rr_send_calls,
                    "rr_request_calls": solve.rr_request_calls,
                    "rr_last_request_sha256": solve.rr_last_request_sha256,
                    "kv_get_calls": solve.kv_get_calls,
                    "kv_set_calls": solve.kv_set_calls,
                    "sched_stats": solve.sched_stats,
                    "mem_stats": solve.mem_stats,
                    "debug_stats": solve.debug_stats,
                    "trap": solve.trap,
                }),
                None => serde_json::Value::Null,
            };

            let ok = result.compile.ok
                && result
                    .solve
                    .as_ref()
                    .map(|s| s.ok && s.exit_status == 0)
                    .unwrap_or(false);
            let exit_code: u8 = if ok { 0 } else { 1 };
            let json = serde_json::json!({
                "schema_version": X07_HOST_RUNNER_REPORT_SCHEMA_VERSION,
                "mode": "compile-run",
                "exit_code": exit_code,
                "compile": {
                    "ok": result.compile.ok,
                    "exit_status": result.compile.exit_status,
                    "lang_id": result.compile.lang_id,
                    "guide_md": result.compile.guide_md,
                    "native_requires": result.compile.native_requires,
                    "c_source_size": result.compile.c_source_size,
                    "compiled_exe": result.compile.compiled_exe.as_ref().map(|p| p.display().to_string()),
                    "compiled_exe_size": result.compile.compiled_exe_size,
                    "compile_error": result.compile.compile_error,
                    "stdout_b64": b64.encode(&result.compile.stdout),
                    "stderr_b64": b64.encode(&result.compile.stderr),
                    "fuel_used": result.compile.fuel_used,
                    "trap": result.compile.trap,
                },
                "solve": solve_json,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);

            Ok(std::process::ExitCode::from(exit_code))
        }

        (None, None, Some(project_path)) => {
            if !cli.module_root.is_empty() {
                anyhow::bail!("--module-root is only valid with --program");
            }
            let manifest = project::load_project_manifest(project_path)?;
            if cli.world.as_str() != manifest.world {
                anyhow::bail!(
                    "--world {} does not match project world {:?}",
                    cli.world.as_str(),
                    manifest.world
                );
            }
            let world = WorldId::parse(&manifest.world)
                .with_context(|| format!("invalid project world {:?}", manifest.world))?;
            if !world.is_eval_world() {
                anyhow::bail!(
                    "x07-host-runner supports only deterministic solve worlds, got {}",
                    world.as_str()
                );
            }
            match world {
                WorldId::SolvePure => {}
                WorldId::SolveFs => {
                    if cli.fixture_fs_dir.is_none() {
                        anyhow::bail!("set --fixture-fs-dir for project world solve-fs");
                    }
                }
                WorldId::SolveRr => {
                    if cli.fixture_rr_dir.is_none() {
                        anyhow::bail!("set --fixture-rr-dir for project world solve-rr");
                    }
                }
                WorldId::SolveKv => {
                    if cli.fixture_kv_dir.is_none() {
                        anyhow::bail!("set --fixture-kv-dir for project world solve-kv");
                    }
                }
                WorldId::SolveFull => {
                    if cli.fixture_fs_dir.is_none() {
                        anyhow::bail!("set --fixture-fs-dir for project world solve-full");
                    }
                    if cli.fixture_rr_dir.is_none() {
                        anyhow::bail!("set --fixture-rr-dir for project world solve-full");
                    }
                    if cli.fixture_kv_dir.is_none() {
                        anyhow::bail!("set --fixture-kv-dir for project world solve-full");
                    }
                }
                _ => anyhow::bail!(
                    "x07-host-runner supports only deterministic solve worlds, got {}",
                    world.as_str()
                ),
            }
            let config = RunnerConfig {
                world,
                fixture_fs_dir: cli.fixture_fs_dir.clone(),
                fixture_fs_root: cli.fixture_fs_root.clone(),
                fixture_fs_latency_index: cli.fixture_fs_latency_index.clone(),
                fixture_rr_dir: cli.fixture_rr_dir.clone(),
                fixture_rr_index: cli.fixture_rr_index.clone(),
                fixture_kv_dir: cli.fixture_kv_dir.clone(),
                fixture_kv_seed: cli.fixture_kv_seed.clone(),
                solve_fuel: cli.solve_fuel,
                max_memory_bytes: cli.max_memory_bytes,
                max_output_bytes: cli.max_output_bytes.unwrap_or(1024 * 1024),
                cpu_time_limit_seconds: cli.cpu_time_limit_seconds,
                debug_borrow_checks: cli.debug_borrow_checks,
            };

            let lock_path = project::default_lockfile_path(project_path, &manifest);
            let lock_bytes = std::fs::read(&lock_path).with_context(|| {
                format!(
                    "[X07LOCK_READ] read lockfile: {} (hint: run `x07 pkg lock`)",
                    lock_path.display()
                )
            })?;
            let lock: project::Lockfile =
                serde_json::from_slice(&lock_bytes).with_context(|| {
                    format!(
                        "[X07LOCK_PARSE] parse lockfile JSON: {} (hint: run `x07 pkg lock`)",
                        lock_path.display()
                    )
                })?;
            project::verify_lockfile(project_path, &manifest, &lock)?;

            let base = project_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            let entry_path = base.join(&manifest.entry);

            let program = std::fs::read(&entry_path).with_context(|| {
                format!(
                    "[X07ENTRY_READ] read entry: {} (hint: check x07.json `entry`)",
                    entry_path.display()
                )
            })?;

            let module_roots = project::collect_module_roots(project_path, &manifest, &lock)?;
            let compile_options = x07_host_runner::compile_options_for_world(world, module_roots)?;

            let compile = compile_program_with_options(
                &program,
                &config,
                cli.compiled_out.as_deref(),
                &compile_options,
                &[],
            )?;
            if cli.compile_only {
                let exit_code: u8 = if compile.ok { 0 } else { 1 };
                let json = serde_json::json!({
                    "schema_version": X07_HOST_RUNNER_REPORT_SCHEMA_VERSION,
                    "mode": "project-compile",
                    "exit_code": exit_code,
                    "compile": {
                        "ok": compile.ok,
                        "exit_status": compile.exit_status,
                        "lang_id": compile.lang_id,
                        "guide_md": compile.guide_md,
                        "native_requires": compile.native_requires,
                        "c_source_size": compile.c_source_size,
                        "compiled_exe": compile.compiled_exe.as_ref().map(|p| p.display().to_string()),
                        "compiled_exe_size": compile.compiled_exe_size,
                        "compile_error": compile.compile_error,
                        "stdout_b64": b64.encode(&compile.stdout),
                        "stderr_b64": b64.encode(&compile.stderr),
                        "fuel_used": compile.fuel_used,
                        "trap": compile.trap,
                    },
                    "solve": serde_json::Value::Null,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);

                return Ok(std::process::ExitCode::from(exit_code));
            }
            if !compile.ok {
                let exit_code: u8 = 1;
                let json = serde_json::json!({
                    "schema_version": X07_HOST_RUNNER_REPORT_SCHEMA_VERSION,
                    "mode": "project-compile-run",
                    "exit_code": exit_code,
                    "compile": {
                        "ok": compile.ok,
                        "exit_status": compile.exit_status,
                        "lang_id": compile.lang_id,
                        "guide_md": compile.guide_md,
                        "native_requires": compile.native_requires,
                        "c_source_size": compile.c_source_size,
                        "compiled_exe": compile.compiled_exe.as_ref().map(|p| p.display().to_string()),
                        "compiled_exe_size": compile.compiled_exe_size,
                        "compile_error": compile.compile_error,
                        "stdout_b64": b64.encode(&compile.stdout),
                        "stderr_b64": b64.encode(&compile.stderr),
                        "fuel_used": compile.fuel_used,
                        "trap": compile.trap,
                    },
                    "solve": serde_json::Value::Null,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
                return Ok(std::process::ExitCode::from(exit_code));
            }

            let exe = compile
                .compiled_exe
                .clone()
                .context("internal error: compile.ok but no compiled_exe")?;
            let solve = run_artifact_file(&config, &exe, &input)?;

            let ok = compile.ok && solve.ok && solve.exit_status == 0;
            let exit_code: u8 = if ok { 0 } else { 1 };
            let json = serde_json::json!({
                "schema_version": X07_HOST_RUNNER_REPORT_SCHEMA_VERSION,
                "mode": "project-compile-run",
                "exit_code": exit_code,
                "compile": {
                    "ok": compile.ok,
                    "exit_status": compile.exit_status,
                    "lang_id": compile.lang_id,
                    "guide_md": compile.guide_md,
                    "native_requires": compile.native_requires,
                    "c_source_size": compile.c_source_size,
                    "compiled_exe": compile.compiled_exe.as_ref().map(|p| p.display().to_string()),
                    "compiled_exe_size": compile.compiled_exe_size,
                    "compile_error": compile.compile_error,
                    "stdout_b64": b64.encode(&compile.stdout),
                    "stderr_b64": b64.encode(&compile.stderr),
                    "fuel_used": compile.fuel_used,
                    "trap": compile.trap,
                },
                "solve": {
                    "ok": solve.ok,
                    "exit_status": solve.exit_status,
                    "solve_output_b64": b64.encode(&solve.solve_output),
                    "stdout_b64": b64.encode(&solve.stdout),
                    "stderr_b64": b64.encode(&solve.stderr),
                    "fuel_used": solve.fuel_used,
                    "heap_used": solve.heap_used,
                    "fs_read_file_calls": solve.fs_read_file_calls,
                    "fs_list_dir_calls": solve.fs_list_dir_calls,
                    "rr_send_calls": solve.rr_send_calls,
                    "rr_request_calls": solve.rr_request_calls,
                    "rr_last_request_sha256": solve.rr_last_request_sha256,
                    "kv_get_calls": solve.kv_get_calls,
                    "kv_set_calls": solve.kv_set_calls,
                    "sched_stats": solve.sched_stats,
                    "mem_stats": solve.mem_stats,
                    "debug_stats": solve.debug_stats,
                    "trap": solve.trap,
                },
            });
            println!("{}", serde_json::to_string_pretty(&json)?);

            Ok(std::process::ExitCode::from(exit_code))
        }
    }
}
