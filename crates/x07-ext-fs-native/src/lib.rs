#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use globset::{Glob, GlobMatcher};
use once_cell::sync::OnceCell;
use std::fs::OpenOptions;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;
use x07_ext_os_native_core::{
    bytes_to_utf8, cap_allow_hidden, cap_allow_symlinks, cap_atomic_write, cap_create_parents,
    cap_overwrite, effective_max, enforce_read_path, enforce_write_path, map_io_err,
    open_atomic_tmp_best_effort, parse_caps_v1, policy, FS_ERR_ALREADY_EXISTS, FS_ERR_BAD_HANDLE,
    FS_ERR_BAD_PATH, FS_ERR_DEPTH_EXCEEDED, FS_ERR_IO, FS_ERR_IS_DIR, FS_ERR_NOT_DIR,
    FS_ERR_NOT_FOUND, FS_ERR_POLICY_DENY, FS_ERR_SYMLINK_DENIED, FS_ERR_TOO_LARGE,
    FS_ERR_TOO_MANY_ENTRIES, FS_ERR_UNSUPPORTED,
};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_bytes {
    pub ptr: *mut u8,
    pub len: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ev_result_bytes_payload {
    pub ok: ev_bytes,
    pub err: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_result_bytes {
    pub tag: u32, // 1 = ok, 0 = err
    pub payload: ev_result_bytes_payload,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ev_result_i32_payload {
    pub ok: u32,  // i32 bits
    pub err: u32, // error code
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_result_i32 {
    pub tag: u32, // 1 = ok, 0 = err
    pub payload: ev_result_i32_payload,
}

extern "C" {
    fn ev_bytes_alloc(len: u32) -> ev_bytes;
    fn ev_trap(code: i32) -> !;
}

const EV_TRAP_FS_INTERNAL: i32 = 9300;

// -------------------------
// Streaming write handles (FS v1)
// -------------------------

#[derive(Debug)]
struct WriterHandleV1 {
    file: Option<std::fs::File>,
    final_path: PathBuf,
    tmp_path: Option<PathBuf>,
    max_write_bytes: u32,
    written: u32,
}

static WRITERS: OnceCell<Mutex<Vec<Option<WriterHandleV1>>>> = OnceCell::new();

fn writers() -> &'static Mutex<Vec<Option<WriterHandleV1>>> {
    WRITERS.get_or_init(|| Mutex::new(Vec::new()))
}

fn handle_idx(handle: i32) -> Option<usize> {
    if handle <= 0 {
        None
    } else {
        Some((handle as usize).saturating_sub(1))
    }
}

fn handle_insert<T>(table: &mut Vec<Option<T>>, v: T) -> Result<i32, i32> {
    for (idx, slot) in table.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(v);
            let h = idx + 1;
            if h > (i32::MAX as usize) {
                *slot = None;
                return Err(FS_ERR_UNSUPPORTED);
            }
            return Ok(h as i32);
        }
    }
    table.push(Some(v));
    let h = table.len();
    if h > (i32::MAX as usize) {
        table.pop();
        return Err(FS_ERR_UNSUPPORTED);
    }
    Ok(h as i32)
}

// -------------------------
// Streaming read handles (FS v1)
// -------------------------

#[derive(Debug)]
struct ReaderHandleV1 {
    file: Option<std::fs::File>,
    max_read_bytes: u32,
    read: u32,
}

static READERS: OnceCell<Mutex<Vec<Option<ReaderHandleV1>>>> = OnceCell::new();

fn readers() -> &'static Mutex<Vec<Option<ReaderHandleV1>>> {
    READERS.get_or_init(|| Mutex::new(Vec::new()))
}

// -------------------------
// Result helpers
// -------------------------

unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    core::slice::from_raw_parts(b.ptr as *const u8, b.len as usize)
}

fn ok_bytes_vec(v: Vec<u8>) -> ev_result_bytes {
    unsafe {
        let out = alloc_bytes(v.len() as u32);
        if !v.is_empty() {
            core::ptr::copy_nonoverlapping(v.as_ptr(), out.ptr, v.len());
        }
        ev_result_bytes {
            tag: 1,
            payload: ev_result_bytes_payload { ok: out },
        }
    }
}

fn err_bytes(code: i32) -> ev_result_bytes {
    ev_result_bytes {
        tag: 0,
        payload: ev_result_bytes_payload { err: code as u32 },
    }
}

fn ok_i32(v: i32) -> ev_result_i32 {
    ev_result_i32 {
        tag: 1,
        payload: ev_result_i32_payload { ok: v as u32 },
    }
}

fn err_i32(code: i32) -> ev_result_i32 {
    ev_result_i32 {
        tag: 0,
        payload: ev_result_i32_payload { err: code as u32 },
    }
}

