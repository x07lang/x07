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
pub enum ProbeKind {
    Http,
    Exec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EventAckMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RolloutStrategy {
    Rolling,
    CanaryLite,
    Recreate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ScheduleConcurrencyPolicy {
    Allow,
    Forbid,
    Replace,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellRuntime {
    #[serde(default)]
    pub event: Option<CellEventRuntime>,
    #[serde(default)]
    pub schedule: Option<CellScheduleRuntime>,
    #[serde(default)]
    pub probes: Option<CellProbeSet>,
    #[serde(default)]
    pub rollout: Option<CellRollout>,
    #[serde(default)]
    pub autoscaling: Option<CellAutoscaling>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellEventRuntime {
    pub binding_ref: String,
    pub topic: String,
    pub consumer_group: String,
    #[serde(default)]
    pub ack_mode: Option<EventAckMode>,
    #[serde(default)]
    pub max_in_flight: Option<u32>,
    #[serde(default)]
    pub drain_timeout_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellScheduleRuntime {
    pub cron: String,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub concurrency_policy: Option<ScheduleConcurrencyPolicy>,
    #[serde(default)]
    pub retry_limit: Option<u32>,
    #[serde(default)]
    pub start_deadline_seconds: Option<u32>,
    #[serde(default)]
    pub suspend: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellProbeSet {
    #[serde(default)]
    pub startup: Option<CellProbe>,
    #[serde(default)]
    pub readiness: Option<CellProbe>,
    #[serde(default)]
    pub liveness: Option<CellProbe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellProbe {
    pub probe_kind: ProbeKind,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub initial_delay_seconds: Option<u32>,
    #[serde(default)]
    pub period_seconds: Option<u32>,
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
    #[serde(default)]
    pub success_threshold: Option<u32>,
    #[serde(default)]
    pub failure_threshold: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellRollout {
    pub strategy: RolloutStrategy,
    #[serde(default)]
    pub max_unavailable: Option<String>,
    #[serde(default)]
    pub max_surge: Option<String>,
    #[serde(default)]
    pub canary_percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellAutoscaling {
    pub min_replicas: u32,
    pub max_replicas: u32,
    #[serde(default)]
    pub target_cpu_utilization: Option<u8>,
    #[serde(default)]
    pub target_inflight: Option<u32>,
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
    #[serde(default)]
    pub runtime: CellRuntime,
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
        validate_cell_runtime(cell, &binding_names)?;
    }

    if let Some(default_trust_profile) = manifest.default_trust_profile.as_deref() {
        if default_trust_profile.trim().is_empty() {
            anyhow::bail!("default_trust_profile must not be empty when provided");
        }
    }

    Ok(())
}

fn validate_cell_runtime(
    cell: &OperationalCell,
    binding_names: &BTreeMap<String, String>,
) -> Result<()> {
    match cell.ingress_kind {
        IngressKind::Event => {
            let event = cell.runtime.event.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "cell {:?} requires runtime.event for event ingress",
                    cell.cell_key
                )
            })?;
            validate_event_runtime(cell, event, binding_names)?;
        }
        _ => {
            if cell.runtime.event.is_some() {
                anyhow::bail!(
                    "cell {:?} must not declare runtime.event unless ingress_kind is event",
                    cell.cell_key
                );
            }
        }
    }

    match cell.ingress_kind {
        IngressKind::Schedule => {
            let schedule = cell.runtime.schedule.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "cell {:?} requires runtime.schedule for schedule ingress",
                    cell.cell_key
                )
            })?;
            validate_schedule_runtime(cell, schedule)?;
        }
        _ => {
            if cell.runtime.schedule.is_some() {
                anyhow::bail!(
                    "cell {:?} must not declare runtime.schedule unless ingress_kind is schedule",
                    cell.cell_key
                );
            }
        }
    }

    if let Some(probes) = cell.runtime.probes.as_ref() {
        for (label, probe) in [
            ("startup", probes.startup.as_ref()),
            ("readiness", probes.readiness.as_ref()),
            ("liveness", probes.liveness.as_ref()),
        ] {
            if let Some(probe) = probe {
                validate_probe(cell, label, probe)?;
            }
        }
    }

    if let Some(rollout) = cell.runtime.rollout.as_ref() {
        validate_rollout(cell, rollout)?;
    }

    if let Some(autoscaling) = cell.runtime.autoscaling.as_ref() {
        validate_autoscaling(cell, autoscaling)?;
    }

    Ok(())
}

fn validate_event_runtime(
    cell: &OperationalCell,
    event: &CellEventRuntime,
    binding_names: &BTreeMap<String, String>,
) -> Result<()> {
    if event.binding_ref.trim().is_empty() {
        anyhow::bail!(
            "cell {:?} runtime.event.binding_ref must not be empty",
            cell.cell_key
        );
    }
    if !cell
        .binding_refs
        .iter()
        .any(|binding_ref| binding_ref == &event.binding_ref)
    {
        anyhow::bail!(
            "cell {:?} runtime.event.binding_ref {:?} must appear in binding_refs",
            cell.cell_key,
            event.binding_ref
        );
    }
    match binding_names.get(&event.binding_ref).map(String::as_str) {
        Some("amqp") | Some("kafka") => {}
        Some(kind) => anyhow::bail!(
            "cell {:?} runtime.event.binding_ref {:?} must reference an amqp or kafka binding, got {}",
            cell.cell_key,
            event.binding_ref,
            kind
        ),
        None => anyhow::bail!(
            "cell {:?} runtime.event.binding_ref {:?} references an unknown binding",
            cell.cell_key,
            event.binding_ref
        ),
    }
    if event.topic.trim().is_empty() {
        anyhow::bail!(
            "cell {:?} runtime.event.topic must not be empty",
            cell.cell_key
        );
    }
    if event.consumer_group.trim().is_empty() {
        anyhow::bail!(
            "cell {:?} runtime.event.consumer_group must not be empty",
            cell.cell_key
        );
    }
    Ok(())
}

fn validate_schedule_runtime(cell: &OperationalCell, schedule: &CellScheduleRuntime) -> Result<()> {
    if schedule.cron.trim().is_empty() {
        anyhow::bail!(
            "cell {:?} runtime.schedule.cron must not be empty",
            cell.cell_key
        );
    }
    if let Some(timezone) = schedule.timezone.as_deref() {
        if timezone.trim().is_empty() {
            anyhow::bail!(
                "cell {:?} runtime.schedule.timezone must not be empty when provided",
                cell.cell_key
            );
        }
    }
    Ok(())
}

fn validate_probe(cell: &OperationalCell, label: &str, probe: &CellProbe) -> Result<()> {
    match probe.probe_kind {
        ProbeKind::Http => {
            let path = probe.path.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "cell {:?} runtime.probes.{} requires path for http probes",
                    cell.cell_key,
                    label
                )
            })?;
            if path.trim().is_empty() || !path.starts_with('/') {
                anyhow::bail!(
                    "cell {:?} runtime.probes.{} path must start with '/'",
                    cell.cell_key,
                    label
                );
            }
            if !probe.command.is_empty() {
                anyhow::bail!(
                    "cell {:?} runtime.probes.{} must not set command for http probes",
                    cell.cell_key,
                    label
                );
            }
        }
        ProbeKind::Exec => {
            if probe.command.is_empty() || probe.command.iter().any(|part| part.trim().is_empty()) {
                anyhow::bail!(
                    "cell {:?} runtime.probes.{} command must contain non-empty entries for exec probes",
                    cell.cell_key,
                    label
                );
            }
            if probe.path.is_some() || probe.port.is_some() {
                anyhow::bail!(
                    "cell {:?} runtime.probes.{} must not set path or port for exec probes",
                    cell.cell_key,
                    label
                );
            }
        }
    }
    Ok(())
}

