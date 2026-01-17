use std::collections::BTreeMap;
use std::ffi::OsStr;
#[cfg(windows)]
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use base64::Engine as _;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use x07_worlds::WorldId;
use x07c::compile;
use x07c::language;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum CcProfile {
    Default,
    Size,
}

const CC_PROFILE_SIZE_MACOS: &[&str] = &["-Os", "-Wl,-dead_strip", "-Wl,-x"];
const CC_PROFILE_SIZE_LINUX: &[&str] = &[
    "-Os",
    "-ffunction-sections",
    "-fdata-sections",
    "-Wl,--gc-sections",
    "-Wl,--strip-all",
];
const CC_PROFILE_SIZE_FALLBACK: &[&str] = &["-Os"];

pub fn apply_cc_profile(profile: CcProfile) {
    let flags = cc_profile_flags(profile);
    if flags.is_empty() {
        return;
    }

    let existing = std::env::var("X07_CC_ARGS").unwrap_or_default();
    let merged = merge_cc_args(&existing, flags);
    if merged.trim().is_empty() {
        return;
    }
    std::env::set_var("X07_CC_ARGS", merged);
}

fn cc_profile_flags(profile: CcProfile) -> &'static [&'static str] {
    match profile {
        CcProfile::Default => &[],
        CcProfile::Size => {
            if cfg!(target_os = "macos") {
                CC_PROFILE_SIZE_MACOS
            } else if cfg!(target_os = "linux") {
                CC_PROFILE_SIZE_LINUX
            } else {
                CC_PROFILE_SIZE_FALLBACK
            }
        }
    }
}

fn merge_cc_args(existing: &str, flags: &[&str]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for tok in existing.split_whitespace() {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        if seen.insert(t.to_string()) {
            out.push(t.to_string());
        }
    }

    for &tok in flags {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        if seen.insert(t.to_string()) {
            out.push(t.to_string());
        }
    }

    out.join(" ")
}

#[derive(Debug, Clone)]
pub struct RunnerConfig {
    /// Deterministic evaluation worlds only (`solve-*`).
    pub world: WorldId,
    pub fixture_fs_dir: Option<PathBuf>,
    pub fixture_fs_root: Option<PathBuf>,
    pub fixture_fs_latency_index: Option<PathBuf>,
    pub fixture_rr_dir: Option<PathBuf>,
    pub fixture_rr_index: Option<PathBuf>,
    pub fixture_kv_dir: Option<PathBuf>,
    pub fixture_kv_seed: Option<PathBuf>,
    pub solve_fuel: u64,
    pub max_memory_bytes: usize,
    pub max_output_bytes: usize,
    pub cpu_time_limit_seconds: u64,
    pub debug_borrow_checks: bool,
}

