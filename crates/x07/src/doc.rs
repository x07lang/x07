use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
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
