use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const X07_LABEL_SCHEMA_KEY: &str = "io.x07.schema";
pub const X07_LABEL_SCHEMA_VALUE: &str = "1";
pub const X07_LABEL_RUN_ID_KEY: &str = "io.x07.run_id";
pub const X07_LABEL_JOB_ID_KEY: &str = "io.x07.job_id";
pub const X07_LABEL_RUNNER_INSTANCE_KEY: &str = "io.x07.runner_instance";
pub const X07_LABEL_DEADLINE_UNIX_MS_KEY: &str = "io.x07.deadline_unix_ms";
pub const X07_LABEL_IMAGE_DIGEST_KEY: &str = "io.x07.image_digest";
pub const X07_LABEL_BACKEND_KEY: &str = "io.x07.backend";
pub const X07_LABEL_CREATED_UNIX_MS_KEY: &str = "io.x07.created_unix_ms";

const CONTAINERD_KV_MAX_BYTES: usize = 4096;
const RUNNER_INSTANCE_FILE: &str = "runner_instance";

#[derive(Debug)]
pub enum LabelError {
    Empty(&'static str),
    InvalidKey {
        key: String,
        why: &'static str,
    },
    InvalidValue {
        key: &'static str,
        value: String,
        why: &'static str,
    },
    TooLarge {
        key: String,
        key_len: usize,
        value_len: usize,
        max: usize,
    },
}

impl std::fmt::Display for LabelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LabelError::Empty(field) => write!(f, "label field {field} is empty"),
            LabelError::InvalidKey { key, why } => write!(f, "invalid label key {key:?}: {why}"),
            LabelError::InvalidValue { key, value, why } => {
                write!(f, "invalid label value for key {key}: {value:?}: {why}")
            }
            LabelError::TooLarge {
                key,
                key_len,
                value_len,
                max,
            } => write!(
                f,
                "label key/value too large for {key:?}: {key_len}+{value_len} > {max} bytes"
            ),
        }
    }
}

impl std::error::Error for LabelError {}

#[derive(Debug, Clone)]
pub struct X07LabelSet {
    pub run_id: String,
    pub runner_instance: String,
    pub deadline_unix_ms: u64,
    pub job_id: Option<String>,
    pub image_digest: Option<String>,
    pub backend: Option<String>,
    pub created_unix_ms: Option<u64>,
}

impl X07LabelSet {
    pub fn new(
        run_id: impl Into<String>,
        runner_instance: impl Into<String>,
        deadline_unix_ms: u64,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            runner_instance: runner_instance.into(),
            deadline_unix_ms,
            job_id: None,
            image_digest: None,
            backend: None,
            created_unix_ms: None,
        }
    }

    pub fn with_job_id(mut self, job_id: impl Into<String>) -> Self {
        self.job_id = Some(job_id.into());
        self
    }

    pub fn with_image_digest(mut self, image_digest: impl Into<String>) -> Self {
        self.image_digest = Some(image_digest.into());
        self
    }

    pub fn with_backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }

    pub fn with_created_unix_ms(mut self, created_unix_ms: u64) -> Self {
        self.created_unix_ms = Some(created_unix_ms);
        self
    }

    fn kv_pairs_ordered(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::with_capacity(8);
        out.push((X07_LABEL_SCHEMA_KEY, X07_LABEL_SCHEMA_VALUE.to_string()));
        out.push((X07_LABEL_RUN_ID_KEY, self.run_id.clone()));
        if let Some(job_id) = &self.job_id {
            out.push((X07_LABEL_JOB_ID_KEY, job_id.clone()));
        }
        out.push((X07_LABEL_RUNNER_INSTANCE_KEY, self.runner_instance.clone()));
        out.push((
            X07_LABEL_DEADLINE_UNIX_MS_KEY,
            self.deadline_unix_ms.to_string(),
        ));

        if let Some(d) = &self.image_digest {
            out.push((X07_LABEL_IMAGE_DIGEST_KEY, d.clone()));
        }
        if let Some(b) = &self.backend {
            out.push((X07_LABEL_BACKEND_KEY, b.clone()));
        }
        if let Some(ts) = self.created_unix_ms {
            out.push((X07_LABEL_CREATED_UNIX_MS_KEY, ts.to_string()));
        }

        out
    }

    pub fn validate(&self) -> Result<(), LabelError> {
        if self.run_id.is_empty() {
            return Err(LabelError::Empty("run_id"));
        }
        if self.runner_instance.is_empty() {
            return Err(LabelError::Empty("runner_instance"));
        }
        if self.deadline_unix_ms == 0 {
            return Err(LabelError::InvalidValue {
                key: X07_LABEL_DEADLINE_UNIX_MS_KEY,
                value: "0".into(),
                why: "deadline_unix_ms must be > 0",
            });
        }

        for (k, v) in self.kv_pairs_ordered() {
            validate_x07_label_key(k)?;
            validate_x07_label_value(k, &v)?;
            validate_containerd_kv_size(k, &v)?;
        }
        Ok(())
    }

    pub fn render_kv_strings(&self) -> Result<Vec<String>, LabelError> {
        self.validate()?;
        Ok(self
            .kv_pairs_ordered()
            .into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect())
    }

    pub fn to_btreemap(&self) -> Result<BTreeMap<String, String>, LabelError> {
        self.validate()?;
        Ok(self
            .kv_pairs_ordered()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect())
    }

    pub fn push_flagged_kv(&self, argv: &mut Vec<OsString>, flag: &str) -> Result<(), LabelError> {
        for kv in self.render_kv_strings()? {
            argv.push(OsString::from(flag));
            argv.push(OsString::from(kv));
        }
        Ok(())
    }
}

