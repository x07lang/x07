use std::collections::BTreeMap;
use std::ffi::CString;
use std::io::{Read, Write};
use std::net::IpAddr;
use std::net::ToSocketAddrs as _;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::io::{FromRawFd as _, RawFd};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::Deserialize;
use x07_contracts::X07_OS_RUNNER_REPORT_SCHEMA_VERSION;

const PORT_STDOUT: u32 = 5000;
const PORT_STDERR: u32 = 5001;
const PORT_CTRL: u32 = 5002;

const EXIT_REQUEST_INVALID: i32 = 120;
const EXIT_MOUNT_FAILED: i32 = 121;
const EXIT_VSOCK_FAILED: i32 = 122;
const EXIT_EXEC_FAILED: i32 = 123;
const EXIT_INTERNAL_ERROR: i32 = 124;

const FLAG_REPORT_COMPLETE: u32 = 0x0000_0001;
const FLAG_STDOUT_TRUNCATED: u32 = 0x0000_0002;
const FLAG_STDERR_TRUNCATED: u32 = 0x0000_0004;
const FLAG_CONTRACT_ERROR: u32 = 0x0000_0008;
const FLAG_METRICS_PRESENT: u32 = 0x0000_0010;
const FLAG_GUEST_FAILURE_WRITTEN: u32 = 0x0000_0020;

const ENV_GUESTD_WORKDIR: &str = "X07_GUESTD_WORKDIR";
const CMDLINE_RUN_ID_KEY: &str = "x07.run_id";
const POLICY_PATH: &str = "/x07/in/policy.json";
const NFT_TABLE_NAME: &str = "x07";

#[derive(Debug, Deserialize)]
struct PolicyLite {
    net: PolicyNet,
}

#[derive(Debug, Deserialize)]
struct PolicyNet {
    enabled: bool,
    allow_dns: bool,
    allow_tcp: bool,
    allow_udp: bool,
    #[serde(default)]
    allow_hosts: Vec<PolicyNetHost>,
}

#[derive(Debug, Deserialize)]
struct PolicyNetHost {
    host: String,
    ports: Vec<u16>,
}

#[derive(Debug, Deserialize)]
struct GuestRequest {
    schema_version: String,
    run_id: String,
    #[serde(default)]
    mounts: Vec<GuestMount>,
    exec: GuestExec,
    #[serde(default)]
    expect: GuestExpect,
    #[serde(default)]
    limits: GuestLimits,
}

#[derive(Debug, Deserialize)]
struct GuestMount {
    tag: String,
    guest_path: String,
    readonly: bool,
}

#[derive(Debug, Deserialize)]
struct GuestExec {
    path: String,
    argv: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, Default)]
struct GuestExpect {
    #[serde(default = "default_expect_schema_version")]
    stdout_schema_version: String,
    #[serde(default = "default_true")]
    stdout_is_single_json_object: bool,
}

