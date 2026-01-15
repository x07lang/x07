fn _rotr(x: i32, n: i32) -> i32 {
    (x >> n) | (x << (32 - n))
}

fn _ch(x: i32, y: i32, z: i32) -> i32 {
    (x & y) ^ ((x ^ (0 - 1)) & z)
}

fn _maj(x: i32, y: i32, z: i32) -> i32 {
    (x & y) ^ (x & z) ^ (y & z)
}

fn _big_sigma0(x: i32) -> i32 {
    _rotr(x, 2) ^ _rotr(x, 13) ^ _rotr(x, 22)
}

fn _big_sigma1(x: i32) -> i32 {
    _rotr(x, 6) ^ _rotr(x, 11) ^ _rotr(x, 25)
}

fn _small_sigma0(x: i32) -> i32 {
    _rotr(x, 7) ^ _rotr(x, 18) ^ (x >> 3)
}

fn _small_sigma1(x: i32) -> i32 {
    _rotr(x, 17) ^ _rotr(x, 19) ^ (x >> 10)
}

fn _read_u32_be(b: BytesView, off: i32) -> i32 {
    let b0 = view_get_u8(b, off);
    let b1 = view_get_u8(b, off + 1);
    let b2 = view_get_u8(b, off + 2);
    let b3 = view_get_u8(b, off + 3);
    ((b0 << 24) | (b1 << 16) | (b2 << 8) | b3)
}

fn _push_u32_be(mut out: VecU8, x: i32) -> VecU8 {
    out = vec_u8_push(out, (x >> 24) & 255);
    out = vec_u8_push(out, (x >> 16) & 255);
    out = vec_u8_push(out, (x >> 8) & 255);
    vec_u8_push(out, x & 255)
}

fn _sha256_k_bytes() -> Bytes {
    let mut v = vec_u8_with_capacity(64 * 4);
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1116352408));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1899447441));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1245643825));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-373957723));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(961987163));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1508970993));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1841331548));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1424204075));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-670586216));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(310598401));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(607225278));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1426881987));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1925078388));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-2132889090));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1680079193));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1046744716));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-459576895));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-272742522));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(264347078));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(604807628));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(770255983));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1249150122));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1555081692));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1996064986));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1740746414));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1473132947));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1341970488));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1084653625));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-958395405));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-710438585));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(113926993));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(338241895));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(666307205));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(773529912));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1294757372));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1396182291));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1695183700));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1986661051));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-2117940946));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1838011259));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1564481375));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1474664885));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1035236496));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-949202525));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-778901479));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-694614492));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-200395387));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(275423344));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(430227734));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(506948616));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(659060556));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(883997877));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(958139571));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1322822218));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1537002063));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1747873779));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1955562222));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(2024104815));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-2067236844));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1933114872));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1866530822));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1538233109));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1090935817));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-965641998));
    vec_u8_into_bytes(v)
}

fn _sha256_init_state() -> Bytes {
    let mut out = vec_u8_with_capacity(32);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(1779033703));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(-1150833019));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(1013904242));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(-1521486534));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(1359893119));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(-1694144372));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(528734635));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(1541459225));
    vec_u8_into_bytes(out)
}