pub fn read_or_create_runner_instance_id(state_root: &Path) -> Result<String> {
    let path = runner_instance_path(state_root);
    if let Ok(raw) = std::fs::read_to_string(&path) {
        let s = raw.trim();
        validate_runner_instance(s)
            .with_context(|| format!("invalid runner_instance in {}", path.display()))?;
        return Ok(s.to_string());
    }

    let id = generate_runner_instance_id().context("generate runner_instance")?;
    validate_runner_instance(&id).context("generated runner_instance invalid")?;
    atomic_write_string(&path, &(id.clone() + "\n"))
        .with_context(|| format!("write runner_instance: {}", path.display()))?;
    Ok(id)
}

fn runner_instance_path(state_root: &Path) -> PathBuf {
    let dir = state_root.parent().unwrap_or(state_root);
    dir.join(RUNNER_INSTANCE_FILE)
}

fn validate_runner_instance(v: &str) -> Result<(), LabelError> {
    if v.is_empty() {
        return Err(LabelError::Empty("runner_instance"));
    }
    if !v.is_ascii() {
        return Err(LabelError::InvalidValue {
            key: X07_LABEL_RUNNER_INSTANCE_KEY,
            value: v.to_string(),
            why: "must be ASCII",
        });
    }
    if v.len() > 128 {
        return Err(LabelError::InvalidValue {
            key: X07_LABEL_RUNNER_INSTANCE_KEY,
            value: v.to_string(),
            why: "must be <= 128 bytes",
        });
    }
    for b in v.bytes() {
        let c = b as char;
        if c.is_ascii_whitespace() || c.is_ascii_control() || c == '=' {
            return Err(LabelError::InvalidValue {
                key: X07_LABEL_RUNNER_INSTANCE_KEY,
                value: v.to_string(),
                why: "must not contain whitespace/control/equals",
            });
        }
    }
    Ok(())
}

fn generate_runner_instance_id() -> Result<String> {
    let mut bytes: [u8; 16] = [0; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read as _;
        if f.read_exact(&mut bytes).is_ok() {
            return Ok(format!("ri-{}", hex_lower(&bytes)));
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let pid = std::process::id();
    Ok(format!("ri-{now:x}-{pid:x}"))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize]);
        out.push(HEX[(b & 0x0f) as usize]);
    }
    String::from_utf8(out).unwrap_or_default()
}

