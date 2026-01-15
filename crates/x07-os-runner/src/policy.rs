use serde::Deserialize;
use x07_contracts::RUN_OS_POLICY_SCHEMA_VERSION;

fn default_true() -> bool {
    true
}

fn default_process_max_exe_bytes() -> u64 {
    4096
}

fn default_process_max_args() -> u64 {
    64
}

fn default_process_max_arg_bytes() -> u64 {
    4096
}

fn default_process_max_env() -> u64 {
    64
}

fn default_process_max_env_key_bytes() -> u64 {
    256
}

fn default_process_max_env_val_bytes() -> u64 {
    4096
}

fn validate_regex_lite_pattern(pat: &str) -> Result<(), String> {
    if pat.contains(';') {
        return Err("pattern must not contain ';'".to_string());
    }

    let bytes = pat.as_bytes();
    if bytes.is_empty() {
        return Err("pattern must be non-empty".to_string());
    }

    let mut i: usize = 0;
    while i < bytes.len() {
        if bytes[i] == b'*' {
            return Err("pattern has '*' with nothing to repeat".to_string());
        }

        if bytes[i] == b'\\' {
            if i + 1 >= bytes.len() {
                return Err("pattern has trailing '\\\\'".to_string());
            }
            i += 2;
        } else {
            i += 1;
        }

        if i < bytes.len() && bytes[i] == b'*' {
            i += 1;
        }
    }

    Ok(())
}

