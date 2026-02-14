use std::fs::File;
use std::io::{BufReader, Read as _};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::{firecracker_ctr_config_from_env, FirecrackerCtrConfig, VmBackend};

#[derive(Debug, Clone, Deserialize)]
struct VzGuestBundleLinux {
    kernel: String,
    rootfs: String,
    cmdline: String,
}

#[derive(Debug, Clone, Deserialize)]
struct VzGuestBundleManifest {
    schema_version: String,
    linux: VzGuestBundleLinux,
}

pub fn resolve_vm_guest_digest(
    backend: VmBackend,
    image_or_bundle: &str,
    firecracker_cfg: Option<&FirecrackerCtrConfig>,
) -> Result<String> {
    match backend {
        VmBackend::Vz => compute_vz_guest_bundle_digest(Path::new(image_or_bundle)),
        VmBackend::FirecrackerCtr => {
            let cfg = firecracker_cfg
                .cloned()
                .unwrap_or_else(firecracker_ctr_config_from_env);
            resolve_ctr_image_target_digest(&cfg, image_or_bundle)
        }
        VmBackend::Docker => resolve_docker_like_image_digest("docker", image_or_bundle),
        VmBackend::Podman => resolve_docker_like_image_digest("podman", image_or_bundle),
        VmBackend::AppleContainer => resolve_apple_container_image_digest(image_or_bundle),
    }
}

pub fn verify_vm_guest_digest(
    backend: VmBackend,
    image_or_bundle: &str,
    expected_digest: &str,
    firecracker_cfg: Option<&FirecrackerCtrConfig>,
) -> Result<()> {
    let got = resolve_vm_guest_digest(backend, image_or_bundle, firecracker_cfg)?;
    if got != expected_digest {
        anyhow::bail!(
            "guest digest mismatch for {backend}: expected {expected_digest:?}, got {got:?}"
        );
    }
    Ok(())
}

fn compute_vz_guest_bundle_digest(bundle_dir: &Path) -> Result<String> {
    let manifest_path = bundle_dir.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("read vz guest bundle manifest: {}", manifest_path.display()))?;
    let manifest: VzGuestBundleManifest =
        serde_json::from_slice(&manifest_bytes).with_context(|| {
            format!(
                "parse vz guest bundle manifest JSON: {}",
                manifest_path.display()
            )
        })?;
    if manifest.schema_version != "x07.vz.guest.bundle@0.1.0" {
        anyhow::bail!(
            "unsupported vz guest bundle schema_version: {:?}",
            manifest.schema_version
        );
    }

    let kernel_path = bundle_dir.join(&manifest.linux.kernel);
    let rootfs_path = bundle_dir.join(&manifest.linux.rootfs);
    let cmdline_path = bundle_dir.join(&manifest.linux.cmdline);

    let mut h = Sha256::new();

    h.update(b"manifest.json\0");
    h.update(&manifest_bytes);

    hash_file(&mut h, b"kernel\0", &kernel_path)?;
    hash_file(&mut h, b"rootfs\0", &rootfs_path)?;
    hash_file(&mut h, b"cmdline\0", &cmdline_path)?;

    Ok(format!("sha256:{:x}", h.finalize()))
}

fn hash_file(h: &mut Sha256, tag: &[u8], path: &Path) -> Result<()> {
    h.update(tag);
    let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut r = BufReader::new(f);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = r
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(())
}

fn resolve_docker_like_image_digest(bin: &str, image: &str) -> Result<String> {
    if let Ok(d) = docker_like_repo_digest(bin, image) {
        return Ok(d);
    }
    docker_like_image_id(bin, image)
}