fn default_expect_schema_version() -> String {
    X07_OS_RUNNER_REPORT_SCHEMA_VERSION.to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
struct GuestLimits {
    #[allow(dead_code)]
    wall_ms: Option<u64>,
    stdout_max_bytes: Option<u64>,
    stderr_max_bytes: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
struct GuestFailure {
    schema_version: &'static str,
    run_id: String,
    failure_kind: &'static str,
    stage: &'static str,
    when_unix_ms: u64,
    tag: Option<String>,
    mountpoint: Option<String>,
    guest_path: Option<String>,
    readonly: Option<bool>,
    errno: Option<i32>,
    message: String,
    detail: Option<String>,
}

pub fn run() -> i32 {
    match try_main() {
        Ok(code) => code,
        Err(err) => {
            let _ = writeln!(&mut std::io::stderr(), "{err:#}");
            EXIT_INTERNAL_ERROR
        }
    }
}

fn try_main() -> Result<i32> {
    let start = Instant::now();

    ensure_dir(Path::new("/x07/m")).context("ensure /x07/m")?;

    // Best-effort: mount proc/sys/dev for a predictable environment.
    let _ = ensure_dir(Path::new("/proc"));
    let _ = ensure_dir(Path::new("/sys"));
    let _ = ensure_dir(Path::new("/dev"));
    let _ = mount_fs("proc", "/proc", "proc", 0, None);
    let _ = mount_fs("sysfs", "/sys", "sysfs", 0, None);
    let _ = mount_fs("devtmpfs", "/dev", "devtmpfs", 0, None);

    let cmdline_run_id = read_run_id_from_cmdline().unwrap_or_else(|| "unknown".to_string());

    // v1.1 ordering: mount x07out first so we can write guest_failure.json on later mount errors.
    ensure_dir(Path::new("/x07/out")).context("create /x07/out mountpoint")?;
    if let Err(err) = mount_virtiofs("x07out", Path::new("/x07/out"), false) {
        // Can't write guest_failure.json without x07out; still try to send CTRL.
        let flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
        let _ = try_send_ctrl_only(EXIT_MOUNT_FAILED, flags, start, 0, 0);
        let _ = writeln!(&mut std::io::stderr(), "mount_x07out failed: {err:#}");
        return Ok(EXIT_MOUNT_FAILED);
    }
    let out_mounted = true;

    ensure_dir(Path::new("/x07/in")).context("create /x07/in mountpoint")?;
    if let Err(err) = mount_virtiofs("x07in", Path::new("/x07/in"), true) {
        let written = write_guest_failure(
            &cmdline_run_id,
            "mount_x07in",
            "x07in",
            Some("/x07/in"),
            None,
            Some(true),
            &err,
            out_mounted,
        )
        .unwrap_or(false);
        let mut flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
        if written {
            flags |= FLAG_GUEST_FAILURE_WRITTEN;
        }
        let _ = try_send_ctrl_only(EXIT_MOUNT_FAILED, flags, start, 0, 0);
        return Ok(EXIT_MOUNT_FAILED);
    }

    let request_path = Path::new("/x07/in/request.json");
    let req_bytes = match std::fs::read(request_path) {
        Ok(b) => b,
        Err(err) => {
            let flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
            let _ = try_send_ctrl_only(EXIT_REQUEST_INVALID, flags, start, 0, 0);
            let _ = writeln!(&mut std::io::stderr(), "request read failed: {err}");
            return Ok(EXIT_REQUEST_INVALID);
        }
    };

    let req: GuestRequest = match serde_json::from_slice(&req_bytes) {
        Ok(v) => v,
        Err(err) => {
            let flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
            let _ = try_send_ctrl_only(EXIT_REQUEST_INVALID, flags, start, 0, 0);
            let _ = writeln!(&mut std::io::stderr(), "request parse failed: {err}");
            return Ok(EXIT_REQUEST_INVALID);
        }
    };

    if let Err(err) = validate_request(&req) {
        let flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
        let _ = try_send_ctrl_only(EXIT_REQUEST_INVALID, flags, start, 0, 0);
        let _ = writeln!(&mut std::io::stderr(), "request validation failed: {err:#}");
        return Ok(EXIT_REQUEST_INVALID);
    }

    // Connect CTRL first so we can still report contract errors.
    let ctrl_fd = match vsock_connect(PORT_CTRL) {
        Ok(fd) => fd,
        Err(err) => {
            let _ = writeln!(&mut std::io::stderr(), "vsock ctrl connect failed: {err}");
            return Ok(EXIT_VSOCK_FAILED);
        }
    };
    let mut ctrl = unsafe { std::fs::File::from_raw_fd(ctrl_fd) };

    let stdout_fd = match vsock_connect(PORT_STDOUT) {
        Ok(fd) => fd,
        Err(err) => {
            let flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
            let _ = write_ctrl_record(&mut ctrl, EXIT_VSOCK_FAILED, flags, start, 0, 0);
            let _ = writeln!(&mut std::io::stderr(), "vsock stdout connect failed: {err}");
            return Ok(EXIT_VSOCK_FAILED);
        }
    };
    let stderr_fd = match vsock_connect(PORT_STDERR) {
        Ok(fd) => fd,
        Err(err) => {
            let flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
            let _ = write_ctrl_record(&mut ctrl, EXIT_VSOCK_FAILED, flags, start, 0, 0);
            let _ = writeln!(&mut std::io::stderr(), "vsock stderr connect failed: {err}");
            return Ok(EXIT_VSOCK_FAILED);
        }
    };

    for m in &req.mounts {
        let tag = m.tag.as_str();
        let stage_mountpoint = PathBuf::from("/x07/m").join(tag);
        ensure_dir(&stage_mountpoint)
            .with_context(|| format!("create mountpoint {}", stage_mountpoint.display()))?;

        if let Err(err) = mount_virtiofs(tag, &stage_mountpoint, m.readonly) {
            let written = write_guest_failure(
                &req.run_id,
                "mount_tag",
                tag,
                Some(stage_mountpoint.to_string_lossy().as_ref()),
                Some(&m.guest_path),
                Some(m.readonly),
                &err,
                out_mounted,
            )
            .unwrap_or(false);
            let mut flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
            if written {
                flags |= FLAG_GUEST_FAILURE_WRITTEN;
            }
            let _ = write_ctrl_record(&mut ctrl, EXIT_MOUNT_FAILED, flags, start, 0, 0);
            return Ok(EXIT_MOUNT_FAILED);
        }

        let guest_path = Path::new(&m.guest_path);
        ensure_dir(guest_path)
            .with_context(|| format!("ensure guest_path {}", guest_path.display()))?;

        if let Err(err) = bind_mount(&stage_mountpoint, guest_path) {
            let written = write_guest_failure(
                &req.run_id,
                "bind_mount",
                tag,
                Some(stage_mountpoint.to_string_lossy().as_ref()),
                Some(&m.guest_path),
                Some(m.readonly),
                &err,
                out_mounted,
            )
            .unwrap_or(false);
            let mut flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
            if written {
                flags |= FLAG_GUEST_FAILURE_WRITTEN;
            }
            let _ = write_ctrl_record(&mut ctrl, EXIT_MOUNT_FAILED, flags, start, 0, 0);
            return Ok(EXIT_MOUNT_FAILED);
        }

        if m.readonly {
            if let Err(err) = remount_bind_readonly(guest_path) {
                let written = write_guest_failure(
                    &req.run_id,
                    "remount_ro",
                    tag,
                    Some(stage_mountpoint.to_string_lossy().as_ref()),
                    Some(&m.guest_path),
                    Some(true),
                    &err,
                    out_mounted,
                )
                .unwrap_or(false);
                let mut flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
                if written {
                    flags |= FLAG_GUEST_FAILURE_WRITTEN;
                }
                let _ = write_ctrl_record(&mut ctrl, EXIT_MOUNT_FAILED, flags, start, 0, 0);
                return Ok(EXIT_MOUNT_FAILED);
            }
        }
    }

    if let Err(err) = enforce_network_policy(POLICY_PATH) {
        let _ = writeln!(
            &mut std::io::stderr(),
            "network policy enforcement failed: {err:#}"
        );
        let flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
        let _ = write_ctrl_record(&mut ctrl, EXIT_INTERNAL_ERROR, flags, start, 0, 0);
        return Ok(EXIT_INTERNAL_ERROR);
    }

    let mut stdout_sock = unsafe { std::fs::File::from_raw_fd(stdout_fd) };
    let mut stderr_sock = unsafe { std::fs::File::from_raw_fd(stderr_fd) };

    let stdout_cap = req.limits.stdout_max_bytes.unwrap_or(u64::MAX);
    let stderr_cap = req.limits.stderr_max_bytes.unwrap_or(u64::MAX);

    let env = req.exec.env.clone();
    let workdir = env.get(ENV_GUESTD_WORKDIR).map(PathBuf::from);

    let mut cmd = std::process::Command::new(&req.exec.path);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.env_clear();
    for (k, v) in &env {
        cmd.env(k, v);
    }
    if let Some(wd) = workdir.as_ref() {
        cmd.current_dir(wd);
    }

    if req.exec.argv.is_empty() {
        let flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
        let _ = write_ctrl_record(&mut ctrl, EXIT_REQUEST_INVALID, flags, start, 0, 0);
        return Ok(EXIT_REQUEST_INVALID);
    }

    cmd.arg0(&req.exec.argv[0]);
    if req.exec.argv.len() > 1 {
        cmd.args(&req.exec.argv[1..]);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            let flags = FLAG_CONTRACT_ERROR | FLAG_METRICS_PRESENT;
            let _ = write_ctrl_record(&mut ctrl, EXIT_EXEC_FAILED, flags, start, 0, 0);
            let _ = writeln!(&mut std::io::stderr(), "exec_failed: {err}");
            return Ok(EXIT_EXEC_FAILED);
        }
    };

    let child_stdout = child.stdout.take().context("take child stdout")?;
    let child_stderr = child.stderr.take().context("take child stderr")?;

    let stdout_thread = std::thread::spawn(move || -> std::io::Result<StreamStats> {
        stream_pipe_to_vsock(child_stdout, &mut stdout_sock, stdout_cap, true)
    });
    let stderr_thread = std::thread::spawn(move || -> std::io::Result<StreamStats> {
        stream_pipe_to_vsock(child_stderr, &mut stderr_sock, stderr_cap, false)
    });

    let exit_status = child.wait().context("wait child")?;
    let stdout_stats = stdout_thread
        .join()
        .unwrap_or_else(|_| Ok(StreamStats::empty()))?;
    let stderr_stats = stderr_thread
        .join()
        .unwrap_or_else(|_| Ok(StreamStats::empty()))?;

    let mut exit_code: i32 = exit_status.code().unwrap_or(1);
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(sig) = exit_status.signal() {
            exit_code = 128 + sig;
        }
    }

    let mut flags: u32 = 0;
    if stdout_stats.truncated {
        flags |= FLAG_STDOUT_TRUNCATED;
    }
    if stderr_stats.truncated {
        flags |= FLAG_STDERR_TRUNCATED;
    }

    if !stdout_stats.truncated
        && req.expect.stdout_is_single_json_object
        && req.expect.stdout_schema_version == X07_OS_RUNNER_REPORT_SCHEMA_VERSION
        && stdout_stats
            .is_expected_report_json_object()
            .unwrap_or(false)
    {
        flags |= FLAG_REPORT_COMPLETE;
    }

    flags |= FLAG_METRICS_PRESENT;

    write_ctrl_record(
        &mut ctrl,
        exit_code,
        flags,
        start,
        stdout_stats.bytes_sent,
        stderr_stats.bytes_sent,
    )?;

    Ok(exit_code)
}