fn _sha256_compress(state: Bytes, block: BytesView, k: BytesView) -> Bytes {
    let st = bytes_view(state);
    let h0_0 = codec_read_u32_le(st, 0);
    let h1_0 = codec_read_u32_le(st, 4);
    let h2_0 = codec_read_u32_le(st, 8);
    let h3_0 = codec_read_u32_le(st, 12);
    let h4_0 = codec_read_u32_le(st, 16);
    let h5_0 = codec_read_u32_le(st, 20);
    let h6_0 = codec_read_u32_le(st, 24);
    let h7_0 = codec_read_u32_le(st, 28);

    let mut w = vec_u8_with_capacity(64 * 4);
    for t in 0..16 {
        let wt = _read_u32_be(block, t * 4);
        w = vec_u8_extend_bytes(w, codec_write_u32_le(wt));
    }
    for t in 16..64 {
        let w2 = codec_read_u32_le(vec_u8_as_view(w), (t - 2) * 4);
        let w7 = codec_read_u32_le(vec_u8_as_view(w), (t - 7) * 4);
        let w15 = codec_read_u32_le(vec_u8_as_view(w), (t - 15) * 4);
        let w16 = codec_read_u32_le(vec_u8_as_view(w), (t - 16) * 4);
        let s0 = _small_sigma0(w15);
        let s1 = _small_sigma1(w2);
        let wt = (((w16 + s0) + w7) + s1);
        w = vec_u8_extend_bytes(w, codec_write_u32_le(wt));
    }

    let wv = vec_u8_as_view(w);
    let mut a = h0_0;
    let mut b = h1_0;
    let mut c = h2_0;
    let mut d = h3_0;
    let mut e = h4_0;
    let mut f = h5_0;
    let mut g = h6_0;
    let mut h = h7_0;

    for t in 0..64 {
        let kt = codec_read_u32_le(k, t * 4);
        let wt = codec_read_u32_le(wv, t * 4);
        let t1 = ((((h + _big_sigma1(e)) + _ch(e, f, g)) + kt) + wt);
        let t2 = _big_sigma0(a) + _maj(a, b, c);

        h = g;
        g = f;
        f = e;
        e = d + t1;
        d = c;
        c = b;
        b = a;
        a = t1 + t2;
    }

    let h0 = h0_0 + a;
    let h1 = h1_0 + b;
    let h2 = h2_0 + c;
    let h3 = h3_0 + d;
    let h4 = h4_0 + e;
    let h5 = h5_0 + f;
    let h6 = h6_0 + g;
    let h7 = h7_0 + h;

    let mut out = vec_u8_with_capacity(32);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h0));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h1));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h2));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h3));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h4));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h5));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h6));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h7));
    vec_u8_into_bytes(out)
}

pub fn sha256(msg: BytesView) -> Bytes {
    let k_bytes = _sha256_k_bytes();
    let k = bytes_view(k_bytes);
    let mut state = _sha256_init_state();

    let n = view_len(msg);
    let mut off = 0;
    for _ in 0..n {
        if lt_u(off + 63, n) {
            let block = view_slice(msg, off, 64);
            state = _sha256_compress(state, block, k);
            off = off + 64;
        }
    }

    let rem = n - off;
    let mut tail = vec_u8_with_capacity(128);
    if rem > 0 {
        tail = vec_u8_extend_bytes_range(tail, msg, off, rem);
    }
    tail = vec_u8_push(tail, 128);

    let cur_mod = vec_u8_len(tail) % 64;
    let pad_len = if cur_mod <= 56 {
        56 - cur_mod
    } else {
        56 + (64 - cur_mod)
    };
    for _ in 0..pad_len {
        tail = vec_u8_push(tail, 0);
    }

    let bit_lo = n << 3;
    let bit_hi = n >> 29;
    tail = vec_u8_push(tail, (bit_hi >> 24) & 255);
    tail = vec_u8_push(tail, (bit_hi >> 16) & 255);
    tail = vec_u8_push(tail, (bit_hi >> 8) & 255);
    tail = vec_u8_push(tail, bit_hi & 255);
    tail = vec_u8_push(tail, (bit_lo >> 24) & 255);
    tail = vec_u8_push(tail, (bit_lo >> 16) & 255);
    tail = vec_u8_push(tail, (bit_lo >> 8) & 255);
    tail = vec_u8_push(tail, bit_lo & 255);

    let tail_b = vec_u8_into_bytes(tail);
    let tail_v = bytes_view(tail_b);
    let tail_n = view_len(tail_v);
    let mut tail_off = 0;
    for _ in 0..tail_n {
        if lt_u(tail_off + 63, tail_n) {
            let block = view_slice(tail_v, tail_off, 64);
            state = _sha256_compress(state, block, k);
            tail_off = tail_off + 64;
        }
    }

    let st = bytes_view(state);
    let mut out = vec_u8_with_capacity(32);
    for i in 0..8 {
        let w = codec_read_u32_le(st, i * 4);
        out = _push_u32_be(out, w);
    }
    vec_u8_into_bytes(out)
}