fn docker_like_repo_digest(bin: &str, image: &str) -> Result<String> {
    let mut cmd = std::process::Command::new(bin);
    cmd.args([
        "image",
        "inspect",
        "--format",
        "{{json .RepoDigests}}",
        image,
    ]);
    let out = crate::run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("{bin} image inspect RepoDigests {image}"))?;
    if out.timed_out || out.exit_status != 0 {
        anyhow::bail!("inspect failed");
    }

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).context("parse RepoDigests JSON")?;
    let a = v.as_array().context("RepoDigests JSON is not an array")?;
    let first = a
        .iter()
        .filter_map(|v| v.as_str())
        .find_map(|s| s.split('@').nth(1))
        .unwrap_or("");
    normalize_sha256_digest(first)
}

fn docker_like_image_id(bin: &str, image: &str) -> Result<String> {
    let mut cmd = std::process::Command::new(bin);
    cmd.args(["image", "inspect", "--format", "{{.Id}}", image]);
    let out = crate::run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("{bin} image inspect Id {image}"))?;
    if out.timed_out {
        anyhow::bail!("{bin} image inspect timed out");
    }
    if out.exit_status != 0 {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("{bin} image inspect failed: {stderr}");
    }
    let s = String::from_utf8_lossy(&out.stdout);
    normalize_sha256_digest(s.trim())
}

fn resolve_ctr_image_target_digest(cfg: &FirecrackerCtrConfig, image: &str) -> Result<String> {
    let mut cmd = std::process::Command::new(&cfg.bin);
    cmd.args([
        "--address",
        &cfg.address,
        "--namespace",
        &cfg.namespace,
        "--timeout",
        "2s",
        "images",
        "info",
        image,
    ]);
    let out = crate::run_command_capped(cmd, 2_000, 256 * 1024, 256 * 1024)
        .with_context(|| format!("firecracker-ctr images info {image}"))?;
    if out.timed_out {
        anyhow::bail!("firecracker-ctr images info timed out");
    }
    if out.exit_status != 0 {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("firecracker-ctr images info failed: {stderr}");
    }

    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
        if let Some(d) = extract_ctr_target_digest(&v) {
            return normalize_sha256_digest(&d);
        }
        if let Some(d) = extract_preferred_digest(&v) {
            return normalize_sha256_digest(&d);
        }
    }

    let s = String::from_utf8_lossy(&out.stdout);
    if let Some(d) = find_first_sha256_digest_in_text(&s) {
        return normalize_sha256_digest(&d);
    }

    anyhow::bail!("could not extract digest from ctr images info output");
}

fn extract_ctr_target_digest(v: &serde_json::Value) -> Option<String> {
    let obj = v.as_object()?;
    for (k, val) in obj {
        if k.eq_ignore_ascii_case("target") {
            let tobj = val.as_object()?;
            for (tk, tv) in tobj {
                if tk.eq_ignore_ascii_case("digest") {
                    return tv.as_str().map(|s| s.to_string());
                }
            }
        }
    }
    None
}

fn resolve_apple_container_image_digest(image: &str) -> Result<String> {
    if !cfg!(target_os = "macos") {
        anyhow::bail!("apple-container digest resolution is only supported on macOS");
    }

    let mut last_missing_subcommand_err: Option<String> = None;

    for subcmd in ["image", "images"] {
        let mut cmd = std::process::Command::new("container");
        cmd.args([subcmd, "inspect", image]);

        let out = crate::run_command_capped(cmd, 2_000, 256 * 1024, 256 * 1024)
            .with_context(|| format!("container {subcmd} inspect {image}"))?;

        if out.timed_out {
            anyhow::bail!("container {subcmd} inspect timed out");
        }
        if out.exit_status != 0 {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();

            if is_container_subcommand_missing(&stderr, subcmd) {
                last_missing_subcommand_err =
                    Some(format!("container {subcmd} inspect failed: {stderr}"));
                continue;
            }

            anyhow::bail!("container {subcmd} inspect failed: {stderr}");
        }

        let v: serde_json::Value = serde_json::from_slice(&out.stdout)
            .with_context(|| format!("parse container {subcmd} inspect JSON"))?;

        let v = v.as_array().and_then(|a| a.first()).unwrap_or(&v);
        if let Some(d) = extract_preferred_digest(v) {
            return normalize_sha256_digest(&d);
        }

        anyhow::bail!("could not extract digest from container {subcmd} inspect output");
    }

    if let Some(err) = last_missing_subcommand_err {
        anyhow::bail!(
            "{err}\n(note: tried both `container image inspect` and `container images inspect`)"
        );
    }
    anyhow::bail!("apple-container digest resolution failed (tried both `container image inspect` and `container images inspect`)");
}

