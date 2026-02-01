use std::io::Read as _;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

#[derive(Debug, Args)]
pub struct RrArgs {
    #[command(subcommand)]
    pub cmd: Option<RrCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum RrCommand {
    /// Record an HTTP response into an RR cassette file (`*.rrbin`).
    Record(RecordArgs),
}

#[derive(Debug, Args)]
pub struct RecordArgs {
    /// Fixture output directory (will contain `*.rrbin`).
    #[arg(long, value_name = "DIR", default_value = "fixtures/rr")]
    pub out: PathBuf,

    /// Cassette file path relative to --out.
    #[arg(long, value_name = "PATH", default_value = "cassette.rrbin")]
    pub cassette: PathBuf,

    /// Entry kind (defaults to `rr`).
    #[arg(long, value_name = "KIND", default_value = "rr")]
    pub kind: String,

    /// Entry op id (defaults to `std.rr.fetch_v1`).
    #[arg(long, value_name = "OP", default_value = "std.rr.fetch_v1")]
    pub op: String,

    /// Entry key (match key).
    #[arg(value_name = "KEY")]
    pub key: String,

    /// URL to fetch (HTTP/HTTPS).
    #[arg(value_name = "URL")]
    pub url: String,

    /// Latency ticks recorded into the entry (virtual-time ticks).
    #[arg(long, value_name = "TICKS", default_value_t = 0)]
    pub latency_ticks: u64,

    /// Replace an existing matching entry (same kind+op+key).
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
    cassette: String,
    kind: String,
    op: String,
    key: String,
    url: String,
    status: u16,
    bytes: usize,
    seq: u64,
}

pub fn cmd_rr(args: RrArgs) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing rr subcommand (try --help)");
    };
    match cmd {
        RrCommand::Record(args) => cmd_rr_record(args),
    }
}