pub fn hmac_sha256(key: BytesView, msg: BytesView) -> Bytes {
    let key_len = view_len(key);
    let key2 = if key_len > 64 { sha256(key) } else { bytes_alloc(0) };
    let keyv = if key_len > 64 { bytes_view(key2) } else { key };
    let keyv_len = view_len(keyv);

    let mut ipad = vec_u8_with_capacity(64);
    let mut opad = vec_u8_with_capacity(64);
    for i in 0..64 {
        let kb = if lt_u(i, keyv_len) { view_get_u8(keyv, i) } else { 0 };
        ipad = vec_u8_push(ipad, kb ^ 54);
        opad = vec_u8_push(opad, kb ^ 92);
    }

    let mut inner = vec_u8_with_capacity(64 + view_len(msg));
    inner = vec_u8_extend_bytes(inner, vec_u8_into_bytes(ipad));
    inner = vec_u8_extend_bytes_range(inner, msg, 0, view_len(msg));
    let inner_b = vec_u8_into_bytes(inner);
    let inner_h = sha256(bytes_view(inner_b));

    let mut outer = vec_u8_with_capacity(64 + 32);
    outer = vec_u8_extend_bytes(outer, vec_u8_into_bytes(opad));
    outer = vec_u8_extend_bytes(outer, inner_h);
    let outer_b = vec_u8_into_bytes(outer);
    sha256(bytes_view(outer_b))
}

// 64-bit helpers (hi/lo u32 halves in i32).
// Similar to ext.u64, but kept private here to avoid cross-package deps.

fn _u64_add_lo(a_lo: i32, b_lo: i32) -> i32 {
    a_lo + b_lo
}

fn _u64_add_hi(a_lo: i32, a_hi: i32, b_lo: i32, b_hi: i32, sum_lo: i32) -> i32 {
    let carry = if lt_u(sum_lo, a_lo) { 1 } else { 0 };
    a_hi + b_hi + carry
}

fn _u64_rotr_lo(lo: i32, hi: i32, n: i32) -> i32 {
    if n == 0 {
        lo
    } else if lt_u(n, 32) {
        (lo >> n) | (hi << (32 - n))
    } else if n == 32 {
        hi
    } else if lt_u(n, 64) {
        (hi >> (n - 32)) | (lo << (64 - n))
    } else {
        0
    }
}

fn _u64_rotr_hi(lo: i32, hi: i32, n: i32) -> i32 {
    if n == 0 {
        hi
    } else if lt_u(n, 32) {
        (hi >> n) | (lo << (32 - n))
    } else if n == 32 {
        lo
    } else if lt_u(n, 64) {
        (lo >> (n - 32)) | (hi << (64 - n))
    } else {
        0
    }
}

fn _u64_shr_u_lo(lo: i32, hi: i32, n: i32) -> i32 {
    if n == 0 {
        lo
    } else if lt_u(n, 32) {
        (lo >> n) | (hi << (32 - n))
    } else if n == 32 {
        hi
    } else if lt_u(n, 64) {
        hi >> (n - 32)
    } else {
        0
    }
}

fn _u64_shr_u_hi(lo: i32, hi: i32, n: i32) -> i32 {
    if n == 0 {
        hi
    } else if lt_u(n, 32) {
        hi >> n
    } else if lt_u(n, 64) {
        0
    } else {
        0
    }
}

fn _sha512_ch_lo(x_lo: i32, y_lo: i32, z_lo: i32) -> i32 {
    (x_lo & y_lo) ^ ((x_lo ^ (0 - 1)) & z_lo)
}

fn _sha512_ch_hi(x_hi: i32, y_hi: i32, z_hi: i32) -> i32 {
    (x_hi & y_hi) ^ ((x_hi ^ (0 - 1)) & z_hi)
}

fn _sha512_maj_lo(x_lo: i32, y_lo: i32, z_lo: i32) -> i32 {
    (x_lo & y_lo) ^ (x_lo & z_lo) ^ (y_lo & z_lo)
}