fn validate_rollout(cell: &OperationalCell, rollout: &CellRollout) -> Result<()> {
    if rollout.strategy == RolloutStrategy::CanaryLite
        && !matches!(cell.ingress_kind, IngressKind::Http | IngressKind::Event)
    {
        anyhow::bail!(
            "cell {:?} only supports canary-lite rollout for http or event ingress",
            cell.cell_key
        );
    }
    if let Some(value) = rollout.max_unavailable.as_deref() {
        validate_rollout_step_value(cell, "max_unavailable", value)?;
    }
    if let Some(value) = rollout.max_surge.as_deref() {
        validate_rollout_step_value(cell, "max_surge", value)?;
    }
    if let Some(percent) = rollout.canary_percent {
        if percent == 0 || percent > 100 {
            anyhow::bail!(
                "cell {:?} runtime.rollout.canary_percent must be between 1 and 100",
                cell.cell_key
            );
        }
    }
    Ok(())
}

fn validate_rollout_step_value(cell: &OperationalCell, label: &str, value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!(
            "cell {:?} runtime.rollout.{} must not be empty",
            cell.cell_key,
            label
        );
    }
    let valid = if let Some(percent) = trimmed.strip_suffix('%') {
        !percent.is_empty() && percent.chars().all(|ch| ch.is_ascii_digit())
    } else {
        trimmed.chars().all(|ch| ch.is_ascii_digit())
    };
    if !valid {
        anyhow::bail!(
            "cell {:?} runtime.rollout.{} must be a decimal count or percentage string",
            cell.cell_key,
            label
        );
    }
    Ok(())
}

