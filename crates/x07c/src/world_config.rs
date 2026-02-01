use std::path::PathBuf;

use anyhow::Result;
use x07_worlds::WorldId;

use crate::{compile, lint};

#[derive(Debug, Clone, Copy)]
pub struct WorldFeatures {
    pub enable_fs: bool,
    pub enable_rr: bool,
    pub enable_kv: bool,
    pub allow_unsafe: Option<bool>,
    pub allow_ffi: Option<bool>,
}

pub fn parse_world_id(raw: &str) -> Result<WorldId> {
    WorldId::parse(raw).ok_or_else(|| anyhow::anyhow!("unknown world: {raw}"))
}

pub fn features_for_world(world: WorldId) -> WorldFeatures {
    match world {
        WorldId::SolvePure => WorldFeatures {
            enable_fs: false,
            enable_rr: false,
            enable_kv: false,
            allow_unsafe: None,
            allow_ffi: None,
        },
        WorldId::SolveFs => WorldFeatures {
            enable_fs: true,
            enable_rr: false,
            enable_kv: false,
            allow_unsafe: None,
            allow_ffi: None,
        },
        WorldId::SolveRr => WorldFeatures {
            enable_fs: false,
            enable_rr: true,
            enable_kv: false,
            allow_unsafe: None,
            allow_ffi: None,
        },
        WorldId::SolveKv => WorldFeatures {
            enable_fs: false,
            enable_rr: false,
            enable_kv: true,
            allow_unsafe: None,
            allow_ffi: None,
        },
        WorldId::SolveFull => WorldFeatures {
            enable_fs: true,
            enable_rr: true,
            enable_kv: true,
            allow_unsafe: None,
            allow_ffi: None,
        },
        WorldId::RunOs => WorldFeatures {
            enable_fs: true,
            enable_rr: true,
            enable_kv: false,
            allow_unsafe: None,
            allow_ffi: None,
        },
        WorldId::RunOsSandboxed => WorldFeatures {
            enable_fs: true,
            enable_rr: true,
            enable_kv: false,
            allow_unsafe: Some(false),
            allow_ffi: Some(false),
        },
    }
}

pub fn compile_options_for_world(
    world: WorldId,
    module_roots: Vec<PathBuf>,
) -> compile::CompileOptions {
    let features = features_for_world(world);
    compile::CompileOptions {
        world,
        enable_fs: features.enable_fs,
        enable_rr: features.enable_rr,
        enable_kv: features.enable_kv,
        module_roots,
        arch_root: None,
        emit_main: true,
        freestanding: false,
        allow_unsafe: features.allow_unsafe,
        allow_ffi: features.allow_ffi,
    }
}

pub fn lint_options_for_world(world: WorldId) -> lint::LintOptions {
    let features = features_for_world(world);
    lint::LintOptions {
        world,
        enable_fs: features.enable_fs,
        enable_rr: features.enable_rr,
        enable_kv: features.enable_kv,
        allow_unsafe: features.allow_unsafe,
        allow_ffi: features.allow_ffi,
    }
}