fn validate_request(req: &GuestRequest) -> Result<()> {
    if req.schema_version != "x07.guest.request@1" {
        anyhow::bail!("unsupported schema_version {:?}", req.schema_version);
    }
    if req.run_id.trim().is_empty() {
        anyhow::bail!("run_id is empty");
    }
    if !req.exec.path.starts_with('/') {
        anyhow::bail!("exec.path must be absolute, got {:?}", req.exec.path);
    }
    if req.exec.argv.is_empty() {
        anyhow::bail!("exec.argv is empty");
    }

    for m in &req.mounts {
        validate_mount_tag(&m.tag)?;
        if !m.guest_path.starts_with('/') {
            anyhow::bail!("mount guest_path must be absolute: {:?}", m.guest_path);
        }
    }

    Ok(())
}

fn validate_mount_tag(tag: &str) -> Result<()> {
    let Some(num) = tag.strip_prefix("x07m") else {
        anyhow::bail!("mount tag must start with x07m, got {tag:?}");
    };
    let n: u8 = num
        .parse()
        .with_context(|| format!("invalid mount tag {tag:?}"))?;
    if n > 63 {
        anyhow::bail!("mount tag out of range (0..63): {tag:?}");
    }
    Ok(())
}

fn enforce_network_policy(policy_path: &str) -> Result<()> {
    let bytes = match std::fs::read(policy_path) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    let policy: PolicyLite =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {policy_path}"))?;

    let nft_bin = resolve_nft_bin();
    if nft_bin.is_none() {
        if policy.net.enabled && !policy.net.allow_hosts.is_empty() {
            anyhow::bail!("missing nft binary (install nftables in the guest image)");
        }
        return Ok(());
    }
    let nft_bin = nft_bin.unwrap();

    let script = build_nft_script(&policy.net)?;

    let _ = std::process::Command::new(nft_bin)
        .args(["delete", "table", "inet", NFT_TABLE_NAME])
        .status();

    let mut child = std::process::Command::new(nft_bin)
        .args(["-f", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn {nft_bin}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(script.as_bytes())
            .context("write nft script")?;
    }

    let out = child.wait_with_output().context("wait nft")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("nft failed: {stderr}");
    }

    Ok(())
}

