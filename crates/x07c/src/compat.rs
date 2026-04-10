use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompatVersion {
    pub major: u32,
    pub minor: u32,
}

impl CompatVersion {
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    pub fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            anyhow::bail!("compat must be non-empty");
        }

        let mut parts = raw.split('.');
        let major = parts
            .next()
            .context("compat major component missing")?
            .trim();
        let minor = parts
            .next()
            .context("compat minor component missing")?
            .trim();
        let patch = parts.next().map(str::trim);
        if parts.next().is_some() {
            anyhow::bail!(
                "compat must be MAJOR.MINOR (or MAJOR.MINOR.PATCH), got {:?}",
                raw
            );
        }

        let major = major
            .parse::<u32>()
            .with_context(|| format!("compat major is not a number: {:?}", major))?;
        let minor = minor
            .parse::<u32>()
            .with_context(|| format!("compat minor is not a number: {:?}", minor))?;

        if let Some(patch) = patch.filter(|p| !p.is_empty()) {
            let _ = patch
                .parse::<u32>()
                .with_context(|| format!("compat patch is not a number: {:?}", patch))?;
        }

        Ok(Self { major, minor })
    }

    pub fn as_str(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Compat {
    pub version: CompatVersion,
    pub strict: bool,
}

impl Compat {
    pub const CURRENT_VERSION: CompatVersion = CompatVersion::new(0, 5);
    pub const CURRENT: Compat = Compat {
        version: Compat::CURRENT_VERSION,
        strict: false,
    };
    pub const STRICT: Compat = Compat {
        version: Compat::CURRENT_VERSION,
        strict: true,
    };

    pub fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            anyhow::bail!("compat must be non-empty");
        }

        match raw {
            "current" => Ok(Self::CURRENT),
            "strict" => Ok(Self::STRICT),
            _ => Ok(Self {
                version: CompatVersion::parse(raw)?,
                strict: false,
            }),
        }
    }

    pub fn to_string_lossy(self) -> String {
        if self.strict {
            "strict".to_string()
        } else {
            self.version.as_str()
        }
    }
}

impl Default for Compat {
    fn default() -> Self {
        Self::CURRENT
    }
}

pub fn resolve_compat(
    cli: Option<&str>,
    env: Option<&str>,
    project: Option<&str>,
) -> Result<Compat> {
    let pick = cli
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| env.map(str::trim).filter(|s| !s.is_empty()))
        .or_else(|| project.map(str::trim).filter(|s| !s.is_empty()));
    match pick {
        Some(raw) => Compat::parse(raw).with_context(|| format!("invalid compat value {:?}", raw)),
        None => Ok(Compat::default()),
    }
}