fn is_container_subcommand_missing(stderr: &str, subcmd: &str) -> bool {
    let s = stderr.to_ascii_lowercase();

    if s.contains("failed to find plugin named") && s.contains(&format!("container-{subcmd}")) {
        return true;
    }

    if s.contains("unknown subcommand") && s.contains(subcmd) {
        return true;
    }
    if s.contains("unknown command") && s.contains(subcmd) {
        return true;
    }

    if s.contains("unexpected argument")
        && s.contains(subcmd)
        && (s.contains("usage: container") || s.contains("see 'container"))
    {
        return true;
    }

    false
}

fn extract_preferred_digest(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Object(map) => {
            let mut digest: Option<String> = None;
            for (k, val) in map {
                if k.eq_ignore_ascii_case("manifestdigest")
                    || k.eq_ignore_ascii_case("manifest_digest")
                {
                    if let Some(s) = val.as_str() {
                        if looks_like_sha256_digest(s) {
                            return Some(s.to_string());
                        }
                    }
                }
                if k.eq_ignore_ascii_case("digest") {
                    if let Some(s) = val.as_str() {
                        if looks_like_sha256_digest(s) {
                            digest = Some(s.to_string());
                        }
                    }
                }

                if let Some(s) = extract_preferred_digest(val) {
                    if looks_like_sha256_digest(&s) {
                        return Some(s);
                    }
                }
            }
            digest
        }
        serde_json::Value::Array(a) => a.iter().find_map(extract_preferred_digest),
        serde_json::Value::String(s) => {
            if looks_like_sha256_digest(s) {
                Some(s.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn looks_like_sha256_digest(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with("sha256:") {
        return false;
    }
    let hex = &s["sha256:".len()..];
    hex.len() == 64 && hex.as_bytes().iter().all(|b| b.is_ascii_hexdigit())
}

fn normalize_sha256_digest(raw: &str) -> Result<String> {
    let s = raw.trim();
    if !looks_like_sha256_digest(s) {
        anyhow::bail!("invalid digest {raw:?} (expected sha256:<64-hex>)");
    }
    Ok(s.to_string())
}

fn find_first_sha256_digest_in_text(s: &str) -> Option<String> {
    let needle = "sha256:";
    let start = s.find(needle)?;
    let rest = &s[start..];
    let mut end = needle.len();
    for b in rest[needle.len()..].bytes() {
        if b.is_ascii_hexdigit() && end < needle.len() + 64 {
            end += 1;
            continue;
        }
        break;
    }
    if end == needle.len() + 64 {
        Some(rest[..end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_sha256_digest_rejects_non_hex() {
        assert!(normalize_sha256_digest("sha256:xyz").is_err());
    }

    #[test]
    fn normalize_sha256_digest_accepts_64_hex() {
        let d = format!("sha256:{}", "a".repeat(64));
        assert_eq!(normalize_sha256_digest(&d).unwrap(), d);
    }

    #[test]
    fn apple_container_subcommand_missing_detection() {
        assert!(is_container_subcommand_missing(
            "Error: unknown command \"images\" for \"container\"",
            "images"
        ));

        assert!(is_container_subcommand_missing(
            "Error: unknown subcommand: image",
            "image"
        ));

        assert!(is_container_subcommand_missing(
            "Error: failed to find plugin named container-images",
            "images"
        ));

        assert!(is_container_subcommand_missing(
            "Error: Unexpected argument \"images\".\nUsage: container ...",
            "images"
        ));

        assert!(!is_container_subcommand_missing(
            "Error: image not found: ghcr.io/x07lang/x07-guest-runner:missing",
            "image"
        ));
    }
}