fn ensure_safe_rel_path(rel: &std::path::Path) -> Result<()> {
    if rel.as_os_str().is_empty() {
        anyhow::bail!("expected non-empty relative path");
    }
    if rel.is_absolute() {
        anyhow::bail!("expected safe relative path, got {}", rel.display());
    }
    for c in rel.components() {
        match c {
            std::path::Component::Normal(_) => {}
            _ => anyhow::bail!("expected safe relative path, got {}", rel.display()),
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct RrEntryMeta {
    kind: Vec<u8>,
    op: Vec<u8>,
    key: Vec<u8>,
    seq: Option<u64>,
}

fn read_u32_le(buf: &[u8], off: usize) -> Option<u32> {
    let bytes = buf.get(off..off + 4)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn dm_skip_value_depth(buf: &[u8], mut off: usize, depth: u32) -> Option<usize> {
    if depth > 64 {
        return None;
    }
    let tag = *buf.get(off)?;
    off += 1;

    match tag {
        0 => Some(off),
        1 => Some(off + 1),
        2 | 3 => {
            let len = read_u32_le(buf, off)? as usize;
            off += 4;
            let end = off.checked_add(len)?;
            if end > buf.len() {
                return None;
            }
            Some(end)
        }
        4 => {
            let count = read_u32_le(buf, off)? as usize;
            off += 4;
            for _ in 0..count {
                off = dm_skip_value_depth(buf, off, depth + 1)?;
            }
            Some(off)
        }
        5 => {
            let count = read_u32_le(buf, off)? as usize;
            off += 4;
            for _ in 0..count {
                let klen = read_u32_le(buf, off)? as usize;
                off += 4;
                let key_end = off.checked_add(klen)?;
                if key_end > buf.len() {
                    return None;
                }
                off = key_end;
                off = dm_skip_value_depth(buf, off, depth + 1)?;
            }
            Some(off)
        }
        _ => None,
    }
}

fn dm_get_string_range(buf: &[u8], off: usize) -> Option<&[u8]> {
    let tag = *buf.get(off)?;
    if tag != 3 {
        return None;
    }
    let len = read_u32_le(buf, off + 1)? as usize;
    let start = off + 1 + 4;
    let end = start.checked_add(len)?;
    if end > buf.len() {
        return None;
    }
    Some(&buf[start..end])
}

fn dm_get_number_str(buf: &[u8], off: usize) -> Option<&[u8]> {
    let tag = *buf.get(off)?;
    if tag != 2 {
        return None;
    }
    let len = read_u32_le(buf, off + 1)? as usize;
    let start = off + 1 + 4;
    let end = start.checked_add(len)?;
    if end > buf.len() {
        return None;
    }
    Some(&buf[start..end])
}

fn parse_u64_dec(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }
    let mut acc: u64 = 0;
    for &b in bytes {
        if !b.is_ascii_digit() {
            return None;
        }
        let d = (b - b'0') as u64;
        acc = acc.checked_mul(10)?;
        acc = acc.checked_add(d)?;
    }
    Some(acc)
}

fn parse_entry_meta_v1(doc: &[u8]) -> Result<RrEntryMeta> {
    if doc.len() < 6 {
        anyhow::bail!("entry doc too short");
    }
    if doc[0] != 1 {
        anyhow::bail!("entry doc is not an ok doc");
    }
    if doc[1] != 5 {
        anyhow::bail!("entry doc root is not a map");
    }
    let count = read_u32_le(doc, 2).context("read map count")? as usize;
    let mut pos: usize = 6;
    let mut found_kind: Option<Vec<u8>> = None;
    let mut found_op: Option<Vec<u8>> = None;
    let mut found_key: Option<Vec<u8>> = None;
    let mut found_seq: Option<u64> = None;

    for _ in 0..count {
        let klen = read_u32_le(doc, pos).context("read key len")? as usize;
        pos += 4;
        let key_end = pos.checked_add(klen).context("key len overflow")?;
        if key_end > doc.len() {
            anyhow::bail!("entry doc truncated");
        }
        let key = &doc[pos..key_end];
        pos = key_end;

        let v_off = pos;
        let v_end = dm_skip_value_depth(doc, v_off, 0).context("skip value")?;
        pos = v_end;

        match key {
            b"kind" => {
                let v = dm_get_string_range(doc, v_off).context("kind must be a string")?;
                found_kind = Some(v.to_vec());
            }
            b"op" => {
                let v = dm_get_string_range(doc, v_off).context("op must be a string")?;
                found_op = Some(v.to_vec());
            }
            b"key" => {
                let v = dm_get_string_range(doc, v_off).context("key must be a string")?;
                found_key = Some(v.to_vec());
            }
            b"seq" => {
                let v = dm_get_number_str(doc, v_off).context("seq must be a number")?;
                found_seq = parse_u64_dec(v);
            }
            _ => {}
        }
    }
    if pos != doc.len() {
        anyhow::bail!("entry doc has trailing bytes");
    }
    Ok(RrEntryMeta {
        kind: found_kind.context("entry doc missing kind")?,
        op: found_op.context("entry doc missing op")?,
        key: found_key.context("entry doc missing key")?,
        seq: found_seq,
    })
}

fn dm_write_u32_le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn dm_write_string(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    let len = u32::try_from(bytes.len()).context("string too long")?;
    out.push(3);
    dm_write_u32_le(out, len);
    out.extend_from_slice(bytes);
    Ok(())
}

fn dm_write_number_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    let len = u32::try_from(bytes.len()).context("number string too long")?;
    out.push(2);
    dm_write_u32_le(out, len);
    out.extend_from_slice(bytes);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn make_entry_v1(
    kind: &[u8],
    op: &[u8],
    key: &[u8],
    req: &[u8],
    resp: &[u8],
    err: i32,
    latency_ticks: Option<u32>,
    seq: u64,
) -> Result<Vec<u8>> {
    let mut items: Vec<(&[u8], Vec<u8>, bool)> = vec![
        // String values (tag 3)
        (b"key", key.to_vec(), true),
        (b"kind", kind.to_vec(), true),
        (b"op", op.to_vec(), true),
        (b"req", req.to_vec(), true),
        (b"resp", resp.to_vec(), true),
        // Number values (tag 2) stored as ASCII bytes in the Vec.
        (b"err", err.to_string().into_bytes(), false),
        (b"seq", seq.to_string().into_bytes(), false),
        (b"v", b"1".to_vec(), false),
    ];
    if let Some(lat) = latency_ticks {
        items.push((b"latency_ticks", lat.to_string().into_bytes(), false));
    }

    items.sort_by(|(ka, _, _), (kb, _, _)| ka.cmp(kb));

    let mut out = Vec::new();
    out.push(1);
    out.push(5);
    dm_write_u32_le(
        &mut out,
        u32::try_from(items.len()).context("too many map items")?,
    );
    for (k, v, is_string) in items {
        dm_write_u32_le(&mut out, u32::try_from(k.len()).context("key too long")?);
        out.extend_from_slice(k);
        if is_string {
            dm_write_string(&mut out, &v)?;
        } else {
            dm_write_number_bytes(&mut out, &v)?;
        }
    }
    Ok(out)
}

fn write_rrbin_frame(mut w: impl std::io::Write, payload: &[u8]) -> Result<()> {
    let len = u32::try_from(payload.len()).context("rr entry too large")?;
    w.write_all(&len.to_le_bytes())?;
    w.write_all(payload)?;
    Ok(())
}

fn read_exact_or_eof(r: &mut impl std::io::Read, buf: &mut [u8]) -> Result<bool> {
    let mut pos = 0;
    while pos < buf.len() {
        let n = r.read(&mut buf[pos..])?;
        if n == 0 {
            if pos == 0 {
                return Ok(false);
            }
            anyhow::bail!("unexpected EOF");
        }
        pos += n;
    }
    Ok(true)
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
    let kind = args.kind.trim();
    if kind.is_empty() {
        let report = RrReport::<RecordResult> {
            ok: false,
            command: "rr.record",
            result: None,
            error: Some(RrError {
                code: "X07RR_KIND_EMPTY".to_string(),
                message: "kind must be non-empty".to_string(),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }
    let op = args.op.trim();
    if op.is_empty() {
        let report = RrReport::<RecordResult> {
            ok: false,
            command: "rr.record",
            result: None,
            error: Some(RrError {
                code: "X07RR_OP_EMPTY".to_string(),
                message: "op must be non-empty".to_string(),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
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

    if args.latency_ticks > u64::from(u32::MAX) {
        let report = RrReport::<RecordResult> {
            ok: false,
            command: "rr.record",
            result: None,
            error: Some(RrError {
                code: "X07RR_LATENCY_OUT_OF_RANGE".to_string(),
                message: format!("latency_ticks must fit in u32, got {}", args.latency_ticks),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    ensure_safe_rel_path(&args.cassette).context("validate --cassette")?;

    let out_dir = args.out;
    let cassette_path = out_dir.join(&args.cassette);
    if let Some(parent) = cassette_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }

    let kind_b = kind.as_bytes();
    let op_b = op.as_bytes();
    let key_b = key.as_bytes();
    let latency_u32 = u32::try_from(args.latency_ticks).ok();

    let mut found = false;
    let mut max_seq: Option<u64> = None;
    if cassette_path.is_file() {
        let mut f = std::fs::File::open(&cassette_path)
            .with_context(|| format!("open: {}", cassette_path.display()))?;
        loop {
            let mut hdr = [0u8; 4];
            if !read_exact_or_eof(&mut f, &mut hdr).context("read rrbin frame header")? {
                break;
            }
            let len = u32::from_le_bytes(hdr) as usize;
            let mut payload = vec![0u8; len];
            f.read_exact(&mut payload)
                .context("read rrbin frame payload")?;
            let meta = parse_entry_meta_v1(&payload).context("parse existing entry")?;
            if meta.kind == kind_b && meta.op == op_b && meta.key == key_b {
                found = true;
            }
            if let Some(seq) = meta.seq {
                max_seq = Some(max_seq.map_or(seq, |m| m.max(seq)));
            }
        }
    }

    if found && !args.overwrite {
        let report = RrReport::<RecordResult> {
            ok: false,
            command: "rr.record",
            result: None,
            error: Some(RrError {
                code: "X07RR_ENTRY_EXISTS".to_string(),
                message: format!(
                    "cassette already contains entry (use --overwrite): {kind} {op} {key}"
                ),
            }),
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let seq = max_seq.and_then(|s| s.checked_add(1)).unwrap_or(0);

    if args.overwrite && cassette_path.is_file() && found {
        let file_name = cassette_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("cassette.rrbin"))
            .to_string_lossy();
        let tmp_path = cassette_path.with_file_name(format!("{file_name}.tmp"));

        let mut fin = std::fs::File::open(&cassette_path)
            .with_context(|| format!("open: {}", cassette_path.display()))?;
        let mut fout = std::fs::File::create(&tmp_path)
            .with_context(|| format!("create: {}", tmp_path.display()))?;
        let mut max_kept_seq: Option<u64> = None;

        loop {
            let mut hdr = [0u8; 4];
            if !read_exact_or_eof(&mut fin, &mut hdr).context("read rrbin frame header")? {
                break;
            }
            let len = u32::from_le_bytes(hdr) as usize;
            let mut payload = vec![0u8; len];
            fin.read_exact(&mut payload)
                .context("read rrbin frame payload")?;
            let meta = parse_entry_meta_v1(&payload).context("parse existing entry")?;
            if meta.kind == kind_b && meta.op == op_b && meta.key == key_b {
                continue;
            }
            if let Some(seq) = meta.seq {
                max_kept_seq = Some(max_kept_seq.map_or(seq, |m| m.max(seq)));
            }
            write_rrbin_frame(&mut fout, &payload).context("write rrbin frame")?;
        }

        let seq = max_kept_seq.and_then(|s| s.checked_add(1)).unwrap_or(0);
        let entry = make_entry_v1(
            kind_b,
            op_b,
            key_b,
            key_b,
            &body,
            0,
            latency_u32.filter(|v| *v != 0),
            seq,
        )?;
        write_rrbin_frame(&mut fout, &entry).context("append rrbin entry")?;
        fout.sync_all().ok();

        if cassette_path.is_file() {
            std::fs::remove_file(&cassette_path)
                .with_context(|| format!("remove: {}", cassette_path.display()))?;
        }
        std::fs::rename(&tmp_path, &cassette_path).with_context(|| {
            format!(
                "rename {} -> {}",
                tmp_path.display(),
                cassette_path.display()
            )
        })?;

        let report = RrReport {
            ok: true,
            command: "rr.record",
            result: Some(RecordResult {
                out_dir: out_dir.display().to_string(),
                cassette: args.cassette.display().to_string(),
                kind: kind.to_string(),
                op: op.to_string(),
                key: key.to_string(),
                url: url.to_string(),
                status,
                bytes: body.len(),
                seq,
            }),
            error: None,
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let entry = make_entry_v1(
        kind_b,
        op_b,
        key_b,
        key_b,
        &body,
        0,
        latency_u32.filter(|v| *v != 0),
        seq,
    )?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cassette_path)
        .with_context(|| format!("open: {}", cassette_path.display()))?;
    write_rrbin_frame(&mut f, &entry).context("append rrbin entry")?;
    f.sync_all().ok();

    let report = RrReport {
        ok: true,
        command: "rr.record",
        result: Some(RecordResult {
            out_dir: out_dir.display().to_string(),
            cassette: args.cassette.display().to_string(),
            kind: kind.to_string(),
            op: op.to_string(),
            key: key.to_string(),
            url: url.to_string(),
            status,
            bytes: body.len(),
            seq,
        }),
        error: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::SUCCESS)
}