#[derive(Debug, Clone)]
pub struct CompilerResult {
    pub ok: bool,
    pub exit_status: i32,
    pub lang_id: String,
    pub guide_md: String,
    pub c_source_size: usize,
    pub compiled_exe: Option<PathBuf>,
    pub compiled_exe_size: Option<u64>,
    pub compile_error: Option<String>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub fuel_used: Option<u64>,
    pub trap: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunnerResult {
    pub ok: bool,
    pub exit_status: i32,
    pub solve_output: Vec<u8>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub fuel_used: Option<u64>,
    pub heap_used: Option<u64>,
    pub fs_read_file_calls: Option<u64>,
    pub fs_list_dir_calls: Option<u64>,
    pub rr_send_calls: Option<u64>,
    pub rr_request_calls: Option<u64>,
    pub rr_last_request_sha256: Option<String>,
    pub kv_get_calls: Option<u64>,
    pub kv_set_calls: Option<u64>,
    pub sched_stats: Option<SchedStats>,
    pub mem_stats: Option<MemStats>,
    pub debug_stats: Option<DebugStats>,
    pub trap: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemStats {
    pub alloc_calls: u64,
    pub realloc_calls: u64,
    pub free_calls: u64,
    pub bytes_alloc_total: u64,
    pub bytes_freed_total: u64,
    pub live_bytes: u64,
    pub peak_live_bytes: u64,
    pub live_allocs: u64,
    pub peak_live_allocs: u64,
    pub memcpy_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DebugStats {
    pub borrow_violations: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchedStats {
    pub tasks_spawned: u64,
    pub spawn_calls: u64,
    pub join_calls: u64,
    pub yield_calls: u64,
    pub sleep_calls: u64,
    pub chan_send_calls: u64,
    pub chan_recv_calls: u64,
    pub ctx_switches: u64,
    pub wake_events: u64,
    pub blocked_waits: u64,
    pub virtual_time_end: u64,
    pub sched_trace_hash: String,
}

#[derive(Debug, Clone)]
pub struct CompileAndRunResult {
    pub compile: CompilerResult,
    pub solve: Option<RunnerResult>,
}

pub fn compile_options_for_world(
    world: WorldId,
    module_roots: Vec<PathBuf>,
) -> Result<compile::CompileOptions> {
    match world {
        WorldId::SolvePure => Ok(compile::CompileOptions {
            world: WorldId::SolvePure,
            enable_fs: false,
            enable_rr: false,
            enable_kv: false,
            module_roots,
            emit_main: true,
            freestanding: false,
            allow_unsafe: None,
            allow_ffi: None,
        }),
        WorldId::SolveFs => Ok(compile::CompileOptions {
            world: WorldId::SolveFs,
            enable_fs: true,
            enable_rr: false,
            enable_kv: false,
            module_roots,
            emit_main: true,
            freestanding: false,
            allow_unsafe: None,
            allow_ffi: None,
        }),
        WorldId::SolveRr => Ok(compile::CompileOptions {
            world: WorldId::SolveRr,
            enable_fs: false,
            enable_rr: true,
            enable_kv: false,
            module_roots,
            emit_main: true,
            freestanding: false,
            allow_unsafe: None,
            allow_ffi: None,
        }),
        WorldId::SolveKv => Ok(compile::CompileOptions {
            world: WorldId::SolveKv,
            enable_fs: false,
            enable_rr: false,
            enable_kv: true,
            module_roots,
            emit_main: true,
            freestanding: false,
            allow_unsafe: None,
            allow_ffi: None,
        }),
        WorldId::SolveFull => Ok(compile::CompileOptions {
            world: WorldId::SolveFull,
            enable_fs: true,
            enable_rr: true,
            enable_kv: true,
            module_roots,
            emit_main: true,
            freestanding: false,
            allow_unsafe: None,
            allow_ffi: None,
        }),
        other => anyhow::bail!(
            "x07-host-runner supports only deterministic solve worlds, got {}",
            other.as_str()
        ),
    }
}

pub fn compile_and_run(
    program: &[u8],
    config: &RunnerConfig,
    input: &[u8],
    compiled_out: Option<&Path>,
) -> Result<CompileAndRunResult> {
    let compile = compile_program(program, config, compiled_out)?;
    if !compile.ok {
        return Ok(CompileAndRunResult {
            compile,
            solve: None,
        });
    }

    let Some(exe) = compile.compiled_exe.clone() else {
        anyhow::bail!("internal error: compile.ok but no compiled_exe");
    };

    let solve = run_artifact_file(config, &exe, input)?;
    Ok(CompileAndRunResult {
        compile,
        solve: Some(solve),
    })
}

pub fn compile_and_run_with_options(
    program: &[u8],
    config: &RunnerConfig,
    input: &[u8],
    compiled_out: Option<&Path>,
    compile_options: &compile::CompileOptions,
) -> Result<CompileAndRunResult> {
    let compile =
        compile_program_with_options(program, config, compiled_out, compile_options, &[])?;
    if !compile.ok {
        return Ok(CompileAndRunResult {
            compile,
            solve: None,
        });
    }

    let Some(exe) = compile.compiled_exe.clone() else {
        anyhow::bail!("internal error: compile.ok but no compiled_exe");
    };

    let solve = run_artifact_file(config, &exe, input)?;
    Ok(CompileAndRunResult {
        compile,
        solve: Some(solve),
    })
}

pub fn compile_program(
    program: &[u8],
    config: &RunnerConfig,
    compiled_out: Option<&Path>,
) -> Result<CompilerResult> {
    let compile_options = compile_options_for_world(config.world, Vec::new())?;
    compile_program_with_options(program, config, compiled_out, &compile_options, &[])
}

pub fn compile_program_with_options(
    program: &[u8],
    config: &RunnerConfig,
    compiled_out: Option<&Path>,
    compile_options: &compile::CompileOptions,
    extra_cc_args: &[String],
) -> Result<CompilerResult> {
    let lang_id = language::LANG_ID.to_string();
    let guide_md = compile::guide_md();

    let (c_source, compile_stats) =
        match compile::compile_program_to_c_with_stats(program, compile_options) {
            Ok((src, stats)) => (src, stats),
            Err(err) => {
                let msg = format!("{:?}: {}", err.kind, err.message);
                return Ok(CompilerResult {
                    ok: false,
                    exit_status: 1,
                    lang_id,
                    guide_md,
                    c_source_size: 0,
                    compiled_exe: None,
                    compiled_exe_size: None,
                    compile_error: Some(msg),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    fuel_used: None,
                    trap: None,
                });
            }
        };

    let mut cc_args = extra_cc_args.to_vec();
    if c_source.contains("= ev_math_f64_") {
        if let Some(lib) = staged_math_native_lib()? {
            cc_args.push(lib.display().to_string());
        } else {
            return Ok(CompilerResult {
                ok: false,
                exit_status: 1,
                lang_id,
                guide_md,
                c_source_size: c_source.len(),
                compiled_exe: None,
                compiled_exe_size: None,
                compile_error: Some(
                    "native math backend missing (build + stage with ./scripts/build_ext_math.sh)"
                        .to_string(),
                ),
                stdout: Vec::new(),
                stderr: Vec::new(),
                fuel_used: Some(compile_stats.fuel_used),
                trap: None,
            });
        }
    }
    if c_source.contains("= ev_time_tzdb_") {
        if let Some(lib) = staged_time_native_lib()? {
            cc_args.push(lib.display().to_string());
        } else {
            return Ok(CompilerResult {
                ok: false,
                exit_status: 1,
                lang_id,
                guide_md,
                c_source_size: c_source.len(),
                compiled_exe: None,
                compiled_exe_size: None,
                compile_error: Some(
                    "native time backend missing (build + stage with ./scripts/build_ext_time.sh)"
                        .to_string(),
                ),
                stdout: Vec::new(),
                stderr: Vec::new(),
                fuel_used: Some(compile_stats.fuel_used),
                trap: None,
            });
        }
    }
    if c_source.contains("= x07_ext_fs_") {
        if let Some(lib) = staged_ext_fs_native_lib()? {
            cc_args.push(lib.display().to_string());
        } else {
            return Ok(CompilerResult {
                ok: false,
                exit_status: 1,
                lang_id,
                guide_md,
                c_source_size: c_source.len(),
                compiled_exe: None,
                compiled_exe_size: None,
                compile_error: Some(
                    "native ext-fs backend missing (build + stage with ./scripts/build_ext_fs.sh)"
                        .to_string(),
                ),
                stdout: Vec::new(),
                stderr: Vec::new(),
                fuel_used: Some(compile_stats.fuel_used),
                trap: None,
            });
        }
    }
    if c_source.contains("= x07_ext_db_sqlite_") {
        if let Some(lib) = staged_ext_db_sqlite_native_lib()? {
            cc_args.push(lib.display().to_string());
        } else {
            return Ok(CompilerResult {
                ok: false,
                exit_status: 1,
                lang_id,
                guide_md,
                c_source_size: c_source.len(),
                compiled_exe: None,
                compiled_exe_size: None,
                compile_error: Some(
                    "native ext-db-sqlite backend missing (build + stage with ./scripts/build_ext_db_sqlite.sh)"
                        .to_string(),
                ),
                stdout: Vec::new(),
                stderr: Vec::new(),
                fuel_used: Some(compile_stats.fuel_used),
                trap: None,
            });
        }
    }
    if c_source.contains("= x07_ext_db_pg_") {
        if let Some(lib) = staged_ext_db_pg_native_lib()? {
            cc_args.push(lib.display().to_string());
            if cfg!(target_os = "macos") {
                cc_args.push("-framework".to_string());
                cc_args.push("CoreFoundation".to_string());
                cc_args.push("-framework".to_string());
                cc_args.push("SystemConfiguration".to_string());
            }
        } else {
            return Ok(CompilerResult {
                ok: false,
                exit_status: 1,
                lang_id,
                guide_md,
                c_source_size: c_source.len(),
                compiled_exe: None,
                compiled_exe_size: None,
                compile_error: Some(
                    "native ext-db-pg backend missing (build + stage with ./scripts/build_ext_db_pg.sh)"
                        .to_string(),
                ),
                stdout: Vec::new(),
                stderr: Vec::new(),
                fuel_used: Some(compile_stats.fuel_used),
                trap: None,
            });
        }
    }
    if c_source.contains("= x07_ext_db_mysql_") {
        if let Some(lib) = staged_ext_db_mysql_native_lib()? {
            cc_args.push(lib.display().to_string());
        } else {
            return Ok(CompilerResult {
                ok: false,
                exit_status: 1,
                lang_id,
                guide_md,
                c_source_size: c_source.len(),
                compiled_exe: None,
                compiled_exe_size: None,
                compile_error: Some(
                    "native ext-db-mysql backend missing (build + stage with ./scripts/build_ext_db_mysql.sh)"
                        .to_string(),
                ),
                stdout: Vec::new(),
                stderr: Vec::new(),
                fuel_used: Some(compile_stats.fuel_used),
                trap: None,
            });
        }
    }
    if c_source.contains("= x07_ext_db_redis_") {
        if let Some(lib) = staged_ext_db_redis_native_lib()? {
            cc_args.push(lib.display().to_string());
        } else {
            return Ok(CompilerResult {
                ok: false,
                exit_status: 1,
                lang_id,
                guide_md,
                c_source_size: c_source.len(),
                compiled_exe: None,
                compiled_exe_size: None,
                compile_error: Some(
                    "native ext-db-redis backend missing (build + stage with ./scripts/build_ext_db_redis.sh)"
                        .to_string(),
                ),
                stdout: Vec::new(),
                stderr: Vec::new(),
                fuel_used: Some(compile_stats.fuel_used),
                trap: None,
            });
        }
    }
    if c_source.contains("= x07_ext_regex_") {
        if let Some(lib) = staged_ext_regex_native_lib()? {
            cc_args.push(lib.display().to_string());
        } else {
            return Ok(CompilerResult {
                ok: false,
                exit_status: 1,
                lang_id,
                guide_md,
                c_source_size: c_source.len(),
                compiled_exe: None,
                compiled_exe_size: None,
                compile_error: Some(
                    "native ext-regex backend missing (build + stage with ./scripts/build_ext_regex.sh)"
                        .to_string(),
                ),
                stdout: Vec::new(),
                stderr: Vec::new(),
                fuel_used: Some(compile_stats.fuel_used),
                trap: None,
            });
        }
    }

    let tool = compile_c_to_exe(&c_source, config, compile_options, &cc_args)?;
    if !tool.ok {
        return Ok(CompilerResult {
            ok: false,
            exit_status: tool.exit_status,
            lang_id,
            guide_md,
            c_source_size: c_source.len(),
            compiled_exe: None,
            compiled_exe_size: None,
            compile_error: Some(format!("C toolchain failed (exit={})", tool.exit_status)),
            stdout: tool.stdout,
            stderr: tool.stderr,
            fuel_used: Some(compile_stats.fuel_used),
            trap: None,
        });
    }

    let exe = tool
        .exe_path
        .context("internal error: toolchain ok but no exe")?;

    let final_exe = if let Some(out_path) = compiled_out {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir: {}", parent.display()))?;
        }
        std::fs::copy(&exe, out_path).with_context(|| {
            format!(
                "copy compiled artifact from {} to {}",
                exe.display(),
                out_path.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let src_mode = std::fs::metadata(&exe)
                .map(|m| m.permissions().mode())
                .unwrap_or(0o755);
            let _ = std::fs::set_permissions(out_path, std::fs::Permissions::from_mode(src_mode));
        }
        out_path.to_path_buf()
    } else {
        exe
    };

    let exe_size = std::fs::metadata(&final_exe).map(|m| m.len()).ok();

    Ok(CompilerResult {
        ok: true,
        exit_status: 0,
        lang_id,
        guide_md,
        c_source_size: c_source.len(),
        compiled_exe: Some(final_exe),
        compiled_exe_size: exe_size,
        compile_error: None,
        stdout: tool.stdout,
        stderr: tool.stderr,
        fuel_used: Some(compile_stats.fuel_used),
        trap: None,
    })
}

pub fn run_artifact_file(
    config: &RunnerConfig,
    artifact_path: &Path,
    input: &[u8],
) -> Result<RunnerResult> {
    let out = run_child(artifact_path, input, config)?;
    let exit_status = out.exit_status;
    let stdout = out.stdout;
    let stderr = out.stderr;

    if out.timed_out {
        return Ok(RunnerResult {
            ok: false,
            exit_status,
            solve_output: Vec::new(),
            stdout,
            stderr,
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
            trap: Some("wall timeout".to_string()),
        });
    }

    if out.stderr_truncated {
        return Ok(RunnerResult {
            ok: false,
            exit_status,
            solve_output: Vec::new(),
            stdout,
            stderr,
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
            exit_status,
            solve_output: Vec::new(),
            stdout,
            stderr,
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

    let parse = parse_native_stdout(&stdout, config.max_output_bytes);

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

    let metrics = parse_metrics(&stderr);
    if exit_status == 0 && metrics.is_none() && trap.is_none() {
        trap = Some("missing metrics json line on stderr".to_string());
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

    let ok = exit_status == 0 && trap.is_none();
    Ok(RunnerResult {
        ok,
        exit_status,
        solve_output,
        stdout,
        stderr,
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

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsLine {
    pub fuel_used: Option<u64>,
    pub heap_used: Option<u64>,
    pub fs_read_file_calls: Option<u64>,
    pub fs_list_dir_calls: Option<u64>,
    pub rr_send_calls: Option<u64>,
    pub rr_request_calls: Option<u64>,
    pub rr_last_request_sha256: Option<String>,
    pub kv_get_calls: Option<u64>,
    pub kv_set_calls: Option<u64>,
    pub sched_stats: Option<SchedStats>,
    pub mem_stats: Option<MemStats>,
    pub debug_stats: Option<DebugStats>,
}

pub fn parse_metrics(stderr: &[u8]) -> Option<MetricsLine> {
    let text = std::str::from_utf8(stderr).ok()?;
    for line in text.lines().rev() {
        let line = line.trim_start();
        if !line.starts_with('{') {
            continue;
        }
        if let Ok(m) = serde_json::from_str::<MetricsLine>(line) {
            if m.fuel_used.is_some()
                || m.heap_used.is_some()
                || m.fs_read_file_calls.is_some()
                || m.fs_list_dir_calls.is_some()
                || m.rr_send_calls.is_some()
                || m.rr_request_calls.is_some()
                || m.rr_last_request_sha256.is_some()
                || m.kv_get_calls.is_some()
                || m.kv_set_calls.is_some()
                || m.sched_stats.is_some()
                || m.mem_stats.is_some()
                || m.debug_stats.is_some()
            {
                return Some(m);
            }
        }
    }
    None
}

pub fn parse_native_stdout(stdout: &[u8], max_output_bytes: usize) -> Result<Vec<u8>> {
    if stdout.len() < 4 {
        anyhow::bail!("native stdout too short for length prefix");
    }
    let len = u32::from_le_bytes([stdout[0], stdout[1], stdout[2], stdout[3]]) as usize;
    if len > max_output_bytes {
        anyhow::bail!("native output too large: {len} > max_output_bytes={max_output_bytes}");
    }
    if stdout.len() != 4 + len {
        anyhow::bail!(
            "native stdout length mismatch: expected {} got {}",
            4 + len,
            stdout.len()
        );
    }
    Ok(stdout[4..].to_vec())
}

fn cache_dir() -> Result<PathBuf> {
    if let Some(override_dir) = std::env::var_os("X07_NATIVE_CACHE_DIR") {
        let dir = PathBuf::from(override_dir);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("create native cache dir: {}", dir.display()))?;
        return Ok(dir);
    }

    let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("target/x07-native-cache"));
    if let Some(dir) = candidate {
        if std::fs::create_dir_all(&dir).is_ok() {
            return Ok(dir);
        }
    }

    let dir = std::env::temp_dir().join("x07-native-cache");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create native cache dir: {}", dir.display()))?;
    Ok(dir)
}

fn workspace_root() -> Result<PathBuf> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(|p| p.parent())
        .context("locate workspace root")?
        .canonicalize()
        .context("canonicalize workspace root")
}

fn staged_math_native_lib() -> Result<Option<PathBuf>> {
    let root = workspace_root()?;
    let candidates = [
        root.join("deps/x07/libx07_math.a"),
        root.join("deps/x07/x07_math.lib"),
    ];
    Ok(candidates.into_iter().find(|p| p.is_file()))
}

fn staged_time_native_lib() -> Result<Option<PathBuf>> {
    let root = workspace_root()?;
    let candidates = [
        root.join("deps/x07/libx07_time.a"),
        root.join("deps/x07/x07_time.lib"),
    ];
    Ok(candidates.into_iter().find(|p| p.is_file()))
}

fn staged_ext_fs_native_lib() -> Result<Option<PathBuf>> {
    let root = workspace_root()?;
    let candidates = [
        root.join("deps/x07/libx07_ext_fs.a"),
        root.join("deps/x07/x07_ext_fs.lib"),
    ];
    Ok(candidates.into_iter().find(|p| p.is_file()))
}

fn staged_ext_db_sqlite_native_lib() -> Result<Option<PathBuf>> {
    let root = workspace_root()?;
    let candidates = [
        root.join("deps/x07/libx07_ext_db_sqlite.a"),
        root.join("deps/x07/x07_ext_db_sqlite.lib"),
    ];
    Ok(candidates.into_iter().find(|p| p.is_file()))
}

fn staged_ext_db_pg_native_lib() -> Result<Option<PathBuf>> {
    let root = workspace_root()?;
    let candidates = [
        root.join("deps/x07/libx07_ext_db_pg.a"),
        root.join("deps/x07/x07_ext_db_pg.lib"),
    ];
    Ok(candidates.into_iter().find(|p| p.is_file()))
}

fn staged_ext_db_mysql_native_lib() -> Result<Option<PathBuf>> {
    let root = workspace_root()?;
    let candidates = [
        root.join("deps/x07/libx07_ext_db_mysql.a"),
        root.join("deps/x07/x07_ext_db_mysql.lib"),
    ];
    Ok(candidates.into_iter().find(|p| p.is_file()))
}

fn staged_ext_db_redis_native_lib() -> Result<Option<PathBuf>> {
    let root = workspace_root()?;
    let candidates = [
        root.join("deps/x07/libx07_ext_db_redis.a"),
        root.join("deps/x07/x07_ext_db_redis.lib"),
    ];
    Ok(candidates.into_iter().find(|p| p.is_file()))
}

fn staged_ext_regex_native_lib() -> Result<Option<PathBuf>> {
    let root = workspace_root()?;
    let candidates = [
        root.join("deps/x07/libx07_ext_regex.a"),
        root.join("deps/x07/x07_ext_regex.lib"),
    ];
    Ok(candidates.into_iter().find(|p| p.is_file()))
}

#[derive(Debug, Clone)]
pub struct NativeToolchainConfig {
    pub world_tag: String,
    pub fuel_init: u64,
    pub mem_cap_bytes: usize,
    pub debug_borrow_checks: bool,
    pub enable_fs: bool,
    pub enable_rr: bool,
    pub enable_kv: bool,
    pub extra_cc_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ToolchainOutput {
    pub ok: bool,
    pub exit_status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exe_path: Option<PathBuf>,
}

pub fn compile_c_to_exe_with_config(
    c_source: &str,
    config: &NativeToolchainConfig,
) -> Result<ToolchainOutput> {
    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    let cc = std::env::var_os("X07_CC").unwrap_or_else(|| OsStr::new("cc").to_os_string());
    let cc_args = std::env::var("X07_CC_ARGS").unwrap_or_default();
    let keep_c = std::env::var("X07_KEEP_C")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        })
        .unwrap_or(false);

    let mut cc_version = Vec::new();
    if let Ok(out) = Command::new(&cc).arg("--version").output() {
        cc_version.extend_from_slice(&out.stdout);
        cc_version.extend_from_slice(&out.stderr);
    }

    let mut hasher = Sha256::new();
    hasher.update(b"x07-native-cache-v2\0");
    hasher.update(c_source.as_bytes());
    hasher.update(b"\0");
    hasher.update(&cc_version);
    hasher.update(b"\0");
    hasher.update(config.world_tag.as_bytes());
    hasher.update(b"\0");
    hasher.update(config.fuel_init.to_le_bytes());
    hasher.update(config.mem_cap_bytes.to_le_bytes());
    hasher.update([config.debug_borrow_checks as u8]);
    hasher.update(b"\0");
    hasher.update(cc_args.trim().as_bytes());
    hasher.update(b"\0");
    for a in cc_args.split_whitespace() {
        let p = Path::new(a);
        if p.is_file() {
            hasher.update(b"cc_arg_file\0");
            hasher.update(a.as_bytes());
            hasher.update(b"\0");
            let mut f = std::fs::File::open(p)
                .with_context(|| format!("open cc_arg file for cache key: {}", p.display()))?;
            let mut buf = [0u8; 8192];
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            hasher.update(b"\0");
        }
    }
    for a in &config.extra_cc_args {
        hasher.update(a.as_bytes());
        hasher.update(b"\0");
        let p = Path::new(a);
        if p.is_file() {
            // Make the cache key depend on linked library contents.
            // Otherwise, rebuilding a staged `.a`/`.lib` would not invalidate the cached exe.
            hasher.update(b"file\0");
            let mut f = std::fs::File::open(p).with_context(|| {
                format!("open extra_cc_arg file for cache key: {}", p.display())
            })?;
            let mut buf = [0u8; 8192];
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            hasher.update(b"\0");
        }
    }
    let key = hex_lower(&hasher.finalize());

    let dir = cache_dir()?.join(&key);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create cache dir: {}", dir.display()))?;

    let exe_path = {
        let mut p = dir.join("solver");
        if cfg!(windows) {
            p.set_extension("exe");
        }
        p
    };
    let keep_c_path = dir.join("solver.c");

    if exe_path.exists() {
        if keep_c && !keep_c_path.exists() {
            let pid = std::process::id();
            let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let tmp_src_path = dir.join(format!("solver_{pid}_{n}.c"));
            if std::fs::write(&tmp_src_path, c_source.as_bytes()).is_ok()
                && std::fs::rename(&tmp_src_path, &keep_c_path).is_err()
            {
                if !keep_c_path.exists() {
                    let _ = std::fs::copy(&tmp_src_path, &keep_c_path);
                }
                let _ = std::fs::remove_file(&tmp_src_path);
            }
        }
        return Ok(ToolchainOutput {
            ok: true,
            exit_status: 0,
            stdout: Vec::new(),
            stderr: Vec::new(),
            exe_path: Some(exe_path),
        });
    }

    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_src_path = dir.join(format!("solver_{pid}_{n}.c"));
    let tmp_exe_path = {
        let mut p = dir.join(format!("solver_{pid}_{n}"));
        if cfg!(windows) {
            p.set_extension("exe");
        }
        p
    };

    std::fs::write(&tmp_src_path, c_source.as_bytes())
        .with_context(|| format!("write C source: {}", tmp_src_path.display()))?;

    let mut cmd = Command::new(&cc);
    cmd.arg("-std=c11");
    cmd.arg("-O2");
    cmd.arg("-fno-builtin");
    #[cfg(target_os = "linux")]
    {
        cmd.arg("-D_GNU_SOURCE");
        cmd.arg("-D_DEFAULT_SOURCE");
    }
    cmd.arg(format!("-DX07_FUEL_INIT={}ULL", config.fuel_init));
    cmd.arg(format!("-DX07_MEM_CAP={}u", config.mem_cap_bytes));
    if config.debug_borrow_checks {
        cmd.arg("-DX07_DEBUG_BORROW=1");
    }
    cmd.arg(format!(
        "-DX07_ENABLE_FS={}",
        if config.enable_fs { 1 } else { 0 }
    ));
    cmd.arg(format!(
        "-DX07_ENABLE_RR={}",
        if config.enable_rr { 1 } else { 0 }
    ));
    cmd.arg(format!(
        "-DX07_ENABLE_KV={}",
        if config.enable_kv { 1 } else { 0 }
    ));

    cmd.arg(&tmp_src_path);
    cmd.arg("-o");
    cmd.arg(&tmp_exe_path);
    for a in cc_args.split_whitespace() {
        if !a.trim().is_empty() {
            cmd.arg(a);
        }
    }
    for a in &config.extra_cc_args {
        cmd.arg(a);
    }

    let cmd_program = cmd.get_program().to_os_string();
    #[cfg(windows)]
    let cmd_args: Vec<OsString> = cmd.get_args().map(|a| a.to_os_string()).collect();

    let out = cmd
        .output()
        .with_context(|| format!("invoke cc: {:?}", cc))?;
    let exit_status = out.status.code().unwrap_or(1);
    let ok = out.status.success();

    let mut stderr = out.stderr;
    if !ok {
        fn tail_truncate(b: &[u8], limit: usize) -> Vec<u8> {
            if b.len() <= limit {
                return b.to_vec();
            }
            let start = b.len() - limit;
            let mut out = Vec::new();
            out.extend_from_slice(b"...<truncated>...\n");
            out.extend_from_slice(&b[start..]);
            out
        }

        let mut diag = Vec::new();
        diag.extend_from_slice(b"--- x07 cc invocation ---\n");
        diag.extend_from_slice(format!("cc: {}\n", cmd_program.to_string_lossy()).as_bytes());
        if !cc_args.trim().is_empty() {
            diag.extend_from_slice(b"\n--- X07_CC_ARGS ---\n");
            diag.extend_from_slice(cc_args.trim().as_bytes());
            diag.extend_from_slice(b"\n");
        }
        diag.extend_from_slice(b"\n--- tmp paths ---\n");
        diag.extend_from_slice(format!("src: {}\n", tmp_src_path.display()).as_bytes());
        diag.extend_from_slice(format!("exe: {}\n", tmp_exe_path.display()).as_bytes());
        if keep_c {
            diag.extend_from_slice(format!("keep_c: {}\n", keep_c_path.display()).as_bytes());
        }

        #[cfg(windows)]
        let mut diag_cc_out = Vec::new();
        #[cfg(not(windows))]
        let diag_cc_out = Vec::new();
        #[cfg(windows)]
        {
            let mut ld_path = Vec::new();
            if let Ok(out) = Command::new(&cc).arg("-print-prog-name=ld").output() {
                ld_path.extend_from_slice(&out.stdout);
                ld_path.extend_from_slice(&out.stderr);
            }
            if !ld_path.is_empty() {
                diag.extend_from_slice(b"\n--- cc -print-prog-name=ld ---\n");
                diag.extend_from_slice(&tail_truncate(&ld_path, 400));
                if !diag.ends_with(b"\n") {
                    diag.extend_from_slice(b"\n");
                }
            }

            let mut search_dirs = Vec::new();
            if let Ok(out) = Command::new(&cc).arg("-print-search-dirs").output() {
                search_dirs.extend_from_slice(&out.stdout);
                search_dirs.extend_from_slice(&out.stderr);
            }
            if !search_dirs.is_empty() {
                diag.extend_from_slice(b"\n--- cc -print-search-dirs (tail) ---\n");
                diag.extend_from_slice(&tail_truncate(&search_dirs, 600));
                if !diag.ends_with(b"\n") {
                    diag.extend_from_slice(b"\n");
                }
            }

            diag.extend_from_slice(b"\n--- cc -print-file-name ---\n");
            for lib in [
                "libssl.dll.a",
                "libssl.a",
                "libssl-3.dll.a",
                "libcrypto.dll.a",
                "libcrypto.a",
                "libcrypto-3.dll.a",
            ] {
                let mut resolved = Vec::new();
                if let Ok(out) = Command::new(&cc)
                    .arg(format!("-print-file-name={lib}"))
                    .output()
                {
                    resolved.extend_from_slice(&out.stdout);
                    resolved.extend_from_slice(&out.stderr);
                }
                let path = String::from_utf8_lossy(&resolved);
                let path = path.trim();
                let exists = Path::new(path).is_file();
                let exists = if exists { " (found)" } else { " (missing)" };
                diag.extend_from_slice(format!("{lib}: {path}{exists}\n").as_bytes());
            }

            let mut dry_run = Vec::new();
            let mut dry_cmd = Command::new(&cc);
            dry_cmd.arg("-###");
            dry_cmd.args(&cmd_args);
            if let Ok(out) = dry_cmd.output() {
                dry_run.extend_from_slice(&out.stdout);
                dry_run.extend_from_slice(&out.stderr);
            }
            if !dry_run.is_empty() {
                diag.extend_from_slice(b"\n--- cc -### (tail) ---\n");
                diag.extend_from_slice(&tail_truncate(&dry_run, 1800));
                if !diag.ends_with(b"\n") {
                    diag.extend_from_slice(b"\n");
                }
            }

            let mut diag_cmd = Command::new(&cc);
            diag_cmd.args(&cmd_args);
            diag_cmd.arg("-Wl,-t");
            if let Ok(out) = diag_cmd.output() {
                diag_cc_out.extend_from_slice(&out.stdout);
                diag_cc_out.extend_from_slice(&out.stderr);
            }
        }
        if !diag_cc_out.is_empty() {
            diag.extend_from_slice(b"\n--- cc -Wl,-t output (tail) ---\n");
            diag.extend_from_slice(&tail_truncate(&diag_cc_out, 2400));
            if !diag.ends_with(b"\n") {
                diag.extend_from_slice(b"\n");
            }
        }

        let mut combined = diag;
        if !stderr.is_empty() {
            combined.extend_from_slice(b"\n--- cc stderr ---\n");
            combined.extend_from_slice(&stderr);
        }
        stderr = combined;
    }

    if keep_c {
        if !keep_c_path.exists() {
            if std::fs::rename(&tmp_src_path, &keep_c_path).is_err() {
                if !keep_c_path.exists() {
                    let _ = std::fs::copy(&tmp_src_path, &keep_c_path);
                }
                let _ = std::fs::remove_file(&tmp_src_path);
            }
        } else {
            let _ = std::fs::remove_file(&tmp_src_path);
        }
    } else {
        let _ = std::fs::remove_file(&tmp_src_path);
    }

    let final_exe_path = if ok {
        match std::fs::rename(&tmp_exe_path, &exe_path) {
            Ok(()) => exe_path.clone(),
            Err(_) if exe_path.exists() => {
                let _ = std::fs::remove_file(&tmp_exe_path);
                exe_path.clone()
            }
            Err(err) => {
                let copy = std::fs::copy(&tmp_exe_path, &exe_path);
                let _ = std::fs::remove_file(&tmp_exe_path);
                copy.with_context(|| format!("finalize compiled artifact: {err}"))?;
                exe_path.clone()
            }
        }
    } else {
        let _ = std::fs::remove_file(&tmp_exe_path);
        exe_path.clone()
    };

    Ok(ToolchainOutput {
        ok,
        exit_status,
        stdout: out.stdout,
        stderr,
        exe_path: ok.then_some(final_exe_path),
    })
}

fn compile_c_to_exe(
    c_source: &str,
    config: &RunnerConfig,
    options: &compile::CompileOptions,
    extra_cc_args: &[String],
) -> Result<ToolchainOutput> {
    let toolchain = NativeToolchainConfig {
        world_tag: options.world.as_str().to_string(),
        fuel_init: config.solve_fuel,
        mem_cap_bytes: config.max_memory_bytes,
        debug_borrow_checks: config.debug_borrow_checks,
        enable_fs: options.enable_fs,
        enable_rr: options.enable_rr,
        enable_kv: options.enable_kv,
        extra_cc_args: extra_cc_args.to_vec(),
    };
    compile_c_to_exe_with_config(c_source, &toolchain)
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir();
        let pid = std::process::id();

        for _ in 0..10_000 {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!("{prefix}_{pid}_{n}"));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(err).with_context(|| format!("create temp dir: {}", path.display()))
                }
            }
        }
        anyhow::bail!("failed to create unique temp dir under {}", base.display())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn setup_run_dir(tmp: &TempDir, config: &RunnerConfig) -> Result<()> {
    match config.world {
        WorldId::SolvePure => Ok(()),
        WorldId::SolveFs => {
            let fixture = config
                .fixture_fs_dir
                .as_ref()
                .context("missing fixture_fs_dir for solve-fs")?;
            let fs_root = config
                .fixture_fs_root
                .as_deref()
                .unwrap_or_else(|| Path::new(""));
            ensure_safe_rel_path(fs_root)?;
            let fs_src = fixture.join(fs_root);
            copy_dir_contents(&fs_src, tmp.path())
                .with_context(|| format!("copy fixture dir: {}", fs_src.display()))?;

            if let Some(latency_index) = config.fixture_fs_latency_index.as_deref() {
                ensure_safe_rel_path(latency_index)?;
                let src = fixture.join(latency_index);
                let dst = tmp.path().join(".x07_fs").join("latency.evfslat");
                write_fs_latency_evfslat(&src, &dst)
                    .with_context(|| format!("generate fs latency index from {}", src.display()))?;
            }
            #[cfg(unix)]
            make_readonly_recursive(tmp.path())?;
            Ok(())
        }
        WorldId::SolveRr => {
            let fixture = config
                .fixture_rr_dir
                .as_ref()
                .context("missing fixture_rr_dir for solve-rr")?;
            let rr_dir = tmp.path().join(".x07_rr");
            std::fs::create_dir(&rr_dir)
                .with_context(|| format!("create rr fixture dir: {}", rr_dir.display()))?;
            copy_dir_contents(fixture, &rr_dir)
                .with_context(|| format!("copy rr fixture dir: {}", fixture.display()))?;

            if let Some(rr_index) = config.fixture_rr_index.as_deref() {
                ensure_safe_rel_path(rr_index)?;
                let src = fixture.join(rr_index);
                let dst = rr_dir.join("index.evrr");
                write_rr_index_evrr(&src, &dst)
                    .with_context(|| format!("generate rr index from {}", src.display()))?;
            }
            #[cfg(unix)]
            make_readonly_recursive(tmp.path())?;
            Ok(())
        }
        WorldId::SolveKv => {
            let fixture = config
                .fixture_kv_dir
                .as_ref()
                .context("missing fixture_kv_dir for solve-kv")?;
            let kv_dir = tmp.path().join(".x07_kv");
            std::fs::create_dir(&kv_dir)
                .with_context(|| format!("create kv fixture dir: {}", kv_dir.display()))?;
            copy_dir_contents(fixture, &kv_dir)
                .with_context(|| format!("copy kv fixture dir: {}", fixture.display()))?;

            let seed_evkv = kv_dir.join("seed.evkv");
            if !seed_evkv.is_file() {
                let seed_json = config
                    .fixture_kv_seed
                    .as_deref()
                    .context("missing fixture_kv_seed for solve-kv (seed.evkv not present)")?;
                ensure_safe_rel_path(seed_json)?;
                let src = fixture.join(seed_json);
                let latency_dst = kv_dir.join("latency.evkvlat");
                write_kv_seed_evkv_and_latency(&src, &seed_evkv, &latency_dst)
                    .with_context(|| format!("generate kv seed from {}", src.display()))?;
            }
            #[cfg(unix)]
            make_readonly_recursive(tmp.path())?;
            Ok(())
        }
        WorldId::SolveFull => {
            let fs_fixture = config
                .fixture_fs_dir
                .as_ref()
                .context("missing fixture_fs_dir for solve-full")?;
            let fs_root = config
                .fixture_fs_root
                .as_deref()
                .unwrap_or_else(|| Path::new(""));
            ensure_safe_rel_path(fs_root)?;
            let fs_src = fs_fixture.join(fs_root);
            copy_dir_contents(&fs_src, tmp.path())
                .with_context(|| format!("copy fixture dir: {}", fs_src.display()))?;

            if let Some(latency_index) = config.fixture_fs_latency_index.as_deref() {
                ensure_safe_rel_path(latency_index)?;
                let src = fs_fixture.join(latency_index);
                let dst = tmp.path().join(".x07_fs").join("latency.evfslat");
                write_fs_latency_evfslat(&src, &dst)
                    .with_context(|| format!("generate fs latency index from {}", src.display()))?;
            }

            let rr_fixture = config
                .fixture_rr_dir
                .as_ref()
                .context("missing fixture_rr_dir for solve-full")?;
            let rr_dir = tmp.path().join(".x07_rr");
            std::fs::create_dir(&rr_dir)
                .with_context(|| format!("create rr fixture dir: {}", rr_dir.display()))?;
            copy_dir_contents(rr_fixture, &rr_dir)
                .with_context(|| format!("copy rr fixture dir: {}", rr_fixture.display()))?;
            if let Some(rr_index) = config.fixture_rr_index.as_deref() {
                ensure_safe_rel_path(rr_index)?;
                let src = rr_fixture.join(rr_index);
                let dst = rr_dir.join("index.evrr");
                write_rr_index_evrr(&src, &dst)
                    .with_context(|| format!("generate rr index from {}", src.display()))?;
            }

            let kv_fixture = config
                .fixture_kv_dir
                .as_ref()
                .context("missing fixture_kv_dir for solve-full")?;
            let kv_dir = tmp.path().join(".x07_kv");
            std::fs::create_dir(&kv_dir)
                .with_context(|| format!("create kv fixture dir: {}", kv_dir.display()))?;
            copy_dir_contents(kv_fixture, &kv_dir)
                .with_context(|| format!("copy kv fixture dir: {}", kv_fixture.display()))?;

            let seed_evkv = kv_dir.join("seed.evkv");
            if !seed_evkv.is_file() {
                let seed_json = config
                    .fixture_kv_seed
                    .as_deref()
                    .context("missing fixture_kv_seed for solve-full (seed.evkv not present)")?;
                ensure_safe_rel_path(seed_json)?;
                let src = kv_fixture.join(seed_json);
                let latency_dst = kv_dir.join("latency.evkvlat");
                write_kv_seed_evkv_and_latency(&src, &seed_evkv, &latency_dst)
                    .with_context(|| format!("generate kv seed from {}", src.display()))?;
            }

            #[cfg(unix)]
            make_readonly_recursive(tmp.path())?;
            Ok(())
        }
        other => anyhow::bail!(
            "x07-host-runner supports only deterministic solve worlds, got {}",
            other.as_str()
        ),
    }
}

fn copy_dir_contents(src_dir: &Path, dst_dir: &Path) -> Result<()> {
    for entry in
        std::fs::read_dir(src_dir).with_context(|| format!("read_dir: {}", src_dir.display()))?
    {
        let entry = entry.context("read_dir entry")?;
        let file_type = entry.file_type().context("file_type")?;
        let src_path = entry.path();
        let dst_path = dst_dir.join(entry.file_name());
        copy_tree(&src_path, &dst_path, &file_type)?;
    }
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path, src_type: &std::fs::FileType) -> Result<()> {
    if src_type.is_dir() {
        std::fs::create_dir(dst).with_context(|| format!("create_dir: {}", dst.display()))?;
        for entry in
            std::fs::read_dir(src).with_context(|| format!("read_dir: {}", src.display()))?
        {
            let entry = entry.context("read_dir entry")?;
            let file_type = entry.file_type().context("file_type")?;
            let child_src = entry.path();
            let child_dst = dst.join(entry.file_name());
            copy_tree(&child_src, &child_dst, &file_type)?;
        }
        return Ok(());
    }
    if src_type.is_file() {
        std::fs::copy(src, dst)
            .with_context(|| format!("copy file from {} to {}", src.display(), dst.display()))?;
        return Ok(());
    }
    anyhow::bail!("unsupported fixture entry type: {}", src.display());
}

pub fn ensure_safe_rel_path(rel: &Path) -> Result<()> {
    if rel.as_os_str().is_empty() {
        return Ok(());
    }
    if rel.is_absolute() {
        anyhow::bail!("expected safe relative path, got {}", rel.display());
    }
    for c in rel.components() {
        match c {
            std::path::Component::Normal(_) => {}
            _ => anyhow::bail!("expected safe relative path, got {}", rel.display()),
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct FsLatencyIndexJsonV1 {
    format: String,
    default_ticks: u64,
    paths: BTreeMap<String, u64>,
}

fn write_fs_latency_evfslat(src_json: &Path, dst_bin: &Path) -> Result<()> {
    let obj = serde_json::from_slice::<FsLatencyIndexJsonV1>(
        &std::fs::read(src_json)
            .with_context(|| format!("read fs latency json: {}", src_json.display()))?,
    )
    .with_context(|| format!("parse fs latency json: {}", src_json.display()))?;
    if obj.format != "x07.fs.latency@0.1.0" {
        anyhow::bail!("unexpected fs latency format: {}", obj.format);
    }
    let default_ticks =
        u32::try_from(obj.default_ticks).context("fs latency default_ticks out of u32 range")?;
    let count = u32::try_from(obj.paths.len()).context("fs latency paths too many")?;

    let mut out = Vec::new();
    out.extend_from_slice(b"X7FL");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&default_ticks.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());

    for (path, ticks64) in obj.paths {
        let ticks = u32::try_from(ticks64).context("fs latency ticks out of u32 range")?;
        let p = path.as_bytes();
        let plen = u32::try_from(p.len()).context("fs latency path too long")?;
        out.extend_from_slice(&plen.to_le_bytes());
        out.extend_from_slice(p);
        out.extend_from_slice(&ticks.to_le_bytes());
    }

    if let Some(parent) = dst_bin.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }
    std::fs::write(dst_bin, out)
        .with_context(|| format!("write fs latency bin: {}", dst_bin.display()))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct RrFixtureIndexJsonV1 {
    format: String,
    default_latency_ticks: u64,
    requests: BTreeMap<String, RrFixtureIndexRequestJsonV1>,
}

#[derive(Debug, Deserialize)]
struct RrFixtureIndexRequestJsonV1 {
    latency_ticks: u64,
    body_file: String,
}

fn write_rr_index_evrr(src_json: &Path, dst_bin: &Path) -> Result<()> {
    let obj = serde_json::from_slice::<RrFixtureIndexJsonV1>(
        &std::fs::read(src_json)
            .with_context(|| format!("read rr index json: {}", src_json.display()))?,
    )
    .with_context(|| format!("parse rr index json: {}", src_json.display()))?;
    if obj.format != "x07.rr.fixture_index@0.1.0" {
        anyhow::bail!("unexpected rr index format: {}", obj.format);
    }
    let default_ticks = u32::try_from(obj.default_latency_ticks)
        .context("rr index default_latency_ticks out of u32 range")?;
    let count = u32::try_from(obj.requests.len()).context("rr index requests too many")?;

    let mut out = Vec::new();
    out.extend_from_slice(b"X7RR");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&default_ticks.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());

    for (key, req) in obj.requests {
        let k = key.as_bytes();
        let klen = u32::try_from(k.len()).context("rr index key too long")?;
        out.extend_from_slice(&klen.to_le_bytes());
        out.extend_from_slice(k);

        let ticks =
            u32::try_from(req.latency_ticks).context("rr index latency_ticks out of u32 range")?;
        out.extend_from_slice(&ticks.to_le_bytes());

        let body = req.body_file.as_bytes();
        let blen = u32::try_from(body.len()).context("rr index body_file too long")?;
        out.extend_from_slice(&blen.to_le_bytes());
        out.extend_from_slice(body);
    }

    if let Some(parent) = dst_bin.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }
    std::fs::write(dst_bin, out)
        .with_context(|| format!("write rr index bin: {}", dst_bin.display()))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct KvSeedJsonV1 {
    format: String,
    default_latency_ticks: u64,
    entries: Vec<KvSeedEntryJsonV1>,
}

#[derive(Debug, Deserialize)]
struct KvSeedEntryJsonV1 {
    key_b64: String,
    value_b64: String,
    latency_ticks: u64,
}

struct KvSeedEntryDecoded {
    key: Vec<u8>,
    value: Vec<u8>,
    latency_ticks: u32,
}

fn write_kv_seed_evkv_and_latency(
    src_json: &Path,
    seed_dst: &Path,
    latency_dst: &Path,
) -> Result<()> {
    let obj = serde_json::from_slice::<KvSeedJsonV1>(
        &std::fs::read(src_json)
            .with_context(|| format!("read kv seed json: {}", src_json.display()))?,
    )
    .with_context(|| format!("parse kv seed json: {}", src_json.display()))?;
    if obj.format != "x07.kv.seed@0.1.0" {
        anyhow::bail!("unexpected kv seed format: {}", obj.format);
    }

    let default_ticks = u32::try_from(obj.default_latency_ticks)
        .context("kv seed default_latency_ticks out of u32 range")?;

    let b64 = base64::engine::general_purpose::STANDARD;
    let mut decoded: Vec<KvSeedEntryDecoded> = Vec::with_capacity(obj.entries.len());
    for e in obj.entries {
        let key = b64
            .decode(e.key_b64.as_bytes())
            .with_context(|| format!("decode kv seed key_b64: {}", e.key_b64))?;
        let value = b64
            .decode(e.value_b64.as_bytes())
            .with_context(|| format!("decode kv seed value_b64: {}", e.value_b64))?;
        let latency_ticks =
            u32::try_from(e.latency_ticks).context("kv seed latency_ticks out of u32 range")?;
        decoded.push(KvSeedEntryDecoded {
            key,
            value,
            latency_ticks,
        });
    }

    decoded.sort_by(|a, b| a.key.as_slice().cmp(b.key.as_slice()));

    let count = u32::try_from(decoded.len()).context("kv seed too many entries")?;
    let mut seed = Vec::new();
    seed.extend_from_slice(b"X7KV");
    seed.extend_from_slice(&1u16.to_le_bytes());
    seed.extend_from_slice(&count.to_le_bytes());
    for e in &decoded {
        let klen = u32::try_from(e.key.len()).context("kv seed key too long")?;
        seed.extend_from_slice(&klen.to_le_bytes());
        seed.extend_from_slice(&e.key);
        let vlen = u32::try_from(e.value.len()).context("kv seed value too long")?;
        seed.extend_from_slice(&vlen.to_le_bytes());
        seed.extend_from_slice(&e.value);
    }

    let mut latency = Vec::new();
    latency.extend_from_slice(b"X7KL");
    latency.extend_from_slice(&1u16.to_le_bytes());
    latency.extend_from_slice(&0u16.to_le_bytes());
    latency.extend_from_slice(&default_ticks.to_le_bytes());
    latency.extend_from_slice(&count.to_le_bytes());
    for e in &decoded {
        let klen = u32::try_from(e.key.len()).context("kv latency key too long")?;
        latency.extend_from_slice(&klen.to_le_bytes());
        latency.extend_from_slice(&e.key);
        latency.extend_from_slice(&e.latency_ticks.to_le_bytes());
    }

    if let Some(parent) = seed_dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }
    std::fs::write(seed_dst, seed)
        .with_context(|| format!("write kv seed bin: {}", seed_dst.display()))?;

    if let Some(parent) = latency_dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }
    std::fs::write(latency_dst, latency)
        .with_context(|| format!("write kv latency bin: {}", latency_dst.display()))?;

    Ok(())
}

#[cfg(unix)]
fn make_readonly_recursive(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let md =
        std::fs::symlink_metadata(path).with_context(|| format!("metadata: {}", path.display()))?;
    let ft = md.file_type();
    if ft.is_dir() {
        for entry in
            std::fs::read_dir(path).with_context(|| format!("read_dir: {}", path.display()))?
        {
            let entry = entry.context("read_dir entry")?;
            make_readonly_recursive(&entry.path())?;
        }
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555));
        return Ok(());
    }
    if ft.is_file() {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o444));
        return Ok(());
    }
    anyhow::bail!("unsupported fixture entry type: {}", path.display());
}

#[cfg(unix)]
fn apply_rlimits(config: &RunnerConfig) -> std::io::Result<()> {
    unsafe {
        let cpu = libc::rlimit {
            rlim_cur: config.cpu_time_limit_seconds as libc::rlim_t,
            rlim_max: config.cpu_time_limit_seconds as libc::rlim_t,
        };
        if libc::setrlimit(libc::RLIMIT_CPU, &cpu) != 0 {
            return Err(std::io::Error::last_os_error());
        }

        let fsize = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::setrlimit(libc::RLIMIT_FSIZE, &fsize) != 0 {
            return Err(std::io::Error::last_os_error());
        }

        let nofile = libc::rlimit {
            rlim_cur: 32,
            rlim_max: 32,
        };
        if libc::setrlimit(libc::RLIMIT_NOFILE, &nofile) != 0 {
            return Err(std::io::Error::last_os_error());
        }

        let core = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::setrlimit(libc::RLIMIT_CORE, &core) != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

fn run_child(artifact_path: &Path, input: &[u8], config: &RunnerConfig) -> Result<ChildOutput> {
    let tmp = TempDir::new("x07_run").context("create tempdir")?;
    let artifact_abs = std::fs::canonicalize(artifact_path)
        .with_context(|| format!("canonicalize artifact path: {}", artifact_path.display()))?;

    setup_run_dir(&tmp, config)?;

    let mut child = {
        let mut cmd = Command::new(&artifact_abs);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.env_clear();
        cmd.current_dir(tmp.path());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            let cfg = config.clone();
            unsafe {
                cmd.pre_exec(move || apply_rlimits(&cfg));
            }
        }

        cmd.spawn()
            .with_context(|| format!("spawn artifact: {}", artifact_path.display()))?
    };

    let mut stdin = child.stdin.take().context("take stdin")?;
    let stdout = child.stdout.take().context("take stdout")?;
    let stderr = child.stderr.take().context("take stderr")?;

    let input_vec = encode_len_prefixed(input);
    let stdin_thread = std::thread::spawn(move || -> std::io::Result<()> {
        stdin.write_all(&input_vec)?;
        stdin.flush()?;
        drop(stdin);
        Ok(())
    });

    let stdout_cap = 4usize
        .saturating_add(config.max_output_bytes)
        .saturating_add(1);
    let stdout_thread = std::thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
        read_to_end_capped(stdout, stdout_cap)
    });

