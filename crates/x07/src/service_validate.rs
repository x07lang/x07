use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use crate::service::{
    archetype_for_manifest, load_service_archetypes_catalog, load_service_manifest,
    service_archetype_by_id, validate_manifest_against_archetype,
};

#[derive(Debug, Clone, Args)]
pub struct ServiceValidateArgs {
    #[arg(
        long,
        value_name = "PATH",
        default_value = "arch/service/index.x07service.json"
    )]
    pub manifest: PathBuf,

    #[arg(long, value_name = "ARCHETYPE")]
    pub archetype: Option<String>,
}

#[derive(Debug, Serialize)]
struct ServiceValidateReport {
    ok: bool,
    manifest: String,
    service_id: String,
    cell_count: usize,
    topology_profiles: Vec<String>,
    binding_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archetype: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

pub fn cmd_service_validate(
    machine: &crate::reporting::MachineArgs,
    args: ServiceValidateArgs,
) -> Result<std::process::ExitCode> {
    let manifest = load_service_manifest(&args.manifest)?;
    let catalog = load_service_archetypes_catalog()?;
    let mut warnings = Vec::new();

    let archetype_id = if let Some(id) = args.archetype.as_deref() {
        let archetype = service_archetype_by_id(&catalog, id)
            .with_context(|| format!("unknown service archetype {:?}", id))?;
        validate_manifest_against_archetype(&manifest, archetype)?;
        Some(archetype.id.clone())
    } else if let Some(archetype) = archetype_for_manifest(&catalog, &manifest) {
        validate_manifest_against_archetype(&manifest, archetype)?;
        Some(archetype.id.clone())
    } else {
        warnings.push(
            "service shape does not match a single built-in service archetype; manifest is still structurally valid."
                .to_string(),
        );
        None
    };

    let topology_profiles = manifest
        .topology_profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect::<Vec<_>>();
    let binding_names = manifest
        .resource_bindings
        .iter()
        .map(|binding| binding.name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let report = ServiceValidateReport {
        ok: true,
        manifest: args.manifest.display().to_string(),
        service_id: manifest.service_id,
        cell_count: manifest.cells.len(),
        topology_profiles,
        binding_names,
        archetype: archetype_id,
        warnings,
    };
    let out = serde_json::to_string_pretty(&report)? + "\n";
    if let Some(path) = machine.out.as_deref() {
        crate::reporting::write_bytes(path, out.as_bytes())?;
    } else {
        print!("{out}");
    }
    Ok(std::process::ExitCode::SUCCESS)
}
