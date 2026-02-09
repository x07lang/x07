use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;
use serde::Serialize;
use serde_json::Value;
use x07_contracts::X07_CONTRACT_REPRO_SCHEMA_VERSION;
use x07_host_runner::RunnerConfig;

use crate::repro::ToolInfo;

const X07_CONTRACT_REPRO_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.contract.repro@0.1.0.schema.json");

const CONTRACT_TRAP_PREFIX: &str = "X07T_CONTRACT_V1 ";

#[derive(Debug, Clone)]
pub(crate) struct ContractTrapInfo {
    pub(crate) clause_id: String,
    pub(crate) payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SourceInfo {
    pub(crate) mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tests_manifest_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) test_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) test_entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RunnerInfo {
    pub(crate) solve_fuel: u64,
    pub(crate) max_memory_bytes: u64,
    pub(crate) max_output_bytes: u64,
    pub(crate) cpu_time_limit_seconds: u64,
    pub(crate) debug_borrow_checks: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fixture_fs_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fixture_fs_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fixture_fs_latency_index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fixture_rr_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fixture_kv_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fixture_kv_seed: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContractRepro {
    pub(crate) schema_version: String,
    pub(crate) tool: ToolInfo,
    pub(crate) source: SourceInfo,
    pub(crate) world: String,
    pub(crate) runner: RunnerInfo,
    pub(crate) input_bytes_b64: String,
    pub(crate) contract: Value,
}

pub(crate) fn try_parse_contract_trap(trap: &str) -> Result<Option<ContractTrapInfo>> {
    if !trap.starts_with(CONTRACT_TRAP_PREFIX) {
        return Ok(None);
    }

    let payload_json = &trap[CONTRACT_TRAP_PREFIX.len()..];
    let payload: Value = serde_json::from_str(payload_json).context("parse contract trap JSON")?;
    let clause_id = payload
        .get("clause_id")
        .and_then(Value::as_str)
        .context("contract trap JSON missing clause_id")?
        .to_string();

    Ok(Some(ContractTrapInfo { clause_id, payload }))
}

pub(crate) fn runner_info(cfg: &RunnerConfig) -> RunnerInfo {
    RunnerInfo {
        solve_fuel: cfg.solve_fuel,
        max_memory_bytes: cfg.max_memory_bytes as u64,
        max_output_bytes: cfg.max_output_bytes as u64,
        cpu_time_limit_seconds: cfg.cpu_time_limit_seconds,
        debug_borrow_checks: cfg.debug_borrow_checks,
        fixture_fs_dir: cfg.fixture_fs_dir.as_ref().map(display_path),
        fixture_fs_root: cfg.fixture_fs_root.as_ref().map(display_path),
        fixture_fs_latency_index: cfg.fixture_fs_latency_index.as_ref().map(display_path),
        fixture_rr_dir: cfg.fixture_rr_dir.as_ref().map(display_path),
        fixture_kv_dir: cfg.fixture_kv_dir.as_ref().map(display_path),
        fixture_kv_seed: cfg.fixture_kv_seed.as_ref().map(display_path),
    }
}

pub(crate) fn repro_to_pretty_canon_bytes(repro: &ContractRepro) -> Result<Vec<u8>> {
    let v = serde_json::to_value(repro).context("serialize contract repro JSON")?;
    let diags = crate::report_common::validate_schema(
        X07_CONTRACT_REPRO_SCHEMA_BYTES,
        "spec/x07.contract.repro@0.1.0.schema.json",
        &v,
    )?;
    if !diags.is_empty() {
        anyhow::bail!(
            "internal error: contract repro JSON is not schema-valid: {}",
            diags[0].message
        );
    }
    crate::report_common::canonical_pretty_json_bytes(&v).context("canon contract repro JSON")
}

pub(crate) fn write_repro(
    artifact_dir: &Path,
    world: &str,
    runner: &RunnerConfig,
    input: &[u8],
    contract_payload: Value,
    source: SourceInfo,
    clause_id: &str,
) -> Result<PathBuf> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let out_dir = artifact_dir
        .join("contract")
        .join(crate::util::safe_artifact_dir_name(clause_id));
    let out_path = out_dir.join("repro.json");

    let repro = ContractRepro {
        schema_version: X07_CONTRACT_REPRO_SCHEMA_VERSION.to_string(),
        tool: crate::repro::tool_info(),
        source,
        world: world.to_string(),
        runner: runner_info(runner),
        input_bytes_b64: b64.encode(input),
        contract: contract_payload,
    };

    crate::util::write_atomic(&out_path, &repro_to_pretty_canon_bytes(&repro)?)
        .with_context(|| format!("write contract repro: {}", out_path.display()))?;
    Ok(out_path)
}

fn display_path<P: AsRef<Path>>(p: P) -> String {
    p.as_ref().display().to_string()
}
