use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Args)]
pub struct DoctorArgs {}

#[derive(Debug, Serialize)]
struct DoctorReport {
    ok: bool,
    command: &'static str,
    platform: PlatformInfo,
    checks: Vec<Check>,
    suggestions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PlatformInfo {
    os: String,
    arch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    distro: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wsl: Option<bool>,
}

#[derive(Debug, Serialize)]
struct Check {
    name: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

pub fn cmd_doctor(
    _machine: &crate::reporting::MachineArgs,
    _args: DoctorArgs,
) -> Result<std::process::ExitCode> {
    let platform = detect_platform();
    let mut checks: Vec<Check> = Vec::new();
    let mut suggestions: Vec<String> = Vec::new();

    let compiler = find_first_in_path(&["clang", "gcc", "cc"]);
    checks.push(Check {
        name: "c_compiler".to_string(),
        ok: compiler.is_some(),
        detail: compiler.as_ref().map(|p| format!("found: {}", p.display())),
    });

    if compiler.is_none() {
        suggestions
            .push("Install a C toolchain (clang or gcc) and ensure it is on PATH.".to_string());
    }

    let compile_ok = if let Some(compiler) = &compiler {
        match check_curl_openssl_link(compiler) {
            Ok(()) => {
                checks.push(Check {
                    name: "net_deps_curl_openssl".to_string(),
                    ok: true,
                    detail: Some("ok".to_string()),
                });
                true
            }
            Err(err) => {
                checks.push(Check {
                    name: "net_deps_curl_openssl".to_string(),
                    ok: false,
                    detail: Some(format!("{err:#}")),
                });
                false
            }
        }
    } else {
        false
    };

    if compiler.is_some() && !compile_ok {
        if let Some(cmd) = platform_install_hint(&platform) {
            suggestions.push(cmd);
        } else {
            suggestions.push("Install libcurl + OpenSSL development headers/libs (and ensure the compiler can find them).".to_string());
        }
    }

    let ok = checks.iter().all(|c| c.ok);

    let report = DoctorReport {
        ok,
        command: "doctor",
        platform,
        checks,
        suggestions,
    };

    let mut bytes = serde_json::to_vec(&report)?;
    bytes.push(b'\n');
    std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;

    Ok(if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn detect_platform() -> PlatformInfo {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let wsl = detect_wsl();
    let distro = if os == "linux" {
        detect_linux_distro()
    } else {
        None
    };

    PlatformInfo {
        os,
        arch,
        distro,
        wsl,
    }
}

fn detect_wsl() -> Option<bool> {
    if std::env::var_os("WSL_DISTRO_NAME").is_some() {
        return Some(true);
    }
    if std::env::consts::OS != "linux" {
        return None;
    }
    let version = std::fs::read_to_string("/proc/version").unwrap_or_default();
    if version.to_ascii_lowercase().contains("microsoft") {
        return Some(true);
    }
    None
}

fn detect_linux_distro() -> Option<String> {
    let os_release = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in os_release.lines() {
        if let Some(rest) = line.strip_prefix("ID=") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

fn platform_install_hint(platform: &PlatformInfo) -> Option<String> {
    match (platform.os.as_str(), platform.distro.as_deref()) {
        ("linux", Some("ubuntu")) | ("linux", Some("debian")) => Some(
            "sudo apt-get update && sudo apt-get install -y clang gcc make pkg-config libcurl4-openssl-dev libssl-dev".to_string(),
        ),
        ("linux", Some("fedora")) => Some(
            "sudo dnf install -y clang gcc make pkgconf-pkg-config libcurl-devel openssl-devel".to_string(),
        ),
        ("linux", Some("arch")) => Some(
            "sudo pacman -S --needed clang gcc make pkgconf curl openssl".to_string(),
        ),
        ("macos", _) => Some("brew install llvm pkg-config curl openssl".to_string()),
        _ => None,
    }
}

fn find_first_in_path(candidates: &[&str]) -> Option<PathBuf> {
    for c in candidates {
        if let Some(p) = find_in_path(c) {
            return Some(p);
        }
    }
    None
}

fn find_in_path(prog: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(prog);
        if cand.is_file() && is_executable(&cand) {
            return Some(cand);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if let Ok(meta) = std::fs::metadata(path) {
            return meta.permissions().mode() & 0o111 != 0;
        }
        false
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn curl_openssl_link_args(openssl_prefix: Option<&Path>) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if let Some(prefix) = openssl_prefix {
        let inc_dir = prefix.join("include");
        let lib_dir = prefix.join("lib");
        args.push(format!("-I{}", inc_dir.display()));
        args.push(format!("-L{}", lib_dir.display()));
        args.push(format!("-Wl,-rpath,{}", lib_dir.display()));
    }
    args.extend(
        ["-lcurl", "-lssl", "-lcrypto"]
            .into_iter()
            .map(str::to_string),
    );
    args
}

fn brew_prefix(formula: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    find_in_path("brew")?;

    let out = std::process::Command::new("brew")
        .args(["--prefix", formula])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if prefix.is_empty() {
        return None;
    }
    let prefix = PathBuf::from(prefix);
    if prefix.join("include").is_dir() && prefix.join("lib").is_dir() {
        Some(prefix)
    } else {
        None
    }
}

fn brew_prefix_openssl() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    for formula in ["openssl@3", "openssl@1.1", "openssl"] {
        if let Some(prefix) = brew_prefix(formula) {
            return Some(prefix);
        }
    }
    None
}

fn check_curl_openssl_link(compiler: &PathBuf) -> Result<()> {
    let tmp = std::env::temp_dir();
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);

    let c_path = tmp.join(format!("x07_doctor_{pid}_{n}.c"));
    let out_path = tmp.join(format!("x07_doctor_{pid}_{n}.out"));

    let c_src = b"#include <curl/curl.h>\n#include <openssl/ssl.h>\nint main(void) { return 0; }\n";
    std::fs::write(&c_path, c_src).with_context(|| format!("write {}", c_path.display()))?;

    let mut cc_cmd = std::process::Command::new(compiler);
    cc_cmd.arg(&c_path).arg("-o").arg(&out_path);
    for arg in curl_openssl_link_args(brew_prefix_openssl().as_deref()) {
        cc_cmd.arg(arg);
    }

    let cmd = cc_cmd
        .output()
        .with_context(|| format!("exec {}", compiler.display()))?;

    let _ = std::fs::remove_file(&c_path);
    let _ = std::fs::remove_file(&out_path);

    if cmd.status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "failed to compile/link curl+openssl test program (status {})\nstdout:\n{}\nstderr:\n{}",
            cmd.status,
            String::from_utf8_lossy(&cmd.stdout),
            String::from_utf8_lossy(&cmd.stderr)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn curl_openssl_link_args_without_prefix() {
        let args = curl_openssl_link_args(None);
        assert_eq!(
            args,
            vec![
                "-lcurl".to_string(),
                "-lssl".to_string(),
                "-lcrypto".to_string()
            ]
        );
    }

    #[test]
    fn curl_openssl_link_args_with_prefix() {
        let prefix = PathBuf::from("openssl-prefix");
        let args = curl_openssl_link_args(Some(&prefix));

        let inc = format!("-I{}", prefix.join("include").display());
        let lib = format!("-L{}", prefix.join("lib").display());
        let rpath = format!("-Wl,-rpath,{}", prefix.join("lib").display());

        assert_eq!(
            args,
            vec![
                inc,
                lib,
                rpath,
                "-lcurl".to_string(),
                "-lssl".to_string(),
                "-lcrypto".to_string()
            ]
        );
    }
}
