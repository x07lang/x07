//! Shared world registry helpers.
//!
//! This crate exists so both:
//! - runners (Rust)
//! - toolchain code (Rust)
//!
//! can share an authoritative list of worlds and whether they are allowed in evaluation.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WorldId {
    #[default]
    SolvePure,
    SolveFs,
    SolveRr,
    SolveKv,
    SolveFull,
    RunOs,
    RunOsSandboxed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorldCaps {
    pub allow_os: bool,
    pub allow_unsafe: bool,
    pub allow_ffi: bool,
}

impl WorldId {
    pub fn as_str(self) -> &'static str {
        match self {
            WorldId::SolvePure => "solve-pure",
            WorldId::SolveFs => "solve-fs",
            WorldId::SolveRr => "solve-rr",
            WorldId::SolveKv => "solve-kv",
            WorldId::SolveFull => "solve-full",
            WorldId::RunOs => "run-os",
            WorldId::RunOsSandboxed => "run-os-sandboxed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "solve-pure" => Some(WorldId::SolvePure),
            "solve-fs" => Some(WorldId::SolveFs),
            "solve-rr" => Some(WorldId::SolveRr),
            "solve-kv" => Some(WorldId::SolveKv),
            "solve-full" => Some(WorldId::SolveFull),
            "run-os" => Some(WorldId::RunOs),
            "run-os-sandboxed" => Some(WorldId::RunOsSandboxed),
            _ => None,
        }
    }

    pub fn caps(self) -> WorldCaps {
        match self {
            WorldId::SolvePure
            | WorldId::SolveFs
            | WorldId::SolveRr
            | WorldId::SolveKv
            | WorldId::SolveFull => WorldCaps {
                allow_os: false,
                allow_unsafe: false,
                allow_ffi: false,
            },
            WorldId::RunOs | WorldId::RunOsSandboxed => WorldCaps {
                allow_os: true,
                allow_unsafe: true,
                allow_ffi: true,
            },
        }
    }

    /// True if this world is permitted in deterministic suite runs.
    pub fn is_eval_world(self) -> bool {
        matches!(
            self,
            WorldId::SolvePure
                | WorldId::SolveFs
                | WorldId::SolveRr
                | WorldId::SolveKv
                | WorldId::SolveFull
        )
    }

    /// True if this world is never permitted in deterministic suite runs.
    pub fn is_standalone_only(self) -> bool {
        matches!(self, WorldId::RunOs | WorldId::RunOsSandboxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_worlds_are_not_standalone_only() {
        for &w in &[
            WorldId::SolvePure,
            WorldId::SolveFs,
            WorldId::SolveRr,
            WorldId::SolveKv,
            WorldId::SolveFull,
        ] {
            assert!(w.is_eval_world());
            assert!(!w.is_standalone_only());
        }
    }

    #[test]
    fn standalone_worlds_are_not_eval_worlds() {
        for &w in &[WorldId::RunOs, WorldId::RunOsSandboxed] {
            assert!(!w.is_eval_world());
            assert!(w.is_standalone_only());
        }
    }

    #[test]
    fn caps_are_consistent_with_world_kind() {
        for &w in &[
            WorldId::SolvePure,
            WorldId::SolveFs,
            WorldId::SolveRr,
            WorldId::SolveKv,
            WorldId::SolveFull,
        ] {
            let caps = w.caps();
            assert!(!caps.allow_os);
            assert!(!caps.allow_unsafe);
            assert!(!caps.allow_ffi);
        }

        for &w in &[WorldId::RunOs, WorldId::RunOsSandboxed] {
            let caps = w.caps();
            assert!(caps.allow_os);
            assert!(caps.allow_unsafe);
            assert!(caps.allow_ffi);
        }
    }
}
