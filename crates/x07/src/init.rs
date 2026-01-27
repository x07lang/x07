use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use x07_contracts::{
    PACKAGE_MANIFEST_SCHEMA_VERSION, PROJECT_LOCKFILE_SCHEMA_VERSION,
    PROJECT_MANIFEST_SCHEMA_VERSION, X07AST_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Args)]
pub struct InitArgs {
    /// Optional scaffold template.
    #[arg(long, value_enum)]
    pub template: Option<InitTemplate>,

    /// Initialize a publishable package repo (modules/ + x07-package.json + tests/).
    #[arg(long)]
    pub package: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum InitTemplate {
    Cli,
    HttpClient,
    WebService,
    FsTool,
    SqliteApp,
    PostgresClient,
    Worker,
}

#[derive(Debug, Clone)]
struct PkgRef {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct CapabilitiesCatalog {
    schema_version: String,
    capabilities: Vec<CapabilityEntry>,
    #[serde(default)]
    aliases: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct CapabilityEntry {
    id: String,
    canonical: CapabilityPackage,
}

#[derive(Debug, Deserialize)]
struct CapabilityPackage {
    name: String,
    version: String,
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

const CAPABILITIES_JSON_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalog/capabilities.json"
));

const TEMPLATE_CLI_APP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/agent-gate/cli-ext-cli/src/app.x07.json"
));
const TEMPLATE_CLI_MAIN: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/agent-gate/cli-ext-cli/src/main.x07.json"
));

const TEMPLATE_HTTP_CLIENT_APP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/agent-gate/http-client-get/src/app.x07.json"
));
const TEMPLATE_HTTP_CLIENT_MAIN: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/agent-gate/http-client-get/src/main.x07.json"
));

fn ensure_trailing_newline(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    out
}

fn load_capabilities_catalog() -> Result<CapabilitiesCatalog> {
    let cat: CapabilitiesCatalog = serde_json::from_slice(CAPABILITIES_JSON_BYTES)
        .context("parse catalog/capabilities.json")?;
    if cat.schema_version != "x07.capabilities@0.1.0" {
        anyhow::bail!(
            "unsupported capabilities schema_version: {:?}",
            cat.schema_version
        );
    }
    Ok(cat)
}

fn resolve_capability_id<'a>(cat: &'a CapabilitiesCatalog, id_or_alias: &'a str) -> &'a str {
    cat.aliases
        .get(id_or_alias)
        .map(|s| s.as_str())
        .unwrap_or(id_or_alias)
}

fn canonical_pkg_for_capability(cat: &CapabilitiesCatalog, id_or_alias: &str) -> Result<PkgRef> {
    let id = resolve_capability_id(cat, id_or_alias);
    let Some(entry) = cat.capabilities.iter().find(|c| c.id == id) else {
        anyhow::bail!("unknown capability id: {id:?}");
    };
    Ok(PkgRef {
        name: entry.canonical.name.clone(),
        version: entry.canonical.version.clone(),
    })
}

fn template_base_capabilities(template: InitTemplate) -> &'static [&'static str] {
    match template {
        InitTemplate::Cli => &["cli", "data.model", "data.json"],
        InitTemplate::HttpClient => &["http-client", "net.curl", "net.sockets", "url.parse"],
        InitTemplate::WebService => &["net.http", "net.sockets", "url.parse"],
        InitTemplate::FsTool => &["fs.io"],
        InitTemplate::SqliteApp => &["db.core", "db.sqlite", "data.model", "fs.io"],
        InitTemplate::PostgresClient => &["db.core", "db.postgres", "data.model"],
        InitTemplate::Worker => &["log.basic"],
    }
}

fn template_default_profile(template: InitTemplate) -> &'static str {
    match template {
        InitTemplate::Cli => "os",
        InitTemplate::HttpClient
        | InitTemplate::WebService
        | InitTemplate::FsTool
        | InitTemplate::SqliteApp
        | InitTemplate::PostgresClient
        | InitTemplate::Worker => "sandbox",
    }
}

fn init_template_policy_template(template: InitTemplate) -> crate::policy::PolicyTemplate {
    match template {
        InitTemplate::Cli => crate::policy::PolicyTemplate::Cli,
        InitTemplate::HttpClient => crate::policy::PolicyTemplate::HttpClient,
        InitTemplate::WebService => crate::policy::PolicyTemplate::WebService,
        InitTemplate::FsTool => crate::policy::PolicyTemplate::FsTool,
        InitTemplate::SqliteApp => crate::policy::PolicyTemplate::SqliteApp,
        InitTemplate::PostgresClient => crate::policy::PolicyTemplate::PostgresClient,
        InitTemplate::Worker => crate::policy::PolicyTemplate::Worker,
    }
}

