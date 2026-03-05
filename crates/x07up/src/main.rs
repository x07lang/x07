use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use xz2::read::XzDecoder;

const DEFAULT_CHANNELS_URL: &str = "https://x07lang.org/install/channels/stable.json";
const X07_TOOLCHAIN_TOML: &str = "x07-toolchain.toml";
const X07_AGENT_DIR: &str = ".agent";
const X07UP_STATE_DIR: &str = ".x07up";
const SELF_UPDATE_GUARD_ENV: &str = "X07UP_SELF_UPDATE_GUARD";

const SHOW_SCHEMA_VERSION: &str = "x07up.show@0.1.0";
const INSTALL_SCHEMA_VERSION: &str = "x07up.install.report@0.1.0";
const DOCTOR_SCHEMA_VERSION: &str = "x07up.doctor.report@0.1.0";

#[derive(Debug, Parser)]
#[command(name = "x07up")]
#[command(about = "X07 toolchain manager.", long_about = None)]
#[command(version)]
struct Cli {
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    #[arg(long, global = true, default_value = DEFAULT_CHANNELS_URL)]
    channels_url: String,

    #[arg(long, global = true)]
    json: bool,

    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Install(InstallArgs),
    Uninstall { toolchain: String },
    Default { toolchain: String },
    Override(OverrideArgs),
    Update(UpdateArgs),
    Show,
    List(ListArgs),
    Which { tool: String },
    Component(ComponentArgs),
    Skills(SkillsArgs),
    Docs(DocsArgs),
    Agent(AgentArgs),
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
struct InstallArgs {
    toolchain: String,

    #[arg(long, value_enum, default_value_t = InstallProfile::Full)]
    profile: InstallProfile,

    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InstallProfile {
    Full,
    Minimal,
}

#[derive(Debug, Args)]
struct UpdateArgs {
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Args)]
struct ListArgs {
    #[arg(long)]
    installed: bool,
}

#[derive(Debug, Args)]
struct ComponentArgs {
    #[command(subcommand)]
    cmd: ComponentCmd,
}

#[derive(Debug, Subcommand)]
enum ComponentCmd {
    Add(ComponentSelectionArgs),
    Update(ComponentSelectionArgs),
    List,
}

#[derive(Debug, Args)]
struct ComponentSelectionArgs {
    #[arg(value_enum)]
    components: Vec<ComponentName>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum, PartialEq, Eq)]
enum ComponentName {
    Wasm,
    #[value(name = "device-host")]
    DeviceHost,
}

#[derive(Debug, Args)]
struct OverrideArgs {
    #[command(subcommand)]
    cmd: OverrideCmd,
}

#[derive(Debug, Subcommand)]
enum OverrideCmd {
    Set { toolchain: String },
    Unset,
}

#[derive(Debug, Args)]
struct SkillsArgs {
    #[command(subcommand)]
    cmd: SkillsCmd,
}

#[derive(Debug, Subcommand)]
enum SkillsCmd {
    Install(SkillsInstallArgs),
    Status,
}

#[derive(Debug, Args)]
struct SkillsInstallArgs {
    #[arg(long)]
    user: bool,

    #[arg(long)]
    project: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct DocsArgs {
    #[command(subcommand)]
    cmd: DocsCmd,
}

#[derive(Debug, Subcommand)]
enum DocsCmd {
    Path,
}

#[derive(Debug, Args)]
struct AgentArgs {
    #[command(subcommand)]
    cmd: AgentCmd,
}

#[derive(Debug, Subcommand)]
enum AgentCmd {
    Init(AgentInitArgs),
}

#[derive(Debug, Args)]
struct AgentInitArgs {
    #[arg(long)]
    project: Option<PathBuf>,

    #[arg(long)]
    pin: Option<String>,

    #[arg(long, value_enum, default_value_t = AgentSkillsMode::None)]
    with_skills: AgentSkillsMode,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentSkillsMode {
    None,
    User,
    Project,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    #[arg(long)]
    network: bool,
}

#[derive(Debug)]
struct Reporter {
    json: bool,
    quiet: bool,
}

impl Reporter {
    fn progress(&self, msg: &str) {
        if self.json || self.quiet {
            return;
        }
        eprintln!("{msg}");
    }
}

impl ComponentName {
    fn cli_name(self) -> &'static str {
        match self {
            Self::Wasm => "wasm",
            Self::DeviceHost => "device-host",
        }
    }

    fn binary_name(self) -> &'static str {
        match self {
            Self::Wasm => "x07-wasm",
            Self::DeviceHost => "x07-device-host-desktop",
        }
    }

    fn manifest_component(self) -> &'static str {
        match self {
            Self::Wasm => "x07_wasm",
            Self::DeviceHost => "x07_device_host",
        }
    }
}

fn main() -> std::process::ExitCode {
    match try_main() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err:#}");
            std::process::ExitCode::from(2)
        }
    }
}

fn try_main() -> Result<std::process::ExitCode> {
    let invoked = invoked_tool_name(std::env::args_os().next().unwrap_or_default());
    if invoked != "x07up" {
        return proxy_dispatch(&invoked);
    }

    let cli = Cli::parse();
    let root = effective_root(cli.root)?;
    let reporter = Reporter {
        json: cli.json,
        quiet: cli.quiet,
    };

    match cli.cmd {
        Command::Install(args) => cmd_install(&root, &cli.channels_url, args, &reporter),
        Command::Uninstall { toolchain } => cmd_uninstall(&root, &toolchain, &reporter),
        Command::Default { toolchain } => cmd_default(&root, &toolchain, &reporter),
        Command::Override(args) => cmd_override(args, &reporter),
        Command::Update(args) => cmd_update(&root, &cli.channels_url, args, &reporter),
        Command::Show => cmd_show(&root, &cli.channels_url, &reporter),
        Command::List(args) => cmd_list(&root, args),
        Command::Which { tool } => cmd_which(&root, &tool),
        Command::Component(args) => cmd_component(&root, &cli.channels_url, args, &reporter),
        Command::Skills(args) => cmd_skills(&root, args, &reporter),
        Command::Docs(args) => cmd_docs(&root, args, &reporter),
        Command::Agent(args) => cmd_agent(&root, &cli.channels_url, args, &reporter),
        Command::Doctor(args) => cmd_doctor(&root, args, &reporter),
    }
}

fn invoked_tool_name(argv0: OsString) -> String {
    let p = PathBuf::from(argv0);
    let file = p.file_stem().unwrap_or_default().to_string_lossy();
    file.to_string()
}

fn effective_root(root: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = root {
        return Ok(root);
    }
    if let Some(v) = std::env::var_os("X07UP_ROOT") {
        if !v.is_empty() {
            return Ok(PathBuf::from(v));
        }
    }
    if let Some(root) = root_from_installed_exe() {
        return Ok(root);
    }
    default_root()
}

fn default_root() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".x07"))
}

fn home_dir() -> Result<PathBuf> {
    if let Some(v) = std::env::var_os("HOME") {
        if !v.is_empty() {
            return Ok(PathBuf::from(v));
        }
    }
    if let Some(v) = std::env::var_os("USERPROFILE") {
        if !v.is_empty() {
            return Ok(PathBuf::from(v));
        }
    }
    if let (Some(drive), Some(path)) = (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
    {
        let mut s = OsString::new();
        s.push(drive);
        s.push(path);
        if !s.is_empty() {
            return Ok(PathBuf::from(s));
        }
    }
    bail!("could not determine home directory (HOME/USERPROFILE/HOMEDRIVE+HOMEPATH)");
}

fn root_from_installed_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe.parent()?;
    if bin.file_name()? != "bin" {
        return None;
    }
    Some(bin.parent()?.to_path_buf())
}

fn config_path(root: &Path) -> PathBuf {
    root.join("config.json")
}

fn toolchains_dir(root: &Path) -> PathBuf {
    root.join("toolchains")
}

fn bin_dir(root: &Path) -> PathBuf {
    root.join("bin")
}

fn cache_dir(root: &Path) -> PathBuf {
    root.join("cache")
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_toolchain: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    channels: BTreeMap<String, String>,
}

impl Config {
    fn load(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Ok(Self {
                schema_version: "x07up.config@0.1.0".to_string(),
                default_toolchain: None,
                channels: BTreeMap::new(),
            });
        }
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let mut cfg: Self =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        if cfg.schema_version != "x07up.config@0.1.0" {
            bail!(
                "unsupported config schema_version: {} (expected x07up.config@0.1.0)",
                cfg.schema_version
            );
        }
        Ok({
            cfg.channels.retain(|k, v| !(k.is_empty() || v.is_empty()));
            cfg
        })
    }

    fn save(&self, path: &Path) -> Result<()> {
        let rendered = serde_json::to_vec_pretty(self)?;
        let mut bytes = rendered;
        bytes.push(b'\n');

        path.parent()
            .context("config has no parent dir")?
            .mkdir_all()
            .with_context(|| format!("mkdir {}", path.parent().unwrap().display()))?;

        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &bytes).with_context(|| format!("write {}", tmp.display()))?;
        rename_overwrite_file(&tmp, path)?;
        Ok(())
    }
}

