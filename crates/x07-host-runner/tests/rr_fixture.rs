use std::path::Path;

use anyhow::Context;
use serde_json::json;
use x07_contracts::{X07_ARCH_RR_INDEX_SCHEMA_VERSION, X07_ARCH_RR_POLICY_SCHEMA_VERSION};

enum DmValue<'a> {
    String(&'a [u8]),
    Number(&'a [u8]),
}

fn encode_dm_string(out: &mut Vec<u8>, v: &[u8]) {
    out.push(3);
    out.extend_from_slice(&(v.len() as u32).to_le_bytes());
    out.extend_from_slice(v);
}

fn encode_dm_number(out: &mut Vec<u8>, v: &[u8]) {
    out.push(2);
    out.extend_from_slice(&(v.len() as u32).to_le_bytes());
    out.extend_from_slice(v);
}

fn encode_dm_doc_ok_map(entries: Vec<(&str, DmValue<'_>)>) -> Vec<u8> {
    let mut items = entries;
    items.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

    let mut out = Vec::new();
    out.push(1);
    out.push(5);
    out.extend_from_slice(&(items.len() as u32).to_le_bytes());
    for (k, v) in items {
        let kb = k.as_bytes();
        out.extend_from_slice(&(kb.len() as u32).to_le_bytes());
        out.extend_from_slice(kb);
        match v {
            DmValue::String(bytes) => encode_dm_string(&mut out, bytes),
            DmValue::Number(bytes) => encode_dm_number(&mut out, bytes),
        }
    }
    out
}

pub struct RrEntry<'a> {
    pub kind: &'a [u8],
    pub op: &'a [u8],
    pub key: &'a [u8],
    pub req: &'a [u8],
    pub resp: &'a [u8],
    pub err_dec: &'a [u8],
    pub latency_ticks: Option<u32>,
}

pub fn write_single_entry_rrbin(path: &Path, entry: &RrEntry<'_>) -> anyhow::Result<()> {
    let latency_ticks_s = entry.latency_ticks.map(|t| t.to_string());

    let mut fields: Vec<(&str, DmValue<'_>)> = vec![
        ("kind", DmValue::String(entry.kind)),
        ("op", DmValue::String(entry.op)),
        ("key", DmValue::String(entry.key)),
        ("req", DmValue::String(entry.req)),
        ("resp", DmValue::String(entry.resp)),
        ("err", DmValue::Number(entry.err_dec)),
    ];
    if let Some(ticks_s) = latency_ticks_s.as_ref() {
        fields.push(("latency_ticks", DmValue::Number(ticks_s.as_bytes())));
    }
    let doc = encode_dm_doc_ok_map(fields);

    let mut out = Vec::new();
    out.extend_from_slice(&(doc.len() as u32).to_le_bytes());
    out.extend_from_slice(&doc);

    std::fs::write(path, out).with_context(|| format!("write rrbin: {}", path.display()))?;
    Ok(())
}

pub fn write_min_rr_arch(repo_root: &Path, policy_id: &str) -> anyhow::Result<()> {
    let index_path = repo_root.join("arch/rr/index.x07rr.json");
    let policy_dir = repo_root.join("arch/rr/policies");
    let policy_path = policy_dir.join(format!("{policy_id}.policy.json"));

    std::fs::create_dir_all(&policy_dir)
        .with_context(|| format!("create arch policy dir: {}", policy_dir.display()))?;

    let index_doc = json!({
        "schema_version": X07_ARCH_RR_INDEX_SCHEMA_VERSION,
        "policies": [
            { "id": policy_id, "policy_path": format!("arch/rr/policies/{policy_id}.policy.json") }
        ],
        "defaults": { "record_modes_allowed": ["replay_v1"] }
    });
    let mut index_bytes = serde_json::to_vec_pretty(&index_doc)?;
    index_bytes.push(b'\n');
    std::fs::write(&index_path, index_bytes)
        .with_context(|| format!("write rr index: {}", index_path.display()))?;

    let policy_doc = json!({
        "schema_version": X07_ARCH_RR_POLICY_SCHEMA_VERSION,
        "id": policy_id,
        "match_mode": "lookup_v1",
        "budgets": {
            "max_cassette_bytes": 1048576,
            "max_entries": 1000,
            "max_req_bytes": 1048576,
            "max_resp_bytes": 1048576,
            "max_key_bytes": 4096
        }
    });
    let mut policy_bytes = serde_json::to_vec_pretty(&policy_doc)?;
    policy_bytes.push(b'\n');
    std::fs::write(&policy_path, policy_bytes)
        .with_context(|| format!("write rr policy: {}", policy_path.display()))?;

    Ok(())
}
