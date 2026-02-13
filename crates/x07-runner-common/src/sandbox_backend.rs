use std::fmt;
use std::str::FromStr;

use anyhow::Context;
use x07_worlds::WorldId;

pub const ENV_SANDBOX_BACKEND: &str = "X07_SANDBOX_BACKEND";
pub const ENV_ACCEPT_WEAKER_ISOLATION: &str = "X07_I_ACCEPT_WEAKER_ISOLATION";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SandboxBackend {
    Auto,
    Vm,
    Os,
    None,
}

impl SandboxBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            SandboxBackend::Auto => "auto",
            SandboxBackend::Vm => "vm",
            SandboxBackend::Os => "os",
            SandboxBackend::None => "none",
        }
    }
}

impl fmt::Display for SandboxBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct SandboxBackendParseError {
    value: String,
}

impl fmt::Display for SandboxBackendParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid sandbox backend {:?} (expected one of: auto, vm, os, none)",
            self.value
        )
    }
}

impl std::error::Error for SandboxBackendParseError {}

impl FromStr for SandboxBackend {
    type Err = SandboxBackendParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let s = s.to_ascii_lowercase();
        match s.as_str() {
            "auto" => Ok(SandboxBackend::Auto),
            "vm" => Ok(SandboxBackend::Vm),
            "os" => Ok(SandboxBackend::Os),
            "native" => Ok(SandboxBackend::Os),
            "none" => Ok(SandboxBackend::None),
            _ => Err(SandboxBackendParseError { value: s }),
        }
    }
}

#[cfg(feature = "clap")]
impl clap::ValueEnum for SandboxBackend {
    fn value_variants<'a>() -> &'a [Self] {
        const ALL: [SandboxBackend; 4] = [
            SandboxBackend::Auto,
            SandboxBackend::Vm,
            SandboxBackend::Os,
            SandboxBackend::None,
        ];
        &ALL
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            SandboxBackend::Auto => Some(clap::builder::PossibleValue::new("auto")),
            SandboxBackend::Vm => Some(clap::builder::PossibleValue::new("vm")),
            SandboxBackend::Os => Some(clap::builder::PossibleValue::new("os").alias("native")),
            SandboxBackend::None => Some(clap::builder::PossibleValue::new("none")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectiveSandboxBackend {
    Vm,
    Os,
    None,
}

impl EffectiveSandboxBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            EffectiveSandboxBackend::Vm => "vm",
            EffectiveSandboxBackend::Os => "os",
            EffectiveSandboxBackend::None => "none",
        }
    }

    pub fn is_vm(self) -> bool {
        matches!(self, EffectiveSandboxBackend::Vm)
    }
}

impl fmt::Display for EffectiveSandboxBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn parse_bool_env(name: &str, raw: &str) -> anyhow::Result<bool> {
    match raw.trim() {
        "1" | "true" | "TRUE" | "yes" | "YES" => Ok(true),
        "0" | "false" | "FALSE" | "no" | "NO" => Ok(false),
        other => anyhow::bail!(
            "invalid environment variable {name}={other:?} (expected one of: 1, 0, true, false, yes, no)"
        ),
    }
}

fn read_sandbox_backend_env() -> anyhow::Result<Option<SandboxBackend>> {
    let Ok(raw) = std::env::var(ENV_SANDBOX_BACKEND) else {
        return Ok(None);
    };
    let backend = SandboxBackend::from_str(&raw)
        .with_context(|| format!("invalid environment variable {ENV_SANDBOX_BACKEND}={raw:?}"))?;
    Ok(Some(backend))
}

fn read_accept_weaker_isolation_env() -> anyhow::Result<Option<bool>> {
    let Ok(raw) = std::env::var(ENV_ACCEPT_WEAKER_ISOLATION) else {
        return Ok(None);
    };
    Ok(Some(parse_bool_env(ENV_ACCEPT_WEAKER_ISOLATION, &raw)?))
}

