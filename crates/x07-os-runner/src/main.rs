use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
use x07_runner_common::{auto_ffi, os_env, os_paths};
use x07_worlds::WorldId;

#[cfg(test)]
use x07c::compile;
use x07c::project;

mod policy;
mod sandbox;

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

    #[arg(long)]
    module_root: Vec<PathBuf>,

    #[arg(long)]
    auto_ffi: bool,
}

fn main() -> std::process::ExitCode {
    // Windows defaults to a 1MiB stack, which is not enough for our current compiler recursion
    // depth (for example, larger template and example projects). Run the real entrypoint on a
    // larger-stack thread to keep behavior consistent across platforms.
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

    let policy = load_policy(world, cli.policy.as_ref())?;
    if let Some(ref pol) = policy {
        sandbox::apply_sandbox(pol).map_err(|e| anyhow::anyhow!("{e}"))?;
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

    match (&cli.artifact, &cli.program, &cli.project) {
        (Some(_), Some(_), _)
        | (Some(_), _, Some(_))
        | (_, Some(_), Some(_))
        | (None, None, None) => {
            anyhow::bail!("set exactly one of --artifact, --program, or --project")
        }

        (Some(artifact), None, None) => {
            if world == WorldId::RunOsSandboxed {
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
                "rr_send_calls": solve.rr_send_calls,
                "rr_request_calls": solve.rr_request_calls,
                "rr_last_request_sha256": solve.rr_last_request_sha256,
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
        "rr_send_calls": solve.rr_send_calls,
        "rr_request_calls": solve.rr_request_calls,
        "rr_last_request_sha256": solve.rr_last_request_sha256,
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
        fixture_rr_index: None,
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
            rr_send_calls: None,
            rr_request_calls: None,
            rr_last_request_sha256: None,
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
            rr_send_calls: None,
            rr_request_calls: None,
            rr_last_request_sha256: None,
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
            rr_send_calls: None,
            rr_request_calls: None,
            rr_last_request_sha256: None,
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
    let rr_send_calls = metrics.as_ref().and_then(|m| m.rr_send_calls);
    let rr_request_calls = metrics.as_ref().and_then(|m| m.rr_request_calls);
    let rr_last_request_sha256 = metrics
        .as_ref()
        .and_then(|m| m.rr_last_request_sha256.clone());
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
        rr_send_calls,
        rr_request_calls,
        rr_last_request_sha256,
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
        std::fs::copy(&src, &dst)
            .unwrap_or_else(|e| panic!("copy helper {} -> {}: {e}", src.display(), dst.display()));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mut perms = std::fs::metadata(&dst)
                .unwrap_or_else(|e| panic!("stat {}: {e}", dst.display()))
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dst, perms)
                .unwrap_or_else(|e| panic!("chmod {}: {e}", dst.display()));
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
            fixture_rr_index: None,
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
        let root = workspace_root();
        let program_path = root.join(rel_path);
        let program = std::fs::read(&program_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", program_path.display()));

        let module_roots = os_paths::default_os_module_roots().expect("stdlib/os module roots");
        let compile_options = compile::CompileOptions {
            world,
            enable_fs: false,
            enable_rr: false,
            enable_kv: false,
            module_roots,
            emit_main: true,
            freestanding: false,
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