fn _sha512_maj_hi(x_hi: i32, y_hi: i32, z_hi: i32) -> i32 {
    (x_hi & y_hi) ^ (x_hi & z_hi) ^ (y_hi & z_hi)
}

fn _sha512_big_sigma0_lo(lo: i32, hi: i32) -> i32 {
    _u64_rotr_lo(lo, hi, 28) ^ _u64_rotr_lo(lo, hi, 34) ^ _u64_rotr_lo(lo, hi, 39)
}

fn _sha512_big_sigma0_hi(lo: i32, hi: i32) -> i32 {
    _u64_rotr_hi(lo, hi, 28) ^ _u64_rotr_hi(lo, hi, 34) ^ _u64_rotr_hi(lo, hi, 39)
}

fn _sha512_big_sigma1_lo(lo: i32, hi: i32) -> i32 {
    _u64_rotr_lo(lo, hi, 14) ^ _u64_rotr_lo(lo, hi, 18) ^ _u64_rotr_lo(lo, hi, 41)
}

fn _sha512_big_sigma1_hi(lo: i32, hi: i32) -> i32 {
    _u64_rotr_hi(lo, hi, 14) ^ _u64_rotr_hi(lo, hi, 18) ^ _u64_rotr_hi(lo, hi, 41)
}

fn _sha512_small_sigma0_lo(lo: i32, hi: i32) -> i32 {
    _u64_rotr_lo(lo, hi, 1) ^ _u64_rotr_lo(lo, hi, 8) ^ _u64_shr_u_lo(lo, hi, 7)
}

fn _sha512_small_sigma0_hi(lo: i32, hi: i32) -> i32 {
    _u64_rotr_hi(lo, hi, 1) ^ _u64_rotr_hi(lo, hi, 8) ^ _u64_shr_u_hi(lo, hi, 7)
}

fn _sha512_small_sigma1_lo(lo: i32, hi: i32) -> i32 {
    _u64_rotr_lo(lo, hi, 19) ^ _u64_rotr_lo(lo, hi, 61) ^ _u64_shr_u_lo(lo, hi, 6)
}

fn _sha512_small_sigma1_hi(lo: i32, hi: i32) -> i32 {
    _u64_rotr_hi(lo, hi, 19) ^ _u64_rotr_hi(lo, hi, 61) ^ _u64_shr_u_hi(lo, hi, 6)
}

fn _sha512_k_bytes_0() -> Bytes {
    let mut v = vec_u8_with_capacity(40 * 8);
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-685199838));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1116352408));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(602891725));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1899447441));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-330482897));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1245643825));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-2121671748));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-373957723));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-213338824));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(961987163));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1241133031));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1508970993));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1357295717));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1841331548));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-630357736));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1424204075));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1560083902));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-670586216));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1164996542));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(310598401));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1323610764));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(607225278));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-704662302));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1426881987));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-226784913));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1925078388));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(991336113));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-2132889090));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(633803317));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1680079193));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-815192428));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1046744716));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1628353838));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-459576895));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(944711139));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-272742522));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1953704523));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(264347078));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(2007800933));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(604807628));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1495990901));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(770255983));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1856431235));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1249150122));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1119749164));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1555081692));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-2096016459));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1996064986));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-295247957));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1740746414));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(766784016));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1473132947));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1728372417));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1341970488));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1091629340));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1084653625));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1034457026));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-958395405));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1828018395));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-710438585));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-536640913));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(113926993));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(168717936));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(338241895));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1188179964));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(666307205));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1546045734));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(773529912));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1522805485));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1294757372));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1651133473));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1396182291));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1951439906));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1695183700));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1014477480));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1986661051));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1206759142));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-2117940946));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(344077627));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1838011259));
    vec_u8_into_bytes(v)
}

