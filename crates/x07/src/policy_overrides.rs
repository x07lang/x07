use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use jsonschema::Draft;
use serde_json::Value;
use sha2::{Digest, Sha256};
use x07_worlds::WorldId;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const RUN_OS_POLICY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../schemas/run-os-policy.schema.json");

#[derive(Debug, Clone, Default)]
pub(crate) struct PolicyOverrides {
    pub allow_host: Vec<String>,
    pub allow_host_file: Vec<PathBuf>,
    pub deny_host: Vec<String>,
    pub deny_host_file: Vec<PathBuf>,
    pub allow_read_root: Vec<String>,
    pub allow_write_root: Vec<String>,
}

impl PolicyOverrides {
    pub(crate) fn has_any(&self) -> bool {
        !self.allow_host.is_empty()
            || !self.allow_host_file.is_empty()
            || !self.deny_host.is_empty()
            || !self.deny_host_file.is_empty()
            || !self.allow_read_root.is_empty()
            || !self.allow_write_root.is_empty()
    }
}

#[derive(Debug)]
pub(crate) enum PolicyResolution {
    None,
    Base(PathBuf),
    Derived { derived: PathBuf },
    SchemaInvalid(Vec<String>),
}

pub(crate) fn resolve_policy_for_world(
    world: WorldId,
    policy_root: &Path,
    cli_policy: Option<PathBuf>,
    profile_policy: Option<PathBuf>,
    overrides: &PolicyOverrides,
) -> Result<PolicyResolution> {
    let has_policy_overrides = overrides.has_any();

    if has_policy_overrides && world != WorldId::RunOsSandboxed {
        anyhow::bail!("--allow-host/--deny-host/--allow-read-root/--allow-write-root requires run-os-sandboxed (policy-enforced OS world)");
    }

    if cli_policy.is_some() && world != WorldId::RunOsSandboxed {
        anyhow::bail!("--policy is only valid for --world run-os-sandboxed");
    }

    if world != WorldId::RunOsSandboxed {
        return Ok(PolicyResolution::None);
    }

    let base_policy = cli_policy
        .or(profile_policy)
        .context("run-os-sandboxed requires a policy file (--policy or profile policy)")?;
    let base_policy = resolve_project_relative(policy_root, &base_policy);

    if !base_policy.is_file() {
        anyhow::bail!("missing policy file: {}", base_policy.display());
    }

    if !has_policy_overrides {
        return Ok(PolicyResolution::Base(base_policy));
    }

    derive_policy_with_overrides(policy_root, &base_policy, overrides)
}

#[derive(Debug, Clone)]
struct DenySpec {
    all_ports: bool,
    ports: BTreeSet<u16>,
}

