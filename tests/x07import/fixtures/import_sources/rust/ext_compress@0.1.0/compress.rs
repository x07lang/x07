fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

fn _write_u32_le(mut b: Bytes, off: i32, v: i32) -> Bytes {
    b = bytes_set_u8(b, off + 0, v & 255);
    b = bytes_set_u8(b, off + 1, (v >> 8) & 255);
    b = bytes_set_u8(b, off + 2, (v >> 16) & 255);
    bytes_set_u8(b, off + 3, (v >> 24) & 255)
}

fn _read_u16_le(b: BytesView, off: i32) -> i32 {
    let b0 = view_get_u8(b, off);
    let b1 = view_get_u8(b, off + 1);
    b0 | (b1 << 8)
}

fn _read_u32_le(b: BytesView, off: i32) -> i32 {
    let b0 = view_get_u8(b, off);
    let b1 = view_get_u8(b, off + 1);
    let b2 = view_get_u8(b, off + 2);
    let b3 = view_get_u8(b, off + 3);
    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}

fn _read_u32_be(b: BytesView, off: i32) -> i32 {
    let b0 = view_get_u8(b, off);
    let b1 = view_get_u8(b, off + 1);
    let b2 = view_get_u8(b, off + 2);
    let b3 = view_get_u8(b, off + 3);
    (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
}

fn _bit_reverse(mut code: i32, bits: i32) -> i32 {
    let mut out = 0;
    for _ in 0..bits {
        out = (out << 1) | (code & 1);
        code = code >> 1;
    }
    out
}

fn _huff_build_table(lens: BytesView, sym_count: i32) -> Bytes {
    // Returns: table bytes with layout:
    //  - u32 max_bits
    //  - table entries u32[len<<16 | sym] of length (1<<max_bits)
    //
    // Returns empty bytes on invalid lens set.
    let mut max_bits = 0;
    for i in 0..sym_count {
        let l = view_get_u8(lens, i);
        if l > 15 {
            return bytes_alloc(0);
        }
        if l > max_bits {
            max_bits = l;
        }
    }
    if max_bits == 0 {
        return bytes_alloc(0);
    }

    let mut counts = bytes_alloc((max_bits + 1) * 4);
    for i in 0..sym_count {
        let l = view_get_u8(lens, i);
        if l != 0 {
            let off = l * 4;
            let cur = codec_read_u32_le(bytes_view(counts), off);
            counts = _write_u32_le(counts, off, cur + 1);
        }
    }

    let mut next = bytes_alloc((max_bits + 1) * 4);
    let mut code = 0;
    for bits in 1..(max_bits + 1) {
        let prev = codec_read_u32_le(bytes_view(counts), (bits - 1) * 4);
        code = (code + prev) << 1;
        let cur = codec_read_u32_le(bytes_view(counts), bits * 4);
        if (code + cur) > (1 << bits) {
            return bytes_alloc(0);
        }
        next = _write_u32_le(next, bits * 4, code);
    }

    let table_len = 1 << max_bits;
    let mut table = bytes_alloc(4 + table_len * 4);
    table = _write_u32_le(table, 0, max_bits);

    for sym in 0..sym_count {
        let l = view_get_u8(lens, sym);
        if l != 0 {
            let off = l * 4;
            let c = codec_read_u32_le(bytes_view(next), off);
            next = _write_u32_le(next, off, c + 1);

            let r = _bit_reverse(c, l);
            let entry = (l << 16) | sym;
            let step = 1 << l;
            let mut j = r;
            for _ in 0..(table_len + 1) {
                if lt_u(j, table_len) {
                    let pos = 4 + (j * 4);
                    if codec_read_u32_le(bytes_view(table), pos) != 0 {
                        return bytes_alloc(0);
                    }
                    table = _write_u32_le(table, pos, entry);
                    j = j + step;
                }
            }
        }
    }

    table
}

fn _adler32(b: BytesView) -> i32 {
    let modp = 65521;
    let mut s1 = 1;
    let mut s2 = 0;
    let n = view_len(b);
    let mut off = 0;
    for _ in 0..(n + 1) {
        if ge_u(off, n) {
            return (s2 << 16) | s1;
        }
        let remain = n - off;
        let block = if lt_u(remain, 5552) { remain } else { 5552 };
        for i in 0..block {
            let x = view_get_u8(b, off + i);
            s1 = s1 + x;
            s2 = s2 + s1;
        }
        s1 = s1 % modp;
        s2 = s2 % modp;
        off = off + block;
    }
    (s2 << 16) | s1
}

fn _crc32_table() -> Bytes {
    let mut out = vec_u8_with_capacity(256 * 4);
    for i in 0..256 {
        let mut c = i;
        for _ in 0..8 {
            if (c & 1) != 0 {
                c = (c >> 1) ^ (-306674912); // 0xEDB88320
            } else {
                c = c >> 1;
            }
        }
        out = vec_u8_extend_bytes(out, codec_write_u32_le(c));
    }
    vec_u8_into_bytes(out)
}

fn _crc32(b: BytesView) -> i32 {
    let table_b = _crc32_table();
    let table = bytes_view(table_b);
    let mut crc = 0 - 1;
    let n = view_len(b);
    for i in 0..n {
        let x = view_get_u8(b, i);
        let idx = (crc ^ x) & 255;
        let t = codec_read_u32_le(table, idx * 4);
        crc = (crc >> 8) ^ t;
    }
    crc ^ (0 - 1)
}

fn _len_extra(sym: i32) -> i32 {
    if sym < 265 {
        0
    } else if sym < 269 {
        1
    } else if sym < 273 {
        2
    } else if sym < 277 {
        3
    } else if sym < 281 {
        4
    } else if sym < 285 {
        5
    } else if sym == 285 {
        0
    } else {
        -1
    }
}

fn _len_base(sym: i32) -> i32 {
    if sym < 257 || sym > 285 {
        return -1;
    }
    if sym < 265 {
        3 + (sym - 257)
    } else if sym < 269 {
        11 + ((sym - 265) * 2)
    } else if sym < 273 {
        19 + ((sym - 269) * 4)
    } else if sym < 277 {
        35 + ((sym - 273) * 8)
    } else if sym < 281 {
        67 + ((sym - 277) * 16)
    } else if sym < 285 {
        131 + ((sym - 281) * 32)
    } else {
        258
    }
}

fn _dist_extra(sym: i32) -> i32 {
    if sym < 0 || sym > 29 {
        return -1;
    }
    if sym < 4 {
        0
    } else {
        (sym / 2) - 1
    }
}

fn _dist_base(sym: i32) -> i32 {
    if sym < 0 || sym > 29 {
        return -1;
    }
    if sym < 4 {
        1 + sym
    } else {
        let eb = (sym / 2) - 1;
        let group = sym - (4 + eb * 2);
        (1 << (eb + 2)) + (group * (1 << eb))
    }
}

fn _inflate_block_huffman_decode(
    data: BytesView,
    n: i32,
    max_out_bytes: i32,
    mut st: Bytes,
    bfinal: i32,
    lit_table: BytesView,
    dist_table: BytesView,
) -> Bytes {
    let mut out_len = codec_read_u32_le(bytes_view(st), 4);
    let mut off = codec_read_u32_le(bytes_view(st), 8);
    let mut bit_buf = codec_read_u32_le(bytes_view(st), 12);
    let mut bit_count = codec_read_u32_le(bytes_view(st), 16);

    let lit_max = codec_read_u32_le(lit_table, 0);
    let lit_mask = (1 << lit_max) - 1;
    let dist_max = codec_read_u32_le(dist_table, 0);
    let dist_mask = (1 << dist_max) - 1;

    let mut block_done = 0;
    for _ in 0..(n * 8 + 1) {
        if block_done == 0 {
            for _ in 0..3 {
                if bit_count < lit_max {
                    if ge_u(off, n) {
                        st = _write_u32_le(st, 0, code_truncated());
                        return st;
                    }
                    bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
                    bit_count = bit_count + 8;
                    off = off + 1;
                }
            }
            if bit_count < lit_max {
                st = _write_u32_le(st, 0, code_truncated());
                return st;
            }

            let entry = codec_read_u32_le(lit_table, 4 + (bit_buf & lit_mask) * 4);
            let elen = entry >> 16;
            if elen == 0 {
                st = _write_u32_le(st, 0, code_invalid_stream());
                return st;
            }
            let sym = entry & 65535;
            bit_buf = bit_buf >> elen;
            bit_count = bit_count - elen;

            if sym < 256 {
                if lt_u(max_out_bytes, out_len + 1) {
                    st = _write_u32_le(st, 0, code_output_limit());
                    return st;
                }
                st = bytes_set_u8(st, 24 + out_len, sym);
                out_len = out_len + 1;
            } else if sym == 256 {
                block_done = 1;
            } else {
                let base = _len_base(sym);
                let eb = _len_extra(sym);
                if base < 0 || eb < 0 {
                    st = _write_u32_le(st, 0, code_invalid_stream());
                    return st;
                }
                let mut extra = 0;
                if eb > 0 {
                    for _ in 0..3 {
                        if bit_count < eb {
                            if ge_u(off, n) {
                                st = _write_u32_le(st, 0, code_truncated());
                                return st;
                            }
                            bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
                            bit_count = bit_count + 8;
                            off = off + 1;
                        }
                    }
                    if bit_count < eb {
                        st = _write_u32_le(st, 0, code_truncated());
                        return st;
                    }
                    extra = bit_buf & ((1 << eb) - 1);
                    bit_buf = bit_buf >> eb;
                    bit_count = bit_count - eb;
                }
                let len = base + extra;

                for _ in 0..3 {
                    if bit_count < dist_max {
                        if ge_u(off, n) {
                            st = _write_u32_le(st, 0, code_truncated());
                            return st;
                        }
                        bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
                        bit_count = bit_count + 8;
                        off = off + 1;
                    }
                }
                if bit_count < dist_max {
                    st = _write_u32_le(st, 0, code_truncated());
                    return st;
                }
                let dentry = codec_read_u32_le(dist_table, 4 + (bit_buf & dist_mask) * 4);
                let dlen = dentry >> 16;
                if dlen == 0 {
                    st = _write_u32_le(st, 0, code_invalid_stream());
                    return st;
                }
                let dsym = dentry & 65535;
                bit_buf = bit_buf >> dlen;
                bit_count = bit_count - dlen;

                let dbase = _dist_base(dsym);
                let deb = _dist_extra(dsym);
                if dbase < 0 || deb < 0 {
                    st = _write_u32_le(st, 0, code_invalid_stream());
                    return st;
                }
                let mut dext = 0;
                if deb > 0 {
                    for _ in 0..3 {
                        if bit_count < deb {
                            if ge_u(off, n) {
                                st = _write_u32_le(st, 0, code_truncated());
                                return st;
                            }
                            bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
                            bit_count = bit_count + 8;
                            off = off + 1;
                        }
                    }
                    if bit_count < deb {
                        st = _write_u32_le(st, 0, code_truncated());
                        return st;
                    }
                    dext = bit_buf & ((1 << deb) - 1);
                    bit_buf = bit_buf >> deb;
                    bit_count = bit_count - deb;
                }
                let dist = dbase + dext;
                if dist <= 0 || dist > out_len {
                    st = _write_u32_le(st, 0, code_invalid_stream());
                    return st;
                }
                if lt_u(max_out_bytes, out_len) || lt_u(max_out_bytes - out_len, len) {
                    st = _write_u32_le(st, 0, code_output_limit());
                    return st;
                }
                for _ in 0..len {
                    let b = bytes_get_u8(st, 24 + (out_len - dist));
                    st = bytes_set_u8(st, 24 + out_len, b);
                    out_len = out_len + 1;
                }
            }
        }
    }

    if block_done == 0 {
        st = _write_u32_le(st, 0, code_truncated());
        return st;
    }

    st = _write_u32_le(st, 4, out_len);
    st = _write_u32_le(st, 8, off);
    st = _write_u32_le(st, 12, bit_buf);
    st = _write_u32_le(st, 16, bit_count);
    if bfinal != 0 {
        st = _write_u32_le(st, 20, 1);
    }
    st
}

fn _inflate_block_fixed(
    data: BytesView,
    n: i32,
    max_out_bytes: i32,
    st: Bytes,
    bfinal: i32,
) -> Bytes {
    let mut lit_lens = bytes_alloc(288);
    for i in 0..144 {
        lit_lens = bytes_set_u8(lit_lens, i, 8);
    }
    for i in 144..256 {
        lit_lens = bytes_set_u8(lit_lens, i, 9);
    }
    for i in 256..280 {
        lit_lens = bytes_set_u8(lit_lens, i, 7);
    }
    for i in 280..288 {
        lit_lens = bytes_set_u8(lit_lens, i, 8);
    }

    let mut dist_lens = bytes_alloc(32);
    for i in 0..32 {
        dist_lens = bytes_set_u8(dist_lens, i, 5);
    }

    let lit_table_b = _huff_build_table(bytes_view(lit_lens), 288);
    let dist_table_b = _huff_build_table(bytes_view(dist_lens), 32);
    if bytes_len(lit_table_b) == 0 || bytes_len(dist_table_b) == 0 {
        return _write_u32_le(st, 0, code_invalid_stream());
    }

    _inflate_block_huffman_decode(
        data,
        n,
        max_out_bytes,
        st,
        bfinal,
        bytes_view(lit_table_b),
        bytes_view(dist_table_b),
    )
}

fn _cl_order(i: i32) -> i32 {
    if i == 0 {
        16
    } else if i == 1 {
        17
    } else if i == 2 {
        18
    } else if i == 3 {
        0
    } else if i == 4 {
        8
    } else if i == 5 {
        7
    } else if i == 6 {
        9
    } else if i == 7 {
        6
    } else if i == 8 {
        10
    } else if i == 9 {
        5
    } else if i == 10 {
        11
    } else if i == 11 {
        4
    } else if i == 12 {
        12
    } else if i == 13 {
        3
    } else if i == 14 {
        13
    } else if i == 15 {
        2
    } else if i == 16 {
        14
    } else if i == 17 {
        1
    } else {
        15
    }
}

fn _dyn_lens_err(code: i32) -> Bytes {
    let mut out = bytes_alloc(24);
    out = _write_u32_le(out, 0, code);
    out
}

fn _inflate_dynamic_read_lens(
    data: BytesView,
    n: i32,
    mut off: i32,
    mut bit_buf: i32,
    mut bit_count: i32,
) -> Bytes {
    for _ in 0..3 {
        if bit_count < (5 + 5 + 4) {
            if ge_u(off, n) {
                return _dyn_lens_err(code_truncated());
            }
            bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
            bit_count = bit_count + 8;
            off = off + 1;
        }
    }
    if bit_count < (5 + 5 + 4) {
        return _dyn_lens_err(code_truncated());
    }

    let hlit = (bit_buf & 31) + 257;
    bit_buf = bit_buf >> 5;
    bit_count = bit_count - 5;
    let hdist = (bit_buf & 31) + 1;
    bit_buf = bit_buf >> 5;
    bit_count = bit_count - 5;
    let hclen = (bit_buf & 15) + 4;
    bit_buf = bit_buf >> 4;
    bit_count = bit_count - 4;

    let mut cl_lens = bytes_alloc(19);
    for i in 0..hclen {
        for _ in 0..3 {
            if bit_count < 3 {
                if ge_u(off, n) {
                    return _dyn_lens_err(code_truncated());
                }
                bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
                bit_count = bit_count + 8;
                off = off + 1;
            }
        }
        if bit_count < 3 {
            return _dyn_lens_err(code_truncated());
        }
        let v = bit_buf & 7;
        bit_buf = bit_buf >> 3;
        bit_count = bit_count - 3;

        let sym = _cl_order(i);
        cl_lens = bytes_set_u8(cl_lens, sym, v);
    }

    let cl_table_b = _huff_build_table(bytes_view(cl_lens), 19);
    if bytes_len(cl_table_b) == 0 {
        return _dyn_lens_err(code_invalid_stream());
    }
    let cl_table = bytes_view(cl_table_b);
    let cl_max = codec_read_u32_le(cl_table, 0);
    let cl_mask = (1 << cl_max) - 1;

    let total = hlit + hdist;
    let mut out = bytes_alloc(24 + total);
    let mut idx = 0;
    let mut prev = 0;
    for _ in 0..(total + 1) {
        if lt_u(idx, total) {
            for _ in 0..3 {
                if bit_count < cl_max {
                    if ge_u(off, n) {
                        return _dyn_lens_err(code_truncated());
                    }
                    bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
                    bit_count = bit_count + 8;
                    off = off + 1;
                }
            }
            if bit_count < cl_max {
                return _dyn_lens_err(code_truncated());
            }

            let entry = codec_read_u32_le(cl_table, 4 + (bit_buf & cl_mask) * 4);
            let elen = entry >> 16;
            if elen == 0 {
                return _dyn_lens_err(code_invalid_stream());
            }
            let sym = entry & 65535;
            bit_buf = bit_buf >> elen;
            bit_count = bit_count - elen;

            if sym <= 15 {
                out = bytes_set_u8(out, 24 + idx, sym);
                prev = sym;
                idx = idx + 1;
            } else if sym == 16 {
                if idx == 0 {
                    return _dyn_lens_err(code_invalid_stream());
                }
                for _ in 0..3 {
                    if bit_count < 2 {
                        if ge_u(off, n) {
                            return _dyn_lens_err(code_truncated());
                        }
                        bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
                        bit_count = bit_count + 8;
                        off = off + 1;
                    }
                }
                if bit_count < 2 {
                    return _dyn_lens_err(code_truncated());
                }
                let extra = bit_buf & 3;
                bit_buf = bit_buf >> 2;
                bit_count = bit_count - 2;
                let rep = 3 + extra;
                for _ in 0..rep {
                    if ge_u(idx, total) {
                        return _dyn_lens_err(code_invalid_stream());
                    }
                    out = bytes_set_u8(out, 24 + idx, prev);
                    idx = idx + 1;
                }
            } else if sym == 17 {
                for _ in 0..3 {
                    if bit_count < 3 {
                        if ge_u(off, n) {
                            return _dyn_lens_err(code_truncated());
                        }
                        bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
                        bit_count = bit_count + 8;
                        off = off + 1;
                    }
                }
                if bit_count < 3 {
                    return _dyn_lens_err(code_truncated());
                }
                let extra = bit_buf & 7;
                bit_buf = bit_buf >> 3;
                bit_count = bit_count - 3;
                let rep = 3 + extra;
                prev = 0;
                for _ in 0..rep {
                    if ge_u(idx, total) {
                        return _dyn_lens_err(code_invalid_stream());
                    }
                    out = bytes_set_u8(out, 24 + idx, 0);
                    idx = idx + 1;
                }
            } else if sym == 18 {
                for _ in 0..3 {
                    if bit_count < 7 {
                        if ge_u(off, n) {
                            return _dyn_lens_err(code_truncated());
                        }
                        bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
                        bit_count = bit_count + 8;
                        off = off + 1;
                    }
                }
                if bit_count < 7 {
                    return _dyn_lens_err(code_truncated());
                }
                let extra = bit_buf & 127;
                bit_buf = bit_buf >> 7;
                bit_count = bit_count - 7;
                let rep = 11 + extra;
                prev = 0;
                for _ in 0..rep {
                    if ge_u(idx, total) {
                        return _dyn_lens_err(code_invalid_stream());
                    }
                    out = bytes_set_u8(out, 24 + idx, 0);
                    idx = idx + 1;
                }
            } else {
                return _dyn_lens_err(code_invalid_stream());
            }
        }
    }

    if idx != total {
        return _dyn_lens_err(code_invalid_stream());
    }

    out = _write_u32_le(out, 0, 0);
    out = _write_u32_le(out, 4, off);
    out = _write_u32_le(out, 8, bit_buf);
    out = _write_u32_le(out, 12, bit_count);
    out = _write_u32_le(out, 16, hlit);
    out = _write_u32_le(out, 20, hdist);
    out
}

fn _inflate_block_dynamic(
    data: BytesView,
    n: i32,
    max_out_bytes: i32,
    mut st: Bytes,
    bfinal: i32,
) -> Bytes {
    let off0 = codec_read_u32_le(bytes_view(st), 8);
    let bit_buf0 = codec_read_u32_le(bytes_view(st), 12);
    let bit_count0 = codec_read_u32_le(bytes_view(st), 16);

    let lens_b = _inflate_dynamic_read_lens(data, n, off0, bit_buf0, bit_count0);
    let lens = bytes_view(lens_b);
    let status = codec_read_u32_le(lens, 0);
    if status != 0 {
        return _write_u32_le(st, 0, status);
    }

    let off1 = codec_read_u32_le(lens, 4);
    let bit_buf1 = codec_read_u32_le(lens, 8);
    let bit_count1 = codec_read_u32_le(lens, 12);
    let hlit = codec_read_u32_le(lens, 16);
    let hdist = codec_read_u32_le(lens, 20);

    let total = hlit + hdist;
    let all_lens = view_slice(lens, 24, total);
    let lit_lens = view_slice(all_lens, 0, hlit);
    let dist_lens = view_slice(all_lens, hlit, hdist);

    let lit_table_b = _huff_build_table(lit_lens, hlit);
    let dist_table_b = _huff_build_table(dist_lens, hdist);
    if bytes_len(lit_table_b) == 0 || bytes_len(dist_table_b) == 0 {
        return _write_u32_le(st, 0, code_invalid_stream());
    }

    st = _write_u32_le(st, 8, off1);
    st = _write_u32_le(st, 12, bit_buf1);
    st = _write_u32_le(st, 16, bit_count1);

    _inflate_block_huffman_decode(
        data,
        n,
        max_out_bytes,
        st,
        bfinal,
        bytes_view(lit_table_b),
        bytes_view(dist_table_b),
    )
}

fn _inflate_one_block(data: BytesView, n: i32, max_out_bytes: i32, mut st: Bytes) -> Bytes {
    let status = codec_read_u32_le(bytes_view(st), 0);
    let done = codec_read_u32_le(bytes_view(st), 20);
    if status != 0 || done != 0 {
        return st;
    }

    let mut out_len = codec_read_u32_le(bytes_view(st), 4);
    let mut off = codec_read_u32_le(bytes_view(st), 8);
    let mut bit_buf = codec_read_u32_le(bytes_view(st), 12);
    let mut bit_count = codec_read_u32_le(bytes_view(st), 16);

    for _ in 0..3 {
        if bit_count < 3 {
            if ge_u(off, n) {
                return _write_u32_le(st, 0, code_truncated());
            }
            bit_buf = bit_buf | (view_get_u8(data, off) << bit_count);
            bit_count = bit_count + 8;
            off = off + 1;
        }
    }
    if bit_count < 3 {
        return _write_u32_le(st, 0, code_truncated());
    }

    let bfinal = bit_buf & 1;
    let btype = (bit_buf >> 1) & 3;
    bit_buf = bit_buf >> 3;
    bit_count = bit_count - 3;

    st = _write_u32_le(st, 4, out_len);
    st = _write_u32_le(st, 8, off);
    st = _write_u32_le(st, 12, bit_buf);
    st = _write_u32_le(st, 16, bit_count);

    if btype == 0 {
        let drop = bit_count & 7;
        bit_buf = bit_buf >> drop;
        bit_count = bit_count - drop;
        let buf_bytes = bit_count >> 3;
        if lt_u(off, buf_bytes) {
            return _write_u32_le(st, 0, code_invalid_stream());
        }
        off = off - buf_bytes;
        bit_buf = 0;
        bit_count = 0;

        if lt_u(n, off + 4) {
            return _write_u32_le(st, 0, code_truncated());
        }
        let len = _read_u16_le(data, off);
        let nlen = _read_u16_le(data, off + 2);
        if ((len ^ nlen) & 65535) != 65535 {
            return _write_u32_le(st, 0, code_invalid_stream());
        }
        off = off + 4;
        if lt_u(n, off + len) {
            return _write_u32_le(st, 0, code_truncated());
        }
        if lt_u(max_out_bytes, out_len) || lt_u(max_out_bytes - out_len, len) {
            return _write_u32_le(st, 0, code_output_limit());
        }
        for i in 0..len {
            st = bytes_set_u8(st, 24 + out_len, view_get_u8(data, off + i));
            out_len = out_len + 1;
        }
        off = off + len;

        st = _write_u32_le(st, 4, out_len);
        st = _write_u32_le(st, 8, off);
        st = _write_u32_le(st, 12, bit_buf);
        st = _write_u32_le(st, 16, bit_count);
        if bfinal != 0 {
            st = _write_u32_le(st, 20, 1);
        }
        st
    } else if btype == 1 {
        _inflate_block_fixed(data, n, max_out_bytes, st, bfinal)
    } else if btype == 2 {
        _inflate_block_dynamic(data, n, max_out_bytes, st, bfinal)
    } else {
        _write_u32_le(st, 0, code_invalid_stream())
    }
}

fn _inflate_deflate(data: BytesView, max_out_bytes: i32) -> Bytes {
    if max_out_bytes < 0 {
        return _make_err(code_output_limit());
    }

    let n = view_len(data);
    let mut st = bytes_alloc(24 + max_out_bytes);

    for _ in 0..(n + 1) {
        let status = codec_read_u32_le(bytes_view(st), 0);
        let done = codec_read_u32_le(bytes_view(st), 20);
        if status == 0 && done == 0 {
            st = _inflate_one_block(data, n, max_out_bytes, st);
        }
    }

    let status = codec_read_u32_le(bytes_view(st), 0);
    if status != 0 {
        return _make_err(status);
    }
    let done = codec_read_u32_le(bytes_view(st), 20);
    if done == 0 {
        return _make_err(code_truncated());
    }

    let mut off = codec_read_u32_le(bytes_view(st), 8);
    let mut bit_buf = codec_read_u32_le(bytes_view(st), 12);
    let mut bit_count = codec_read_u32_le(bytes_view(st), 16);

    let drop = bit_count & 7;
    bit_buf = bit_buf >> drop;
    bit_count = bit_count - drop;
    let buf_bytes = bit_count >> 3;
    if lt_u(off, buf_bytes) {
        return _make_err(code_invalid_stream());
    }
    off = off - buf_bytes;
    if off != n {
        return _make_err(code_invalid_stream());
    }

    let out_len = codec_read_u32_le(bytes_view(st), 4);
    let mut out = vec_u8_with_capacity(1 + out_len);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes_range(out, bytes_view(st), 24, out_len);
    vec_u8_into_bytes(out)
}

pub fn is_err(doc: BytesView) -> bool {
    if view_len(doc) < 1 {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn err_code(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 0 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn code_truncated() -> i32 {
    1
}

pub fn code_invalid_header() -> i32 {
    2
}

pub fn code_invalid_stream() -> i32 {
    3
}

pub fn code_checksum_mismatch() -> i32 {
    4
}

pub fn code_output_limit() -> i32 {
    5
}

pub fn crc32(b: BytesView) -> i32 {
    _crc32(b)
}

pub fn out_len(doc: BytesView) -> i32 {
    let n = view_len(doc);
    if n < 1 {
        return 0;
    }
    if view_get_u8(doc, 0) != 1 {
        return 0;
    }
    n - 1
}

pub fn get_view(doc: BytesView) -> BytesView {
    let n = view_len(doc);
    if n < 1 {
        return view_slice(doc, 0, 0);
    }
    if view_get_u8(doc, 0) != 1 {
        return view_slice(doc, 0, 0);
    }
    view_slice(doc, 1, n - 1)
}

pub fn get_bytes(doc: BytesView) -> Bytes {
    view_to_bytes(get_view(doc))
}

pub fn inflate_raw(data: BytesView, max_out_bytes: i32) -> Bytes {
    _inflate_deflate(data, max_out_bytes)
}

pub fn zlib_decompress(data: BytesView, max_out_bytes: i32) -> Bytes {
    let n = view_len(data);
    if lt_u(n, 2) {
        return _make_err(code_truncated());
    }

    let cmf = view_get_u8(data, 0);
    let flg = view_get_u8(data, 1);
    let cm = cmf & 15;
    let cinfo = cmf >> 4;
    if cm != 8 || cinfo > 7 {
        return _make_err(code_invalid_header());
    }
    let check = (cmf << 8) | flg;
    if (check % 31) != 0 {
        return _make_err(code_invalid_header());
    }
    if (flg & 32) != 0 {
        return _make_err(code_invalid_header());
    }

    if lt_u(n, 2 + 4) {
        return _make_err(code_truncated());
    }
    let deflate_len = n - 2 - 4;
    let deflate = view_slice(data, 2, deflate_len);
    let doc = _inflate_deflate(deflate, max_out_bytes);
    if is_err(bytes_view(doc)) {
        return doc;
    }

    let expected = _read_u32_be(data, n - 4);
    let actual = _adler32(get_view(bytes_view(doc)));
    if expected != actual {
        return _make_err(code_checksum_mismatch());
    }
    doc
}

pub fn gzip_decompress(data: BytesView, max_out_bytes: i32) -> Bytes {
    let n = view_len(data);
    if lt_u(n, 10) {
        return _make_err(code_truncated());
    }
    if view_get_u8(data, 0) != 31 || view_get_u8(data, 1) != 139 {
        return _make_err(code_invalid_header());
    }
    if view_get_u8(data, 2) != 8 {
        return _make_err(code_invalid_header());
    }
    let flg = view_get_u8(data, 3);
    if (flg & 224) != 0 {
        return _make_err(code_invalid_header());
    }
    let mut off = 10;

    if (flg & 4) != 0 {
        if lt_u(n, off + 2) {
            return _make_err(code_truncated());
        }
        let xlen = _read_u16_le(data, off);
        off = off + 2;
        if lt_u(n, off + xlen) {
            return _make_err(code_truncated());
        }
        off = off + xlen;
    }

    if (flg & 8) != 0 {
        let mut found = 0;
        for i in off..n {
            if found == 0 && view_get_u8(data, i) == 0 {
                found = 1;
                off = i + 1;
            }
        }
        if found == 0 {
            return _make_err(code_truncated());
        }
    }

    if (flg & 16) != 0 {
        let mut found = 0;
        for i in off..n {
            if found == 0 && view_get_u8(data, i) == 0 {
                found = 1;
                off = i + 1;
            }
        }
        if found == 0 {
            return _make_err(code_truncated());
        }
    }

    if (flg & 2) != 0 {
        if lt_u(n, off + 2) {
            return _make_err(code_truncated());
        }
        off = off + 2;
    }

    if lt_u(n, off + 8) {
        return _make_err(code_truncated());
    }

    let expected_crc = _read_u32_le(data, n - 8);
    let expected_isize = _read_u32_le(data, n - 4);

    let deflate_len = (n - 8) - off;
    let deflate = view_slice(data, off, deflate_len);
    let doc = _inflate_deflate(deflate, max_out_bytes);
    if is_err(bytes_view(doc)) {
        return doc;
    }

    let actual_crc = _crc32(get_view(bytes_view(doc)));
    let actual_isize = out_len(bytes_view(doc));
    if expected_crc != actual_crc {
        return _make_err(code_checksum_mismatch());
    }
    if expected_isize != actual_isize {
        return _make_err(code_checksum_mismatch());
    }
    doc
}
