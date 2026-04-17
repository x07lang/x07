use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::{report_common, util};

pub(crate) const DEFAULT_VIOLATIONS_DIR: &str = "target/xtal/violations";
pub(crate) const ENV_X07_XTAL_VIOLATIONS_DIR: &str = "X07_XTAL_VIOLATIONS_DIR";

pub(crate) const VIOLATION_SCHEMA_VERSION: &str = "x07.xtal.violation@0.1.0";
pub(crate) const VIOLATION_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.xtal.violation@0.1.0.schema.json");

pub(crate) fn resolve_violation_root_dir(project_root: &Path) -> Option<PathBuf> {
    if let Ok(raw) = std::env::var(ENV_X07_XTAL_VIOLATIONS_DIR) {
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
        return Some(project_root.join(DEFAULT_VIOLATIONS_DIR));
    }

    None
}

pub(crate) fn build_contract_violation_doc(
    project_root: &Path,
    original_repro_path: Option<&Path>,
    repro_bytes: &[u8],
) -> Result<(String, Value)> {
    let id = util::sha256_hex(repro_bytes);

    let repro_value: Value =
        serde_json::from_slice(repro_bytes).context("parse contract repro JSON")?;

    let clause_id = repro_value
        .pointer("/contract/clause_id")
        .and_then(Value::as_str)
        .context("missing /contract/clause_id in contract repro")?
        .to_string();

    let world = repro_value
        .get("world")
        .and_then(Value::as_str)
        .context("missing world in contract repro")?
        .to_string();

    let source = repro_value
        .get("source")
        .cloned()
        .unwrap_or_else(|| json!({ "mode": "unknown" }));

    let original_rel = original_repro_path.map(|p| {
        p.strip_prefix(project_root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    });

    let mut doc = json!({
        "schema_version": VIOLATION_SCHEMA_VERSION,
        "kind": "contract_violation",
        "id": id.clone(),
        "clause_id": clause_id,
        "world": world,
        "source": source,
        "repro": {
            "path": "repro.json",
            "sha256": id.clone(),
            "bytes_len": repro_bytes.len(),
        },
        "generated_at": "2000-01-01T00:00:00Z",
    });

    if let Some(original_rel) = original_rel {
        if let Some(obj) = doc.as_object_mut() {
            obj.insert(
                "original_repro_path".to_string(),
                Value::String(original_rel),
            );
        }
    }

    let schema_diags = report_common::validate_schema(
        VIOLATION_SCHEMA_BYTES,
        "spec/x07.xtal.violation@0.1.0.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "internal error: xtal violation JSON is not schema-valid: {}",
            schema_diags[0].message
        );
    }

    Ok((id, doc))
}

pub(crate) fn write_violation_bundle(
    out_dir: &Path,
    violation_doc: &Value,
    repro_bytes: &[u8],
) -> Result<()> {
    std::fs::create_dir_all(out_dir).with_context(|| format!("mkdir: {}", out_dir.display()))?;

    let repro_out_path = out_dir.join("repro.json");
    util::write_atomic(&repro_out_path, repro_bytes)
        .with_context(|| format!("write: {}", repro_out_path.display()))?;

    let violation_out_path = out_dir.join("violation.json");
    let violation_bytes =
        report_common::canonical_pretty_json_bytes(violation_doc).context("serialize violation")?;
    util::write_atomic(&violation_out_path, &violation_bytes)
        .with_context(|| format!("write: {}", violation_out_path.display()))?;

    Ok(())
}

pub(crate) fn maybe_write_contract_violation_bundle(
    project_root: &Path,
    original_repro_path: &Path,
) -> Result<Option<PathBuf>> {
    let Some(root_dir) = resolve_violation_root_dir(project_root) else {
        return Ok(None);
    };

    let repro_bytes = std::fs::read(original_repro_path)
        .with_context(|| format!("read: {}", original_repro_path.display()))?;

    let (id, doc) =
        build_contract_violation_doc(project_root, Some(original_repro_path), &repro_bytes)?;

    let out_dir = root_dir.join(&id);
    write_violation_bundle(&out_dir, &doc, &repro_bytes)?;

    let _ = crate::xtal_events::maybe_write_task_failed_event_for_contract_violation(
        project_root,
        &id,
        &repro_bytes,
    );

    Ok(Some(out_dir.join("violation.json")))
}