fn resolve_nft_bin() -> Option<&'static str> {
    for cand in ["/usr/sbin/nft", "/usr/bin/nft"] {
        if Path::new(cand).is_file() {
            return Some(cand);
        }
    }
    None
}

#[derive(Debug, Clone)]
enum Dest {
    Ip(IpAddr),
    Cidr { addr: IpAddr, prefix: u8 },
}

fn build_nft_script(net: &PolicyNet) -> Result<String> {
    let nameservers = if net.enabled && net.allow_dns {
        read_resolv_nameservers().unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut allowed: Vec<(Dest, Vec<u16>)> = Vec::new();
    if net.enabled {
        for entry in &net.allow_hosts {
            let ports = unique_ports(&entry.ports);
            if ports.is_empty() {
                continue;
            }
            for dest in resolve_destinations(&entry.host, net.allow_dns)? {
                allowed.push((dest, ports.clone()));
            }
        }
    }

    let mut rules: Vec<String> = Vec::new();
    rules.push("  chain output {".to_string());
    rules.push("    type filter hook output priority 0; policy drop;".to_string());
    rules.push("    oifname \"lo\" accept".to_string());
    rules.push("    ct state established,related accept".to_string());

    if net.enabled && net.allow_dns {
        for ns in &nameservers {
            let dst = ns.to_string();
            match ns {
                IpAddr::V4(_) => {
                    rules.push(format!("    ip daddr {dst} udp dport 53 accept"));
                    rules.push(format!("    ip daddr {dst} tcp dport 53 accept"));
                }
                IpAddr::V6(_) => {
                    rules.push(format!("    ip6 daddr {dst} udp dport 53 accept"));
                    rules.push(format!("    ip6 daddr {dst} tcp dport 53 accept"));
                }
            }
        }
    }

    for (dest, ports) in &allowed {
        let dst = dest_to_nft(dest);
        let ports_str = ports_to_nft_set(ports);
        let (v4_kw, v6_kw) = match dest {
            Dest::Ip(IpAddr::V4(_))
            | Dest::Cidr {
                addr: IpAddr::V4(_),
                ..
            } => ("ip", ""),
            Dest::Ip(IpAddr::V6(_))
            | Dest::Cidr {
                addr: IpAddr::V6(_),
                ..
            } => ("", "ip6"),
        };

        if net.allow_tcp {
            if !v4_kw.is_empty() {
                rules.push(format!(
                    "    {v4_kw} daddr {dst} tcp dport {ports_str} accept"
                ));
            } else {
                rules.push(format!(
                    "    {v6_kw} daddr {dst} tcp dport {ports_str} accept"
                ));
            }
        }
        if net.allow_udp {
            if !v4_kw.is_empty() {
                rules.push(format!(
                    "    {v4_kw} daddr {dst} udp dport {ports_str} accept"
                ));
            } else {
                rules.push(format!(
                    "    {v6_kw} daddr {dst} udp dport {ports_str} accept"
                ));
            }
        }
    }

    rules.push("  }".to_string());

    let mut out = String::new();
    out.push_str(&format!("table inet {NFT_TABLE_NAME} {{\n"));
    out.push_str(&rules.join("\n"));
    out.push_str("\n}\n");
    Ok(out)
}

fn unique_ports(ports: &[u16]) -> Vec<u16> {
    let mut out = ports
        .iter()
        .copied()
        .filter(|p| *p != 0)
        .collect::<Vec<_>>();
    out.sort_unstable();
    out.dedup();
    out
}

fn ports_to_nft_set(ports: &[u16]) -> String {
    if ports.len() == 1 {
        return ports[0].to_string();
    }
    let joined = ports
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {joined} }}")
}

