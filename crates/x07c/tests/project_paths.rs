use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use x07_contracts::{PACKAGE_MANIFEST_SCHEMA_VERSION, PROJECT_MANIFEST_SCHEMA_VERSION};
use x07c::project;

fn create_temp_dir(prefix: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir();
    let pid = std::process::id();
    for _ in 0..10_000 {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!("{prefix}_{pid}_{n}"));
        if std::fs::create_dir(&path).is_ok() {
            return path;
        }
    }
    panic!("failed to create temp dir under {}", base.display());
}

fn rm_rf(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn project_manifest_rejects_absolute_entry() {
    let dir = create_temp_dir("x07_project_paths");
    let path = dir.join("x07.json");
    let abs_entry = dir.join("abs_entry.x07.json");
    let manifest = json!({
        "schema_version": "x07.project@0.2.0",
        "world": "solve-pure",
        "entry": abs_entry,
        "module_roots": ["src"],
    });
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&manifest).expect("encode project manifest"),
    )
    .expect("write project manifest");

    let err = project::load_project_manifest(&path).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("project.entry must be a relative path"));

    rm_rf(&dir);
}

#[test]
fn project_manifest_rejects_parent_dir_entry() {
    let dir = create_temp_dir("x07_project_paths");
    let path = dir.join("x07.json");
    std::fs::write(
        &path,
        r#"
        {
          "schema_version": "x07.project@0.2.0",
          "world": "solve-pure",
          "entry": "../main.x07.json",
          "module_roots": ["src"]
        }
        "#,
    )
    .expect("write project manifest");

    let err = project::load_project_manifest(&path).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("project.entry must not contain '..' segments"));

    rm_rf(&dir);
}

#[test]
fn project_manifest_rejects_non_x07ast_json_entry() {
    let dir = create_temp_dir("x07_project_paths");
    let path = dir.join("x07.json");
    std::fs::write(
        &path,
        r#"
        {
          "schema_version": "x07.project@0.2.0",
          "world": "solve-pure",
          "entry": "main.json",
          "module_roots": ["src"]
        }
        "#,
    )
    .expect("write project manifest");

    let err = project::load_project_manifest(&path).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("project entry must be a *.x07.json file"));

    rm_rf(&dir);
}

#[test]
fn project_manifest_rejects_parent_dir_module_root() {
    let dir = create_temp_dir("x07_project_paths");
    let path = dir.join("x07.json");
    std::fs::write(
        &path,
        r#"
        {
          "schema_version": "x07.project@0.2.0",
          "world": "solve-pure",
          "entry": "main.x07.json",
          "module_roots": ["src/../evil"]
        }
        "#,
    )
    .expect("write project manifest");

    let err = project::load_project_manifest(&path).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("project.module_roots[0] must not contain '..' segments"));

    rm_rf(&dir);
}

#[test]
fn project_manifest_rejects_absolute_dependency_path() {
    let dir = create_temp_dir("x07_project_paths");
    let path = dir.join("x07.json");
    let abs_dep_path = dir.join("abs_dep");
    let manifest = json!({
        "schema_version": "x07.project@0.2.0",
        "world": "solve-pure",
        "entry": "main.x07.json",
        "module_roots": ["src"],
        "dependencies": [
            {"name": "dep", "version": "0.1.0", "path": abs_dep_path}
        ],
    });
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&manifest).expect("encode project manifest"),
    )
    .expect("write project manifest");

    let err = project::load_project_manifest(&path).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("project.dependencies[0].path must be a relative path"));

    rm_rf(&dir);
}

#[test]
fn package_manifest_rejects_parent_dir_module_root() {
    let dir = create_temp_dir("x07_package_paths");
    let pkg = dir.join("x07-package.json");
    std::fs::write(
        &pkg,
        r#"
        {
          "schema_version": "x07.package@0.1.0",
          "name": "dep",
          "version": "0.1.0",
          "module_root": "../src",
          "modules": ["dep.main"]
        }
        "#,
    )
    .expect("write package manifest");

    let err = project::load_package_manifest(&dir).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("package.module_root must not contain '..' segments"));

    rm_rf(&dir);
}

#[test]
fn package_manifest_rejects_module_id_with_slash() {
    let dir = create_temp_dir("x07_package_paths");
    let pkg = dir.join("x07-package.json");
    std::fs::write(
        &pkg,
        r#"
        {
          "schema_version": "x07.package@0.1.0",
          "name": "dep",
          "version": "0.1.0",
          "module_root": "src",
          "modules": ["foo/bar"]
        }
        "#,
    )
    .expect("write package manifest");

    let err = project::load_package_manifest(&dir).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("invalid package.modules[0]"));

    rm_rf(&dir);
}