    let stderr_cap = 256usize * 1024;
    let stderr_thread = std::thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
        read_to_end_capped(stderr, stderr_cap)
    });

    let (status, timed_out) = wait_child_with_wall_timeout(&mut child, config)?;
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

fn wait_child_with_wall_timeout(
    child: &mut std::process::Child,
    config: &RunnerConfig,
) -> Result<(std::process::ExitStatus, bool)> {
    let wall_limit = Duration::from_secs(config.cpu_time_limit_seconds.saturating_add(1));
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

pub fn encode_len_prefixed(payload: &[u8]) -> Vec<u8> {
    let len: u32 = payload.len().try_into().unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn read_to_end_capped<R: Read>(mut reader: R, cap: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let mut truncated = false;

    loop {
        let n = reader.read(&mut tmp)?;
        if n == 0 {
            break;
        }

        if truncated {
            continue;
        }

        let remaining = cap.saturating_sub(buf.len());
        if n <= remaining {
            buf.extend_from_slice(&tmp[..n]);
        } else {
            buf.extend_from_slice(&tmp[..remaining]);
            truncated = true;
        }
    }

    Ok((buf, truncated))
}

fn hex_lower(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(LUT[(b >> 4) as usize] as char);
        out.push(LUT[(b & 0x0F) as usize] as char);
    }
    out
}

struct ChildOutput {
    exit_status: i32,
    exit_signal: Option<i32>,
    timed_out: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
}
