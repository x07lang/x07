use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use serde::Deserialize;
use serde_json::Value;

use crate::run;
use crate::util;

#[derive(Debug, Args)]
pub struct DocArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Module root directories (defaults to project roots if available).
    #[arg(long, value_name = "DIR")]
    pub module_root: Vec<PathBuf>,

    /// Module id (example: `ext.cli`) or exported symbol (example: `ext.cli.parse_specrows`).
    #[arg(value_name = "QUERY")]
    pub query: String,
}

pub fn cmd_doc(args: DocArgs) -> Result<std::process::ExitCode> {
    let query = args.query.trim();
    if query.is_empty() {
        anyhow::bail!("missing QUERY (try --help)");
    }

    let cwd = std::env::current_dir().context("get cwd")?;

    let project_path = match args.project {
        Some(p) => Some(util::resolve_existing_path_upwards(&p)),
        None => run::discover_project_manifest(&cwd)?,
    };

    let module_roots = if !args.module_root.is_empty() {
        args.module_root.clone()
    } else if let Some(project_path) = project_path.as_deref() {
        resolve_project_module_roots(project_path)?
    } else {
        vec![cwd.clone()]
    };

    if query.contains('/') || query.contains('\\') || query.ends_with(".x07.json") {
        let path = util::resolve_existing_path_upwards_from(&cwd, Path::new(query));
        let (module_id, exports) = parse_module_file(&path)?;
        print_module(&module_id, &path, &exports);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    if let Some(path) = find_module_file(query, &module_roots) {
        let (module_id, exports) = parse_module_file(&path)?;
        print_module(&module_id, &path, &exports);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    if let Some((module_id, _suffix)) = query.rsplit_once('.') {
        if let Some(path) = find_module_file(module_id, &module_roots) {
            let (_mid, exports) = parse_module_file(&path)?;
            if let Some(sig) = exports.get(query) {
                print_symbol(query, sig);
                return Ok(std::process::ExitCode::SUCCESS);
            }
            anyhow::bail!("symbol not exported by {module_id}: {query}");
        }
    }

    if query.starts_with("std.") && try_print_stdlib_docs(query, &cwd)? {
        return Ok(std::process::ExitCode::SUCCESS);
    }

    if let Some(project_path) = project_path.as_deref() {
        if try_print_package_docs(query, project_path)? {
            return Ok(std::process::ExitCode::SUCCESS);
        }
    }

    anyhow::bail!("module/symbol not found: {query}");
}

fn resolve_project_module_roots(project_path: &Path) -> Result<Vec<PathBuf>> {
    let manifest = x07c::project::load_project_manifest(project_path).context("load project")?;
    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut roots = Vec::new();
    for r in &manifest.module_roots {
        roots.push(base.join(r));
    }
    for dep in &manifest.dependencies {
        let dep_dir = base.join(&dep.path);
        let (pkg, _, _) = x07c::project::load_package_manifest(&dep_dir).with_context(|| {
            format!(
                "load package manifest for {:?}@{:?} from {}",
                dep.name,
                dep.version,
                dep_dir.display()
            )
        })?;
        roots.push(dep_dir.join(pkg.module_root));
    }
    Ok(roots)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PackageDocManifest {
    name: String,
    version: String,
    description: String,
    docs: String,
    module_root: String,
    modules: Vec<String>,
}

fn try_print_package_docs(query: &str, project_path: &Path) -> Result<bool> {
    let (pkg, _pkg_dir) = match resolve_project_package_by_query(project_path, query)? {
        Some(v) => v,
        None => return Ok(false),
    };

    let name = pkg.name.trim();
    let version = pkg.version.trim();
    if name.is_empty() || version.is_empty() {
        return Ok(false);
    }

    println!("package: {name}@{version}");
    if !pkg.description.trim().is_empty() {
        println!("description: {}", pkg.description.trim());
    }
    if !pkg.modules.is_empty() {
        println!("modules:");
        for module_id in &pkg.modules {
            println!("  - {module_id}");
        }
        if let Some(first) = pkg.modules.first() {
            println!("hint: x07 doc {first}");
        }
        return Ok(true);
    }
    if !pkg.docs.trim().is_empty() {
        println!("docs:\n{}", pkg.docs.trim_end());
        return Ok(true);
    }

    Ok(false)
}

fn resolve_project_package_by_query(
    project_path: &Path,
    query: &str,
) -> Result<Option<(PackageDocManifest, PathBuf)>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(None);
    }
    let want = query.split('@').next().unwrap_or(query).trim();
    if want.is_empty() {
        return Ok(None);
    }

    let manifest = x07c::project::load_project_manifest(project_path).context("load project")?;
    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let dep = manifest.dependencies.iter().find(|d| d.name == want);
    let Some(dep) = dep else {
        return Ok(None);
    };
    let dep_dir = base.join(&dep.path);
    let pkg_path = dep_dir.join("x07-package.json");
    if !pkg_path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&pkg_path)
        .with_context(|| format!("read package: {}", pkg_path.display()))?;
    let pkg: PackageDocManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse JSON: {}", pkg_path.display()))?;
    Ok(Some((pkg, dep_dir)))
}

