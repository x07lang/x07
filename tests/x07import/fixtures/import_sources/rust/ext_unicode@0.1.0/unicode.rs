// Unified Unicode utilities for X07 external packages (x07import-compatible Rust subset)
//
// Result bytes format (shared across APIs):
//   Error:   [0x00][u32_le code][u32_le msg_len=0]
//   Success: [0x01][payload_bytes...]
//
// Error codes:
//   1  invalid UTF-8
//   2  invalid u32le payload length (not multiple of 4)
//   3  invalid Unicode scalar value (surrogate or > U+10FFFF)
//   4  invalid UTF-16 payload length (not multiple of 2)
//   5  invalid UTF-16 surrogate sequence
//   6  cannot encode to Latin-1
//   7  cannot encode to Windows-1252
//   9  invalid Windows-1252 byte (undefined)

fn _cont_range(c: i32, lo: i32, hi: i32) -> bool {
    if ge_u(c, lo) {
        lt_u(c, hi + 1)
    } else {
        false
    }
}

fn _is_cont(c: i32) -> bool {
    if ge_u(c, 128) {
        if lt_u(c, 192) {
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn _pack_utf8(cp: i32, len: i32) -> i32 {
    cp + (len << 24)
}

fn _utf8_decode_packed_or_neg1(b: BytesView, i: i32, n: i32) -> i32 {
    let b1 = bytes_get_u8(b, i);
    if lt_u(b1, 128) {
        return _pack_utf8(b1, 1);
    }

    // Disallow overlong 2-byte sequences.
    if lt_u(b1, 194) {
        return -1;
    }

    if lt_u(b1, 224) {
        if ge_u(i + 1, n) {
            return -1;
        }
        let b2 = bytes_get_u8(b, i + 1);
        if !_is_cont(b2) {
            return -1;
        }
        let cp = ((b1 & 31) << 6) | (b2 & 63);
        return _pack_utf8(cp, 2);
    }

    if ge_u(i + 2, n) {
        return -1;
    }
    let b2 = bytes_get_u8(b, i + 1);
    let b3 = bytes_get_u8(b, i + 2);

    if b1 == 224 {
        if !_cont_range(b2, 160, 191) {
            return -1;
        }
    } else if b1 == 237 {
        // Disallow surrogates.
        if !_cont_range(b2, 128, 159) {
            return -1;
        }
    } else {
        if !_is_cont(b2) {
            return -1;
        }
    }
    if !_is_cont(b3) {
        return -1;
    }

    if lt_u(b1, 240) {
        let cp = ((b1 & 15) << 12) | ((b2 & 63) << 6) | (b3 & 63);
        return _pack_utf8(cp, 3);
    }

    if ge_u(i + 3, n) {
        return -1;
    }
    let b4 = bytes_get_u8(b, i + 3);
    if b1 == 240 {
        if !_cont_range(b2, 144, 191) {
            return -1;
        }
    } else if b1 == 244 {
        if !_cont_range(b2, 128, 143) {
            return -1;
        }
    } else {
        if !lt_u(b1, 244) {
            return -1;
        }
        if !_is_cont(b2) {
            return -1;
        }
    }
    if !_is_cont(b3) {
        return -1;
    }
    if !_is_cont(b4) {
        return -1;
    }

    let cp = ((b1 & 7) << 18) | ((b2 & 63) << 12) | ((b3 & 63) << 6) | (b4 & 63);
    _pack_utf8(cp, 4)
}

fn _utf8_push_cp(out: VecU8, cp: i32) -> VecU8 {
    let mut o = out;
    if lt_u(cp, 128) {
        o = vec_u8_push(o, cp);
        return o;
    }
    if lt_u(cp, 2048) {
        o = vec_u8_push(o, 192 | (cp >> 6));
        o = vec_u8_push(o, 128 | (cp & 63));
        return o;
    }
    if lt_u(cp, 65536) {
        o = vec_u8_push(o, 224 | (cp >> 12));
        o = vec_u8_push(o, 128 | ((cp >> 6) & 63));
        o = vec_u8_push(o, 128 | (cp & 63));
        return o;
    }
    o = vec_u8_push(o, 240 | (cp >> 18));
    o = vec_u8_push(o, 128 | ((cp >> 12) & 63));
    o = vec_u8_push(o, 128 | ((cp >> 6) & 63));
    o = vec_u8_push(o, 128 | (cp & 63));
    o
}

fn _push_u16_le(out: VecU8, u: i32) -> VecU8 {
    let mut o = out;
    o = vec_u8_push(o, u & 255);
    o = vec_u8_push(o, (u >> 8) & 255);
    o
}

fn _push_u16_be(out: VecU8, u: i32) -> VecU8 {
    let mut o = out;
    o = vec_u8_push(o, (u >> 8) & 255);
    o = vec_u8_push(o, u & 255);
    o
}

fn _read_u16_le(b: BytesView, off: i32) -> i32 {
    let lo = view_get_u8(b, off);
    let hi = view_get_u8(b, off + 1);
    lo | (hi << 8)
}

fn _read_u16_be(b: BytesView, off: i32) -> i32 {
    let hi = view_get_u8(b, off);
    let lo = view_get_u8(b, off + 1);
    lo | (hi << 8)
}

fn _push_u32_le(mut out: VecU8, x: i32) -> VecU8 {
    let b = codec_write_u32_le(x);
    out = vec_u8_extend_bytes_range(out, b, 0, bytes_len(b));
    out
}

fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

fn _finish_slices_doc_x7sl_v1(out: VecU8, count: i32) -> Bytes {
    let mut doc = vec_u8_into_bytes(out);
    let count_bytes = codec_write_u32_le(count);
    doc = bytes_set_u8(doc, 9, bytes_get_u8(count_bytes, 0));
    doc = bytes_set_u8(doc, 10, bytes_get_u8(count_bytes, 1));
    doc = bytes_set_u8(doc, 11, bytes_get_u8(count_bytes, 2));
    doc = bytes_set_u8(doc, 12, bytes_get_u8(count_bytes, 3));
    doc
}

fn _is_regional_indicator(cp: i32) -> bool {
    ge_u(cp, 127462) && lt_u(cp, 127488)
}

fn _is_variation_selector(cp: i32) -> bool {
    if ge_u(cp, 65024) {
        if lt_u(cp, 65040) {
            return true;
        }
    }
    ge_u(cp, 917760) && lt_u(cp, 917999)
}

fn _is_emoji_modifier(cp: i32) -> bool {
    ge_u(cp, 127995) && lt_u(cp, 128000)
}

fn _is_combining_mark_basic(cp: i32) -> bool {
    // Common combining diacritical mark blocks (not exhaustive).
    if ge_u(cp, 768) && lt_u(cp, 880) {
        return true;
    }
    if ge_u(cp, 6832) && lt_u(cp, 6912) {
        return true;
    }
    if ge_u(cp, 7616) && lt_u(cp, 7680) {
        return true;
    }
    if ge_u(cp, 8400) && lt_u(cp, 8448) {
        return true;
    }
    ge_u(cp, 65056) && lt_u(cp, 65072)
}

fn _is_extend(cp: i32) -> bool {
    _is_combining_mark_basic(cp) || _is_variation_selector(cp) || _is_emoji_modifier(cp)
}

fn _win1252_decode_byte(b: i32) -> i32 {
    if lt_u(b, 128) {
        return b;
    }
    if ge_u(b, 160) {
        return b;
    }
    if b == 128 {
        return 8364;
    }
    if b == 130 {
        return 8218;
    }
    if b == 131 {
        return 402;
    }
    if b == 132 {
        return 8222;
    }
    if b == 133 {
        return 8230;
    }
    if b == 134 {
        return 8224;
    }
    if b == 135 {
        return 8225;
    }
    if b == 136 {
        return 710;
    }
    if b == 137 {
        return 8240;
    }
    if b == 138 {
        return 352;
    }
    if b == 139 {
        return 8249;
    }
    if b == 140 {
        return 338;
    }
    if b == 142 {
        return 381;
    }
    if b == 145 {
        return 8216;
    }
    if b == 146 {
        return 8217;
    }
    if b == 147 {
        return 8220;
    }
    if b == 148 {
        return 8221;
    }
    if b == 149 {
        return 8226;
    }
    if b == 150 {
        return 8211;
    }
    if b == 151 {
        return 8212;
    }
    if b == 152 {
        return 732;
    }
    if b == 153 {
        return 8482;
    }
    if b == 154 {
        return 353;
    }
    if b == 155 {
        return 8250;
    }
    if b == 156 {
        return 339;
    }
    if b == 158 {
        return 382;
    }
    if b == 159 {
        return 376;
    }
    -1
}

fn _win1252_encode_cp(cp: i32) -> i32 {
    if lt_u(cp, 128) {
        return cp;
    }
    if ge_u(cp, 160) && lt_u(cp, 256) {
        return cp;
    }
    if cp == 8364 {
        return 128;
    }
    if cp == 8218 {
        return 130;
    }
    if cp == 402 {
        return 131;
    }
    if cp == 8222 {
        return 132;
    }
    if cp == 8230 {
        return 133;
    }
    if cp == 8224 {
        return 134;
    }
    if cp == 8225 {
        return 135;
    }
    if cp == 710 {
        return 136;
    }
    if cp == 8240 {
        return 137;
    }
    if cp == 352 {
        return 138;
    }
    if cp == 8249 {
        return 139;
    }
    if cp == 338 {
        return 140;
    }
    if cp == 381 {
        return 142;
    }
    if cp == 8216 {
        return 145;
    }
    if cp == 8217 {
        return 146;
    }
    if cp == 8220 {
        return 147;
    }
    if cp == 8221 {
        return 148;
    }
    if cp == 8226 {
        return 149;
    }
    if cp == 8211 {
        return 150;
    }
    if cp == 8212 {
        return 151;
    }
    if cp == 732 {
        return 152;
    }
    if cp == 8482 {
        return 153;
    }
    if cp == 353 {
        return 154;
    }
    if cp == 8250 {
        return 155;
    }
    if cp == 339 {
        return 156;
    }
    if cp == 382 {
        return 158;
    }
    if cp == 376 {
        return 159;
    }
    -1
}

pub fn unicode_is_err(doc: BytesView) -> bool {
    if lt_u(view_len(doc), 1) {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn unicode_err_code(doc: BytesView) -> i32 {
    if lt_u(view_len(doc), 5) {
        return 0;
    }
    if view_get_u8(doc, 0) != 0 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn unicode_get_bytes(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if lt_u(n, 1) {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 1, n - 1))
}

pub fn unicode_utf8_is_valid(b: BytesView) -> bool {
    let n = bytes_len(b);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return true;
        }
        let packed = _utf8_decode_packed_or_neg1(b, i, n);
        if packed < 0 {
            return false;
        }
        let len = packed >> 24;
        i = i + len;
    }
    true
}

pub fn unicode_utf8_decode_u32le(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(1 + (n * 4));
    out = vec_u8_push(out, 1);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return vec_u8_into_bytes(out);
        }
        let packed = _utf8_decode_packed_or_neg1(b, i, n);
        if packed < 0 {
            return _make_err(1);
        }
        let cp = packed & 16777215;
        let len = packed >> 24;
        out = vec_u8_extend_bytes(out, codec_write_u32_le(cp));
        i = i + len;
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_utf8_encode_u32le(codepoints_u32le: BytesView) -> Bytes {
    let n = view_len(codepoints_u32le);
    if (n % 4) != 0 {
        return _make_err(2);
    }
    let mut out = vec_u8_with_capacity(1 + n);
    out = vec_u8_push(out, 1);
    let mut off = 0;
    for _ in 0..n {
        if ge_u(off, n) {
            return vec_u8_into_bytes(out);
        }
        let cp = codec_read_u32_le(codepoints_u32le, off);
        if ge_u(cp, 1114112) {
            return _make_err(3);
        }
        if ge_u(cp, 55296) && lt_u(cp, 57344) {
            return _make_err(3);
        }
        out = _utf8_push_cp(out, cp);
        off = off + 4;
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_decode_latin1_to_utf8(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(1 + (n * 2));
    out = vec_u8_push(out, 1);
    for i in 0..n {
        out = _utf8_push_cp(out, view_get_u8(b, i));
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_decode_windows1252_to_utf8(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(1 + (n * 3));
    out = vec_u8_push(out, 1);
    for i in 0..n {
        let byte = view_get_u8(b, i);
        let cp = _win1252_decode_byte(byte);
        if cp < 0 {
            return _make_err(9);
        }
        out = _utf8_push_cp(out, cp);
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_encode_utf8_to_latin1(utf8: BytesView) -> Bytes {
    let n = view_len(utf8);
    let mut out = vec_u8_with_capacity(1 + n);
    out = vec_u8_push(out, 1);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return vec_u8_into_bytes(out);
        }
        let packed = _utf8_decode_packed_or_neg1(utf8, i, n);
        if packed < 0 {
            return _make_err(1);
        }
        let cp = packed & 16777215;
        let len = packed >> 24;
        if ge_u(cp, 256) {
            return _make_err(6);
        }
        out = vec_u8_push(out, cp);
        i = i + len;
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_encode_utf8_to_windows1252(utf8: BytesView) -> Bytes {
    let n = view_len(utf8);
    let mut out = vec_u8_with_capacity(1 + n);
    out = vec_u8_push(out, 1);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return vec_u8_into_bytes(out);
        }
        let packed = _utf8_decode_packed_or_neg1(utf8, i, n);
        if packed < 0 {
            return _make_err(1);
        }
        let cp = packed & 16777215;
        let len = packed >> 24;
        let b = _win1252_encode_cp(cp);
        if b < 0 {
            return _make_err(7);
        }
        out = vec_u8_push(out, b);
        i = i + len;
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_decode_utf16_le_to_utf8(b: BytesView) -> Bytes {
    let n = view_len(b);
    if (n % 2) != 0 {
        return _make_err(4);
    }
    let mut out = vec_u8_with_capacity(1 + n);
    out = vec_u8_push(out, 1);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return vec_u8_into_bytes(out);
        }
        let u = _read_u16_le(b, i);
        i = i + 2;
        if ge_u(u, 55296) && lt_u(u, 56320) {
            if ge_u(i, n) {
                return _make_err(5);
            }
            let u2 = _read_u16_le(b, i);
            i = i + 2;
            if !(ge_u(u2, 56320) && lt_u(u2, 57344)) {
                return _make_err(5);
            }
            let cp = 65536 + (((u - 55296) << 10) | (u2 - 56320));
            out = _utf8_push_cp(out, cp);
        } else if ge_u(u, 56320) && lt_u(u, 57344) {
            return _make_err(5);
        } else {
            out = _utf8_push_cp(out, u);
        }
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_decode_utf16_be_to_utf8(b: BytesView) -> Bytes {
    let n = view_len(b);
    if (n % 2) != 0 {
        return _make_err(4);
    }
    let mut out = vec_u8_with_capacity(1 + n);
    out = vec_u8_push(out, 1);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return vec_u8_into_bytes(out);
        }
        let u = _read_u16_be(b, i);
        i = i + 2;
        if ge_u(u, 55296) && lt_u(u, 56320) {
            if ge_u(i, n) {
                return _make_err(5);
            }
            let u2 = _read_u16_be(b, i);
            i = i + 2;
            if !(ge_u(u2, 56320) && lt_u(u2, 57344)) {
                return _make_err(5);
            }
            let cp = 65536 + (((u - 55296) << 10) | (u2 - 56320));
            out = _utf8_push_cp(out, cp);
        } else if ge_u(u, 56320) && lt_u(u, 57344) {
            return _make_err(5);
        } else {
            out = _utf8_push_cp(out, u);
        }
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_encode_utf8_to_utf16_le(utf8: BytesView) -> Bytes {
    let n = view_len(utf8);
    let mut out = vec_u8_with_capacity(1 + (n * 2));
    out = vec_u8_push(out, 1);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return vec_u8_into_bytes(out);
        }
        let packed = _utf8_decode_packed_or_neg1(utf8, i, n);
        if packed < 0 {
            return _make_err(1);
        }
        let cp = packed & 16777215;
        let len = packed >> 24;
        if ge_u(cp, 1114112) {
            return _make_err(3);
        }
        if ge_u(cp, 55296) && lt_u(cp, 57344) {
            return _make_err(3);
        }
        if lt_u(cp, 65536) {
            out = _push_u16_le(out, cp);
        } else {
            let x = cp - 65536;
            let hi = 55296 + (x >> 10);
            let lo = 56320 + (x & 1023);
            out = _push_u16_le(out, hi);
            out = _push_u16_le(out, lo);
        }
        i = i + len;
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_encode_utf8_to_utf16_be(utf8: BytesView) -> Bytes {
    let n = view_len(utf8);
    let mut out = vec_u8_with_capacity(1 + (n * 2));
    out = vec_u8_push(out, 1);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return vec_u8_into_bytes(out);
        }
        let packed = _utf8_decode_packed_or_neg1(utf8, i, n);
        if packed < 0 {
            return _make_err(1);
        }
        let cp = packed & 16777215;
        let len = packed >> 24;
        if ge_u(cp, 1114112) {
            return _make_err(3);
        }
        if ge_u(cp, 55296) && lt_u(cp, 57344) {
            return _make_err(3);
        }
        if lt_u(cp, 65536) {
            out = _push_u16_be(out, cp);
        } else {
            let x = cp - 65536;
            let hi = 55296 + (x >> 10);
            let lo = 56320 + (x & 1023);
            out = _push_u16_be(out, hi);
            out = _push_u16_be(out, lo);
        }
        i = i + len;
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_nfkc_basic(utf8: BytesView) -> Bytes {
    let n = view_len(utf8);
    let mut out = vec_u8_with_capacity(1 + n);
    out = vec_u8_push(out, 1);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return vec_u8_into_bytes(out);
        }
        let packed = _utf8_decode_packed_or_neg1(utf8, i, n);
        if packed < 0 {
            return _make_err(1);
        }
        let cp0 = packed & 16777215;
        let len = packed >> 24;
        let mut cp = cp0;
        // U+3000 IDEOGRAPHIC SPACE -> ASCII space.
        if cp == 12288 {
            cp = 32;
        } else if cp == 160 {
            // U+00A0 NO-BREAK SPACE -> ASCII space.
            cp = 32;
        } else if ge_u(cp, 65281) && lt_u(cp, 65375) {
            // U+FF01..U+FF5E FULLWIDTH ASCII variants.
            cp = cp - 65248;
        }
        out = _utf8_push_cp(out, cp);
        i = i + len;
    }
    vec_u8_into_bytes(out)
}

pub fn unicode_grapheme_slices(utf8: BytesView) -> Bytes {
    let n = view_len(utf8);
    let mut out = vec_u8_with_capacity(13 + (n * 8));
    out = vec_u8_push(out, 1);
    out = vec_u8_push(out, 69);
    out = vec_u8_push(out, 86);
    out = vec_u8_push(out, 83);
    out = vec_u8_push(out, 76);
    out = _push_u32_le(out, 1);
    out = _push_u32_le(out, 0);

    let mut count = 0;

    let mut i = 0;
    let mut ri_pending = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return _finish_slices_doc_x7sl_v1(out, count);
        }

        let start = i;
        let packed = _utf8_decode_packed_or_neg1(utf8, i, n);
        if packed < 0 {
            return _make_err(1);
        }
        let cp = packed & 16777215;
        let len = packed >> 24;
        i = i + len;
        let mut end = i;

        let mut handled = false;

        // CRLF is a single grapheme cluster.
        if cp == 13 {
            if lt_u(i, n) {
                let p2 = _utf8_decode_packed_or_neg1(utf8, i, n);
                if p2 < 0 {
                    return _make_err(1);
                }
                let cp2 = p2 & 16777215;
                let len2 = p2 >> 24;
                if cp2 == 10 {
                    i = i + len2;
                    end = i;
                }
            }
            out = _push_u32_le(out, start);
            out = _push_u32_le(out, end - start);
            count = count + 1;
            ri_pending = 0;
            handled = true;
        }

        // Regional indicator pairing (flags).
        if !handled && _is_regional_indicator(cp) {
            if ri_pending == 0 {
                ri_pending = 1;
                if lt_u(i, n) {
                    let p2 = _utf8_decode_packed_or_neg1(utf8, i, n);
                    if p2 < 0 {
                        return _make_err(1);
                    }
                    let cp2 = p2 & 16777215;
                    let len2 = p2 >> 24;
                    if _is_regional_indicator(cp2) {
                        i = i + len2;
                        end = i;
                        ri_pending = 0;
                    }
                }
            } else {
                ri_pending = 0;
            }
            out = _push_u32_le(out, start);
            out = _push_u32_le(out, end - start);
            count = count + 1;
            handled = true;
        } else {
            if !handled {
                ri_pending = 0;
            }
        }

        // Extend grapheme with combining marks, variation selectors, emoji modifiers,
        // and a simplified ZWJ-joiner rule.
        if !handled {
            let mut done = false;
            for _ in 0..n {
                if !done {
                    if lt_u(i, n) {
                        let p2 = _utf8_decode_packed_or_neg1(utf8, i, n);
                        if p2 < 0 {
                            return _make_err(1);
                        }
                        let cp2 = p2 & 16777215;
                        let len2 = p2 >> 24;

                        if _is_extend(cp2) {
                            i = i + len2;
                            end = i;
                        } else if cp2 == 8205 {
                            // U+200D ZERO WIDTH JOINER: include joiner and the next scalar.
                            i = i + len2;
                            end = i;
                            if lt_u(i, n) {
                                let p3 = _utf8_decode_packed_or_neg1(utf8, i, n);
                                if p3 < 0 {
                                    return _make_err(1);
                                }
                                let len3 = p3 >> 24;
                                i = i + len3;
                                end = i;
                            } else {
                                done = true;
                            }
                        } else {
                            done = true;
                        }
                    } else {
                        done = true;
                    }
                }
            }

            out = _push_u32_le(out, start);
            out = _push_u32_le(out, end - start);
            count = count + 1;
        }
    }

    _finish_slices_doc_x7sl_v1(out, count)
}