fn default_backend_for_world(world: WorldId) -> EffectiveSandboxBackend {
    match world {
        WorldId::RunOsSandboxed => EffectiveSandboxBackend::Vm,
        WorldId::RunOs => EffectiveSandboxBackend::None,
        _ => EffectiveSandboxBackend::None,
    }
}

fn resolve_sandbox_backend_with_env(
    world: WorldId,
    cli_backend: Option<SandboxBackend>,
    cli_accept_weaker_isolation: bool,
    env_backend: Option<SandboxBackend>,
    env_accept_weaker_isolation: Option<bool>,
) -> anyhow::Result<EffectiveSandboxBackend> {
    let accept_weaker_isolation =
        cli_accept_weaker_isolation || env_accept_weaker_isolation.unwrap_or(false);

    let requested = match cli_backend {
        Some(v) => v,
        None => env_backend.unwrap_or(SandboxBackend::Auto),
    };

    let effective = match requested {
        SandboxBackend::Auto => default_backend_for_world(world),
        SandboxBackend::Vm => EffectiveSandboxBackend::Vm,
        SandboxBackend::Os => EffectiveSandboxBackend::Os,
        SandboxBackend::None => EffectiveSandboxBackend::None,
    };

    if world == WorldId::RunOsSandboxed && !effective.is_vm() && !accept_weaker_isolation {
        anyhow::bail!(
            "run-os-sandboxed defaults to sandbox_backend=vm and fails closed; effective sandbox backend is {effective:?}\n\n\
fix:\n  - use a VM backend: --sandbox-backend=vm (default), or\n  - explicitly accept weaker isolation: --i-accept-weaker-isolation (or set {ENV_ACCEPT_WEAKER_ISOLATION}=1)"
        );
    }

    Ok(effective)
}

pub fn resolve_sandbox_backend(
    world: WorldId,
    cli_backend: Option<SandboxBackend>,
    cli_accept_weaker_isolation: bool,
) -> anyhow::Result<EffectiveSandboxBackend> {
    let env_backend = read_sandbox_backend_env()?;
    let env_accept_weaker_isolation = read_accept_weaker_isolation_env()?;
    resolve_sandbox_backend_with_env(
        world,
        cli_backend,
        cli_accept_weaker_isolation,
        env_backend,
        env_accept_weaker_isolation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_backend_from_str() {
        assert_eq!(
            SandboxBackend::from_str("auto").unwrap(),
            SandboxBackend::Auto
        );
        assert_eq!(SandboxBackend::from_str("vm").unwrap(), SandboxBackend::Vm);
        assert_eq!(SandboxBackend::from_str("os").unwrap(), SandboxBackend::Os);
        assert_eq!(
            SandboxBackend::from_str("native").unwrap(),
            SandboxBackend::Os
        );
        assert_eq!(
            SandboxBackend::from_str("none").unwrap(),
            SandboxBackend::None
        );
        assert!(SandboxBackend::from_str("wat").is_err());
    }

    #[test]
    fn resolve_defaults_for_world() {
        let backend =
            resolve_sandbox_backend_with_env(WorldId::RunOsSandboxed, None, false, None, None)
                .unwrap();
        assert_eq!(backend, EffectiveSandboxBackend::Vm);

        let backend =
            resolve_sandbox_backend_with_env(WorldId::RunOs, None, false, None, None).unwrap();
        assert_eq!(backend, EffectiveSandboxBackend::None);
    }

    #[test]
    fn resolve_run_os_sandboxed_requires_acceptance_for_weaker_modes() {
        let err = resolve_sandbox_backend_with_env(
            WorldId::RunOsSandboxed,
            Some(SandboxBackend::Os),
            false,
            None,
            None,
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("fails closed"));

        let backend = resolve_sandbox_backend_with_env(
            WorldId::RunOsSandboxed,
            Some(SandboxBackend::Os),
            true,
            None,
            None,
        )
        .unwrap();
        assert_eq!(backend, EffectiveSandboxBackend::Os);
    }
}
