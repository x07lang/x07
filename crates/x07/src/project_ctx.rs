use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use x07_runner_common::os_paths;
use x07_worlds::WorldId;
use x07c::project;

#[derive(Debug, Clone)]
pub(crate) struct ProjectCtx {
    pub(crate) base: PathBuf,
    pub(crate) manifest: project::ProjectManifest,
    pub(crate) lock: project::Lockfile,
    pub(crate) lock_path: PathBuf,
    pub(crate) program_path: PathBuf,
    pub(crate) module_roots: Vec<PathBuf>,
    pub(crate) world: WorldId,
}

pub(crate) fn load_project_ctx(project_path: &Path, hydrate_deps: bool) -> Result<ProjectCtx> {
    if hydrate_deps {
        let hydrated = crate::pkg::ensure_project_deps_hydrated_quiet(
            project_path.to_path_buf(),
            crate::util::x07_offline_enabled(),
        )
        .context("hydrate project deps")?;
        if hydrated {
            eprintln!(
                "x07: hydrated project dependencies via `x07 pkg lock --project {}`",
                project_path.display()
            );
        }
    }

    let manifest = project::load_project_manifest(project_path).context("load project manifest")?;
    let lock_path = project::default_lockfile_path(project_path, &manifest);
    let lock_bytes = std::fs::read(&lock_path)
        .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
    let lock: project::Lockfile = serde_json::from_slice(&lock_bytes)
        .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;

    project::verify_lockfile(project_path, &manifest, &lock).context("verify lockfile")?;

    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let program_path = base.join(&manifest.entry);

    let module_roots =
        project::collect_module_roots(project_path, &manifest, &lock).context("module roots")?;
    let world = x07c::world_config::parse_world_id(&manifest.world)
        .with_context(|| format!("invalid project world {:?}", manifest.world))?;

    let mut module_roots = module_roots;
    if matches!(world, WorldId::RunOs | WorldId::RunOsSandboxed) {
        for root in os_paths::default_os_module_roots_best_effort_from_exe(
            std::env::current_exe().ok().as_deref(),
        ) {
            if !module_roots.contains(&root) {
                module_roots.push(root);
            }
        }
    }

    Ok(ProjectCtx {
        base,
        manifest,
        lock,
        lock_path,
        program_path,
        module_roots,
        world,
    })
}
