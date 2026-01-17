use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexConfig {
    pub dl: String,
    pub api: String,
    #[serde(default, rename = "auth-required")]
    pub auth_required: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexEntry {
    pub schema_version: String,
    pub name: String,
    pub version: String,
    pub cksum: String,
    pub yanked: bool,
}

#[derive(Debug, Clone)]
pub struct SparseIndexClient {
    index_root: Url,
    dl_root: Url,
    api_root: Url,
    auth_required: bool,
    token: Option<String>,
}

impl SparseIndexClient {
    pub fn from_index_url(index_url: &str, token: Option<String>) -> Result<Self> {
        let raw = index_url.strip_prefix("sparse+").unwrap_or(index_url);
        let index_root = Url::parse(raw)
            .with_context(|| format!("invalid index url: {index_url:?} (expected URL)"))?;
        if !index_root.as_str().ends_with('/') {
            anyhow::bail!(
                "index url must end with '/': got {:?} (example: sparse+https://host/index/)",
                index_root.as_str()
            );
        }

        let config_url = index_root
            .join("config.json")
            .context("index url join config.json")?;
        let config_bytes =
            fetch_bytes(&config_url, token.as_deref()).context("fetch index config.json")?;
        let config: IndexConfig = serde_json::from_slice(&config_bytes)
            .with_context(|| format!("parse index config.json: {}", config_url.as_str()))?;

        let dl = if config.dl.as_str().ends_with('/') {
            config.dl.clone()
        } else {
            format!("{}/", config.dl)
        };
        let api = if config.api.as_str().ends_with('/') {
            config.api.clone()
        } else {
            format!("{}/", config.api)
        };
        let dl_root =
            Url::parse(&dl).with_context(|| format!("invalid index config dl url: {:?}", dl))?;
        let api_root =
            Url::parse(&api).with_context(|| format!("invalid index config api url: {:?}", api))?;

        Ok(Self {
            index_root,
            dl_root,
            api_root,
            auth_required: config.auth_required,
            token,
        })
    }

    pub fn auth_required(&self) -> bool {
        self.auth_required
    }

    pub fn canonical_index_url(index_url: &str) -> Result<String> {
        let raw = index_url.strip_prefix("sparse+").unwrap_or(index_url);
        let url = Url::parse(raw).with_context(|| format!("invalid index url: {index_url:?}"))?;
        if !url.as_str().ends_with('/') {
            anyhow::bail!("index url must end with '/': got {:?}", url.as_str());
        }
        Ok(format!("sparse+{}", url.as_str()))
    }

    pub fn api_root(&self) -> &Url {
        &self.api_root
    }

    pub fn fetch_entries(&self, package_name: &str) -> Result<Vec<IndexEntry>> {
        let rel = index_relative_path(package_name)?;
        let url = self
            .index_root
            .join(&rel)
            .with_context(|| format!("index url join: {rel:?}"))?;
        let bytes = fetch_bytes(&url, self.token_for_fetch())?;
        parse_ndjson(&bytes).with_context(|| format!("parse index file: {}", url.as_str()))
    }

    pub fn download_url(&self, package_name: &str, version: &str) -> Result<Url> {
        let url = self
            .dl_root
            .join(&format!("{package_name}/{version}/download"))
            .with_context(|| format!("dl url join for {package_name}@{version}"))?;
        Ok(url)
    }

    pub fn download_to_file(
        &self,
        package_name: &str,
        version: &str,
        expected_sha256_hex: &str,
        out: &Path,
    ) -> Result<()> {
        let url = self.download_url(package_name, version)?;
        let bytes = fetch_bytes(&url, self.token_for_fetch())?;
        let actual = sha256_hex(&bytes);
        if actual != expected_sha256_hex {
            anyhow::bail!(
                "download sha256 mismatch for {package_name}@{version}: expected {expected_sha256_hex} got {actual}"
            );
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir: {}", parent.display()))?;
        }
        std::fs::write(out, &bytes).with_context(|| format!("write: {}", out.display()))?;
        Ok(())
    }

    fn token_for_fetch(&self) -> Option<&str> {
        self.token.as_deref()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct CredentialsFile {
    schema_version: String,
    #[serde(default)]
    tokens: BTreeMap<String, String>,
}

pub fn credentials_path() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("X07_PKG_HOME") {
        let base = PathBuf::from(dir);
        return Ok(base.join("credentials.json"));
    }

    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".x07").join("credentials.json"));
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        return Ok(PathBuf::from(home).join(".x07").join("credentials.json"));
    }

    anyhow::bail!("missing HOME/USERPROFILE; set X07_PKG_HOME to store credentials")
}

pub fn load_token(index_url: &str) -> Result<Option<String>> {
    let key = SparseIndexClient::canonical_index_url(index_url)?;
    let path = credentials_path()?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    let creds: CredentialsFile =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    Ok(creds.tokens.get(&key).cloned())
}