fn _sha512_k_bytes_1() -> Bytes {
    let mut v = vec_u8_with_capacity(40 * 8);
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1290863460));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1564481375));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1136513023));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1474664885));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-789014639));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1035236496));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(106217008));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-949202525));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-688958952));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-778901479));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1432725776));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-694614492));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1467031594));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-200395387));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(851169720));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(275423344));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1194143544));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(430227734));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1363258195));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(506948616));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-544281703));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(659060556));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-509917016));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(883997877));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-976659869));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(958139571));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-482243893));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1322822218));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(2003034995));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1537002063));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-692930397));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1747873779));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1575990012));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1955562222));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1125592928));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(2024104815));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1578062990));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-2067236844));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(442776044));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1933114872));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(593698344));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1866530822));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-561857047));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1538233109));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1295615723));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1090935817));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-479046869));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-965641998));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-366583396));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-903397682));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(566280711));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-779700025));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-840897762));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-354779690));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-294727304));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-176337025));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1914138554));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(116418474));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1563912026));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(174292421));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1090974290));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(289380356));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(320620315));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(460393269));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(587496836));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(685471733));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1086792851));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(852142971));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(365543100));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1017036298));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1676669620));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1126000580));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-885112138));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1288033470));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-60457430));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1501505948));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(987167468));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1607167915));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1246189591));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1816402316));
    vec_u8_into_bytes(v)
}

fn _sha512_k_bytes() -> Bytes {
    let k0 = _sha512_k_bytes_0();
    let k1 = _sha512_k_bytes_1();
    let mut v = vec_u8_with_capacity(80 * 8);
    v = vec_u8_extend_bytes(v, k0);
    v = vec_u8_extend_bytes(v, k1);
    vec_u8_into_bytes(v)
}

fn _sha512_init_state() -> Bytes {
    let mut v = vec_u8_with_capacity(64);
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-205731576));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1779033703));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-2067093701));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1150833019));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-23791573));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1013904242));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1595750129));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1521486534));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1377402159));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1359893119));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(725511199));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-1694144372));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(-79577749));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(528734635));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(327033209));
    v = vec_u8_extend_bytes(v, codec_write_u32_le(1541459225));
    vec_u8_into_bytes(v)
}

