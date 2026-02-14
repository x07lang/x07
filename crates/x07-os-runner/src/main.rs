use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use base64::Engine;
use clap::Parser;
#[cfg(test)]
use x07_contracts::RUN_OS_POLICY_SCHEMA_VERSION;
use x07_contracts::X07_OS_RUNNER_REPORT_SCHEMA_VERSION;
use x07_host_runner::{
    apply_cc_profile, compile_program_with_options, CcProfile, CompilerResult, RunnerConfig,
    RunnerResult,
};
use x07_runner_common::sandbox_backend::{
    resolve_sandbox_backend, EffectiveSandboxBackend, SandboxBackend,
};
use x07_runner_common::{auto_ffi, os_env, os_paths};
use x07_vm::{
    copy_dir_recursive, default_cleanup_ms, default_grace_ms, firecracker_ctr_config_from_env,
    resolve_sibling_or_path as resolve_sibling_or_path_vm, resolve_vm_backend, LimitsSpec,
    MountSpec, NetworkMode, RunSpec, VmBackend,
};
use x07_worlds::WorldId;

#[cfg(test)]
use x07c::compile;
use x07c::project;

mod policy;
mod sandbox;

static VM_RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Parser)]
#[command(name = "x07-os-runner")]
#[command(about = "Standalone runner for run-os / run-os-sandboxed worlds.", long_about = None)]
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

    #[arg(long, value_enum, default_value_t = WorldId::RunOs)]
    world: WorldId,

    /// Sandbox backend selection (run-os-sandboxed defaults to "vm").
    #[arg(long, value_enum)]
    sandbox_backend: Option<SandboxBackend>,

    /// Required to run run-os-sandboxed without a VM boundary.
    #[arg(long)]
    i_accept_weaker_isolation: bool,

    #[arg(long, value_name = "BYTES")]
    max_c_bytes: Option<usize>,

    #[arg(long)]
    policy: Option<PathBuf>,

    #[arg(long)]
    input: Option<PathBuf>,

    #[arg(long, default_value_t = 50_000_000)]
    solve_fuel: u64,

    #[arg(long, default_value_t = 64 * 1024 * 1024)]
    max_memory_bytes: usize,

    #[arg(long)]
    max_output_bytes: Option<usize>,

    #[arg(long, default_value_t = 30)]
    cpu_time_limit_seconds: u64,

    #[arg(long)]
    debug_borrow_checks: bool,

    #[arg(long)]
    compiled_out: Option<PathBuf>,

    /// Compile but do not run (internal; used for VM build/run separation).
    #[arg(long, hide = true)]
    compile_only: bool,

    /// Allow `--artifact` for run-os-sandboxed (internal; build/run separation only).
    #[arg(long, hide = true)]
    i_accept_precompiled_artifact: bool,

    #[arg(long)]
    module_root: Vec<PathBuf>,

    #[arg(long)]
    auto_ffi: bool,
}

