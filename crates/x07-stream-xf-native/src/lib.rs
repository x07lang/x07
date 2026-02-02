#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use core::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_bytes {
    pub ptr: *mut u8,
    pub len: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct x07_bytes_view_v1 {
    pub ptr: *const u8,
    pub len: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct x07_out_buf_v1 {
    pub ptr: *mut u8,
    pub cap: u32,
    pub len: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct x07_scratch_v1 {
    pub ptr: *mut u8,
    pub cap: u32,
    pub used: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct x07_xf_budget_v1 {
    pub max_out_bytes_per_step: u32,
    pub max_out_items_per_step: u32,
    pub max_out_buf_bytes: u32,
    pub max_state_bytes: u32,
    pub max_cfg_bytes: u32,
    pub max_scratch_bytes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct x07_xf_emit_v1 {
    pub emit_ctx: *mut c_void,
    pub emit_alloc: Option<unsafe extern "C" fn(*mut c_void, u32, *mut x07_out_buf_v1) -> i32>,
    pub emit_commit: Option<unsafe extern "C" fn(*mut c_void, *const x07_out_buf_v1) -> i32>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct x07_stream_xf_plugin_v1 {
    pub abi_tag: u32,
    pub abi_version: u32,
    pub plugin_id: *const c_char,
    pub flags: u32,
    pub in_item_brand: *const c_char,
    pub out_item_brand: *const c_char,
    pub state_size: u32,
    pub state_align: u32,
    pub scratch_hint: u32,
    pub scratch_max: u32,
    pub init: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *mut x07_scratch_v1,
            x07_bytes_view_v1,
            x07_xf_emit_v1,
            x07_xf_budget_v1,
        ) -> i32,
    >,
    pub step: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *mut x07_scratch_v1,
            x07_bytes_view_v1,
            x07_xf_emit_v1,
            x07_xf_budget_v1,
        ) -> i32,
    >,
    pub flush: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *mut x07_scratch_v1,
            x07_xf_emit_v1,
            x07_xf_budget_v1,
        ) -> i32,
    >,
    pub drop: Option<unsafe extern "C" fn(*mut c_void)>,
}

unsafe impl Sync for x07_stream_xf_plugin_v1 {}

extern "C" {
    fn ev_bytes_alloc(len: u32) -> ev_bytes;
    fn ev_trap(code: i32) -> !;

    // Provided by the generated C runtime when JSON JCS support is linked in.
    fn x07_json_jcs_canon_doc_v1(
        input_ptr: *const u8,
        input_len: u32,
        max_depth: i32,
        max_object_members: i32,
        max_object_total_bytes: i32,
    ) -> ev_bytes;
}

const X07_XF_ABI_TAG_X7XF: u32 = 0x4658_4637;
const X07_XF_ABI_VERSION: u32 = 1;

const X07_XF_FLAG_DETERMINISTIC_ONLY: u32 = 1u32 << 0;

// Stream pipe error codes (mirrored from x07c stream_pipe.rs).
const E_CFG_INVALID: i32 = 1;
const E_BUDGET_IN_BYTES: i32 = 2;
const E_LINE_TOO_LONG: i32 = 5;
const E_FRAME_TOO_LARGE: i32 = 10;

const E_DEFRAME_FRAME_TOO_LARGE: i32 = 80;
const E_DEFRAME_TRUNCATED: i32 = 81;
const E_DEFRAME_EMPTY_FORBIDDEN: i32 = 82;
const E_DEFRAME_MAX_FRAMES: i32 = 83;

const TRAP_INTERNAL: i32 = 9200;

const JSON_CANON_ERR_NONE: u32 = 0;
const JSON_CANON_ERR_INPUT_TOO_LARGE: u32 = 1;
const JSON_CANON_ERR_MAX_TOTAL_JSON_BYTES: u32 = 2;

#[inline]
unsafe fn alloc_bytes_exact(len: u32) -> ev_bytes {
    let out = ev_bytes_alloc(len);
    if out.len != len {
        ev_trap(TRAP_INTERNAL);
    }
    out
}

#[inline]
unsafe fn emit_alloc(emit: x07_xf_emit_v1, cap: u32, out: *mut x07_out_buf_v1) -> i32 {
    match emit.emit_alloc {
        Some(f) => f(emit.emit_ctx, cap, out),
        None => ev_trap(TRAP_INTERNAL),
    }
}

#[inline]
unsafe fn emit_commit(emit: x07_xf_emit_v1, out: *const x07_out_buf_v1) -> i32 {
    match emit.emit_commit {
        Some(f) => f(emit.emit_ctx, out),
        None => ev_trap(TRAP_INTERNAL),
    }
}

#[inline]
unsafe fn view_as_slice<'a>(v: x07_bytes_view_v1) -> &'a [u8] {
    core::slice::from_raw_parts(v.ptr, v.len as usize)
}

#[inline]
unsafe fn write_u32_le(dst: *mut u8, x: u32) {
    let b = x.to_le_bytes();
    core::ptr::copy_nonoverlapping(b.as_ptr(), dst, 4);
}

#[inline]
unsafe fn read_i32_le(v: x07_bytes_view_v1, off: usize) -> Option<i32> {
    let s = view_as_slice(v);
    let bytes = s.get(off..off + 4)?;
    let mut arr = [0u8; 4];
    arr.copy_from_slice(bytes);
    Some(i32::from_le_bytes(arr))
}

// --- xf.frame_u32le_v1 ---

unsafe extern "C" fn xf_frame_u32le_init_v1(
    _state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    cfg: x07_bytes_view_v1,
    _emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    if cfg.len != 0 {
        return -E_CFG_INVALID;
    }
    0
}

unsafe extern "C" fn xf_frame_u32le_step_v1(
    _state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    input: x07_bytes_view_v1,
    emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    if input.len > (i32::MAX as u32) {
        return -E_FRAME_TOO_LARGE;
    }

    let cap = input.len.saturating_add(4);
    let mut out_buf = x07_out_buf_v1 {
        ptr: core::ptr::null_mut(),
        cap: 0,
        len: 0,
    };
    let rc = emit_alloc(emit, cap, &mut out_buf as *mut x07_out_buf_v1);
    if rc != 0 {
        return rc;
    }
    if out_buf.cap != cap {
        ev_trap(TRAP_INTERNAL);
    }

    write_u32_le(out_buf.ptr, input.len);
    if input.len != 0 {
        core::ptr::copy_nonoverlapping(input.ptr, out_buf.ptr.add(4), input.len as usize);
    }
    out_buf.len = cap;

    let rc = emit_commit(emit, &out_buf as *const x07_out_buf_v1);
    if rc != 0 {
        return rc;
    }
    0
}

unsafe extern "C" fn xf_frame_u32le_flush_v1(
    _state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    _emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    0
}

// --- xf.split_lines_v1 ---

#[repr(C)]
struct SplitLinesStateV1 {
    delim: i32,
    max_line_bytes: u32,
    carry_ptr: *mut u8,
    carry_len: u32,
    _pad: u32,
}

unsafe extern "C" fn xf_split_lines_init_v1(
    state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    cfg: x07_bytes_view_v1,
    _emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    if cfg.len != 8 {
        return -E_CFG_INVALID;
    }

    let Some(delim) = read_i32_le(cfg, 0) else {
        return -E_CFG_INVALID;
    };
    let Some(max_line_i32) = read_i32_le(cfg, 4) else {
        return -E_CFG_INVALID;
    };
    if max_line_i32 <= 0 {
        return -E_CFG_INVALID;
    }
    let max_line_bytes = max_line_i32 as u32;

    let carry = alloc_bytes_exact(max_line_bytes);

    let st = &mut *(state as *mut SplitLinesStateV1);
    *st = SplitLinesStateV1 {
        delim,
        max_line_bytes,
        carry_ptr: carry.ptr,
        carry_len: 0,
        _pad: 0,
    };
    0
}

unsafe extern "C" fn xf_split_lines_step_v1(
    state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    input: x07_bytes_view_v1,
    emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    let st = &mut *(state as *mut SplitLinesStateV1);
    let s = view_as_slice(input);
    let mut start: usize = 0;

    for (i, &b) in s.iter().enumerate() {
        if (b as i32) != st.delim {
            continue;
        }

        let seg_len = i - start;
        let total = (st.carry_len as usize).saturating_add(seg_len);
        if total > (st.max_line_bytes as usize) {
            return -E_LINE_TOO_LONG;
        }

        let mut out_buf = x07_out_buf_v1 {
            ptr: core::ptr::null_mut(),
            cap: 0,
            len: 0,
        };
        let rc = emit_alloc(emit, total as u32, &mut out_buf as *mut x07_out_buf_v1);
        if rc != 0 {
            return rc;
        }
        if out_buf.cap != total as u32 {
            ev_trap(TRAP_INTERNAL);
        }

        if st.carry_len != 0 {
            core::ptr::copy_nonoverlapping(st.carry_ptr, out_buf.ptr, st.carry_len as usize);
        }
        if seg_len != 0 {
            core::ptr::copy_nonoverlapping(
                input.ptr.add(start),
                out_buf.ptr.add(st.carry_len as usize),
                seg_len,
            );
        }

        out_buf.len = total as u32;
        let rc = emit_commit(emit, &out_buf as *const x07_out_buf_v1);
        if rc != 0 {
            return rc;
        }

        st.carry_len = 0;
        start = i + 1;
    }

    let tail_len = s.len().saturating_sub(start);
    if tail_len != 0 {
        let total = (st.carry_len as usize).saturating_add(tail_len);
        if total > (st.max_line_bytes as usize) {
            return -E_LINE_TOO_LONG;
        }
        core::ptr::copy_nonoverlapping(
            input.ptr.add(start),
            st.carry_ptr.add(st.carry_len as usize),
            tail_len,
        );
        st.carry_len = total as u32;
    }

    0
}

unsafe extern "C" fn xf_split_lines_flush_v1(
    state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    let st = &mut *(state as *mut SplitLinesStateV1);
    if st.carry_len == 0 {
        return 0;
    }

    let mut out_buf = x07_out_buf_v1 {
        ptr: core::ptr::null_mut(),
        cap: 0,
        len: 0,
    };
    let rc = emit_alloc(emit, st.carry_len, &mut out_buf as *mut x07_out_buf_v1);
    if rc != 0 {
        return rc;
    }
    if out_buf.cap != st.carry_len {
        ev_trap(TRAP_INTERNAL);
    }

    core::ptr::copy_nonoverlapping(st.carry_ptr, out_buf.ptr, st.carry_len as usize);
    out_buf.len = st.carry_len;
    let rc = emit_commit(emit, &out_buf as *const x07_out_buf_v1);
    if rc != 0 {
        return rc;
    }

    st.carry_len = 0;
    0
}

// --- xf.deframe_u32le_v1 ---

#[repr(C)]
struct DeframeStateV1 {
    max_frame_bytes: u32,
    max_frames: u32,
    allow_empty: u32,
    on_truncated: u32, // 0 = err, 1 = drop
    frames_emitted: u32,
    hdr_fill: u32,
    need: u32,
    buf_fill: u32,
    hdr: [u8; 4],
    buf_ptr: *mut u8,
}

unsafe extern "C" fn xf_deframe_u32le_init_v1(
    state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    cfg: x07_bytes_view_v1,
    _emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    if cfg.len != 16 {
        return -E_CFG_INVALID;
    }

    let Some(max_frame_bytes) = read_i32_le(cfg, 0) else {
        return -E_CFG_INVALID;
    };
    let Some(max_frames_i32) = read_i32_le(cfg, 4) else {
        return -E_CFG_INVALID;
    };
    let Some(allow_empty_i32) = read_i32_le(cfg, 8) else {
        return -E_CFG_INVALID;
    };
    let Some(on_truncated_i32) = read_i32_le(cfg, 12) else {
        return -E_CFG_INVALID;
    };

    if max_frame_bytes <= 0 {
        return -E_CFG_INVALID;
    }

    let max_frames = if max_frames_i32 > 0 {
        max_frames_i32 as u32
    } else {
        0
    };
    let allow_empty = if allow_empty_i32 == 0 { 0u32 } else { 1u32 };

    let on_truncated = match on_truncated_i32 {
        0 => 0u32,
        1 => 1u32,
        _ => return -E_CFG_INVALID,
    };

    let buf = alloc_bytes_exact(max_frame_bytes as u32);

    let st = &mut *(state as *mut DeframeStateV1);
    *st = DeframeStateV1 {
        max_frame_bytes: max_frame_bytes as u32,
        max_frames,
        allow_empty,
        on_truncated,
        frames_emitted: 0,
        hdr_fill: 0,
        need: 0,
        buf_fill: 0,
        hdr: [0u8; 4],
        buf_ptr: buf.ptr,
    };

    0
}

unsafe extern "C" fn xf_deframe_u32le_step_v1(
    state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    input: x07_bytes_view_v1,
    emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    let st = &mut *(state as *mut DeframeStateV1);
    let s = view_as_slice(input);

    let mut i: usize = 0;
    while i < s.len() {
        if st.hdr_fill < 4 {
            st.hdr[st.hdr_fill as usize] = s[i];
            st.hdr_fill += 1;
            i += 1;

            if st.hdr_fill == 4 {
                let need_u32 = u32::from_le_bytes(st.hdr);
                if need_u32 > (i32::MAX as u32) {
                    return -E_DEFRAME_FRAME_TOO_LARGE;
                }
                if need_u32 > st.max_frame_bytes {
                    return -E_DEFRAME_FRAME_TOO_LARGE;
                }
                if need_u32 == 0 {
                    if st.allow_empty == 0 {
                        return -E_DEFRAME_EMPTY_FORBIDDEN;
                    }
                    // Emit empty frame.
                    if st.max_frames != 0 {
                        let new_frames = st.frames_emitted.saturating_add(1);
                        if new_frames > st.max_frames {
                            return -E_DEFRAME_MAX_FRAMES;
                        }
                        st.frames_emitted = new_frames;
                    } else {
                        st.frames_emitted = st.frames_emitted.saturating_add(1);
                    }

                    let mut out_buf = x07_out_buf_v1 {
                        ptr: core::ptr::null_mut(),
                        cap: 0,
                        len: 0,
                    };
                    let rc = emit_alloc(emit, 0, &mut out_buf as *mut x07_out_buf_v1);
                    if rc != 0 {
                        return rc;
                    }
                    out_buf.len = 0;
                    let rc = emit_commit(emit, &out_buf as *const x07_out_buf_v1);
                    if rc != 0 {
                        return rc;
                    }

                    st.hdr_fill = 0;
                    st.need = 0;
                    st.buf_fill = 0;
                } else {
                    st.need = need_u32;
                    st.buf_fill = 0;
                }
            }
            continue;
        }

        // READ_PAYLOAD
        let need = st.need as usize;
        if need == 0 {
            // Should be impossible: hdr_fill==4 implies need was set or was empty frame emitted.
            return -E_CFG_INVALID;
        }

        st.buf_ptr.add(st.buf_fill as usize).write(s[i]);
        st.buf_fill += 1;
        i += 1;

        if st.buf_fill == st.need {
            if st.max_frames != 0 {
                let new_frames = st.frames_emitted.saturating_add(1);
                if new_frames > st.max_frames {
                    return -E_DEFRAME_MAX_FRAMES;
                }
                st.frames_emitted = new_frames;
            } else {
                st.frames_emitted = st.frames_emitted.saturating_add(1);
            }

            let mut out_buf = x07_out_buf_v1 {
                ptr: core::ptr::null_mut(),
                cap: 0,
                len: 0,
            };
            let rc = emit_alloc(emit, st.need, &mut out_buf as *mut x07_out_buf_v1);
            if rc != 0 {
                return rc;
            }
            if out_buf.cap != st.need {
                ev_trap(TRAP_INTERNAL);
            }
            core::ptr::copy_nonoverlapping(st.buf_ptr, out_buf.ptr, st.need as usize);
            out_buf.len = st.need;
            let rc = emit_commit(emit, &out_buf as *const x07_out_buf_v1);
            if rc != 0 {
                return rc;
            }

            st.hdr_fill = 0;
            st.need = 0;
            st.buf_fill = 0;
        }
    }

    0
}

unsafe extern "C" fn xf_deframe_u32le_flush_v1(
    state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    _emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    let st = &mut *(state as *mut DeframeStateV1);
    if st.hdr_fill == 0 && st.buf_fill == 0 {
        return 0;
    }

    if st.on_truncated != 0 {
        st.hdr_fill = 0;
        st.need = 0;
        st.buf_fill = 0;
        return 0;
    }

    -E_DEFRAME_TRUNCATED
}

// --- xf.json_canon_stream_v1 ---

#[repr(C)]
struct JsonCanonStateV1 {
    max_depth: i32,
    max_total_json_bytes: u32,
    max_object_members: i32,
    max_object_total_bytes: i32,
    emit_chunk_max_bytes: u32,
    last_err_kind: u32,
    last_err_off: u32,
    buf_ptr: *mut u8,
    buf_fill: u32,
}

unsafe extern "C" fn xf_json_canon_stream_init_v1(
    state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    cfg: x07_bytes_view_v1,
    _emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    if cfg.len != 20 {
        return -E_CFG_INVALID;
    }

    let Some(max_depth) = read_i32_le(cfg, 0) else {
        return -E_CFG_INVALID;
    };
    let Some(max_total_json_bytes) = read_i32_le(cfg, 4) else {
        return -E_CFG_INVALID;
    };
    let Some(max_object_members) = read_i32_le(cfg, 8) else {
        return -E_CFG_INVALID;
    };
    let Some(max_object_total_bytes) = read_i32_le(cfg, 12) else {
        return -E_CFG_INVALID;
    };
    let Some(emit_chunk_max_bytes) = read_i32_le(cfg, 16) else {
        return -E_CFG_INVALID;
    };

    if max_depth <= 0
        || max_total_json_bytes <= 0
        || max_object_members <= 0
        || max_object_total_bytes <= 0
        || emit_chunk_max_bytes <= 0
    {
        return -E_CFG_INVALID;
    }

    let buf = alloc_bytes_exact(max_total_json_bytes as u32);
    let st = &mut *(state as *mut JsonCanonStateV1);
    *st = JsonCanonStateV1 {
        max_depth,
        max_total_json_bytes: max_total_json_bytes as u32,
        max_object_members,
        max_object_total_bytes,
        emit_chunk_max_bytes: emit_chunk_max_bytes as u32,
        last_err_kind: JSON_CANON_ERR_NONE,
        last_err_off: 0,
        buf_ptr: buf.ptr,
        buf_fill: 0,
    };
    0
}

unsafe extern "C" fn xf_json_canon_stream_step_v1(
    state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    input: x07_bytes_view_v1,
    _emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    let st = &mut *(state as *mut JsonCanonStateV1);
    st.last_err_kind = JSON_CANON_ERR_NONE;
    st.last_err_off = 0;
    if input.len > (i32::MAX as u32) {
        st.last_err_kind = JSON_CANON_ERR_INPUT_TOO_LARGE;
        return -E_BUDGET_IN_BYTES;
    }
    let new_len = st.buf_fill.saturating_add(input.len);
    if new_len > st.max_total_json_bytes {
        st.last_err_kind = JSON_CANON_ERR_MAX_TOTAL_JSON_BYTES;
        return -E_BUDGET_IN_BYTES;
    }
    if input.len != 0 {
        core::ptr::copy_nonoverlapping(
            input.ptr,
            st.buf_ptr.add(st.buf_fill as usize),
            input.len as usize,
        );
    }
    st.buf_fill = new_len;
    0
}

unsafe extern "C" fn xf_json_canon_stream_flush_v1(
    state: *mut c_void,
    _scratch: *mut x07_scratch_v1,
    emit: x07_xf_emit_v1,
    _budget: x07_xf_budget_v1,
) -> i32 {
    let st = &mut *(state as *mut JsonCanonStateV1);
    st.last_err_kind = JSON_CANON_ERR_NONE;
    st.last_err_off = 0;

    let doc = x07_json_jcs_canon_doc_v1(
        st.buf_ptr as *const u8,
        st.buf_fill,
        st.max_depth,
        st.max_object_members,
        st.max_object_total_bytes,
    );
    if doc.len < 1 {
        return -E_CFG_INVALID;
    }

    let tag = *doc.ptr;
    if tag == 0 {
        if doc.len < 9 {
            return -E_CFG_INVALID;
        }
        let code_bytes = core::slice::from_raw_parts(doc.ptr.add(1), 4);
        let mut arr = [0u8; 4];
        arr.copy_from_slice(code_bytes);
        let code = u32::from_le_bytes(arr) as i32;
        let off_bytes = core::slice::from_raw_parts(doc.ptr.add(5), 4);
        let mut off_arr = [0u8; 4];
        off_arr.copy_from_slice(off_bytes);
        st.last_err_off = u32::from_le_bytes(off_arr);
        return -code;
    }
    if tag != 1 {
        return -E_CFG_INVALID;
    }

    let canon_ptr = doc.ptr.add(1);
    let canon_len = doc.len - 1;

    let mut pos: u32 = 0;
    while pos < canon_len {
        let remain = canon_len - pos;
        let take = if remain < st.emit_chunk_max_bytes {
            remain
        } else {
            st.emit_chunk_max_bytes
        };

        let mut out_buf = x07_out_buf_v1 {
            ptr: core::ptr::null_mut(),
            cap: 0,
            len: 0,
        };
        let rc = emit_alloc(emit, take, &mut out_buf as *mut x07_out_buf_v1);
        if rc != 0 {
            return rc;
        }
        if out_buf.cap != take {
            ev_trap(TRAP_INTERNAL);
        }
        core::ptr::copy_nonoverlapping(canon_ptr.add(pos as usize), out_buf.ptr, take as usize);
        out_buf.len = take;
        let rc = emit_commit(emit, &out_buf as *const x07_out_buf_v1);
        if rc != 0 {
            return rc;
        }

        pos += take;
    }

    0
}

// --- Exported descriptors ---

#[no_mangle]
pub static x07_xf_frame_u32le_v1: x07_stream_xf_plugin_v1 = x07_stream_xf_plugin_v1 {
    abi_tag: X07_XF_ABI_TAG_X7XF,
    abi_version: X07_XF_ABI_VERSION,
    plugin_id: c"xf.frame_u32le_v1".as_ptr(),
    flags: X07_XF_FLAG_DETERMINISTIC_ONLY,
    in_item_brand: c"any".as_ptr(),
    out_item_brand: c"none".as_ptr(),
    state_size: 32,
    state_align: 8,
    scratch_hint: 0,
    scratch_max: 0,
    init: Some(xf_frame_u32le_init_v1),
    step: Some(xf_frame_u32le_step_v1),
    flush: Some(xf_frame_u32le_flush_v1),
    drop: None,
};

#[no_mangle]
pub static x07_xf_split_lines_v1: x07_stream_xf_plugin_v1 = x07_stream_xf_plugin_v1 {
    abi_tag: X07_XF_ABI_TAG_X7XF,
    abi_version: X07_XF_ABI_VERSION,
    plugin_id: c"xf.split_lines_v1".as_ptr(),
    flags: X07_XF_FLAG_DETERMINISTIC_ONLY,
    in_item_brand: c"any".as_ptr(),
    out_item_brand: c"none".as_ptr(),
    state_size: 32,
    state_align: 8,
    scratch_hint: 0,
    scratch_max: 1048576,
    init: Some(xf_split_lines_init_v1),
    step: Some(xf_split_lines_step_v1),
    flush: Some(xf_split_lines_flush_v1),
    drop: None,
};

#[no_mangle]
pub static x07_xf_deframe_u32le_v1: x07_stream_xf_plugin_v1 = x07_stream_xf_plugin_v1 {
    abi_tag: X07_XF_ABI_TAG_X7XF,
    abi_version: X07_XF_ABI_VERSION,
    plugin_id: c"xf.deframe_u32le_v1".as_ptr(),
    flags: X07_XF_FLAG_DETERMINISTIC_ONLY,
    in_item_brand: c"any".as_ptr(),
    out_item_brand: c"none".as_ptr(),
    state_size: 64,
    state_align: 8,
    scratch_hint: 0,
    scratch_max: 8388608,
    init: Some(xf_deframe_u32le_init_v1),
    step: Some(xf_deframe_u32le_step_v1),
    flush: Some(xf_deframe_u32le_flush_v1),
    drop: None,
};

#[no_mangle]
pub static x07_xf_json_canon_stream_v1: x07_stream_xf_plugin_v1 = x07_stream_xf_plugin_v1 {
    abi_tag: X07_XF_ABI_TAG_X7XF,
    abi_version: X07_XF_ABI_VERSION,
    plugin_id: c"xf.json_canon_stream_v1".as_ptr(),
    flags: X07_XF_FLAG_DETERMINISTIC_ONLY,
    in_item_brand: c"any".as_ptr(),
    out_item_brand: c"none".as_ptr(),
    state_size: 64,
    state_align: 8,
    scratch_hint: 0,
    scratch_max: 8388608,
    init: Some(xf_json_canon_stream_init_v1),
    step: Some(xf_json_canon_stream_step_v1),
    flush: Some(xf_json_canon_stream_flush_v1),
    drop: None,
};