fn _sha512_compress(state: Bytes, block: BytesView, k: BytesView) -> Bytes {
    let st = bytes_view(state);
    let h0_lo_0 = codec_read_u32_le(st, 0);
    let h0_hi_0 = codec_read_u32_le(st, 4);
    let h1_lo_0 = codec_read_u32_le(st, 8);
    let h1_hi_0 = codec_read_u32_le(st, 12);
    let h2_lo_0 = codec_read_u32_le(st, 16);
    let h2_hi_0 = codec_read_u32_le(st, 20);
    let h3_lo_0 = codec_read_u32_le(st, 24);
    let h3_hi_0 = codec_read_u32_le(st, 28);
    let h4_lo_0 = codec_read_u32_le(st, 32);
    let h4_hi_0 = codec_read_u32_le(st, 36);
    let h5_lo_0 = codec_read_u32_le(st, 40);
    let h5_hi_0 = codec_read_u32_le(st, 44);
    let h6_lo_0 = codec_read_u32_le(st, 48);
    let h6_hi_0 = codec_read_u32_le(st, 52);
    let h7_lo_0 = codec_read_u32_le(st, 56);
    let h7_hi_0 = codec_read_u32_le(st, 60);

    let mut w = vec_u8_with_capacity(80 * 8);
    for t in 0..16 {
        let wi_hi = _read_u32_be(block, t * 8);
        let wi_lo = _read_u32_be(block, (t * 8) + 4);
        w = vec_u8_extend_bytes(w, codec_write_u32_le(wi_lo));
        w = vec_u8_extend_bytes(w, codec_write_u32_le(wi_hi));
    }
    for t in 16..80 {
        let w2_lo = codec_read_u32_le(vec_u8_as_view(w), (t - 2) * 8);
        let w2_hi = codec_read_u32_le(vec_u8_as_view(w), ((t - 2) * 8) + 4);
        let w7_lo = codec_read_u32_le(vec_u8_as_view(w), (t - 7) * 8);
        let w7_hi = codec_read_u32_le(vec_u8_as_view(w), ((t - 7) * 8) + 4);
        let w15_lo = codec_read_u32_le(vec_u8_as_view(w), (t - 15) * 8);
        let w15_hi = codec_read_u32_le(vec_u8_as_view(w), ((t - 15) * 8) + 4);
        let w16_lo = codec_read_u32_le(vec_u8_as_view(w), (t - 16) * 8);
        let w16_hi = codec_read_u32_le(vec_u8_as_view(w), ((t - 16) * 8) + 4);

        let s0_lo = _sha512_small_sigma0_lo(w15_lo, w15_hi);
        let s0_hi = _sha512_small_sigma0_hi(w15_lo, w15_hi);
        let s1_lo = _sha512_small_sigma1_lo(w2_lo, w2_hi);
        let s1_hi = _sha512_small_sigma1_hi(w2_lo, w2_hi);

        let t0_lo = _u64_add_lo(w16_lo, s0_lo);
        let t0_hi = _u64_add_hi(w16_lo, w16_hi, s0_lo, s0_hi, t0_lo);
        let t1_lo = _u64_add_lo(t0_lo, w7_lo);
        let t1_hi = _u64_add_hi(t0_lo, t0_hi, w7_lo, w7_hi, t1_lo);
        let wt_lo = _u64_add_lo(t1_lo, s1_lo);
        let wt_hi = _u64_add_hi(t1_lo, t1_hi, s1_lo, s1_hi, wt_lo);

        w = vec_u8_extend_bytes(w, codec_write_u32_le(wt_lo));
        w = vec_u8_extend_bytes(w, codec_write_u32_le(wt_hi));
    }

    let wv = vec_u8_as_view(w);

    let mut a_lo = h0_lo_0;
    let mut a_hi = h0_hi_0;
    let mut b_lo = h1_lo_0;
    let mut b_hi = h1_hi_0;
    let mut c_lo = h2_lo_0;
    let mut c_hi = h2_hi_0;
    let mut d_lo = h3_lo_0;
    let mut d_hi = h3_hi_0;
    let mut e_lo = h4_lo_0;
    let mut e_hi = h4_hi_0;
    let mut f_lo = h5_lo_0;
    let mut f_hi = h5_hi_0;
    let mut g_lo = h6_lo_0;
    let mut g_hi = h6_hi_0;
    let mut h_lo = h7_lo_0;
    let mut h_hi = h7_hi_0;

    for t in 0..80 {
        let kt_lo = codec_read_u32_le(k, t * 8);
        let kt_hi = codec_read_u32_le(k, (t * 8) + 4);
        let wt_lo = codec_read_u32_le(wv, t * 8);
        let wt_hi = codec_read_u32_le(wv, (t * 8) + 4);

        let s1_lo = _sha512_big_sigma1_lo(e_lo, e_hi);
        let s1_hi = _sha512_big_sigma1_hi(e_lo, e_hi);
        let ch_lo = _sha512_ch_lo(e_lo, f_lo, g_lo);
        let ch_hi = _sha512_ch_hi(e_hi, f_hi, g_hi);

        let t1a_lo = _u64_add_lo(h_lo, s1_lo);
        let t1a_hi = _u64_add_hi(h_lo, h_hi, s1_lo, s1_hi, t1a_lo);
        let t1b_lo = _u64_add_lo(t1a_lo, ch_lo);
        let t1b_hi = _u64_add_hi(t1a_lo, t1a_hi, ch_lo, ch_hi, t1b_lo);
        let t1c_lo = _u64_add_lo(t1b_lo, kt_lo);
        let t1c_hi = _u64_add_hi(t1b_lo, t1b_hi, kt_lo, kt_hi, t1c_lo);
        let t1_lo = _u64_add_lo(t1c_lo, wt_lo);
        let t1_hi = _u64_add_hi(t1c_lo, t1c_hi, wt_lo, wt_hi, t1_lo);

        let s0_lo = _sha512_big_sigma0_lo(a_lo, a_hi);
        let s0_hi = _sha512_big_sigma0_hi(a_lo, a_hi);
        let maj_lo = _sha512_maj_lo(a_lo, b_lo, c_lo);
        let maj_hi = _sha512_maj_hi(a_hi, b_hi, c_hi);
        let t2_lo = _u64_add_lo(s0_lo, maj_lo);
        let t2_hi = _u64_add_hi(s0_lo, s0_hi, maj_lo, maj_hi, t2_lo);

        h_lo = g_lo;
        h_hi = g_hi;
        g_lo = f_lo;
        g_hi = f_hi;
        f_lo = e_lo;
        f_hi = e_hi;

        let e2_lo = _u64_add_lo(d_lo, t1_lo);
        let e2_hi = _u64_add_hi(d_lo, d_hi, t1_lo, t1_hi, e2_lo);
        e_lo = e2_lo;
        e_hi = e2_hi;

        d_lo = c_lo;
        d_hi = c_hi;
        c_lo = b_lo;
        c_hi = b_hi;
        b_lo = a_lo;
        b_hi = a_hi;

        let a2_lo = _u64_add_lo(t1_lo, t2_lo);
        let a2_hi = _u64_add_hi(t1_lo, t1_hi, t2_lo, t2_hi, a2_lo);
        a_lo = a2_lo;
        a_hi = a2_hi;
    }

    let h0_lo = _u64_add_lo(h0_lo_0, a_lo);
    let h0_hi = _u64_add_hi(h0_lo_0, h0_hi_0, a_lo, a_hi, h0_lo);
    let h1_lo = _u64_add_lo(h1_lo_0, b_lo);
    let h1_hi = _u64_add_hi(h1_lo_0, h1_hi_0, b_lo, b_hi, h1_lo);
    let h2_lo = _u64_add_lo(h2_lo_0, c_lo);
    let h2_hi = _u64_add_hi(h2_lo_0, h2_hi_0, c_lo, c_hi, h2_lo);
    let h3_lo = _u64_add_lo(h3_lo_0, d_lo);
    let h3_hi = _u64_add_hi(h3_lo_0, h3_hi_0, d_lo, d_hi, h3_lo);
    let h4_lo = _u64_add_lo(h4_lo_0, e_lo);
    let h4_hi = _u64_add_hi(h4_lo_0, h4_hi_0, e_lo, e_hi, h4_lo);
    let h5_lo = _u64_add_lo(h5_lo_0, f_lo);
    let h5_hi = _u64_add_hi(h5_lo_0, h5_hi_0, f_lo, f_hi, h5_lo);
    let h6_lo = _u64_add_lo(h6_lo_0, g_lo);
    let h6_hi = _u64_add_hi(h6_lo_0, h6_hi_0, g_lo, g_hi, h6_lo);
    let h7_lo = _u64_add_lo(h7_lo_0, h_lo);
    let h7_hi = _u64_add_hi(h7_lo_0, h7_hi_0, h_lo, h_hi, h7_lo);

    let mut out = vec_u8_with_capacity(64);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h0_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h0_hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h1_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h1_hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h2_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h2_hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h3_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h3_hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h4_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h4_hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h5_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h5_hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h6_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h6_hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h7_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(h7_hi));
    vec_u8_into_bytes(out)
}