trait MkdirAll {
    fn mkdir_all(&self) -> Result<()>;
}

impl MkdirAll for Path {
    fn mkdir_all(&self) -> Result<()> {
        std::fs::create_dir_all(self).with_context(|| format!("create_dir_all {}", self.display()))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct BundleComponentRef {
    #[allow(dead_code)]
    version: String,
    tag: String,
    release_manifest_url: String,
    release_manifest_sha256: String,
}

#[derive(Debug, Deserialize)]
struct BundlePackages {
    #[allow(dead_code)]
    std_web_ui: String,
}

#[derive(Debug, Deserialize)]
struct BundleManifest {
    schema_version: String,
    channel: String,
    #[allow(dead_code)]
    published_at_utc: String,
    min_x07up_version: String,
    x07_core: BundleComponentRef,
    x07_wasm: BundleComponentRef,
    #[allow(dead_code)]
    x07_web_ui_host: BundleComponentRef,
    x07_device_host: BundleComponentRef,
    #[allow(dead_code)]
    packages: BundlePackages,
}

#[derive(Debug, Deserialize)]
struct ComponentReleaseManifest {
    schema_version: String,
    component: String,
    version: String,
    tag: String,
    #[allow(dead_code)]
    repo: String,
    #[allow(dead_code)]
    published_at_utc: String,
    assets: Vec<ReleaseAsset>,
    compatibility: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ReleaseAsset {
    name: String,
    kind: String,
    url: String,
    sha256: String,
    #[allow(dead_code)]
    bytes_len: u64,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InstalledComponentState {
    schema_version: String,
    component: String,
    version: String,
    tag: String,
    binary: String,
    release_manifest_url: String,
}

fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let resp = ureq::get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    let mut reader = resp.into_body().into_reader();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

fn verify_bytes_sha256(bytes: &[u8], expected_sha256: &str, url: &str) -> Result<()> {
    let actual = hex_lower(&Sha256::digest(bytes));
    if !eq_hex_sha256(
        &actual,
        expected_sha256
            .strip_prefix("sha256:")
            .unwrap_or(expected_sha256),
    ) {
        bail!("sha256 mismatch for {url}: expected {expected_sha256}, got sha256:{actual}");
    }
    Ok(())
}

fn fetch_bundle_manifest(url: &str) -> Result<BundleManifest> {
    let bytes = fetch_bytes(url)?;
    let doc: BundleManifest =
        serde_json::from_slice(&bytes).context("parse release bundle manifest")?;
    if doc.schema_version != "x07.release.bundle@0.1.0" {
        bail!(
            "unsupported bundle manifest schema_version: {} (expected x07.release.bundle@0.1.0)",
            doc.schema_version
        );
    }
    Ok(doc)
}

fn fetch_component_release_manifest(
    reference: &BundleComponentRef,
) -> Result<ComponentReleaseManifest> {
    let bytes = fetch_bytes(&reference.release_manifest_url)?;
    verify_bytes_sha256(
        &bytes,
        &reference.release_manifest_sha256,
        &reference.release_manifest_url,
    )?;
    let doc: ComponentReleaseManifest =
        serde_json::from_slice(&bytes).context("parse component release manifest")?;
    if doc.schema_version != "x07.component.release@0.1.0" {
        bail!(
            "unsupported component release schema_version: {} (expected x07.component.release@0.1.0)",
            doc.schema_version
        );
    }
    Ok(doc)
}

fn detect_target_key() -> Result<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let key = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => "unknown",
    };
    if key == "unknown" {
        bail!("unsupported host for x07up: os={os} arch={arch}");
    }
    Ok(key.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
}

fn parse_semver_prefix(s: &str) -> Option<SemVer> {
    let s = s.trim();
    let s = s.strip_prefix('v').unwrap_or(s);
    let end = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let prefix = &s[..end];
    let mut parts = prefix.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts.next()?.parse::<u64>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(SemVer {
        major,
        minor,
        patch,
    })
}

fn current_x07up_version() -> Option<SemVer> {
    parse_semver_prefix(env!("CARGO_PKG_VERSION"))
}

fn looks_like_tag(s: &str) -> bool {
    s.starts_with('v') && s.len() >= 2
}

fn is_channel_name(s: &str) -> bool {
    matches!(s, "stable" | "beta" | "nightly")
}

fn release_bundle_url_for_tag(tag: &str) -> Result<String> {
    if !looks_like_tag(tag) {
        bail!("release bundle tag must look like vX.Y.Z: {tag}");
    }
    let version = tag.trim_start_matches('v');
    Ok(format!(
        "https://github.com/x07lang/x07/releases/download/{tag}/x07-{version}-bundle.json"
    ))
}

fn resolve_channel_bundle_url(base_url: &str, channel: &str) -> Result<String> {
    if !is_channel_name(channel) {
        bail!("unsupported channel: {channel}");
    }
    if let Some(prefix) = base_url.strip_suffix("/channels.json") {
        return Ok(format!("{prefix}/channels/{channel}.json"));
    }
    for known in ["stable", "beta", "nightly"] {
        let suffix = format!("/channels/{known}.json");
        if let Some(prefix) = base_url.strip_suffix(&suffix) {
            return Ok(format!("{prefix}/channels/{channel}.json"));
        }
    }
    if channel == "stable" {
        return Ok(base_url.to_string());
    }
    bail!(
        "cannot derive {channel} bundle URL from {}; use --channels-url with /channels/<channel>.json",
        base_url
    );
}

fn bundle_url_for_spec(channels_url: &str, spec: &str) -> Result<String> {
    if looks_like_tag(spec) {
        release_bundle_url_for_tag(spec)
    } else {
        resolve_channel_bundle_url(channels_url, spec)
    }
}

fn validate_toolchain_id(id: &str) -> Result<()> {
    if id.is_empty() {
        bail!("toolchain id must be non-empty");
    }
    if id.contains('/') || id.contains('\\') {
        bail!("invalid toolchain id: contains path separators");
    }
    if id.contains("..") {
        bail!("invalid toolchain id: contains '..'");
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct InstallReport {
    schema_version: &'static str,
    ok: bool,
    root: String,
    toolchain: String,
    profile: String,
    target: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorReport>,
}

#[derive(Debug, Serialize)]
struct ErrorReport {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
}

fn cmd_install(
    root: &Path,
    channels_url: &str,
    args: InstallArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    let _ = args.yes;
    let target = detect_target_key()?;

    let mut cfg = Config::load(&config_path(root))?;
    let spec = args.toolchain.trim().to_string();
    if spec.is_empty() {
        bail!("missing toolchain argument");
    }

    let bundle_url = bundle_url_for_spec(channels_url, &spec)?;
    let bundle = fetch_bundle_manifest(&bundle_url)?;
    let channel = if is_channel_name(&spec) {
        Some(spec.clone())
    } else {
        None
    };
    if let Some(expected_channel) = &channel {
        if bundle.channel != *expected_channel {
            bail!(
                "bundle channel mismatch: expected {} got {} ({})",
                expected_channel,
                bundle.channel,
                bundle_url
            );
        }
    }
    let toolchain_tag = bundle.x07_core.tag.clone();
    validate_toolchain_id(&toolchain_tag)?;
    maybe_self_update_and_reexec(root, &bundle.x07_core, &bundle.min_x07up_version, reporter)?;
    let release = fetch_component_release_manifest(&bundle.x07_core)?;
    if release.component != "x07_core" {
        bail!(
            "bundle x07_core reference does not resolve to x07_core manifest: {}",
            bundle.x07_core.release_manifest_url
        );
    }

    let out_dir = toolchains_dir(root);
    out_dir.mkdir_all()?;

    let final_dir = out_dir.join(&toolchain_tag);
    let mut warnings: Vec<String> = Vec::new();

    if final_dir.is_dir() {
        reporter.progress("toolchain already installed");
    } else {
        let asset = find_archive_asset(&release, &target)?;
        let archive_path = download_release_asset(root, asset, reporter, "toolchain")?;
        let tmp_dir = out_dir.join(format!(".tmp_{toolchain_tag}_{}", std::process::id()));
        if tmp_dir.exists() {
            std::fs::remove_dir_all(&tmp_dir).ok();
        }
        tmp_dir.mkdir_all()?;
        reporter.progress("extract toolchain");
        extract_archive(&archive_path, archive_format(&asset.name), &tmp_dir)?;

        reporter.progress("finalize toolchain install");
        std::fs::rename(&tmp_dir, &final_dir)
            .with_context(|| format!("rename {} -> {}", tmp_dir.display(), final_dir.display()))?;
    }

    if let Some(channel) = &channel {
        cfg.channels.insert(channel.clone(), toolchain_tag.clone());
    }
    if cfg.default_toolchain.as_deref().unwrap_or("").is_empty() {
        cfg.default_toolchain = Some(spec.clone());
    }
    cfg.save(&config_path(root))?;

    if let Some(channel) = &channel {
        ensure_toolchain_channel_link(root, channel, &toolchain_tag)?;
    }

    reporter.progress("ensure x07 proxies");
    ensure_proxies(root)?;

    match args.profile {
        InstallProfile::Full => {
            let docs_root = final_dir.join(X07_AGENT_DIR).join("docs");
            if !docs_root.is_dir() {
                return report_install_json(
                    reporter,
                    InstallReport {
                        schema_version: INSTALL_SCHEMA_VERSION,
                        ok: false,
                        root: root.display().to_string(),
                        toolchain: toolchain_tag,
                        profile: "full".to_string(),
                        target,
                        warnings,
                        error: Some(ErrorReport {
                            code: "COMPONENT_MISSING".to_string(),
                            message: "docs component missing from installed toolchain".to_string(),
                            hint: Some(format!("expected directory: {}", docs_root.display())),
                        }),
                    },
                );
            }

            let skills_root = final_dir.join(X07_AGENT_DIR).join("skills");
            if !skills_root.is_dir() {
                return report_install_json(
                    reporter,
                    InstallReport {
                        schema_version: INSTALL_SCHEMA_VERSION,
                        ok: false,
                        root: root.display().to_string(),
                        toolchain: toolchain_tag,
                        profile: "full".to_string(),
                        target,
                        warnings,
                        error: Some(ErrorReport {
                            code: "COMPONENT_MISSING".to_string(),
                            message: "skills component missing from installed toolchain"
                                .to_string(),
                            hint: Some(format!("expected directory: {}", skills_root.display())),
                        }),
                    },
                );
            }
        }
        InstallProfile::Minimal => {
            let docs_root = final_dir.join(X07_AGENT_DIR).join("docs");
            if !docs_root.is_dir() {
                warnings.push("docs missing (profile=minimal)".to_string());
            }
            let skills_root = final_dir.join(X07_AGENT_DIR).join("skills");
            if !skills_root.is_dir() {
                warnings.push("skills missing (profile=minimal)".to_string());
            }
        }
    }

    if reporter.json {
        report_install_json(
            reporter,
            InstallReport {
                schema_version: INSTALL_SCHEMA_VERSION,
                ok: true,
                root: root.display().to_string(),
                toolchain: toolchain_tag,
                profile: match args.profile {
                    InstallProfile::Full => "full",
                    InstallProfile::Minimal => "minimal",
                }
                .to_string(),
                target,
                warnings,
                error: None,
            },
        )?;
    }

    reporter.progress("");
    reporter
        .progress("next: create a project (includes agent kit: AGENT.md + .agent/{docs,skills})");
    reporter.progress("  mkdir myapp && cd myapp && x07 init");
    reporter.progress("  x07 run");
    reporter.progress("  x07 test --manifest tests/tests.json");
    reporter.progress("");
    reporter.progress("next: open offline docs / check skills");
    reporter.progress("  x07up docs path");
    reporter.progress("  x07up skills status --json");
    reporter.progress("");
    reporter.progress("next: create a publishable package repo");
    reporter.progress("  mkdir mypkg && cd mypkg && x07 init --package");

    Ok(std::process::ExitCode::SUCCESS)
}

fn ensure_toolchain_channel_link(root: &Path, channel: &str, toolchain_tag: &str) -> Result<()> {
    let target = toolchains_dir(root).join(toolchain_tag);
    if !target.is_dir() {
        bail!(
            "toolchain dir missing while updating channel link: {}",
            target.display()
        );
    }

    let links_dir = toolchains_dir(root).join("_channels");
    links_dir.mkdir_all()?;
    let link_path = links_dir.join(channel);

    if link_path.exists() {
        remove_link_dir(&link_path)?;
    }

    create_dir_link(&target, &link_path)
        .with_context(|| format!("link channel {channel} -> {}", target.display()))?;
    Ok(())
}

fn remove_link_dir(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => return Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {}
    }

    std::fs::remove_dir(path).with_context(|| format!("remove {}", path.display()))?;
    Ok(())
}

fn create_dir_link(target: &Path, link: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(target, link)
            .with_context(|| format!("symlink {} -> {}", link.display(), target.display()))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        let _ = target;
        let _ = link;
        bail!("create_dir_link: unsupported platform");
    }
}

fn component_state_dir(toolchain_dir: &Path) -> PathBuf {
    toolchain_dir.join(X07UP_STATE_DIR).join("components")
}

fn component_state_path(toolchain_dir: &Path, component: ComponentName) -> PathBuf {
    component_state_dir(toolchain_dir).join(format!("{}.json", component.cli_name()))
}

fn load_component_state(
    toolchain_dir: &Path,
    component: ComponentName,
) -> Result<Option<InstalledComponentState>> {
    let path = component_state_path(toolchain_dir, component);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let state: InstalledComponentState =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    Ok(Some(state))
}

fn save_component_state(
    toolchain_dir: &Path,
    component: ComponentName,
    release: &ComponentReleaseManifest,
    reference: &BundleComponentRef,
) -> Result<()> {
    let path = component_state_path(toolchain_dir, component);
    path.parent()
        .context("component state path missing parent")?
        .mkdir_all()?;
    let state = InstalledComponentState {
        schema_version: "x07up.component.state@0.1.0".to_string(),
        component: component.cli_name().to_string(),
        version: release.version.clone(),
        tag: release.tag.clone(),
        binary: component.binary_name().to_string(),
        release_manifest_url: reference.release_manifest_url.clone(),
    };
    let mut bytes = serde_json::to_vec_pretty(&state)?;
    bytes.push(b'\n');
    std::fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn archive_format(name: &str) -> &'static str {
    if name.ends_with(".tar.gz") {
        "tar.gz"
    } else if name.ends_with(".tar.xz") {
        "tar.xz"
    } else if name.ends_with(".zip") {
        "zip"
    } else {
        "unknown"
    }
}

fn find_release_asset<'a>(
    release: &'a ComponentReleaseManifest,
    target: &str,
    kind: &str,
) -> Result<&'a ReleaseAsset> {
    let mut available = Vec::new();
    for asset in &release.assets {
        if asset.kind != kind {
            continue;
        }
        if let Some(asset_target) = &asset.target {
            available.push(asset_target.clone());
            if asset_target == target {
                return Ok(asset);
            }
        }
    }
    bail!(
        "no {kind} asset for target={target}; available: {}",
        available.join(", ")
    )
}

fn find_archive_asset<'a>(
    release: &'a ComponentReleaseManifest,
    target: &str,
) -> Result<&'a ReleaseAsset> {
    find_release_asset(release, target, "archive")
}