pub fn store_token(index_url: &str, token: &str) -> Result<()> {
    let token = token.trim();
    if token.is_empty() {
        anyhow::bail!("token must be non-empty");
    }
    let key = SparseIndexClient::canonical_index_url(index_url)?;
    let path = credentials_path()?;
    let mut creds: CredentialsFile = match std::fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => CredentialsFile {
            schema_version: "x07.credentials@0.1.0".to_string(),
            tokens: BTreeMap::new(),
        },
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };

    if creds.schema_version.trim().is_empty() {
        creds.schema_version = "x07.credentials@0.1.0".to_string();
    }
    creds.tokens.insert(key, token.to_string());

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }
    let mut out = serde_json::to_vec_pretty(&creds)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    std::fs::write(&path, &out).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn http_post_bytes(url: &Url, token: Option<&str>, body: &[u8]) -> Result<Vec<u8>> {
    match url.scheme() {
        "http" | "https" => {
            let mut req =
                ureq::post(url.as_str()).header("Content-Type", "application/octet-stream");
            if let Some(token) = token {
                req = req.header("Authorization", &format!("Bearer {token}"));
            }
            let resp = req
                .send(body)
                .map_err(|e| anyhow::anyhow!("http POST {}: {e}", url))?;
            let mut reader = resp.into_body().into_reader();
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).context("read http response")?;
            Ok(buf)
        }
        other => anyhow::bail!("unsupported url scheme {other:?} for {}", url.as_str()),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

pub fn unpack_tar_bytes(archive_bytes: &[u8], dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("create dest dir: {}", dest_dir.display()))?;

    let mut archive = tar::Archive::new(std::io::Cursor::new(archive_bytes));
    for entry in archive.entries().context("read tar entries")? {
        let mut entry = entry.context("read tar entry")?;
        let path = entry.path().context("read tar entry path")?.into_owned();
        validate_archive_rel_path(&path)?;
        let out_path = dest_dir.join(&path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir: {}", parent.display()))?;
        }
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            std::fs::create_dir_all(&out_path)
                .with_context(|| format!("create dir: {}", out_path.display()))?;
            continue;
        }
        if !entry_type.is_file() {
            anyhow::bail!("unsupported tar entry type for {:?}", path);
        }
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .context("read tar entry bytes")?;
        std::fs::write(&out_path, &buf)
            .with_context(|| format!("write file: {}", out_path.display()))?;
    }
    Ok(())
}

pub fn build_tar_bytes(entries: &[(PathBuf, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut normalized: Vec<(PathBuf, Vec<u8>)> = entries
        .iter()
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect();
    normalized.sort_by(|(a, _), (b, _)| {
        a.as_os_str()
            .as_encoded_bytes()
            .cmp(b.as_os_str().as_encoded_bytes())
    });

    let mut buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut buf);
        builder.mode(tar::HeaderMode::Deterministic);
        for (path, bytes) in &normalized {
            validate_archive_rel_path(path)?;
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_cksum();
            builder
                .append_data(&mut header, path, std::io::Cursor::new(bytes))
                .with_context(|| format!("append tar entry: {}", path.display()))?;
        }
        builder.finish().context("finish tar")?;
    }
    Ok(buf)
}

fn validate_archive_rel_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        anyhow::bail!("empty archive path");
    }
    if path.is_absolute() {
        anyhow::bail!("absolute archive paths are not allowed: {:?}", path);
    }
    for component in path.components() {
        match component {
            std::path::Component::Prefix(_) => {
                anyhow::bail!("windows prefix archive paths are not allowed: {:?}", path);
            }
            std::path::Component::ParentDir => {
                anyhow::bail!("archive paths must not contain '..': {:?}", path);
            }
            std::path::Component::CurDir => {
                anyhow::bail!("archive paths must not contain '.': {:?}", path);
            }
            std::path::Component::RootDir | std::path::Component::Normal(_) => {}
        }
    }
    Ok(())
}

fn index_relative_path(package_name: &str) -> Result<String> {
    let name = package_name.trim();
    if name.is_empty() {
        anyhow::bail!("package name must be non-empty");
    }
    let lower = name.to_ascii_lowercase();
    if lower != name {
        anyhow::bail!("package name must be lowercase: got {:?}", name);
    }
    if !lower.is_ascii() {
        anyhow::bail!("package name must be ASCII: got {:?}", name);
    }
    let bytes = lower.as_bytes();
    for &b in bytes {
        match b {
            b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' => {}
            _ => anyhow::bail!("package name contains invalid characters: {:?}", name),
        }
    }

    let shard = match bytes.len() {
        1 => "1".to_string(),
        2 => "2".to_string(),
        3 => format!("3/{}", &lower[0..1]),
        _ => format!("{}/{}", &lower[0..2], &lower[2..4]),
    };
    Ok(format!("{shard}/{lower}"))
}

fn fetch_bytes(url: &Url, token: Option<&str>) -> Result<Vec<u8>> {
    match url.scheme() {
        "file" => {
            let path = url.to_file_path().map_err(|_| {
                anyhow::anyhow!("file url could not be converted to a path: {:?}", url)
            })?;
            std::fs::read(&path).with_context(|| format!("read {}", path.display()))
        }
        "http" | "https" => {
            let mut req = ureq::get(url.as_str());
            if let Some(token) = token {
                req = req.header("Authorization", &format!("Bearer {token}"));
            }
            let resp = req
                .call()
                .map_err(|e| anyhow::anyhow!("http GET {}: {e}", url))?;
            let mut reader = resp.into_body().into_reader();
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).context("read http response")?;
            Ok(buf)
        }
        other => anyhow::bail!("unsupported url scheme {other:?} for {}", url.as_str()),
    }
}

fn parse_ndjson(bytes: &[u8]) -> Result<Vec<IndexEntry>> {
    let text = std::str::from_utf8(bytes).context("index file is not utf-8")?;
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: IndexEntry = serde_json::from_str(line)
            .with_context(|| format!("parse ndjson line {}: {}", idx + 1, line))?;
        if entry.schema_version.trim() != "x07.index-entry@0.1.0" {
            anyhow::bail!(
                "index entry schema_version mismatch: expected x07.index-entry@0.1.0 got {:?}",
                entry.schema_version
            );
        }
        out.push(entry);
    }
    Ok(out)
}
