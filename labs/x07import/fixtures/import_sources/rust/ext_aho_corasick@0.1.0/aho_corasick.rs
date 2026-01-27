fn _push_u32_le(out: VecU8, x: i32) -> VecU8 {
    vec_u8_extend_bytes(out, codec_write_u32_le(x))
}

fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

fn _write_u32_le_at_bytes(mut b: Bytes, off: i32, x: i32) -> Bytes {
    b = bytes_set_u8(b, off, x & 255);
    b = bytes_set_u8(b, off + 1, (x >> 8) & 255);
    b = bytes_set_u8(b, off + 2, (x >> 16) & 255);
    b = bytes_set_u8(b, off + 3, (x >> 24) & 255);
    b
}

fn _goto_view(
    state_head: BytesView,
    trans_byte: BytesView,
    trans_next: BytesView,
    trans_link: BytesView,
    trans_count: i32,
    state: i32,
    c: i32,
) -> i32 {
    let mut t = codec_read_u32_le(state_head, state * 4);
    let mut done = false;
    for _ in 0..(trans_count + 1) {
        if done {
        } else if t < 0 {
            done = true;
        } else {
            if view_get_u8(trans_byte, t) == c {
                return codec_read_u32_le(trans_next, t * 4);
            }
            t = codec_read_u32_le(trans_link, t * 4);
        }
    }
    -1
}

fn _ok_match(is_match: bool, start: i32, end: i32, pat_id: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(14);
    out = vec_u8_push(out, 1);
    out = vec_u8_push(out, if is_match { 1 } else { 0 });
    out = vec_u8_extend_bytes(out, codec_write_u32_le(start));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(end));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(pat_id));
    vec_u8_into_bytes(out)
}

pub fn code_compile_invalid_needles() -> i32 {
    1
}

pub fn code_compile_empty_needle() -> i32 {
    2
}

pub fn code_exec_invalid_compiled() -> i32 {
    3
}

