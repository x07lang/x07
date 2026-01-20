use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use x07_contracts::{
    PACKAGE_MANIFEST_SCHEMA_VERSION, PROJECT_LOCKFILE_SCHEMA_VERSION,
    PROJECT_MANIFEST_SCHEMA_VERSION, X07AST_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Copy)]
pub struct InitOptions {
    pub package: bool,
}

#[derive(Debug, Serialize)]
struct InitError {
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct InitReport {
    ok: bool,
    command: &'static str,
    root: String,
    created: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<InitError>,
}

pub fn cmd_init(options: InitOptions) -> Result<std::process::ExitCode> {
    let root = match std::env::current_dir() {
        Ok(p) => p,
        Err(err) => {
            let report = InitReport {
                ok: false,
                command: "init",
                root: ".".to_string(),
                created: Vec::new(),
                error: Some(InitError {
                    code: "X07INIT_CWD".to_string(),
                    message: format!("get current dir: {err}"),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };

    let paths = InitPaths {
        project: root.join("x07.json"),
        package: root.join("x07-package.json"),
        lock: root.join("x07.lock.json"),
        gitignore: root.join(".gitignore"),
        src_dir: root.join("src"),
        app: root.join("src").join("app.x07.json"),
        main: root.join("src").join("main.x07.json"),
    };

    let mut conflicts = Vec::new();
    let mut required_paths: Vec<&PathBuf> =
        vec![&paths.project, &paths.lock, &paths.app, &paths.main];
    if options.package {
        required_paths.push(&paths.package);
    }
    for p in required_paths {
        if p.exists() {
            conflicts.push(rel(&root, p));
        }
    }
    if !conflicts.is_empty() {
        let report = InitReport {
            ok: false,
            command: "init",
            root: root.display().to_string(),
            created: Vec::new(),
            error: Some(InitError {
                code: "X07INIT_EXISTS".to_string(),
                message: format!(
                    "refusing to overwrite existing paths: {}",
                    conflicts.join(", ")
                ),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    if paths.src_dir.exists() && !paths.src_dir.is_dir() {
        let report = InitReport {
            ok: false,
            command: "init",
            root: root.display().to_string(),
            created: Vec::new(),
            error: Some(InitError {
                code: "X07INIT_SRC".to_string(),
                message: format!(
                    "src exists but is not a directory: {}",
                    rel(&root, &paths.src_dir)
                ),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let mut created: Vec<String> = Vec::new();

    if let Err(err) = std::fs::create_dir_all(&paths.src_dir) {
        let report = InitReport {
            ok: false,
            command: "init",
            root: root.display().to_string(),
            created: Vec::new(),
            error: Some(InitError {
                code: "X07INIT_IO".to_string(),
                message: format!("create src dir: {err}"),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    if let Err(err) = write_new_file(&paths.project, &project_json_bytes()?) {
        return print_io_error(&root, &created, "x07.json", err);
    }
    created.push(rel(&root, &paths.project));

    if options.package {
        let pkg_name = sanitize_pkg_name(
            root.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        );
        if let Err(err) = write_new_file(&paths.package, &package_json_bytes(&pkg_name)?) {
            return print_io_error(&root, &created, "x07-package.json", err);
        }
        created.push(rel(&root, &paths.package));
    }

    if let Err(err) = write_new_file(&paths.lock, &lock_json_bytes()?) {
        return print_io_error(&root, &created, "x07.lock.json", err);
    }
    created.push(rel(&root, &paths.lock));

    if let Err(err) = write_new_file(&paths.app, &app_module_bytes()?) {
        return print_io_error(&root, &created, "src/app.x07.json", err);
    }
    created.push(rel(&root, &paths.app));

    if let Err(err) = write_new_file(&paths.main, &main_entry_bytes()?) {
        return print_io_error(&root, &created, "src/main.x07.json", err);
    }
    created.push(rel(&root, &paths.main));

    match ensure_gitignore(&paths.gitignore) {
        Ok(true) => created.push(rel(&root, &paths.gitignore)),
        Ok(false) => {}
        Err(err) => {
            let report = InitReport {
                ok: false,
                command: "init",
                root: root.display().to_string(),
                created: created.clone(),
                error: Some(InitError {
                    code: "X07INIT_IO".to_string(),
                    message: format!("update .gitignore: {err:#}"),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    }

    let report = InitReport {
        ok: true,
        command: "init",
        root: root.display().to_string(),
        created,
        error: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::SUCCESS)
}

struct InitPaths {
    project: PathBuf,
    package: PathBuf,
    lock: PathBuf,
    gitignore: PathBuf,
    src_dir: PathBuf,
    app: PathBuf,
    main: PathBuf,
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn print_io_error(
    root: &Path,
    created: &[String],
    path_hint: &str,
    err: std::io::Error,
) -> Result<std::process::ExitCode> {
    let report = InitReport {
        ok: false,
        command: "init",
        root: root.display().to_string(),
        created: created.to_vec(),
        error: Some(InitError {
            code: "X07INIT_IO".to_string(),
            message: format!("write {path_hint}: {err}"),
        }),
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::from(20))
}

fn sanitize_pkg_name(raw: &str) -> String {
    let raw = raw.trim();
    let raw = if raw.is_empty() { "x07-project" } else { raw };

    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }

    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out = out.trim_matches(&['-', '_'][..]).to_string();

    if out.is_empty() {
        out = "x07-project".to_string();
    }
    if !out
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_lowercase())
    {
        out = format!("x07-{out}");
    }
    out
}

fn write_new_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    use std::io::Write as _;
    f.write_all(bytes)?;
    Ok(())
}

fn project_json_bytes() -> Result<Vec<u8>> {
    let v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(PROJECT_MANIFEST_SCHEMA_VERSION.to_string()),
            ),
            ("world".to_string(), Value::String("solve-pure".to_string())),
            (
                "entry".to_string(),
                Value::String("src/main.x07.json".to_string()),
            ),
            (
                "module_roots".to_string(),
                Value::Array(vec![Value::String("src".to_string())]),
            ),
            ("dependencies".to_string(), Value::Array(Vec::new())),
            (
                "lockfile".to_string(),
                Value::String("x07.lock.json".to_string()),
            ),
            (
                "default_profile".to_string(),
                Value::String("test".to_string()),
            ),
            (
                "profiles".to_string(),
                Value::Object(
                    [
                        (
                            "test".to_string(),
                            Value::Object(
                                [("world".to_string(), Value::String("solve-pure".to_string()))]
                                    .into_iter()
                                    .collect(),
                            ),
                        ),
                        (
                            "os".to_string(),
                            Value::Object(
                                [
                                    ("world".to_string(), Value::String("run-os".to_string())),
                                    ("auto_ffi".to_string(), Value::Bool(true)),
                                ]
                                .into_iter()
                                .collect(),
                            ),
                        ),
                        (
                            "sandbox".to_string(),
                            Value::Object(
                                [
                                    (
                                        "world".to_string(),
                                        Value::String("run-os-sandboxed".to_string()),
                                    ),
                                    (
                                        "policy".to_string(),
                                        Value::String(
                                            ".x07/policies/base/cli.sandbox.base.policy.json"
                                                .to_string(),
                                        ),
                                    ),
                                    ("auto_ffi".to_string(), Value::Bool(true)),
                                ]
                                .into_iter()
                                .collect(),
                            ),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );

    let mut out = serde_json::to_vec_pretty(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn package_json_bytes(name: &str) -> Result<Vec<u8>> {
    let v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(PACKAGE_MANIFEST_SCHEMA_VERSION.to_string()),
            ),
            ("name".to_string(), Value::String(name.to_string())),
            (
                "description".to_string(),
                Value::String("A new X07 package.".to_string()),
            ),
            ("version".to_string(), Value::String("0.1.0".to_string())),
            ("module_root".to_string(), Value::String("src".to_string())),
            (
                "modules".to_string(),
                Value::Array(vec![Value::String("app".to_string())]),
            ),
        ]
        .into_iter()
        .collect(),
    );

    let mut out = serde_json::to_vec_pretty(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn lock_json_bytes() -> Result<Vec<u8>> {
    let v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(PROJECT_LOCKFILE_SCHEMA_VERSION.to_string()),
            ),
            ("dependencies".to_string(), Value::Array(Vec::new())),
        ]
        .into_iter()
        .collect(),
    );
    let mut out = serde_json::to_vec_pretty(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn app_module_bytes() -> Result<Vec<u8>> {
    let mut v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(X07AST_SCHEMA_VERSION.to_string()),
            ),
            ("kind".to_string(), Value::String("module".to_string())),
            ("module_id".to_string(), Value::String("app".to_string())),
            ("imports".to_string(), Value::Array(Vec::new())),
            (
                "decls".to_string(),
                Value::Array(vec![
                    Value::Object(
                        [
                            ("kind".to_string(), Value::String("export".to_string())),
                            (
                                "names".to_string(),
                                Value::Array(vec![Value::String("app.solve".to_string())]),
                            ),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                    Value::Object(
                        [
                            ("kind".to_string(), Value::String("defn".to_string())),
                            ("name".to_string(), Value::String("app.solve".to_string())),
                            (
                                "params".to_string(),
                                Value::Array(vec![Value::Object(
                                    [
                                        ("name".to_string(), Value::String("b".to_string())),
                                        ("ty".to_string(), Value::String("bytes_view".to_string())),
                                    ]
                                    .into_iter()
                                    .collect(),
                                )]),
                            ),
                            ("result".to_string(), Value::String("bytes".to_string())),
                            (
                                "body".to_string(),
                                Value::Array(vec![
                                    Value::String("view.to_bytes".to_string()),
                                    Value::String("b".to_string()),
                                ]),
                            ),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                ]),
            ),
        ]
        .into_iter()
        .collect(),
    );
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_string(&v)?.into_bytes();
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn main_entry_bytes() -> Result<Vec<u8>> {
    let mut v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(X07AST_SCHEMA_VERSION.to_string()),
            ),
            ("kind".to_string(), Value::String("entry".to_string())),
            ("module_id".to_string(), Value::String("main".to_string())),
            (
                "imports".to_string(),
                Value::Array(vec![Value::String("app".to_string())]),
            ),
            ("decls".to_string(), Value::Array(Vec::new())),
            (
                "solve".to_string(),
                Value::Array(vec![
                    Value::String("app.solve".to_string()),
                    Value::String("input".to_string()),
                ]),
            ),
        ]
        .into_iter()
        .collect(),
    );
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_string(&v)?.into_bytes();
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn ensure_gitignore(path: &Path) -> Result<bool> {
    const REQUIRED: [&str; 2] = [".x07/", "target/"];

    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s.replace("\r\n", "\n"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };

    let missing: Vec<&str> = REQUIRED
        .into_iter()
        .filter(|pat| !existing.lines().any(|line| line.trim() == *pat))
        .collect();

    if missing.is_empty() {
        return Ok(false);
    }

    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for pat in missing {
        out.push_str(pat);
        out.push('\n');
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }
    std::fs::write(path, out).with_context(|| format!("write {}", path.display()))?;
    Ok(true)
}