fn atomic_write_string(path: &Path, content: &str) -> Result<()> {
    let Some(parent) = path.parent() else {
        anyhow::bail!("path has no parent: {}", path.display());
    };
    std::fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;

    let tmp = parent.join(format!(
        ".{}.tmp.{}",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("x07"),
        std::process::id()
    ));
    std::fs::write(&tmp, content).with_context(|| format!("write tmp file {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

fn validate_x07_label_key(key: &str) -> Result<(), LabelError> {
    if key.is_empty() {
        return Err(LabelError::InvalidKey {
            key: key.into(),
            why: "empty",
        });
    }
    if !key.starts_with("io.x07.") {
        return Err(LabelError::InvalidKey {
            key: key.into(),
            why: "must start with 'io.x07.'",
        });
    }
    if !key.is_ascii() {
        return Err(LabelError::InvalidKey {
            key: key.into(),
            why: "must be ASCII",
        });
    }
    for b in key.bytes() {
        let c = b as char;
        let ok = matches!(c, 'a'..='z' | '0'..='9' | '.' | '_' | '-');
        if !ok {
            return Err(LabelError::InvalidKey {
                key: key.into(),
                why: "contains invalid character",
            });
        }
    }
    Ok(())
}

fn validate_x07_label_value(key: &'static str, value: &str) -> Result<(), LabelError> {
    if value.is_empty() {
        return Err(LabelError::InvalidValue {
            key,
            value: value.into(),
            why: "empty",
        });
    }
    if !value.is_ascii() {
        return Err(LabelError::InvalidValue {
            key,
            value: value.into(),
            why: "must be ASCII",
        });
    }
    for b in value.bytes() {
        let c = b as char;
        if c == '=' {
            return Err(LabelError::InvalidValue {
                key,
                value: value.into(),
                why: "must not contain '='",
            });
        }
        if c.is_ascii_whitespace() {
            return Err(LabelError::InvalidValue {
                key,
                value: value.into(),
                why: "must not contain whitespace",
            });
        }
        if c.is_ascii_control() {
            return Err(LabelError::InvalidValue {
                key,
                value: value.into(),
                why: "must not contain control characters",
            });
        }
        let ok = matches!(
            c,
            'A'..='Z'
                | 'a'..='z'
                | '0'..='9'
                | '.'
                | '_'
                | '-'
                | ':'
                | '@'
                | '/'
                | '+'
        );
        if !ok {
            return Err(LabelError::InvalidValue {
                key,
                value: value.into(),
                why: "contains invalid character",
            });
        }
    }

    if key == X07_LABEL_SCHEMA_KEY && value != X07_LABEL_SCHEMA_VALUE {
        return Err(LabelError::InvalidValue {
            key,
            value: value.into(),
            why: "schema must be '1'",
        });
    }
    if key == X07_LABEL_DEADLINE_UNIX_MS_KEY && !value.bytes().all(|b| (b as char).is_ascii_digit())
    {
        return Err(LabelError::InvalidValue {
            key,
            value: value.into(),
            why: "deadline must be base-10 digits",
        });
    }
    Ok(())
}

fn validate_containerd_kv_size(key: &str, value: &str) -> Result<(), LabelError> {
    let key_len = key.len();
    let value_len = value.len();
    if key_len + value_len > CONTAINERD_KV_MAX_BYTES {
        return Err(LabelError::TooLarge {
            key: key.into(),
            key_len,
            value_len,
            max: CONTAINERD_KV_MAX_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_set_renders_required_keys() {
        let set = X07LabelSet::new("r1", "ri-abc", 123)
            .with_job_id("j1")
            .with_backend("vm.vz")
            .with_created_unix_ms(11);
        let kv = set.render_kv_strings().unwrap();
        assert!(kv.iter().any(|s| s == "io.x07.schema=1"));
        assert!(kv.iter().any(|s| s == "io.x07.run_id=r1"));
        assert!(kv.iter().any(|s| s == "io.x07.runner_instance=ri-abc"));
        assert!(kv.iter().any(|s| s == "io.x07.deadline_unix_ms=123"));
    }

    #[test]
    fn label_value_rejects_whitespace() {
        let set = X07LabelSet::new("r1", "bad value", 123);
        assert!(set.render_kv_strings().is_err());
    }
}