fn template_program_bytes(template: InitTemplate) -> Result<(Vec<u8>, Vec<u8>)> {
    match template {
        InitTemplate::Cli => Ok((
            ensure_trailing_newline(TEMPLATE_CLI_APP),
            ensure_trailing_newline(TEMPLATE_CLI_MAIN),
        )),
        InitTemplate::HttpClient => Ok((
            ensure_trailing_newline(TEMPLATE_HTTP_CLIENT_APP),
            ensure_trailing_newline(TEMPLATE_HTTP_CLIENT_MAIN),
        )),
        InitTemplate::WebService => Ok((app_module_web_service_bytes()?, main_entry_bytes()?)),
        InitTemplate::FsTool
        | InitTemplate::SqliteApp
        | InitTemplate::PostgresClient
        | InitTemplate::Worker => Ok((app_module_bytes()?, main_entry_bytes()?)),
    }
}

pub fn cmd_init(args: InitArgs) -> Result<std::process::ExitCode> {
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

    if args.package {
        if args.template.is_some() {
            let report = InitReport {
                ok: false,
                command: "init",
                root: root.display().to_string(),
                created: Vec::new(),
                error: Some(InitError {
                    code: "X07INIT_ARGS".to_string(),
                    message: "x07 init --package does not support --template (use x07 init --template ... for app scaffolds, or x07 init --package for a publishable package scaffold)".to_string(),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }

        return cmd_init_package(&root);
    }

    let paths = InitPaths {
        project: root.join("x07.json"),
        lock: root.join("x07.lock.json"),
        gitignore: root.join(".gitignore"),
        src_dir: root.join("src"),
        app: root.join("src").join("app.x07.json"),
        main: root.join("src").join("main.x07.json"),
        tests_dir: root.join("tests"),
        tests_manifest: root.join("tests").join("tests.json"),
        tests_smoke: root.join("tests").join("smoke.x07.json"),
    };

    let (default_profile, policy_template) = match args.template {
        Some(t) => (
            template_default_profile(t),
            init_template_policy_template(t),
        ),
        None => ("os", crate::policy::PolicyTemplate::Cli),
    };
    let should_create_policy = default_profile == "sandbox";
    let policy_path = root.join(crate::policy::default_base_policy_rel_path(policy_template));

    let mut conflicts = Vec::new();
    let required_paths: [&PathBuf; 6] = [
        &paths.project,
        &paths.lock,
        &paths.app,
        &paths.main,
        &paths.tests_manifest,
        &paths.tests_smoke,
    ];
    for p in required_paths {
        if p.exists() {
            conflicts.push(rel(&root, p));
        }
    }
    if should_create_policy && policy_path.exists() {
        conflicts.push(rel(&root, &policy_path));
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

    if paths.tests_dir.exists() && !paths.tests_dir.is_dir() {
        let report = InitReport {
            ok: false,
            command: "init",
            root: root.display().to_string(),
            created: Vec::new(),
            error: Some(InitError {
                code: "X07INIT_TESTS".to_string(),
                message: format!(
                    "tests exists but is not a directory: {}",
                    rel(&root, &paths.tests_dir)
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

    if let Err(err) = std::fs::create_dir_all(&paths.tests_dir) {
        let report = InitReport {
            ok: false,
            command: "init",
            root: root.display().to_string(),
            created: Vec::new(),
            error: Some(InitError {
                code: "X07INIT_IO".to_string(),
                message: format!("create tests dir: {err}"),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let (deps, app_bytes, main_bytes) = match args.template {
        Some(t) => {
            let cat = load_capabilities_catalog()?;
            let mut pkgs_by_name: BTreeMap<String, String> = BTreeMap::new();
            for cap_id in template_base_capabilities(t) {
                let pkg = canonical_pkg_for_capability(&cat, cap_id)?;
                match pkgs_by_name.get(&pkg.name) {
                    None => {
                        pkgs_by_name.insert(pkg.name.clone(), pkg.version.clone());
                    }
                    Some(existing) if existing == &pkg.version => {}
                    Some(existing) => {
                        anyhow::bail!(
                            "capabilities resolve to conflicting versions for {:?}: {:?} vs {:?}",
                            pkg.name,
                            existing,
                            pkg.version
                        );
                    }
                }
            }
            let deps: Vec<PkgRef> = pkgs_by_name
                .into_iter()
                .map(|(name, version)| PkgRef { name, version })
                .collect();
            let (app_bytes, main_bytes) = template_program_bytes(t)?;
            (deps, app_bytes, main_bytes)
        }
        None => (Vec::new(), app_module_bytes()?, main_entry_bytes()?),
    };

    if let Err(err) = write_new_file(&paths.project, &project_json_bytes(args.template, &deps)?) {
        return print_io_error(&root, &created, "x07.json", err);
    }
    created.push(rel(&root, &paths.project));

    if let Err(err) = write_new_file(&paths.lock, &lock_json_bytes()?) {
        return print_io_error(&root, &created, "x07.lock.json", err);
    }
    created.push(rel(&root, &paths.lock));

    if let Err(err) = write_new_file(&paths.app, &app_bytes) {
        return print_io_error(&root, &created, "src/app.x07.json", err);
    }
    created.push(rel(&root, &paths.app));

    if let Err(err) = write_new_file(&paths.main, &main_bytes) {
        return print_io_error(&root, &created, "src/main.x07.json", err);
    }
    created.push(rel(&root, &paths.main));

    if let Err(err) = write_new_file(&paths.tests_manifest, &tests_manifest_bytes()?) {
        return print_io_error(&root, &created, "tests/tests.json", err);
    }
    created.push(rel(&root, &paths.tests_manifest));

    if let Err(err) = write_new_file(&paths.tests_smoke, &tests_smoke_module_bytes()?) {
        return print_io_error(&root, &created, "tests/smoke.x07.json", err);
    }
    created.push(rel(&root, &paths.tests_smoke));

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

    if should_create_policy {
        let policy_bytes = crate::policy::render_base_policy_template_bytes(policy_template, None)?;
        if let Err(err) = write_new_file(&policy_path, &policy_bytes) {
            return print_io_error(
                &root,
                &created,
                crate::policy::default_base_policy_rel_path(policy_template),
                err,
            );
        }
        created.push(rel(&root, &policy_path));
    }

    if args.template.is_some() {
        // Resolve/fetch dependencies and write an up-to-date lockfile.
        let lock_args = crate::pkg::LockArgs {
            project: paths.project.clone(),
            index: None,
            check: false,
            offline: false,
        };
        let (code, err_msg) = crate::pkg::pkg_lock_for_init(&lock_args)?;
        if let Some(msg) = err_msg {
            let report = InitReport {
                ok: false,
                command: "init",
                root: root.display().to_string(),
                created,
                error: Some(InitError {
                    code: "X07INIT_PKG_LOCK".to_string(),
                    message: msg,
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(code);
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

fn cmd_init_package(root: &Path) -> Result<std::process::ExitCode> {
    let pkg_name = sanitize_pkg_name(
        root.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref(),
    );
    let ids = package_ids(&pkg_name);

    let entry_rel = format!("modules/ext/{}/tests.x07.json", ids.tail);
    let module_main_rel = format!("modules/ext/{}.x07.json", ids.tail);
    let module_tests_rel = entry_rel.clone();

    let paths = PackageInitPaths {
        project: root.join("x07.json"),
        package: root.join("x07-package.json"),
        lock: root.join("x07.lock.json"),
        gitignore: root.join(".gitignore"),
        modules_dir: root.join("modules"),
        module_main: root.join(&module_main_rel),
        module_tests: root.join(&module_tests_rel),
        tests_dir: root.join("tests"),
        tests_manifest: root.join("tests").join("tests.json"),
    };

    let mut conflicts = Vec::new();
    for p in [
        &paths.project,
        &paths.package,
        &paths.lock,
        &paths.module_main,
        &paths.module_tests,
        &paths.tests_manifest,
    ] {
        if p.exists() {
            conflicts.push(rel(root, p));
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

    if paths.modules_dir.exists() && !paths.modules_dir.is_dir() {
        let report = InitReport {
            ok: false,
            command: "init",
            root: root.display().to_string(),
            created: Vec::new(),
            error: Some(InitError {
                code: "X07INIT_MODULES".to_string(),
                message: format!(
                    "modules exists but is not a directory: {}",
                    rel(root, &paths.modules_dir)
                ),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    if paths.tests_dir.exists() && !paths.tests_dir.is_dir() {
        let report = InitReport {
            ok: false,
            command: "init",
            root: root.display().to_string(),
            created: Vec::new(),
            error: Some(InitError {
                code: "X07INIT_TESTS".to_string(),
                message: format!(
                    "tests exists but is not a directory: {}",
                    rel(root, &paths.tests_dir)
                ),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let mut created: Vec<String> = Vec::new();

    // x07.json
    if let Err(err) = write_new_file(&paths.project, &package_project_json_bytes(&entry_rel)?) {
        return print_io_error(root, &created, "x07.json", err);
    }
    created.push(rel(root, &paths.project));

    // x07-package.json
    if let Err(err) = write_new_file(&paths.package, &package_json_bytes(&pkg_name, &ids)?) {
        return print_io_error(root, &created, "x07-package.json", err);
    }
    created.push(rel(root, &paths.package));

    // x07.lock.json
    if let Err(err) = write_new_file(&paths.lock, &lock_json_bytes()?) {
        return print_io_error(root, &created, "x07.lock.json", err);
    }
    created.push(rel(root, &paths.lock));

    // modules/ext/<tail>.x07.json
    if let Err(err) = write_new_file(&paths.module_main, &package_module_bytes(&ids)?) {
        return print_io_error(root, &created, &module_main_rel, err);
    }
    created.push(rel(root, &paths.module_main));

    // modules/ext/<tail>/tests.x07.json
    if let Err(err) = write_new_file(&paths.module_tests, &package_tests_module_bytes(&ids)?) {
        return print_io_error(root, &created, &module_tests_rel, err);
    }
    created.push(rel(root, &paths.module_tests));

    // tests/tests.json
    if let Err(err) = write_new_file(
        &paths.tests_manifest,
        &package_tests_manifest_bytes(&ids.test_fn)?,
    ) {
        return print_io_error(root, &created, "tests/tests.json", err);
    }
    created.push(rel(root, &paths.tests_manifest));

    match ensure_gitignore(&paths.gitignore) {
        Ok(wrote) => {
            if wrote {
                created.push(rel(root, &paths.gitignore));
            }
        }
        Err(err) => {
            let report = InitReport {
                ok: false,
                command: "init",
                root: root.display().to_string(),
                created,
                error: Some(InitError {
                    code: "X07INIT_GITIGNORE".to_string(),
                    message: format!("ensure .gitignore: {err}"),
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
    lock: PathBuf,
    gitignore: PathBuf,
    src_dir: PathBuf,
    app: PathBuf,
    main: PathBuf,
    tests_dir: PathBuf,
    tests_manifest: PathBuf,
    tests_smoke: PathBuf,
}

struct PackageInitPaths {
    project: PathBuf,
    package: PathBuf,
    lock: PathBuf,
    gitignore: PathBuf,
    modules_dir: PathBuf,
    module_main: PathBuf,
    module_tests: PathBuf,
    tests_dir: PathBuf,
    tests_manifest: PathBuf,
}

#[derive(Debug, Clone)]
struct PackageIds {
    tail: String,
    module_id: String,
    tests_module_id: String,
    hello_fn: String,
    test_fn: String,
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

fn package_ids(pkg_name: &str) -> PackageIds {
    // Canonical mapping:
    //   pkg name: ext-foo-bar  -> module_id: ext.foo_bar
    //   tests:                 -> ext.foo_bar.tests
    // This mirrors the publishing-by-example tutorial layout under examples/tutorials/.
    let tail_raw = pkg_name.strip_prefix("ext-").unwrap_or(pkg_name);
    let mut tail = tail_raw.replace('-', "_");
    while tail.contains("__") {
        tail = tail.replace("__", "_");
    }
    tail = tail.trim_matches('_').to_string();
    if tail.is_empty() {
        tail = "pkg".to_string();
    }
    if !tail
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_lowercase())
    {
        tail = format!("pkg_{tail}");
    }

    let module_id = format!("ext.{tail}");
    let tests_module_id = format!("{module_id}.tests");
    let hello_fn = format!("{module_id}.hello_v1");
    let test_fn = format!("{tests_module_id}.test_hello_v1");
    PackageIds {
        tail,
        module_id,
        tests_module_id,
        hello_fn,
        test_fn,
    }
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

fn project_json_bytes(template: Option<InitTemplate>, deps: &[PkgRef]) -> Result<Vec<u8>> {
    let (default_profile, policy_template) = match template {
        None => ("os", crate::policy::PolicyTemplate::Cli),
        Some(t) => (
            template_default_profile(t),
            init_template_policy_template(t),
        ),
    };

    let deps_val = Value::Array(
        deps.iter()
            .map(|p| {
                Value::Object(
                    [
                        ("name".to_string(), Value::String(p.name.clone())),
                        ("version".to_string(), Value::String(p.version.clone())),
                        (
                            "path".to_string(),
                            Value::String(format!(".x07/deps/{}/{}", p.name, p.version)),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                )
            })
            .collect(),
    );

    let v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(PROJECT_MANIFEST_SCHEMA_VERSION.to_string()),
            ),
            ("world".to_string(), Value::String("run-os".to_string())),
            (
                "entry".to_string(),
                Value::String("src/main.x07.json".to_string()),
            ),
            (
                "module_roots".to_string(),
                Value::Array(vec![Value::String("src".to_string())]),
            ),
            ("dependencies".to_string(), deps_val),
            (
                "lockfile".to_string(),
                Value::String("x07.lock.json".to_string()),
            ),
            (
                "default_profile".to_string(),
                Value::String(default_profile.to_string()),
            ),
            (
                "profiles".to_string(),
                Value::Object(
                    [
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
                                            crate::policy::default_base_policy_rel_path(
                                                policy_template,
                                            )
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

fn package_project_json_bytes(entry_rel: &str) -> Result<Vec<u8>> {
    // Package repos are designed primarily for `x07 test` + `x07 pkg publish`.
    // We mirror the minimal shape used in examples/tutorials/package_publish_ext_hello.
    let v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(PROJECT_MANIFEST_SCHEMA_VERSION.to_string()),
            ),
            ("world".to_string(), Value::String("run-os".to_string())),
            ("entry".to_string(), Value::String(entry_rel.to_string())),
            (
                "module_roots".to_string(),
                Value::Array(vec![Value::String("modules".to_string())]),
            ),
            (
                "lockfile".to_string(),
                Value::String("x07.lock.json".to_string()),
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

fn package_json_bytes(name: &str, ids: &PackageIds) -> Result<Vec<u8>> {
    let version = "0.1.0";
    let docs = format!(
        "Starter package generated by `x07 init --package`.\n\nModules:\n- {}\n- {}\n\nUsage:\n- Add: x07 pkg add {}@{} --sync\n- Import: {}\n- Call: {}\n\nDev:\n- Test: x07 test --manifest tests/tests.json\n- Pack: x07 pkg pack --package . --out dist/{}-{}.x07pkg\n",
        ids.module_id,
        ids.tests_module_id,
        name,
        version,
        ids.module_id,
        ids.hello_fn,
        name,
        version,
    );

    let v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(PACKAGE_MANIFEST_SCHEMA_VERSION.to_string()),
            ),
            ("name".to_string(), Value::String(name.to_string())),
            (
                "description".to_string(),
                Value::String(format!(
                    "Starter package generated by `x07 init --package`: {}(name) -> bytes.",
                    ids.hello_fn
                )),
            ),
            ("docs".to_string(), Value::String(docs)),
            ("version".to_string(), Value::String(version.to_string())),
            (
                "module_root".to_string(),
                Value::String("modules".to_string()),
            ),
            (
                "modules".to_string(),
                Value::Array(vec![
                    Value::String(ids.module_id.clone()),
                    Value::String(ids.tests_module_id.clone()),
                ]),
            ),
            (
                "meta".to_string(),
                Value::Object(
                    [
                        (
                            "determinism_tier".to_string(),
                            Value::String("pure".to_string()),
                        ),
                        (
                            "worlds_allowed".to_string(),
                            Value::Array(
                                ["run-os", "run-os-sandboxed"]
                                    .into_iter()
                                    .map(|s| Value::String(s.to_string()))
                                    .collect(),
                            ),
                        ),
                        (
                            "import_mode".to_string(),
                            Value::String("handwritten".to_string()),
                        ),
                        (
                            "visibility".to_string(),
                            Value::String("experimental".to_string()),
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

fn package_module_bytes(ids: &PackageIds) -> Result<Vec<u8>> {
    let mut v = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": ids.module_id.clone(),
        "imports": [],
        "decls": [
            {"kind": "export", "names": [ids.hello_fn.clone()]},
            {
                "kind": "defn",
                "name": ids.hello_fn.clone(),
                "params": [{"name": "name", "ty": "bytes_view"}],
                "result": "bytes",
                "body": [
                    "begin",
                    ["let", "prefix", ["bytes.concat", ["bytes.lit", "hello,"], ["bytes1", 32]]],
                    ["let", "tmp", ["bytes.concat", "prefix", ["view.to_bytes", "name"]]],
                    ["bytes.concat", "tmp", ["bytes1", 10]]
                ]
            }
        ]
    });
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_string(&v)?.into_bytes();
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn package_tests_module_bytes(ids: &PackageIds) -> Result<Vec<u8>> {
    let mut v = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": ids.tests_module_id.clone(),
        "imports": [ids.module_id.clone(), "std.test"],
        "decls": [
            {"kind": "export", "names": [ids.test_fn.clone()]},
            {
                "kind": "defn",
                "name": ids.test_fn.clone(),
                "params": [],
                "result": "result_i32",
                "body": [
                    "begin",
                    ["let", "name", ["bytes.lit", "x07"]],
                    ["let", "got", [ids.hello_fn.clone(), ["bytes.view", "name"]]],
                    ["let", "expected_prefix", ["bytes.concat", ["bytes.lit", "hello,"], ["bytes1", 32]]],
                    ["let", "expected_tmp", ["bytes.concat", "expected_prefix", "name"]],
                    ["let", "expected", ["bytes.concat", "expected_tmp", ["bytes1", 10]]],
                    ["try", ["std.test.assert_bytes_eq", "got", "expected", ["std.test.code_assert_bytes_eq"]]],
                    ["std.test.pass"]
                ]
            }
        ]
    });
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_string(&v)?.into_bytes();
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

fn app_module_web_service_bytes() -> Result<Vec<u8>> {
    let body = serde_json::json!([
        "begin",
        ["let", "caps", ["std.net.codec.caps_default_v1"]],
        ["let", "caps_v", ["bytes.view", "caps"]],
        [
            "let",
            "addr",
            ["std.net.codec.addr_ipv4_v1", 127, 0, 0, 1, 30031]
        ],
        [
            "let",
            "ldoc",
            ["std.net.tcp.listen_v1", ["bytes.view", "addr"], "caps_v"]
        ],
        ["let", "ldoc_v", ["bytes.view", "ldoc"]],
        [
            "if",
            ["std.net.err.is_err_doc_v1", "ldoc_v"],
            ["return", ["bytes.lit", "FAIL_listen"]],
            0
        ],
        [
            "let",
            "listener",
            ["std.net.tcp.listen_listener_handle_v1", "ldoc_v"]
        ],
        [
            "if",
            ["<=", "listener", 0],
            ["return", ["bytes.lit", "FAIL_listen_handle"]],
            0
        ],
        [
            "let",
            "bound_addr",
            ["std.net.tcp.listen_bound_addr_v1", "ldoc_v"]
        ],
        [
            "if",
            ["<=", ["bytes.len", "bound_addr"], 0],
            ["return", ["bytes.lit", "FAIL_bound_addr"]],
            0
        ],
        [
            "let",
            "conn_doc",
            [
                "std.net.tcp.connect_v1",
                ["bytes.view", "bound_addr"],
                "caps_v"
            ]
        ],
        ["let", "conn_v", ["bytes.view", "conn_doc"]],
        [
            "if",
            ["std.net.err.is_err_doc_v1", "conn_v"],
            ["return", ["bytes.lit", "FAIL_connect"]],
            0
        ],
        [
            "let",
            "client",
            ["std.net.tcp.connect_stream_handle_v1", "conn_v"]
        ],
        [
            "if",
            ["<=", "client", 0],
            ["return", ["bytes.lit", "FAIL_client_handle"]],
            0
        ],
        [
            "let",
            "acc_doc",
            ["std.net.tcp.accept_v1", "listener", "caps_v"]
        ],
        ["let", "acc_v", ["bytes.view", "acc_doc"]],
        [
            "if",
            ["std.net.err.is_err_doc_v1", "acc_v"],
            ["return", ["bytes.lit", "FAIL_accept"]],
            0
        ],
        [
            "let",
            "server",
            ["std.net.tcp.accept_stream_handle_v1", "acc_v"]
        ],
        [
            "if",
            ["<=", "server", 0],
            ["return", ["bytes.lit", "FAIL_server_handle"]],
            0
        ],
        // bytes.lit cannot contain whitespace or escapes; build the request bytes explicitly.
        ["let", "t_get", ["bytes.lit", "GET"]],
        ["let", "t_path", ["bytes.lit", "/hello"]],
        ["let", "t_http", ["bytes.lit", "HTTP/1.1"]],
        ["let", "t_host", ["bytes.lit", "Host"]],
        ["let", "t_localhost", ["bytes.lit", "localhost"]],
        ["let", "req_buf", ["std.vec.with_capacity", 64]],
        [
            "set",
            "req_buf",
            ["std.vec.extend_bytes", "req_buf", ["bytes.view", "t_get"]]
        ],
        ["set", "req_buf", ["std.vec.push", "req_buf", 32]],
        [
            "set",
            "req_buf",
            ["std.vec.extend_bytes", "req_buf", ["bytes.view", "t_path"]]
        ],
        ["set", "req_buf", ["std.vec.push", "req_buf", 32]],
        [
            "set",
            "req_buf",
            ["std.vec.extend_bytes", "req_buf", ["bytes.view", "t_http"]]
        ],
        ["set", "req_buf", ["std.vec.push", "req_buf", 13]],
        ["set", "req_buf", ["std.vec.push", "req_buf", 10]],
        [
            "set",
            "req_buf",
            ["std.vec.extend_bytes", "req_buf", ["bytes.view", "t_host"]]
        ],
        ["set", "req_buf", ["std.vec.push", "req_buf", 58]],
        ["set", "req_buf", ["std.vec.push", "req_buf", 32]],
        [
            "set",
            "req_buf",
            [
                "std.vec.extend_bytes",
                "req_buf",
                ["bytes.view", "t_localhost"]
            ]
        ],
        ["set", "req_buf", ["std.vec.push", "req_buf", 13]],
        ["set", "req_buf", ["std.vec.push", "req_buf", 10]],
        ["set", "req_buf", ["std.vec.push", "req_buf", 13]],
        ["set", "req_buf", ["std.vec.push", "req_buf", 10]],
        ["let", "req", ["std.vec.as_bytes", "req_buf"]],
        [
            "let",
            "wdoc",
            [
                "std.net.io.write_all_v1",
                "client",
                ["bytes.view", "req"],
                "caps_v"
            ]
        ],
        ["let", "wdoc_v", ["bytes.view", "wdoc"]],
        [
            "if",
            ["std.net.err.is_err_doc_v1", "wdoc_v"],
            ["return", ["bytes.lit", "FAIL_write_req"]],
            0
        ],
        [
            "let",
            "req_doc",
            ["std.net.http.server.read_req_v1", "server", "caps_v"]
        ],
        ["let", "req_v", ["bytes.view", "req_doc"]],
        [
            "if",
            ["std.net.err.is_err_doc_v1", "req_v"],
            ["return", ["bytes.lit", "FAIL_read_req"]],
            0
        ],
        [
            "let",
            "target",
            ["std.net.http.server.req_target_v1", "req_v"]
        ],
        ["let", "expect", ["bytes.lit", "/hello"]],
        [
            "if",
            [
                "=",
                [
                    "bytes.eq",
                    ["bytes.view", "target"],
                    ["bytes.view", "expect"]
                ],
                0
            ],
            ["return", ["bytes.lit", "FAIL_target"]],
            0
        ],
        ["let", "empty", ["bytes.alloc", 0]],
        ["let", "body", ["bytes.lit", "hello"]],
        [
            "let",
            "resp_doc",
            [
                "std.net.http.server.write_response_v1",
                "server",
                200,
                ["bytes.view", "empty"],
                ["bytes.view", "body"],
                "caps_v"
            ]
        ],
        ["let", "resp_v", ["bytes.view", "resp_doc"]],
        [
            "if",
            ["std.net.err.is_err_doc_v1", "resp_v"],
            ["return", ["bytes.lit", "FAIL_write_resp"]],
            0
        ],
        [
            "let",
            "rdoc",
            ["std.net.tcp.stream_read_v1", "client", 4096, "caps_v"]
        ],
        ["let", "rdoc_v", ["bytes.view", "rdoc"]],
        [
            "if",
            ["std.net.err.is_err_doc_v1", "rdoc_v"],
            ["return", ["bytes.lit", "FAIL_read_resp"]],
            0
        ],
        [
            "let",
            "resp_bytes",
            ["std.net.tcp.stream_read_payload_v1", "rdoc_v"]
        ],
        ["std.net.tcp.stream_close_v1", "client"],
        ["std.net.tcp.stream_drop_v1", "client"],
        ["std.net.tcp.stream_close_v1", "server"],
        ["std.net.tcp.stream_drop_v1", "server"],
        ["std.net.tcp.listener_close_v1", "listener"],
        ["std.net.tcp.listener_drop_v1", "listener"],
        "resp_bytes"
    ]);

    let mut v = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "app",
        "imports": [
          "std.net.codec",
          "std.net.err",
          "std.net.http.server",
          "std.net.io",
          "std.net.tcp",
          "std.vec"
        ],
        "decls": [
          { "kind": "export", "names": ["app.solve"] },
          {
            "kind": "defn",
            "name": "app.solve",
            "params": [ { "name": "b", "ty": "bytes_view" } ],
            "result": "bytes",
            "body": body
          }
        ]
    });

    // Keep output canonical for stable diffs and to match x07 fmt behavior.
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

fn package_tests_manifest_bytes(test_entry: &str) -> Result<Vec<u8>> {
    let v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String("x07.tests_manifest@0.1.0".to_string()),
            ),
            (
                "tests".to_string(),
                Value::Array(vec![Value::Object(
                    [
                        ("id".to_string(), Value::String("hello_v1".to_string())),
                        ("world".to_string(), Value::String("run-os".to_string())),
                        ("entry".to_string(), Value::String(test_entry.to_string())),
                        ("expect".to_string(), Value::String("pass".to_string())),
                    ]
                    .into_iter()
                    .collect(),
                )]),
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

fn tests_manifest_bytes() -> Result<Vec<u8>> {
    let v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String("x07.tests_manifest@0.1.0".to_string()),
            ),
            (
                "tests".to_string(),
                Value::Array(vec![Value::Object(
                    [
                        ("id".to_string(), Value::String("smoke/pass".to_string())),
                        ("world".to_string(), Value::String("run-os".to_string())),
                        ("entry".to_string(), Value::String("smoke.pass".to_string())),
                        ("expect".to_string(), Value::String("pass".to_string())),
                    ]
                    .into_iter()
                    .collect(),
                )]),
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

fn tests_smoke_module_bytes() -> Result<Vec<u8>> {
    let mut v = Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(X07AST_SCHEMA_VERSION.to_string()),
            ),
            ("kind".to_string(), Value::String("module".to_string())),
            ("module_id".to_string(), Value::String("smoke".to_string())),
            (
                "imports".to_string(),
                Value::Array(vec![Value::String("std.test".to_string())]),
            ),
            (
                "decls".to_string(),
                Value::Array(vec![
                    Value::Object(
                        [
                            ("kind".to_string(), Value::String("export".to_string())),
                            (
                                "names".to_string(),
                                Value::Array(vec![Value::String("smoke.pass".to_string())]),
                            ),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                    Value::Object(
                        [
                            ("kind".to_string(), Value::String("defn".to_string())),
                            ("name".to_string(), Value::String("smoke.pass".to_string())),
                            ("params".to_string(), Value::Array(Vec::new())),
                            (
                                "result".to_string(),
                                Value::String("result_i32".to_string()),
                            ),
                            (
                                "body".to_string(),
                                Value::Array(vec![Value::String("std.test.pass".to_string())]),
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

fn ensure_gitignore(path: &Path) -> Result<bool> {
    // Keep policy files committable by default, but ignore generated artifacts.
    const REQUIRED: [&str; 8] = [
        ".x07/deps/",
        ".x07/tmp/",
        ".x07/policies/_generated/",
        "target/",
        "dist/",
        "artifacts/",
        ".DS_Store",
        "*.log",
    ];

    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s.replace("\r\n", "\n"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };

    if existing.is_empty() {
        let out = "\
# X07 deps + generated artifacts
.x07/deps/
.x07/tmp/
.x07/policies/_generated/

# Build outputs
target/
dist/
artifacts/

# Editor noise
.DS_Store
*.log
";

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir: {}", parent.display()))?;
        }
        std::fs::write(path, out).with_context(|| format!("write {}", path.display()))?;
        return Ok(true);
    }

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
