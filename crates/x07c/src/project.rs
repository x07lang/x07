use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x07_contracts::{
    PACKAGE_MANIFEST_SCHEMA_VERSION, PROJECT_LOCKFILE_SCHEMA_VERSION,
    PROJECT_MANIFEST_SCHEMA_VERSION,
};

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
        match component {
            std::path::Component::ParentDir => {
                anyhow::bail!("{field} must not contain '..' segments: {:?}", raw)
            }
            std::path::Component::Prefix(_) => {
                anyhow::bail!("{field} must not contain Windows path prefixes: {:?}", raw)
            }
            _ => {}
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
    pub world: String,
    pub entry: String,
    pub module_roots: Vec<String>,
    #[serde(default)]
    pub link: LinkConfig,
    #[serde(default)]
    pub dependencies: Vec<DependencySpec>,
    #[serde(default)]
    pub lockfile: Option<String>,
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
pub struct Lockfile {
    pub schema_version: String,
    pub dependencies: Vec<LockedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedDependency {
    pub name: String,
    pub version: String,
    pub path: String,
    pub package_manifest_sha256: String,
    pub module_root: String,
    pub modules_sha256: BTreeMap<String, String>,
}

pub fn load_project_manifest(path: &Path) -> Result<ProjectManifest> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("[X07PROJECT_READ] read project: {}", path.display()))?;
    let mut m: ProjectManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("[X07PROJECT_PARSE] parse project JSON: {}", path.display()))?;

    normalize_string_in_place(&mut m.schema_version);
    normalize_string_in_place(&mut m.world);
    normalize_string_in_place(&mut m.entry);
    normalize_vec_in_place(&mut m.module_roots);
    m.link.normalize();
    for dep in &mut m.dependencies {
        normalize_string_in_place(&mut dep.name);
        normalize_string_in_place(&mut dep.version);
        normalize_string_in_place(&mut dep.path);
    }
    if let Some(lockfile) = m.lockfile.as_mut() {
        normalize_string_in_place(lockfile);
    }
    if m.lockfile.as_deref().unwrap_or("").is_empty() {
        m.lockfile = None;
    }

    if m.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION {
        anyhow::bail!(
            "project schema_version mismatch: expected {} got {:?}",
            PROJECT_MANIFEST_SCHEMA_VERSION,
            m.schema_version
        );
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
    if let Some(lockfile) = &m.lockfile {
        validate_rel_path("project.lockfile", lockfile)?;
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
        let dep_dir = base.join(&dep.path);
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
        });
    }

    Ok(Lockfile {
        schema_version: PROJECT_LOCKFILE_SCHEMA_VERSION.to_string(),
        dependencies: locked_deps,
    })
}

pub fn verify_lockfile(
    project_path: &Path,
    manifest: &ProjectManifest,
    lock: &Lockfile,
) -> Result<()> {
    if lock.schema_version.trim() != PROJECT_LOCKFILE_SCHEMA_VERSION {
        anyhow::bail!(
            "[X07LOCK_SCHEMA] lockfile schema_version mismatch: expected {} got {:?} (hint: run `x07 pkg lock`)",
            PROJECT_LOCKFILE_SCHEMA_VERSION,
            lock.schema_version
        );
    }

    let expected = compute_lockfile(project_path, manifest)?;

    if lock.dependencies.len() != expected.dependencies.len() {
        anyhow::bail!("lockfile dependency list does not match project");
    }

    for (a, b) in lock.dependencies.iter().zip(expected.dependencies.iter()) {
        if a.name != b.name || a.version != b.version || a.path != b.path {
            anyhow::bail!("lockfile dependencies do not match project manifest");
        }
        if a.package_manifest_sha256 != b.package_manifest_sha256 {
            anyhow::bail!("lockfile package manifest hash mismatch for {:?}", a.name);
        }
        if a.module_root != b.module_root {
            anyhow::bail!("lockfile module_root mismatch for {:?}", a.name);
        }
        if a.modules_sha256 != b.modules_sha256 {
            anyhow::bail!("lockfile module hashes mismatch for {:?}", a.name);
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
        let root = normalize_module_root_path(base.join(r));
        if seen.insert(root.clone()) {
            roots.push(root);
        }
    }
    for dep in &lock.dependencies {
        let root = normalize_module_root_path(base.join(&dep.path).join(&dep.module_root));
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