pub fn is_err(doc: BytesView) -> bool {
    if view_len(doc) < 1 {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn err_code(doc: BytesView) -> i32 {
    if !is_err(doc) {
        return 0;
    }
    if view_len(doc) < 5 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn is_match(doc: BytesView) -> bool {
    if view_len(doc) < 2 {
        return false;
    }
    if view_get_u8(doc, 0) != 1 {
        return false;
    }
    view_get_u8(doc, 1) == 1
}

pub fn match_start(doc: BytesView) -> i32 {
    if !is_match(doc) {
        return 0;
    }
    if view_len(doc) < 6 {
        return 0;
    }
    codec_read_u32_le(doc, 2)
}

pub fn match_end(doc: BytesView) -> i32 {
    if !is_match(doc) {
        return 0;
    }
    if view_len(doc) < 10 {
        return 0;
    }
    codec_read_u32_le(doc, 6)
}

pub fn match_len(doc: BytesView) -> i32 {
    if !is_match(doc) {
        return 0;
    }
    let a = match_start(doc);
    let b = match_end(doc);
    if lt_u(b, a) {
        0
    } else {
        b - a
    }
}

pub fn match_pat_id(doc: BytesView) -> i32 {
    if !is_match(doc) {
        return 0;
    }
    if view_len(doc) < 14 {
        return 0;
    }
    codec_read_u32_le(doc, 10)
}

pub fn compile(needles: BytesView) -> Bytes {
    let n = view_len(needles);
    if lt_u(n, 4) {
        return _make_err(code_compile_invalid_needles());
    }

    let pat_count = codec_read_u32_le(needles, 0);
    if pat_count < 0 {
        return _make_err(code_compile_invalid_needles());
    }

    let mut max_pat_len = 0;
    let mut total_pat_bytes = 0;

    let mut off = 4;
    for _ in 0..pat_count {
        if ge_u(off + 3, n) {
            return _make_err(code_compile_invalid_needles());
        }
        let pat_len = codec_read_u32_le(needles, off);
        off = off + 4;

        if pat_len == 0 {
            return _make_err(code_compile_empty_needle());
        }
        if pat_len < 0 {
            return _make_err(code_compile_invalid_needles());
        }
        if lt_u(max_pat_len, pat_len) {
            max_pat_len = pat_len;
        }
        total_pat_bytes = total_pat_bytes + pat_len;

        if ge_u(off + pat_len, n + 1) {
            return _make_err(code_compile_invalid_needles());
        }
        off = off + pat_len;
    }

    if off != n {
        return _make_err(code_compile_invalid_needles());
    }

    let max_states = total_pat_bytes + 1;
    let max_trans = total_pat_bytes;

    let mut pat_lens = bytes_alloc(pat_count * 4);
    let mut state_head = bytes_alloc(max_states * 4);
    let mut fail = bytes_alloc(max_states * 4);
    let mut out = bytes_alloc(max_states * 4);
    let mut trans_byte = bytes_alloc(max_trans);
    let mut trans_next = bytes_alloc(max_trans * 4);
    let mut trans_link = bytes_alloc(max_trans * 4);

    state_head = _write_u32_le_at_bytes(state_head, 0, -1);
    let mut state_count = 1;
    let mut trans_count = 0;

    off = 4;
    for pat_id in 0..pat_count {
        let pat_len = codec_read_u32_le(needles, off);
        off = off + 4;
        pat_lens = _write_u32_le_at_bytes(pat_lens, pat_id * 4, pat_len);

        let pat_start = off;
        let mut state = 0;
        for j in 0..pat_len {
            let c = view_get_u8(needles, pat_start + j);
            let next = _goto_view(
                bytes_view(state_head),
                bytes_view(trans_byte),
                bytes_view(trans_next),
                bytes_view(trans_link),
                trans_count,
                state,
                c,
            );
            if next < 0 {
                let new_state = state_count;
                state_count = state_count + 1;
                state_head = _write_u32_le_at_bytes(state_head, new_state * 4, -1);

                let head = codec_read_u32_le(bytes_view(state_head), state * 4);
                trans_byte = bytes_set_u8(trans_byte, trans_count, c);
                trans_next = _write_u32_le_at_bytes(trans_next, trans_count * 4, new_state);
                trans_link = _write_u32_le_at_bytes(trans_link, trans_count * 4, head);
                state_head = _write_u32_le_at_bytes(state_head, state * 4, trans_count);

                trans_count = trans_count + 1;
                state = new_state;
            } else {
                state = next;
            }
        }

        let cur_out = codec_read_u32_le(bytes_view(out), state * 4);
        let new_out = pat_id + 1;
        if cur_out == 0 || lt_u(new_out, cur_out) {
            out = _write_u32_le_at_bytes(out, state * 4, new_out);
            let _unit = 0;
        }

        off = off + pat_len;
    }

    let mut q = bytes_alloc(max_states * 4);
    let mut q_head = 0;
    let mut q_tail = 0;

    let mut t = codec_read_u32_le(bytes_view(state_head), 0);
    let mut done_init = false;
    for _ in 0..(trans_count + 1) {
        if done_init {
        } else if t < 0 {
            done_init = true;
        } else {
            let s = codec_read_u32_le(bytes_view(trans_next), t * 4);
            fail = _write_u32_le_at_bytes(fail, s * 4, 0);
            q = _write_u32_le_at_bytes(q, q_tail, s);
            q_tail = q_tail + 4;
            t = codec_read_u32_le(bytes_view(trans_link), t * 4);
        }
    }

    let mut done_bfs = false;
    for _ in 0..state_count {
        if done_bfs {
        } else if ge_u(q_head, q_tail) {
            done_bfs = true;
        } else {
            let r = codec_read_u32_le(bytes_view(q), q_head);
            q_head = q_head + 4;

            let mut tt = codec_read_u32_le(bytes_view(state_head), r * 4);
            let mut done_tt = false;
            for _ in 0..(trans_count + 1) {
                if done_tt {
                } else if tt < 0 {
                    done_tt = true;
                } else {
                    let a = view_get_u8(bytes_view(trans_byte), tt);
                    let s = codec_read_u32_le(bytes_view(trans_next), tt * 4);
                    q = _write_u32_le_at_bytes(q, q_tail, s);
                    q_tail = q_tail + 4;

                    let mut f = codec_read_u32_le(bytes_view(fail), r * 4);
                    let mut g = -1;
                    let mut done_fail = false;
                    for _ in 0..state_count {
                        if done_fail {
                        } else if f == 0 {
                            g = _goto_view(
                                bytes_view(state_head),
                                bytes_view(trans_byte),
                                bytes_view(trans_next),
                                bytes_view(trans_link),
                                trans_count,
                                0,
                                a,
                            );
                            done_fail = true;
                        } else {
                            g = _goto_view(
                                bytes_view(state_head),
                                bytes_view(trans_byte),
                                bytes_view(trans_next),
                                bytes_view(trans_link),
                                trans_count,
                                f,
                                a,
                            );
                            if g < 0 {
                                f = codec_read_u32_le(bytes_view(fail), f * 4);
                            } else {
                                done_fail = true;
                            }
                        }
                    }
                    if g < 0 {
                        g = 0;
                    }

                    fail = _write_u32_le_at_bytes(fail, s * 4, g);

                    let out_g = codec_read_u32_le(bytes_view(out), g * 4);
                    if out_g != 0 {
                        let out_s = codec_read_u32_le(bytes_view(out), s * 4);
                        if out_s == 0 || lt_u(out_g, out_s) {
                            out = _write_u32_le_at_bytes(out, s * 4, out_g);
                            let _unit = 0;
                        }
                    }

                    tt = codec_read_u32_le(bytes_view(trans_link), tt * 4);
                }
            }
        }
    }

    let state_head_used = bytes_slice(bytes_view(state_head), 0, state_count * 4);
    let fail_used = bytes_slice(bytes_view(fail), 0, state_count * 4);
    let out_used = bytes_slice(bytes_view(out), 0, state_count * 4);
    let trans_byte_used = bytes_slice(bytes_view(trans_byte), 0, trans_count);
    let trans_next_used = bytes_slice(bytes_view(trans_next), 0, trans_count * 4);
    let trans_link_used = bytes_slice(bytes_view(trans_link), 0, trans_count * 4);

    let total = 17
        + bytes_len(pat_lens)
        + bytes_len(state_head_used)
        + bytes_len(fail_used)
        + bytes_len(out_used)
        + bytes_len(trans_byte_used)
        + bytes_len(trans_next_used)
        + bytes_len(trans_link_used);
    let mut compiled = vec_u8_with_capacity(total);
    compiled = vec_u8_push(compiled, 1);
    compiled = _push_u32_le(compiled, pat_count);
    compiled = _push_u32_le(compiled, state_count);
    compiled = _push_u32_le(compiled, trans_count);
    compiled = _push_u32_le(compiled, max_pat_len);
    compiled = vec_u8_extend_bytes(compiled, bytes_view(pat_lens));
    compiled = vec_u8_extend_bytes(compiled, bytes_view(state_head_used));
    compiled = vec_u8_extend_bytes(compiled, bytes_view(fail_used));
    compiled = vec_u8_extend_bytes(compiled, bytes_view(out_used));
    compiled = vec_u8_extend_bytes(compiled, bytes_view(trans_byte_used));
    compiled = vec_u8_extend_bytes(compiled, bytes_view(trans_next_used));
    compiled = vec_u8_extend_bytes(compiled, bytes_view(trans_link_used));
    vec_u8_into_bytes(compiled)
}

pub fn find(compiled: BytesView, hay: BytesView) -> Bytes {
    if is_err(compiled) {
        return view_to_bytes(compiled);
    }
    let n = view_len(compiled);
    if lt_u(n, 17) {
        return _make_err(code_exec_invalid_compiled());
    }
    if view_get_u8(compiled, 0) != 1 {
        return _make_err(code_exec_invalid_compiled());
    }

    let pat_count = codec_read_u32_le(compiled, 1);
    let state_count = codec_read_u32_le(compiled, 5);
    let trans_count = codec_read_u32_le(compiled, 9);
    let _max_pat_len = codec_read_u32_le(compiled, 13);
    if pat_count < 0 || state_count < 1 || trans_count < 0 {
        return _make_err(code_exec_invalid_compiled());
    }

    let off_pat_lens = 17;
    let off_state_head = off_pat_lens + (pat_count * 4);
    let off_fail = off_state_head + (state_count * 4);
    let off_out = off_fail + (state_count * 4);
    let off_trans_byte = off_out + (state_count * 4);
    let off_trans_next = off_trans_byte + trans_count;
    let off_trans_link = off_trans_next + (trans_count * 4);
    let need = off_trans_link + (trans_count * 4);
    if ge_u(need, n + 1) {
        return _make_err(code_exec_invalid_compiled());
    }

    let pat_lens_v = view_slice(compiled, off_pat_lens, pat_count * 4);
    let state_head_v = view_slice(compiled, off_state_head, state_count * 4);
    let fail_v = view_slice(compiled, off_fail, state_count * 4);
    let out_v = view_slice(compiled, off_out, state_count * 4);
    let trans_byte_v = view_slice(compiled, off_trans_byte, trans_count);
    let trans_next_v = view_slice(compiled, off_trans_next, trans_count * 4);
    let trans_link_v = view_slice(compiled, off_trans_link, trans_count * 4);

    let hay_len = view_len(hay);
    let mut state = 0;
    for i in 0..hay_len {
        let c = view_get_u8(hay, i);

        let mut next = _goto_view(
            state_head_v,
            trans_byte_v,
            trans_next_v,
            trans_link_v,
            trans_count,
            state,
            c,
        );

        let mut done = false;
        for _ in 0..state_count {
            if done {
            } else if next >= 0 {
                done = true;
            } else if state == 0 {
                done = true;
            } else {
                state = codec_read_u32_le(fail_v, state * 4);
                next = _goto_view(
                    state_head_v,
                    trans_byte_v,
                    trans_next_v,
                    trans_link_v,
                    trans_count,
                    state,
                    c,
                );
            }
        }

        if next >= 0 {
            state = next;
        } else {
            state = 0;
        }

        let out_id = codec_read_u32_le(out_v, state * 4);
        if out_id != 0 {
            let pat_id = out_id - 1;
            if pat_id < 0 || ge_u(pat_id, pat_count) {
                return _make_err(code_exec_invalid_compiled());
            }
            let pat_len = codec_read_u32_le(pat_lens_v, pat_id * 4);
            if pat_len <= 0 {
                return _make_err(code_exec_invalid_compiled());
            }
            let end = i + 1;
            let start = end - pat_len;
            if start < 0 {
                return _make_err(code_exec_invalid_compiled());
            }
            return _ok_match(true, start, end, pat_id);
        }
    }

    _ok_match(false, 0, 0, 0)
}