fn dest_to_nft(dest: &Dest) -> String {
    match dest {
        Dest::Ip(ip) => ip.to_string(),
        Dest::Cidr { addr, prefix } => format!("{addr}/{prefix}"),
    }
}

fn resolve_destinations(host: &str, allow_dns: bool) -> Result<Vec<Dest>> {
    let host = host.trim();
    if host.is_empty() {
        return Ok(Vec::new());
    }

    if let Some((ip, prefix)) = parse_cidr(host) {
        return Ok(vec![Dest::Cidr { addr: ip, prefix }]);
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![Dest::Ip(ip)]);
    }

    if !allow_dns {
        anyhow::bail!(
            "policy.net.allow_dns is false, but policy.net.allow_hosts contains a hostname: {host:?}"
        );
    }

    let addrs = (host, 0)
        .to_socket_addrs()
        .with_context(|| format!("resolve host {host:?}"))?;
    let mut ips: Vec<IpAddr> = addrs.map(|a| a.ip()).collect();
    ips.sort_unstable();
    ips.dedup();
    Ok(ips.into_iter().map(Dest::Ip).collect())
}

fn parse_cidr(host: &str) -> Option<(IpAddr, u8)> {
    let (ip_s, prefix_s) = host.split_once('/')?;
    let ip: IpAddr = ip_s.parse().ok()?;
    let prefix: u8 = prefix_s.parse().ok()?;
    let ok = match ip {
        IpAddr::V4(_) => prefix <= 32,
        IpAddr::V6(_) => prefix <= 128,
    };
    if ok {
        Some((ip, prefix))
    } else {
        None
    }
}