fn find_installer_archive_asset<'a>(
    release: &'a ComponentReleaseManifest,
    target: &str,
) -> Result<&'a ReleaseAsset> {
    find_release_asset(release, target, "installer_archive")
}

fn download_release_asset(
    root: &Path,
    asset: &ReleaseAsset,
    reporter: &Reporter,
    label: &str,
) -> Result<PathBuf> {
    reporter.progress(&format!("download {label}: {}", asset.url));
    let dl_dir = cache_dir(root).join("downloads");
    dl_dir.mkdir_all()?;
    let filename = url_filename(&asset.url).unwrap_or_else(|| asset.name.clone());
    let archive_path = dl_dir.join(filename);
    download_verify(
        &asset.url,
        &archive_path,
        asset
            .sha256
            .strip_prefix("sha256:")
            .unwrap_or(&asset.sha256),
    )?;
    Ok(archive_path)
}

fn component_reference<'a>(
    bundle: &'a BundleManifest,
    component: ComponentName,
) -> &'a BundleComponentRef {
    match component {
        ComponentName::Wasm => &bundle.x07_wasm,
        ComponentName::DeviceHost => &bundle.x07_device_host,
    }
}

fn ensure_release_compatibility(
    release: &ComponentReleaseManifest,
    toolchain_tag: &str,
    component: ComponentName,
) -> Result<()> {
    if release.component != component.manifest_component() {
        bail!(
            "release manifest component mismatch: expected {} got {}",
            component.manifest_component(),
            release.component
        );
    }
    let expected_core = toolchain_tag.trim_start_matches('v');
    let actual_core = release
        .compatibility
        .get("x07_core")
        .map(String::as_str)
        .unwrap_or("");
    if actual_core != expected_core {
        bail!(
            "component {} is not compatible with toolchain {} (manifest requires x07_core={})",
            component.cli_name(),
            toolchain_tag,
            actual_core
        );
    }
    Ok(())
}

