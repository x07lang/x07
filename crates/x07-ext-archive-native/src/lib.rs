#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use crc32fast::Hasher as Crc32;
use flate2::read::GzDecoder;
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tar::Archive;
use x07_ext_os_native_core::{
    canonicalize_best_effort, canonicalize_existing_prefix, cap_allow_symlinks, cap_atomic_write,
    cap_create_parents, cap_overwrite, effective_max, enforce_read_path, enforce_write_path,
    map_io_err, open_atomic_tmp_best_effort, parse_caps_v1, policy, FS_ERR_BAD_CAPS,
    FS_ERR_BAD_PATH, FS_ERR_DISABLED, FS_ERR_IO, FS_ERR_IS_DIR, FS_ERR_NOT_FOUND,
    FS_ERR_POLICY_DENY, FS_ERR_SYMLINK_DENIED, FS_ERR_TOO_LARGE,
};
use zip::unstable::stream::{ZipStreamFileMetadata, ZipStreamReader, ZipStreamVisitor};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_bytes {
    pub ptr: *mut u8,
    pub len: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_result_i32 {
    pub tag: u32,
    pub payload: ev_result_i32_payload,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ev_result_i32_payload {
    pub ok: i32,
    pub err: u32,
}

extern "C" {
    fn ev_bytes_alloc(len: u32) -> ev_bytes;
    fn ev_trap(code: i32) -> !;
}

const EV_TRAP_ARCHIVE_INTERNAL: i32 = 9850;

const ARCHIVE_MAX_ENTRIES: usize = 20_000;
const ARCHIVE_MAX_FILE_BYTES: u64 = 268_435_456;
const ARCHIVE_MAX_TOTAL_OUT_BYTES: u64 = 1_073_741_824;

const ZIP_MAX_TOTAL_NAME_BYTES: usize = 81_920_000;

const TGZ_MAX_INFLATE_RATIO_X100: u64 = 20_000;

#[inline]
unsafe fn alloc_bytes(len: u32) -> ev_bytes {
    let out = ev_bytes_alloc(len);
    if out.len != len {
        ev_trap(EV_TRAP_ARCHIVE_INTERNAL);
    }
    out
}

#[inline]
unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    core::slice::from_raw_parts(b.ptr as *const u8, b.len as usize)
}

fn write_u32_le(dst: &mut [u8], x: u32) {
    dst.copy_from_slice(&x.to_le_bytes());
}

unsafe fn ok_doc(payload: &[u8]) -> ev_bytes {
    let len = 1u32.saturating_add(payload.len().try_into().unwrap_or(u32::MAX));
    let out = alloc_bytes(len);
    let dst = core::slice::from_raw_parts_mut(out.ptr, out.len as usize);
    dst[0] = 1;
    dst[1..].copy_from_slice(payload);
    out
}

unsafe fn err_doc(code: u32, msg_json: &[u8]) -> ev_bytes {
    let msg_len: u32 = msg_json.len().try_into().unwrap_or(u32::MAX);
    let len = 1u32
        .saturating_add(4)
        .saturating_add(4)
        .saturating_add(msg_len);
    let out = alloc_bytes(len);
    let dst = core::slice::from_raw_parts_mut(out.ptr, out.len as usize);
    dst[0] = 0;
    write_u32_le(&mut dst[1..5], code);
    write_u32_le(&mut dst[5..9], msg_len);
    dst[9..].copy_from_slice(msg_json);
    out
}

#[allow(clippy::too_many_arguments)]
fn canonical_issue_json(
    kind: &str,
    op: &str,
    profile_id: &str,
    path: &str,
    code: u32,
    detail: u32,
    message: &str,
    expected_form: &str,
    rewrite_hint: &str,
) -> Vec<u8> {
    let mut m: BTreeMap<String, Value> = BTreeMap::new();
    m.insert("code".to_string(), Value::from(code));
    m.insert("detail".to_string(), Value::from(detail));
    m.insert("expected_form".to_string(), Value::from(expected_form));
    m.insert("kind".to_string(), Value::from(kind));
    m.insert("message".to_string(), Value::from(message));
    m.insert("op".to_string(), Value::from(op));
    m.insert("path".to_string(), Value::from(path));
    m.insert("profile_id".to_string(), Value::from(profile_id));
    m.insert("rewrite_hint".to_string(), Value::from(rewrite_hint));
    m.insert(
        "schema_version".to_string(),
        Value::from("x07.archive.issue@0.1.0"),
    );
    serde_json::to_vec(&m).unwrap_or_else(|_| {
        br#"{"schema_version":"x07.archive.issue@0.1.0","kind":"internal","op":"ext.archive","profile_id":"","path":"","code":9851,"detail":0,"message":"internal error","expected_form":"","rewrite_hint":""}"#.to_vec()
    })
}

fn posix_strict_check_v1(path: &[u8]) -> Result<(), u32> {
    if std::str::from_utf8(path).is_err() {
        return Err(1);
    }
    let n = path.len();
    if n == 0 {
        return Err(7);
    }
    if n > 4096 {
        return Err(5);
    }
    if path[0] == b'/' {
        return Err(2);
    }
    if path.contains(&b'\\') {
        return Err(4);
    }

    let mut seg_len: usize = 0;
    let mut seg_start: usize = 0;
    for (i, &b) in path.iter().enumerate() {
        if b != b'/' {
            seg_len += 1;
            continue;
        }
        if seg_len == 0 {
            return Err(7);
        }
        if seg_len > 255 {
            return Err(6);
        }
        if seg_len == 2 && &path[seg_start..seg_start + 2] == b".." {
            return Err(3);
        }
        seg_start = i + 1;
        seg_len = 0;
    }
    if seg_len == 0 {
        return Err(7);
    }
    if seg_len > 255 {
        return Err(6);
    }
    if seg_len == 2 && &path[seg_start..seg_start + 2] == b".." {
        return Err(3);
    }

    Ok(())
}

fn path_policy_err_doc(op: &str, profile_id: &str, path: &str, rc: u32) -> Vec<u8> {
    let (msg, hint) = match rc {
        1 => (
            "path is not valid UTF-8",
            "re-archive with UTF-8 entry paths",
        ),
        2 => (
            "absolute paths are not allowed",
            "strip leading / from archive entry paths",
        ),
        3 => (
            "parent '..' segments are not allowed",
            "remove .. segments from archive entry paths",
        ),
        4 => (
            "backslashes are not allowed",
            "normalize entry paths to use / separators",
        ),
        5 => (
            "path exceeds max_path_bytes",
            "shorten the entry path (or relax max_path_bytes in the archive profile)",
        ),
        6 => (
            "path segment exceeds max_segment_bytes",
            "shorten the path segment (or relax max_segment_bytes in the archive profile)",
        ),
        7 => (
            "empty path segment is not allowed",
            "remove duplicate / and trailing / from entry paths",
        ),
        _ => (
            "path violates archive path policy",
            "rewrite the entry path to a safe form",
        ),
    };

    canonical_issue_json(
        "path_policy",
        op,
        profile_id,
        path,
        100 + rc,
        rc,
        msg,
        "safe relative POSIX path (UTF-8) without leading /, without .. segments, without backslashes; max 4096 bytes; max segment 255",
        hint,
    )
}

fn unsupported_profile_doc(op: &str, profile_id: &str, expected: &str) -> Vec<u8> {
    canonical_issue_json(
        "unsupported_profile",
        op,
        profile_id,
        "",
        9001,
        0,
        "unsupported profile_id",
        expected,
        "pass a supported profile_id (or update arch/archive/index.x07archive.json + ext-archive-c)",
    )
}

fn fs_err_doc(op: &str, profile_id: &str, path: &str, code: u32) -> Vec<u8> {
    let (msg, expected, hint) = match code as i32 {
        FS_ERR_DISABLED => (
            "os filesystem backend disabled",
            "filesystem backend enabled by policy",
            "enable filesystem access in the run policy (or use a world that allows fs access)",
        ),
        FS_ERR_POLICY_DENY => (
            "filesystem policy denied operation",
            "filesystem policy allows the requested operation and roots",
            "expand the policy allowlist (read/write roots, mkdir/rename toggles) or narrow the requested paths",
        ),
        FS_ERR_BAD_PATH => (
            "invalid filesystem path",
            "safe UTF-8 path without NUL/backslash/.. segments",
            "rewrite the path to a safe UTF-8 POSIX path",
        ),
        FS_ERR_BAD_CAPS => (
            "invalid caps bytes",
            "caps bytes encoded by std.os.fs.spec caps_v1",
            "construct caps via std.os.fs.spec and pass the resulting bytes",
        ),
        FS_ERR_SYMLINK_DENIED => (
            "symlink access denied by policy",
            "policy permits symlink access when caps request it",
            "disable symlink traversal or update the run policy to allow symlinks",
        ),
        FS_ERR_TOO_LARGE => (
            "filesystem size limit exceeded",
            "inputs/outputs within policy max_read_bytes/max_write_bytes",
            "raise policy limits (X07_OS_FS_MAX_*), or lower archive profile limits",
        ),
        FS_ERR_NOT_FOUND => (
            "path not found",
            "existing path within allowed roots",
            "ensure the input path exists and is within allowed read roots",
        ),
        FS_ERR_IS_DIR => (
            "expected file but found directory",
            "file path referencing a regular file",
            "pass a regular file path (not a directory)",
        ),
        FS_ERR_IO => (
            "filesystem IO error",
            "valid filesystem operation",
            "retry, or inspect stderr for OS-level IO diagnostics",
        ),
        _ => (
            "filesystem error",
            "valid filesystem operation",
            "inspect policy + path + caps and retry",
        ),
    };

    canonical_issue_json(
        "fs_error", op, profile_id, path, code, 0, msg, expected, hint,
    )
}

unsafe fn err_doc_from_issue(code: u32, issue_json: Vec<u8>) -> ev_bytes {
    err_doc(code, &issue_json)
}

fn write_json_entries(entries: Vec<Value>) -> Vec<u8> {
    let mut root: BTreeMap<String, Value> = BTreeMap::new();
    root.insert("entries".to_string(), Value::Array(entries));
    serde_json::to_vec(&root).unwrap_or_else(|_| br#"{"entries":[]}"#.to_vec())
}

fn archive_limits_err_doc(
    op: &str,
    profile_id: &str,
    kind: &str,
    code: u32,
    msg: &str,
    expected: &str,
    hint: &str,
) -> Vec<u8> {
    canonical_issue_json(kind, op, profile_id, "", code, 0, msg, expected, hint)
}

fn tar_mode(header: &tar::Header) -> u32 {
    header.mode().unwrap_or(0)
}

fn ensure_out_root(caps_write: x07_ext_os_native_core::CapsV1, out_root: &Path) -> Result<(), u32> {
    if out_root.exists() {
        return Ok(());
    }
    if !cap_create_parents(caps_write) {
        return Err(FS_ERR_POLICY_DENY as u32);
    }
    if !policy().allow_mkdir {
        return Err(FS_ERR_POLICY_DENY as u32);
    }
    fs::create_dir_all(out_root).map_err(|e| map_io_err(&e) as u32)?;
    Ok(())
}

fn copy_to_file<R: io::Read>(mut reader: R, mut out: fs::File, max_bytes: u64) -> Result<u64, u32> {
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = reader.read(&mut buf).map_err(|_| FS_ERR_IO as u32)?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        if total > max_bytes {
            return Err(FS_ERR_TOO_LARGE as u32);
        }
        out.write_all(&buf[..n])
            .map_err(|e| map_io_err(&e) as u32)?;
    }
    Ok(total)
}

fn open_output_file(
    path: &Path,
    caps_write: x07_ext_os_native_core::CapsV1,
) -> Result<(fs::File, Option<PathBuf>), u32> {
    let pol = policy();
    if cap_allow_symlinks(caps_write) && !pol.allow_symlinks {
        return Err(FS_ERR_SYMLINK_DENIED as u32);
    }
    if cap_create_parents(caps_write) && !pol.allow_mkdir {
        return Err(FS_ERR_POLICY_DENY as u32);
    }
    if cap_atomic_write(caps_write) && !pol.allow_rename {
        return Err(FS_ERR_POLICY_DENY as u32);
    }

    if cap_create_parents(caps_write) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| map_io_err(&e) as u32)?;
        }
    }

    let overwrite = cap_overwrite(caps_write);
    if cap_atomic_write(caps_write) {
        let (f, tmp) = open_atomic_tmp_best_effort(path, overwrite).map_err(|c| c as u32)?;
        return Ok((f, Some(tmp)));
    }

    let open = if overwrite {
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
    };

    let f = open.map_err(|e| map_io_err(&e) as u32)?;
    Ok((f, None))
}