pub fn sha512(msg: BytesView) -> Bytes {
    let k_bytes = _sha512_k_bytes();
    let k = bytes_view(k_bytes);
    let mut state = _sha512_init_state();

    let n = view_len(msg);
    let blocks = n / 128;
    let mut off = 0;
    for _ in 0..blocks {
        let block = view_slice(msg, off, 128);
        state = _sha512_compress(state, block, k);
        off = off + 128;
    }

    let rem = n - off;
    let mut tail = vec_u8_with_capacity(256);
    if rem > 0 {
        tail = vec_u8_extend_bytes_range(tail, msg, off, rem);
    }
    tail = vec_u8_push(tail, 128);

    let cur_mod = vec_u8_len(tail) % 128;
    let pad_len = if cur_mod <= 112 {
        112 - cur_mod
    } else {
        112 + (128 - cur_mod)
    };
    for _ in 0..pad_len {
        tail = vec_u8_push(tail, 0);
    }

    for _ in 0..8 {
        tail = vec_u8_push(tail, 0);
    }

    let bit_lo = n << 3;
    let bit_hi = n >> 29;
    tail = vec_u8_push(tail, (bit_hi >> 24) & 255);
    tail = vec_u8_push(tail, (bit_hi >> 16) & 255);
    tail = vec_u8_push(tail, (bit_hi >> 8) & 255);
    tail = vec_u8_push(tail, bit_hi & 255);
    tail = vec_u8_push(tail, (bit_lo >> 24) & 255);
    tail = vec_u8_push(tail, (bit_lo >> 16) & 255);
    tail = vec_u8_push(tail, (bit_lo >> 8) & 255);
    tail = vec_u8_push(tail, bit_lo & 255);

    let tail_b = vec_u8_into_bytes(tail);
    let tail_v = bytes_view(tail_b);
    let tail_n = view_len(tail_v);
    let tail_blocks = tail_n / 128;
    let mut tail_off = 0;
    for _ in 0..tail_blocks {
        let block = view_slice(tail_v, tail_off, 128);
        state = _sha512_compress(state, block, k);
        tail_off = tail_off + 128;
    }

    let st2 = bytes_view(state);
    let mut out = vec_u8_with_capacity(64);
    for i in 0..8 {
        let lo = codec_read_u32_le(st2, i * 8);
        let hi = codec_read_u32_le(st2, (i * 8) + 4);
        out = _push_u32_be(out, hi);
        out = _push_u32_be(out, lo);
    }
    vec_u8_into_bytes(out)
}