fn read_resolv_nameservers() -> Result<Vec<IpAddr>> {
    let txt = std::fs::read_to_string("/etc/resolv.conf").context("read /etc/resolv.conf")?;
    let mut out: Vec<IpAddr> = Vec::new();
    for line in txt.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("nameserver") else {
            continue;
        };
        let addr = rest.trim();
        if let Ok(ip) = addr.parse::<IpAddr>() {
            out.push(ip);
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("create dir {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_nft_script_net_disabled_is_default_drop() {
        let net = PolicyNet {
            enabled: false,
            allow_dns: false,
            allow_tcp: false,
            allow_udp: false,
            allow_hosts: Vec::new(),
        };

        let script = build_nft_script(&net).unwrap();
        assert!(script.contains("policy drop"));
        assert!(script.contains("oifname \"lo\" accept"));
        assert!(script.contains("ct state established,related accept"));
    }

    #[test]
    fn build_nft_script_allows_explicit_ip_tcp_ports() {
        let net = PolicyNet {
            enabled: true,
            allow_dns: false,
            allow_tcp: true,
            allow_udp: false,
            allow_hosts: vec![PolicyNetHost {
                host: "1.2.3.4".to_string(),
                ports: vec![443, 80, 443],
            }],
        };

        let script = build_nft_script(&net).unwrap();
        assert!(script.contains("ip daddr 1.2.3.4 tcp dport { 80, 443 } accept"));
        assert!(!script.contains(" udp dport "));
    }

    #[test]
    fn build_nft_script_rejects_hostname_without_dns() {
        let net = PolicyNet {
            enabled: true,
            allow_dns: false,
            allow_tcp: true,
            allow_udp: false,
            allow_hosts: vec![PolicyNetHost {
                host: "example.com".to_string(),
                ports: vec![443],
            }],
        };

        let err = build_nft_script(&net).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("policy.net.allow_dns is false"));
    }
}