fn finalize_atomic_write(tmp: PathBuf, final_path: &Path) -> Result<(), u32> {
    fs::rename(&tmp, final_path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        map_io_err(&e) as u32
    })?;
    Ok(())
}

fn enforce_out_path(out_root_canon: &Path, out_path: &Path) -> Result<(), u32> {
    let out_path_canon = canonicalize_existing_prefix(&canonicalize_best_effort(out_path));
    if !out_path_canon.starts_with(out_root_canon) {
        return Err(FS_ERR_POLICY_DENY as u32);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn tar_extract_impl(
    op: &str,
    profile_expected: &str,
    profile_id: &str,
    out_root: &[u8],
    tar_path: &[u8],
    caps_read: &[u8],
    caps_write: &[u8],
    reader: impl Read,
) -> ev_bytes {
    if profile_id != profile_expected {
        let issue = unsupported_profile_doc(op, profile_id, profile_expected);
        return unsafe { err_doc_from_issue(9001, issue) };
    }

    let caps_read = match parse_caps_v1(caps_read) {
        Ok(c) => c,
        Err(code) => {
            let p = String::from_utf8_lossy(tar_path);
            let issue = fs_err_doc(op, profile_id, &p, code as u32);
            return unsafe { err_doc_from_issue(code as u32, issue) };
        }
    };
    let caps_write = match parse_caps_v1(caps_write) {
        Ok(c) => c,
        Err(code) => {
            let p = String::from_utf8_lossy(out_root);
            let issue = fs_err_doc(op, profile_id, &p, code as u32);
            return unsafe { err_doc_from_issue(code as u32, issue) };
        }
    };

    let pol = policy();
    if cap_allow_symlinks(caps_read) && !pol.allow_symlinks {
        let p = String::from_utf8_lossy(tar_path);
        let issue = fs_err_doc(op, profile_id, &p, FS_ERR_SYMLINK_DENIED as u32);
        return unsafe { err_doc_from_issue(FS_ERR_SYMLINK_DENIED as u32, issue) };
    }
    if cap_allow_symlinks(caps_write) && !pol.allow_symlinks {
        let p = String::from_utf8_lossy(out_root);
        let issue = fs_err_doc(op, profile_id, &p, FS_ERR_SYMLINK_DENIED as u32);
        return unsafe { err_doc_from_issue(FS_ERR_SYMLINK_DENIED as u32, issue) };
    }

    let out_root_pb = match enforce_write_path(caps_write, out_root) {
        Ok(p) => p,
        Err(code) => {
            let p = String::from_utf8_lossy(out_root);
            let issue = fs_err_doc(op, profile_id, &p, code as u32);
            return unsafe { err_doc_from_issue(code as u32, issue) };
        }
    };
    let tar_pb = match enforce_read_path(caps_read, tar_path) {
        Ok(p) => p,
        Err(code) => {
            let p = String::from_utf8_lossy(tar_path);
            let issue = fs_err_doc(op, profile_id, &p, code as u32);
            return unsafe { err_doc_from_issue(code as u32, issue) };
        }
    };

    if let Err(code) = ensure_out_root(caps_write, &out_root_pb) {
        let p = String::from_utf8_lossy(out_root);
        let issue = fs_err_doc(op, profile_id, &p, code);
        return unsafe { err_doc_from_issue(code, issue) };
    }

    let out_root_canon = canonicalize_best_effort(&out_root_pb);

    let md = match fs::metadata(&tar_pb) {
        Ok(m) => m,
        Err(e) => {
            let p = tar_pb.display().to_string();
            let code = map_io_err(&e) as u32;
            let issue = fs_err_doc(op, profile_id, &p, code);
            return unsafe { err_doc_from_issue(code, issue) };
        }
    };
    if md.is_dir() {
        let p = tar_pb.display().to_string();
        let issue = fs_err_doc(op, profile_id, &p, FS_ERR_IS_DIR as u32);
        return unsafe { err_doc_from_issue(FS_ERR_IS_DIR as u32, issue) };
    }
    let max_read = effective_max(pol.max_read_bytes, caps_read.max_read_bytes) as u64;
    if md.len() > max_read {
        let p = tar_pb.display().to_string();
        let issue = fs_err_doc(op, profile_id, &p, FS_ERR_TOO_LARGE as u32);
        return unsafe { err_doc_from_issue(FS_ERR_TOO_LARGE as u32, issue) };
    }

    let mut archive = Archive::new(reader);
    let entries = match archive.entries() {
        Ok(e) => e,
        Err(_) => {
            let issue = archive_limits_err_doc(
                op,
                profile_id,
                "invalid_archive",
                1,
                "invalid tar: truncated header or invalid fields",
                "valid tar bytes",
                "ensure the tar input is complete and correctly formatted",
            );
            return unsafe { err_doc_from_issue(1, issue) };
        }
    };

    let mut extracted: Vec<Value> = Vec::new();
    let mut count: usize = 0;
    let mut total_out: u64 = 0;

    for ent in entries {
        let mut entry = match ent {
            Ok(e) => e,
            Err(_) => {
                let issue = archive_limits_err_doc(
                    op,
                    profile_id,
                    "invalid_archive",
                    1,
                    "invalid tar: truncated header or invalid fields",
                    "valid tar bytes",
                    "ensure the tar input is complete and correctly formatted",
                );
                return unsafe { err_doc_from_issue(1, issue) };
            }
        };

        if !entry.header().entry_type().is_file() {
            continue;
        }

        let size = entry.size();
        if size > ARCHIVE_MAX_FILE_BYTES {
            let issue = archive_limits_err_doc(
                op,
                profile_id,
                "output_limit",
                3,
                "output limit exceeded",
                "tar where each file <= 268435456 bytes and total <= 1073741824 bytes",
                "reduce extracted output or relax archive profile limits",
            );
            return unsafe { err_doc_from_issue(3, issue) };
        }

        total_out = total_out.saturating_add(size);
        if total_out > ARCHIVE_MAX_TOTAL_OUT_BYTES {
            let issue = archive_limits_err_doc(
                op,
                profile_id,
                "output_limit",
                3,
                "output limit exceeded",
                "tar where each file <= 268435456 bytes and total <= 1073741824 bytes",
                "reduce extracted output or relax archive profile limits",
            );
            return unsafe { err_doc_from_issue(3, issue) };
        }

        count += 1;
        if count > ARCHIVE_MAX_ENTRIES {
            let issue = archive_limits_err_doc(
                op,
                profile_id,
                "entry_limit",
                2,
                "entry limit exceeded",
                "tar with <= 20000 entries",
                "reduce entry count/paths or relax limits in the archive profile",
            );
            return unsafe { err_doc_from_issue(2, issue) };
        }

        let path_bytes = entry.path_bytes();
        let path_bytes = path_bytes.as_ref();
        let path_str: String = match posix_strict_check_v1(path_bytes) {
            Ok(_) => unsafe { std::str::from_utf8_unchecked(path_bytes) }.to_string(),
            Err(rc) => {
                let path_s = String::from_utf8_lossy(path_bytes);
                let issue = path_policy_err_doc(op, profile_id, &path_s, rc);
                return unsafe { err_doc_from_issue(100 + rc, issue) };
            }
        };

        let out_path_bytes = format!(
            "{}/{}",
            x07_ext_os_native_core::bytes_to_utf8(out_root).unwrap_or(""),
            path_str
        );
        let out_path = match enforce_write_path(caps_write, out_path_bytes.as_bytes()) {
            Ok(p) => p,
            Err(code) => {
                let issue = fs_err_doc(op, profile_id, &path_str, code as u32);
                return unsafe { err_doc_from_issue(code as u32, issue) };
            }
        };

        if let Err(code) = enforce_out_path(&out_root_canon, &out_path) {
            let issue = fs_err_doc(op, profile_id, &path_str, code);
            return unsafe { err_doc_from_issue(code, issue) };
        }

        let max_write = effective_max(pol.max_write_bytes, caps_write.max_write_bytes) as u64;
        let per_file_max = max_write.min(ARCHIVE_MAX_FILE_BYTES);

        let (outfile, tmp) = match open_output_file(&out_path, caps_write) {
            Ok(v) => v,
            Err(code) => {
                let issue = fs_err_doc(op, profile_id, &path_str, code);
                return unsafe { err_doc_from_issue(code, issue) };
            }
        };
        let written = match copy_to_file(&mut entry, outfile, per_file_max) {
            Ok(n) => n,
            Err(code) => {
                let issue = fs_err_doc(op, profile_id, &path_str, code);
                return unsafe { err_doc_from_issue(code, issue) };
            }
        };
        if written != size {
            let issue = archive_limits_err_doc(
                op,
                profile_id,
                "invalid_archive",
                1,
                "invalid tar: truncated file payload",
                "valid tar bytes",
                "ensure the tar input is complete and correctly formatted",
            );
            return unsafe { err_doc_from_issue(1, issue) };
        }
        if let Some(tmp) = tmp {
            if let Err(code) = finalize_atomic_write(tmp, &out_path) {
                let issue = fs_err_doc(op, profile_id, &path_str, code);
                return unsafe { err_doc_from_issue(code, issue) };
            }
        }

        let mut entry_obj: BTreeMap<String, Value> = BTreeMap::new();
        entry_obj.insert("mode".to_string(), Value::from(tar_mode(entry.header())));
        entry_obj.insert("path".to_string(), Value::from(path_str));
        entry_obj.insert("size".to_string(), Value::from(size));
        extracted.push(Value::Object(entry_obj.into_iter().collect()));
    }

    unsafe { ok_doc(&write_json_entries(extracted)) }
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_archive_tar_extract_to_fs_v1(
    out_root: ev_bytes,
    tar_path: ev_bytes,
    caps_read: ev_bytes,
    caps_write: ev_bytes,
    profile_id: ev_bytes,
) -> ev_bytes {
    std::panic::catch_unwind(|| unsafe {
        let out_root_b = bytes_as_slice(out_root);
        let tar_path_b = bytes_as_slice(tar_path);
        let caps_read_b = bytes_as_slice(caps_read);
        let caps_write_b = bytes_as_slice(caps_write);
        let profile_id_b = bytes_as_slice(profile_id);
        let profile_id_s = String::from_utf8_lossy(profile_id_b);

        let op = "os.archive.tar_extract_to_fs_v1";
        let caps_read = match parse_caps_v1(caps_read_b) {
            Ok(c) => c,
            Err(code) => {
                let issue = fs_err_doc(
                    op,
                    &profile_id_s,
                    &String::from_utf8_lossy(tar_path_b),
                    code as u32,
                );
                return err_doc_from_issue(code as u32, issue);
            }
        };
        let tar_pb = match enforce_read_path(caps_read, tar_path_b) {
            Ok(p) => p,
            Err(code) => {
                let issue = fs_err_doc(
                    op,
                    &profile_id_s,
                    &String::from_utf8_lossy(tar_path_b),
                    code as u32,
                );
                return err_doc_from_issue(code as u32, issue);
            }
        };
        let f = match fs::File::open(&tar_pb) {
            Ok(f) => f,
            Err(e) => {
                let code = map_io_err(&e) as u32;
                let issue = fs_err_doc(
                    op,
                    &profile_id_s,
                    &String::from_utf8_lossy(tar_path_b),
                    code,
                );
                return err_doc_from_issue(code, issue);
            }
        };

        tar_extract_impl(
            op,
            "tar_extract_safe_v1",
            &profile_id_s,
            out_root_b,
            tar_path_b,
            caps_read_b,
            caps_write_b,
            f,
        )
    })
    .unwrap_or_else(|_| {
        let msg = canonical_issue_json(
            "internal",
            "os.archive.tar_extract_to_fs_v1",
            "",
            "",
            9850,
            0,
            "panic in ext-archive-native backend",
            "valid archive inputs and caps",
            "file a bug with the repro input",
        );
        err_doc(9850, &msg)
    })
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_archive_tgz_extract_to_fs_v1(
    out_root: ev_bytes,
    tgz_path: ev_bytes,
    caps_read: ev_bytes,
    caps_write: ev_bytes,
    profile_id: ev_bytes,
) -> ev_bytes {
    std::panic::catch_unwind(|| unsafe {
        let out_root_b = bytes_as_slice(out_root);
        let tgz_path_b = bytes_as_slice(tgz_path);
        let caps_read_b = bytes_as_slice(caps_read);
        let caps_write_b = bytes_as_slice(caps_write);
        let profile_id_b = bytes_as_slice(profile_id);
        let profile_id_s = String::from_utf8_lossy(profile_id_b);

        let op = "os.archive.tgz_extract_to_fs_v1";
        let caps_read = match parse_caps_v1(caps_read_b) {
            Ok(c) => c,
            Err(code) => {
                let issue = fs_err_doc(
                    op,
                    &profile_id_s,
                    &String::from_utf8_lossy(tgz_path_b),
                    code as u32,
                );
                return err_doc_from_issue(code as u32, issue);
            }
        };
        let tgz_pb = match enforce_read_path(caps_read, tgz_path_b) {
            Ok(p) => p,
            Err(code) => {
                let issue = fs_err_doc(
                    op,
                    &profile_id_s,
                    &String::from_utf8_lossy(tgz_path_b),
                    code as u32,
                );
                return err_doc_from_issue(code as u32, issue);
            }
        };
        let md = match fs::metadata(&tgz_pb) {
            Ok(m) => m,
            Err(e) => {
                let code = map_io_err(&e) as u32;
                let issue = fs_err_doc(op, &profile_id_s, &tgz_pb.display().to_string(), code);
                return err_doc_from_issue(code, issue);
            }
        };

        let max_out = ARCHIVE_MAX_TOTAL_OUT_BYTES;
        let ratio_mult: u64 = TGZ_MAX_INFLATE_RATIO_X100 / 100;
        let ratio_cap = if md.len() < 1 {
            max_out
        } else {
            let scaled = md.len().saturating_mul(ratio_mult);
            scaled.min(max_out)
        };

        let f = match fs::File::open(&tgz_pb) {
            Ok(f) => f,
            Err(e) => {
                let code = map_io_err(&e) as u32;
                let issue = fs_err_doc(op, &profile_id_s, &tgz_pb.display().to_string(), code);
                return err_doc_from_issue(code, issue);
            }
        };

        struct LimitReader<R> {
            inner: R,
            cap: u64,
            read: u64,
            hit_cap: bool,
        }
        impl<R: Read> Read for LimitReader<R> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.hit_cap {
                    return Err(io::Error::other("inflate cap"));
                }
                let n = self.inner.read(buf)?;
                self.read = self.read.saturating_add(n as u64);
                if self.read > self.cap {
                    self.hit_cap = true;
                    return Err(io::Error::other("inflate cap"));
                }
                Ok(n)
            }
        }

        let gz = GzDecoder::new(f);
        let mut limited = LimitReader {
            inner: gz,
            cap: ratio_cap,
            read: 0,
            hit_cap: false,
        };
        let out = tar_extract_impl(
            op,
            "tgz_extract_safe_v1",
            &profile_id_s,
            out_root_b,
            tgz_path_b,
            caps_read_b,
            caps_write_b,
            &mut limited,
        );
        if limited.hit_cap {
            let code = if ratio_cap < max_out { 21 } else { 22 };
            let kind = if code == 21 {
                "inflate_ratio_limit"
            } else {
                "inflate_output_limit"
            };
            let expected = if code == 21 {
                "tgz with inflate_ratio_x100 <= 20000"
            } else {
                "tgz with inflate_out_bytes <= 1073741824"
            };
            let hint = if code == 21 {
                "reduce compression ratio or relax max_inflate_ratio_x100 in the archive profile"
            } else {
                "reduce inflated output or relax max_inflate_out_bytes in the archive profile"
            };
            let issue = archive_limits_err_doc(
                op,
                &profile_id_s,
                kind,
                code,
                if code == 21 {
                    "inflate ratio limit exceeded"
                } else {
                    "inflate output limit exceeded"
                },
                expected,
                hint,
            );
            return err_doc_from_issue(code, issue);
        }
        out
    })
    .unwrap_or_else(|_| {
        let msg = canonical_issue_json(
            "internal",
            "os.archive.tgz_extract_to_fs_v1",
            "",
            "",
            9850,
            0,
            "panic in ext-archive-native backend",
            "valid archive inputs and caps",
            "file a bug with the repro input",
        );
        err_doc(9850, &msg)
    })
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_archive_zip_extract_to_fs_v1(
    out_root: ev_bytes,
    zip_path: ev_bytes,
    caps_read: ev_bytes,
    caps_write: ev_bytes,
    profile_id: ev_bytes,
) -> ev_bytes {
    std::panic::catch_unwind(|| unsafe {
        let out_root_b = bytes_as_slice(out_root);
        let zip_path_b = bytes_as_slice(zip_path);
        let caps_read_b = bytes_as_slice(caps_read);
        let caps_write_b = bytes_as_slice(caps_write);
        let profile_id_b = bytes_as_slice(profile_id);
        let profile_id_s = String::from_utf8_lossy(profile_id_b);

        let op = "os.archive.zip_extract_to_fs_v1";
        if profile_id_s.as_ref() != "zip_extract_safe_v1" {
            let issue = unsupported_profile_doc(op, &profile_id_s, "zip_extract_safe_v1");
            return err_doc_from_issue(9001, issue);
        }

        let caps_read = match parse_caps_v1(caps_read_b) {
            Ok(c) => c,
            Err(code) => {
                let issue = fs_err_doc(
                    op,
                    &profile_id_s,
                    &String::from_utf8_lossy(zip_path_b),
                    code as u32,
                );
                return err_doc_from_issue(code as u32, issue);
            }
        };
        let caps_write = match parse_caps_v1(caps_write_b) {
            Ok(c) => c,
            Err(code) => {
                let issue = fs_err_doc(
                    op,
                    &profile_id_s,
                    &String::from_utf8_lossy(out_root_b),
                    code as u32,
                );
                return err_doc_from_issue(code as u32, issue);
            }
        };

        let pol = policy();
        if cap_allow_symlinks(caps_read) && !pol.allow_symlinks {
            let issue = fs_err_doc(
                op,
                &profile_id_s,
                &String::from_utf8_lossy(zip_path_b),
                FS_ERR_SYMLINK_DENIED as u32,
            );
            return err_doc_from_issue(FS_ERR_SYMLINK_DENIED as u32, issue);
        }
        if cap_allow_symlinks(caps_write) && !pol.allow_symlinks {
            let issue = fs_err_doc(
                op,
                &profile_id_s,
                &String::from_utf8_lossy(out_root_b),
                FS_ERR_SYMLINK_DENIED as u32,
            );
            return err_doc_from_issue(FS_ERR_SYMLINK_DENIED as u32, issue);
        }

        let out_root_pb = match enforce_write_path(caps_write, out_root_b) {
            Ok(p) => p,
            Err(code) => {
                let issue = fs_err_doc(
                    op,
                    &profile_id_s,
                    &String::from_utf8_lossy(out_root_b),
                    code as u32,
                );
                return err_doc_from_issue(code as u32, issue);
            }
        };
        if let Err(code) = ensure_out_root(caps_write, &out_root_pb) {
            let issue = fs_err_doc(
                op,
                &profile_id_s,
                &String::from_utf8_lossy(out_root_b),
                code,
            );
            return err_doc_from_issue(code, issue);
        }
        let out_root_canon = canonicalize_best_effort(&out_root_pb);

        let zip_pb = match enforce_read_path(caps_read, zip_path_b) {
            Ok(p) => p,
            Err(code) => {
                let issue = fs_err_doc(
                    op,
                    &profile_id_s,
                    &String::from_utf8_lossy(zip_path_b),
                    code as u32,
                );
                return err_doc_from_issue(code as u32, issue);
            }
        };
        let md = match fs::metadata(&zip_pb) {
            Ok(m) => m,
            Err(e) => {
                let code = map_io_err(&e) as u32;
                let issue = fs_err_doc(op, &profile_id_s, &zip_pb.display().to_string(), code);
                return err_doc_from_issue(code, issue);
            }
        };
        if md.is_dir() {
            let issue = fs_err_doc(
                op,
                &profile_id_s,
                &zip_pb.display().to_string(),
                FS_ERR_IS_DIR as u32,
            );
            return err_doc_from_issue(FS_ERR_IS_DIR as u32, issue);
        }
        let max_read = effective_max(pol.max_read_bytes, caps_read.max_read_bytes) as u64;
        if md.len() > max_read {
            let issue = fs_err_doc(
                op,
                &profile_id_s,
                &zip_pb.display().to_string(),
                FS_ERR_TOO_LARGE as u32,
            );
            return err_doc_from_issue(FS_ERR_TOO_LARGE as u32, issue);
        }

        let f = match fs::File::open(&zip_pb) {
            Ok(f) => f,
            Err(e) => {
                let code = map_io_err(&e) as u32;
                let issue = fs_err_doc(op, &profile_id_s, &zip_pb.display().to_string(), code);
                return err_doc_from_issue(code, issue);
            }
        };

        let mut extracted: Vec<Value> = Vec::new();
        let mut total_out: u64 = 0;
        let mut total_name_bytes: usize = 0;
        let mut names: HashSet<Vec<u8>> = HashSet::new();

        struct Visitor<'a> {
            op: &'a str,
            profile_id: &'a str,
            out_root: &'a [u8],
            out_root_canon: &'a Path,
            caps_write: x07_ext_os_native_core::CapsV1,
            pol: &'a x07_ext_os_native_core::Policy,
            extracted: &'a mut Vec<Value>,
            total_out: &'a mut u64,
            total_name_bytes: &'a mut usize,
            names: &'a mut HashSet<Vec<u8>>,
            abort: &'a mut Option<(u32, Vec<u8>)>,
        }

        impl<'a> Visitor<'a> {
            fn abort_with_issue(&mut self, code: u32, issue: Vec<u8>) -> zip::result::ZipError {
                *self.abort = Some((code, issue));
                zip::result::ZipError::Io(io::Error::other("x07 abort"))
            }
        }

        impl<'a> ZipStreamVisitor for Visitor<'a> {
            fn visit_file(
                &mut self,
                file: &mut zip::read::ZipFile<'_>,
            ) -> zip::result::ZipResult<()> {
                if self.names.len() >= ARCHIVE_MAX_ENTRIES {
                    let issue = archive_limits_err_doc(
                        self.op,
                        self.profile_id,
                        "entry_limit",
                        2,
                        "entry limit exceeded",
                        "zip with <= 20000 entries and total name bytes <= 81920000",
                        "reduce entry count/paths or relax limits in the archive profile",
                    );
                    return Err(self.abort_with_issue(2, issue));
                }

                let name_raw = file.name_raw().to_vec();
                *self.total_name_bytes = self.total_name_bytes.saturating_add(name_raw.len());
                if *self.total_name_bytes > ZIP_MAX_TOTAL_NAME_BYTES {
                    let issue = archive_limits_err_doc(
                        self.op,
                        self.profile_id,
                        "entry_limit",
                        2,
                        "entry limit exceeded",
                        "zip with <= 20000 entries and total name bytes <= 81920000",
                        "reduce entry count/paths or relax limits in the archive profile",
                    );
                    return Err(self.abort_with_issue(2, issue));
                }

                let is_dir = file.is_dir();
                let name_trimmed = if is_dir && name_raw.ends_with(b"/") {
                    &name_raw[..name_raw.len().saturating_sub(1)]
                } else {
                    &name_raw
                };
                if name_trimmed.is_empty() {
                    return Ok(());
                }
                if !name_trimmed.is_empty() {
                    if let Err(rc) = posix_strict_check_v1(name_trimmed) {
                        let name_s = String::from_utf8_lossy(name_trimmed).to_string();
                        let issue = path_policy_err_doc(self.op, self.profile_id, &name_s, rc);
                        return Err(self.abort_with_issue(100 + rc, issue));
                    }
                }

                if !self.names.insert(name_trimmed.to_vec()) {
                    let issue = archive_limits_err_doc(
                        self.op,
                        self.profile_id,
                        "duplicate_name",
                        5,
                        "duplicate name in central directory",
                        "zip with unique file names",
                        "dedupe file names or re-create the zip without duplicates",
                    );
                    return Err(self.abort_with_issue(5, issue));
                }

                if file.is_symlink() {
                    let issue = archive_limits_err_doc(
                        self.op,
                        self.profile_id,
                        "symlink_denied",
                        6,
                        "symlinks are not allowed",
                        "zip entries must be regular files or directories",
                        "re-create the zip without symlink entries",
                    );
                    return Err(self.abort_with_issue(6, issue));
                }
                if file.is_dir() {
                    return Ok(());
                }

                let path_str = match std::str::from_utf8(name_trimmed) {
                    Ok(s) => s,
                    Err(_) => {
                        let name_s = String::from_utf8_lossy(name_trimmed).to_string();
                        let issue = path_policy_err_doc(self.op, self.profile_id, &name_s, 1);
                        return Err(self.abort_with_issue(101, issue));
                    }
                };
                let out_path_bytes = format!(
                    "{}/{}",
                    x07_ext_os_native_core::bytes_to_utf8(self.out_root).unwrap_or(""),
                    path_str
                );
                let out_path = match enforce_write_path(self.caps_write, out_path_bytes.as_bytes())
                {
                    Ok(p) => p,
                    Err(code) => {
                        let issue = fs_err_doc(self.op, self.profile_id, path_str, code as u32);
                        return Err(self.abort_with_issue(code as u32, issue));
                    }
                };
                if let Err(code) = enforce_out_path(self.out_root_canon, &out_path) {
                    let issue = fs_err_doc(self.op, self.profile_id, path_str, code);
                    return Err(self.abort_with_issue(code, issue));
                }

                let max_write =
                    effective_max(self.pol.max_write_bytes, self.caps_write.max_write_bytes) as u64;
                let per_file_max = max_write.min(ARCHIVE_MAX_FILE_BYTES);

                let (mut outfile, tmp) = match open_output_file(&out_path, self.caps_write) {
                    Ok(v) => v,
                    Err(code) => {
                        let issue = fs_err_doc(self.op, self.profile_id, path_str, code);
                        return Err(self.abort_with_issue(code, issue));
                    }
                };

                let mut buf = [0u8; 64 * 1024];
                let mut hasher = Crc32::new();
                let mut total: u64 = 0;
                loop {
                    let n = match file.read(&mut buf) {
                        Ok(n) => n,
                        Err(e) => {
                            let issue = archive_limits_err_doc(
                                self.op,
                                self.profile_id,
                                "invalid_archive",
                                1,
                                &format!("invalid zip: {e}"),
                                "valid zip bytes",
                                "ensure the zip input is complete and correctly formatted",
                            );
                            return Err(self.abort_with_issue(1, issue));
                        }
                    };
                    if n == 0 {
                        break;
                    }
                    total = total.saturating_add(n as u64);
                    if total > per_file_max {
                        if max_write < ARCHIVE_MAX_FILE_BYTES {
                            let issue = fs_err_doc(
                                self.op,
                                self.profile_id,
                                path_str,
                                FS_ERR_TOO_LARGE as u32,
                            );
                            return Err(self.abort_with_issue(FS_ERR_TOO_LARGE as u32, issue));
                        }
                        let issue = archive_limits_err_doc(
                            self.op,
                            self.profile_id,
                            "output_limit",
                            3,
                            "output limit exceeded",
                            "zip where each file <= 268435456 bytes and total <= 1073741824 bytes",
                            "reduce extracted output or relax archive profile limits",
                        );
                        return Err(self.abort_with_issue(3, issue));
                    }
                    *self.total_out = self.total_out.saturating_add(n as u64);
                    if *self.total_out > ARCHIVE_MAX_TOTAL_OUT_BYTES {
                        let issue = archive_limits_err_doc(
                            self.op,
                            self.profile_id,
                            "output_limit",
                            3,
                            "output limit exceeded",
                            "zip where each file <= 268435456 bytes and total <= 1073741824 bytes",
                            "reduce extracted output or relax archive profile limits",
                        );
                        return Err(self.abort_with_issue(3, issue));
                    }
                    hasher.update(&buf[..n]);
                    if let Err(e) = outfile.write_all(&buf[..n]) {
                        let code = map_io_err(&e) as u32;
                        let issue = fs_err_doc(self.op, self.profile_id, path_str, code);
                        return Err(self.abort_with_issue(code, issue));
                    }
                }
                let actual_crc = hasher.finalize();
                if actual_crc != file.crc32() {
                    let issue = archive_limits_err_doc(
                        self.op,
                        self.profile_id,
                        "checksum_mismatch",
                        4,
                        "checksum mismatch",
                        "zip with valid CRC32 checksums",
                        "ensure the zip is not corrupted",
                    );
                    return Err(self.abort_with_issue(4, issue));
                }

                if let Err(e) = outfile.flush() {
                    let code = map_io_err(&e) as u32;
                    let issue = fs_err_doc(self.op, self.profile_id, path_str, code);
                    return Err(self.abort_with_issue(code, issue));
                }
                drop(outfile);

                if let Some(tmp) = tmp {
                    if let Err(code) = finalize_atomic_write(tmp, &out_path) {
                        let issue = fs_err_doc(self.op, self.profile_id, path_str, code);
                        return Err(self.abort_with_issue(code, issue));
                    }
                }

                let mut entry_obj: BTreeMap<String, Value> = BTreeMap::new();
                entry_obj.insert("path".to_string(), Value::from(path_str));
                entry_obj.insert("size".to_string(), Value::from(total));
                self.extracted
                    .push(Value::Object(entry_obj.into_iter().collect()));

                Ok(())
            }

            fn visit_additional_metadata(
                &mut self,
                _metadata: &ZipStreamFileMetadata,
            ) -> zip::result::ZipResult<()> {
                Ok(())
            }
        }

        let mut abort: Option<(u32, Vec<u8>)> = None;
        let mut visitor = Visitor {
            op,
            profile_id: &profile_id_s,
            out_root: out_root_b,
            out_root_canon: &out_root_canon,
            caps_write,
            pol,
            extracted: &mut extracted,
            total_out: &mut total_out,
            total_name_bytes: &mut total_name_bytes,
            names: &mut names,
            abort: &mut abort,
        };

        let res = ZipStreamReader::new(f).visit(&mut visitor);
        if let Some((code, issue)) = abort {
            return err_doc_from_issue(code, issue);
        }
        if let Err(e) = res {
            let issue = archive_limits_err_doc(
                op,
                &profile_id_s,
                "invalid_archive",
                1,
                &format!("invalid zip: {e}"),
                "valid zip bytes",
                "ensure the zip input is complete and correctly formatted",
            );
            return err_doc_from_issue(1, issue);
        }

        ok_doc(&write_json_entries(extracted))
    })
    .unwrap_or_else(|_| {
        let msg = canonical_issue_json(
            "internal",
            "os.archive.zip_extract_to_fs_v1",
            "",
            "",
            9850,
            0,
            "panic in ext-archive-native backend",
            "valid archive inputs and caps",
            "file a bug with the repro input",
        );
        err_doc(9850, &msg)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use x07_ext_os_native_core::{CAP_ATOMIC_WRITE, CAP_CREATE_PARENTS, CAP_OVERWRITE};

    #[no_mangle]
    extern "C" fn ev_bytes_alloc(len: u32) -> ev_bytes {
        let mut v = vec![0u8; len as usize];
        let ptr = v.as_mut_ptr();
        std::mem::forget(v);
        ev_bytes { ptr, len }
    }

    #[no_mangle]
    extern "C" fn ev_trap(code: i32) -> ! {
        panic!("ev_trap({code})")
    }

    fn caps_v1(max_read_bytes: u32, max_write_bytes: u32, flags: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(24);
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&max_read_bytes.to_le_bytes());
        out.extend_from_slice(&max_write_bytes.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // max_entries
        out.extend_from_slice(&0u32.to_le_bytes()); // max_depth
        out.extend_from_slice(&flags.to_le_bytes());
        out
    }

    fn to_ev_bytes(b: &[u8]) -> ev_bytes {
        ev_bytes {
            ptr: b.as_ptr() as *mut u8,
            len: b.len() as u32,
        }
    }

    unsafe fn ev_to_vec(b: ev_bytes) -> Vec<u8> {
        std::slice::from_raw_parts(b.ptr, b.len as usize).to_vec()
    }

    fn parse_ok_payload(doc: &[u8]) -> serde_json::Value {
        assert!(!doc.is_empty(), "expected doc bytes");
        assert_eq!(doc[0], 1, "expected ok doc tag");
        serde_json::from_slice(&doc[1..]).expect("parse ok json payload")
    }

    fn parse_err(doc: &[u8]) -> (u32, serde_json::Value) {
        assert!(doc.len() >= 9, "expected err doc bytes");
        assert_eq!(doc[0], 0, "expected err doc tag");
        let code = u32::from_le_bytes([doc[1], doc[2], doc[3], doc[4]]);
        let msg_len = u32::from_le_bytes([doc[5], doc[6], doc[7], doc[8]]) as usize;
        assert_eq!(doc.len(), 9 + msg_len);
        let msg = &doc[9..];
        let issue = serde_json::from_slice(msg).expect("parse issue json");
        (code, issue)
    }

    fn make_root(label: &str) -> std::path::PathBuf {
        let root = format!("target/x07_ext_archive_{label}_{}", std::process::id());
        let root = std::path::PathBuf::from(root);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create test root dir");
        root
    }

    fn write_zip_file(path: &std::path::Path, name: &str, data: &[u8]) {
        let f = std::fs::File::create(path).expect("create zip file");
        let mut zip = zip::ZipWriter::new(f);
        let opts = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o644);
        zip.start_file(name, opts).expect("start zip entry");
        zip.write_all(data).expect("write zip entry");
        zip.finish().expect("finish zip");
    }

    fn write_tar_file(path: &std::path::Path, name: &str, data: &[u8]) {
        let f = std::fs::File::create(path).expect("create tar file");
        let mut tar = tar::Builder::new(f);
        let mut header = tar::Header::new_ustar();
        header.set_path(name).expect("set tar path");
        header.set_mode(0o644);
        header.set_size(data.len() as u64);
        header.set_cksum();
        tar.append(&header, Cursor::new(data))
            .expect("append tar entry");
        tar.finish().expect("finish tar");
    }

    fn write_tgz_file(path: &std::path::Path, name: &str, data: &[u8]) {
        let f = std::fs::File::create(path).expect("create tgz file");
        let enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        let mut header = tar::Header::new_ustar();
        header.set_path(name).expect("set tar path");
        header.set_mode(0o644);
        header.set_size(data.len() as u64);
        header.set_cksum();
        tar.append(&header, Cursor::new(data))
            .expect("append tar entry");
        let enc = tar.into_inner().expect("finish tar builder");
        enc.finish().expect("finish gz");
    }

    fn assert_issue_has_hints(issue: &serde_json::Value) {
        let expected = issue
            .get("expected_form")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let hint = issue
            .get("rewrite_hint")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            !expected.is_empty(),
            "expected expected_form to be non-empty"
        );
        assert!(!hint.is_empty(), "expected rewrite_hint to be non-empty");
    }

    #[test]
    fn zip_extract_to_fs_v1_hello_smoke() {
        std::env::set_var("X07_OS_SANDBOXED", "0");
        std::env::set_var("X07_OS_FS", "1");
        std::env::set_var("X07_OS_FS_ALLOW_MKDIR", "1");
        std::env::set_var("X07_OS_FS_ALLOW_RENAME", "1");
        std::env::set_var("X07_OS_FS_MAX_READ_BYTES", "1000000");
        std::env::set_var("X07_OS_FS_MAX_WRITE_BYTES", "1000000");

        let root = make_root("zip_smoke");
        let zip_path = root.join("in.zip");
        write_zip_file(&zip_path, "hello.txt", b"hello");

        let out_root = root.join("out");
        let caps = caps_v1(
            1_000_000,
            1_000_000,
            CAP_CREATE_PARENTS | CAP_OVERWRITE | CAP_ATOMIC_WRITE,
        );
        let pid = b"zip_extract_safe_v1";

        let doc = unsafe {
            ev_to_vec(x07_ext_archive_zip_extract_to_fs_v1(
                to_ev_bytes(out_root.to_string_lossy().as_bytes()),
                to_ev_bytes(zip_path.to_string_lossy().as_bytes()),
                to_ev_bytes(&caps),
                to_ev_bytes(&caps),
                to_ev_bytes(pid),
            ))
        };
        let payload = parse_ok_payload(&doc);
        let entries = payload
            .get("entries")
            .and_then(|v| v.as_array())
            .expect("entries array");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].get("path").and_then(|v| v.as_str()),
            Some("hello.txt")
        );
        assert_eq!(entries[0].get("size").and_then(|v| v.as_u64()), Some(5));

        let extracted = out_root.join("hello.txt");
        let got = std::fs::read(&extracted).expect("read extracted file");
        assert_eq!(got, b"hello");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tar_extract_to_fs_v1_hello_smoke() {
        std::env::set_var("X07_OS_SANDBOXED", "0");
        std::env::set_var("X07_OS_FS", "1");
        std::env::set_var("X07_OS_FS_ALLOW_MKDIR", "1");
        std::env::set_var("X07_OS_FS_ALLOW_RENAME", "1");
        std::env::set_var("X07_OS_FS_MAX_READ_BYTES", "1000000");
        std::env::set_var("X07_OS_FS_MAX_WRITE_BYTES", "1000000");

        let root = make_root("tar_smoke");
        let tar_path = root.join("in.tar");
        write_tar_file(&tar_path, "hello.txt", b"hello");

        let out_root = root.join("out");
        let caps = caps_v1(
            1_000_000,
            1_000_000,
            CAP_CREATE_PARENTS | CAP_OVERWRITE | CAP_ATOMIC_WRITE,
        );
        let pid = b"tar_extract_safe_v1";

        let doc = unsafe {
            ev_to_vec(x07_ext_archive_tar_extract_to_fs_v1(
                to_ev_bytes(out_root.to_string_lossy().as_bytes()),
                to_ev_bytes(tar_path.to_string_lossy().as_bytes()),
                to_ev_bytes(&caps),
                to_ev_bytes(&caps),
                to_ev_bytes(pid),
            ))
        };
        let payload = parse_ok_payload(&doc);
        let entries = payload
            .get("entries")
            .and_then(|v| v.as_array())
            .expect("entries array");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].get("path").and_then(|v| v.as_str()),
            Some("hello.txt")
        );
        assert_eq!(entries[0].get("size").and_then(|v| v.as_u64()), Some(5));
        assert_eq!(entries[0].get("mode").and_then(|v| v.as_u64()), Some(0o644));

        let extracted = out_root.join("hello.txt");
        let got = std::fs::read(&extracted).expect("read extracted file");
        assert_eq!(got, b"hello");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tgz_extract_to_fs_v1_hello_smoke() {
        std::env::set_var("X07_OS_SANDBOXED", "0");
        std::env::set_var("X07_OS_FS", "1");
        std::env::set_var("X07_OS_FS_ALLOW_MKDIR", "1");
        std::env::set_var("X07_OS_FS_ALLOW_RENAME", "1");
        std::env::set_var("X07_OS_FS_MAX_READ_BYTES", "1000000");
        std::env::set_var("X07_OS_FS_MAX_WRITE_BYTES", "1000000");

        let root = make_root("tgz_smoke");
        let tgz_path = root.join("in.tgz");
        write_tgz_file(&tgz_path, "hello.txt", b"hello");

        let out_root = root.join("out");
        let caps = caps_v1(
            1_000_000,
            1_000_000,
            CAP_CREATE_PARENTS | CAP_OVERWRITE | CAP_ATOMIC_WRITE,
        );
        let pid = b"tgz_extract_safe_v1";

        let doc = unsafe {
            ev_to_vec(x07_ext_archive_tgz_extract_to_fs_v1(
                to_ev_bytes(out_root.to_string_lossy().as_bytes()),
                to_ev_bytes(tgz_path.to_string_lossy().as_bytes()),
                to_ev_bytes(&caps),
                to_ev_bytes(&caps),
                to_ev_bytes(pid),
            ))
        };
        let payload = parse_ok_payload(&doc);
        let entries = payload
            .get("entries")
            .and_then(|v| v.as_array())
            .expect("entries array");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].get("path").and_then(|v| v.as_str()),
            Some("hello.txt")
        );
        assert_eq!(entries[0].get("size").and_then(|v| v.as_u64()), Some(5));
        assert_eq!(entries[0].get("mode").and_then(|v| v.as_u64()), Some(0o644));

        let extracted = out_root.join("hello.txt");
        let got = std::fs::read(&extracted).expect("read extracted file");
        assert_eq!(got, b"hello");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn zip_extract_rejects_zip_slip_paths() {
        std::env::set_var("X07_OS_SANDBOXED", "0");
        std::env::set_var("X07_OS_FS", "1");
        std::env::set_var("X07_OS_FS_ALLOW_MKDIR", "1");
        std::env::set_var("X07_OS_FS_ALLOW_RENAME", "1");
        std::env::set_var("X07_OS_FS_MAX_READ_BYTES", "1000000");
        std::env::set_var("X07_OS_FS_MAX_WRITE_BYTES", "1000000");

        let root = make_root("zip_slip");
        let zip_path = root.join("in.zip");
        write_zip_file(&zip_path, "../evil.txt", b"bad");

        let out_root = root.join("out");
        let caps = caps_v1(
            1_000_000,
            1_000_000,
            CAP_CREATE_PARENTS | CAP_OVERWRITE | CAP_ATOMIC_WRITE,
        );
        let pid = b"zip_extract_safe_v1";

        let doc = unsafe {
            ev_to_vec(x07_ext_archive_zip_extract_to_fs_v1(
                to_ev_bytes(out_root.to_string_lossy().as_bytes()),
                to_ev_bytes(zip_path.to_string_lossy().as_bytes()),
                to_ev_bytes(&caps),
                to_ev_bytes(&caps),
                to_ev_bytes(pid),
            ))
        };
        let (code, issue) = parse_err(&doc);
        assert_eq!(code, 103);
        assert_eq!(
            issue.get("schema_version").and_then(|v| v.as_str()),
            Some("x07.archive.issue@0.1.0")
        );
        assert_eq!(
            issue.get("kind").and_then(|v| v.as_str()),
            Some("path_policy")
        );
        assert_eq!(
            issue.get("op").and_then(|v| v.as_str()),
            Some("os.archive.zip_extract_to_fs_v1")
        );
        assert_eq!(
            issue.get("profile_id").and_then(|v| v.as_str()),
            Some("zip_extract_safe_v1")
        );
        assert_issue_has_hints(&issue);

        let _ = std::fs::remove_dir_all(&root);
    }
}