#[derive(Debug, Clone)]
struct ExportSig {
    kind: String,
    params: Vec<(String, String)>,
    result: String,
}

fn parse_module_file(
    path: &Path,
) -> Result<(String, std::collections::BTreeMap<String, ExportSig>)> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read module file: {}", path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse JSON: {}", path.display()))?;
    let obj = doc
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("module file must be a JSON object"))?;

    let module_id = obj
        .get("module_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("module file missing module_id"))?
        .trim()
        .to_string();
    if module_id.is_empty() {
        anyhow::bail!("module file has empty module_id");
    }

    let decls = obj
        .get("decls")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("module file missing decls[]"))?;

    let mut exported: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for decl in decls {
        let Some(kind) = decl.get("kind").and_then(Value::as_str) else {
            continue;
        };
        if kind != "export" {
            continue;
        }
        let Some(names) = decl.get("names").and_then(Value::as_array) else {
            continue;
        };
        for name in names {
            let Some(name) = name.as_str() else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            exported.insert(name.to_string());
        }
    }

    let mut sigs: std::collections::BTreeMap<String, ExportSig> = std::collections::BTreeMap::new();
    for name in &exported {
        sigs.insert(
            name.clone(),
            ExportSig {
                kind: "export".to_string(),
                params: Vec::new(),
                result: String::new(),
            },
        );
    }
    for decl in decls {
        let Some(kind) = decl.get("kind").and_then(Value::as_str) else {
            continue;
        };
        if kind != "defn" && kind != "defasync" {
            continue;
        }
        let Some(name) = decl.get("name").and_then(Value::as_str) else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || !exported.contains(name) {
            continue;
        }
        let params = decl.get("params").and_then(Value::as_array);
        let mut out_params = Vec::new();
        if let Some(params) = params {
            for p in params {
                let pname = p.get("name").and_then(Value::as_str).unwrap_or("").trim();
                let pty = p.get("ty").and_then(Value::as_str).unwrap_or("").trim();
                out_params.push((pname.to_string(), pty.to_string()));
            }
        }
        let result = decl
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        sigs.insert(
            name.to_string(),
            ExportSig {
                kind: kind.to_string(),
                params: out_params,
                result,
            },
        );
    }

    Ok((module_id, sigs))
}

