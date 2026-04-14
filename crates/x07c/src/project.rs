use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x07_contracts::{
    PACKAGE_MANIFEST_SCHEMA_VERSION, PROJECT_LOCKFILE_SCHEMA_VERSION,
    PROJECT_LOCKFILE_SCHEMA_VERSIONS_SUPPORTED, PROJECT_LOCKFILE_SCHEMA_VERSION_V0_2_0,
    PROJECT_LOCKFILE_SCHEMA_VERSION_V0_4_0, PROJECT_MANIFEST_SCHEMA_VERSION,
    PROJECT_MANIFEST_SCHEMA_VERSIONS_SUPPORTED, PROJECT_MANIFEST_SCHEMA_VERSION_V0_2_0,
    PROJECT_MANIFEST_SCHEMA_VERSION_V0_4_0, PROJECT_MANIFEST_SCHEMA_VERSION_V0_5_0,
};

fn workspace_path_remainder(raw: &str) -> Option<&str> {
    if raw == "$workspace" {
        return Some("");
    }
    raw.strip_prefix("$workspace/")
}

pub fn is_vendored_dep_path(raw: &str) -> bool {
    let raw = raw.trim();
    if raw.starts_with(".x07/deps/") || raw.starts_with("$workspace/.x07/deps/") {
        return true;
    }

    workspace_path_remainder(raw)
        .is_some_and(|remainder| remainder == ".x07/deps" || remainder.contains("/.x07/deps/"))
}

fn discover_workspace_root_from_git(base: &Path) -> Option<PathBuf> {
    let base = base.canonicalize().ok()?;
    for anc in base.ancestors() {
        let git = anc.join(".git");
        if git.is_dir() || git.is_file() {
            return Some(anc.to_path_buf());
        }
    }
    None
}

pub fn resolve_rel_path_with_workspace(base: &Path, raw: &str) -> Result<PathBuf> {
    let raw = raw.trim();
    let Some(remainder) = workspace_path_remainder(raw) else {
        return Ok(base.join(raw));
    };

    let root = match std::env::var_os("X07_WORKSPACE_ROOT") {
        Some(root) => PathBuf::from(root),
        None => discover_workspace_root_from_git(base).ok_or_else(|| {
            anyhow::anyhow!(
                "X07_WORKSPACE_ROOT must be set when using {raw:?} (or use $workspace within a git repo)"
            )
        })?,
    };
    if root.as_os_str().is_empty() {
        anyhow::bail!("X07_WORKSPACE_ROOT must be non-empty when using {raw:?}");
    }
    let root = root
        .canonicalize()
        .with_context(|| format!("canonicalize X07_WORKSPACE_ROOT: {}", root.display()))?;

    let resolved = if remainder.is_empty() {
        root.clone()
    } else {
        root.join(remainder)
    };

    if resolved.exists() {
        let resolved = resolved.canonicalize().with_context(|| {
            format!(
                "canonicalize $workspace path: {raw:?} -> {}",
                resolved.display()
            )
        })?;
        if !resolved.starts_with(&root) {
            anyhow::bail!(
                "$workspace path escapes workspace root: {raw:?} -> {} (root {})",
                resolved.display(),
                root.display()
            );
        }
        Ok(resolved)
    } else {
        Ok(resolved)
    }
}

fn validate_rel_path(field: &str, raw: &str) -> Result<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("{field} must be non-empty");
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        anyhow::bail!("{field} must be a relative path, got {:?}", raw);
    }
    for component in path.components() {
        if component == Component::ParentDir {
            anyhow::bail!("{field} must not contain '..' segments: {:?}", raw)
        }
    }
    Ok(())
}

fn validate_module_id(raw: &str) -> Result<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("module id must be non-empty");
    }
    if raw
        .as_bytes()
        .iter()
        .any(|&b| b == 0 || b == b'/' || b == b'\\')
    {
        anyhow::bail!("module id must not contain '/', '\\\\', or NUL: {:?}", raw);
    }
    for seg in raw.split('.') {
        if seg.is_empty() {
            anyhow::bail!("module id must not contain empty segments: {:?}", raw);
        }
        if seg == "." || seg == ".." {
            anyhow::bail!("module id must not contain '.' or '..' segments: {:?}", raw);
        }
    }
    Ok(())
}

