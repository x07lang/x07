use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::{report_common, util};

pub(crate) const DEFAULT_EVENTS_DIR: &str = "target/xtal/events";
pub(crate) const ENV_X07_XTAL_EVENTS_DIR: &str = "X07_XTAL_EVENTS_DIR";

pub(crate) const RECOVERY_EVENT_SCHEMA_VERSION: &str = "x07.xtal.recovery_event@0.1.0";
const RECOVERY_EVENT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.xtal.recovery_event@0.1.0.schema.json");

pub(crate) fn resolve_events_root_dir(project_root: &Path) -> Option<PathBuf> {
    if let Ok(raw) = std::env::var(ENV_X07_XTAL_EVENTS_DIR) {
        let raw = raw.trim();
        if !raw.is_empty() {
            let p = PathBuf::from(raw);
            return Some(if p.is_absolute() {
                p
            } else {
                project_root.join(p)
            });
        }
    }

    if project_root
        .join("arch")
        .join("xtal")
        .join("xtal.json")
        .is_file()
    {
        return Some(project_root.join(DEFAULT_EVENTS_DIR));
    }

    None
}

pub(crate) fn maybe_write_task_failed_event_for_contract_violation(
    project_root: &Path,
    incident_id: &str,
    repro_bytes: &[u8],
) -> Result<Option<PathBuf>> {
    let Some(root_dir) = resolve_events_root_dir(project_root) else {
        return Ok(None);
    };

    let repro: Value = serde_json::from_slice(repro_bytes).context("parse contract repro JSON")?;
    let world = repro
        .get("world")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let source = repro
        .get("source")
        .cloned()
        .unwrap_or_else(|| json!({ "mode": "unknown" }));

    let contract = repro.get("contract").cloned().unwrap_or(Value::Null);
    let task_id = repro
        .pointer("/contract/fn")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let preimage = {
        let mut doc = json!({
            "schema_version": RECOVERY_EVENT_SCHEMA_VERSION,
            "kind": "task_failed_v1",
            "world": world,
            "source": source,
            "related_violation_id": incident_id,
            "details": {
                "contract": contract
            }
        });
        if let Some(task_id) = task_id {
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("task_id".to_string(), Value::String(task_id));
            }
        }
        doc
    };

    let event_id = util::sha256_hex(&util::canonical_jcs_bytes(&preimage)?);
    let mut event = preimage;
    if let Some(obj) = event.as_object_mut() {
        obj.insert("event_id".to_string(), Value::String(event_id));
    }

    let schema_diags = report_common::validate_schema(
        RECOVERY_EVENT_SCHEMA_BYTES,
        "spec/x07.xtal.recovery_event@0.1.0.schema.json",
        &event,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "internal error: recovery event JSON is not schema-valid: {}",
            schema_diags[0].message
        );
    }

    let out_dir = root_dir.join(incident_id);
    std::fs::create_dir_all(&out_dir).with_context(|| format!("mkdir: {}", out_dir.display()))?;
    let out_path = out_dir.join("events.jsonl");

    let mut line = util::canonical_jcs_bytes(&event).context("canon recovery event JSON")?;
    line.push(b'\n');

    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)
        .with_context(|| format!("open: {}", out_path.display()))?;
    f.write_all(&line)
        .with_context(|| format!("append: {}", out_path.display()))?;

    Ok(Some(out_path))
}