fn extract_single_archive_root(extract_dir: &Path) -> Result<PathBuf> {
    let entries = std::fs::read_dir(extract_dir)
        .with_context(|| format!("read_dir {}", extract_dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if entries.len() == 1 && entries[0].file_type()?.is_dir() {
        return Ok(entries[0].path());
    }
    Ok(extract_dir.to_path_buf())
}

fn copy_file_preserve_mode(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        parent.mkdir_all()?;
    }
    std::fs::copy(src, dst)
        .with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(src)?.permissions().mode();
        std::fs::set_permissions(dst, std::fs::Permissions::from_mode(mode))
            .with_context(|| format!("chmod {}", dst.display()))?;
    }
    Ok(())
}

fn merge_dir_overwrite(src: &Path, dst: &Path) -> Result<()> {
    dst.mkdir_all()?;
    for entry in std::fs::read_dir(src).with_context(|| format!("read_dir {}", src.display()))? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            merge_dir_overwrite(&src_path, &dst_path)?;
        } else if ty.is_file() {
            copy_file_preserve_mode(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn install_component_from_bundle(
    root: &Path,
    toolchain_tag: &str,
    bundle: &BundleManifest,
    component: ComponentName,
    reporter: &Reporter,
) -> Result<bool> {
    let toolchain_dir = toolchains_dir(root).join(toolchain_tag);
    if !toolchain_dir.is_dir() {
        bail!(
            "toolchain dir missing during component install: {}",
            toolchain_dir.display()
        );
    }

    let reference = component_reference(bundle, component);
    let release = fetch_component_release_manifest(reference)?;
    ensure_release_compatibility(&release, toolchain_tag, component)?;

    if let Some(state) = load_component_state(&toolchain_dir, component)? {
        let binary_path = toolchain_dir.join("bin").join(component.binary_name());
        if state.version == release.version && binary_path.is_file() {
            reporter.progress(&format!(
                "component {} already installed ({})",
                component.cli_name(),
                state.version
            ));
            return Ok(false);
        }
    }

    let target = detect_target_key()?;
    let asset = find_archive_asset(&release, &target)?;
    let archive_path = download_release_asset(root, asset, reporter, component.cli_name())?;
    let extract_root = cache_dir(root).join("components");
    extract_root.mkdir_all()?;
    let tmp_dir = extract_root.join(format!(
        ".tmp_{}_{}_{}",
        component.cli_name(),
        release.tag,
        std::process::id()
    ));
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir).ok();
    }
    tmp_dir.mkdir_all()?;
    reporter.progress(&format!("extract component {}", component.cli_name()));
    extract_archive(&archive_path, archive_format(&asset.name), &tmp_dir)?;
    let stage_root = extract_single_archive_root(&tmp_dir)?;
    merge_dir_overwrite(&stage_root, &toolchain_dir)?;
    std::fs::remove_dir_all(&tmp_dir).ok();
    save_component_state(&toolchain_dir, component, &release, reference)?;
    Ok(true)
}