fn validate_link_name(field: &str, raw: &str) -> Result<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("{field} must be non-empty");
    }
    if raw.starts_with('-') {
        anyhow::bail!("{field} must not start with '-': {:?}", raw);
    }
    if raw
        .as_bytes()
        .iter()
        .any(|&b| b == 0 || b.is_ascii_whitespace() || b == b'/' || b == b'\\')
    {
        anyhow::bail!(
            "{field} must not contain whitespace, '/', '\\\\', or NUL: {:?}",
            raw
        );
    }
    Ok(())
}

fn validate_pkg_name(field: &str, raw: &str) -> Result<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("{field} must be non-empty");
    }
    if !raw
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_lowercase())
        || raw
            .as_bytes()
            .iter()
            .any(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit() && !matches!(b, b'_' | b'-'))
    {
        anyhow::bail!("{field} must match ^[a-z][a-z0-9_-]*$, got {:?}", raw);
    }
    Ok(())
}

fn normalize_string_in_place(s: &mut String) {
    if s.trim() != s {
        *s = s.trim().to_string();
    }
}

fn normalize_vec_in_place(vec: &mut [String]) {
    for s in vec {
        normalize_string_in_place(s);
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectManifest {
    pub schema_version: String,
    #[serde(default)]
    pub compat: Option<String>,
    pub world: String,
    pub entry: String,
    #[serde(default)]
    pub operational_entry_symbol: Option<String>,
    #[serde(default)]
    pub certification_entry_symbol: Option<String>,
    pub module_roots: Vec<String>,
    #[serde(default)]
    pub link: LinkConfig,
    #[serde(default)]
    pub dependencies: Vec<DependencySpec>,
    #[serde(default)]
    pub patch: BTreeMap<String, PatchSpec>,
    #[serde(default)]
    pub lockfile: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatchSpec {
    pub version: String,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LinkConfig {
    pub libs: Vec<String>,
    pub search_paths: Vec<String>,
    pub frameworks: Vec<String>,
    #[serde(rename = "static")]
    pub static_link: bool,
}

impl LinkConfig {
    pub fn cc_args(&self, project_base: &Path) -> Vec<String> {
        let mut out = Vec::new();
        if self.static_link {
            out.push("-static".to_string());
        }
        for p in &self.search_paths {
            out.push("-L".to_string());
            out.push(project_base.join(p).display().to_string());
        }
        for lib in &self.libs {
            out.push("-l".to_string());
            out.push(lib.clone());
        }
        for fw in &self.frameworks {
            out.push("-framework".to_string());
            out.push(fw.clone());
        }
        out
    }

    fn normalize(&mut self) {
        normalize_vec_in_place(&mut self.libs);
        normalize_vec_in_place(&mut self.search_paths);
        normalize_vec_in_place(&mut self.frameworks);
    }

    fn validate(&self) -> Result<()> {
        for (idx, lib) in self.libs.iter().enumerate() {
            validate_link_name(&format!("project.link.libs[{idx}]"), lib)?;
        }
        for (idx, path) in self.search_paths.iter().enumerate() {
            validate_rel_path(&format!("project.link.search_paths[{idx}]"), path)?;
        }
        for (idx, fw) in self.frameworks.iter().enumerate() {
            validate_link_name(&format!("project.link.frameworks[{idx}]"), fw)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DependencySpec {
    pub name: String,
    pub version: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageManifest {
    pub schema_version: String,
    pub name: String,
    pub version: String,
    pub module_root: String,
    pub modules: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockfileToolchain {
    pub x07_version: String,
    pub x07c_version: String,
    pub lang_id: String,
    pub compat: String,
}

pub fn default_lockfile_toolchain(manifest: &ProjectManifest) -> LockfileToolchain {
    let compat = manifest
        .compat
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("current");
    LockfileToolchain {
        // `x07` and `x07c` ship as a single release bundle and are version-aligned.
        x07_version: crate::X07C_VERSION.to_string(),
        x07c_version: crate::X07C_VERSION.to_string(),
        lang_id: crate::language::LANG_ID.to_string(),
        compat: compat.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockfileRegistry {
    pub index_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_hash: Option<String>,
}

pub fn default_lockfile_registry() -> LockfileRegistry {
    let index_url = match std::env::var("X07_PKG_INDEX_URL") {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                x07_contracts::X07_PKG_DEFAULT_INDEX_URL.to_string()
            } else {
                trimmed.to_string()
            }
        }
        Err(_) => x07_contracts::X07_PKG_DEFAULT_INDEX_URL.to_string(),
    };
    LockfileRegistry {
        index_url,
        snapshot_hash: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lockfile {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<LockfileToolchain>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<LockfileRegistry>,
    pub dependencies: Vec<LockedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockAdvisory {
    pub schema_version: String,
    pub id: String,
    pub package: String,
    pub version: String,
    pub kind: String,
    pub severity: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    pub created_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withdrawn_at_utc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedDependency {
    pub name: String,
    pub version: String,
    pub path: String,
    pub package_manifest_sha256: String,
    pub module_root: String,
    pub modules_sha256: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overridden_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yanked: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisories: Vec<LockAdvisory>,
}

pub fn load_project_manifest(path: &Path) -> Result<ProjectManifest> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("[X07PROJECT_READ] read project: {}", path.display()))?;
    parse_project_manifest_bytes(&bytes, path)
}

pub fn parse_project_manifest_bytes(bytes: &[u8], path: &Path) -> Result<ProjectManifest> {
    let mut m: ProjectManifest = serde_json::from_slice(bytes)
        .with_context(|| format!("[X07PROJECT_PARSE] parse project JSON: {}", path.display()))?;

    normalize_string_in_place(&mut m.schema_version);
    if let Some(compat) = m.compat.as_mut() {
        normalize_string_in_place(compat);
    }
    if m.compat.as_deref().unwrap_or("").is_empty() {
        m.compat = None;
    }
    normalize_string_in_place(&mut m.world);
    normalize_string_in_place(&mut m.entry);
    if let Some(symbol) = m.operational_entry_symbol.as_mut() {
        normalize_string_in_place(symbol);
    }
    if m.operational_entry_symbol
        .as_deref()
        .unwrap_or("")
        .is_empty()
    {
        m.operational_entry_symbol = None;
    }
    if let Some(symbol) = m.certification_entry_symbol.as_mut() {
        normalize_string_in_place(symbol);
    }
    if m.certification_entry_symbol
        .as_deref()
        .unwrap_or("")
        .is_empty()
    {
        m.certification_entry_symbol = None;
    }
    normalize_vec_in_place(&mut m.module_roots);
    m.link.normalize();
    for dep in &mut m.dependencies {
        normalize_string_in_place(&mut dep.name);
        normalize_string_in_place(&mut dep.version);
        normalize_string_in_place(&mut dep.path);
    }
    let normalized_patch = {
        let mut out: BTreeMap<String, PatchSpec> = BTreeMap::new();
        for (raw_key, mut spec) in std::mem::take(&mut m.patch) {
            let key = raw_key.trim().to_string();
            if key.is_empty() {
                anyhow::bail!("project.patch key must be non-empty");
            }
            if out.contains_key(&key) {
                anyhow::bail!("project.patch has duplicate key {:?}", key);
            }
            normalize_string_in_place(&mut spec.version);
            if let Some(path) = spec.path.as_mut() {
                normalize_string_in_place(path);
                if path.is_empty() {
                    spec.path = None;
                }
            }
            out.insert(key, spec);
        }
        out
    };
    m.patch = normalized_patch;
    if let Some(lockfile) = m.lockfile.as_mut() {
        normalize_string_in_place(lockfile);
    }
    if m.lockfile.as_deref().unwrap_or("").is_empty() {
        m.lockfile = None;
    }

    if !PROJECT_MANIFEST_SCHEMA_VERSIONS_SUPPORTED
        .iter()
        .any(|v| *v == m.schema_version)
    {
        anyhow::bail!(
            "project schema_version mismatch: expected one of {:?} got {:?}",
            PROJECT_MANIFEST_SCHEMA_VERSIONS_SUPPORTED,
            m.schema_version
        );
    }
    if m.schema_version == PROJECT_MANIFEST_SCHEMA_VERSION_V0_2_0 && !m.patch.is_empty() {
        anyhow::bail!(
            "project.patch requires project schema_version {} (got {})",
            PROJECT_MANIFEST_SCHEMA_VERSION,
            PROJECT_MANIFEST_SCHEMA_VERSION_V0_2_0
        );
    }
    if m.compat.is_some() && m.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION_V0_5_0 {
        anyhow::bail!(
            "project.compat requires project schema_version {}",
            PROJECT_MANIFEST_SCHEMA_VERSION_V0_5_0
        );
    }

    let supports_entry_symbols = m.schema_version == PROJECT_MANIFEST_SCHEMA_VERSION_V0_4_0
        || m.schema_version == PROJECT_MANIFEST_SCHEMA_VERSION_V0_5_0;
    if !supports_entry_symbols {
        if m.operational_entry_symbol.is_some() {
            anyhow::bail!(
                "project.operational_entry_symbol requires project schema_version {}",
                PROJECT_MANIFEST_SCHEMA_VERSION_V0_4_0
            );
        }
        if m.certification_entry_symbol.is_some() {
            anyhow::bail!(
                "project.certification_entry_symbol requires project schema_version {}",
                PROJECT_MANIFEST_SCHEMA_VERSION_V0_4_0
            );
        }
    }
    crate::world_config::parse_world_id(&m.world)
        .with_context(|| format!("invalid project world {:?}", m.world))?;
    if m.entry.trim().is_empty() {
        anyhow::bail!("project entry must be non-empty");
    }
    validate_rel_path("project.entry", &m.entry)?;
    if !m.entry.ends_with(".x07.json") {
        anyhow::bail!("project entry must be a *.x07.json file, got {:?}", m.entry);
    }
    if m.module_roots.is_empty() {
        anyhow::bail!("project module_roots must be non-empty");
    }
    for (idx, root) in m.module_roots.iter().enumerate() {
        validate_rel_path(&format!("project.module_roots[{idx}]"), root)?;
    }
    m.link.validate()?;
    for (idx, dep) in m.dependencies.iter().enumerate() {
        validate_rel_path(&format!("project.dependencies[{idx}].path"), &dep.path)?;
    }
    for (name, spec) in &m.patch {
        validate_pkg_name("project.patch key", name)?;
        if spec.version.trim().is_empty() {
            anyhow::bail!("project.patch[{name:?}].version must be non-empty");
        }
        if let Some(path) = &spec.path {
            validate_rel_path(&format!("project.patch[{name:?}].path"), path)?;
        }
    }
    if let Some(lockfile) = &m.lockfile {
        validate_rel_path("project.lockfile", lockfile)?;
    }
    if let Some(symbol) = &m.operational_entry_symbol {
        crate::validate::validate_symbol(symbol)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("invalid project operational_entry_symbol {:?}", symbol))?;
    }
    if let Some(symbol) = &m.certification_entry_symbol {
        crate::validate::validate_symbol(symbol)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("invalid project certification_entry_symbol {:?}", symbol))?;
    }
    Ok(m)
}

pub fn default_lockfile_path(project_path: &Path, manifest: &ProjectManifest) -> PathBuf {
    let dir = project_path.parent().unwrap_or_else(|| Path::new("."));
    let name = manifest.lockfile.as_deref().unwrap_or("x07.lock.json");
    dir.join(name)
}

pub fn load_package_manifest(dir: &Path) -> Result<(PackageManifest, PathBuf, Vec<u8>)> {
    let path = dir.join("x07-package.json");
    let bytes =
        std::fs::read(&path).with_context(|| format!("read package: {}", path.display()))?;
    let mut m: PackageManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse package JSON: {}", path.display()))?;

    normalize_string_in_place(&mut m.schema_version);
    normalize_string_in_place(&mut m.name);
    normalize_string_in_place(&mut m.version);
    normalize_string_in_place(&mut m.module_root);
    normalize_vec_in_place(&mut m.modules);

    if m.schema_version != PACKAGE_MANIFEST_SCHEMA_VERSION {
        anyhow::bail!(
            "package schema_version mismatch: expected {} got {:?}",
            PACKAGE_MANIFEST_SCHEMA_VERSION,
            m.schema_version
        );
    }
    if m.name.trim().is_empty() {
        anyhow::bail!("package name must be non-empty: {}", path.display());
    }
    if m.version.trim().is_empty() {
        anyhow::bail!("package version must be non-empty: {}", path.display());
    }
    if m.module_root.trim().is_empty() {
        anyhow::bail!("package module_root must be non-empty: {}", path.display());
    }
    validate_rel_path("package.module_root", &m.module_root)?;
    for (idx, module_id) in m.modules.iter().enumerate() {
        validate_module_id(module_id)
            .with_context(|| format!("invalid package.modules[{idx}] in {}", path.display()))?;
    }
    Ok((m, path, bytes))
}

pub fn compute_lockfile(project_path: &Path, manifest: &ProjectManifest) -> Result<Lockfile> {
    let base = project_path.parent().unwrap_or_else(|| Path::new("."));

    let mut locked_deps = Vec::with_capacity(manifest.dependencies.len());
    for dep in &manifest.dependencies {
        let dep_dir = resolve_rel_path_with_workspace(base, &dep.path)?;
        let (pkg, pkg_manifest_path, pkg_manifest_bytes) = load_package_manifest(&dep_dir)?;
        if pkg.name != dep.name {
            anyhow::bail!(
                "dependency name mismatch: project wants {:?} but package at {} is {:?}",
                dep.name,
                pkg_manifest_path.display(),
                pkg.name
            );
        }
        if pkg.version != dep.version {
            anyhow::bail!(
                "dependency version mismatch: project wants {:?} but package at {} is {:?}",
                dep.version,
                pkg_manifest_path.display(),
                pkg.version
            );
        }

        let manifest_sha = sha256_hex(&pkg_manifest_bytes);
        let mut modules_sha256: BTreeMap<String, String> = BTreeMap::new();
        for module_id in &pkg.modules {
            let rel = format!("{}.x07.json", module_id.replace('.', "/"));
            let path = dep_dir.join(&pkg.module_root).join(rel);
            let bytes = std::fs::read(&path)
                .with_context(|| format!("read module {module_id:?}: {}", path.display()))?;
            modules_sha256.insert(module_id.clone(), sha256_hex(&bytes));
        }

        locked_deps.push(LockedDependency {
            name: dep.name.clone(),
            version: dep.version.clone(),
            path: dep.path.clone(),
            package_manifest_sha256: manifest_sha,
            module_root: pkg.module_root.clone(),
            modules_sha256,
            overridden_by: None,
            yanked: is_vendored_dep_path(&dep.path).then_some(false),
            advisories: Vec::new(),
        });
    }

    locked_deps.sort_by(|a, b| {
        (
            a.name.as_str(),
            a.version.as_str(),
            a.path.as_str(),
            a.module_root.as_str(),
        )
            .cmp(&(
                b.name.as_str(),
                b.version.as_str(),
                b.path.as_str(),
                b.module_root.as_str(),
            ))
    });

    Ok(Lockfile {
        schema_version: PROJECT_LOCKFILE_SCHEMA_VERSION.to_string(),
        toolchain: Some(default_lockfile_toolchain(manifest)),
        registry: Some(default_lockfile_registry()),
        dependencies: locked_deps,
    })
}

pub fn verify_lockfile(
    project_path: &Path,
    manifest: &ProjectManifest,
    lock: &Lockfile,
) -> Result<()> {
    if !PROJECT_LOCKFILE_SCHEMA_VERSIONS_SUPPORTED
        .iter()
        .any(|v| *v == lock.schema_version.trim())
    {
        let hint = if lock.schema_version.trim() == PROJECT_LOCKFILE_SCHEMA_VERSION_V0_2_0 {
            format!(
                " (hint: run `x07 pkg lock` to update to {})",
                PROJECT_LOCKFILE_SCHEMA_VERSION
            )
        } else {
            " (hint: run `x07 pkg lock`)".to_string()
        };
        anyhow::bail!(
            "[X07LOCK_SCHEMA] lockfile schema_version mismatch: expected one of {:?} got {:?}{}",
            PROJECT_LOCKFILE_SCHEMA_VERSIONS_SUPPORTED,
            lock.schema_version,
            hint
        );
    }

    if lock.schema_version.trim() == PROJECT_LOCKFILE_SCHEMA_VERSION_V0_4_0 {
        if lock.toolchain.is_none() {
            anyhow::bail!("[X07LOCK_SCHEMA] lockfile is missing required field toolchain");
        }
        if lock.registry.is_none() {
            anyhow::bail!("[X07LOCK_SCHEMA] lockfile is missing required field registry");
        }
    }

    let expected = compute_lockfile(project_path, manifest)?;
    let hint = {
        let uses_workspace_paths = manifest
            .module_roots
            .iter()
            .any(|p| workspace_path_remainder(p).is_some())
            || manifest
                .dependencies
                .iter()
                .any(|d| workspace_path_remainder(&d.path).is_some());
        if uses_workspace_paths {
            format!(
                " (hint: run `X07_WORKSPACE_ROOT=... x07 pkg lock --project {}`)",
                project_path.display()
            )
        } else {
            format!(
                " (hint: run `x07 pkg lock --project {}`)",
                project_path.display()
            )
        }
    };

    if lock.dependencies.len() != expected.dependencies.len() {
        anyhow::bail!("lockfile dependency list does not match project{hint}");
    }

    let mut actual_deps: Vec<&LockedDependency> = lock.dependencies.iter().collect();
    let mut expected_deps: Vec<&LockedDependency> = expected.dependencies.iter().collect();
    actual_deps.sort_by(|a, b| {
        (
            a.name.as_str(),
            a.version.as_str(),
            a.path.as_str(),
            a.module_root.as_str(),
            a.package_manifest_sha256.as_str(),
        )
            .cmp(&(
                b.name.as_str(),
                b.version.as_str(),
                b.path.as_str(),
                b.module_root.as_str(),
                b.package_manifest_sha256.as_str(),
            ))
    });
    expected_deps.sort_by(|a, b| {
        (
            a.name.as_str(),
            a.version.as_str(),
            a.path.as_str(),
            a.module_root.as_str(),
            a.package_manifest_sha256.as_str(),
        )
            .cmp(&(
                b.name.as_str(),
                b.version.as_str(),
                b.path.as_str(),
                b.module_root.as_str(),
                b.package_manifest_sha256.as_str(),
            ))
    });

    for (a, b) in actual_deps.iter().zip(expected_deps.iter()) {
        if a.name != b.name || a.version != b.version || a.path != b.path {
            anyhow::bail!("lockfile dependencies do not match project manifest{hint}");
        }
        if a.package_manifest_sha256 != b.package_manifest_sha256 {
            anyhow::bail!(
                "lockfile package manifest hash mismatch for {:?}{hint}",
                a.name
            );
        }
        if a.module_root != b.module_root {
            anyhow::bail!("lockfile module_root mismatch for {:?}{hint}", a.name);
        }
        if a.modules_sha256 != b.modules_sha256 {
            anyhow::bail!("lockfile module hashes mismatch for {:?}{hint}", a.name);
        }
    }

    Ok(())
}

pub fn collect_module_roots(
    project_path: &Path,
    manifest: &ProjectManifest,
    lock: &Lockfile,
) -> Result<Vec<PathBuf>> {
    let base = project_path.parent().unwrap_or_else(|| Path::new("."));

    fn normalize_module_root_path(path: PathBuf) -> PathBuf {
        let mut out = PathBuf::new();
        for component in path.components() {
            if component == Component::CurDir {
                continue;
            }
            out.push(Path::new(component.as_os_str()));
        }
        out
    }

    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut roots: Vec<PathBuf> = Vec::new();
    for r in &manifest.module_roots {
        let root = normalize_module_root_path(resolve_rel_path_with_workspace(base, r)?);
        if seen.insert(root.clone()) {
            roots.push(root);
        }
    }
    for dep in &lock.dependencies {
        let dep_dir = resolve_rel_path_with_workspace(base, &dep.path)?;
        let root = normalize_module_root_path(dep_dir.join(&dep.module_root));
        if seen.insert(root.clone()) {
            roots.push(root);
        }
    }
    Ok(roots)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "x07c_project_{prefix}_{}_{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn load_project_manifest_accepts_v0_4_0_operational_entry() {
        let dir = temp_dir("manifest_v0_4");
        let path = dir.join("x07.json");
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": "x07.project@0.4.0",
                "world": "solve-pure",
                "entry": "src/main.x07.json",
                "operational_entry_symbol": "app.main",
                "module_roots": ["src"],
                "dependencies": []
            }))
            .expect("serialize manifest"),
        )
        .expect("write manifest");

        let manifest = load_project_manifest(&path).expect("load project manifest");
        assert_eq!(manifest.schema_version, "x07.project@0.4.0");
        assert_eq!(
            manifest.operational_entry_symbol.as_deref(),
            Some("app.main")
        );
        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }
}