fn _make_err_doc(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

pub fn hkdf_is_err(doc: BytesView) -> i32 {
    if view_len(doc) < 1 {
        return 1;
    }
    view_get_u8(doc, 0) == 0
}

pub fn hkdf_err_code(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 0 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn hkdf_get_bytes(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 1 || view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 1, n - 1))
}

pub fn hkdf_sha256_extract(salt: BytesView, ikm: BytesView) -> Bytes {
    if view_len(salt) == 0 {
        let zeros = bytes_alloc(32);
        return hmac_sha256(bytes_view(zeros), ikm);
    }
    hmac_sha256(salt, ikm)
}

pub fn hkdf_sha256_expand_v1(prk: BytesView, info: BytesView, out_len: i32) -> Bytes {
    if out_len < 0 {
        return _make_err_doc(1);
    }
    if out_len > 8160 {
        return _make_err_doc(1);
    }

    let n_blocks = (out_len + 31) / 32;
    if n_blocks > 255 {
        return _make_err_doc(1);
    }

    let mut out = vec_u8_with_capacity(out_len + 1);
    out = vec_u8_push(out, 1);

    let info_len = view_len(info);
    let mut t_prev = bytes_alloc(0);
    for i in 0..n_blocks {
        let counter = i + 1;
        let t_prev_len = bytes_len(t_prev);
        let mut msg = vec_u8_with_capacity(t_prev_len + info_len + 1);
        msg = vec_u8_extend_bytes(msg, bytes_view(t_prev));
        msg = vec_u8_extend_bytes(msg, info);
        msg = vec_u8_push(msg, counter);
        let msg_b = vec_u8_into_bytes(msg);
        let t = hmac_sha256(prk, bytes_view(msg_b));

        let written = vec_u8_len(out) - 1;
        let remaining = out_len - written;
        let take = if remaining < 32 { remaining } else { 32 };
        out = vec_u8_extend_bytes_range(out, bytes_view(t), 0, take);

        t_prev = t;
    }

    vec_u8_into_bytes(out)
}

pub fn hkdf_sha256_v1(salt: BytesView, ikm: BytesView, info: BytesView, out_len: i32) -> Bytes {
    let prk = hkdf_sha256_extract(salt, ikm);
    hkdf_sha256_expand_v1(bytes_view(prk), info, out_len)
}

pub fn eq_ct(a: BytesView, b: BytesView) -> bool {
    let n = view_len(a);
    if n != view_len(b) {
        return false;
    }
    let mut diff = 0;
    for i in 0..n {
        diff = diff | (view_get_u8(a, i) ^ view_get_u8(b, i));
    }
    diff == 0
}
