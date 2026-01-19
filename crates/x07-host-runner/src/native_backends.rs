use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use x07_contracts::NATIVE_BACKENDS_SCHEMA_VERSION;
use x07c::native::NativeRequires;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBackendsManifest {
    pub schema_version: String,
    pub backends: Vec<NativeBackend>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBackend {
    pub backend_id: String,
    pub abi_major: u32,
    pub link: LinkByPlatform,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkByPlatform {
    pub linux: LinkSpec,
    pub macos: LinkSpec,
    #[serde(rename = "windows-msvc")]
    pub windows_msvc: LinkSpec,
    #[serde(rename = "windows-gnu", default)]
    pub windows_gnu: Option<LinkSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkSpec {
    pub kind: String,
    pub files: Vec<String>,
    pub args: Vec<String>,
    #[serde(default)]
    pub search_paths: Vec<String>,
    #[serde(default)]
    pub force_load: bool,
    #[serde(default)]
    pub whole_archive: bool,
}

#[derive(Debug, Copy, Clone)]
enum HostPlatform {
    Linux,
    MacOS,
    WindowsMsvc,
    WindowsGnu,
}

pub fn plan_native_link_argv(
    toolchain_root: &Path,
    requires: &NativeRequires,
) -> Result<Vec<String>> {
    if requires.requires.is_empty() {
        return Ok(Vec::new());
    }

    let platform = host_platform()?;

    let manifest_path = toolchain_root.join("deps/x07/native_backends.json");
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("read native backends manifest: {}", manifest_path.display()))?;
    let manifest: NativeBackendsManifest =
        serde_json::from_str(&manifest_text).with_context(|| {
            format!(
                "parse native backends manifest: {}",
                manifest_path.display()
            )
        })?;
    if manifest.schema_version != NATIVE_BACKENDS_SCHEMA_VERSION {
        anyhow::bail!(
            "native backends manifest schema_version mismatch: expected {} got {}",
            NATIVE_BACKENDS_SCHEMA_VERSION,
            manifest.schema_version
        );
    }

    let mut backends: BTreeMap<&str, &NativeBackend> = BTreeMap::new();
    for backend in &manifest.backends {
        backends.insert(backend.backend_id.as_str(), backend);
    }

    let mut reqs = requires.requires.clone();
    reqs.sort_by(|a, b| a.backend_id.cmp(&b.backend_id));
    reqs.dedup_by(|a, b| a.backend_id == b.backend_id);

    let mut out: Vec<String> = Vec::new();
    let mut seen_args: BTreeSet<String> = BTreeSet::new();
    let mut libs: Vec<String> = Vec::new();
    let mut seen_libs: BTreeSet<String> = BTreeSet::new();

    for req in &reqs {
        let backend = backends
            .get(req.backend_id.as_str())
            .copied()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "missing backend_id in deps/x07/native_backends.json: {}",
                    req.backend_id
                )
            })?;

        if backend.abi_major != req.abi_major {
            anyhow::bail!(
                "native backend ABI mismatch for {}: requires abi_major={}, toolchain has abi_major={}",
                req.backend_id,
                req.abi_major,
                backend.abi_major
            );
        }

        let spec = match platform {
            HostPlatform::Linux => &backend.link.linux,
            HostPlatform::MacOS => &backend.link.macos,
            HostPlatform::WindowsMsvc => &backend.link.windows_msvc,
            HostPlatform::WindowsGnu => backend.link.windows_gnu.as_ref().with_context(|| {
                format!("backend {} missing link.windows-gnu", backend.backend_id)
            })?,
        };

        match spec.kind.as_str() {
            "static" | "dynamic" => {}
            other => anyhow::bail!(
                "native backend {} has unsupported link kind: {}",
                req.backend_id,
                other
            ),
        }

        for rel in &spec.search_paths {
            let full = join_rel(toolchain_root, rel)?;
            let flag = match platform {
                HostPlatform::Linux | HostPlatform::MacOS => format!("-L{}", full.display()),
                HostPlatform::WindowsMsvc | HostPlatform::WindowsGnu => {
                    format!("/LIBPATH:{}", full.display())
                }
            };
            if seen_args.insert(flag.clone()) {
                out.push(flag);
            }
        }

        for arg in &spec.args {
            if seen_args.insert(arg.clone()) {
                out.push(arg.clone());
            }
        }

        if spec.force_load {
            anyhow::bail!(
                "native backend {} uses force_load=true which is not supported yet",
                req.backend_id
            );
        }
        if spec.whole_archive {
            anyhow::bail!(
                "native backend {} uses whole_archive=true which is not supported yet",
                req.backend_id
            );
        }

        for rel in &spec.files {
            let full = join_rel(toolchain_root, rel)?;
            if !full.is_file() {
                anyhow::bail!(
                    "native backend file missing: backend_id={} path={}",
                    req.backend_id,
                    full.display()
                );
            }
            let s = full.to_string_lossy().to_string();
            if seen_libs.insert(s.clone()) {
                libs.push(s);
            }
        }
    }

    match platform {
        HostPlatform::Linux => {
            if !libs.is_empty() {
                out.push("-Wl,--start-group".to_string());
                out.extend(libs);
                out.push("-Wl,--end-group".to_string());
            }
        }
        HostPlatform::MacOS | HostPlatform::WindowsMsvc | HostPlatform::WindowsGnu => {
            out.extend(libs);
        }
    }

    Ok(out)
}

fn host_platform() -> Result<HostPlatform> {
    if cfg!(target_os = "linux") {
        return Ok(HostPlatform::Linux);
    }
    if cfg!(target_os = "macos") {
        return Ok(HostPlatform::MacOS);
    }
    if cfg!(windows) {
        if cfg!(target_env = "msvc") {
            return Ok(HostPlatform::WindowsMsvc);
        }
        return Ok(HostPlatform::WindowsGnu);
    }
    anyhow::bail!("unsupported host platform");
}

fn join_rel(root: &Path, rel: &str) -> Result<PathBuf> {
    if rel.is_empty() {
        anyhow::bail!("native backend relpath is empty");
    }
    if rel.starts_with('/') || rel.starts_with('\\') {
        anyhow::bail!("native backend relpath must be relative: {rel:?}");
    }
    if rel.contains('\\') {
        anyhow::bail!("native backend relpath must use forward slashes: {rel:?}");
    }
    if rel.split('/').any(|p| p == "..") {
        anyhow::bail!("native backend relpath must not contain '..': {rel:?}");
    }

    let mut out = PathBuf::from(root);
    for part in rel.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        out.push(part);
    }
    Ok(out)
}