fn report_install_json(
    reporter: &Reporter,
    report: InstallReport,
) -> Result<std::process::ExitCode> {
    if !reporter.json {
        if report.ok {
            println!("ok: installed {}", report.toolchain);
        } else if let Some(err) = &report.error {
            println!("error: {}: {}", err.code, err.message);
            if let Some(hint) = &err.hint {
                println!("hint: {hint}");
            }
        }
        return Ok(if report.ok {
            std::process::ExitCode::SUCCESS
        } else {
            std::process::ExitCode::from(1)
        });
    }
    write_json_stdout(&report)?;
    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_uninstall(
    root: &Path,
    toolchain: &str,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    validate_toolchain_id(toolchain)?;
    let dir = toolchains_dir(root).join(toolchain);
    if !dir.is_dir() {
        bail!("toolchain not installed: {toolchain}");
    }
    reporter.progress(&format!("remove {}", dir.display()));
    std::fs::remove_dir_all(&dir).with_context(|| format!("remove_dir_all {}", dir.display()))?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_default(
    root: &Path,
    toolchain: &str,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    let spec = toolchain.trim();
    if spec.is_empty() {
        bail!("default toolchain must be non-empty");
    }
    let mut cfg = Config::load(&config_path(root))?;
    cfg.default_toolchain = Some(spec.to_string());
    cfg.save(&config_path(root))?;
    reporter.progress(&format!("default toolchain set to {spec}"));
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_override(args: OverrideArgs, reporter: &Reporter) -> Result<std::process::ExitCode> {
    match args.cmd {
        OverrideCmd::Set { toolchain } => {
            let spec = toolchain.trim();
            if spec.is_empty() {
                bail!("override toolchain must be non-empty");
            }
            let toml = format!(
                "[toolchain]\nchannel = \"{}\"\ncomponents = [\"docs\", \"skills\"]\n",
                escape_toml_string(spec)
            );
            std::fs::write(X07_TOOLCHAIN_TOML, toml.as_bytes())
                .context("write x07-toolchain.toml")?;
            reporter.progress("ok: wrote x07-toolchain.toml");
            Ok(std::process::ExitCode::SUCCESS)
        }
        OverrideCmd::Unset => {
            let path = Path::new(X07_TOOLCHAIN_TOML);
            if path.exists() {
                std::fs::remove_file(path).context("remove x07-toolchain.toml")?;
                reporter.progress("ok: removed x07-toolchain.toml");
            }
            Ok(std::process::ExitCode::SUCCESS)
        }
    }
}

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Debug, Serialize)]
struct ShowReport {
    schema_version: &'static str,
    root: String,
    toolchains: ToolchainsReport,
    active: ActiveReport,
    channels: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ToolchainsReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<String>,
    installed: Vec<InstalledToolchain>,
}

#[derive(Debug, Serialize)]
struct InstalledToolchain {
    toolchain: String,
    dir: String,
}

#[derive(Debug, Serialize)]
struct ActiveReport {
    spec: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved: Option<String>,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    override_file: Option<String>,
}

fn cmd_show(
    root: &Path,
    _channels_url: &str,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    let cfg = Config::load(&config_path(root))?;
    let installed = list_installed_toolchains(root)?;
    let sel = select_active_toolchain(root, &cfg)?;
    let mut warnings = Vec::new();
    if sel.resolved.is_none() && sel.spec != "stable" {
        warnings.push("active toolchain spec does not resolve to an installed tag".to_string());
    }

    if reporter.json {
        write_json_stdout(&ShowReport {
            schema_version: SHOW_SCHEMA_VERSION,
            root: root.display().to_string(),
            toolchains: ToolchainsReport {
                default: cfg.default_toolchain.clone(),
                installed,
            },
            active: ActiveReport {
                spec: sel.spec,
                resolved: sel.resolved,
                source: sel.source,
                override_file: sel.override_file.map(|p| p.display().to_string()),
            },
            channels: cfg.channels,
            warnings,
        })?;
    } else {
        println!("root: {}", root.display());
        println!("active: {} ({})", sel.spec, sel.source);
    }
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_list(root: &Path, args: ListArgs) -> Result<std::process::ExitCode> {
    if !args.installed {
        bail!("only --installed is supported");
    }
    for t in list_installed_toolchains(root)? {
        println!("{}", t.toolchain);
    }
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_which(root: &Path, tool: &str) -> Result<std::process::ExitCode> {
    let cfg = Config::load(&config_path(root))?;
    let sel = select_active_toolchain(root, &cfg)?;
    let tag = sel
        .resolved
        .ok_or_else(|| anyhow!("no active toolchain resolved; run: x07up install stable"))?;
    let path = tool_path(root, &tag, tool)?;
    println!("{}", path.display());
    Ok(std::process::ExitCode::SUCCESS)
}

fn active_toolchain_bundle(root: &Path, cfg: &Config) -> Result<(String, BundleManifest)> {
    let sel = select_active_toolchain(root, cfg)?;
    let tag = sel
        .resolved
        .ok_or_else(|| anyhow!("no active toolchain resolved; run: x07up install stable"))?;
    let bundle = fetch_bundle_manifest(&release_bundle_url_for_tag(&tag)?)?;
    Ok((tag, bundle))
}

fn cmd_component(
    root: &Path,
    _channels_url: &str,
    args: ComponentArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    match args.cmd {
        ComponentCmd::Add(args) => cmd_component_add(root, args, reporter),
        ComponentCmd::Update(args) => cmd_component_update(root, args, reporter),
        ComponentCmd::List => cmd_component_list(root, reporter),
    }
}

fn selected_or_default_components(
    toolchain_dir: &Path,
    selected: &[ComponentName],
) -> Result<Vec<ComponentName>> {
    if !selected.is_empty() {
        return Ok(selected.to_vec());
    }
    let mut out = Vec::new();
    for component in [ComponentName::Wasm, ComponentName::DeviceHost] {
        let binary = toolchain_dir.join("bin").join(component.binary_name());
        if binary.is_file() || load_component_state(toolchain_dir, component)?.is_some() {
            out.push(component);
        }
    }
    Ok(out)
}

fn cmd_component_add(
    root: &Path,
    args: ComponentSelectionArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    if args.components.is_empty() {
        bail!("component add requires at least one component");
    }
    let cfg = Config::load(&config_path(root))?;
    let (toolchain_tag, bundle) = active_toolchain_bundle(root, &cfg)?;
    let mut changed = false;
    for component in args.components {
        changed |=
            install_component_from_bundle(root, &toolchain_tag, &bundle, component, reporter)?;
    }
    if changed {
        ensure_proxies(root)?;
    }
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_component_update(
    root: &Path,
    args: ComponentSelectionArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    let cfg = Config::load(&config_path(root))?;
    let (toolchain_tag, bundle) = active_toolchain_bundle(root, &cfg)?;
    let toolchain_dir = toolchains_dir(root).join(&toolchain_tag);
    let components = selected_or_default_components(&toolchain_dir, &args.components)?;
    if components.is_empty() {
        reporter.progress("no installed components to update");
        return Ok(std::process::ExitCode::SUCCESS);
    }
    let mut changed = false;
    for component in components {
        changed |=
            install_component_from_bundle(root, &toolchain_tag, &bundle, component, reporter)?;
    }
    if changed {
        ensure_proxies(root)?;
    }
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_component_list(root: &Path, reporter: &Reporter) -> Result<std::process::ExitCode> {
    #[derive(Serialize)]
    struct ComponentEntry {
        name: String,
        binary: String,
        installed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tag: Option<String>,
    }

    #[derive(Serialize)]
    struct ComponentListReport {
        schema_version: &'static str,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        toolchain: Option<String>,
        components: Vec<ComponentEntry>,
    }

    let cfg = Config::load(&config_path(root))?;
    let sel = select_active_toolchain(root, &cfg)?;
    let toolchain = sel.resolved.clone();
    let mut components = Vec::new();
    for component in [ComponentName::Wasm, ComponentName::DeviceHost] {
        let (installed, version, tag) = if let Some(toolchain_tag) = &toolchain {
            let toolchain_dir = toolchains_dir(root).join(toolchain_tag);
            let state = load_component_state(&toolchain_dir, component)?;
            let binary = toolchain_dir.join("bin").join(component.binary_name());
            (
                binary.is_file() || state.is_some(),
                state.as_ref().map(|s| s.version.clone()),
                state.as_ref().map(|s| s.tag.clone()),
            )
        } else {
            (false, None, None)
        };
        components.push(ComponentEntry {
            name: component.cli_name().to_string(),
            binary: component.binary_name().to_string(),
            installed,
            version,
            tag,
        });
    }

    if reporter.json {
        write_json_stdout(&ComponentListReport {
            schema_version: "x07up.component.list@0.1.0",
            ok: true,
            toolchain,
            components,
        })?;
    } else {
        for component in components {
            if component.installed {
                println!(
                    "{}\tinstalled\t{}",
                    component.name,
                    component.version.unwrap_or_else(|| "unknown".to_string())
                );
            } else {
                println!("{}\tnot-installed", component.name);
            }
        }
    }
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_update(
    root: &Path,
    channels_url: &str,
    args: UpdateArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    let cfg = Config::load(&config_path(root))?;
    let sel = select_active_toolchain(root, &cfg)?;
    let update_spec = sel.spec.clone();
    let bundle_url = bundle_url_for_spec(channels_url, &update_spec)?;
    let bundle = fetch_bundle_manifest(&bundle_url)?;
    let latest = bundle.x07_core.tag.clone();
    let current = cfg
        .channels
        .get(&update_spec)
        .cloned()
        .or_else(|| sel.resolved.clone());

    #[derive(Serialize)]
    struct UpdateReport {
        schema_version: &'static str,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        current: Option<String>,
        latest: String,
        update_available: bool,
    }

    if args.check {
        if reporter.json {
            write_json_stdout(&UpdateReport {
                schema_version: "x07up.update.check@0.1.0",
                ok: true,
                current: current.clone(),
                latest: latest.clone(),
                update_available: current.as_deref() != Some(latest.as_str()),
            })?;
            return Ok(std::process::ExitCode::SUCCESS);
        }
        if current.as_deref() == Some(latest.as_str()) {
            println!("ok: {update_spec} is up to date ({latest})");
        } else {
            println!("update available: {latest}");
        }
        return Ok(std::process::ExitCode::SUCCESS);
    }

    reporter.progress(&format!("install latest {update_spec}"));
    cmd_install(
        root,
        channels_url,
        InstallArgs {
            toolchain: update_spec.clone(),
            profile: InstallProfile::Full,
            yes: true,
        },
        reporter,
    )?;
    cmd_default(root, &update_spec, reporter)?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn maybe_self_update_and_reexec(
    root: &Path,
    core_ref: &BundleComponentRef,
    min_x07up_version: &str,
    reporter: &Reporter,
) -> Result<()> {
    let Some(current) = current_x07up_version() else {
        return Ok(());
    };
    let Some(desired) = parse_semver_prefix(min_x07up_version) else {
        return Ok(());
    };
    if current >= desired {
        return Ok(());
    }

    if let Some(prev) = std::env::var_os(SELF_UPDATE_GUARD_ENV) {
        let prev = prev.to_string_lossy();
        if prev == core_ref.tag {
            bail!(
                "x07up self-update loop detected (current={} desired={}); hint: rerun install.sh to refresh x07up",
                env!("CARGO_PKG_VERSION"),
                min_x07up_version
            );
        }
    }

    reporter.progress(&format!(
        "self-update x07up: {} (core {})",
        min_x07up_version, core_ref.tag
    ));
    let installed = install_x07up_release(root, core_ref, reporter)?;
    exec_updated_x07up(&installed, &core_ref.tag)?;
    Ok(())
}

fn install_x07up_release(
    root: &Path,
    core_ref: &BundleComponentRef,
    reporter: &Reporter,
) -> Result<PathBuf> {
    let target = detect_target_key()?;
    let release = fetch_component_release_manifest(core_ref)?;
    let asset = find_installer_archive_asset(&release, &target)?;
    let archive_path = download_release_asset(root, asset, reporter, "x07up")?;

    let extract_root = cache_dir(root).join("x07up");
    extract_root.mkdir_all()?;
    let tmp_dir = extract_root.join(format!(".tmp_{}_{}", core_ref.tag, std::process::id()));
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir).ok();
    }
    tmp_dir.mkdir_all()?;
    reporter.progress("extract x07up");
    extract_archive(&archive_path, archive_format(&asset.name), &tmp_dir)?;

    let found = find_x07up_binary(&tmp_dir)?;

    reporter.progress("install x07up proxies");
    let dir = bin_dir(root);
    dir.mkdir_all()?;
    let tools = [
        "x07",
        "x07c",
        "x07-host-runner",
        "x07-os-runner",
        "x07import-cli",
        "x07-wasm",
        "x07-device-host-desktop",
        "x07up",
    ];
    for tool in tools {
        let dst = dir.join(tool);
        install_proxy_binary(&found, &dst)?;
    }

    std::fs::remove_dir_all(&tmp_dir).ok();
    Ok(dir.join("x07up"))
}

fn find_x07up_binary(extract_dir: &Path) -> Result<PathBuf> {
    let direct = extract_dir.join("x07up");
    if direct.is_file() {
        return Ok(direct);
    }
    let bin = extract_dir.join("bin").join("x07up");
    if bin.is_file() {
        return Ok(bin);
    }

    fn walk(dir: &Path) -> Result<Option<PathBuf>> {
        for entry in
            std::fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))?
        {
            let entry = entry?;
            let ty = entry.file_type()?;
            let path = entry.path();
            if ty.is_file() && entry.file_name() == "x07up" {
                return Ok(Some(path));
            }
            if ty.is_dir() {
                if let Some(found) = walk(&path)? {
                    return Ok(Some(found));
                }
            }
        }
        Ok(None)
    }

    walk(extract_dir)?.ok_or_else(|| {
        anyhow!(
            "x07up binary not found in archive extract dir: {}",
            extract_dir.display()
        )
    })
}

fn exec_updated_x07up(path: &Path, guard: &str) -> Result<std::process::ExitCode> {
    let mut cmd = std::process::Command::new(path);
    cmd.args(std::env::args_os().skip(1));
    cmd.env(SELF_UPDATE_GUARD_ENV, guard);
    cmd.stdin(std::process::Stdio::inherit());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        let err = cmd.exec();
        bail!("exec updated x07up failed: {err}");
    }

    #[cfg(not(unix))]
    {
        let _ = cmd;
        bail!("exec_updated_x07up: unsupported platform");
    }
}

fn cmd_docs(root: &Path, args: DocsArgs, reporter: &Reporter) -> Result<std::process::ExitCode> {
    match args.cmd {
        DocsCmd::Path => {
            let cfg = Config::load(&config_path(root))?;
            let sel = select_active_toolchain(root, &cfg)?;
            let tag = sel.resolved.ok_or_else(|| {
                anyhow!("no active toolchain resolved; run: x07up install stable")
            })?;
            let docs = toolchains_dir(root)
                .join(&tag)
                .join(X07_AGENT_DIR)
                .join("docs");
            if reporter.json {
                #[derive(Serialize)]
                struct DocsPathReport {
                    schema_version: &'static str,
                    ok: bool,
                    docs_root: String,
                }
                write_json_stdout(&DocsPathReport {
                    schema_version: "x07up.docs.path@0.1.0",
                    ok: docs.is_dir(),
                    docs_root: docs.display().to_string(),
                })?;
            } else {
                println!("{}", docs.display());
            }
            Ok(std::process::ExitCode::SUCCESS)
        }
    }
}

fn cmd_skills(
    root: &Path,
    args: SkillsArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    match args.cmd {
        SkillsCmd::Install(install) => cmd_skills_install(root, install, reporter),
        SkillsCmd::Status => cmd_skills_status(root, reporter),
    }
}

fn cmd_skills_install(
    root: &Path,
    args: SkillsInstallArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    if args.user == args.project.is_some() {
        bail!("exactly one of --user or --project must be set");
    }

    let cfg = Config::load(&config_path(root))?;
    let sel = select_active_toolchain(root, &cfg)?;
    let tag = sel
        .resolved
        .ok_or_else(|| anyhow!("no active toolchain resolved; run: x07up install stable"))?;
    let src = toolchains_dir(root)
        .join(&tag)
        .join(X07_AGENT_DIR)
        .join("skills");
    if !src.is_dir() {
        bail!("toolchain skills dir missing: {}", src.display());
    }

    let dst = if args.user {
        home_dir()?.join(X07_AGENT_DIR).join("skills")
    } else {
        let project = args.project.as_ref().unwrap();
        project.join(X07_AGENT_DIR).join("skills")
    };

    reporter.progress(&format!("copy skills to {}", dst.display()));
    copy_tree_skip_existing(&src, &dst)?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_skills_status(root: &Path, reporter: &Reporter) -> Result<std::process::ExitCode> {
    let cfg = Config::load(&config_path(root))?;
    let sel = select_active_toolchain(root, &cfg)?;
    let tag = sel.resolved.clone();
    let toolchain_skills = tag.as_ref().map(|t| {
        toolchains_dir(root)
            .join(t)
            .join(X07_AGENT_DIR)
            .join("skills")
    });
    let user_skills = home_dir()?.join(X07_AGENT_DIR).join("skills");

    #[derive(Serialize)]
    struct SkillsStatus {
        schema_version: &'static str,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        toolchain_skills_root: Option<String>,
        user_skills_root: String,
    }

    if reporter.json {
        write_json_stdout(&SkillsStatus {
            schema_version: "x07up.skills.status@0.1.0",
            ok: true,
            toolchain_skills_root: toolchain_skills.map(|p| p.display().to_string()),
            user_skills_root: user_skills.display().to_string(),
        })?;
    } else {
        println!("user: {}", user_skills.display());
        if let Some(p) = toolchain_skills {
            println!("toolchain: {}", p.display());
        }
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_agent(
    root: &Path,
    channels_url: &str,
    args: AgentArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    match args.cmd {
        AgentCmd::Init(init) => cmd_agent_init(root, channels_url, init, reporter),
    }
}

fn cmd_agent_init(
    root: &Path,
    channels_url: &str,
    args: AgentInitArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    let project = args.project.unwrap_or(std::env::current_dir()?);
    project.mkdir_all()?;

    if let Some(pin) = &args.pin {
        let toml = format!(
            "[toolchain]\nchannel = \"{}\"\ncomponents = [\"docs\", \"skills\"]\n",
            escape_toml_string(pin.trim())
        );
        std::fs::write(project.join(X07_TOOLCHAIN_TOML), toml.as_bytes())
            .context("write x07-toolchain.toml")?;
    }

    let cfg = Config::load(&config_path(root))?;
    let sel = select_active_toolchain(root, &cfg)?;
    let resolved = sel
        .resolved
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    match args.with_skills {
        AgentSkillsMode::None => {}
        AgentSkillsMode::User => {
            cmd_skills_install(
                root,
                SkillsInstallArgs {
                    user: true,
                    project: None,
                },
                reporter,
            )?;
        }
        AgentSkillsMode::Project => {
            cmd_skills_install(
                root,
                SkillsInstallArgs {
                    user: false,
                    project: Some(project.clone()),
                },
                reporter,
            )?;
        }
    }

    let docs_root = toolchains_dir(root)
        .join(&resolved)
        .join(X07_AGENT_DIR)
        .join("docs");
    let skills_root = match args.with_skills {
        AgentSkillsMode::Project => project.join(X07_AGENT_DIR).join("skills"),
        AgentSkillsMode::User => home_dir()?.join(X07_AGENT_DIR).join("skills"),
        AgentSkillsMode::None => PathBuf::from(""),
    };

    let template = include_str!("../assets/AGENT.template.md");
    let rendered = template
        .replace("{{X07_TOOLCHAIN_VERSION}}", &resolved)
        .replace(
            "{{X07_CHANNEL}}",
            cfg.default_toolchain.as_deref().unwrap_or("stable"),
        )
        .replace("{{X07_DOCS_ROOT}}", &docs_root.display().to_string())
        .replace("{{X07_SKILLS_ROOT}}", &skills_root.display().to_string());

    let out = project.join("AGENT.md");
    if out.exists() {
        bail!("refusing to overwrite existing AGENT.md: {}", out.display());
    }
    std::fs::write(&out, rendered.as_bytes())
        .with_context(|| format!("write {}", out.display()))?;
    reporter.progress("ok: wrote AGENT.md");
    reporter.progress(&format!(
        "hint: run: cd {} && x07up doctor",
        project.display()
    ));

    if reporter.json {
        #[derive(Serialize)]
        struct AgentInitReport {
            schema_version: &'static str,
            ok: bool,
            project: String,
            toolchain: String,
            docs_root: String,
            skills_root: String,
            channels_url: String,
        }
        write_json_stdout(&AgentInitReport {
            schema_version: "x07up.agent.init@0.1.0",
            ok: true,
            project: project.display().to_string(),
            toolchain: resolved,
            docs_root: docs_root.display().to_string(),
            skills_root: skills_root.display().to_string(),
            channels_url: channels_url.to_string(),
        })?;
    }

    Ok(std::process::ExitCode::SUCCESS)
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    schema_version: &'static str,
    ok: bool,
    root: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_toolchain: Option<String>,
    checks: Vec<DoctorCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    suggestions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    toolchain_doctor: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct DoctorCheck {
    name: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

fn cmd_doctor(
    root: &Path,
    _args: DoctorArgs,
    reporter: &Reporter,
) -> Result<std::process::ExitCode> {
    let target = detect_target_key()?;
    let cfg = Config::load(&config_path(root))?;
    let sel = select_active_toolchain(root, &cfg)?;
    let mut checks: Vec<DoctorCheck> = Vec::new();
    let mut suggestions: Vec<String> = Vec::new();
    let mut toolchain_doctor: Option<serde_json::Value> = None;

    let tag = match sel.resolved.clone() {
        Some(t) => t,
        None => {
            suggestions.push("Install a toolchain: x07up install stable".to_string());
            let report = DoctorReport {
                schema_version: DOCTOR_SCHEMA_VERSION,
                ok: false,
                root: root.display().to_string(),
                target,
                active_toolchain: None,
                checks: vec![DoctorCheck {
                    name: "active_toolchain".to_string(),
                    ok: false,
                    detail: Some("no active toolchain resolved".to_string()),
                }],
                suggestions,
                toolchain_doctor: None,
            };
            if reporter.json {
                write_json_stdout(&report)?;
            } else {
                println!("error: no active toolchain resolved");
            }
            return Ok(std::process::ExitCode::from(1));
        }
    };

    let tdir = toolchains_dir(root).join(&tag);
    checks.push(DoctorCheck {
        name: "toolchain_dir".to_string(),
        ok: tdir.is_dir(),
        detail: Some(tdir.display().to_string()),
    });

    let required_bins = [
        "x07",
        "x07c",
        "x07-host-runner",
        "x07-os-runner",
        "x07import-cli",
    ];
    for b in required_bins {
        let p = tool_path(root, &tag, b)?;
        checks.push(DoctorCheck {
            name: format!("bin:{b}"),
            ok: p.is_file(),
            detail: Some(p.display().to_string()),
        });
    }

    let os_modules = tdir.join("stdlib/os/0.2.0/modules");
    checks.push(DoctorCheck {
        name: "stdlib_os_modules".to_string(),
        ok: os_modules.is_dir(),
        detail: Some(os_modules.display().to_string()),
    });

    match run_x07_doctor(root, &tag) {
        Ok(out) => {
            checks.push(DoctorCheck {
                name: "x07_doctor".to_string(),
                ok: out.ok,
                detail: Some(format!("ok={}", out.ok)),
            });
            suggestions.extend(out.suggestions);
            toolchain_doctor = Some(out.raw);
        }
        Err(err) => {
            checks.push(DoctorCheck {
                name: "x07_doctor".to_string(),
                ok: false,
                detail: Some(format!("{err:#}")),
            });
        }
    }

    let host_smoke_ok = match check_host_runner_smoke(root, &tag) {
        Ok(()) => true,
        Err(err) => {
            checks.push(DoctorCheck {
                name: "host_runner_smoke".to_string(),
                ok: false,
                detail: Some(format!("{err:#}")),
            });
            false
        }
    };
    if host_smoke_ok {
        checks.push(DoctorCheck {
            name: "host_runner_smoke".to_string(),
            ok: true,
            detail: Some("ok".to_string()),
        });
    } else {
        suggestions.push("Reinstall the toolchain: x07up install stable".to_string());
    }

    let ok = checks.iter().all(|c| c.ok);
    let report = DoctorReport {
        schema_version: DOCTOR_SCHEMA_VERSION,
        ok,
        root: root.display().to_string(),
        target,
        active_toolchain: Some(tag),
        checks,
        suggestions,
        toolchain_doctor,
    };

    if reporter.json {
        write_json_stdout(&report)?;
    } else if ok {
        println!("ok: x07up doctor");
    } else {
        println!("error: x07up doctor found problems");
    }
    Ok(if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn check_host_runner_smoke(root: &Path, tag: &str) -> Result<()> {
    let exe = tool_path(root, tag, "x07-host-runner")?;
    if !exe.is_file() {
        bail!("missing x07-host-runner: {}", exe.display());
    }
    let tmp = std::env::temp_dir().join(format!("x07up_doctor_{}", std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).ok();
    }
    tmp.mkdir_all()?;
    let prog = tmp.join("program.x07.json");
    let input = tmp.join("input.bin");
    std::fs::write(
        &prog,
        br#"{"schema_version":"x07.x07ast@0.3.0","kind":"entry","module_id":"main","imports":[],"decls":[],"solve":["view.to_bytes","input"]}"#,
    )?;
    std::fs::write(&input, b"hi")?;

    let out = std::process::Command::new(exe)
        .arg("--program")
        .arg(&prog)
        .arg("--world")
        .arg("solve-pure")
        .arg("--input")
        .arg(&input)
        .output()
        .context("exec x07-host-runner")?;

    if !out.status.success() {
        bail!(
            "x07-host-runner smoke failed (status {})\nstdout:\n{}\nstderr:\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    Ok(())
}

#[derive(Debug)]
struct X07DoctorOut {
    ok: bool,
    suggestions: Vec<String>,
    raw: serde_json::Value,
}

fn run_x07_doctor(root: &Path, tag: &str) -> Result<X07DoctorOut> {
    #[derive(Debug, Deserialize)]
    struct X07DoctorReport {
        ok: bool,
        #[serde(default)]
        suggestions: Vec<String>,
    }

    let exe = tool_path(root, tag, "x07")?;
    let out = std::process::Command::new(exe)
        .arg("doctor")
        .output()
        .context("exec x07 doctor")?;
    let raw: serde_json::Value =
        serde_json::from_slice(&out.stdout).context("parse x07 doctor json")?;
    let parsed: X07DoctorReport =
        serde_json::from_value(raw.clone()).context("parse x07 doctor shape")?;
    Ok(X07DoctorOut {
        ok: out.status.success() && parsed.ok,
        suggestions: parsed.suggestions,
        raw,
    })
}

#[derive(Debug)]
struct ActiveSelection {
    spec: String,
    resolved: Option<String>,
    source: String,
    override_file: Option<PathBuf>,
}

fn select_active_toolchain(root: &Path, cfg: &Config) -> Result<ActiveSelection> {
    if let Some(v) = std::env::var_os("X07UP_TOOLCHAIN") {
        let spec = v.to_string_lossy().to_string();
        return Ok(ActiveSelection {
            resolved: resolve_spec(root, cfg, &spec),
            spec,
            source: "env:X07UP_TOOLCHAIN".to_string(),
            override_file: None,
        });
    }

    if let Some((path, spec)) = find_project_override(std::env::current_dir()?)? {
        return Ok(ActiveSelection {
            resolved: resolve_spec(root, cfg, &spec),
            spec,
            source: "override".to_string(),
            override_file: Some(path),
        });
    }

    if let Some(spec) = cfg.default_toolchain.clone().filter(|s| !s.is_empty()) {
        return Ok(ActiveSelection {
            resolved: resolve_spec(root, cfg, &spec),
            spec,
            source: "config".to_string(),
            override_file: None,
        });
    }

    let spec = "stable".to_string();
    Ok(ActiveSelection {
        resolved: resolve_spec(root, cfg, &spec),
        spec,
        source: "fallback".to_string(),
        override_file: None,
    })
}

fn resolve_spec(root: &Path, cfg: &Config, spec: &str) -> Option<String> {
    if looks_like_tag(spec) {
        return Some(spec.to_string());
    }
    if let Some(tag) = cfg.channels.get(spec) {
        return Some(tag.clone());
    }
    let candidate = toolchains_dir(root).join(spec);
    if candidate.is_dir() {
        return Some(spec.to_string());
    }
    None
}

fn find_project_override(mut dir: PathBuf) -> Result<Option<(PathBuf, String)>> {
    loop {
        let cand = dir.join(X07_TOOLCHAIN_TOML);
        if cand.is_file() {
            let spec = parse_toolchain_toml(&std::fs::read_to_string(&cand)?)?;
            return Ok(Some((cand, spec)));
        }
        if !dir.pop() {
            break;
        }
    }
    Ok(None)
}

fn parse_toolchain_toml(contents: &str) -> Result<String> {
    // Minimal parser: accept `channel = "..."` anywhere in the file.
    for line in contents.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("channel") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let val = rest.trim();
                if let Some(val) = val.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
                    let v = val.trim();
                    if v.is_empty() {
                        bail!("x07-toolchain.toml has empty channel");
                    }
                    return Ok(v.to_string());
                }
            }
        }
    }
    bail!("x07-toolchain.toml missing toolchain.channel")
}

fn tool_path(root: &Path, toolchain_tag: &str, tool: &str) -> Result<PathBuf> {
    validate_toolchain_id(toolchain_tag)?;
    let p = toolchains_dir(root)
        .join(toolchain_tag)
        .join("bin")
        .join(tool);
    Ok(p)
}

fn list_installed_toolchains(root: &Path) -> Result<Vec<InstalledToolchain>> {
    let dir = toolchains_dir(root);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if !ty.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        out.push(InstalledToolchain {
            toolchain: name,
            dir: entry.path().display().to_string(),
        });
    }
    out.sort_by(|a, b| a.toolchain.cmp(&b.toolchain));
    Ok(out)
}

fn write_json_stdout<T: Serialize>(v: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec(v)?;
    bytes.push(b'\n');
    std::io::stdout()
        .write_all(&bytes)
        .context("write stdout")?;
    Ok(())
}

fn url_filename(url: &str) -> Option<String> {
    let parsed = url.split('?').next().unwrap_or(url);
    let file = parsed.rsplit('/').next()?;
    if file.is_empty() {
        return None;
    }
    Some(file.to_string())
}

fn download_verify(url: &str, dest: &Path, expected_sha256: &str) -> Result<()> {
    let resp = ureq::get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    let mut reader = resp.into_body().into_reader();

    let tmp = dest.with_extension("download.tmp");
    if let Some(parent) = tmp.parent() {
        parent.mkdir_all()?;
    }
    let mut f = File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 64];
    loop {
        let n = reader.read(&mut buf).context("read download stream")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        f.write_all(&buf[..n]).context("write download")?;
    }
    f.flush().ok();
    drop(f);

    let actual = hex_lower(&hasher.finalize());
    if !eq_hex_sha256(&actual, expected_sha256) {
        let _ = std::fs::remove_file(&tmp);
        bail!("sha256 mismatch for {url}: expected {expected_sha256}, got {actual}");
    }

    rename_overwrite_file(&tmp, dest)?;
    Ok(())
}

fn eq_hex_sha256(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn extract_archive(path: &Path, format: &str, out_dir: &Path) -> Result<()> {
    match format {
        "tar.gz" => extract_tar_gz(path, out_dir),
        "tar.xz" => extract_tar_xz(path, out_dir),
        "zip" => extract_zip(path, out_dir),
        other => bail!("unsupported archive format: {other}"),
    }
}

fn extract_tar_gz(path: &Path, out_dir: &Path) -> Result<()> {
    let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let gz = GzDecoder::new(f);
    let mut ar = tar::Archive::new(gz);
    for entry in ar.entries().context("read tar entries")? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_path_buf();
        let rel = sanitize_rel_path(&entry_path)?;
        let out_path = out_dir.join(rel);
        if let Some(parent) = out_path.parent() {
            parent.mkdir_all()?;
        }
        entry
            .unpack(&out_path)
            .with_context(|| format!("unpack {}", out_path.display()))?;
    }
    Ok(())
}

fn extract_tar_xz(path: &Path, out_dir: &Path) -> Result<()> {
    let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let xz = XzDecoder::new(f);
    let mut ar = tar::Archive::new(xz);
    for entry in ar.entries().context("read tar entries")? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_path_buf();
        let rel = sanitize_rel_path(&entry_path)?;
        let out_path = out_dir.join(rel);
        if let Some(parent) = out_path.parent() {
            parent.mkdir_all()?;
        }
        entry
            .unpack(&out_path)
            .with_context(|| format!("unpack {}", out_path.display()))?;
    }
    Ok(())
}

fn extract_zip(path: &Path, out_dir: &Path) -> Result<()> {
    let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut z = zip::ZipArchive::new(f).context("open zip")?;
    for i in 0..z.len() {
        let mut file = z.by_index(i).context("zip entry")?;
        let name = file.name().to_string();
        let rel = sanitize_rel_path(Path::new(&name))?;
        let out_path = out_dir.join(rel);
        if file.is_dir() {
            out_path.mkdir_all()?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            parent.mkdir_all()?;
        }
        let mut out =
            File::create(&out_path).with_context(|| format!("create {}", out_path.display()))?;
        std::io::copy(&mut file, &mut out)
            .with_context(|| format!("write {}", out_path.display()))?;
    }
    Ok(())
}

fn sanitize_rel_path(path: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => {
                bail!("invalid archive path (absolute): {}", path.display())
            }
            Component::ParentDir => bail!("invalid archive path (..): {}", path.display()),
            Component::CurDir => {}
            Component::Normal(p) => out.push(p),
        }
    }
    Ok(out)
}

fn rename_overwrite_file(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() && dst.is_dir() {
        bail!("refusing to overwrite directory: {}", dst.display());
    }
    std::fs::rename(src, dst)
        .with_context(|| format!("rename {} -> {}", src.display(), dst.display()))?;
    Ok(())
}

fn ensure_proxies(root: &Path) -> Result<()> {
    let src = std::env::current_exe().context("current_exe")?;
    let dir = bin_dir(root);
    dir.mkdir_all()?;

    let tools = [
        "x07",
        "x07c",
        "x07-host-runner",
        "x07-os-runner",
        "x07import-cli",
        "x07-wasm",
        "x07-device-host-desktop",
        "x07up",
    ];
    for tool in tools {
        let dst = dir.join(tool);
        install_proxy_binary(&src, &dst)?;
    }
    Ok(())
}

fn install_proxy_binary(src: &Path, dst: &Path) -> Result<()> {
    if src == dst {
        return Ok(());
    }
    let tmp = dst.with_extension("tmp");
    if let Some(parent) = tmp.parent() {
        parent.mkdir_all()?;
    }
    std::fs::copy(src, &tmp)
        .with_context(|| format!("copy {} -> {}", src.display(), tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let perm = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&tmp, perm).with_context(|| format!("chmod {}", tmp.display()))?;
    }
    rename_overwrite_file(&tmp, dst)?;
    Ok(())
}

fn proxy_dispatch(invoked: &str) -> Result<std::process::ExitCode> {
    let root = effective_root(None)?;
    let cfg = Config::load(&config_path(&root))?;
    let sel = select_active_toolchain(&root, &cfg)?;
    let tag = sel
        .resolved
        .ok_or_else(|| anyhow!("no active toolchain resolved; run: x07up install stable"))?;
    let exe = tool_path(&root, &tag, invoked)?;
    if !exe.is_file() {
        let hint = match invoked {
            "x07-wasm" => format!(
                "install the wasm component: x07up component add {}",
                ComponentName::Wasm.cli_name()
            ),
            "x07-device-host-desktop" => format!(
                "install the device-host component: x07up component add {}",
                ComponentName::DeviceHost.cli_name()
            ),
            _ => format!("reinstall toolchain: x07up install {}", sel.spec),
        };
        bail!(
            "tool missing in active toolchain: {}\nhint: {}",
            exe.display(),
            hint
        );
    }

    let mut cmd = std::process::Command::new(exe);
    cmd.args(std::env::args_os().skip(1));
    cmd.stdin(std::process::Stdio::inherit());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    use std::os::unix::process::CommandExt as _;
    let err = cmd.exec();
    bail!("exec failed: {err}");
}

fn copy_tree_skip_existing(src: &Path, dst: &Path) -> Result<()> {
    if !src.is_dir() {
        bail!("copy_tree source is not a directory: {}", src.display());
    }
    dst.mkdir_all()?;
    for entry in std::fs::read_dir(src).with_context(|| format!("read_dir {}", src.display()))? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let name = entry.file_name();
        let src_path = entry.path();
        let dst_path = dst.join(name);
        if dst_path.exists() {
            continue;
        }
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if ty.is_file() {
            std::fs::copy(&src_path, &dst_path).with_context(|| {
                format!("copy {} -> {}", src_path.display(), dst_path.display())
            })?;
        }
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    dst.mkdir_all()?;
    for entry in std::fs::read_dir(src).with_context(|| format!("read_dir {}", src.display()))? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if ty.is_file() {
            std::fs::copy(&src_path, &dst_path).with_context(|| {
                format!("copy {} -> {}", src_path.display(), dst_path.display())
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_prefix_accepts_plain() {
        assert_eq!(
            parse_semver_prefix("0.0.86"),
            Some(SemVer {
                major: 0,
                minor: 0,
                patch: 86
            })
        );
    }

    #[test]
    fn parse_semver_prefix_accepts_tag_and_suffix() {
        assert_eq!(
            parse_semver_prefix("v0.0.0-ci"),
            Some(SemVer {
                major: 0,
                minor: 0,
                patch: 0
            })
        );
        assert_eq!(
            parse_semver_prefix("v1.2.3+meta"),
            Some(SemVer {
                major: 1,
                minor: 2,
                patch: 3
            })
        );
    }

    #[test]
    fn bundle_url_for_spec_derives_release_bundle() -> Result<()> {
        assert_eq!(
            bundle_url_for_spec("https://x07lang.org/install/channels/stable.json", "beta")?,
            "https://x07lang.org/install/channels/beta.json"
        );
        assert_eq!(
            bundle_url_for_spec(
                "https://x07lang.org/install/channels/stable.json",
                "v0.1.56"
            )?,
            "https://github.com/x07lang/x07/releases/download/v0.1.56/x07-0.1.56-bundle.json"
        );
        Ok(())
    }

    #[test]
    fn find_x07up_binary_prefers_common_locations() -> Result<()> {
        let base = std::env::temp_dir().join(format!("x07up-test-{}", std::process::id()));
        if base.exists() {
            std::fs::remove_dir_all(&base).ok();
        }
        base.mkdir_all()?;
        let bin_dir = base.join("bin");
        bin_dir.mkdir_all()?;
        std::fs::write(bin_dir.join("x07up"), b"stub")?;
        let found = find_x07up_binary(&base)?;
        assert_eq!(found, bin_dir.join("x07up"));
        std::fs::remove_dir_all(&base).ok();
        Ok(())
    }
}
