use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

const SERVICE_ARCHETYPES_JSON_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalog/service_archetypes.json"
));

pub const SERVICE_MANIFEST_SCHEMA_VERSION: &str = "x07.service.manifest@0.1.0";
pub const SERVICE_ARCHETYPES_SCHEMA_VERSION: &str = "x07.service.archetypes@0.1.0";

#[derive(Debug, Clone, Args)]
pub struct ServiceArgs {
    #[command(subcommand)]
    pub cmd: Option<ServiceCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ServiceCommand {
    /// List the built-in service archetypes and their package defaults.
    Archetypes(ServiceArchetypesArgs),
    /// Emit archetype-aware constrained generation surfaces.
    Genpack(crate::service_genpack::ServiceGenpackArgs),
    /// Validate an x07 service manifest.
    Validate(crate::service_validate::ServiceValidateArgs),
}

#[derive(Debug, Clone, Args, Default)]
pub struct ServiceArchetypesArgs {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CellKind {
    ApiCell,
    EventConsumer,
    ScheduledJob,
    PolicyService,
    WorkflowService,
    McpTool,
}

impl CellKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CellKind::ApiCell => "api-cell",
            CellKind::EventConsumer => "event-consumer",
            CellKind::ScheduledJob => "scheduled-job",
            CellKind::PolicyService => "policy-service",
            CellKind::WorkflowService => "workflow-service",
            CellKind::McpTool => "mcp-tool",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IngressKind {
    Http,
    Event,
    Schedule,
    Workflow,
    Mcp,
}

impl IngressKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            IngressKind::Http => "http",
            IngressKind::Event => "event",
            IngressKind::Schedule => "schedule",
            IngressKind::Workflow => "workflow",
            IngressKind::Mcp => "mcp",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeClass {
    WasmComponent,
    NativeHttp,
    NativeWorker,
    EmbeddedKernel,
}

impl RuntimeClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuntimeClass::WasmComponent => "wasm-component",
            RuntimeClass::NativeHttp => "native-http",
            RuntimeClass::NativeWorker => "native-worker",
            RuntimeClass::EmbeddedKernel => "embedded-kernel",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ScaleClass {
    ReplicatedHttp,
    PartitionedConsumer,
    SingletonOrchestrator,
    LeasedWorker,
    BurstBatch,
    EmbeddedKernel,
}

impl ScaleClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScaleClass::ReplicatedHttp => "replicated-http",
            ScaleClass::PartitionedConsumer => "partitioned-consumer",
            ScaleClass::SingletonOrchestrator => "singleton-orchestrator",
            ScaleClass::LeasedWorker => "leased-worker",
            ScaleClass::BurstBatch => "burst-batch",
            ScaleClass::EmbeddedKernel => "embedded-kernel",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BindingKind {
    Postgres,
    Mysql,
    Sqlite,
    Redis,
    Kafka,
    Amqp,
    S3,
    Secret,
    Otlp,
}

impl BindingKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BindingKind::Postgres => "postgres",
            BindingKind::Mysql => "mysql",
            BindingKind::Sqlite => "sqlite",
            BindingKind::Redis => "redis",
            BindingKind::Kafka => "kafka",
            BindingKind::Amqp => "amqp",
            BindingKind::S3 => "s3",
            BindingKind::Secret => "secret",
            BindingKind::Otlp => "otlp",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBindingDecl {
    pub name: String,
    pub kind: BindingKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationalCell {
    pub cell_key: String,
    pub cell_kind: CellKind,
    pub entry_symbol: String,
    pub ingress_kind: IngressKind,
    pub runtime_class: RuntimeClass,
    pub scale_class: ScaleClass,
    #[serde(default)]
    pub binding_refs: Vec<String>,
    pub topology_group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopologyProfile {
    pub id: String,
    #[serde(default)]
    pub target_kind: Option<String>,
    pub placement: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRef {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceManifest {
    pub schema_version: String,
    pub service_id: String,
    pub display_name: String,
    pub domain_pack: DomainPackRef,
    pub cells: Vec<OperationalCell>,
    #[serde(default)]
    pub topology_profiles: Vec<TopologyProfile>,
    #[serde(default)]
    pub resource_bindings: Vec<ResourceBindingDecl>,
    #[serde(default)]
    pub default_trust_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicePackageRef {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceArchetype {
    pub id: String,
    pub summary: String,
    pub cell_kind: CellKind,
    pub ingress_kind: IngressKind,
    pub runtime_class: RuntimeClass,
    pub scale_class: ScaleClass,
    pub example_path: String,
    #[serde(default)]
    pub default_capabilities: Vec<String>,
    #[serde(default)]
    pub default_packages: Vec<ServicePackageRef>,
    #[serde(default)]
    pub supported_bindings: Vec<BindingKind>,
    #[serde(default)]
    pub scaffold_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceArchetypesCatalog {
    pub schema_version: String,
    pub archetypes: Vec<ServiceArchetype>,
}

pub fn cmd_service(
    machine: &crate::reporting::MachineArgs,
    args: ServiceArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing service subcommand (try --help)");
    };
    match cmd {
        ServiceCommand::Archetypes(_args) => cmd_service_archetypes(machine),
        ServiceCommand::Genpack(args) => crate::service_genpack::cmd_service_genpack(machine, args),
        ServiceCommand::Validate(args) => {
            crate::service_validate::cmd_service_validate(machine, args)
        }
    }
}

fn cmd_service_archetypes(
    machine: &crate::reporting::MachineArgs,
) -> Result<std::process::ExitCode> {
    let catalog = load_service_archetypes_catalog()?;
    let out = serde_json::to_string_pretty(&catalog)? + "\n";
    if let Some(path) = machine.out.as_deref() {
        crate::reporting::write_bytes(path, out.as_bytes())?;
    } else {
        print!("{out}");
    }
    Ok(std::process::ExitCode::SUCCESS)
}

pub fn load_service_archetypes_catalog() -> Result<ServiceArchetypesCatalog> {
    let catalog: ServiceArchetypesCatalog = serde_json::from_slice(SERVICE_ARCHETYPES_JSON_BYTES)
        .context("parse catalog/service_archetypes.json")?;
    validate_service_archetypes_catalog(&catalog)?;
    Ok(catalog)
}

pub fn service_archetype_by_id<'a>(
    catalog: &'a ServiceArchetypesCatalog,
    archetype_id: &str,
) -> Option<&'a ServiceArchetype> {
    catalog
        .archetypes
        .iter()
        .find(|archetype| archetype.id == archetype_id)
}

pub fn archetype_for_manifest<'a>(
    catalog: &'a ServiceArchetypesCatalog,
    manifest: &ServiceManifest,
) -> Option<&'a ServiceArchetype> {
    if manifest.cells.len() != 1 {
        return None;
    }
    let cell = &manifest.cells[0];
    catalog.archetypes.iter().find(|archetype| {
        archetype.cell_kind == cell.cell_kind
            && archetype.ingress_kind == cell.ingress_kind
            && archetype.scale_class == cell.scale_class
    })
}

pub fn load_service_manifest(path: &Path) -> Result<ServiceManifest> {
    let raw =
        fs::read(path).with_context(|| format!("read service manifest: {}", path.display()))?;
    let manifest: ServiceManifest = serde_json::from_slice(&raw)
        .with_context(|| format!("parse service manifest: {}", path.display()))?;
    validate_service_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest_against_archetype(
    manifest: &ServiceManifest,
    archetype: &ServiceArchetype,
) -> Result<()> {
    let Some(cell) = manifest.cells.first() else {
        anyhow::bail!("service manifest must define at least one cell");
    };
    if cell.cell_kind != archetype.cell_kind {
        anyhow::bail!(
            "service archetype {:?} expects cell_kind {}, got {}",
            archetype.id,
            archetype.cell_kind.as_str(),
            cell.cell_kind.as_str()
        );
    }
    if cell.ingress_kind != archetype.ingress_kind {
        anyhow::bail!(
            "service archetype {:?} expects ingress_kind {}, got {}",
            archetype.id,
            archetype.ingress_kind.as_str(),
            cell.ingress_kind.as_str()
        );
    }
    if !archetype.supported_bindings.is_empty()
        && manifest.resource_bindings.iter().any(|binding| {
            !archetype
                .supported_bindings
                .iter()
                .any(|supported| supported == &binding.kind)
        })
    {
        anyhow::bail!(
            "service archetype {:?} only supports binding kinds: {}",
            archetype.id,
            archetype
                .supported_bindings
                .iter()
                .map(BindingKind::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

pub fn validate_service_manifest(manifest: &ServiceManifest) -> Result<()> {
    if manifest.schema_version != SERVICE_MANIFEST_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported service manifest schema_version: expected {}, got {}",
            SERVICE_MANIFEST_SCHEMA_VERSION,
            manifest.schema_version
        );
    }
    if manifest.service_id.trim().is_empty() {
        anyhow::bail!("service_id must not be empty");
    }
    if manifest.display_name.trim().is_empty() {
        anyhow::bail!("display_name must not be empty");
    }
    if manifest.domain_pack.id.trim().is_empty() {
        anyhow::bail!("domain_pack.id must not be empty");
    }
    if manifest.domain_pack.display_name.trim().is_empty() {
        anyhow::bail!("domain_pack.display_name must not be empty");
    }
    if manifest.cells.is_empty() {
        anyhow::bail!("service manifest must define at least one cell");
    }

    let mut cell_keys = BTreeSet::new();
    let mut binding_names = BTreeMap::new();
    let mut topology_ids = BTreeSet::new();

    for binding in &manifest.resource_bindings {
        if binding.name.trim().is_empty() {
            anyhow::bail!("resource binding name must not be empty");
        }
        if binding_names
            .insert(binding.name.clone(), binding.kind.as_str().to_string())
            .is_some()
        {
            anyhow::bail!("duplicate resource binding name {:?}", binding.name);
        }
        if let Some(notes) = binding.notes.as_deref() {
            if notes.trim().is_empty() {
                anyhow::bail!("resource binding notes must not be empty when provided");
            }
        }
    }

    for profile in &manifest.topology_profiles {
        if profile.id.trim().is_empty() {
            anyhow::bail!("topology profile id must not be empty");
        }
        if !topology_ids.insert(profile.id.clone()) {
            anyhow::bail!("duplicate topology profile id {:?}", profile.id);
        }
        if profile.placement.trim().is_empty() {
            anyhow::bail!("topology profile placement must not be empty");
        }
    }

    for cell in &manifest.cells {
        if cell.cell_key.trim().is_empty() {
            anyhow::bail!("cell_key must not be empty");
        }
        if !cell_keys.insert(cell.cell_key.clone()) {
            anyhow::bail!("duplicate cell_key {:?}", cell.cell_key);
        }
        if cell.entry_symbol.trim().is_empty() {
            anyhow::bail!("entry_symbol must not be empty");
        }
        if cell.topology_group.trim().is_empty() {
            anyhow::bail!("topology_group must not be empty");
        }
        if cell.ingress_kind != expected_ingress_kind(&cell.cell_kind) {
            anyhow::bail!(
                "cell {:?} has ingress_kind {} but {} expects {}",
                cell.cell_key,
                cell.ingress_kind.as_str(),
                cell.cell_kind.as_str(),
                expected_ingress_kind(&cell.cell_kind).as_str()
            );
        }

        let mut seen_binding_refs = BTreeSet::new();
        for binding_ref in &cell.binding_refs {
            if binding_ref.trim().is_empty() {
                anyhow::bail!("binding_refs entries must not be empty");
            }
            if !seen_binding_refs.insert(binding_ref.clone()) {
                anyhow::bail!(
                    "cell {:?} references duplicate binding {:?}",
                    cell.cell_key,
                    binding_ref
                );
            }
            if !binding_names.contains_key(binding_ref) {
                anyhow::bail!(
                    "cell {:?} references unknown binding {:?}",
                    cell.cell_key,
                    binding_ref
                );
            }
        }
    }

    if let Some(default_trust_profile) = manifest.default_trust_profile.as_deref() {
        if default_trust_profile.trim().is_empty() {
            anyhow::bail!("default_trust_profile must not be empty when provided");
        }
    }

    Ok(())
}

fn validate_service_archetypes_catalog(catalog: &ServiceArchetypesCatalog) -> Result<()> {
    if catalog.schema_version != SERVICE_ARCHETYPES_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported service archetypes schema_version: expected {}, got {}",
            SERVICE_ARCHETYPES_SCHEMA_VERSION,
            catalog.schema_version
        );
    }
    let mut ids = BTreeSet::new();
    for archetype in &catalog.archetypes {
        if archetype.id.trim().is_empty() {
            anyhow::bail!("service archetype id must not be empty");
        }
        if !ids.insert(archetype.id.clone()) {
            anyhow::bail!("duplicate service archetype id {:?}", archetype.id);
        }
        if archetype.summary.trim().is_empty() {
            anyhow::bail!("service archetype summary must not be empty");
        }
        if archetype.example_path.trim().is_empty() {
            anyhow::bail!("service archetype example_path must not be empty");
        }
        let mut pkg_names = BTreeSet::new();
        for pkg in &archetype.default_packages {
            if pkg.name.trim().is_empty() || pkg.version.trim().is_empty() {
                anyhow::bail!(
                    "service archetype {:?} contains an invalid package default",
                    archetype.id
                );
            }
            if !pkg_names.insert(pkg.name.clone()) {
                anyhow::bail!(
                    "service archetype {:?} contains duplicate package default {:?}",
                    archetype.id,
                    pkg.name
                );
            }
        }
    }
    Ok(())
}

fn expected_ingress_kind(cell_kind: &CellKind) -> IngressKind {
    match cell_kind {
        CellKind::ApiCell | CellKind::PolicyService => IngressKind::Http,
        CellKind::EventConsumer => IngressKind::Event,
        CellKind::ScheduledJob => IngressKind::Schedule,
        CellKind::WorkflowService => IngressKind::Workflow,
        CellKind::McpTool => IngressKind::Mcp,
    }
}
