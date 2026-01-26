use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use jsonschema::Draft;
use serde::Serialize;
use serde_json::Value;
use x07_contracts::{RUN_OS_POLICY_SCHEMA_VERSION, X07_POLICY_INIT_REPORT_SCHEMA_VERSION};

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

const RUN_OS_POLICY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../schemas/run-os-policy.schema.json");

#[derive(Debug, Args)]
pub struct PolicyArgs {
    #[command(subcommand)]
    pub cmd: Option<PolicyCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum PolicyCommand {
    /// Generate a schema-valid base run-os-sandboxed policy template.
    Init(PolicyInitArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum PolicyTemplate {
    Cli,
    HttpClient,
    WebService,
    FsTool,
    SqliteApp,
    PostgresClient,
    Worker,
    WorkerParallel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum PolicyEmit {
    Report,
    Policy,
}

#[derive(Debug, Args)]
pub struct PolicyInitArgs {
    /// Base template to emit.
    #[arg(long, value_enum)]
    pub template: PolicyTemplate,

    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Output path for the generated policy.
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,

    /// Override the policy_id field (must match the schema regex).
    #[arg(long, value_name = "ID")]
    pub policy_id: Option<String>,

    /// Overwrite existing output file when it differs.
    #[arg(long)]
    pub force: bool,

    /// Stdout output mode.
    #[arg(long, value_enum, default_value_t = PolicyEmit::Report)]
    pub emit: PolicyEmit,

    /// Create parent directories for an explicit --out path.
    #[arg(long)]
    pub mkdir_out: bool,
}

#[derive(Debug, Serialize)]
struct PolicyInitReport {
    schema_version: &'static str,
    template: &'static str,
    project_root: String,
    out: String,
    status: &'static str,
    policy_id: String,
}

pub fn cmd_policy(args: PolicyArgs) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing policy subcommand (try --help)");
    };

    match cmd {
        PolicyCommand::Init(args) => cmd_policy_init(args),
    }
}

pub(crate) fn default_base_policy_rel_path(template: PolicyTemplate) -> &'static str {
    match template {
        PolicyTemplate::Cli => ".x07/policies/base/cli.sandbox.base.policy.json",
        PolicyTemplate::HttpClient => ".x07/policies/base/http-client.sandbox.base.policy.json",
        PolicyTemplate::WebService => ".x07/policies/base/web-service.sandbox.base.policy.json",
        PolicyTemplate::FsTool => ".x07/policies/base/fs-tool.sandbox.base.policy.json",
        PolicyTemplate::SqliteApp => ".x07/policies/base/sqlite-app.sandbox.base.policy.json",
        PolicyTemplate::PostgresClient => {
            ".x07/policies/base/postgres-client.sandbox.base.policy.json"
        }
        PolicyTemplate::Worker => ".x07/policies/base/worker.sandbox.base.policy.json",
        PolicyTemplate::WorkerParallel => {
            ".x07/policies/base/worker-parallel.sandbox.base.policy.json"
        }
    }
}

pub(crate) fn render_base_policy_template_bytes(
    template: PolicyTemplate,
    policy_id_override: Option<&str>,
) -> Result<Vec<u8>> {
    let policy = base_policy_template(template, policy_id_override);
    let (bytes, value) = canonical_policy_bytes(policy)?;
    if let Err(errors) = validate_run_os_policy_schema(&value) {
        anyhow::bail!("rendered base policy template is not schema-valid: {errors:?}");
    }
    Ok(bytes)
}

fn cmd_policy_init(args: PolicyInitArgs) -> Result<std::process::ExitCode> {
    if let Some(id) = args.policy_id.as_deref() {
        validate_policy_id(id).map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (project_root, _discovered_project) = resolve_project_root(&cwd, args.project.as_deref())?;
    let out_path = resolve_out_path(&project_root, &args);

    let policy = base_policy_template(args.template, args.policy_id.as_deref());
    let (policy_bytes, policy_value) = canonical_policy_bytes(policy)?;
    if let Err(diag) = validate_run_os_policy_schema(&policy_value) {
        print_x07diag(diag);
        return Ok(std::process::ExitCode::from(3));
    }

    let out_rel = rel(&project_root, &out_path);
    let status = match out_path.is_file() {
        false => {
            ensure_parent_dir_for_write(
                &project_root,
                &out_path,
                args.out.is_none(),
                args.mkdir_out,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            write_atomic(&out_path, &policy_bytes)?;
            "created"
        }
        true => {
            let existing = std::fs::read(&out_path)
                .with_context(|| format!("read: {}", out_path.display()))?;
            if existing == policy_bytes {
                "unchanged"
            } else if args.force {
                ensure_parent_dir_for_write(
                    &project_root,
                    &out_path,
                    args.out.is_none(),
                    args.mkdir_out,
                )
                .map_err(|e| anyhow::anyhow!("{e}"))?;
                write_atomic(&out_path, &policy_bytes)?;
                "overwritten"
            } else {
                "exists_different"
            }
        }
    };

    match args.emit {
        PolicyEmit::Policy => {
            std::io::Write::write_all(&mut std::io::stdout(), &policy_bytes)
                .context("write stdout")?;
        }
        PolicyEmit::Report => {
            let report = PolicyInitReport {
                schema_version: X07_POLICY_INIT_REPORT_SCHEMA_VERSION,
                template: match args.template {
                    PolicyTemplate::Cli => "cli",
                    PolicyTemplate::HttpClient => "http-client",
                    PolicyTemplate::WebService => "web-service",
                    PolicyTemplate::FsTool => "fs-tool",
                    PolicyTemplate::SqliteApp => "sqlite-app",
                    PolicyTemplate::PostgresClient => "postgres-client",
                    PolicyTemplate::Worker => "worker",
                    PolicyTemplate::WorkerParallel => "worker-parallel",
                },
                project_root: project_root.display().to_string(),
                out: out_rel,
                status,
                policy_id: policy_value
                    .get("policy_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            };
            let mut bytes = serde_json::to_vec(&report)?;
            bytes.push(b'\n');
            std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;
        }
    }

    let exit = match status {
        "created" | "unchanged" | "overwritten" => 0,
        "exists_different" => 2,
        _ => 2,
    };

    Ok(std::process::ExitCode::from(exit))
}

fn resolve_project_root(cwd: &Path, project: Option<&Path>) -> Result<(PathBuf, Option<PathBuf>)> {
    if let Some(project) = project {
        let project_path = if project.is_absolute() {
            project.to_path_buf()
        } else {
            cwd.join(project)
        };
        if !project_path.is_file() {
            anyhow::bail!("--project is not a file: {}", project_path.display());
        }
        let root = project_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        return Ok((root, Some(project_path)));
    }

    // Search upward from CWD for x07.json; if not found, default to CWD.
    let mut dir: Option<&Path> = Some(cwd);
    while let Some(d) = dir {
        let x07_json = d.join("x07.json");
        if x07_json.is_file() {
            return Ok((d.to_path_buf(), Some(x07_json)));
        }
        dir = d.parent();
    }
    Ok((cwd.to_path_buf(), None))
}

fn resolve_out_path(project_root: &Path, args: &PolicyInitArgs) -> PathBuf {
    let rel = match (args.out.as_deref(), args.template) {
        (Some(out), _) => out.to_path_buf(),
        (None, template) => PathBuf::from(default_base_policy_rel_path(template)),
    };

    if rel.is_absolute() {
        rel
    } else {
        project_root.join(rel)
    }
}

fn ensure_parent_dir_for_write(
    project_root: &Path,
    out: &Path,
    out_is_default: bool,
    mkdir_out: bool,
) -> Result<()> {
    let Some(parent) = out.parent() else {
        return Ok(());
    };

    if out_is_default {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output dir: {}", parent.display()))?;
        return Ok(());
    }

    if mkdir_out {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output dir: {}", parent.display()))?;
        return Ok(());
    }

    // If out is explicitly provided, only create its parents when asked.
    // We still allow writing if the directory already exists.
    if !parent.exists() {
        let rel_parent = rel(project_root, parent);
        anyhow::bail!("refusing to create output dir {rel_parent:?} (pass --mkdir-out to allow)");
    }

    Ok(())
}

fn validate_policy_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 64 {
        anyhow::bail!("--policy-id must be 1..=64 chars");
    }
    if id
        .as_bytes()
        .iter()
        .any(|&b| !(b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-'))
    {
        anyhow::bail!("--policy-id must match ^[a-zA-Z0-9_.-]{{1,64}}$");
    }
    Ok(())
}

fn base_process_policy() -> Value {
    serde_json::json!({
        "enabled": true,
        "allow_spawn": false,
        "max_live": 0,
        "max_spawns": 0,
        "allow_exec": false,
        "allow_exit": true,
        "allow_execs": [],
        "allow_exec_prefixes": [],
        "allow_args_regex_lite": [],
        "allow_env_keys": [],
        "max_exe_bytes": 4096,
        "max_args": 64,
        "max_arg_bytes": 4096,
        "max_env": 64,
        "max_env_key_bytes": 256,
        "max_env_val_bytes": 4096,
        "max_runtime_ms": 0,
        "max_stdout_bytes": 10485760,
        "max_stderr_bytes": 10485760,
        "max_total_bytes": 16777216,
        "max_stdin_bytes": 1048576,
        "kill_on_drop": true,
        "kill_tree": true,
        "allow_cwd": false,
        "allow_cwd_roots": []
    })
}

fn base_threads_policy() -> Value {
    serde_json::json!({
        "enabled": true,
        "max_workers": 0,
        "max_blocking": 4,
        "max_queue": 1024
    })
}

fn base_policy_template(template: PolicyTemplate, policy_id_override: Option<&str>) -> Value {
    let mut policy = match template {
        PolicyTemplate::Cli => serde_json::json!({
            "schema_version": RUN_OS_POLICY_SCHEMA_VERSION,
            "policy_id": "sandbox.cli.base",
            "notes": "Base policy for local CLI tools. No networking. Reads project tree; writes under out/.",
            "limits": { "cpu_ms": 60000, "wall_ms": 120000, "mem_bytes": 536870912, "fds": 128, "procs": 16, "core_dumps": false },
            "fs": {
              "enabled": true,
              "read_roots": ["."],
              "write_roots": ["out"],
              "deny_hidden": true,
              "allow_symlinks": false,
              "allow_mkdir": true,
              "allow_remove": false,
              "allow_rename": true,
              "allow_walk": true,
              "allow_glob": true,
              "max_read_bytes": 67108864,
              "max_write_bytes": 67108864,
              "max_entries": 50000,
              "max_depth": 64
            },
            "net": { "enabled": false, "allow_dns": false, "allow_tcp": false, "allow_udp": false, "allow_hosts": [] },
            "env": { "enabled": false, "allow_keys": [], "deny_keys": [] },
            "time": { "enabled": true, "allow_monotonic": true, "allow_wall_clock": true, "allow_sleep": false, "max_sleep_ms": 0, "allow_local_tzid": false },
            "language": { "allow_unsafe": false, "allow_ffi": false },
            "threads": base_threads_policy(),
            "process": base_process_policy()
        }),
        PolicyTemplate::HttpClient => serde_json::json!({
            "schema_version": RUN_OS_POLICY_SCHEMA_VERSION,
            "policy_id": "sandbox.http-client.base",
            "notes": "Base policy for outbound HTTP/TLS. net.allow_hosts is intentionally empty. Use x07 run --allow-host to derive a policy with explicit destinations.",
            "limits": { "cpu_ms": 180000, "wall_ms": 300000, "mem_bytes": 1073741824, "fds": 256, "procs": 32, "core_dumps": false },
            "fs": {
              "enabled": true,
              "read_roots": ["."],
              "write_roots": ["out"],
              "deny_hidden": true,
              "allow_symlinks": false,
              "allow_mkdir": true,
              "allow_remove": false,
              "allow_rename": true,
              "allow_walk": true,
              "allow_glob": false,
              "max_read_bytes": 67108864,
              "max_write_bytes": 268435456,
              "max_entries": 20000,
              "max_depth": 32
            },
            "net": { "enabled": true, "allow_dns": true, "allow_tcp": true, "allow_udp": false, "allow_hosts": [] },
            "env": { "enabled": false, "allow_keys": [], "deny_keys": [] },
            "time": { "enabled": true, "allow_monotonic": true, "allow_wall_clock": true, "allow_sleep": true, "max_sleep_ms": 5000, "allow_local_tzid": false },
            "language": { "allow_unsafe": true, "allow_ffi": true },
            "threads": base_threads_policy(),
            "process": base_process_policy()
        }),
        PolicyTemplate::WebService => serde_json::json!({
            "schema_version": RUN_OS_POLICY_SCHEMA_VERSION,
            "policy_id": "sandbox.web-service.base",
            "notes": "Base policy for web services. net.allow_hosts is intentionally empty. Allow bind/connect targets via x07 run --allow-host. Env restricted to PORT/HOST/LOG_LEVEL.",
            "limits": { "cpu_ms": 180000, "wall_ms": 600000, "mem_bytes": 1073741824, "fds": 512, "procs": 64, "core_dumps": false },
            "fs": {
              "enabled": true,
              "read_roots": ["."],
              "write_roots": ["out"],
              "deny_hidden": true,
              "allow_symlinks": false,
              "allow_mkdir": true,
              "allow_remove": false,
              "allow_rename": true,
              "allow_walk": true,
              "allow_glob": true,
              "max_read_bytes": 67108864,
              "max_write_bytes": 134217728,
              "max_entries": 50000,
              "max_depth": 64
            },
            "net": { "enabled": true, "allow_dns": true, "allow_tcp": true, "allow_udp": false, "allow_hosts": [] },
            "env": { "enabled": true, "allow_keys": ["PORT", "HOST", "LOG_LEVEL"], "deny_keys": [] },
            "time": { "enabled": true, "allow_monotonic": true, "allow_wall_clock": true, "allow_sleep": true, "max_sleep_ms": 1000, "allow_local_tzid": false },
            "language": { "allow_unsafe": true, "allow_ffi": true },
            "threads": base_threads_policy(),
            "process": base_process_policy()
        }),
        PolicyTemplate::FsTool => serde_json::json!({
            "schema_version": RUN_OS_POLICY_SCHEMA_VERSION,
            "policy_id": "sandbox.fs-tool.base",
            "notes": "Base policy for filesystem tools (formatters/generators). No networking. Reads project; writes limited to src/ and out/.",
            "limits": { "cpu_ms": 90000, "wall_ms": 180000, "mem_bytes": 536870912, "fds": 256, "procs": 16, "core_dumps": false },
            "fs": {
              "enabled": true,
              "read_roots": ["."],
              "write_roots": ["src", "out"],
              "deny_hidden": true,
              "allow_symlinks": false,
              "allow_mkdir": true,
              "allow_remove": false,
              "allow_rename": true,
              "allow_walk": true,
              "allow_glob": true,
              "max_read_bytes": 268435456,
              "max_write_bytes": 268435456,
              "max_entries": 200000,
              "max_depth": 128
            },
            "net": { "enabled": false, "allow_dns": false, "allow_tcp": false, "allow_udp": false, "allow_hosts": [] },
            "env": { "enabled": false, "allow_keys": [], "deny_keys": [] },
            "time": { "enabled": true, "allow_monotonic": true, "allow_wall_clock": true, "allow_sleep": false, "max_sleep_ms": 0, "allow_local_tzid": false },
            "language": { "allow_unsafe": false, "allow_ffi": false },
            "threads": base_threads_policy(),
            "process": base_process_policy()
        }),
        PolicyTemplate::SqliteApp => serde_json::json!({
            "schema_version": RUN_OS_POLICY_SCHEMA_VERSION,
            "policy_id": "sandbox.sqlite-app.base",
            "notes": "Base policy for local SQLite apps. No networking. SQLite allowed only at out/app.sqlite (and optionally in-memory).",
            "limits": { "cpu_ms": 120000, "wall_ms": 300000, "mem_bytes": 1073741824, "fds": 256, "procs": 32, "core_dumps": false },
            "fs": {
              "enabled": true,
              "read_roots": ["."],
              "write_roots": ["out"],
              "deny_hidden": true,
              "allow_symlinks": false,
              "allow_mkdir": true,
              "allow_remove": false,
              "allow_rename": true,
              "allow_walk": true,
              "allow_glob": true,
              "max_read_bytes": 134217728,
              "max_write_bytes": 268435456,
              "max_entries": 100000,
              "max_depth": 64
            },
            "net": { "enabled": false, "allow_dns": false, "allow_tcp": false, "allow_udp": false, "allow_hosts": [] },
            "db": {
              "enabled": true,
              "drivers": { "sqlite": true, "postgres": false, "mysql": false, "redis": false },
              "max_live_conns": 8,
              "max_queries": 10000,
              "connect_timeout_ms": 2000,
              "query_timeout_ms": 30000,
              "max_sql_bytes": 1048576,
              "max_rows": 100000,
              "max_resp_bytes": 16777216,
              "sqlite": {
                "allow_paths": ["out/app.sqlite"],
                "readonly_only": false,
                "allow_create": true,
                "allow_in_memory": true
              }
            },
            "env": { "enabled": false, "allow_keys": [], "deny_keys": [] },
            "time": { "enabled": true, "allow_monotonic": true, "allow_wall_clock": true, "allow_sleep": false, "max_sleep_ms": 0, "allow_local_tzid": false },
            "language": { "allow_unsafe": true, "allow_ffi": true },
            "threads": base_threads_policy(),
            "process": base_process_policy()
        }),
        PolicyTemplate::PostgresClient => serde_json::json!({
            "schema_version": RUN_OS_POLICY_SCHEMA_VERSION,
            "policy_id": "sandbox.postgres-client.base",
            "notes": "Base policy for Postgres clients. General net is disabled; DB access is enabled but DB allowlists start empty (deny-by-default).",
            "limits": { "cpu_ms": 120000, "wall_ms": 300000, "mem_bytes": 1073741824, "fds": 256, "procs": 32, "core_dumps": false },
            "fs": {
              "enabled": true,
              "read_roots": ["."],
              "write_roots": ["out"],
              "deny_hidden": true,
              "allow_symlinks": false,
              "allow_mkdir": true,
              "allow_remove": false,
              "allow_rename": true,
              "allow_walk": true,
              "allow_glob": true,
              "max_read_bytes": 67108864,
              "max_write_bytes": 67108864,
              "max_entries": 50000,
              "max_depth": 64
            },
            "net": { "enabled": false, "allow_dns": false, "allow_tcp": false, "allow_udp": false, "allow_hosts": [] },
            "db": {
              "enabled": true,
              "drivers": { "sqlite": false, "postgres": true, "mysql": false, "redis": false },
              "max_live_conns": 8,
              "max_queries": 10000,
              "connect_timeout_ms": 5000,
              "query_timeout_ms": 30000,
              "max_sql_bytes": 1048576,
              "max_rows": 100000,
              "max_resp_bytes": 16777216,
              "net": {
                "allow_dns": [],
                "allow_cidrs": [],
                "allow_ports": [],
                "require_tls": true,
                "require_verify": true
              }
            },
            "env": { "enabled": false, "allow_keys": [], "deny_keys": [] },
            "time": { "enabled": true, "allow_monotonic": true, "allow_wall_clock": true, "allow_sleep": false, "max_sleep_ms": 0, "allow_local_tzid": false },
            "language": { "allow_unsafe": true, "allow_ffi": true },
            "threads": base_threads_policy(),
            "process": base_process_policy()
        }),
        PolicyTemplate::Worker => serde_json::json!({
            "schema_version": RUN_OS_POLICY_SCHEMA_VERSION,
            "policy_id": "sandbox.worker.base",
            "notes": "Base policy for compute workers. No fs/net/env by default. Allows bounded sleep for backoff.",
            "limits": { "cpu_ms": 300000, "wall_ms": 300000, "mem_bytes": 1073741824, "fds": 64, "procs": 16, "core_dumps": false },
            "fs": {
              "enabled": false,
              "read_roots": [],
              "write_roots": [],
              "deny_hidden": true,
              "allow_symlinks": false,
              "allow_mkdir": false,
              "allow_remove": false,
              "allow_rename": false,
              "allow_walk": false,
              "allow_glob": false,
              "max_read_bytes": 0,
              "max_write_bytes": 0,
              "max_entries": 0,
              "max_depth": 0
            },
            "net": { "enabled": false, "allow_dns": false, "allow_tcp": false, "allow_udp": false, "allow_hosts": [] },
            "env": { "enabled": false, "allow_keys": [], "deny_keys": [] },
            "time": { "enabled": true, "allow_monotonic": true, "allow_wall_clock": false, "allow_sleep": true, "max_sleep_ms": 5000, "allow_local_tzid": false },
            "language": { "allow_unsafe": false, "allow_ffi": false },
            "threads": base_threads_policy(),
            "process": base_process_policy()
        }),
        PolicyTemplate::WorkerParallel => serde_json::json!({
            "schema_version": RUN_OS_POLICY_SCHEMA_VERSION,
            "policy_id": "sandbox.worker-parallel.base",
            "notes": "Base policy for compute workers that spawn subprocesses (parallelism). No fs/net/env by default. Allows bounded sleep for backoff. Allows exec under deps/x07/.",
            "limits": { "cpu_ms": 300000, "wall_ms": 300000, "mem_bytes": 1073741824, "fds": 128, "procs": 1024, "core_dumps": false },
            "fs": {
              "enabled": false,
              "read_roots": [],
              "write_roots": [],
              "deny_hidden": true,
              "allow_symlinks": false,
              "allow_mkdir": false,
              "allow_remove": false,
              "allow_rename": false,
              "allow_walk": false,
              "allow_glob": false,
              "max_read_bytes": 0,
              "max_write_bytes": 0,
              "max_entries": 0,
              "max_depth": 0
            },
            "net": { "enabled": false, "allow_dns": false, "allow_tcp": false, "allow_udp": false, "allow_hosts": [] },
            "env": { "enabled": false, "allow_keys": [], "deny_keys": [] },
            "time": { "enabled": true, "allow_monotonic": true, "allow_wall_clock": false, "allow_sleep": true, "max_sleep_ms": 5000, "allow_local_tzid": false },
            "language": { "allow_unsafe": false, "allow_ffi": false },
            "threads": base_threads_policy(),
            "process": {
              "enabled": true,
              "allow_spawn": true,
              "max_live": 16,
              "max_spawns": 64,
              "allow_exec": true,
              "allow_exit": true,
              "allow_execs": [],
              "allow_exec_prefixes": ["deps/x07/"],
              "allow_args_regex_lite": [],
              "allow_env_keys": [],
              "max_exe_bytes": 4096,
              "max_args": 64,
              "max_arg_bytes": 4096,
              "max_env": 64,
              "max_env_key_bytes": 256,
              "max_env_val_bytes": 4096,
              "max_runtime_ms": 60000,
              "max_stdout_bytes": 10485760,
              "max_stderr_bytes": 10485760,
              "max_total_bytes": 16777216,
              "max_stdin_bytes": 1048576,
              "kill_on_drop": true,
              "kill_tree": true,
              "allow_cwd": false,
              "allow_cwd_roots": []
            }
        }),
    };

    if let Some(policy_id_override) = policy_id_override {
        if let Some(obj) = policy.as_object_mut() {
            obj.insert(
                "policy_id".to_string(),
                Value::String(policy_id_override.to_string()),
            );
        }
    }

    policy
}

fn canonical_policy_bytes(mut policy: Value) -> Result<(Vec<u8>, Value)> {
    x07c::x07ast::canon_value_jcs(&mut policy);
    let mut bytes = serde_json::to_vec_pretty(&policy)?;
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    Ok((bytes, policy))
}

fn validate_run_os_policy_schema(doc: &Value) -> std::result::Result<(), Vec<String>> {
    let schema_json: Value =
        serde_json::from_slice(RUN_OS_POLICY_SCHEMA_BYTES).expect("parse run-os-policy schema");
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema_json)
        .expect("build run-os-policy schema validator");

    let errors: Vec<String> = validator
        .iter_errors(doc)
        .map(|err| format!("{} ({})", err, err.instance_path()))
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn print_x07diag(errors: Vec<String>) {
    use x07c::diagnostics::{Diagnostic, Report, Severity, Stage};

    let diagnostics = errors
        .into_iter()
        .map(|message| Diagnostic {
            code: "X07-POLICY-SCHEMA-0001".to_string(),
            severity: Severity::Error,
            stage: Stage::Parse,
            message,
            loc: None,
            notes: Vec::new(),
            related: Vec::new(),
            data: Default::default(),
            quickfix: None,
        })
        .collect();

    let report = Report {
        schema_version: x07_contracts::X07DIAG_SCHEMA_VERSION.to_string(),
        ok: false,
        diagnostics,
        meta: Default::default(),
    };

    if let Ok(mut bytes) = serde_json::to_vec(&report) {
        bytes.push(b'\n');
        let _ = std::io::Write::write_all(&mut std::io::stdout(), &bytes);
    }
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

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