fn validate_no_semicolon(v: &str) -> Result<(), String> {
    if v.contains(';') {
        Err("value must not contain ';'".to_string())
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Language {
    #[serde(default)]
    pub allow_unsafe: bool,
    #[serde(default)]
    pub allow_ffi: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Policy {
    pub schema_version: String,
    pub policy_id: String,
    pub limits: Limits,
    pub fs: Fs,
    pub net: Net,
    #[serde(default)]
    pub db: Db,
    pub env: Env,
    pub time: Time,
    #[serde(default)]
    pub language: Language,
    pub process: Process,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Limits {
    pub cpu_ms: u64,
    pub wall_ms: u64,
    pub mem_bytes: u64,
    pub fds: u64,
    pub procs: u64,
    #[serde(default)]
    pub core_dumps: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Fs {
    pub enabled: bool,
    pub read_roots: Vec<String>,
    pub write_roots: Vec<String>,
    pub deny_hidden: bool,
    #[serde(default)]
    pub allow_symlinks: bool,
    #[serde(default)]
    pub allow_mkdir: bool,
    #[serde(default)]
    pub allow_remove: bool,
    #[serde(default)]
    pub allow_rename: bool,
    #[serde(default)]
    pub allow_walk: bool,
    #[serde(default)]
    pub allow_glob: bool,
    #[serde(default)]
    pub max_read_bytes: u32,
    #[serde(default)]
    pub max_write_bytes: u32,
    #[serde(default)]
    pub max_entries: u32,
    #[serde(default)]
    pub max_depth: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Net {
    pub enabled: bool,
    pub allow_dns: bool,
    pub allow_tcp: bool,
    pub allow_udp: bool,
    pub allow_hosts: Vec<NetHost>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetHost {
    pub host: String,
    pub ports: Vec<u16>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Db {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub drivers: DbDrivers,
    #[serde(default)]
    pub max_live_conns: u32,
    #[serde(default)]
    pub max_queries: u32,
    #[serde(default)]
    pub connect_timeout_ms: u32,
    #[serde(default)]
    pub query_timeout_ms: u32,
    #[serde(default)]
    pub max_sql_bytes: u32,
    #[serde(default)]
    pub max_rows: u32,
    #[serde(default)]
    pub max_resp_bytes: u32,
    #[serde(default)]
    pub sqlite: DbSqlite,
    #[serde(default)]
    pub net: DbNet,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DbDrivers {
    #[serde(default)]
    pub sqlite: bool,
    #[serde(default)]
    pub postgres: bool,
    #[serde(default)]
    pub mysql: bool,
    #[serde(default)]
    pub redis: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DbSqlite {
    #[serde(default)]
    pub allow_paths: Vec<String>,
    #[serde(default)]
    pub readonly_only: bool,
    #[serde(default)]
    pub allow_create: bool,
    #[serde(default)]
    pub allow_in_memory: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DbNet {
    #[serde(default)]
    pub allow_dns: Vec<String>,
    #[serde(default)]
    pub allow_cidrs: Vec<String>,
    #[serde(default)]
    pub allow_ports: Vec<u16>,
    #[serde(default = "default_true")]
    pub require_tls: bool,
    #[serde(default = "default_true")]
    pub require_verify: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Env {
    pub enabled: bool,
    pub allow_keys: Vec<String>,
    pub deny_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Time {
    pub enabled: bool,
    pub allow_monotonic: bool,
    pub allow_wall_clock: bool,
    pub allow_sleep: bool,
    pub max_sleep_ms: u64,
    pub allow_local_tzid: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Process {
    pub enabled: bool,
    pub allow_spawn: bool,
    pub max_live: u64,
    pub max_spawns: u64,
    #[serde(default)]
    pub allow_execs: Vec<String>,
    #[serde(default)]
    pub allow_exec_prefixes: Vec<String>,
    #[serde(default)]
    pub allow_args_regex_lite: Vec<String>,
    #[serde(default)]
    pub allow_env_keys: Vec<String>,
    pub allow_exec: bool,
    pub allow_exit: bool,

    #[serde(default = "default_process_max_exe_bytes")]
    pub max_exe_bytes: u64,
    #[serde(default = "default_process_max_args")]
    pub max_args: u64,
    #[serde(default = "default_process_max_arg_bytes")]
    pub max_arg_bytes: u64,
    #[serde(default = "default_process_max_env")]
    pub max_env: u64,
    #[serde(default = "default_process_max_env_key_bytes")]
    pub max_env_key_bytes: u64,
    #[serde(default = "default_process_max_env_val_bytes")]
    pub max_env_val_bytes: u64,

    #[serde(default)]
    pub max_runtime_ms: u64,

    #[serde(default)]
    pub max_stdout_bytes: u64,
    #[serde(default)]
    pub max_stderr_bytes: u64,
    #[serde(default)]
    pub max_total_bytes: u64,
    #[serde(default)]
    pub max_stdin_bytes: u64,

    #[serde(default = "default_true")]
    pub kill_on_drop: bool,
    #[serde(default = "default_true")]
    pub kill_tree: bool,

    #[serde(default)]
    pub allow_cwd: bool,
    #[serde(default)]
    pub allow_cwd_roots: Vec<String>,
}

impl Policy {
    pub fn validate_basic(&self) -> Result<(), String> {
        if self.schema_version.trim() != RUN_OS_POLICY_SCHEMA_VERSION {
            return Err(format!(
                "policy.schema_version mismatch: expected {} got {:?}",
                RUN_OS_POLICY_SCHEMA_VERSION, self.schema_version
            ));
        }
        if self.policy_id.trim().is_empty() {
            return Err("policy.policy_id must be non-empty".to_string());
        }
        if self.limits.cpu_ms == 0 || self.limits.cpu_ms > 600_000 {
            return Err(format!(
                "policy.limits.cpu_ms must be 1..600000 (got {})",
                self.limits.cpu_ms
            ));
        }
        if self.limits.wall_ms == 0 || self.limits.wall_ms > 600_000 {
            return Err(format!(
                "policy.limits.wall_ms must be 1..600000 (got {})",
                self.limits.wall_ms
            ));
        }
        if self.limits.mem_bytes < 1_048_576 || self.limits.mem_bytes > 17_179_869_184 {
            return Err(format!(
                "policy.limits.mem_bytes must be 1048576..17179869184 (got {})",
                self.limits.mem_bytes
            ));
        }
        if self.limits.fds > 1024 {
            return Err(format!(
                "policy.limits.fds must be 0..1024 (got {})",
                self.limits.fds
            ));
        }
        if self.limits.procs > 1024 {
            return Err(format!(
                "policy.limits.procs must be 0..1024 (got {})",
                self.limits.procs
            ));
        }
        let _ = self.limits.core_dumps;
        if self.process.allow_spawn
            && self.process.allow_execs.is_empty()
            && self.process.allow_exec_prefixes.is_empty()
        {
            return Err(
                "policy.process.allow_execs or policy.process.allow_exec_prefixes must be non-empty when allow_spawn is true".to_string(),
            );
        }
        if self.process.allow_spawn && self.process.max_live == 0 {
            return Err(
                "policy.process.max_live must be non-zero when allow_spawn is true".to_string(),
            );
        }
        if self.process.allow_spawn && self.process.max_spawns == 0 {
            return Err(
                "policy.process.max_spawns must be non-zero when allow_spawn is true".to_string(),
            );
        }
        if self.process.allow_spawn && self.process.max_runtime_ms == 0 {
            return Err(
                "policy.process.max_runtime_ms must be non-zero when allow_spawn is true"
                    .to_string(),
            );
        }
        if self.process.allow_cwd && self.process.allow_cwd_roots.is_empty() {
            return Err(
                "policy.process.allow_cwd_roots must be non-empty when allow_cwd is true"
                    .to_string(),
            );
        }
        if self.time.max_sleep_ms > 86_400_000 {
            return Err("policy.time.max_sleep_ms must be <= 86400000".to_string());
        }

        for (idx, v) in self.fs.read_roots.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.fs.read_roots[{idx}] is invalid: {e}"))?;
        }
        for (idx, v) in self.fs.write_roots.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.fs.write_roots[{idx}] is invalid: {e}"))?;
        }
        for (idx, v) in self.env.allow_keys.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.env.allow_keys[{idx}] is invalid: {e}"))?;
        }
        for (idx, v) in self.env.deny_keys.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.env.deny_keys[{idx}] is invalid: {e}"))?;
        }
        for (idx, v) in self.process.allow_execs.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.process.allow_execs[{idx}] is invalid: {e}"))?;
        }
        for (idx, v) in self.process.allow_exec_prefixes.iter().enumerate() {
            validate_no_semicolon(v).map_err(|e| {
                format!("policy.process.allow_exec_prefixes[{idx}] is invalid: {e}")
            })?;
        }
        for (idx, v) in self.process.allow_env_keys.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.process.allow_env_keys[{idx}] is invalid: {e}"))?;
        }
        for (idx, v) in self.process.allow_cwd_roots.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.process.allow_cwd_roots[{idx}] is invalid: {e}"))?;
        }

        if self.db.enabled
            && !(self.db.drivers.sqlite
                || self.db.drivers.postgres
                || self.db.drivers.mysql
                || self.db.drivers.redis)
        {
            return Err(
                "policy.db.drivers must allow at least one driver when policy.db.enabled is true"
                    .to_string(),
            );
        }
        for (idx, v) in self.db.sqlite.allow_paths.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.db.sqlite.allow_paths[{idx}] is invalid: {e}"))?;
        }
        for (idx, v) in self.db.net.allow_dns.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.db.net.allow_dns[{idx}] is invalid: {e}"))?;
        }
        for (idx, v) in self.db.net.allow_cidrs.iter().enumerate() {
            validate_no_semicolon(v)
                .map_err(|e| format!("policy.db.net.allow_cidrs[{idx}] is invalid: {e}"))?;
        }
        for (idx, port) in self.db.net.allow_ports.iter().enumerate() {
            if *port == 0 {
                return Err(format!("policy.db.net.allow_ports[{idx}] must be 1..65535"));
            }
        }

        if self.net.allow_hosts.len() > 256 {
            return Err("policy.net.allow_hosts must have <= 256 items".to_string());
        }
        for (idx, h) in self.net.allow_hosts.iter().enumerate() {
            let host = h.host.trim();
            if host.is_empty() {
                return Err(format!(
                    "policy.net.allow_hosts[{idx}].host must be non-empty"
                ));
            }
            if host.len() > 255 {
                return Err(format!(
                    "policy.net.allow_hosts[{idx}].host is too long (max 255 bytes)"
                ));
            }
            validate_no_semicolon(host)
                .map_err(|e| format!("policy.net.allow_hosts[{idx}].host is invalid: {e}"))?;
            if host.chars().any(|c| c.is_whitespace()) {
                return Err(format!(
                    "policy.net.allow_hosts[{idx}].host must not contain whitespace"
                ));
            }
            if host.contains('\0') {
                return Err(format!(
                    "policy.net.allow_hosts[{idx}].host must not contain NUL"
                ));
            }
            if h.ports.is_empty() {
                return Err(format!(
                    "policy.net.allow_hosts[{idx}].ports must be non-empty"
                ));
            }
            if h.ports.len() > 64 {
                return Err(format!(
                    "policy.net.allow_hosts[{idx}].ports must have <= 64 items"
                ));
            }
            for (pidx, port) in h.ports.iter().enumerate() {
                if *port == 0 {
                    return Err(format!(
                        "policy.net.allow_hosts[{idx}].ports[{pidx}] must be 1..65535"
                    ));
                }
            }
        }

        for (idx, pat) in self.process.allow_args_regex_lite.iter().enumerate() {
            validate_regex_lite_pattern(pat).map_err(|e| {
                format!("policy.process.allow_args_regex_lite[{idx}] is invalid: {e}")
            })?;
        }
        Ok(())
    }
}