fn mount_fs(
    source: &str,
    target: &str,
    fstype: &str,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<()> {
    let src = CString::new(source)?;
    let tgt = CString::new(target)?;
    let typ = CString::new(fstype)?;
    let data_c = data.map(CString::new).transpose()?;
    let data_ptr = data_c
        .as_ref()
        .map(|s| s.as_ptr() as *const libc::c_void)
        .unwrap_or(std::ptr::null());
    let rc = unsafe { libc::mount(src.as_ptr(), tgt.as_ptr(), typ.as_ptr(), flags, data_ptr) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("mount");
    }
    Ok(())
}

fn mount_virtiofs(tag: &str, mountpoint: &Path, readonly: bool) -> Result<()> {
    let tag_c = CString::new(tag)?;
    let mp_c = CString::new(mountpoint.as_os_str().as_bytes())?;
    let ty_c = CString::new("virtiofs")?;
    let flags = if readonly { libc::MS_RDONLY } else { 0 } as libc::c_ulong;
    let rc = unsafe {
        libc::mount(
            tag_c.as_ptr(),
            mp_c.as_ptr(),
            ty_c.as_ptr(),
            flags,
            std::ptr::null(),
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("mount virtiofs");
    }
    Ok(())
}

fn bind_mount(src: &Path, dst: &Path) -> Result<()> {
    let src_c = CString::new(src.as_os_str().as_bytes())?;
    let dst_c = CString::new(dst.as_os_str().as_bytes())?;
    let rc = unsafe {
        libc::mount(
            src_c.as_ptr(),
            dst_c.as_ptr(),
            std::ptr::null(),
            (libc::MS_BIND | libc::MS_REC) as libc::c_ulong,
            std::ptr::null(),
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("bind mount");
    }
    Ok(())
}

fn remount_bind_readonly(dst: &Path) -> Result<()> {
    let dst_c = CString::new(dst.as_os_str().as_bytes())?;
    let rc = unsafe {
        libc::mount(
            std::ptr::null(),
            dst_c.as_ptr(),
            std::ptr::null(),
            (libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY) as libc::c_ulong,
            std::ptr::null(),
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("remount ro");
    }
    Ok(())
}

fn vsock_connect(port: u32) -> std::io::Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_VSOCK, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut addr: libc::sockaddr_vm = unsafe { std::mem::zeroed() };
    addr.svm_family = libc::AF_VSOCK as libc::sa_family_t;
    addr.svm_port = port;
    addr.svm_cid = libc::VMADDR_CID_HOST;

    let rc = unsafe {
        libc::connect(
            fd,
            &addr as *const libc::sockaddr_vm as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_vm>() as libc::socklen_t,
        )
    };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }
    Ok(fd)
}

#[derive(Debug, Clone)]
struct StreamStats {
    bytes_sent: u64,
    truncated: bool,
    captured_stdout: Vec<u8>,
}

impl StreamStats {
    fn empty() -> Self {
        Self {
            bytes_sent: 0,
            truncated: false,
            captured_stdout: Vec::new(),
        }
    }

    fn is_expected_report_json_object(&self) -> Result<bool> {
        let s = std::str::from_utf8(&self.captured_stdout).context("stdout not utf-8")?;
        let v: serde_json::Value = serde_json::from_str(s).context("stdout not JSON")?;
        let obj = v.as_object().context("stdout not a JSON object")?;
        Ok(obj
            .get("schema_version")
            .and_then(|v| v.as_str())
            .is_some_and(|sv| sv == X07_OS_RUNNER_REPORT_SCHEMA_VERSION))
    }
}

fn stream_pipe_to_vsock<R: Read>(
    mut r: R,
    w: &mut dyn Write,
    cap: u64,
    capture_for_report_complete: bool,
) -> std::io::Result<StreamStats> {
    let mut buf = [0u8; 8192];
    let mut bytes_sent: u64 = 0;
    let mut truncated = false;
    let mut captured: Vec<u8> = Vec::new();

    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }

        let remaining = cap.saturating_sub(bytes_sent);
        if remaining == 0 {
            truncated = true;
            continue;
        }

        let to_write = (n as u64).min(remaining) as usize;
        if to_write < n {
            truncated = true;
        }

        if to_write > 0 {
            w.write_all(&buf[..to_write])?;
            bytes_sent = bytes_sent.saturating_add(to_write as u64);
            if capture_for_report_complete {
                captured.extend_from_slice(&buf[..to_write]);
            }
        }
    }

    Ok(StreamStats {
        bytes_sent,
        truncated,
        captured_stdout: captured,
    })
}