fn derive_policy_with_overrides(
    policy_root: &Path,
    base_policy_path: &Path,
    overrides: &PolicyOverrides,
) -> Result<PolicyResolution> {
    let base_bytes = match std::fs::read(base_policy_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            anyhow::bail!("read policy: {}: {err}", base_policy_path.display());
        }
    };

    let base_policy: Value = match serde_json::from_slice(&base_bytes) {
        Ok(v) => v,
        Err(err) => {
            return Ok(PolicyResolution::SchemaInvalid(vec![format!(
                "parse policy JSON: {err}"
            )]));
        }
    };

    let base_schema_errors = validate_run_os_policy_schema(&base_policy);
    if !base_schema_errors.is_empty() {
        return Ok(PolicyResolution::SchemaInvalid(base_schema_errors));
    }

    let base_policy_id = base_policy
        .get("policy_id")
        .and_then(Value::as_str)
        .context("policy.policy_id must be a string")?
        .to_string();

    let net_enabled = base_policy
        .pointer("/net/enabled")
        .and_then(Value::as_bool)
        .context("policy.net.enabled must be a bool")?;
    if !net_enabled
        && (!overrides.allow_host.is_empty()
            || !overrides.allow_host_file.is_empty()
            || !overrides.deny_host.is_empty()
            || !overrides.deny_host_file.is_empty())
    {
        anyhow::bail!("base policy disables networking (net.enabled=false)");
    }

    let allow_tcp = base_policy
        .pointer("/net/allow_tcp")
        .and_then(Value::as_bool)
        .context("policy.net.allow_tcp must be a bool")?;
    if !allow_tcp
        && (!overrides.allow_host.is_empty()
            || !overrides.allow_host_file.is_empty()
            || !overrides.deny_host.is_empty()
            || !overrides.deny_host_file.is_empty())
    {
        anyhow::bail!("base policy forbids TCP (net.allow_tcp=false)");
    }

    let fs_enabled = base_policy
        .pointer("/fs/enabled")
        .and_then(Value::as_bool)
        .context("policy.fs.enabled must be a bool")?;
    if !fs_enabled
        && (!overrides.allow_read_root.is_empty() || !overrides.allow_write_root.is_empty())
    {
        anyhow::bail!("base policy disables filesystem access (fs.enabled=false)");
    }

    let mut allow_map: HashMap<String, BTreeSet<u16>> = HashMap::new();
    for path in &overrides.allow_host_file {
        for spec in read_host_specs_from_file(policy_root, path)? {
            let (host, ports) = parse_allow_host_spec(&spec)?;
            allow_map.entry(host).or_default().extend(ports);
        }
    }
    for spec in &overrides.allow_host {
        let (host, ports) = parse_allow_host_spec(spec)?;
        allow_map.entry(host).or_default().extend(ports);
    }

    let mut deny_map: HashMap<String, DenySpec> = HashMap::new();
    for path in &overrides.deny_host_file {
        for spec in read_host_specs_from_file(policy_root, path)? {
            let (host, deny) = parse_deny_host_spec(&spec)?;
            merge_deny_spec(&mut deny_map, host, deny);
        }
    }
    for spec in &overrides.deny_host {
        let (host, deny) = parse_deny_host_spec(spec)?;
        merge_deny_spec(&mut deny_map, host, deny);
    }

    let mut allow_read_roots = normalize_roots(&overrides.allow_read_root)?;
    let mut allow_write_roots = normalize_roots(&overrides.allow_write_root)?;

    // Canonicalize overrides for hashing.
    let mut allow_hosts_digest: Vec<Value> = allow_map
        .iter()
        .map(|(host, ports)| {
            let mut ports: Vec<u16> = ports.iter().copied().collect();
            ports.sort_unstable();
            Value::Object(
                [
                    ("host".to_string(), Value::String(host.clone())),
                    (
                        "ports".to_string(),
                        Value::Array(ports.into_iter().map(Value::from).collect()),
                    ),
                ]
                .into_iter()
                .collect(),
            )
        })
        .collect();
    allow_hosts_digest.sort_by(|a, b| {
        let ah = a.get("host").and_then(Value::as_str).unwrap_or("");
        let bh = b.get("host").and_then(Value::as_str).unwrap_or("");
        ah.cmp(bh)
    });

    let mut deny_hosts_digest: Vec<Value> = deny_map
        .iter()
        .map(|(host, deny)| {
            let mut ports: Vec<u16> = deny.ports.iter().copied().collect();
            ports.sort_unstable();
            Value::Object(
                [
                    ("host".to_string(), Value::String(host.clone())),
                    ("all_ports".to_string(), Value::Bool(deny.all_ports)),
                    (
                        "ports".to_string(),
                        Value::Array(ports.into_iter().map(Value::from).collect()),
                    ),
                ]
                .into_iter()
                .collect(),
            )
        })
        .collect();
    deny_hosts_digest.sort_by(|a, b| {
        let ah = a.get("host").and_then(Value::as_str).unwrap_or("");
        let bh = b.get("host").and_then(Value::as_str).unwrap_or("");
        ah.cmp(bh)
    });

    allow_read_roots.sort();
    allow_read_roots.dedup();
    allow_write_roots.sort();
    allow_write_roots.dedup();

    let mut overrides_value = Value::Object(
        [
            ("allow_hosts".to_string(), Value::Array(allow_hosts_digest)),
            ("deny_hosts".to_string(), Value::Array(deny_hosts_digest)),
            (
                "allow_read_roots".to_string(),
                Value::Array(
                    allow_read_roots
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
            (
                "allow_write_roots".to_string(),
                Value::Array(
                    allow_write_roots
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );
    x07c::x07ast::canon_value_jcs(&mut overrides_value);
    let overrides_bytes = serde_json::to_vec(&overrides_value)?;

    let mut hasher = Sha256::new();
    hasher.update(&base_bytes);
    hasher.update(&overrides_bytes);
    let digest = hasher.finalize();
    let digest8 = hex8(&digest);

    let derived_dir = policy_root.join(".x07/policies/_generated");
    std::fs::create_dir_all(&derived_dir)
        .with_context(|| format!("create dir: {}", derived_dir.display()))?;

    let derived_path = derived_dir.join(format!("{base_policy_id}.g{digest8}.policy.json"));

    let mut derived_policy = base_policy;
    apply_policy_net_overrides(&mut derived_policy, &allow_map, &deny_map)?;
    apply_policy_fs_overrides(
        &mut derived_policy,
        &overrides.allow_read_root,
        &overrides.allow_write_root,
    )?;
    apply_policy_id_and_notes(
        &mut derived_policy,
        &base_policy_id,
        &digest8,
        base_policy_path,
    );

    let derived_schema_errors = validate_run_os_policy_schema(&derived_policy);
    if !derived_schema_errors.is_empty() {
        return Ok(PolicyResolution::SchemaInvalid(derived_schema_errors));
    }

    let mut derived_value = derived_policy;
    x07c::x07ast::canon_value_jcs(&mut derived_value);
    let mut derived_bytes = serde_json::to_vec_pretty(&derived_value)?;
    if derived_bytes.last() != Some(&b'\n') {
        derived_bytes.push(b'\n');
    }

    if derived_path.is_file() {
        if let Ok(existing) = std::fs::read(&derived_path) {
            if existing == derived_bytes {
                return Ok(PolicyResolution::Derived {
                    derived: derived_path,
                });
            }
        }
    }

    write_atomic_next_to(&derived_path, &derived_bytes)?;

    Ok(PolicyResolution::Derived {
        derived: derived_path,
    })
}

pub(crate) fn validate_run_os_policy_schema(doc: &Value) -> Vec<String> {
    let schema_json: Value = match serde_json::from_slice(RUN_OS_POLICY_SCHEMA_BYTES) {
        Ok(v) => v,
        Err(err) => return vec![format!("parse run-os-policy schema: {err}")],
    };
    let validator = match jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema_json)
    {
        Ok(v) => v,
        Err(err) => return vec![format!("build run-os-policy schema validator: {err}")],
    };

    validator
        .iter_errors(doc)
        .map(|err| format!("{} ({})", err, err.instance_path()))
        .collect()
}

pub(crate) fn print_policy_schema_x07diag_stderr(errors: Vec<String>) {
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
        let _ = std::io::Write::write_all(&mut std::io::stderr(), &bytes);
    }
}

fn resolve_project_relative(project_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

fn write_atomic_next_to(path: &Path, contents: &[u8]) -> Result<()> {
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

fn hex8(digest: &[u8]) -> String {
    let mut out = String::with_capacity(8);
    for b in digest.iter().take(4) {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn read_host_specs_from_file(root: &Path, path: &Path) -> Result<Vec<String>> {
    let path = resolve_project_relative(root, path);
    let bytes = std::fs::read(&path).with_context(|| format!("read: {}", path.display()))?;
    let s = std::str::from_utf8(&bytes).context("host spec file must be utf-8")?;
    let mut out = Vec::new();
    for line in s.lines() {
        let mut line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((before, _)) = line.split_once('#') {
            line = before.trim();
            if line.is_empty() {
                continue;
            }
        }
        out.push(line.to_string());
    }
    Ok(out)
}

fn parse_allow_host_spec(raw: &str) -> Result<(String, BTreeSet<u16>)> {
    let (host_raw, ports_raw) =
        split_host_ports(raw).context("Expected HOST:PORTS (ports are 1..65535).")?;
    if ports_raw.trim() == "*" {
        anyhow::bail!(
            "Expected HOST:PORTS (ports are 1..65535; '*' is only valid for --deny-host)."
        );
    }
    let host = normalize_host_token(host_raw)?;
    let ports = parse_ports_list(ports_raw)?;
    Ok((host, ports))
}

fn parse_deny_host_spec(raw: &str) -> Result<(String, DenySpec)> {
    let (host_raw, ports_raw) =
        split_host_ports(raw).context("Expected HOST:PORTS (ports are 1..65535 or *).")?;
    let host = normalize_host_token(host_raw)?;
    let ports_raw = ports_raw.trim();
    if ports_raw == "*" {
        return Ok((
            host,
            DenySpec {
                all_ports: true,
                ports: BTreeSet::new(),
            },
        ));
    }
    let ports = parse_ports_list(ports_raw)?;
    Ok((
        host,
        DenySpec {
            all_ports: false,
            ports,
        },
    ))
}

fn split_host_ports(raw: &str) -> Result<(&str, &str)> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("Expected HOST:PORTS.");
    }
    if raw.starts_with('[') {
        let close = raw.find(']').context("Expected HOST:PORTS.")?;
        let after = &raw[close + 1..];
        let ports = after.strip_prefix(':').context("Expected HOST:PORTS.")?;
        let host = &raw[1..close];
        if host.is_empty() || ports.trim().is_empty() {
            anyhow::bail!("Expected HOST:PORTS.");
        }
        return Ok((host, ports));
    }

    let idx = raw.rfind(':').context("Expected HOST:PORTS.")?;
    let (host, ports) = raw.split_at(idx);
    let ports = &ports[1..];
    if host.trim().is_empty() || ports.trim().is_empty() {
        anyhow::bail!("Expected HOST:PORTS.");
    }
    Ok((host, ports))
}

fn normalize_host_token(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("host must be non-empty");
    }
    if raw.len() > 255 {
        anyhow::bail!("host must be <= 255 chars");
    }
    if raw
        .as_bytes()
        .iter()
        .any(|&b| b == b';' || b.is_ascii_whitespace())
    {
        anyhow::bail!("host must not contain whitespace or semicolons");
    }
    let mut out = raw.to_string();
    out.make_ascii_lowercase();
    Ok(out)
}

fn parse_ports_list(raw: &str) -> Result<BTreeSet<u16>> {
    let mut out: BTreeSet<u16> = BTreeSet::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            anyhow::bail!("Expected HOST:PORTS (ports are 1..65535).");
        }
        let port: u16 = part
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected HOST:PORTS (ports are 1..65535)."))?;
        if port == 0 {
            anyhow::bail!("ports are 1..65535");
        }
        out.insert(port);
    }
    if out.len() > 64 {
        anyhow::bail!("ports list would exceed 64 entries");
    }
    Ok(out)
}

fn merge_deny_spec(map: &mut HashMap<String, DenySpec>, host: String, deny: DenySpec) {
    map.entry(host)
        .and_modify(|existing| {
            if deny.all_ports {
                existing.all_ports = true;
                existing.ports.clear();
            } else if !existing.all_ports {
                existing.ports.extend(deny.ports.iter().copied());
            }
        })
        .or_insert(deny);
}

fn normalize_roots(roots: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for raw in roots {
        let s = raw.trim();
        if s.is_empty() {
            anyhow::bail!("root path must be non-empty");
        }
        if s.len() > 4096 {
            anyhow::bail!("root path must be <= 4096 chars");
        }
        out.push(s.to_string());
    }
    Ok(out)
}

fn apply_policy_net_overrides(
    policy: &mut Value,
    allow_map: &HashMap<String, BTreeSet<u16>>,
    deny_map: &HashMap<String, DenySpec>,
) -> Result<()> {
    let base_allow_dns = policy
        .pointer("/net/allow_dns")
        .and_then(Value::as_bool)
        .context("policy.net.allow_dns must be a bool")?;

    let base_allow_hosts = policy
        .pointer("/net/allow_hosts")
        .and_then(Value::as_array)
        .context("policy.net.allow_hosts must be an array")?;

    let mut ordered_hosts: Vec<String> = Vec::new();
    let mut allowed: HashMap<String, BTreeSet<u16>> = HashMap::new();

    for entry in base_allow_hosts {
        let host_raw = entry
            .get("host")
            .and_then(Value::as_str)
            .context("policy.net.allow_hosts[].host must be a string")?;
        let host = normalize_host_token(host_raw)?;
        if !allowed.contains_key(&host) {
            ordered_hosts.push(host.clone());
            allowed.insert(host.clone(), BTreeSet::new());
        }

        let ports_val = entry
            .get("ports")
            .and_then(Value::as_array)
            .context("policy.net.allow_hosts[].ports must be an array")?;
        let ports_set = allowed.get_mut(&host).expect("inserted");
        for port in ports_val {
            let port = port
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .context("policy.net.allow_hosts[].ports must be u16")?;
            if port == 0 {
                anyhow::bail!("policy contains port 0");
            }
            ports_set.insert(port);
        }
        if ports_set.len() > 64 {
            anyhow::bail!("Host {host} would exceed 64 allowed ports");
        }
    }

    for (host, ports) in allow_map {
        if !allowed.contains_key(host) {
            ordered_hosts.push(host.clone());
            allowed.insert(host.clone(), BTreeSet::new());
            if ordered_hosts.len() > 256 {
                anyhow::bail!("Policy net.allow_hosts would exceed 256 entries");
            }
        }
        let set = allowed.get_mut(host).expect("present");
        set.extend(ports.iter().copied());
        if set.len() > 64 {
            anyhow::bail!("Host {host} would exceed 64 allowed ports");
        }
    }

    for (host, deny) in deny_map {
        if deny.all_ports {
            allowed.remove(host);
            continue;
        }
        let Some(set) = allowed.get_mut(host) else {
            continue;
        };
        for port in &deny.ports {
            set.remove(port);
        }
        if set.is_empty() {
            allowed.remove(host);
        }
    }

    let allow_dns_final = base_allow_dns
        || allowed
            .keys()
            .any(|h| h.as_bytes().iter().any(|b| b.is_ascii_alphabetic()));

    let mut out_allow_hosts: Vec<Value> = Vec::new();
    for host in ordered_hosts {
        let Some(ports) = allowed.get(&host) else {
            continue;
        };
        let ports: Vec<Value> = ports.iter().copied().map(Value::from).collect();
        out_allow_hosts.push(Value::Object(
            [
                ("host".to_string(), Value::String(host)),
                ("ports".to_string(), Value::Array(ports)),
            ]
            .into_iter()
            .collect(),
        ));
    }

    *policy
        .pointer_mut("/net/allow_dns")
        .context("missing policy.net.allow_dns")? = Value::Bool(allow_dns_final);
    *policy
        .pointer_mut("/net/allow_hosts")
        .context("missing policy.net.allow_hosts")? = Value::Array(out_allow_hosts);

    Ok(())
}

fn apply_policy_fs_overrides(
    policy: &mut Value,
    allow_read_roots: &[String],
    allow_write_roots: &[String],
) -> Result<()> {
    if allow_read_roots.is_empty() && allow_write_roots.is_empty() {
        return Ok(());
    }

    let fs_enabled = policy
        .pointer("/fs/enabled")
        .and_then(Value::as_bool)
        .context("policy.fs.enabled must be a bool")?;
    if !fs_enabled {
        anyhow::bail!("base policy disables filesystem access (fs.enabled=false)");
    }

    let mut read_roots: Vec<String> = policy
        .pointer("/fs/read_roots")
        .and_then(Value::as_array)
        .context("policy.fs.read_roots must be an array")?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    let mut write_roots: Vec<String> = policy
        .pointer("/fs/write_roots")
        .and_then(Value::as_array)
        .context("policy.fs.write_roots must be an array")?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let mut seen_read: HashSet<String> = read_roots.iter().cloned().collect();
    for root in allow_read_roots
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if root.len() > 4096 {
            anyhow::bail!("root path must be <= 4096 chars");
        }
        if seen_read.insert(root.to_string()) {
            read_roots.push(root.to_string());
        }
    }
    if read_roots.len() > 128 {
        anyhow::bail!("fs.read_roots would exceed 128 entries");
    }

    let mut seen_write: HashSet<String> = write_roots.iter().cloned().collect();
    for root in allow_write_roots
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if root.len() > 4096 {
            anyhow::bail!("root path must be <= 4096 chars");
        }
        if seen_write.insert(root.to_string()) {
            write_roots.push(root.to_string());
        }
    }
    if write_roots.len() > 128 {
        anyhow::bail!("fs.write_roots would exceed 128 entries");
    }

    *policy
        .pointer_mut("/fs/read_roots")
        .context("missing policy.fs.read_roots")? =
        Value::Array(read_roots.into_iter().map(Value::String).collect());
    *policy
        .pointer_mut("/fs/write_roots")
        .context("missing policy.fs.write_roots")? =
        Value::Array(write_roots.into_iter().map(Value::String).collect());

    Ok(())
}

fn apply_policy_id_and_notes(
    policy: &mut Value,
    base_policy_id: &str,
    digest8: &str,
    base_path: &Path,
) {
    let suffix = format!(".g{digest8}");
    let max_base_len = 64usize.saturating_sub(suffix.len());
    let truncated = if base_policy_id.len() > max_base_len {
        &base_policy_id[..max_base_len]
    } else {
        base_policy_id
    };
    let derived_id = format!("{truncated}{suffix}");
    if let Some(v) = policy.pointer_mut("/policy_id") {
        *v = Value::String(derived_id);
    }

    let line = format!(
        "Derived by x07 run from `{}` (g{digest8})",
        base_path.display()
    );
    let Some(notes_val) = policy.pointer_mut("/notes") else {
        policy
            .as_object_mut()
            .map(|obj| obj.insert("notes".to_string(), Value::String(line)));
        return;
    };
    let existing = notes_val.as_str().unwrap_or("");
    if existing.is_empty() {
        *notes_val = Value::String(line);
        return;
    }
    let candidate = format!("{existing}\n{line}");
    if candidate.len() <= 4096 {
        *notes_val = Value::String(candidate);
    }
}