#[test]
fn project_manifest_accepts_all_solve_worlds() {
    let dir = create_temp_dir("x07_project_worlds");
    let path = dir.join("x07.json");

    for world in [
        "solve-pure",
        "solve-fs",
        "solve-rr",
        "solve-kv",
        "solve-full",
    ] {
        std::fs::write(
            &path,
            format!(
                r#"
        {{
          "schema_version": "x07.project@0.2.0",
          "world": "{world}",
          "entry": "main.x07.json",
          "module_roots": ["src"]
        }}
        "#
            ),
        )
        .expect("write project manifest");

        let manifest = project::load_project_manifest(&path).expect("load project manifest");
        assert_eq!(manifest.world, world);
    }

    rm_rf(&dir);
}

#[test]
fn project_manifest_accepts_link_config_and_trims_fields() {
    let dir = create_temp_dir("x07_project_link");
    let path = dir.join("x07.json");
    std::fs::write(
        &path,
        r#"
        {
          "schema_version": " x07.project@0.2.0 ",
          "world": " solve-pure ",
          "entry": " main.x07.json ",
          "module_roots": [" src "],
          "link": {
            "libs": [" m "],
            "search_paths": [" lib "],
            "frameworks": [" CoreFoundation "],
            "static": true
          }
        }
        "#,
    )
    .expect("write project manifest");

    let manifest = project::load_project_manifest(&path).expect("load project manifest");
    assert_eq!(manifest.schema_version, PROJECT_MANIFEST_SCHEMA_VERSION);
    assert_eq!(manifest.world, "solve-pure");
    assert_eq!(manifest.entry, "main.x07.json");
    assert_eq!(manifest.module_roots, vec!["src".to_string()]);

    assert_eq!(manifest.link.libs, vec!["m".to_string()]);
    assert_eq!(manifest.link.search_paths, vec!["lib".to_string()]);
    assert_eq!(manifest.link.frameworks, vec!["CoreFoundation".to_string()]);
    assert!(manifest.link.static_link);

    rm_rf(&dir);
}

#[test]
fn project_manifest_rejects_link_search_path_parent_dir() {
    let dir = create_temp_dir("x07_project_link");
    let path = dir.join("x07.json");
    std::fs::write(
        &path,
        r#"
        {
          "schema_version": "x07.project@0.2.0",
          "world": "solve-pure",
          "entry": "main.x07.json",
          "module_roots": ["src"],
          "link": {
            "search_paths": ["../lib"]
          }
        }
        "#,
    )
    .expect("write project manifest");

    let err = project::load_project_manifest(&path).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("project.link.search_paths[0] must not contain '..' segments"));

    rm_rf(&dir);
}

#[test]
fn project_manifest_rejects_link_lib_starting_with_dash() {
    let dir = create_temp_dir("x07_project_link");
    let path = dir.join("x07.json");
    std::fs::write(
        &path,
        r#"
        {
          "schema_version": "x07.project@0.2.0",
          "world": "solve-pure",
          "entry": "main.x07.json",
          "module_roots": ["src"],
          "link": {
            "libs": ["-Wl,-rpath,/tmp"]
          }
        }
        "#,
    )
    .expect("write project manifest");

    let err = project::load_project_manifest(&path).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("project.link.libs[0] must not start with '-'"));

    rm_rf(&dir);
}

#[test]
fn package_manifest_trims_fields() {
    let dir = create_temp_dir("x07_package_paths");
    let pkg = dir.join("x07-package.json");
    std::fs::write(
        &pkg,
        r#"
        {
          "schema_version": " x07.package@0.1.0 ",
          "name": " dep ",
          "version": " 0.1.0 ",
          "module_root": " src ",
          "modules": [" dep.main "]
        }
        "#,
    )
    .expect("write package manifest");

    let (m, _, _) = project::load_package_manifest(&dir).expect("load package manifest");
    assert_eq!(m.schema_version, PACKAGE_MANIFEST_SCHEMA_VERSION);
    assert_eq!(m.name, "dep");
    assert_eq!(m.version, "0.1.0");
    assert_eq!(m.module_root, "src");
    assert_eq!(m.modules, vec!["dep.main".to_string()]);

    rm_rf(&dir);
}