fn write_ctrl_record(
    ctrl: &mut dyn Write,
    exit_code: i32,
    mut flags: u32,
    start: Instant,
    stdout_bytes_sent: u64,
    stderr_bytes_sent: u64,
) -> Result<()> {
    if flags & FLAG_METRICS_PRESENT != 0 {
        // Always set; treat metrics write as best-effort.
    } else {
        flags |= FLAG_METRICS_PRESENT;
    }

    let mut base = [0u8; 8];
    base[0..4].copy_from_slice(&exit_code.to_le_bytes());
    base[4..8].copy_from_slice(&flags.to_le_bytes());
    ctrl.write_all(&base).context("write ctrl base")?;

    let elapsed_wall_ms: u64 = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    let mut metrics = [0u8; 24];
    metrics[0..8].copy_from_slice(&elapsed_wall_ms.to_le_bytes());
    metrics[8..16].copy_from_slice(&stdout_bytes_sent.to_le_bytes());
    metrics[16..24].copy_from_slice(&stderr_bytes_sent.to_le_bytes());
    let _ = ctrl.write_all(&metrics);
    let _ = ctrl.flush();

    Ok(())
}

fn write_guest_failure_generic(
    run_id: &str,
    stage: &'static str,
    tag: Option<&str>,
    mountpoint: Option<&str>,
    guest_path: Option<&str>,
    readonly: Option<bool>,
    err: &anyhow::Error,
    out_mounted: bool,
) -> Result<bool> {
    if !out_mounted {
        return Ok(false);
    }

    let when_unix_ms = now_unix_ms().unwrap_or(0);
    let failure = GuestFailure {
        schema_version: "x07.guest.failure@0.1.0",
        run_id: if run_id.trim().is_empty() {
            "unknown".to_string()
        } else {
            run_id.to_string()
        },
        failure_kind: "mount_failed",
        stage,
        when_unix_ms,
        tag: tag.map(|s| s.to_string()),
        mountpoint: mountpoint.map(|s| s.to_string()),
        guest_path: guest_path.map(|s| s.to_string()),
        readonly,
        errno: err
            .root_cause()
            .downcast_ref::<std::io::Error>()
            .map(|e| e.raw_os_error())
            .flatten(),
        message: format!("{err:#}"),
        detail: None,
    };

    let out_path = Path::new("/x07/out/guest_failure.json");
    let tmp_path = Path::new("/x07/out/guest_failure.json.tmp");

    let mut bytes = serde_json::to_vec(&failure)?;
    bytes.push(b'\n');
    std::fs::write(tmp_path, &bytes).context("write guest_failure.json.tmp")?;

    let _ = fsync_path(tmp_path);
    std::fs::rename(tmp_path, out_path).context("rename guest_failure.json")?;
    let _ = fsync_path(out_path);
    Ok(true)
}

fn write_guest_failure(
    run_id: &str,
    stage: &'static str,
    tag: &str,
    mountpoint: Option<&str>,
    guest_path: Option<&str>,
    readonly: Option<bool>,
    err: &anyhow::Error,
    out_mounted: bool,
) -> Result<bool> {
    write_guest_failure_generic(
        run_id,
        stage,
        Some(tag),
        mountpoint,
        guest_path,
        readonly,
        err,
        out_mounted,
    )
}

fn fsync_path(path: &Path) -> Result<()> {
    use std::os::unix::io::AsRawFd as _;
    let f = std::fs::OpenOptions::new().read(true).open(path)?;
    let rc = unsafe { libc::fsync(f.as_raw_fd()) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("fsync");
    }
    Ok(())
}

fn now_unix_ms() -> Result<u64> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(d.as_millis().try_into().unwrap_or(u64::MAX))
}

fn try_send_ctrl_only(
    exit_code: i32,
    flags: u32,
    start: Instant,
    stdout_bytes: u64,
    stderr_bytes: u64,
) -> Result<()> {
    let fd = vsock_connect(PORT_CTRL).context("vsock ctrl connect")?;
    let mut f = unsafe { std::fs::File::from_raw_fd(fd) };
    write_ctrl_record(&mut f, exit_code, flags, start, stdout_bytes, stderr_bytes)
}

fn read_run_id_from_cmdline() -> Option<String> {
    let cmdline = std::fs::read_to_string("/proc/cmdline").ok()?;
    for tok in cmdline.split_whitespace() {
        let mut it = tok.splitn(2, '=');
        let k = it.next()?;
        let v = it.next()?;
        if k == CMDLINE_RUN_ID_KEY && !v.trim().is_empty() {
            let v = v.trim().to_string();
            if v.len() <= 128 {
                return Some(v);
            }
        }
    }
    None
}
