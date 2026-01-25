use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

fn dedup_cc_args(args: &mut Vec<String>) {
    let mut seen: HashSet<String> = HashSet::new();
    args.retain(|a| seen.insert(a.clone()));
}

fn find_package_manifest_for_module_root(module_root: &Path) -> Option<PathBuf> {
    let direct = module_root.join("x07-package.json");
    if direct.is_file() {
        return Some(direct);
    }
    let parent = module_root.parent()?.join("x07-package.json");
    if parent.is_file() {
        return Some(parent);
    }
    None
}

fn brew_prefix_openssl() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }

    for formula in ["openssl@3", "openssl@1.1", "openssl"] {
        let out = Command::new("brew")
            .args(["--prefix", formula])
            .output()
            .ok()?;
        if !out.status.success() {
            continue;
        }
        let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if prefix.is_empty() {
            continue;
        }
        let prefix = PathBuf::from(prefix);
        if prefix.join("include").is_dir() && prefix.join("lib").is_dir() {
            return Some(prefix);
        }
    }
    None
}

pub fn collect_auto_ffi_cc_args(module_roots: &[PathBuf]) -> Result<Vec<String>> {
    let mut include_args: Vec<String> = Vec::new();
    let mut source_args: Vec<String> = Vec::new();
    let mut lib_search_args: Vec<String> = Vec::new();
    let mut lib_args: Vec<String> = Vec::new();

    let mut seen_packages: HashSet<PathBuf> = HashSet::new();
    let mut seen_sources: HashSet<String> = HashSet::new();
    let mut seen_includes: HashSet<String> = HashSet::new();
    let mut seen_lib_search: HashSet<String> = HashSet::new();
    let mut seen_libs: HashSet<String> = HashSet::new();

    let mut need_openssl_prefix = false;
    let mut need_winsock = false;

    for module_root in module_roots {
        let Some(manifest_path) = find_package_manifest_for_module_root(module_root) else {
            continue;
        };
        if !manifest_path.is_file() {
            continue;
        }

        let manifest_path = std::fs::canonicalize(&manifest_path).unwrap_or(manifest_path);
        let pkg_root = manifest_path
            .parent()
            .context("package manifest missing parent dir")?
            .to_path_buf();
        if !seen_packages.insert(pkg_root.clone()) {
            continue;
        }

        let txt = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("read package manifest: {}", manifest_path.display()))?;
        let doc: serde_json::Value = serde_json::from_str(&txt)
            .with_context(|| format!("parse package manifest JSON: {}", manifest_path.display()))?;

        let import_mode = doc
            .get("meta")
            .and_then(|v| v.get("import_mode"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if import_mode != "ffi" {
            continue;
        }

        let name = doc
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if cfg!(windows) && name == "ext-sockets-c" {
            need_winsock = true;
        }

        let mut ffi_libs: Vec<String> = Vec::new();
        if let Some(libs) = doc
            .get("meta")
            .and_then(|v| v.get("ffi_libs"))
            .and_then(|v| v.as_array())
        {
            for lib in libs {
                if let Some(lib) = lib.as_str() {
                    ffi_libs.push(lib.to_string());
                }
            }
        }

        if ffi_libs.iter().any(|l| l == "ssl" || l == "crypto") {
            need_openssl_prefix = true;
        }

        let ffi_dir = pkg_root.join("ffi");
        if !ffi_dir.is_dir() {
            anyhow::bail!(
                "package {:?} is meta.import_mode=ffi but missing ffi directory: {}",
                name,
                ffi_dir.display()
            );
        }

        let mut c_files: Vec<PathBuf> = Vec::new();
        for entry in std::fs::read_dir(&ffi_dir)
            .with_context(|| format!("list ffi dir: {}", ffi_dir.display()))?
        {
            let entry =
                entry.with_context(|| format!("read ffi dir entry: {}", ffi_dir.display()))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("c") {
                c_files.push(path);
            }
        }
        c_files.sort();
        if c_files.is_empty() {
            anyhow::bail!(
                "package {:?} is meta.import_mode=ffi but has no ffi/*.c sources: {}",
                name,
                ffi_dir.display()
            );
        }
        for p in c_files {
            let p = std::fs::canonicalize(&p).unwrap_or(p);
            let arg = p.display().to_string();
            if seen_sources.insert(arg.clone()) {
                source_args.push(arg);
            }
        }

        for lib in ffi_libs {
            let arg = format!("-l{lib}");
            if seen_libs.insert(arg.clone()) {
                lib_args.push(arg);
            }
        }
    }

    if cfg!(target_os = "macos") && need_openssl_prefix {
        if let Some(prefix) = brew_prefix_openssl() {
            let inc = format!("-I{}", prefix.join("include").display());
            if seen_includes.insert(inc.clone()) {
                include_args.push(inc);
            }

            let libdir = prefix.join("lib");
            let libdir_s = libdir.display().to_string();
            let l = format!("-L{libdir_s}");
            if seen_lib_search.insert(l.clone()) {
                lib_search_args.push(l);
            }
            let r = format!("-Wl,-rpath,{libdir_s}");
            if seen_lib_search.insert(r.clone()) {
                lib_search_args.push(r);
            }
        }
    }

    if need_winsock {
        let arg = "-lws2_32".to_string();
        if seen_libs.insert(arg.clone()) {
            lib_args.push(arg);
        }
    }

    let mut out = Vec::new();
    out.extend(include_args);
    out.extend(source_args);
    out.extend(lib_search_args);
    out.extend(lib_args);
    dedup_cc_args(&mut out);
    Ok(out)
}
