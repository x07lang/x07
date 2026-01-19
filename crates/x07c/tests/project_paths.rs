use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use x07_contracts::{PACKAGE_MANIFEST_SCHEMA_VERSION, PROJECT_MANIFEST_SCHEMA_VERSION};
use x07_worlds::WorldId;
use x07c::project;
use x07c::{compile, world_config};

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

#[test]
fn project_module_roots_dedup_prevents_duplicate_module_hits() {
    let dir = create_temp_dir("x07_project_roots_dedup");

    let dep_mod_dir = dir.join(".x07/deps/dep/0.1.0/modules/dep");
    std::fs::create_dir_all(&dep_mod_dir).expect("create dep module dir");

    let dep_module = json!({
        "schema_version": "x07.x07ast@0.1.0",
        "kind": "module",
        "module_id": "dep.main",
        "imports": [],
        "decls": [
            {"kind":"export","names":["dep.main.f"]},
            {"kind":"defn","name":"dep.main.f","params":[],"result":"bytes","body":["bytes.lit","ok"]}
        ]
    });
    std::fs::write(
        dep_mod_dir.join("main.x07.json"),
        serde_json::to_string(&dep_module).expect("encode dep module"),
    )
    .expect("write dep module");

    let entry = json!({
        "schema_version": "x07.x07ast@0.1.0",
        "kind": "entry",
        "module_id": "main",
        "imports": ["dep.main"],
        "decls": [],
        "solve": ["dep.main.f"],
    });
    let entry_bytes = serde_json::to_vec(&entry).expect("encode entry module");

    let manifest = project::ProjectManifest {
        schema_version: PROJECT_MANIFEST_SCHEMA_VERSION.to_string(),
        world: "solve-pure".to_string(),
        entry: "src/main.x07.json".to_string(),
        module_roots: vec![
            "./src".to_string(),
            "./.x07/deps/dep/0.1.0/modules".to_string(),
        ],
        link: project::LinkConfig::default(),
        dependencies: Vec::new(),
        lockfile: Some("x07.lock.json".to_string()),
    };

    let lock = project::Lockfile {
        schema_version: x07_contracts::PROJECT_LOCKFILE_SCHEMA_VERSION.to_string(),
        dependencies: vec![project::LockedDependency {
            name: "dep".to_string(),
            version: "0.1.0".to_string(),
            path: ".x07/deps/dep/0.1.0".to_string(),
            package_manifest_sha256: "0".repeat(64),
            module_root: "modules".to_string(),
            modules_sha256: std::collections::BTreeMap::new(),
        }],
    };

    let project_path = dir.join("x07.json");
    let roots = project::collect_module_roots(&project_path, &manifest, &lock)
        .expect("collect module roots");
    assert_eq!(roots.len(), 2, "expected module roots to be deduplicated");

    let options = world_config::compile_options_for_world(WorldId::SolvePure, roots);
    compile::compile_program_to_c(&entry_bytes, &options)
        .expect("compile should not fail with duplicate roots");

    rm_rf(&dir);
}