unsafe fn alloc_bytes(len: u32) -> ev_bytes {
    let out = ev_bytes_alloc(len);
    if out.len != len {
        ev_trap(EV_TRAP_FS_INTERNAL);
    }
    out
}

fn join_lines_sorted(mut lines: Vec<String>) -> Vec<u8> {
    lines.sort(); // UTF-8 string order
    let mut out = String::new();
    if lines.is_empty() {
        out.push('\n');
        return out.into_bytes();
    }
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    out.into_bytes()
}

fn build_glob_matcher(glob: &str) -> Result<GlobMatcher, i32> {
    Glob::new(glob)
        .map_err(|_| FS_ERR_BAD_PATH)
        .map(|g| g.compile_matcher())
}

// -------------------------
// Exported C ABI functions
// -------------------------

#[no_mangle]
pub extern "C" fn x07_ext_fs_read_all_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_read_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_bytes(code),
        };

        if !policy().allow_symlinks && cap_allow_symlinks(caps) {
            return err_bytes(FS_ERR_SYMLINK_DENIED);
        }

        let md = match std::fs::metadata(&pb) {
            Ok(m) => m,
            Err(e) => return err_bytes(map_io_err(&e)),
        };
        if md.is_dir() {
            return err_bytes(FS_ERR_IS_DIR);
        }

        let max = effective_max(policy().max_read_bytes, caps.max_read_bytes);
        if md.len() > (max as u64) {
            return err_bytes(FS_ERR_TOO_LARGE);
        }

        let mut f = match std::fs::File::open(&pb) {
            Ok(f) => f,
            Err(e) => return err_bytes(map_io_err(&e)),
        };

        let mut data: Vec<u8> = Vec::with_capacity(md.len().min(max as u64) as usize);
        let mut buf = [0u8; 8192];
        loop {
            let n = match f.read(&mut buf) {
                Ok(n) => n,
                Err(e) => return err_bytes(map_io_err(&e)),
            };
            if n == 0 {
                break;
            }
            if data.len() + n > (max as usize) {
                return err_bytes(FS_ERR_TOO_LARGE);
            }
            data.extend_from_slice(&buf[..n]);
        }
        ok_bytes_vec(data)
    })
    .unwrap_or_else(|_| err_bytes(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_write_all_v1(
    path: ev_bytes,
    data: ev_bytes,
    caps: ev_bytes,
) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        if cap_create_parents(caps) && !pol.allow_mkdir {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_atomic_write(caps) && !pol.allow_rename {
            return err_i32(FS_ERR_POLICY_DENY);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        let data_bytes = bytes_as_slice(data);

        let max = effective_max(pol.max_write_bytes, caps.max_write_bytes);
        if data_bytes.len() > (max as usize) {
            return err_i32(FS_ERR_TOO_LARGE);
        }

        if cap_create_parents(caps) {
            if let Some(parent) = pb.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return err_i32(map_io_err(&e));
                }
            }
        }

        if !cap_overwrite(caps) {
            match std::fs::metadata(&pb) {
                Ok(m) => {
                    if m.is_dir() {
                        return err_i32(FS_ERR_IS_DIR);
                    }
                    return err_i32(FS_ERR_ALREADY_EXISTS);
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return err_i32(map_io_err(&e)),
            }
        }

        if cap_atomic_write(caps) {
            return write_atomic_best_effort(&pb, data_bytes, cap_overwrite(caps));
        }

        if let Err(e) = std::fs::write(&pb, data_bytes) {
            return err_i32(map_io_err(&e));
        }
        ok_i32(data_bytes.len() as i32)
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_append_all_v1(
    path: ev_bytes,
    data: ev_bytes,
    caps: ev_bytes,
) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        if cap_create_parents(caps) && !pol.allow_mkdir {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_atomic_write(caps) {
            return err_i32(FS_ERR_UNSUPPORTED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        let data_bytes = bytes_as_slice(data);

        let max = effective_max(pol.max_write_bytes, caps.max_write_bytes);
        if data_bytes.len() > (max as usize) {
            return err_i32(FS_ERR_TOO_LARGE);
        }

        if cap_create_parents(caps) {
            if let Some(parent) = pb.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return err_i32(map_io_err(&e));
                }
            }
        }

        match std::fs::metadata(&pb) {
            Ok(m) => {
                if m.is_dir() {
                    return err_i32(FS_ERR_IS_DIR);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return err_i32(map_io_err(&e)),
        }

        let mut f = match OpenOptions::new().create(true).append(true).open(&pb) {
            Ok(f) => f,
            Err(e) => return err_i32(map_io_err(&e)),
        };
        if let Err(e) = f.write_all(data_bytes) {
            return err_i32(map_io_err(&e));
        }
        ok_i32(data_bytes.len() as i32)
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_open_write_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        if cap_create_parents(caps) && !pol.allow_mkdir {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_atomic_write(caps) && !pol.allow_rename {
            return err_i32(FS_ERR_POLICY_DENY);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        let max_write = effective_max(pol.max_write_bytes, caps.max_write_bytes);

        if cap_create_parents(caps) {
            if let Some(parent) = pb.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return err_i32(map_io_err(&e));
                }
            }
        }

        let overwrite = cap_overwrite(caps);

        if cap_atomic_write(caps) {
            let (f, tmp) = match open_atomic_tmp_best_effort(&pb, overwrite) {
                Ok(v) => v,
                Err(code) => return err_i32(code),
            };

            let handle = match writers().lock() {
                Ok(mut table) => handle_insert(
                    &mut table,
                    WriterHandleV1 {
                        file: Some(f),
                        final_path: pb,
                        tmp_path: Some(tmp),
                        max_write_bytes: max_write,
                        written: 0,
                    },
                ),
                Err(_) => Err(FS_ERR_IO),
            };

            return match handle {
                Ok(h) => ok_i32(h),
                Err(code) => err_i32(code),
            };
        }

        if overwrite {
            match std::fs::metadata(&pb) {
                Ok(m) if m.is_dir() => return err_i32(FS_ERR_IS_DIR),
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return err_i32(map_io_err(&e)),
            }
        } else {
            match std::fs::metadata(&pb) {
                Ok(m) => {
                    if m.is_dir() {
                        return err_i32(FS_ERR_IS_DIR);
                    }
                    return err_i32(FS_ERR_ALREADY_EXISTS);
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return err_i32(map_io_err(&e)),
            }
        }

        let open = if overwrite {
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&pb)
        } else {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&pb)
        };

        let f = match open {
            Ok(f) => f,
            Err(e) => return err_i32(map_io_err(&e)),
        };

        let handle = match writers().lock() {
            Ok(mut table) => handle_insert(
                &mut table,
                WriterHandleV1 {
                    file: Some(f),
                    final_path: pb,
                    tmp_path: None,
                    max_write_bytes: max_write,
                    written: 0,
                },
            ),
            Err(_) => Err(FS_ERR_IO),
        };

        match handle {
            Ok(h) => ok_i32(h),
            Err(code) => err_i32(code),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_write_all_v1(
    writer_handle: i32,
    data: ev_bytes,
) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let Ok(mut table) = writers().lock() else {
            return err_i32(FS_ERR_IO);
        };
        let Some(idx) = handle_idx(writer_handle) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };
        let Some(w) = table.get_mut(idx).and_then(|v| v.as_mut()) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };
        let Some(f) = w.file.as_mut() else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };

        let data_bytes = bytes_as_slice(data);
        let Some(rem) = w.max_write_bytes.checked_sub(w.written) else {
            return err_i32(FS_ERR_TOO_LARGE);
        };
        if data_bytes.len() > (rem as usize) {
            return err_i32(FS_ERR_TOO_LARGE);
        }

        if let Err(e) = f.write_all(data_bytes) {
            return err_i32(map_io_err(&e));
        }
        w.written = w.written.saturating_add(data_bytes.len() as u32);

        ok_i32(data_bytes.len() as i32)
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_close_v1(writer_handle: i32) -> ev_result_i32 {
    std::panic::catch_unwind(|| {
        let Ok(mut table) = writers().lock() else {
            return err_i32(FS_ERR_IO);
        };
        let Some(idx) = handle_idx(writer_handle) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };
        let Some(w) = table.get_mut(idx).and_then(|v| v.as_mut()) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };

        // Idempotent close.
        let Some(f) = w.file.take() else {
            return ok_i32(1);
        };
        drop(f);

        if let Some(tmp) = w.tmp_path.take() {
            if let Err(e) = std::fs::rename(&tmp, &w.final_path) {
                let _ = std::fs::remove_file(&tmp);
                w.tmp_path = Some(tmp);
                return err_i32(map_io_err(&e));
            }
        }

        ok_i32(1)
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_drop_v1(writer_handle: i32) -> i32 {
    std::panic::catch_unwind(|| {
        let Ok(mut table) = writers().lock() else {
            return 1;
        };
        let Some(idx) = handle_idx(writer_handle) else {
            return 1;
        };
        let Some(w) = table.get_mut(idx).and_then(|v| v.take()) else {
            return 1;
        };

        drop(w.file);
        if let Some(tmp) = w.tmp_path {
            let _ = std::fs::remove_file(&tmp);
        }

        1
    })
    .unwrap_or(1)
}

fn write_atomic_best_effort(path: &Path, data: &[u8], overwrite: bool) -> ev_result_i32 {
    let Some(parent) = path.parent() else {
        return err_i32(FS_ERR_BAD_PATH);
    };
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return err_i32(FS_ERR_BAD_PATH);
    };

    if !overwrite && path.exists() {
        return err_i32(FS_ERR_ALREADY_EXISTS);
    }

    let mut counter: u32 = 0;
    let tmp_path = loop {
        let candidate = parent.join(format!("{name}.x07_tmp_{counter}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut f) => {
                if let Err(e) = f.write_all(data) {
                    let _ = std::fs::remove_file(&candidate);
                    return err_i32(map_io_err(&e));
                }
                let _ = f.sync_all();
                break candidate;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                counter = counter.wrapping_add(1);
                continue;
            }
            Err(e) => return err_i32(map_io_err(&e)),
        }
    };

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return err_i32(map_io_err(&e));
    }
    ok_i32(data.len() as i32)
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_open_read_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_read_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        let md = match std::fs::metadata(&pb) {
            Ok(m) => m,
            Err(e) => return err_i32(map_io_err(&e)),
        };
        if md.is_dir() {
            return err_i32(FS_ERR_IS_DIR);
        }

        let max_read = effective_max(pol.max_read_bytes, caps.max_read_bytes);
        if md.len() > (max_read as u64) {
            return err_i32(FS_ERR_TOO_LARGE);
        }

        let f = match std::fs::File::open(&pb) {
            Ok(f) => f,
            Err(e) => return err_i32(map_io_err(&e)),
        };

        let handle = match readers().lock() {
            Ok(mut table) => handle_insert(
                &mut table,
                ReaderHandleV1 {
                    file: Some(f),
                    max_read_bytes: max_read,
                    read: 0,
                },
            ),
            Err(_) => Err(FS_ERR_IO),
        };

        match handle {
            Ok(h) => ok_i32(h),
            Err(code) => err_i32(code),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_read_some_v1(
    reader_handle: i32,
    max_bytes: i32,
) -> ev_result_bytes {
    std::panic::catch_unwind(|| {
        if max_bytes <= 0 {
            return ok_bytes_vec(Vec::new());
        }

        let Ok(mut table) = readers().lock() else {
            return err_bytes(FS_ERR_IO);
        };
        let Some(idx) = handle_idx(reader_handle) else {
            return err_bytes(FS_ERR_BAD_HANDLE);
        };
        let Some(r) = table.get_mut(idx).and_then(|v| v.as_mut()) else {
            return err_bytes(FS_ERR_BAD_HANDLE);
        };
        let Some(f) = r.file.as_mut() else {
            return ok_bytes_vec(Vec::new());
        };

        let Some(rem) = r.max_read_bytes.checked_sub(r.read) else {
            r.file = None;
            return err_bytes(FS_ERR_TOO_LARGE);
        };
        if rem == 0 {
            r.file = None;
            return ok_bytes_vec(Vec::new());
        }

        let want = (max_bytes as u32).min(rem);
        let mut buf: Vec<u8> = vec![0u8; want as usize];
        let got = match f.read(&mut buf) {
            Ok(n) => n,
            Err(e) => return err_bytes(map_io_err(&e)),
        };
        if got == 0 {
            r.file = None;
            return ok_bytes_vec(Vec::new());
        }
        buf.truncate(got);

        r.read = r.read.saturating_add(got as u32);
        ok_bytes_vec(buf)
    })
    .unwrap_or_else(|_| err_bytes(FS_ERR_IO))
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_fs_stream_read_into_v1(
    reader_handle: i32,
    dst_ptr: *mut u8,
    dst_cap: u32,
) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        if dst_cap == 0 {
            return ok_i32(0);
        }
        if dst_ptr.is_null() {
            return err_i32(FS_ERR_IO);
        }

        let Ok(mut table) = readers().lock() else {
            return err_i32(FS_ERR_IO);
        };
        let Some(idx) = handle_idx(reader_handle) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };
        let Some(r) = table.get_mut(idx).and_then(|v| v.as_mut()) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };
        let Some(f) = r.file.as_mut() else {
            return ok_i32(0);
        };

        let Some(rem) = r.max_read_bytes.checked_sub(r.read) else {
            r.file = None;
            return err_i32(FS_ERR_TOO_LARGE);
        };
        if rem == 0 {
            r.file = None;
            return ok_i32(0);
        }
        let cap = dst_cap.min(rem);
        let dst = core::slice::from_raw_parts_mut(dst_ptr, cap as usize);
        let got = match f.read(dst) {
            Ok(n) => n,
            Err(e) => return err_i32(map_io_err(&e)),
        };
        if got == 0 {
            r.file = None;
            return ok_i32(0);
        }
        r.read = r.read.saturating_add(got as u32);
        if got > (i32::MAX as usize) {
            return err_i32(FS_ERR_UNSUPPORTED);
        }
        ok_i32(got as i32)
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_close_read_v1(reader_handle: i32) -> ev_result_i32 {
    std::panic::catch_unwind(|| {
        let Ok(mut table) = readers().lock() else {
            return err_i32(FS_ERR_IO);
        };
        let Some(idx) = handle_idx(reader_handle) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };
        let Some(r) = table.get_mut(idx).and_then(|v| v.as_mut()) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };

        // Idempotent close.
        let Some(f) = r.file.take() else {
            return ok_i32(1);
        };
        drop(f);
        ok_i32(1)
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_drop_read_v1(reader_handle: i32) -> i32 {
    std::panic::catch_unwind(|| {
        let Ok(mut table) = readers().lock() else {
            return 1;
        };
        let Some(idx) = handle_idx(reader_handle) else {
            return 1;
        };
        let Some(r) = table.get_mut(idx).and_then(|v| v.take()) else {
            return 1;
        };
        drop(r.file);
        1
    })
    .unwrap_or(1)
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_mkdirs_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if !pol.allow_mkdir {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };
        match std::fs::create_dir_all(&pb) {
            Ok(()) => ok_i32(1),
            Err(e) => err_i32(map_io_err(&e)),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_remove_file_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if !pol.allow_remove {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        match std::fs::metadata(&pb) {
            Ok(m) => {
                if m.is_dir() {
                    return err_i32(FS_ERR_IS_DIR);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => return err_i32(FS_ERR_NOT_FOUND),
            Err(e) => return err_i32(map_io_err(&e)),
        }

        match std::fs::remove_file(&pb) {
            Ok(()) => ok_i32(1),
            Err(e) => err_i32(map_io_err(&e)),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_remove_dir_all_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if !pol.allow_remove {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        match std::fs::metadata(&pb) {
            Ok(m) => {
                if !m.is_dir() {
                    return err_i32(FS_ERR_NOT_DIR);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => return err_i32(FS_ERR_NOT_FOUND),
            Err(e) => return err_i32(map_io_err(&e)),
        }

        match std::fs::remove_dir_all(&pb) {
            Ok(()) => ok_i32(1),
            Err(e) => err_i32(map_io_err(&e)),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_rename_v1(
    src: ev_bytes,
    dst: ev_bytes,
    caps: ev_bytes,
) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if !pol.allow_rename {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        let src_bytes = bytes_as_slice(src);
        let dst_bytes = bytes_as_slice(dst);
        let src_pb = match enforce_write_path(caps, src_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };
        let dst_pb = match enforce_write_path(caps, dst_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        match std::fs::rename(&src_pb, &dst_pb) {
            Ok(()) => ok_i32(1),
            Err(e) => err_i32(map_io_err(&e)),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_list_dir_sorted_text_v1(
    path: ev_bytes,
    caps: ev_bytes,
) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };

        let pol = policy();
        if !pol.allow_walk {
            return err_bytes(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_bytes(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_read_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_bytes(code),
        };

        let md = match std::fs::metadata(&pb) {
            Ok(m) => m,
            Err(e) => return err_bytes(map_io_err(&e)),
        };
        if !md.is_dir() {
            return err_bytes(FS_ERR_NOT_DIR);
        }

        let max = effective_max(pol.max_entries, caps.max_entries) as usize;
        let mut names: Vec<String> = Vec::new();

        let rd = match std::fs::read_dir(&pb) {
            Ok(r) => r,
            Err(e) => return err_bytes(map_io_err(&e)),
        };
        for ent in rd {
            let ent = match ent {
                Ok(e) => e,
                Err(e) => return err_bytes(map_io_err(&e)),
            };
            let Ok(name) = ent.file_name().into_string() else {
                continue;
            };
            if pol.deny_hidden && name.starts_with('.') && !cap_allow_hidden(caps) {
                continue;
            }
            names.push(name);
            if names.len() > max {
                return err_bytes(FS_ERR_TOO_MANY_ENTRIES);
            }
        }

        ok_bytes_vec(join_lines_sorted(names))
    })
    .unwrap_or_else(|_| err_bytes(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_walk_glob_sorted_text_v1(
    root: ev_bytes,
    glob: ev_bytes,
    caps: ev_bytes,
) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };

        let pol = policy();
        if !pol.allow_walk || !pol.allow_glob {
            return err_bytes(FS_ERR_POLICY_DENY);
        }

        let root_b = bytes_as_slice(root);
        let root_pb = match enforce_read_path(caps, root_b) {
            Ok(p) => p,
            Err(code) => return err_bytes(code),
        };

        let md = match std::fs::metadata(&root_pb) {
            Ok(m) => m,
            Err(e) => return err_bytes(map_io_err(&e)),
        };
        if !md.is_dir() {
            return err_bytes(FS_ERR_NOT_DIR);
        }

        let glob_b = bytes_as_slice(glob);
        let glob_s = match bytes_to_utf8(glob_b) {
            Ok(s) => s,
            Err(code) => return err_bytes(code),
        };
        let matcher = match build_glob_matcher(glob_s) {
            Ok(m) => m,
            Err(code) => return err_bytes(code),
        };

        let follow_links = cap_allow_symlinks(caps) && pol.allow_symlinks;
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_bytes(FS_ERR_SYMLINK_DENIED);
        }

        let max_entries = effective_max(pol.max_entries, caps.max_entries) as usize;
        let max_depth = effective_max(pol.max_depth, caps.max_depth) as usize;

        let walker = WalkDir::new(&root_pb)
            .follow_links(follow_links)
            .max_depth(max_depth.saturating_add(1));

        let mut out: Vec<String> = Vec::new();

        for ent in walker {
            let ent = match ent {
                Ok(e) => e,
                Err(_) => return err_bytes(FS_ERR_IO),
            };
            if ent.depth() > max_depth {
                return err_bytes(FS_ERR_DEPTH_EXCEEDED);
            }
            if ent.file_type().is_dir() {
                continue;
            }
            let rel = match ent.path().strip_prefix(&root_pb) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let Some(rel_s) = rel.to_str() else {
                continue;
            };
            let rel_s = rel_s.replace('\\', "/");
            if pol.deny_hidden
                && !cap_allow_hidden(caps)
                && rel_s.split('/').any(|s| s.starts_with('.'))
            {
                continue;
            }
            if matcher.is_match(rel_s.as_str()) {
                out.push(rel_s);
                if out.len() > max_entries {
                    return err_bytes(FS_ERR_TOO_MANY_ENTRIES);
                }
            }
        }

        ok_bytes_vec(join_lines_sorted(out))
    })
    .unwrap_or_else(|_| err_bytes(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stat_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };

        let pol = policy();
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_bytes(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_read_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_bytes(code),
        };

        let md = match std::fs::symlink_metadata(&pb) {
            Ok(m) => m,
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    let mut stat = vec![0u8; 16];
                    stat[0..4].copy_from_slice(&1u32.to_le_bytes()); // version
                    stat[4..8].copy_from_slice(&0u32.to_le_bytes()); // kind=0 missing
                    return ok_bytes_vec(stat);
                }
                return err_bytes(map_io_err(&e));
            }
        };

        let ft = md.file_type();
        let kind: u32 = if ft.is_file() {
            1
        } else if ft.is_dir() {
            2
        } else if ft.is_symlink() {
            3
        } else {
            4
        };
        let size: u32 = if ft.is_file() {
            md.len().min(u32::MAX as u64) as u32
        } else {
            0
        };
        let mtime_s: u32 = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs().min(u32::MAX as u64) as u32)
            .unwrap_or(0);

        let mut stat = vec![0u8; 16];
        stat[0..4].copy_from_slice(&1u32.to_le_bytes());
        stat[4..8].copy_from_slice(&kind.to_le_bytes());
        stat[8..12].copy_from_slice(&size.to_le_bytes());
        stat[12..16].copy_from_slice(&mtime_s.to_le_bytes());
        ok_bytes_vec(stat)
    })
    .unwrap_or_else(|_| err_bytes(FS_ERR_IO))
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn caps_v1(max_write_bytes: u32, flags: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(24);
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // max_read_bytes
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

    fn ok_i32(res: ev_result_i32) -> i32 {
        assert_eq!(res.tag, 1, "expected ok, got err={}", unsafe {
            res.payload.err
        });
        unsafe { res.payload.ok as i32 }
    }

    fn err_i32(res: ev_result_i32) -> i32 {
        assert_eq!(res.tag, 0, "expected err");
        unsafe { res.payload.err as i32 }
    }

    fn ok_bytes(res: ev_result_bytes) -> Vec<u8> {
        assert_eq!(res.tag, 1, "expected ok, got err={}", unsafe {
            res.payload.err
        });
        let out = unsafe { res.payload.ok };
        unsafe { std::slice::from_raw_parts(out.ptr, out.len as usize).to_vec() }
    }

    fn err_bytes(res: ev_result_bytes) -> i32 {
        assert_eq!(res.tag, 0, "expected err");
        unsafe { res.payload.err as i32 }
    }

    fn caps_read_v1(max_read_bytes: u32, flags: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(24);
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&max_read_bytes.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // max_write_bytes
        out.extend_from_slice(&0u32.to_le_bytes()); // max_entries
        out.extend_from_slice(&0u32.to_le_bytes()); // max_depth
        out.extend_from_slice(&flags.to_le_bytes());
        out
    }

    #[test]
    fn fs_stream_writer_handle_v1_smoke() {
        std::env::set_var("X07_OS_SANDBOXED", "0");
        std::env::set_var("X07_OS_FS", "1");
        std::env::set_var("X07_OS_FS_ALLOW_MKDIR", "1");
        std::env::set_var("X07_OS_FS_ALLOW_RENAME", "1");
        std::env::set_var("X07_OS_FS_MAX_WRITE_BYTES", "1000000");

        let root = format!("target/x07_ext_fs_stream_test_{}", std::process::id());
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create test dir");

        // Non-atomic writer, max_write_bytes enforced cumulatively.
        let out_path = format!("{root}/out.txt");
        let caps = caps_v1(8, CAP_CREATE_PARENTS | CAP_OVERWRITE);
        let h = ok_i32(x07_ext_fs_stream_open_write_v1(
            to_ev_bytes(out_path.as_bytes()),
            to_ev_bytes(&caps),
        ));
        assert!(h > 0);
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h, to_ev_bytes(b"abc"))),
            3
        );
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h, to_ev_bytes(b"def"))),
            3
        );
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h, to_ev_bytes(b"gh"))),
            2
        );
        assert_eq!(
            err_i32(x07_ext_fs_stream_write_all_v1(h, to_ev_bytes(b"i"))),
            FS_ERR_TOO_LARGE
        );
        assert_eq!(ok_i32(x07_ext_fs_stream_close_v1(h)), 1);
        assert_eq!(x07_ext_fs_stream_drop_v1(h), 1);

        let got = std::fs::read(&out_path).expect("read out.txt");
        assert_eq!(got, b"abcdefgh");

        // Atomic writer commits on close.
        let atomic_path = format!("{root}/atomic.txt");
        let caps_atomic = caps_v1(1024, CAP_CREATE_PARENTS | CAP_OVERWRITE | CAP_ATOMIC_WRITE);
        let h2 = ok_i32(x07_ext_fs_stream_open_write_v1(
            to_ev_bytes(atomic_path.as_bytes()),
            to_ev_bytes(&caps_atomic),
        ));
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h2, to_ev_bytes(b"hi"))),
            2
        );
        assert_eq!(ok_i32(x07_ext_fs_stream_close_v1(h2)), 1);
        assert_eq!(x07_ext_fs_stream_drop_v1(h2), 1);
        let got2 = std::fs::read(&atomic_path).expect("read atomic.txt");
        assert_eq!(got2, b"hi");

        // Dropping without close should clean up the tmp file and not create the final path.
        let atomic_drop_path = format!("{root}/atomic_drop.txt");
        let h3 = ok_i32(x07_ext_fs_stream_open_write_v1(
            to_ev_bytes(atomic_drop_path.as_bytes()),
            to_ev_bytes(&caps_atomic),
        ));
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h3, to_ev_bytes(b"x"))),
            1
        );
        assert_eq!(x07_ext_fs_stream_drop_v1(h3), 1);
        assert_eq!(err_i32(x07_ext_fs_stream_close_v1(h3)), FS_ERR_BAD_HANDLE);
        assert!(!Path::new(&atomic_drop_path).exists());
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.starts_with("atomic_drop.txt.x07_tmp_"))
            .collect();
        assert_eq!(leftovers, Vec::<String>::new());

        // Invalid handle errors.
        assert_eq!(
            err_i32(x07_ext_fs_stream_write_all_v1(123, to_ev_bytes(b"z"))),
            FS_ERR_BAD_HANDLE
        );
        assert_eq!(err_i32(x07_ext_fs_stream_close_v1(123)), FS_ERR_BAD_HANDLE);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fs_stream_reader_handle_v1_smoke() {
        std::env::set_var("X07_OS_SANDBOXED", "0");
        std::env::set_var("X07_OS_FS", "1");
        std::env::set_var("X07_OS_FS_MAX_READ_BYTES", "1000000");

        let root = format!("target/x07_ext_fs_stream_read_test_{}", std::process::id());
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create test dir");

        let in_path = format!("{root}/in.txt");
        std::fs::write(&in_path, b"abcdefgh").expect("write in.txt");

        let caps = caps_read_v1(8, 0);
        let h = ok_i32(x07_ext_fs_stream_open_read_v1(
            to_ev_bytes(in_path.as_bytes()),
            to_ev_bytes(&caps),
        ));
        assert!(h > 0);

        assert_eq!(
            ok_bytes(x07_ext_fs_stream_read_some_v1(h, 3)),
            b"abc".to_vec()
        );
        assert_eq!(
            ok_bytes(x07_ext_fs_stream_read_some_v1(h, 3)),
            b"def".to_vec()
        );
        assert_eq!(
            ok_bytes(x07_ext_fs_stream_read_some_v1(h, 3)),
            b"gh".to_vec()
        );
        assert_eq!(
            ok_bytes(x07_ext_fs_stream_read_some_v1(h, 3)),
            Vec::<u8>::new()
        );
        assert_eq!(ok_i32(x07_ext_fs_stream_close_read_v1(h)), 1);
        assert_eq!(x07_ext_fs_stream_drop_read_v1(h), 1);

        // read_into variant (no allocations).
        let h2 = ok_i32(x07_ext_fs_stream_open_read_v1(
            to_ev_bytes(in_path.as_bytes()),
            to_ev_bytes(&caps),
        ));
        let mut tmp = vec![0u8; 3];
        assert_eq!(
            ok_i32(unsafe {
                x07_ext_fs_stream_read_into_v1(h2, tmp.as_mut_ptr(), tmp.len() as u32)
            }),
            3
        );
        assert_eq!(tmp, b"abc");
        assert_eq!(ok_i32(x07_ext_fs_stream_close_read_v1(h2)), 1);
        assert_eq!(x07_ext_fs_stream_drop_read_v1(h2), 1);

        // Too-large files are rejected at open.
        let too_big_path = format!("{root}/too_big.txt");
        std::fs::write(&too_big_path, b"abcdefghi").expect("write too_big.txt");
        assert_eq!(
            err_i32(x07_ext_fs_stream_open_read_v1(
                to_ev_bytes(too_big_path.as_bytes()),
                to_ev_bytes(&caps),
            )),
            FS_ERR_TOO_LARGE
        );

        // Invalid handle errors.
        assert_eq!(
            err_bytes(x07_ext_fs_stream_read_some_v1(123, 1)),
            FS_ERR_BAD_HANDLE
        );
        assert_eq!(
            err_i32(x07_ext_fs_stream_close_read_v1(123)),
            FS_ERR_BAD_HANDLE
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fs_append_all_v1_smoke() {
        std::env::set_var("X07_OS_SANDBOXED", "0");
        std::env::set_var("X07_OS_FS", "1");
        std::env::set_var("X07_OS_FS_ALLOW_MKDIR", "1");
        std::env::set_var("X07_OS_FS_MAX_WRITE_BYTES", "1000000");

        let root = format!("target/x07_ext_fs_append_test_{}", std::process::id());
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create test dir");

        let out_path = format!("{root}/out.txt");
        let caps = caps_v1(1024, CAP_CREATE_PARENTS);

        assert_eq!(
            ok_i32(x07_ext_fs_append_all_v1(
                to_ev_bytes(out_path.as_bytes()),
                to_ev_bytes(b"abc"),
                to_ev_bytes(&caps),
            )),
            3
        );
        assert_eq!(
            ok_i32(x07_ext_fs_append_all_v1(
                to_ev_bytes(out_path.as_bytes()),
                to_ev_bytes(b"def"),
                to_ev_bytes(&caps),
            )),
            3
        );
        let got = std::fs::read(&out_path).expect("read out.txt");
        assert_eq!(got, b"abcdef");

        // max_write_bytes enforced per call.
        let caps_small = caps_v1(2, CAP_CREATE_PARENTS);
        assert_eq!(
            err_i32(x07_ext_fs_append_all_v1(
                to_ev_bytes(out_path.as_bytes()),
                to_ev_bytes(b"xyz"),
                to_ev_bytes(&caps_small),
            )),
            FS_ERR_TOO_LARGE
        );

        // Atomic write is not supported for append.
        let caps_atomic = caps_v1(1024, CAP_CREATE_PARENTS | CAP_ATOMIC_WRITE);
        assert_eq!(
            err_i32(x07_ext_fs_append_all_v1(
                to_ev_bytes(out_path.as_bytes()),
                to_ev_bytes(b"z"),
                to_ev_bytes(&caps_atomic),
            )),
            FS_ERR_UNSUPPORTED
        );

        // Directory paths are rejected.
        let dir_path = format!("{root}/dir");
        std::fs::create_dir_all(&dir_path).expect("create dir");
        assert_eq!(
            err_i32(x07_ext_fs_append_all_v1(
                to_ev_bytes(dir_path.as_bytes()),
                to_ev_bytes(b"x"),
                to_ev_bytes(&caps),
            )),
            FS_ERR_IS_DIR
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fs_read_write_v1_accept_absolute_paths_in_run_os() {
        std::env::set_var("X07_OS_SANDBOXED", "0");
        std::env::set_var("X07_OS_FS", "1");
        std::env::set_var("X07_OS_FS_ALLOW_MKDIR", "1");
        std::env::set_var("X07_OS_FS_MAX_WRITE_BYTES", "1000000");

        let root = std::env::temp_dir().join(format!("x07_ext_fs_abs_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        let out_path = root.join("nested").join("out.txt");
        let out_path_s = out_path.to_str().expect("utf8 temp path");
        let caps = caps_v1(1024, CAP_CREATE_PARENTS | CAP_OVERWRITE);

        assert_eq!(
            ok_i32(x07_ext_fs_write_all_v1(
                to_ev_bytes(out_path_s.as_bytes()),
                to_ev_bytes(b"ok"),
                to_ev_bytes(&caps),
            )),
            2
        );
        assert_eq!(std::fs::read(&out_path).expect("read out.txt"), b"ok");

        let read_back = ok_bytes(x07_ext_fs_read_all_v1(
            to_ev_bytes(out_path_s.as_bytes()),
            to_ev_bytes(&caps),
        ));
        assert_eq!(read_back, b"ok");

        let _ = std::fs::remove_dir_all(&root);
    }
}
