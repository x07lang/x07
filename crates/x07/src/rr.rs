use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use serde_json::Value;

use crate::util;

#[derive(Debug, Args)]
pub struct RrArgs {
    #[command(subcommand)]
    pub cmd: Option<RrCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum RrCommand {
    /// Record an HTTP response into a solve-rr fixture directory.
    Record(RecordArgs),
}

#[derive(Debug, Args)]
pub struct RecordArgs {
    /// Fixture output directory (will contain `index.json` + `bodies/`).
    #[arg(long, value_name = "DIR", default_value = "fixtures/rr")]
    pub out: PathBuf,

    /// Key used by `rr.fetch(key)` / `rr.send(key)`.
    #[arg(value_name = "KEY")]
    pub key: String,

    /// URL to fetch (HTTP/HTTPS).
    #[arg(value_name = "URL")]
    pub url: String,

    /// Latency ticks recorded into the fixture index entry.
    #[arg(long, value_name = "TICKS", default_value_t = 0)]
    pub latency_ticks: u64,

    /// Replace an existing entry for KEY.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Debug, Serialize)]
struct RrError {
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct RrReport<T> {
    ok: bool,
    command: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RrError>,
}

#[derive(Debug, Serialize)]
struct RecordResult {
    out_dir: String,
    key: String,
    url: String,
    status: u16,
    bytes: usize,
    body_file: String,
    index: String,
}

pub fn cmd_rr(args: RrArgs) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing rr subcommand (try --help)");
    };
    match cmd {
        RrCommand::Record(args) => cmd_rr_record(args),
    }
}

fn cmd_rr_record(args: RecordArgs) -> Result<std::process::ExitCode> {
    let key = args.key.trim();
    if key.is_empty() {
        let report = RrReport::<RecordResult> {
            ok: false,
            command: "rr.record",
            result: None,
            error: Some(RrError {
                code: "X07RR_KEY_EMPTY".to_string(),
                message: "key must be non-empty".to_string(),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }
    let url = args.url.trim();
    if url.is_empty() {
        let report = RrReport::<RecordResult> {
            ok: false,
            command: "rr.record",
            result: None,
            error: Some(RrError {
                code: "X07RR_URL_EMPTY".to_string(),
                message: "url must be non-empty".to_string(),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let key_hash = util::sha256_hex(key.as_bytes());
    let body_rel = format!("bodies/{key_hash}.bin");

    let out_dir = args.out;
    let body_path = out_dir.join(&body_rel);
    if let Some(parent) = body_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }

    let resp = match ureq::get(url)
        .config()
        .http_status_as_error(false)
        .build()
        .call()
    {
        Ok(resp) => resp,
        Err(err) => {
            let report = RrReport::<RecordResult> {
                ok: false,
                command: "rr.record",
                result: None,
                error: Some(RrError {
                    code: "X07RR_HTTP".to_string(),
                    message: format!("{err}"),
                }),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };

    let status: u16 = resp.status().into();
    let mut reader = resp.into_body().into_reader();
    let mut body = Vec::new();
    reader
        .read_to_end(&mut body)
        .context("read http response body")?;

    std::fs::write(&body_path, &body).with_context(|| format!("write: {}", body_path.display()))?;

    let index_path = out_dir.join("index.json");
    let mut index_doc: Value = if index_path.is_file() {
        let bytes = std::fs::read(&index_path)
            .with_context(|| format!("read: {}", index_path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("parse JSON: {}", index_path.display()))?
    } else {
        Value::Object(serde_json::Map::new())
    };

    let Some(obj) = index_doc.as_object_mut() else {
        anyhow::bail!("rr index must be a JSON object: {}", index_path.display());
    };
    let format = obj
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if !format.is_empty() && format != "x07.rr.fixture_index@0.1.0" {
        anyhow::bail!(
            "rr index format mismatch: expected x07.rr.fixture_index@0.1.0 got {:?} ({})",
            format,
            index_path.display()
        );
    }
    obj.insert(
        "format".to_string(),
        Value::String("x07.rr.fixture_index@0.1.0".to_string()),
    );
    if !obj.contains_key("default_latency_ticks") {
        obj.insert("default_latency_ticks".to_string(), Value::Number(0.into()));
    }

    let requests_val = obj
        .entry("requests".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let Some(requests_obj) = requests_val.as_object_mut() else {
        anyhow::bail!(
            "rr index requests must be a JSON object: {}",
            index_path.display()
        );
    };

    if requests_obj.contains_key(key) && !args.overwrite {
        let report = RrReport::<RecordResult> {
            ok: false,
            command: "rr.record",
            result: None,
            error: Some(RrError {
                code: "X07RR_KEY_EXISTS".to_string(),
                message: format!("fixture already contains key (use --overwrite): {key}"),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let mut req_obj = serde_json::Map::new();
    req_obj.insert(
        "latency_ticks".to_string(),
        Value::Number(serde_json::Number::from(args.latency_ticks)),
    );
    req_obj.insert("body_file".to_string(), Value::String(body_rel.clone()));
    req_obj.insert("status".to_string(), Value::Number(status.into()));
    requests_obj.insert(key.to_string(), Value::Object(req_obj));

    let sorted: BTreeMap<String, Value> = requests_obj
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    obj.insert(
        "requests".to_string(),
        Value::Object(sorted.into_iter().collect()),
    );

    let mut out = serde_json::to_vec_pretty(&index_doc)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }
    std::fs::write(&index_path, &out)
        .with_context(|| format!("write: {}", index_path.display()))?;

    let report = RrReport {
        ok: true,
        command: "rr.record",
        result: Some(RecordResult {
            out_dir: out_dir.display().to_string(),
            key: key.to_string(),
            url: url.to_string(),
            status,
            bytes: body.len(),
            body_file: body_rel,
            index: index_path.display().to_string(),
        }),
        error: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::SUCCESS)
}