fn validate_autoscaling(cell: &OperationalCell, autoscaling: &CellAutoscaling) -> Result<()> {
    if !matches!(cell.ingress_kind, IngressKind::Http | IngressKind::Event) {
        anyhow::bail!(
            "cell {:?} only supports autoscaling hints for http or event ingress",
            cell.cell_key
        );
    }
    if autoscaling.min_replicas > autoscaling.max_replicas {
        anyhow::bail!(
            "cell {:?} runtime.autoscaling min_replicas must be <= max_replicas",
            cell.cell_key
        );
    }
    if autoscaling.target_cpu_utilization.is_none() && autoscaling.target_inflight.is_none() {
        anyhow::bail!(
            "cell {:?} runtime.autoscaling must define target_cpu_utilization or target_inflight",
            cell.cell_key
        );
    }
    if let Some(target) = autoscaling.target_cpu_utilization {
        if target == 0 || target > 100 {
            anyhow::bail!(
                "cell {:?} runtime.autoscaling target_cpu_utilization must be between 1 and 100",
                cell.cell_key
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> ServiceManifest {
        ServiceManifest {
            schema_version: SERVICE_MANIFEST_SCHEMA_VERSION.to_string(),
            service_id: "orders".to_string(),
            display_name: "Orders".to_string(),
            domain_pack: DomainPackRef {
                id: "orders".to_string(),
                display_name: "Orders".to_string(),
            },
            cells: vec![
                OperationalCell {
                    cell_key: "api".to_string(),
                    cell_kind: CellKind::ApiCell,
                    entry_symbol: "orders.api.main".to_string(),
                    ingress_kind: IngressKind::Http,
                    runtime_class: RuntimeClass::NativeHttp,
                    scale_class: ScaleClass::ReplicatedHttp,
                    binding_refs: vec!["db.primary".to_string()],
                    topology_group: "frontdoor".to_string(),
                    runtime: CellRuntime {
                        probes: Some(CellProbeSet {
                            readiness: Some(CellProbe {
                                probe_kind: ProbeKind::Http,
                                path: Some("/readyz".to_string()),
                                port: Some(8080),
                                command: Vec::new(),
                                initial_delay_seconds: None,
                                period_seconds: Some(5),
                                timeout_seconds: None,
                                success_threshold: None,
                                failure_threshold: Some(3),
                            }),
                            liveness: Some(CellProbe {
                                probe_kind: ProbeKind::Http,
                                path: Some("/livez".to_string()),
                                port: Some(8080),
                                command: Vec::new(),
                                initial_delay_seconds: None,
                                period_seconds: Some(10),
                                timeout_seconds: None,
                                success_threshold: None,
                                failure_threshold: Some(3),
                            }),
                            startup: None,
                        }),
                        rollout: Some(CellRollout {
                            strategy: RolloutStrategy::Rolling,
                            max_unavailable: Some("25%".to_string()),
                            max_surge: Some("25%".to_string()),
                            canary_percent: None,
                        }),
                        autoscaling: Some(CellAutoscaling {
                            min_replicas: 2,
                            max_replicas: 6,
                            target_cpu_utilization: Some(70),
                            target_inflight: None,
                        }),
                        ..CellRuntime::default()
                    },
                },
                OperationalCell {
                    cell_key: "events".to_string(),
                    cell_kind: CellKind::EventConsumer,
                    entry_symbol: "orders.events.main".to_string(),
                    ingress_kind: IngressKind::Event,
                    runtime_class: RuntimeClass::NativeWorker,
                    scale_class: ScaleClass::PartitionedConsumer,
                    binding_refs: vec!["db.primary".to_string(), "msg.orders".to_string()],
                    topology_group: "async".to_string(),
                    runtime: CellRuntime {
                        event: Some(CellEventRuntime {
                            binding_ref: "msg.orders".to_string(),
                            topic: "orders.created".to_string(),
                            consumer_group: "orders-workers".to_string(),
                            ack_mode: Some(EventAckMode::Manual),
                            max_in_flight: Some(32),
                            drain_timeout_seconds: Some(30),
                        }),
                        ..CellRuntime::default()
                    },
                },
                OperationalCell {
                    cell_key: "settlement".to_string(),
                    cell_kind: CellKind::ScheduledJob,
                    entry_symbol: "orders.settlement.main".to_string(),
                    ingress_kind: IngressKind::Schedule,
                    runtime_class: RuntimeClass::NativeWorker,
                    scale_class: ScaleClass::BurstBatch,
                    binding_refs: vec!["db.primary".to_string()],
                    topology_group: "async".to_string(),
                    runtime: CellRuntime {
                        schedule: Some(CellScheduleRuntime {
                            cron: "0 */6 * * *".to_string(),
                            timezone: Some("UTC".to_string()),
                            concurrency_policy: Some(ScheduleConcurrencyPolicy::Forbid),
                            retry_limit: Some(3),
                            start_deadline_seconds: Some(600),
                            suspend: None,
                        }),
                        ..CellRuntime::default()
                    },
                },
            ],
            topology_profiles: vec![TopologyProfile {
                id: "prod".to_string(),
                target_kind: Some("k8s".to_string()),
                placement: "split-by-cell".to_string(),
                notes: None,
            }],
            resource_bindings: vec![
                ResourceBindingDecl {
                    name: "db.primary".to_string(),
                    kind: BindingKind::Postgres,
                    required: true,
                    notes: None,
                },
                ResourceBindingDecl {
                    name: "msg.orders".to_string(),
                    kind: BindingKind::Amqp,
                    required: true,
                    notes: None,
                },
            ],
            default_trust_profile: Some("sandboxed_service_v1".to_string()),
        }
    }

    #[test]
    fn service_manifest_accepts_runtime_hints() {
        validate_service_manifest(&sample_manifest()).expect("manifest should validate");
    }

    #[test]
    fn event_consumer_requires_runtime_event() {
        let mut manifest = sample_manifest();
        manifest.cells[1].runtime.event = None;
        let err =
            validate_service_manifest(&manifest).expect_err("event runtime should be required");
        assert!(err
            .to_string()
            .contains("requires runtime.event for event ingress"));
    }

    #[test]
    fn scheduled_job_requires_runtime_schedule() {
        let mut manifest = sample_manifest();
        manifest.cells[2].runtime.schedule = None;
        let err =
            validate_service_manifest(&manifest).expect_err("schedule runtime should be required");
        assert!(err
            .to_string()
            .contains("requires runtime.schedule for schedule ingress"));
    }
}