fn main() -> std::process::ExitCode {
    // Some platforms default to a small stack, which is not enough for our current compiler
    // recursion depth (for example, larger template and example projects). Run the real entrypoint
    // on a larger-stack thread to keep behavior consistent.
    let handle = std::thread::Builder::new()
        .name("x07-os-runner".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(run);

    match handle {
        Ok(handle) => match handle.join() {
            Ok(code) => code,
            Err(panic) => {
                if let Some(message) = panic.downcast_ref::<&str>() {
                    eprintln!("x07-os-runner panicked: {message}");
                } else if let Some(message) = panic.downcast_ref::<String>() {
                    eprintln!("x07-os-runner panicked: {message}");
                } else {
                    eprintln!("x07-os-runner panicked");
                }
                std::process::ExitCode::from(2)
            }
        },
        Err(err) => {
            eprintln!("failed to spawn x07-os-runner thread: {err}");
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

    if cli.compile_only && cli.artifact.is_some() {
        anyhow::bail!("--compile-only is not supported with --artifact");
    }

    if cli.i_accept_weaker_isolation {
        std::env::set_var(x07_vm::ENV_ACCEPT_WEAKER_ISOLATION, "1");
    }

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

    let world = cli.world;
    if world.is_eval_world() {
        anyhow::bail!(
            "refusing to run eval world {:?} in x07-os-runner",
            world.as_str()
        );
    }

    let sandbox_backend =
        resolve_sandbox_backend(world, cli.sandbox_backend, cli.i_accept_weaker_isolation)?;

    let policy = load_policy(world, cli.policy.as_ref())?;
    if let Some(ref pol) = policy {
        if sandbox_backend == EffectiveSandboxBackend::Os {
            sandbox::apply_sandbox(pol).map_err(|e| anyhow::anyhow!("{e}"))?;
        }
    }
    let (allow_unsafe, allow_ffi) = match (world, policy.as_ref()) {
        (WorldId::RunOsSandboxed, Some(pol)) => (
            Some(pol.language.allow_unsafe),
            Some(pol.language.allow_ffi),
        ),
        _ => (None, None),
    };

    let input = match &cli.input {
        Some(path) => {
            std::fs::read(path).with_context(|| format!("read input: {}", path.display()))?
        }
        None => Vec::new(),
    };

    let max_output_bytes = cli.max_output_bytes.unwrap_or(1024 * 1024);
    #[cfg(unix)]
    let run_limits = run_limits_for_cli(&cli, world, policy.as_ref());
    let wall_ms = wall_timeout_ms_for_cli(world, policy.as_ref(), &cli);

    if sandbox_backend == EffectiveSandboxBackend::Vm {
        match run_vm(
            &cli,
            world,
            policy.as_ref(),
            &input,
            wall_ms,
            max_output_bytes,
        ) {
            Ok(code) => return Ok(code),
            Err(err) => {
                let _ = std::io::Write::write_all(
                    &mut std::io::stderr(),
                    format!("{err:#}\n").as_bytes(),
                );

                let mode = if cli.project.is_some() {
                    "project-compile-run"
                } else {
                    "compile-run"
                };
                let json =
                    synthesize_vm_backend_failure_report(mode, world, "vm backend failure", &err)?;
                println!("{}", serde_json::to_string_pretty(&json)?);
                return Ok(std::process::ExitCode::from(1));
            }
        }
    }

    match (&cli.artifact, &cli.program, &cli.project) {
        (Some(_), Some(_), _)
        | (Some(_), _, Some(_))
        | (_, Some(_), Some(_))
        | (None, None, None) => {
            anyhow::bail!("set exactly one of --artifact, --program, or --project")
        }

        (Some(artifact), None, None) => {
            if world == WorldId::RunOsSandboxed && !cli.i_accept_precompiled_artifact {
                anyhow::bail!("run-os-sandboxed does not support --artifact; use --program or --project so x07-os-runner can enforce policy.language.allow_unsafe/allow_ffi at compile time");
            }
            let inv = RunInvocation {
                artifact,
                world,
                policy: policy.as_ref(),
                input: &input,
                max_output_bytes,
                #[cfg(unix)]
                limits: &run_limits,
                wall_ms,
                run_dir: None,
            };
            let solve = run_os_artifact(&inv)?;

            let exit_code: u8 = if solve.ok && solve.exit_status == 0 {
                0
            } else {
                1
            };
            let b64 = base64::engine::general_purpose::STANDARD;
            let json = serde_json::json!({
                "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
                "mode": "run-os",
                "world": world.as_str(),
                "ok": solve.ok,
                "exit_code": exit_code,
                "exit_status": solve.exit_status,
                "solve_output_b64": b64.encode(&solve.solve_output),
                "stdout_b64": b64.encode(&solve.stdout),
                "stderr_b64": b64.encode(&solve.stderr),
                "fuel_used": solve.fuel_used,
                "heap_used": solve.heap_used,
                "fs_read_file_calls": solve.fs_read_file_calls,
                "fs_list_dir_calls": solve.fs_list_dir_calls,
                "rr_open_calls": solve.rr_open_calls,
                "rr_close_calls": solve.rr_close_calls,
                "rr_stats_calls": solve.rr_stats_calls,
                "rr_next_calls": solve.rr_next_calls,
                "rr_next_miss_calls": solve.rr_next_miss_calls,
                "rr_append_calls": solve.rr_append_calls,
                "kv_get_calls": solve.kv_get_calls,
                "kv_set_calls": solve.kv_set_calls,
                "sched_stats": solve.sched_stats,
                "mem_stats": solve.mem_stats,
                "debug_stats": solve.debug_stats,
                "trap": solve.trap,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);

            Ok(std::process::ExitCode::from(exit_code))
        }

        (None, Some(program_path), None) => {
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

            let module_roots = collect_module_roots_for_os(&cli)?;
            let auto_ffi_cc_args = if cli.auto_ffi {
                auto_ffi::collect_auto_ffi_cc_args(&module_roots)?
            } else {
                Vec::new()
            };
            let mut compile_options =
                x07c::world_config::compile_options_for_world(world, module_roots);
            compile_options.arch_root =
                infer_arch_root_from_path(program_path).or_else(|| std::env::current_dir().ok());
            compile_options.allow_unsafe = allow_unsafe;
            compile_options.allow_ffi = allow_ffi;

            let cfg = compile_runner_config(&cli, max_output_bytes);
            let compile = compile_program_with_options(
                &program,
                &cfg,
                cli.compiled_out.as_deref(),
                &compile_options,
                &auto_ffi_cc_args,
            )?;

            if !compile.ok {
                let b64 = base64::engine::general_purpose::STANDARD;
                let exit_code: u8 = 1;
                let json = serde_json::json!({
                    "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
                    "mode": "compile-run",
                    "world": world.as_str(),
                    "exit_code": exit_code,
                    "compile": compiler_json(&compile, &b64),
                    "solve": serde_json::Value::Null,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
                return Ok(std::process::ExitCode::from(exit_code));
            }

            if cli.compile_only {
                let b64 = base64::engine::general_purpose::STANDARD;
                let exit_code: u8 = 0;
                let json = serde_json::json!({
                    "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
                    "mode": "compile-run",
                    "world": world.as_str(),
                    "exit_code": exit_code,
                    "compile": compiler_json(&compile, &b64),
                    "solve": serde_json::Value::Null,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
                return Ok(std::process::ExitCode::from(exit_code));
            }

            if cli.compile_only {
                let b64 = base64::engine::general_purpose::STANDARD;
                let exit_code: u8 = 0;
                let json = serde_json::json!({
                    "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
                    "mode": "project-compile-run",
                    "world": world.as_str(),
                    "exit_code": exit_code,
                    "compile": compiler_json(&compile, &b64),
                    "solve": serde_json::Value::Null,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
                return Ok(std::process::ExitCode::from(exit_code));
            }

            let exe = compile
                .compiled_exe
                .clone()
                .context("internal error: compile.ok but no compiled_exe")?;
            let inv = RunInvocation {
                artifact: &exe,
                world,
                policy: policy.as_ref(),
                input: &input,
                max_output_bytes,
                #[cfg(unix)]
                limits: &run_limits,
                wall_ms,
                run_dir: None,
            };
            let solve = run_os_artifact(&inv)?;

            let exit_code: u8 = if compile.ok && solve.ok && solve.exit_status == 0 {
                0
            } else {
                1
            };
            let b64 = base64::engine::general_purpose::STANDARD;
            let json = serde_json::json!({
                "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
                "mode": "compile-run",
                "world": world.as_str(),
                "exit_code": exit_code,
                "compile": compiler_json(&compile, &b64),
                "solve": runner_json(&solve, &b64),
            });
            println!("{}", serde_json::to_string_pretty(&json)?);

            Ok(std::process::ExitCode::from(exit_code))
        }

        (None, None, Some(project_path)) => {
            let manifest = project::load_project_manifest(project_path)?;
            let base = project_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let mut extra_cc_args = manifest.link.cc_args(base);
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

            let entry_path = base.join(&manifest.entry);

            let program = std::fs::read(&entry_path).with_context(|| {
                format!(
                    "[X07ENTRY_READ] read entry: {} (hint: check x07.json `entry`)",
                    entry_path.display()
                )
            })?;

            let mut module_roots = project::collect_module_roots(project_path, &manifest, &lock)?;
            let os_roots = os_paths::default_os_module_roots()?;
            for r in os_roots {
                if !module_roots.contains(&r) {
                    module_roots.push(r);
                }
            }

            if cli.auto_ffi {
                extra_cc_args.extend(auto_ffi::collect_auto_ffi_cc_args(&module_roots)?);
            }

            let mut compile_options =
                x07c::world_config::compile_options_for_world(world, module_roots);
            compile_options.arch_root = infer_arch_root_from_path(project_path)
                .or_else(|| Some(base.to_path_buf()))
                .or_else(|| std::env::current_dir().ok());
            compile_options.allow_unsafe = allow_unsafe;
            compile_options.allow_ffi = allow_ffi;

            let cfg = compile_runner_config(&cli, max_output_bytes);
            let compile = compile_program_with_options(
                &program,
                &cfg,
                cli.compiled_out.as_deref(),
                &compile_options,
                &extra_cc_args,
            )?;
            if !compile.ok {
                let b64 = base64::engine::general_purpose::STANDARD;
                let exit_code: u8 = 1;
                let json = serde_json::json!({
                    "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
                    "mode": "project-compile-run",
                    "world": world.as_str(),
                    "exit_code": exit_code,
                    "compile": compiler_json(&compile, &b64),
                    "solve": serde_json::Value::Null,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
                return Ok(std::process::ExitCode::from(exit_code));
            }

            let exe = compile
                .compiled_exe
                .clone()
                .context("internal error: compile.ok but no compiled_exe")?;
            let inv = RunInvocation {
                artifact: &exe,
                world,
                policy: policy.as_ref(),
                input: &input,
                max_output_bytes,
                #[cfg(unix)]
                limits: &run_limits,
                wall_ms,
                run_dir: Some(base),
            };
            let solve = run_os_artifact(&inv)?;

            let exit_code: u8 = if compile.ok && solve.ok && solve.exit_status == 0 {
                0
            } else {
                1
            };
            let b64 = base64::engine::general_purpose::STANDARD;
            let json = serde_json::json!({
                "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
                "mode": "project-compile-run",
                "world": world.as_str(),
                "exit_code": exit_code,
                "compile": compiler_json(&compile, &b64),
                "solve": runner_json(&solve, &b64),
            });
            println!("{}", serde_json::to_string_pretty(&json)?);

            Ok(std::process::ExitCode::from(exit_code))
        }
    }
}

fn synthesize_vm_backend_failure_report(
    mode: &str,
    world: WorldId,
    context: &str,
    err: &anyhow::Error,
) -> Result<serde_json::Value> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let err_msg = err.to_string();
    let compile = serde_json::json!({
        "ok": false,
        "exit_status": 1,
        "lang_id": "",
        "native_requires": {
            "schema_version": "x07.native-requires@0.1.0",
            "world": world.as_str(),
            "requires": [],
        },
        "c_source_size": 0,
        "compiled_exe": serde_json::Value::Null,
        "compiled_exe_size": serde_json::Value::Null,
        "compile_error": serde_json::Value::Null,
        "stdout_b64": b64.encode(b""),
        "stderr_b64": b64.encode(err_msg.as_bytes()),
        "fuel_used": serde_json::Value::Null,
        "trap": format!("{context}: {err_msg}"),
    });

    Ok(serde_json::json!({
        "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
        "mode": mode,
        "world": world.as_str(),
        "exit_code": 1,
        "compile": compile,
        "solve": serde_json::Value::Null,
    }))
}

fn now_unix_ms() -> Result<u64> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(d.as_millis().try_into().unwrap_or(u64::MAX))
}

fn default_vm_guest_image() -> String {
    format!(
        "ghcr.io/x07lang/x07-guest-runner:{}",
        env!("CARGO_PKG_VERSION")
    )
}

fn policy_bytes_with_wall_override(policy_bytes: &[u8], wall_ms: u64) -> Result<Vec<u8>> {
    let mut v: serde_json::Value =
        serde_json::from_slice(policy_bytes).context("parse policy JSON")?;
    let limits = v
        .get_mut("limits")
        .and_then(|v| v.as_object_mut())
        .context("policy JSON missing limits object")?;
    limits.insert(
        "wall_ms".to_string(),
        serde_json::Value::from(wall_ms.max(1)),
    );
    let mut out = serde_json::to_vec_pretty(&v)?;
    out.push(b'\n');
    Ok(out)
}

fn extract_runner_result_from_run_os_report_json(
    run_os_report: &serde_json::Value,
) -> Result<serde_json::Value> {
    let run_os_report = run_os_report
        .as_object()
        .context("run-os report JSON must be an object")?;

    let mut out = serde_json::Map::new();
    for key in [
        "ok",
        "exit_status",
        "solve_output_b64",
        "stdout_b64",
        "stderr_b64",
        "fuel_used",
        "heap_used",
        "fs_read_file_calls",
        "fs_list_dir_calls",
        "rr_open_calls",
        "rr_close_calls",
        "rr_stats_calls",
        "rr_next_calls",
        "rr_next_miss_calls",
        "rr_append_calls",
        "kv_get_calls",
        "kv_set_calls",
        "sched_stats",
        "mem_stats",
        "debug_stats",
        "trap",
    ] {
        let v = run_os_report
            .get(key)
            .with_context(|| format!("missing required key in run-os report: {key:?}"))?;
        out.insert(key.to_string(), v.clone());
    }

    Ok(serde_json::Value::Object(out))
}

fn synthesize_vm_solve_failure_runner_result(
    exit_status: i32,
    stdout: &[u8],
    stderr: &[u8],
    trap: String,
) -> serde_json::Value {
    let b64 = base64::engine::general_purpose::STANDARD;
    serde_json::json!({
        "ok": false,
        "exit_status": exit_status,
        "solve_output_b64": b64.encode(b""),
        "stdout_b64": b64.encode(stdout),
        "stderr_b64": b64.encode(stderr),
        "fuel_used": serde_json::Value::Null,
        "heap_used": serde_json::Value::Null,
        "fs_read_file_calls": serde_json::Value::Null,
        "fs_list_dir_calls": serde_json::Value::Null,
        "rr_open_calls": serde_json::Value::Null,
        "rr_close_calls": serde_json::Value::Null,
        "rr_stats_calls": serde_json::Value::Null,
        "rr_next_calls": serde_json::Value::Null,
        "rr_next_miss_calls": serde_json::Value::Null,
        "rr_append_calls": serde_json::Value::Null,
        "kv_get_calls": serde_json::Value::Null,
        "kv_set_calls": serde_json::Value::Null,
        "sched_stats": serde_json::Value::Null,
        "mem_stats": serde_json::Value::Null,
        "debug_stats": serde_json::Value::Null,
        "trap": trap,
    })
}

fn run_vm(
    cli: &Cli,
    world: WorldId,
    policy: Option<&policy::Policy>,
    input: &[u8],
    wall_ms: u64,
    max_output_bytes: usize,
) -> Result<std::process::ExitCode> {
    if world != WorldId::RunOsSandboxed {
        anyhow::bail!("sandbox_backend=vm is only supported for --world run-os-sandboxed");
    }
    let policy = policy.context("internal error: run-os-sandboxed policy missing")?;

    let backend = resolve_vm_backend()?;

    let guest_image = if backend == VmBackend::Vz {
        std::env::var(x07_vm::ENV_VZ_GUEST_BUNDLE).unwrap_or_default()
    } else {
        std::env::var("X07_VM_GUEST_IMAGE").unwrap_or_else(|_| default_vm_guest_image())
    };

    if let Ok(expected_digest) = std::env::var(x07_vm::ENV_VM_GUEST_IMAGE_DIGEST) {
        let accept_weaker_isolation = x07_vm::read_accept_weaker_isolation_env().unwrap_or(false);
        if !accept_weaker_isolation {
            let firecracker_cfg = if backend == VmBackend::FirecrackerCtr {
                Some(firecracker_ctr_config_from_env())
            } else {
                None
            };
            x07_vm::verify_vm_guest_digest(
                backend,
                &guest_image,
                &expected_digest,
                firecracker_cfg.as_ref(),
            )?;
        }
    }

    let overall_created_unix_ms = now_unix_ms()?;
    let run_id_base = {
        let pid = std::process::id();
        let n = VM_RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{overall_created_unix_ms}-{pid}-{n}")
    };

    let overall_deadline_unix_ms = overall_created_unix_ms.saturating_add(wall_ms.max(1));

    let state_root = x07_vm::default_vm_state_root()?;

    let build_run_id = format!("{run_id_base}-build");
    let run_run_id = format!("{run_id_base}-run");

    let policy_path = cli
        .policy
        .as_ref()
        .context("run-os-sandboxed requires --policy")?;
    let policy_bytes = std::fs::read(policy_path)
        .with_context(|| format!("read policy: {}", policy_path.display()))?;

    let build_state_dir = state_root.join(&build_run_id);
    let build_job_in = build_state_dir.join("in");
    let build_job_out = build_state_dir.join("out");
    std::fs::create_dir_all(&build_job_in)
        .with_context(|| format!("create build job input dir: {}", build_job_in.display()))?;
    std::fs::create_dir_all(&build_job_out)
        .with_context(|| format!("create build job output dir: {}", build_job_out.display()))?;

    std::fs::write(build_job_in.join("policy.json"), &policy_bytes)
        .context("write build policy.json")?;
    std::fs::write(build_job_in.join("input.bin"), input).context("write build input.bin")?;

    let (mode, guest_target_args, base_host, base_guest) = match (&cli.program, &cli.project) {
        (Some(program_path), None) => {
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

            let bytes = std::fs::read(program_path)
                .with_context(|| format!("read program: {}", program_path.display()))?;
            let program_dir = build_job_in.join("program");
            std::fs::create_dir_all(&program_dir)
                .with_context(|| format!("create program dir: {}", program_dir.display()))?;
            let guest_program_path = PathBuf::from("/x07/in/program/main.x07.json");
            std::fs::write(program_dir.join("main.x07.json"), bytes)
                .with_context(|| format!("write program to {}", program_dir.display()))?;

            let module_roots_dir = build_job_in.join("module_roots");
            std::fs::create_dir_all(&module_roots_dir).with_context(|| {
                format!("create module_roots dir: {}", module_roots_dir.display())
            })?;

            let mut guest_target_args: Vec<String> = vec![
                "--program".to_string(),
                guest_program_path.display().to_string(),
            ];

            for (idx, root) in cli.module_root.iter().enumerate() {
                let root_abs = if root.is_absolute() {
                    root.to_path_buf()
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(root)
                };
                let dst = module_roots_dir.join(idx.to_string());
                copy_dir_recursive(&root_abs, &dst).with_context(|| {
                    format!(
                        "copy module root {} -> {}",
                        root_abs.display(),
                        dst.display()
                    )
                })?;

                guest_target_args.push("--module-root".to_string());
                guest_target_args.push(format!("/x07/in/module_roots/{idx}"));
            }

            (
                "compile-run",
                guest_target_args,
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                PathBuf::from("/opt/x07"),
            )
        }

        (None, Some(project_path)) => {
            let base = project_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();

            let project_dst = build_job_in.join("project");
            copy_dir_recursive(&base, &project_dst).with_context(|| {
                format!(
                    "copy project dir {} -> {}",
                    base.display(),
                    project_dst.display()
                )
            })?;

            let file_name = project_path
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("x07.json"));
            let guest_project_path = PathBuf::from("/x07/in/project").join(file_name);

            let guest_target_args: Vec<String> = vec![
                "--project".to_string(),
                guest_project_path.display().to_string(),
            ];

            (
                "project-compile-run",
                guest_target_args,
                base,
                PathBuf::from("/x07/in/project"),
            )
        }

        (Some(_), Some(_)) => {
            anyhow::bail!("set exactly one of --program or --project");
        }
        (None, None) => {
            anyhow::bail!("set --program or --project for sandbox_backend=vm");
        }
    };

    let build_mounts: Vec<MountSpec> = vec![
        MountSpec {
            host_path: build_job_in.clone(),
            guest_path: PathBuf::from("/x07/in"),
            readonly: true,
        },
        MountSpec {
            host_path: build_job_out.clone(),
            guest_path: PathBuf::from("/x07/out"),
            readonly: false,
        },
    ];

    let compiled_out_guest_path = "/x07/out/compiled-out";
    let mut build_guest_argv: Vec<String> = vec![
        "x07-os-runner".to_string(),
        "--cc-profile".to_string(),
        match cli.cc_profile {
            CcProfile::Default => "default",
            CcProfile::Size => "size",
        }
        .to_string(),
        "--world".to_string(),
        world.as_str().to_string(),
        "--sandbox-backend".to_string(),
        "os".to_string(),
        "--i-accept-weaker-isolation".to_string(),
    ];

    if let Some(max_c_bytes) = cli.max_c_bytes {
        build_guest_argv.push("--max-c-bytes".to_string());
        build_guest_argv.push(max_c_bytes.to_string());
    }

    build_guest_argv.push("--policy".to_string());
    build_guest_argv.push("/x07/in/policy.json".to_string());

    build_guest_argv.push("--input".to_string());
    build_guest_argv.push("/x07/in/input.bin".to_string());

    build_guest_argv.push("--solve-fuel".to_string());
    build_guest_argv.push(cli.solve_fuel.to_string());

    build_guest_argv.push("--max-memory-bytes".to_string());
    build_guest_argv.push(cli.max_memory_bytes.to_string());

    if cli.max_output_bytes.is_some() {
        build_guest_argv.push("--max-output-bytes".to_string());
        build_guest_argv.push(max_output_bytes.to_string());
    }

    build_guest_argv.push("--cpu-time-limit-seconds".to_string());
    build_guest_argv.push(cli.cpu_time_limit_seconds.to_string());

    if cli.debug_borrow_checks {
        build_guest_argv.push("--debug-borrow-checks".to_string());
    }

    build_guest_argv.push("--compiled-out".to_string());
    build_guest_argv.push(compiled_out_guest_path.to_string());
    build_guest_argv.push("--compile-only".to_string());

    if cli.auto_ffi {
        build_guest_argv.push("--auto-ffi".to_string());
    }

    build_guest_argv.extend(guest_target_args.clone());

    let build_created_unix_ms = now_unix_ms()?;
    let build_wall_ms = overall_deadline_unix_ms
        .saturating_sub(build_created_unix_ms)
        .max(1);
    let build_limits = LimitsSpec {
        wall_ms: build_wall_ms,
        grace_ms: default_grace_ms(build_wall_ms),
        cleanup_ms: default_cleanup_ms(),
        mem_bytes: Some(policy.limits.mem_bytes),
        vcpus: None,
        max_stdout_bytes: 32 * 1024 * 1024,
        max_stderr_bytes: 32 * 1024 * 1024,
        network: NetworkMode::None,
    };

    let build_spec = RunSpec {
        run_id: build_run_id.clone(),
        backend,
        image: guest_image.clone(),
        argv: build_guest_argv,
        env: BTreeMap::new(),
        mounts: build_mounts,
        workdir: Some(PathBuf::from("/opt/x07")),
        limits: build_limits,
    };

    let firecracker_cfg = if backend == VmBackend::FirecrackerCtr {
        Some(firecracker_ctr_config_from_env())
    } else {
        None
    };

    let reaper_bin = resolve_sibling_or_path_vm("x07-vm-reaper");
    let build_out = x07_vm::run_vm_job(
        &build_spec,
        x07_vm::VmJobRunParams {
            state_root: &state_root,
            state_dir: &build_state_dir,
            reaper_bin: &reaper_bin,
            created_unix_ms: build_created_unix_ms,
            deadline_unix_ms: overall_deadline_unix_ms,
            firecracker_cfg: firecracker_cfg.as_ref(),
        },
    )?;

    if !build_out.stderr.is_empty() {
        let _ = std::io::Write::write_all(&mut std::io::stderr(), &build_out.stderr);
    }

    let mut build_report_bytes = build_out.stdout;
    if !build_report_bytes.ends_with(b"\n") {
        build_report_bytes.push(b'\n');
    }

    let mut build_exit_code: u8 = 1;
    let build_report_json: serde_json::Value = match serde_json::from_slice(&build_report_bytes) {
        Ok(v) => v,
        Err(err) => {
            let json = synthesize_vm_runner_output_failure_report(
                mode,
                world,
                build_out.exit_status,
                &build_report_bytes,
                &build_out.stderr,
                format!("invalid runner report JSON: {err}"),
            )?;
            println!("{}", serde_json::to_string_pretty(&json)?);
            return Ok(std::process::ExitCode::from(1));
        }
    };

    if build_report_json
        .get("schema_version")
        .and_then(|v| v.as_str())
        != Some(X07_OS_RUNNER_REPORT_SCHEMA_VERSION)
    {
        let json = synthesize_vm_runner_output_failure_report(
            mode,
            world,
            build_out.exit_status,
            &build_report_bytes,
            &build_out.stderr,
            "runner report schema_version mismatch".to_string(),
        )?;
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(std::process::ExitCode::from(1));
    }

    if let Some(v) = build_report_json.get("exit_code").and_then(|v| v.as_u64()) {
        build_exit_code = v.min(255) as u8;
    } else if build_out.exit_status == 0 && !build_out.timed_out {
        build_exit_code = 0;
    }

    if build_report_json.get("mode").and_then(|v| v.as_str()) != Some(mode) {
        let json = synthesize_vm_runner_output_failure_report(
            mode,
            world,
            build_out.exit_status,
            &build_report_bytes,
            &build_out.stderr,
            "runner report mode mismatch".to_string(),
        )?;
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(std::process::ExitCode::from(1));
    }

    let compile_ok = build_report_json
        .get("compile")
        .and_then(|v| v.get("ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !compile_ok {
        std::io::Write::write_all(&mut std::io::stdout(), &build_report_bytes)
            .context("write report")?;
        return Ok(std::process::ExitCode::from(build_exit_code));
    }

    let compiled_artifact = build_job_out.join("compiled-out");
    if !compiled_artifact.is_file() {
        anyhow::bail!(
            "vm build phase did not produce expected compiled artifact at {}",
            compiled_artifact.display()
        );
    }

    if let Some(host_compiled_out) = cli.compiled_out.as_ref() {
        if let Some(parent) = host_compiled_out.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create compiled_out parent dir: {}", parent.display()))?;
        }
        std::fs::copy(&compiled_artifact, host_compiled_out).with_context(|| {
            format!(
                "copy compiled_out {} -> {}",
                compiled_artifact.display(),
                host_compiled_out.display()
            )
        })?;
    }

    let run_state_dir = state_root.join(&run_run_id);
    let run_job_in = run_state_dir.join("in");
    let run_job_out = run_state_dir.join("out");
    std::fs::create_dir_all(&run_job_in)
        .with_context(|| format!("create run job input dir: {}", run_job_in.display()))?;
    std::fs::create_dir_all(&run_job_out)
        .with_context(|| format!("create run job output dir: {}", run_job_out.display()))?;

    let run_created_unix_ms = now_unix_ms()?;
    let run_wall_ms = overall_deadline_unix_ms
        .saturating_sub(run_created_unix_ms)
        .max(1);
    let run_policy_bytes = policy_bytes_with_wall_override(&policy_bytes, run_wall_ms)?;
    std::fs::write(run_job_in.join("policy.json"), &run_policy_bytes)
        .context("write run policy.json")?;
    std::fs::write(run_job_in.join("input.bin"), input).context("write run input.bin")?;

    if mode == "project-compile-run" {
        std::fs::create_dir_all(run_job_in.join("project"))
            .context("create run project mountpoint dir")?;
    }

    let artifact_path = run_job_in.join("artifact");
    std::fs::copy(&compiled_artifact, &artifact_path).with_context(|| {
        format!(
            "copy compiled artifact {} -> {}",
            compiled_artifact.display(),
            artifact_path.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(&artifact_path, std::fs::Permissions::from_mode(0o755));
    }

    let mut run_mounts: Vec<MountSpec> = vec![
        MountSpec {
            host_path: run_job_in.clone(),
            guest_path: PathBuf::from("/x07/in"),
            readonly: true,
        },
        MountSpec {
            host_path: run_job_out.clone(),
            guest_path: PathBuf::from("/x07/out"),
            readonly: false,
        },
    ];
    x07_vm::append_root_mounts(
        &mut run_mounts,
        &policy.fs.read_roots,
        &policy.fs.write_roots,
        &base_host,
        &base_guest,
    )?;

    let mut run_guest_argv: Vec<String> = vec![
        "x07-os-runner".to_string(),
        "--world".to_string(),
        world.as_str().to_string(),
        "--sandbox-backend".to_string(),
        "os".to_string(),
        "--i-accept-weaker-isolation".to_string(),
        "--i-accept-precompiled-artifact".to_string(),
        "--policy".to_string(),
        "/x07/in/policy.json".to_string(),
        "--input".to_string(),
        "/x07/in/input.bin".to_string(),
        "--artifact".to_string(),
        "/x07/in/artifact".to_string(),
        "--cpu-time-limit-seconds".to_string(),
        cli.cpu_time_limit_seconds.to_string(),
    ];
    if cli.max_output_bytes.is_some() {
        run_guest_argv.push("--max-output-bytes".to_string());
        run_guest_argv.push(max_output_bytes.to_string());
    }

    let accept_weaker_isolation = cli.i_accept_weaker_isolation
        || x07_vm::read_accept_weaker_isolation_env().unwrap_or(false);
    let allowlist_requested = policy.net.enabled && !policy.net.allow_hosts.is_empty();
    let run_network_mode = if allowlist_requested {
        if backend == VmBackend::Vz || accept_weaker_isolation {
            NetworkMode::Default
        } else {
            anyhow::bail!(
                "VM backend {backend} does not yet enforce policy.net.allow_hosts at the VM boundary.\n\nfix:\n  - use the VZ backend (macOS): X07_VM_BACKEND=vz, or\n  - set X07_I_ACCEPT_WEAKER_ISOLATION=1 to allow networking without VM-boundary allowlist enforcement"
            );
        }
    } else {
        NetworkMode::None
    };

    let run_limits = LimitsSpec {
        wall_ms: run_wall_ms,
        grace_ms: default_grace_ms(run_wall_ms),
        cleanup_ms: default_cleanup_ms(),
        mem_bytes: Some(policy.limits.mem_bytes),
        vcpus: None,
        max_stdout_bytes: 32 * 1024 * 1024,
        max_stderr_bytes: 32 * 1024 * 1024,
        network: run_network_mode,
    };

    let run_spec = RunSpec {
        run_id: run_run_id.clone(),
        backend,
        image: guest_image,
        argv: run_guest_argv,
        env: BTreeMap::new(),
        mounts: run_mounts,
        workdir: Some(PathBuf::from("/opt/x07")),
        limits: run_limits,
    };

    let run_out = x07_vm::run_vm_job(
        &run_spec,
        x07_vm::VmJobRunParams {
            state_root: &state_root,
            state_dir: &run_state_dir,
            reaper_bin: &reaper_bin,
            created_unix_ms: run_created_unix_ms,
            deadline_unix_ms: overall_deadline_unix_ms,
            firecracker_cfg: firecracker_cfg.as_ref(),
        },
    )?;

    if !run_out.stderr.is_empty() {
        let _ = std::io::Write::write_all(&mut std::io::stderr(), &run_out.stderr);
    }

    let mut run_report_bytes = run_out.stdout;
    if !run_report_bytes.ends_with(b"\n") {
        run_report_bytes.push(b'\n');
    }

    let solve = match serde_json::from_slice::<serde_json::Value>(&run_report_bytes) {
        Ok(run_report_json) => {
            if run_report_json
                .get("schema_version")
                .and_then(|v| v.as_str())
                != Some(X07_OS_RUNNER_REPORT_SCHEMA_VERSION)
            {
                synthesize_vm_solve_failure_runner_result(
                    run_out.exit_status,
                    &run_report_bytes,
                    &run_out.stderr,
                    "runner report schema_version mismatch".to_string(),
                )
            } else {
                extract_runner_result_from_run_os_report_json(&run_report_json).unwrap_or_else(
                    |err| {
                        synthesize_vm_solve_failure_runner_result(
                            run_out.exit_status,
                            &run_report_bytes,
                            &run_out.stderr,
                            format!("invalid run-os runner report: {err}"),
                        )
                    },
                )
            }
        }
        Err(err) => synthesize_vm_solve_failure_runner_result(
            run_out.exit_status,
            &run_report_bytes,
            &run_out.stderr,
            format!("invalid run-os runner report JSON: {err}"),
        ),
    };

    let compile = build_report_json
        .get("compile")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let solve_ok = solve.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let solve_exit_status = solve
        .get("exit_status")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);

    let exit_code: u8 = if compile_ok && solve_ok && solve_exit_status == 0 {
        0
    } else {
        1
    };

    let combined = serde_json::json!({
        "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
        "mode": mode,
        "world": world.as_str(),
        "exit_code": exit_code,
        "compile": compile,
        "solve": solve,
    });
    println!("{}", serde_json::to_string_pretty(&combined)?);
    Ok(std::process::ExitCode::from(exit_code))
}

fn synthesize_vm_runner_output_failure_report(
    mode: &str,
    world: WorldId,
    exit_status: i32,
    stdout: &[u8],
    stderr: &[u8],
    trap: String,
) -> Result<serde_json::Value> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let compile = serde_json::json!({
        "ok": false,
        "exit_status": exit_status,
        "lang_id": "",
        "native_requires": {
            "schema_version": "x07.native-requires@0.1.0",
            "world": world.as_str(),
            "requires": [],
        },
        "c_source_size": 0,
        "compiled_exe": serde_json::Value::Null,
        "compiled_exe_size": serde_json::Value::Null,
        "compile_error": serde_json::Value::Null,
        "stdout_b64": b64.encode(stdout),
        "stderr_b64": b64.encode(stderr),
        "fuel_used": serde_json::Value::Null,
        "trap": trap,
    });

    Ok(serde_json::json!({
        "schema_version": X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
        "mode": mode,
        "world": world.as_str(),
        "exit_code": 1,
        "compile": compile,
        "solve": serde_json::Value::Null,
    }))
}

fn compiler_json(
    compile: &CompilerResult,
    b64: &base64::engine::general_purpose::GeneralPurpose,
) -> serde_json::Value {
    serde_json::json!({
        "ok": compile.ok,
        "exit_status": compile.exit_status,
        "lang_id": compile.lang_id,
        "native_requires": compile.native_requires,
        "c_source_size": compile.c_source_size,
        "compiled_exe": compile.compiled_exe.as_ref().map(|p| p.display().to_string()),
        "compiled_exe_size": compile.compiled_exe_size,
        "compile_error": compile.compile_error,
        "stdout_b64": b64.encode(&compile.stdout),
        "stderr_b64": b64.encode(&compile.stderr),
        "fuel_used": compile.fuel_used,
        "trap": compile.trap,
    })
}

fn runner_json(
    solve: &RunnerResult,
    b64: &base64::engine::general_purpose::GeneralPurpose,
) -> serde_json::Value {
    serde_json::json!({
        "ok": solve.ok,
        "exit_status": solve.exit_status,
        "solve_output_b64": b64.encode(&solve.solve_output),
        "stdout_b64": b64.encode(&solve.stdout),
        "stderr_b64": b64.encode(&solve.stderr),
        "fuel_used": solve.fuel_used,
        "heap_used": solve.heap_used,
        "fs_read_file_calls": solve.fs_read_file_calls,
        "fs_list_dir_calls": solve.fs_list_dir_calls,
        "rr_open_calls": solve.rr_open_calls,
        "rr_close_calls": solve.rr_close_calls,
        "rr_stats_calls": solve.rr_stats_calls,
        "rr_next_calls": solve.rr_next_calls,
        "rr_next_miss_calls": solve.rr_next_miss_calls,
        "rr_append_calls": solve.rr_append_calls,
        "kv_get_calls": solve.kv_get_calls,
        "kv_set_calls": solve.kv_set_calls,
        "sched_stats": solve.sched_stats,
        "mem_stats": solve.mem_stats,
        "debug_stats": solve.debug_stats,
        "trap": solve.trap,
    })
}

fn compile_runner_config(cli: &Cli, max_output_bytes: usize) -> RunnerConfig {
    RunnerConfig {
        world: x07_worlds::WorldId::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: cli.solve_fuel,
        max_memory_bytes: cli.max_memory_bytes,
        max_output_bytes,
        cpu_time_limit_seconds: cli.cpu_time_limit_seconds,
        debug_borrow_checks: cli.debug_borrow_checks,
    }
}

fn collect_module_roots_for_os(cli: &Cli) -> Result<Vec<PathBuf>> {
    let mut roots = cli.module_root.clone();
    let os_roots = os_paths::default_os_module_roots()?;
    for r in os_roots {
        if !roots.contains(&r) {
            roots.push(r);
        }
    }
    Ok(roots)
}

fn infer_arch_root_from_path(start: &Path) -> Option<PathBuf> {
    let start_dir = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent().map(Path::to_path_buf)?
    };
    let start_dir = std::fs::canonicalize(&start_dir).unwrap_or(start_dir);

    let mut dir: Option<&Path> = Some(start_dir.as_path());
    while let Some(d) = dir {
        if d.join("arch").is_dir() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

fn load_policy(world: WorldId, policy_path: Option<&PathBuf>) -> Result<Option<policy::Policy>> {
    match world {
        WorldId::RunOs => {
            if policy_path.is_some() {
                anyhow::bail!("--policy is only valid for --world run-os-sandboxed");
            }
            Ok(None)
        }
        WorldId::RunOsSandboxed => {
            let policy_path = policy_path.context("run-os-sandboxed requires --policy")?;
            let txt = std::fs::read_to_string(policy_path)
                .with_context(|| format!("read policy: {}", policy_path.display()))?;
            let pol: policy::Policy = serde_json::from_str(&txt)
                .with_context(|| format!("parse policy JSON: {}", policy_path.display()))?;
            pol.validate_basic()
                .map_err(|e| anyhow::anyhow!("invalid policy: {e}"))?;
            Ok(Some(pol))
        }
        _ => anyhow::bail!("internal error: unexpected world enum"),
    }
}

#[cfg(unix)]
#[derive(Debug, Clone)]
struct RunLimits {
    cpu_ms: Option<u64>,
    mem_bytes: Option<u64>,
    fds: Option<u64>,
    procs: Option<u64>,
    core_dumps: bool,
}

#[cfg(unix)]
fn apply_rlimits(limits: &RunLimits) -> std::io::Result<()> {
    unsafe {
        if let Some(cpu_ms) = limits.cpu_ms {
            let secs = cpu_ms.saturating_add(999) / 1000;
            let cpu = libc::rlimit {
                rlim_cur: secs as libc::rlim_t,
                rlim_max: secs as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_CPU, &cpu) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if let Some(mem_bytes) = limits.mem_bytes {
            #[allow(clippy::useless_conversion)]
            let v: libc::rlim_t = mem_bytes as libc::rlim_t;
            let as_limit = libc::rlimit {
                rlim_cur: v,
                rlim_max: v,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &as_limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        #[cfg(target_os = "macos")]
        {
            let _ = limits.mem_bytes;
        }

        if let Some(fds) = limits.fds {
            let v = fds as libc::rlim_t;
            let nofile = libc::rlimit {
                rlim_cur: v,
                rlim_max: v,
            };
            if libc::setrlimit(libc::RLIMIT_NOFILE, &nofile) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }

        if let Some(procs) = limits.procs {
            let v = procs as libc::rlim_t;
            let nproc = libc::rlimit {
                rlim_cur: v,
                rlim_max: v,
            };
            #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
            {
                if libc::setrlimit(libc::RLIMIT_NPROC, &nproc) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
        }

        let core = libc::rlimit {
            rlim_cur: if limits.core_dumps {
                libc::RLIM_INFINITY
            } else {
                0
            },
            rlim_max: if limits.core_dumps {
                libc::RLIM_INFINITY
            } else {
                0
            },
        };
        if libc::setrlimit(libc::RLIMIT_CORE, &core) != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn run_limits_for_cli(cli: &Cli, world: WorldId, policy: Option<&policy::Policy>) -> RunLimits {
    match (world, policy) {
        (WorldId::RunOsSandboxed, Some(pol)) => RunLimits {
            cpu_ms: Some(pol.limits.cpu_ms),
            mem_bytes: Some(pol.limits.mem_bytes),
            fds: Some(pol.limits.fds),
            procs: Some(pol.limits.procs),
            core_dumps: pol.limits.core_dumps.unwrap_or(false),
        },
        _ => RunLimits {
            cpu_ms: Some(cli.cpu_time_limit_seconds.saturating_mul(1000)),
            mem_bytes: None,
            fds: None,
            procs: None,
            core_dumps: false,
        },
    }
}

fn wall_timeout_ms_for_cli(world: WorldId, policy: Option<&policy::Policy>, cli: &Cli) -> u64 {
    match (world, policy) {
        (WorldId::RunOsSandboxed, Some(pol)) => pol.limits.wall_ms,
        _ => cli
            .cpu_time_limit_seconds
            .saturating_add(1)
            .saturating_mul(1000),
    }
}

#[derive(Debug)]
struct ChildOutput {
    exit_status: i32,
    exit_signal: Option<i32>,
    timed_out: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

struct RunInvocation<'a> {
    artifact: &'a Path,
    world: WorldId,
    policy: Option<&'a policy::Policy>,
    input: &'a [u8],
    max_output_bytes: usize,
    #[cfg(unix)]
    limits: &'a RunLimits,
    wall_ms: u64,
    run_dir: Option<&'a Path>,
}

fn wait_child_with_wall_timeout_ms(
    child: &mut std::process::Child,
    wall_ms: u64,
) -> Result<(std::process::ExitStatus, bool)> {
    let wall_limit = Duration::from_millis(wall_ms.max(1));
    let start = Instant::now();
    let deadline = start.checked_add(wall_limit);

    loop {
        if let Some(status) = child.try_wait().context("try_wait child")? {
            return Ok((status, false));
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            let _ = child.kill();
            let status = child.wait().context("wait child after kill")?;
            return Ok((status, true));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn run_child(inv: &RunInvocation<'_>) -> Result<ChildOutput> {
    let artifact_abs = std::fs::canonicalize(inv.artifact)
        .with_context(|| format!("canonicalize artifact path: {}", inv.artifact.display()))?;

    let mut cmd = Command::new(&artifact_abs);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.env("X07_WORLD", inv.world.as_str());
    if let Some(dir) = inv.run_dir {
        cmd.current_dir(dir);
    }

    if inv.world == WorldId::RunOsSandboxed {
        cmd.env("X07_OS_SANDBOXED", "1");
    }

    if let Some(pol) = inv.policy {
        for (k, v) in os_env::policy_to_env(pol) {
            cmd.env(k, v);
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        let limits = inv.limits.clone();
        unsafe {
            cmd.pre_exec(move || apply_rlimits(&limits));
        }
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawn artifact: {}", artifact_abs.display()))?;

    let mut stdin = child.stdin.take().context("take stdin")?;
    let stdout = child.stdout.take().context("take stdout")?;
    let stderr = child.stderr.take().context("take stderr")?;

    let input_vec = x07_host_runner::encode_len_prefixed(inv.input);
    let stdin_thread = std::thread::spawn(move || -> std::io::Result<()> {
        use std::io::Write;
        stdin.write_all(&input_vec)?;
        stdin.flush()?;
        drop(stdin);
        Ok(())
    });

    let stdout_cap = 4usize
        .saturating_add(inv.max_output_bytes)
        .saturating_add(1);
    let stdout_thread = std::thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
        x07_host_runner::read_to_end_capped(stdout, stdout_cap)
    });

    let stderr_cap = 256usize * 1024;
    let stderr_thread = std::thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
        x07_host_runner::read_to_end_capped(stderr, stderr_cap)
    });

    let (status, timed_out) = wait_child_with_wall_timeout_ms(&mut child, inv.wall_ms)?;
    let _ = stdin_thread.join();
    let (stdout_bytes, stdout_truncated) = stdout_thread
        .join()
        .unwrap_or_else(|_| Ok((Vec::new(), false)))?;
    let (stderr_bytes, stderr_truncated) = stderr_thread
        .join()
        .unwrap_or_else(|_| Ok((Vec::new(), false)))?;

    #[cfg(unix)]
    let exit_signal = {
        use std::os::unix::process::ExitStatusExt as _;
        status.signal()
    };
    #[cfg(not(unix))]
    let exit_signal: Option<i32> = None;

    let exit_status = match status.code() {
        Some(code) => code,
        None => exit_signal.map(|s| 128 + s).unwrap_or(1),
    };

    Ok(ChildOutput {
        exit_status,
        exit_signal,
        timed_out,
        stdout: stdout_bytes,
        stderr: stderr_bytes,
        stdout_truncated,
        stderr_truncated,
    })
}

fn run_os_artifact(inv: &RunInvocation<'_>) -> Result<RunnerResult> {
    let out = run_child(inv)?;

    if out.timed_out {
        return Ok(RunnerResult {
            ok: false,
            exit_status: out.exit_status,
            solve_output: Vec::new(),
            stdout: out.stdout,
            stderr: out.stderr,
            fuel_used: None,
            heap_used: None,
            fs_read_file_calls: None,
            fs_list_dir_calls: None,
            rr_open_calls: None,
            rr_close_calls: None,
            rr_stats_calls: None,
            rr_next_calls: None,
            rr_next_miss_calls: None,
            rr_append_calls: None,
            kv_get_calls: None,
            kv_set_calls: None,
            sched_stats: None,
            mem_stats: None,
            debug_stats: None,
            trap: Some("timed out".to_string()),
        });
    }

    if out.stderr_truncated {
        return Ok(RunnerResult {
            ok: false,
            exit_status: out.exit_status,
            solve_output: Vec::new(),
            stdout: out.stdout,
            stderr: out.stderr,
            fuel_used: None,
            heap_used: None,
            fs_read_file_calls: None,
            fs_list_dir_calls: None,
            rr_open_calls: None,
            rr_close_calls: None,
            rr_stats_calls: None,
            rr_next_calls: None,
            rr_next_miss_calls: None,
            rr_append_calls: None,
            kv_get_calls: None,
            kv_set_calls: None,
            sched_stats: None,
            mem_stats: None,
            debug_stats: None,
            trap: Some("stderr exceeded cap".to_string()),
        });
    }

    if out.stdout_truncated {
        return Ok(RunnerResult {
            ok: false,
            exit_status: out.exit_status,
            solve_output: Vec::new(),
            stdout: out.stdout,
            stderr: out.stderr,
            fuel_used: None,
            heap_used: None,
            fs_read_file_calls: None,
            fs_list_dir_calls: None,
            rr_open_calls: None,
            rr_close_calls: None,
            rr_stats_calls: None,
            rr_next_calls: None,
            rr_next_miss_calls: None,
            rr_append_calls: None,
            kv_get_calls: None,
            kv_set_calls: None,
            sched_stats: None,
            mem_stats: None,
            debug_stats: None,
            trap: Some("stdout exceeded cap".to_string()),
        });
    }

    let parse = x07_host_runner::parse_native_stdout(&out.stdout, inv.max_output_bytes);
    let (solve_output, mut trap) = match parse {
        Ok(bytes) => (
            bytes,
            out.exit_signal.map(|s| format!("terminated by signal {s}")),
        ),
        Err(err) => (
            Vec::new(),
            out.exit_signal
                .map(|s| format!("terminated by signal {s}"))
                .or_else(|| Some(err.to_string())),
        ),
    };

    let metrics = x07_host_runner::parse_metrics(&out.stderr);
    if out.exit_status == 0 && metrics.is_none() && trap.is_none() {
        trap = Some("missing metrics json line on stderr".to_string());
    }

    if out.exit_status != 0 || out.exit_signal.is_some() {
        if let Some(msg) = x07_host_runner::parse_trap_stderr(&out.stderr) {
            trap = Some(msg);
        }
    }

    let fuel_used = metrics.as_ref().and_then(|m| m.fuel_used);
    let heap_used = metrics.as_ref().and_then(|m| m.heap_used);
    let fs_read_file_calls = metrics.as_ref().and_then(|m| m.fs_read_file_calls);
    let fs_list_dir_calls = metrics.as_ref().and_then(|m| m.fs_list_dir_calls);
    let rr_open_calls = metrics.as_ref().and_then(|m| m.rr_open_calls);
    let rr_close_calls = metrics.as_ref().and_then(|m| m.rr_close_calls);
    let rr_stats_calls = metrics.as_ref().and_then(|m| m.rr_stats_calls);
    let rr_next_calls = metrics.as_ref().and_then(|m| m.rr_next_calls);
    let rr_next_miss_calls = metrics.as_ref().and_then(|m| m.rr_next_miss_calls);
    let rr_append_calls = metrics.as_ref().and_then(|m| m.rr_append_calls);
    let kv_get_calls = metrics.as_ref().and_then(|m| m.kv_get_calls);
    let kv_set_calls = metrics.as_ref().and_then(|m| m.kv_set_calls);
    let sched_stats = metrics.as_ref().and_then(|m| m.sched_stats.clone());
    let mem_stats = metrics.as_ref().and_then(|m| m.mem_stats);
    let debug_stats = metrics.as_ref().and_then(|m| m.debug_stats);

    let ok = out.exit_status == 0 && trap.is_none();
    Ok(RunnerResult {
        ok,
        exit_status: out.exit_status,
        solve_output,
        stdout: out.stdout,
        stderr: out.stderr,
        fuel_used,
        heap_used,
        fs_read_file_calls,
        fs_list_dir_calls,
        rr_open_calls,
        rr_close_calls,
        rr_stats_calls,
        rr_next_calls,
        rr_next_miss_calls,
        rr_append_calls,
        kv_get_calls,
        kv_set_calls,
        sched_stats,
        mem_stats,
        debug_stats,
        trap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, Once};

    static OS_HELPERS_READY: Once = Once::new();
    static OS_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn os_test_lock() -> MutexGuard<'static, ()> {
        OS_TEST_LOCK
            .lock()
            .unwrap_or_else(|_| panic!("failed to lock OS test mutex"))
    }

    fn workspace_root() -> PathBuf {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        crate_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn ensure_os_helpers_installed() {
        OS_HELPERS_READY.call_once(|| {
            let root = workspace_root();
            let deps_dir = root.join("deps").join("x07");
            std::fs::create_dir_all(&deps_dir).unwrap_or_else(|e| panic!("create deps/x07: {e}"));

            let target_dir = root.join("target").join("os-helpers");
            build_and_install_helper(&root, &target_dir, &deps_dir, "x07-proc-echo");
            build_and_install_helper(&root, &target_dir, &deps_dir, "x07-proc-worker-frame-echo");
        });
    }

    fn build_and_install_helper(root: &Path, target_dir: &Path, deps_dir: &Path, name: &str) {
        let out = Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", target_dir)
            .args(["build", "-p", name, "--release"])
            .output()
            .unwrap_or_else(|e| panic!("cargo build -p {name}: {e}"));
        if !out.status.success() {
            panic!(
                "cargo build -p {name} failed: {}\nstdout:\n{}\nstderr:\n{}",
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr),
            );
        }

        let src_dir = target_dir.join("release");
        let src_exe = src_dir.join(format!("{name}.exe"));
        let src = if src_exe.is_file() {
            src_exe
        } else {
            src_dir.join(name)
        };
        assert!(src.is_file(), "missing helper binary: {}", src.display());

        let dst = deps_dir.join(name);
        if dst.exists() {
            std::fs::remove_file(&dst).unwrap_or_else(|e| panic!("remove {}: {e}", dst.display()));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs as unix_fs;

            let rel_src = src
                .strip_prefix(root)
                .map(|p| PathBuf::from("..").join("..").join(p))
                .unwrap_or_else(|_| src.clone());

            unix_fs::symlink(&rel_src, &dst).unwrap_or_else(|e| {
                panic!(
                    "symlink helper {} -> {}: {e}",
                    rel_src.display(),
                    dst.display()
                )
            });
        }

        #[cfg(not(unix))]
        {
            std::fs::copy(&src, &dst).unwrap_or_else(|e| {
                panic!("copy helper {} -> {}: {e}", src.display(), dst.display())
            });
        }
    }

    fn read_u32_le(b: &[u8], off: usize) -> u32 {
        u32::from_le_bytes(
            b.get(off..off + 4)
                .expect("u32 range")
                .try_into()
                .expect("u32 bytes"),
        )
    }

    fn assert_proc_ok_doc(doc: &[u8]) {
        if doc.len() >= 9 && doc[0] == 0 {
            let code = read_u32_le(doc, 1);
            panic!("expected ok doc, got err code={code} bytes={doc:?}");
        }
    }

    fn base_runner_config(max_output_bytes: usize) -> RunnerConfig {
        RunnerConfig {
            world: x07_worlds::WorldId::SolvePure,
            fixture_fs_dir: None,
            fixture_fs_root: None,
            fixture_fs_latency_index: None,
            fixture_rr_dir: None,
            fixture_kv_dir: None,
            fixture_kv_seed: None,
            solve_fuel: 10_000_000,
            max_memory_bytes: 64 * 1024 * 1024,
            max_output_bytes,
            cpu_time_limit_seconds: 5,
            debug_borrow_checks: false,
        }
    }

    fn assert_compile_ok(compile: &CompilerResult) {
        if compile.ok {
            return;
        }

        let stdout = String::from_utf8_lossy(&compile.stdout);
        let stderr = String::from_utf8_lossy(&compile.stderr);
        panic!(
            "compile failed: {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            compile.compile_error
        );
    }

    fn compile_process_smoke_program(world: WorldId) -> CompilerResult {
        compile_external_os_program(world, "tests/external_os/process/src/main.x07.json")
    }

    fn compile_external_os_program(world: WorldId, rel_path: &str) -> CompilerResult {
        compile_external_os_program_with_extra_roots(world, rel_path, &[])
    }

    fn compile_external_os_program_with_extra_roots(
        world: WorldId,
        rel_path: &str,
        extra_roots: &[PathBuf],
    ) -> CompilerResult {
        let root = workspace_root();
        let program_path = root.join(rel_path);
        let program = std::fs::read(&program_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", program_path.display()));

        let mut module_roots = os_paths::default_os_module_roots().expect("stdlib/os module roots");
        for r in extra_roots {
            if !module_roots.contains(r) {
                module_roots.push(r.clone());
            }
        }
        let compile_options = compile::CompileOptions {
            world,
            enable_fs: false,
            enable_rr: false,
            enable_kv: false,
            module_roots,
            arch_root: None,
            emit_main: true,
            freestanding: false,
            contract_mode: compile::ContractMode::RuntimeTrap,
            allow_unsafe: None,
            allow_ffi: None,
        };

        let cfg = base_runner_config(1024 * 1024);
        compile_program_with_options(&program, &cfg, None, &compile_options, &[])
            .expect("compile_program_with_options")
    }

    fn run_compiled_program(
        world: WorldId,
        compile: &CompilerResult,
        policy: Option<&policy::Policy>,
        input: &[u8],
        wall_ms: u64,
    ) -> RunnerResult {
        ensure_os_helpers_installed();

        let root = workspace_root();
        assert!(compile.ok);
        let exe = compile.compiled_exe.as_ref().expect("compiled exe");

        #[cfg(unix)]
        let limits = RunLimits {
            cpu_ms: Some(5_000),
            mem_bytes: None,
            fds: None,
            procs: None,
            core_dumps: false,
        };

        let inv = RunInvocation {
            artifact: exe,
            world,
            policy,
            input,
            max_output_bytes: 1024 * 1024,
            #[cfg(unix)]
            limits: &limits,
            wall_ms,
            run_dir: Some(root.as_path()),
        };

        run_os_artifact(&inv).expect("run_os_artifact")
    }

    fn process_defaults() -> policy::Process {
        policy::Process {
            enabled: false,
            allow_spawn: false,
            max_live: 0,
            max_spawns: 0,
            allow_execs: Vec::new(),
            allow_exec_prefixes: Vec::new(),
            allow_args_regex_lite: Vec::new(),
            allow_env_keys: Vec::new(),
            allow_exec: false,
            allow_exit: false,
            max_exe_bytes: 4096,
            max_args: 64,
            max_arg_bytes: 4096,
            max_env: 64,
            max_env_key_bytes: 256,
            max_env_val_bytes: 4096,
            max_runtime_ms: 1000,
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
            max_total_bytes: 2 * 1024 * 1024,
            max_stdin_bytes: 1024 * 1024,
            kill_on_drop: true,
            kill_tree: true,
            allow_cwd: false,
            allow_cwd_roots: Vec::new(),
        }
    }

    fn policy_base(process: policy::Process) -> policy::Policy {
        policy::Policy {
            schema_version: RUN_OS_POLICY_SCHEMA_VERSION.to_string(),
            policy_id: "test".to_string(),
            limits: policy::Limits {
                cpu_ms: 5_000,
                wall_ms: 6_000,
                mem_bytes: 64 * 1024 * 1024,
                fds: 64,
                procs: 64,
                core_dumps: None,
            },
            fs: policy::Fs {
                enabled: false,
                read_roots: Vec::new(),
                write_roots: Vec::new(),
                deny_hidden: true,
                allow_symlinks: false,
                allow_mkdir: false,
                allow_remove: false,
                allow_rename: false,
                allow_walk: false,
                allow_glob: false,
                max_read_bytes: 0,
                max_write_bytes: 0,
                max_entries: 0,
                max_depth: 0,
            },
            net: policy::Net {
                enabled: false,
                allow_dns: false,
                allow_tcp: false,
                allow_udp: false,
                allow_hosts: Vec::new(),
            },
            db: Default::default(),
            env: policy::Env {
                enabled: false,
                allow_keys: Vec::new(),
                deny_keys: Vec::new(),
            },
            time: policy::Time {
                enabled: false,
                allow_monotonic: false,
                allow_wall_clock: false,
                allow_sleep: false,
                max_sleep_ms: 0,
                allow_local_tzid: false,
            },
            language: Default::default(),
            threads: Default::default(),
            process,
        }
    }

    #[test]
    fn run_capture_run_os_echoes_input() {
        let _lock = os_test_lock();
        let compile = compile_process_smoke_program(WorldId::RunOs);
        assert_compile_ok(&compile);

        let input = b"abc";
        let res = run_compiled_program(WorldId::RunOs, &compile, None, input, 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert_proc_ok_doc(&doc);
        assert!(doc.len() >= 18, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 1, "expected ok doc, got tag={}", doc[0]);
        assert_eq!(doc[1], 1, "expected ProcRespV1 ver=1, got {}", doc[1]);
        assert_eq!(read_u32_le(&doc, 2), 0, "exit_code != 0");
        assert_eq!(read_u32_le(&doc, 6), 0, "flags != 0");
        let stdout_len = read_u32_le(&doc, 10) as usize;
        assert!(doc.len() >= 18 + stdout_len, "doc too short for stdout");
        assert_eq!(&doc[14..14 + stdout_len], input);
        let stderr_len_off = 14 + stdout_len;
        let stderr_len = read_u32_le(&doc, stderr_len_off) as usize;
        assert_eq!(stderr_len, 0);
    }

    #[test]
    fn run_capture_run_os_large_stdout_no_deadlock() {
        let _lock = os_test_lock();
        let compile = compile_process_smoke_program(WorldId::RunOs);
        assert_compile_ok(&compile);

        let input = vec![b'a'; 1024 * 1024 - 18];
        let res = run_compiled_program(WorldId::RunOs, &compile, None, &input, 10_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert_proc_ok_doc(&doc);
        assert_eq!(doc[0], 1, "expected ok doc, got tag={}", doc[0]);
        assert_eq!(doc[1], 1, "expected ProcRespV1 ver=1, got {}", doc[1]);
        assert_eq!(read_u32_le(&doc, 2), 0, "exit_code != 0");
        assert_eq!(read_u32_le(&doc, 6), 0, "flags != 0");
        let stdout_len = read_u32_le(&doc, 10) as usize;
        assert_eq!(stdout_len, input.len());
        assert!(doc.len() >= 18 + stdout_len, "doc too short for stdout");
        assert_eq!(&doc[14..14 + stdout_len], &input);
        let stderr_len_off = 14 + stdout_len;
        let stderr_len = read_u32_le(&doc, stderr_len_off) as usize;
        assert_eq!(stderr_len, 0);
    }

    #[test]
    fn run_capture_run_os_sandboxed_denied_when_spawn_disabled() {
        let _lock = os_test_lock();
        let compile = compile_process_smoke_program(WorldId::RunOsSandboxed);
        assert_compile_ok(&compile);

        let pol = policy_base(policy::Process {
            enabled: true,
            allow_spawn: false,
            ..process_defaults()
        });
        pol.validate_basic().expect("policy validate");

        let res =
            run_compiled_program(WorldId::RunOsSandboxed, &compile, Some(&pol), b"abc", 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert!(doc.len() >= 9, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 0, "expected err doc tag=0");
        assert_eq!(read_u32_le(&doc, 1), 1, "expected POLICY_DENIED");
    }

    #[test]
    fn run_capture_run_os_output_limit() {
        let _lock = os_test_lock();
        let compile = compile_external_os_program(
            WorldId::RunOs,
            "tests/external_os/process_output_limit/src/main.x07.json",
        );
        assert_compile_ok(&compile);

        let input = vec![b'a'; 1024 * 1024];
        let res = run_compiled_program(WorldId::RunOs, &compile, None, &input, 10_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert!(doc.len() >= 9, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 0, "expected err doc tag=0");
        assert_eq!(read_u32_le(&doc, 1), 5, "expected OUTPUT_LIMIT");
    }

    #[test]
    fn run_os_process_pool_maps_ok() {
        let _lock = os_test_lock();
        let compile = compile_external_os_program(
            WorldId::RunOs,
            "tests/external_os/process_pool/src/main.x07.json",
        );
        assert_compile_ok(&compile);

        let input = vec![b'x'; 70_000];
        let res = run_compiled_program(WorldId::RunOs, &compile, None, &input, 10_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);
        assert_eq!(res.solve_output, b"ok");
    }

    #[test]
    fn run_os_sandboxed_process_pool_allow_exec_prefix() {
        let _lock = os_test_lock();
        let compile = compile_external_os_program(
            WorldId::RunOsSandboxed,
            "tests/external_os/process_pool/src/main.x07.json",
        );
        assert_compile_ok(&compile);

        let pol = policy_base(policy::Process {
            enabled: true,
            allow_spawn: true,
            max_live: 10,
            max_spawns: 10,
            allow_exec_prefixes: vec!["deps/x07/".to_string()],
            ..process_defaults()
        });
        pol.validate_basic().expect("policy validate");

        let res = run_compiled_program(WorldId::RunOsSandboxed, &compile, Some(&pol), b"", 10_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);
        assert_eq!(res.solve_output, b"ok");
    }

    #[test]
    fn run_capture_run_os_sandboxed_denied_when_exec_not_allowlisted() {
        let _lock = os_test_lock();
        let compile = compile_process_smoke_program(WorldId::RunOsSandboxed);
        assert_compile_ok(&compile);

        let pol = policy_base(policy::Process {
            enabled: true,
            allow_spawn: true,
            max_live: 10,
            max_spawns: 10,
            allow_execs: vec!["/bin/false".to_string()],
            ..process_defaults()
        });
        pol.validate_basic().expect("policy validate");

        let res =
            run_compiled_program(WorldId::RunOsSandboxed, &compile, Some(&pol), b"abc", 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert!(doc.len() >= 9, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 0, "expected err doc tag=0");
        assert_eq!(read_u32_le(&doc, 1), 1, "expected POLICY_DENIED");
    }

    #[test]
    fn run_capture_run_os_sandboxed_allows_allowlisted_exec() {
        let _lock = os_test_lock();
        let compile = compile_process_smoke_program(WorldId::RunOsSandboxed);
        assert_compile_ok(&compile);

        let pol = policy_base(policy::Process {
            enabled: true,
            allow_spawn: true,
            max_live: 10,
            max_spawns: 10,
            allow_execs: vec!["deps/x07/x07-proc-echo".to_string()],
            ..process_defaults()
        });
        pol.validate_basic().expect("policy validate");

        let input = b"abc";
        let res = run_compiled_program(WorldId::RunOsSandboxed, &compile, Some(&pol), input, 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert_proc_ok_doc(&doc);
        assert!(doc.len() >= 18, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 1, "expected ok doc, got tag={}", doc[0]);
        assert_eq!(doc[1], 1, "expected ProcRespV1 ver=1, got {}", doc[1]);
        assert_eq!(read_u32_le(&doc, 2), 0, "exit_code != 0");
        let stdout_len = read_u32_le(&doc, 10) as usize;
        assert!(doc.len() >= 18 + stdout_len, "doc too short for stdout");
        assert_eq!(&doc[14..14 + stdout_len], input);
    }

    #[test]
    fn run_capture_run_os_sandboxed_denied_when_caps_exceed_policy() {
        let _lock = os_test_lock();
        let compile = compile_process_smoke_program(WorldId::RunOsSandboxed);
        assert_compile_ok(&compile);

        let pol = policy_base(policy::Process {
            enabled: true,
            allow_spawn: true,
            max_live: 10,
            max_spawns: 10,
            allow_execs: vec!["deps/x07/x07-proc-echo".to_string()],
            max_stdout_bytes: 1024,
            ..process_defaults()
        });
        pol.validate_basic().expect("policy validate");

        let res =
            run_compiled_program(WorldId::RunOsSandboxed, &compile, Some(&pol), b"abc", 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert!(doc.len() >= 9, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 0, "expected err doc tag=0");
        assert_eq!(read_u32_le(&doc, 1), 1, "expected POLICY_DENIED");
    }

    #[test]
    fn run_capture_run_os_sandboxed_denied_when_stdin_exceeds_policy() {
        let _lock = os_test_lock();
        let compile = compile_process_smoke_program(WorldId::RunOsSandboxed);
        assert_compile_ok(&compile);

        let pol = policy_base(policy::Process {
            enabled: true,
            allow_spawn: true,
            max_live: 10,
            max_spawns: 10,
            allow_execs: vec!["deps/x07/x07-proc-echo".to_string()],
            max_stdin_bytes: 2,
            ..process_defaults()
        });
        pol.validate_basic().expect("policy validate");

        let res = run_compiled_program(
            WorldId::RunOsSandboxed,
            &compile,
            Some(&pol),
            b"abcd",
            5_000,
        );
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert!(doc.len() >= 9, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 0, "expected err doc tag=0");
        assert_eq!(read_u32_le(&doc, 1), 1, "expected POLICY_DENIED");
    }

    #[test]
    fn spawn_join_run_os_echoes_input() {
        let _lock = os_test_lock();
        let compile = compile_external_os_program(
            WorldId::RunOs,
            "tests/external_os/process_spawn/src/main.x07.json",
        );
        assert_compile_ok(&compile);

        let input = b"abc";
        let res = run_compiled_program(WorldId::RunOs, &compile, None, input, 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert_proc_ok_doc(&doc);
        assert!(doc.len() >= 18, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 1, "expected ok doc, got tag={}", doc[0]);
        assert_eq!(doc[1], 1, "expected ProcRespV1 ver=1, got {}", doc[1]);
        assert_eq!(read_u32_le(&doc, 2), 0, "exit_code != 0");
        assert_eq!(read_u32_le(&doc, 6), 0, "flags != 0");
        let stdout_len = read_u32_le(&doc, 10) as usize;
        assert!(doc.len() >= 18 + stdout_len, "doc too short for stdout");
        assert_eq!(&doc[14..14 + stdout_len], input);
        let stderr_len_off = 14 + stdout_len;
        let stderr_len = read_u32_le(&doc, stderr_len_off) as usize;
        assert_eq!(stderr_len, 0);
    }

    #[test]
    fn spawn_join_async_task_run_os_echoes_input() {
        let _lock = os_test_lock();
        let compile = compile_external_os_program(
            WorldId::RunOs,
            "tests/external_os/process_spawn_async_join/src/main.x07.json",
        );
        assert_compile_ok(&compile);

        let input = b"abc";
        let res = run_compiled_program(WorldId::RunOs, &compile, None, input, 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert_proc_ok_doc(&doc);
        assert!(doc.len() >= 18, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 1, "expected ok doc, got tag={}", doc[0]);
        assert_eq!(doc[1], 1, "expected ProcRespV1 ver=1, got {}", doc[1]);
        assert_eq!(read_u32_le(&doc, 2), 0, "exit_code != 0");
        assert_eq!(read_u32_le(&doc, 6), 0, "flags != 0");
        let stdout_len = read_u32_le(&doc, 10) as usize;
        assert!(doc.len() >= 18 + stdout_len, "doc too short for stdout");
        assert_eq!(&doc[14..14 + stdout_len], input);
        let stderr_len_off = 14 + stdout_len;
        let stderr_len = read_u32_le(&doc, stderr_len_off) as usize;
        assert_eq!(stderr_len, 0);
    }

    #[test]
    fn spawn_join_run_os_sandboxed_allows_allowlisted_exec() {
        let _lock = os_test_lock();
        let compile = compile_external_os_program(
            WorldId::RunOsSandboxed,
            "tests/external_os/process_spawn/src/main.x07.json",
        );
        assert_compile_ok(&compile);

        let pol = policy_base(policy::Process {
            enabled: true,
            allow_spawn: true,
            max_live: 10,
            max_spawns: 10,
            allow_execs: vec!["deps/x07/x07-proc-echo".to_string()],
            ..process_defaults()
        });
        pol.validate_basic().expect("policy validate");

        let input = b"abc";
        let res = run_compiled_program(WorldId::RunOsSandboxed, &compile, Some(&pol), input, 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert_proc_ok_doc(&doc);
        assert!(doc.len() >= 18, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 1, "expected ok doc, got tag={}", doc[0]);
        assert_eq!(doc[1], 1, "expected ProcRespV1 ver=1, got {}", doc[1]);
        assert_eq!(read_u32_le(&doc, 2), 0, "exit_code != 0");
        let stdout_len = read_u32_le(&doc, 10) as usize;
        assert!(doc.len() >= 18 + stdout_len, "doc too short for stdout");
        assert_eq!(&doc[14..14 + stdout_len], input);
    }

    #[test]
    fn spawn_join_run_os_sandboxed_enforces_max_live() {
        let _lock = os_test_lock();
        let compile = compile_external_os_program(
            WorldId::RunOsSandboxed,
            "tests/external_os/process_spawn_max_live/src/main.x07.json",
        );
        assert_compile_ok(&compile);

        let pol = policy_base(policy::Process {
            enabled: true,
            allow_spawn: true,
            max_live: 1,
            max_spawns: 10,
            allow_execs: vec!["deps/x07/x07-proc-echo".to_string()],
            ..process_defaults()
        });
        pol.validate_basic().expect("policy validate");

        let res =
            run_compiled_program(WorldId::RunOsSandboxed, &compile, Some(&pol), b"abc", 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert!(doc.len() >= 9, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 0, "expected err doc tag=0");
        assert_eq!(read_u32_le(&doc, 1), 1, "expected POLICY_DENIED");
    }

    #[test]
    fn spawn_join_run_os_sandboxed_enforces_max_spawns() {
        let _lock = os_test_lock();
        let compile = compile_external_os_program(
            WorldId::RunOsSandboxed,
            "tests/external_os/process_spawn_max_live/src/main.x07.json",
        );
        assert_compile_ok(&compile);

        let pol = policy_base(policy::Process {
            enabled: true,
            allow_spawn: true,
            max_live: 10,
            max_spawns: 1,
            allow_execs: vec!["deps/x07/x07-proc-echo".to_string()],
            ..process_defaults()
        });
        pol.validate_basic().expect("policy validate");

        let res =
            run_compiled_program(WorldId::RunOsSandboxed, &compile, Some(&pol), b"abc", 5_000);
        assert!(res.ok, "trap={:?} stderr={:?}", res.trap, res.stderr);
        assert_eq!(res.exit_status, 0);

        let doc = res.solve_output;
        assert!(doc.len() >= 9, "doc too short: {}", doc.len());
        assert_eq!(doc[0], 0, "expected err doc tag=0");
        assert_eq!(read_u32_le(&doc, 1), 1, "expected POLICY_DENIED");
    }
}