fn find_module_file(module_id: &str, module_roots: &[PathBuf]) -> Option<PathBuf> {
    let rel = format!("{}.x07.json", module_id.trim().replace('.', "/"));
    for root in module_roots {
        let path = root.join(&rel);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn print_module(
    module_id: &str,
    path: &Path,
    exports: &std::collections::BTreeMap<String, ExportSig>,
) {
    println!("module: {module_id}");
    println!("file: {}", path.display());
    if exports.is_empty() {
        println!("exports: (none)");
        return;
    }
    println!("exports:");
    for (name, sig) in exports {
        print!("  - {name}(");
        for (idx, (pname, pty)) in sig.params.iter().enumerate() {
            if idx != 0 {
                print!(", ");
            }
            if pname.is_empty() && pty.is_empty() {
                continue;
            }
            if pname.is_empty() {
                print!("{pty}");
            } else if pty.is_empty() {
                print!("{pname}");
            } else {
                print!("{pname}: {pty}");
            }
        }
        if sig.result.is_empty() {
            println!(")");
        } else {
            println!(") -> {}", sig.result);
        }
    }
}

fn print_symbol(symbol: &str, sig: &ExportSig) {
    print!("{symbol}(");
    for (idx, (pname, pty)) in sig.params.iter().enumerate() {
        if idx != 0 {
            print!(", ");
        }
        if pname.is_empty() && pty.is_empty() {
            continue;
        }
        if pname.is_empty() {
            print!("{pty}");
        } else if pty.is_empty() {
            print!("{pname}");
        } else {
            print!("{pname}: {pty}");
        }
    }
    if sig.result.is_empty() {
        println!(")");
    } else {
        println!(") -> {}", sig.result);
    }
    if sig.kind != "defn" {
        println!("kind: {}", sig.kind);
    }
}

#[derive(Debug, Deserialize)]
struct StdlibLockFile {
    format: String,
    packages: Vec<StdlibLockPackage>,
}

#[derive(Debug, Deserialize)]
struct StdlibLockPackage {
    name: String,
    path: String,
    version: String,
}

#[derive(Debug, Clone)]
struct StdlibResolvedPackage {
    version: String,
    path: PathBuf,
}

#[derive(Debug)]
struct StdlibLocks {
    base_dir: PathBuf,
    stdlib_lock: PathBuf,
    stdlib_os_lock: Option<PathBuf>,
}

fn parse_semver_triplet(v: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = v.trim().split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let major: u32 = parts[0].parse().ok()?;
    let minor: u32 = parts[1].parse().ok()?;
    let patch: u32 = parts[2].parse().ok()?;
    Some((major, minor, patch))
}

fn resolve_stdlib_lock_path_best_effort(cwd: &Path) -> Option<PathBuf> {
    let cand = util::resolve_existing_path_upwards_from(cwd, Path::new("stdlib.lock"));
    if cand.is_file() {
        return Some(cand);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for base in [Some(exe_dir), exe_dir.parent()] {
                let Some(base) = base else { continue };
                let cand = base.join("stdlib.lock");
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
    }

    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let toolchains_dir = home.join(".x07").join("toolchains");
    let mut best: Option<((u32, u32, u32), PathBuf)> = None;
    for entry in std::fs::read_dir(&toolchains_dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path.file_name()?.to_string_lossy();
        let dir_name = dir_name.strip_prefix('v').unwrap_or(&dir_name);
        let Some(ver) = parse_semver_triplet(dir_name) else {
            continue;
        };
        let lock = path.join("stdlib.lock");
        if !lock.is_file() {
            continue;
        }
        if best.as_ref().map(|(b, _)| ver > *b).unwrap_or(true) {
            best = Some((ver, lock));
        }
    }

    best.map(|(_, p)| p)
}

fn resolve_stdlib_locks_best_effort(cwd: &Path) -> Option<StdlibLocks> {
    let stdlib_lock = resolve_stdlib_lock_path_best_effort(cwd)?;
    let base_dir = stdlib_lock
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let os_lock = base_dir.join("stdlib.os.lock");
    let stdlib_os_lock = os_lock.is_file().then_some(os_lock);
    Some(StdlibLocks {
        base_dir,
        stdlib_lock,
        stdlib_os_lock,
    })
}

fn load_stdlib_index(locks: &StdlibLocks) -> Result<BTreeMap<String, StdlibResolvedPackage>> {
    fn load_lock(
        out: &mut BTreeMap<String, StdlibResolvedPackage>,
        base_dir: &Path,
        lock_path: &Path,
    ) -> Result<()> {
        let bytes = std::fs::read(lock_path)
            .with_context(|| format!("read stdlib lock: {}", lock_path.display()))?;
        let lock: StdlibLockFile = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse stdlib lock JSON: {}", lock_path.display()))?;
        if lock.format.trim() != "x07.stdlib.lock@0.1.0" {
            anyhow::bail!(
                "unsupported stdlib lock format: {} (expected x07.stdlib.lock@0.1.0)",
                lock.format
            );
        }

        for pkg in lock.packages {
            let name = pkg.name.trim();
            if name.is_empty() {
                continue;
            }
            let version = pkg.version.trim().to_string();
            let path = pkg.path.trim();
            if path.is_empty() {
                continue;
            }
            let abs = if Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else {
                base_dir.join(path)
            };

            let replace = match out.get(name) {
                None => true,
                Some(existing) => {
                    let a = parse_semver_triplet(&version).unwrap_or((0, 0, 0));
                    let b = parse_semver_triplet(&existing.version).unwrap_or((0, 0, 0));
                    a > b
                }
            };
            if replace {
                out.insert(
                    name.to_string(),
                    StdlibResolvedPackage { version, path: abs },
                );
            }
        }
        Ok(())
    }

    let mut out: BTreeMap<String, StdlibResolvedPackage> = BTreeMap::new();
    load_lock(&mut out, &locks.base_dir, &locks.stdlib_lock)?;
    if let Some(os_lock) = locks.stdlib_os_lock.as_deref() {
        load_lock(&mut out, &locks.base_dir, os_lock)?;
    }
    Ok(out)
}

fn try_print_stdlib_docs(query: &str, cwd: &Path) -> Result<bool> {
    let Some(locks) = resolve_stdlib_locks_best_effort(cwd) else {
        anyhow::bail!(
            "could not locate stdlib.lock (try running from the x07 repo root, or install a toolchain via x07up)"
        );
    };
    let idx = load_stdlib_index(&locks)?;

    if let Some(pkg) = idx.get(query) {
        let (module_id, exports) = parse_module_file(&pkg.path)?;
        print_module(&module_id, &pkg.path, &exports);
        return Ok(true);
    }

    if let Some((module_id, _suffix)) = query.rsplit_once('.') {
        if let Some(pkg) = idx.get(module_id) {
            let (_mid, exports) = parse_module_file(&pkg.path)?;
            if let Some(sig) = exports.get(query) {
                print_symbol(query, sig);
                return Ok(true);
            }
            anyhow::bail!("symbol not exported by {module_id}: {query}");
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let base = std::env::temp_dir();
        let pid = std::process::id();
        for n in 0..10_000u32 {
            let p = base.join(format!("x07-doc-{prefix}-{pid}-{n}"));
            if std::fs::create_dir(&p).is_ok() {
                return p;
            }
        }
        panic!("failed to create temp dir under {}", base.display());
    }

    #[test]
    fn resolves_package_manifest_from_project_deps() {
        let root = make_temp_dir("resolve_pkg");
        let project_path = root.join("x07.json");

        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("deps/ext-net/0.1.4")).unwrap();
        std::fs::write(
            &project_path,
            r#"{
  "schema_version": "x07.project@0.2.0",
  "world": "run-os",
  "entry": "src/main.x07.json",
  "module_roots": ["src"],
  "dependencies": [{"name":"ext-net","version":"0.1.4","path":"deps/ext-net/0.1.4"}],
  "lockfile": "x07.lock.json"
}
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("deps/ext-net/0.1.4/x07-package.json"),
            r#"{
  "schema_version": "x07.package@0.1.0",
  "name": "ext-net",
  "version": "0.1.4",
  "description": "Networking APIs",
  "docs": "Use ext-net",
  "module_root": "modules",
  "modules": ["std.net.http.client"]
}
"#,
        )
        .unwrap();

        let (pkg, _dir) = resolve_project_package_by_query(&project_path, "ext-net")
            .unwrap()
            .unwrap();
        assert_eq!(pkg.name, "ext-net");
        assert_eq!(pkg.version, "0.1.4");
        assert_eq!(pkg.modules, vec!["std.net.http.client"]);

        let (pkg2, _dir2) = resolve_project_package_by_query(&project_path, "ext-net@0.1.4")
            .unwrap()
            .unwrap();
        assert_eq!(pkg2.name, "ext-net");

        std::fs::remove_dir_all(&root).unwrap();
    }
}
