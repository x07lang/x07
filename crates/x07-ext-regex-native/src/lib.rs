#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use core::cmp::min;
use regex_automata::meta::Regex;
use regex_automata::util::syntax;
use regex_automata::{Anchored, Input, MatchKind};
use regex_syntax::ast;
use std::sync::{Mutex, OnceLock};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_bytes {
    pub ptr: *mut u8,
    pub len: u32,
}

extern "C" {
    fn ev_bytes_alloc(len: u32) -> ev_bytes;
    fn ev_trap(code: i32) -> !;
}

const EV_TRAP_REGEX_INTERNAL: i32 = 9400;

// ext.regex error codes (must match ext.regex module constants).
const CODE_PARSE_UNBALANCED_PAREN: u32 = 1;
const CODE_PARSE_UNCLOSED_CLASS: u32 = 2;
const CODE_PARSE_INVALID_CLASS: u32 = 3;
const CODE_PARSE_INVALID_ESCAPE: u32 = 4;
const CODE_PARSE_NOTHING_TO_REPEAT: u32 = 5;
const CODE_PARSE_INVALID_REPEAT: u32 = 6;
const CODE_PARSE_REPEAT_RANGE: u32 = 7;
const CODE_COMPILE_STACK_OVERFLOW: u32 = 8;
const CODE_COMPILE_TOO_MANY_STATES: u32 = 9;
const CODE_EXEC_INVALID_COMPILED: u32 = 10;
const CODE_PARSE_TOO_MANY_CAPTURES: u32 = 11;

const MAX_CAPS: usize = 32;

const COMPILED_MAGIC: &[u8; 4] = b"X7RG";
const COMPILED_VERSION: u8 = 1;
const COMPILED_LEN: u32 = 18;

#[derive(Clone)]
struct Compiled {
    re_leftmost: Regex,
    re_all: Regex,
    cap_count: u32,
}

struct RegexTable {
    entries: Vec<Option<Compiled>>,
}

impl RegexTable {
    fn new() -> Self {
        Self {
            // Handle 0 reserved as invalid.
            entries: vec![None],
        }
    }

    fn insert(&mut self, compiled: Compiled) -> Option<u32> {
        // Deterministic handle assignment: first free slot, else append.
        for (i, slot) in self.entries.iter_mut().enumerate().skip(1) {
            if slot.is_none() {
                *slot = Some(compiled);
                return Some(i as u32);
            }
        }
        let h = self.entries.len() as u32;
        self.entries.push(Some(compiled));
        Some(h)
    }

    fn get(&self, h: u32) -> Option<&Compiled> {
        self.entries.get(h as usize)?.as_ref()
    }
}

static TABLE: OnceLock<Mutex<RegexTable>> = OnceLock::new();

fn table() -> &'static Mutex<RegexTable> {
    TABLE.get_or_init(|| Mutex::new(RegexTable::new()))
}

#[inline]
unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    core::slice::from_raw_parts(b.ptr as *const u8, b.len as usize)
}

#[inline]
unsafe fn bytes_as_mut_slice<'a>(b: ev_bytes) -> &'a mut [u8] {
    core::slice::from_raw_parts_mut(b.ptr, b.len as usize)
}

#[inline]
unsafe fn alloc_bytes(len: u32) -> ev_bytes {
    let out = ev_bytes_alloc(len);
    if out.len != len {
        ev_trap(EV_TRAP_REGEX_INTERNAL);
    }
    out
}

#[inline]
fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

#[inline]
fn write_u32_le(dst: &mut [u8], x: u32) {
    dst[0] = (x & 0xFF) as u8;
    dst[1] = ((x >> 8) & 0xFF) as u8;
    dst[2] = ((x >> 16) & 0xFF) as u8;
    dst[3] = ((x >> 24) & 0xFF) as u8;
}

#[inline]
unsafe fn make_err(code: u32, pos: u32) -> ev_bytes {
    let out = alloc_bytes(9);
    let b = bytes_as_mut_slice(out);
    b[0] = 0;
    write_u32_le(&mut b[1..5], code);
    write_u32_le(&mut b[5..9], pos);
    out
}

#[inline]
unsafe fn make_match_doc(is_match: bool, start: u32, end: u32) -> ev_bytes {
    let out = alloc_bytes(10);
    let b = bytes_as_mut_slice(out);
    b[0] = 1;
    b[1] = if is_match { 1 } else { 0 };
    write_u32_le(&mut b[2..6], start);
    write_u32_le(&mut b[6..10], end);
    out
}

#[inline]
unsafe fn make_caps_doc(
    is_match: bool,
    start: u32,
    end: u32,
    cap_count: u32,
    caps: Option<&regex_automata::util::captures::Captures>,
) -> ev_bytes {
    let total = 14u32.saturating_add(cap_count.saturating_mul(8).min(u32::MAX.saturating_sub(14)));
    let out = alloc_bytes(total);
    let b = bytes_as_mut_slice(out);
    b[0] = 1;
    b[1] = if is_match { 1 } else { 0 };
    write_u32_le(&mut b[2..6], start);
    write_u32_le(&mut b[6..10], end);
    write_u32_le(&mut b[10..14], cap_count);

    let mut off = 14usize;
    for i in 1..=cap_count {
        let (cs, ce) = if is_match {
            let span = caps.and_then(|c| c.get_group(i as usize));
            match span {
                Some(sp) => (sp.start as u32, sp.end as u32),
                None => (0xFFFF_FFFF, 0xFFFF_FFFF),
            }
        } else {
            (0xFFFF_FFFF, 0xFFFF_FFFF)
        };
        write_u32_le(&mut b[off..off + 4], cs);
        write_u32_le(&mut b[off + 4..off + 8], ce);
        off += 8;
    }
    out
}

#[inline]
unsafe fn copy_bytes(src: ev_bytes) -> ev_bytes {
    let out = alloc_bytes(src.len);
    let src_s = bytes_as_slice(src);
    let dst_s = bytes_as_mut_slice(out);
    dst_s.copy_from_slice(src_s);
    out
}

fn opts_bit(opts: u32, bit: u32) -> bool {
    (opts & bit) != 0
}

fn syntax_config(opts: u32) -> syntax::Config {
    syntax::Config::new()
        .unicode(false)
        .utf8(false)
        .case_insensitive(opts_bit(opts, 1))
        .multi_line(opts_bit(opts, 2))
        .dot_matches_new_line(opts_bit(opts, 4))
}

fn map_syntax_error(err: &regex_syntax::Error) -> (u32, u32) {
    match err {
        regex_syntax::Error::Parse(e) => map_ast_error(e),
        regex_syntax::Error::Translate(e) => {
            let pos = min(e.span().start.offset, u32::MAX as usize) as u32;
            (CODE_PARSE_INVALID_ESCAPE, pos)
        }
        _ => (CODE_PARSE_INVALID_ESCAPE, 0),
    }
}

fn map_ast_error(err: &ast::Error) -> (u32, u32) {
    let kind = err.kind();
    let pat_len = min(err.pattern().len(), u32::MAX as usize) as u32;
    let span_start = min(err.span().start.offset, u32::MAX as usize) as u32;

    let code = match kind {
        ast::ErrorKind::GroupUnclosed => CODE_PARSE_UNBALANCED_PAREN,
        ast::ErrorKind::ClassUnclosed => CODE_PARSE_UNCLOSED_CLASS,
        ast::ErrorKind::ClassEscapeInvalid
        | ast::ErrorKind::ClassRangeInvalid
        | ast::ErrorKind::ClassRangeLiteral => CODE_PARSE_INVALID_CLASS,
        ast::ErrorKind::EscapeUnrecognized
        | ast::ErrorKind::EscapeUnexpectedEof
        | ast::ErrorKind::EscapeHexEmpty
        | ast::ErrorKind::EscapeHexInvalid
        | ast::ErrorKind::EscapeHexInvalidDigit => CODE_PARSE_INVALID_ESCAPE,
        ast::ErrorKind::RepetitionMissing => CODE_PARSE_NOTHING_TO_REPEAT,
        ast::ErrorKind::RepetitionCountInvalid => CODE_PARSE_REPEAT_RANGE,
        ast::ErrorKind::RepetitionCountDecimalEmpty
        | ast::ErrorKind::RepetitionCountUnclosed
        | ast::ErrorKind::DecimalInvalid => CODE_PARSE_INVALID_REPEAT,
        ast::ErrorKind::CaptureLimitExceeded => CODE_PARSE_TOO_MANY_CAPTURES,
        ast::ErrorKind::NestLimitExceeded(_) => CODE_COMPILE_STACK_OVERFLOW,
        _ => CODE_PARSE_INVALID_ESCAPE,
    };

    let pos = match kind {
        // Keep legacy behavior: report position where a closing delimiter was expected.
        ast::ErrorKind::GroupUnclosed | ast::ErrorKind::ClassUnclosed => pat_len,
        _ => span_start,
    };
    (code, pos)
}

fn build_regex_pair(
    pattern: &str,
    opts: u32,
) -> Result<(Regex, Regex), Box<regex_automata::meta::BuildError>> {
    let mut b_left = Regex::builder();
    b_left.configure(Regex::config().match_kind(MatchKind::LeftmostFirst));
    b_left.syntax(syntax_config(opts));
    let left = b_left.build(pattern).map_err(Box::new)?;

    let mut b_all = Regex::builder();
    b_all.configure(Regex::config().match_kind(MatchKind::All));
    b_all.syntax(syntax_config(opts));
    let all = b_all.build(pattern).map_err(Box::new)?;

    Ok((left, all))
}

fn limit_from_i32(v: i32) -> usize {
    if v > 0 {
        v as usize
    } else {
        usize::MAX
    }
}

fn clamp_start(start: i32, hay_len: usize) -> usize {
    if start <= 0 {
        0
    } else {
        let s = start as usize;
        if s > hay_len {
            hay_len.saturating_add(1)
        } else {
            s
        }
    }
}

fn find_leftmost_longest_at(
    c: &Compiled,
    hay: &[u8],
    start: usize,
    cache_left: &mut regex_automata::meta::Cache,
    cache_all: &mut regex_automata::meta::Cache,
) -> Option<(usize, usize)> {
    let hay_len = hay.len();
    if start > hay_len {
        return None;
    }

    let input = Input::new(hay).span(start..hay_len).anchored(Anchored::No);
    let m = c.re_leftmost.search_with(cache_left, &input)?;
    let s = m.start();

    let input = Input::new(hay).span(s..hay_len).anchored(Anchored::Yes);
    let m_long = c.re_all.search_with(cache_all, &input)?;
    Some((s, m_long.end()))
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_regex_compile_opts_v1(pat: ev_bytes, opts: i32) -> ev_bytes {
    let pat_bytes = bytes_as_slice(pat);
    let Ok(pat_str) = core::str::from_utf8(pat_bytes) else {
        return make_err(CODE_PARSE_INVALID_ESCAPE, 0);
    };
    let opts_u32 = opts as u32;

    let (re_leftmost, re_all) = match build_regex_pair(pat_str, opts_u32) {
        Ok(pair) => pair,
        Err(err) => {
            if let Some(se) = err.syntax_error() {
                let (code, pos) = map_syntax_error(se);
                return make_err(code, pos);
            }
            return make_err(CODE_COMPILE_TOO_MANY_STATES, 0);
        }
    };

    let cap_count = re_leftmost.captures_len().saturating_sub(1);
    if cap_count > MAX_CAPS {
        return make_err(CODE_PARSE_TOO_MANY_CAPTURES, 0);
    }
    let cap_count_u32 = cap_count as u32;

    let compiled = Compiled {
        re_leftmost,
        re_all,
        cap_count: cap_count_u32,
    };

    let mut guard = table().lock().unwrap();
    let Some(handle) = guard.insert(compiled) else {
        return make_err(CODE_COMPILE_TOO_MANY_STATES, 0);
    };

    let out = alloc_bytes(COMPILED_LEN);
    let b = bytes_as_mut_slice(out);
    b[0] = 1;
    b[1..5].copy_from_slice(COMPILED_MAGIC);
    b[5] = COMPILED_VERSION;
    write_u32_le(&mut b[6..10], handle);
    write_u32_le(&mut b[10..14], opts_u32);
    write_u32_le(&mut b[14..18], fnv1a32(pat_bytes));
    out
}

fn parse_compiled(doc: ev_bytes) -> Result<u32, u32> {
    unsafe {
        let b = bytes_as_slice(doc);
        if b.len() < COMPILED_LEN as usize {
            return Err(CODE_EXEC_INVALID_COMPILED);
        }
        if b[0] == 0 {
            return Err(CODE_EXEC_INVALID_COMPILED);
        }
        if &b[1..5] != COMPILED_MAGIC {
            return Err(CODE_EXEC_INVALID_COMPILED);
        }
        if b[5] != COMPILED_VERSION {
            return Err(CODE_EXEC_INVALID_COMPILED);
        }
        let h = u32::from_le_bytes([b[6], b[7], b[8], b[9]]);
        Ok(h)
    }
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_regex_exec_from_v1(
    compiled: ev_bytes,
    text: ev_bytes,
    start_i32: i32,
) -> ev_bytes {
    let compiled_bytes = bytes_as_slice(compiled);
    if compiled_bytes.first().copied() == Some(0) {
        return copy_bytes(compiled);
    }

    let h = match parse_compiled(compiled) {
        Ok(h) => h,
        Err(code) => return make_err(code, 0),
    };
    let guard = table().lock().unwrap();
    let Some(c) = guard.get(h).cloned() else {
        return make_err(CODE_EXEC_INVALID_COMPILED, 0);
    };
    drop(guard);

    let hay = bytes_as_slice(text);
    let hay_len = hay.len();
    let start = clamp_start(start_i32, hay_len);

    let mut cache_left = c.re_leftmost.create_cache();
    let mut cache_all = c.re_all.create_cache();

    let Some((s, e)) = find_leftmost_longest_at(&c, hay, start, &mut cache_left, &mut cache_all)
    else {
        return make_match_doc(false, 0, 0);
    };

    make_match_doc(true, s as u32, e as u32)
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_regex_exec_caps_from_v1(
    compiled: ev_bytes,
    text: ev_bytes,
    start_i32: i32,
) -> ev_bytes {
    let compiled_bytes = bytes_as_slice(compiled);
    if compiled_bytes.first().copied() == Some(0) {
        return copy_bytes(compiled);
    }

    let h = match parse_compiled(compiled) {
        Ok(h) => h,
        Err(code) => return make_err(code, 0),
    };
    let guard = table().lock().unwrap();
    let Some(c) = guard.get(h).cloned() else {
        return make_err(CODE_EXEC_INVALID_COMPILED, 0);
    };
    drop(guard);

    let hay = bytes_as_slice(text);
    let hay_len = hay.len();
    let start = clamp_start(start_i32, hay_len);

    let mut cache_left = c.re_leftmost.create_cache();
    let mut cache_all = c.re_all.create_cache();

    let Some((s, _e)) = find_leftmost_longest_at(&c, hay, start, &mut cache_left, &mut cache_all)
    else {
        return make_caps_doc(false, 0, 0, c.cap_count, None);
    };

    let input = Input::new(hay).span(s..hay_len).anchored(Anchored::Yes);
    let mut caps = c.re_all.create_captures();
    c.re_all
        .search_captures_with(&mut cache_all, &input, &mut caps);
    let m = caps.get_match();
    let Some(m) = m else {
        return make_err(CODE_EXEC_INVALID_COMPILED, 0);
    };
    make_caps_doc(
        true,
        m.start() as u32,
        m.end() as u32,
        c.cap_count,
        Some(&caps),
    )
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_regex_find_all_x7sl_v1(
    compiled: ev_bytes,
    text: ev_bytes,
    max_matches: i32,
) -> ev_bytes {
    let compiled_bytes = bytes_as_slice(compiled);
    if compiled_bytes.first().copied() == Some(0) {
        return copy_bytes(compiled);
    }

    let h = match parse_compiled(compiled) {
        Ok(h) => h,
        Err(code) => return make_err(code, 0),
    };
    let guard = table().lock().unwrap();
    let Some(c) = guard.get(h).cloned() else {
        return make_err(CODE_EXEC_INVALID_COMPILED, 0);
    };
    drop(guard);

    let hay = bytes_as_slice(text);
    let hay_len = hay.len();
    let limit = limit_from_i32(max_matches);

    let mut cache_left = c.re_leftmost.create_cache();
    let mut cache_all = c.re_all.create_cache();

    let mut rows: Vec<(u32, u32)> = Vec::new();
    let mut pos: usize = 0;
    while rows.len() < limit && pos <= hay_len {
        let Some((s, e)) = find_leftmost_longest_at(&c, hay, pos, &mut cache_left, &mut cache_all)
        else {
            break;
        };
        let su = s as u32;
        let eu = e as u32;
        rows.push((su, eu.saturating_sub(su)));
        if e > s {
            pos = e;
        } else {
            pos = s.saturating_add(1);
        }
    }

    let count = min(rows.len(), u32::MAX as usize) as u32;
    let out_len = 12u32.saturating_add(count.saturating_mul(8));
    let out = alloc_bytes(out_len);
    let b = bytes_as_mut_slice(out);
    b[0..4].copy_from_slice(b"X7SL");
    write_u32_le(&mut b[4..8], 1);
    write_u32_le(&mut b[8..12], count);
    let mut off = 12usize;
    for (s, l) in rows.into_iter().take(count as usize) {
        write_u32_le(&mut b[off..off + 4], s);
        write_u32_le(&mut b[off + 4..off + 8], l);
        off += 8;
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_regex_split_v1(
    compiled: ev_bytes,
    text: ev_bytes,
    max_parts: i32,
) -> ev_bytes {
    let compiled_bytes = bytes_as_slice(compiled);
    if compiled_bytes.first().copied() == Some(0) {
        return copy_bytes(compiled);
    }

    let h = match parse_compiled(compiled) {
        Ok(h) => h,
        Err(code) => return make_err(code, 0),
    };
    let guard = table().lock().unwrap();
    let Some(c) = guard.get(h).cloned() else {
        return make_err(CODE_EXEC_INVALID_COMPILED, 0);
    };
    drop(guard);

    let hay = bytes_as_slice(text);
    let hay_len = hay.len();
    let limit = limit_from_i32(max_parts);

    let mut cache_left = c.re_leftmost.create_cache();
    let mut cache_all = c.re_all.create_cache();

    let mut rows: Vec<(u32, u32)> = Vec::new();
    let mut last_end: usize = 0;
    let mut pos: usize = 0;
    while rows.len().saturating_add(1) < limit && pos <= hay_len {
        let Some((s, e)) = find_leftmost_longest_at(&c, hay, pos, &mut cache_left, &mut cache_all)
        else {
            break;
        };
        rows.push((last_end as u32, (s.saturating_sub(last_end)) as u32));
        last_end = e;
        if e > s {
            pos = e;
        } else {
            pos = s.saturating_add(1);
        }
    }
    rows.push((last_end as u32, (hay_len.saturating_sub(last_end)) as u32));

    let count = min(rows.len(), u32::MAX as usize) as u32;
    let out_len = 12u32.saturating_add(count.saturating_mul(8));
    let out = alloc_bytes(out_len);
    let b = bytes_as_mut_slice(out);
    b[0..4].copy_from_slice(b"X7SL");
    write_u32_le(&mut b[4..8], 1);
    write_u32_le(&mut b[8..12], count);
    let mut off = 12usize;
    for (s, l) in rows.into_iter().take(count as usize) {
        write_u32_le(&mut b[off..off + 4], s);
        write_u32_le(&mut b[off + 4..off + 8], l);
        off += 8;
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_regex_replace_all_v1(
    compiled: ev_bytes,
    text: ev_bytes,
    repl: ev_bytes,
    cap_limit: i32,
) -> ev_bytes {
    let compiled_bytes = bytes_as_slice(compiled);
    if compiled_bytes.first().copied() == Some(0) {
        return copy_bytes(compiled);
    }

    let h = match parse_compiled(compiled) {
        Ok(h) => h,
        Err(code) => return make_err(code, 0),
    };
    let guard = table().lock().unwrap();
    let Some(c) = guard.get(h).cloned() else {
        return make_err(CODE_EXEC_INVALID_COMPILED, 0);
    };
    drop(guard);

    let hay = bytes_as_slice(text);
    let hay_len = hay.len();
    let repl_s = bytes_as_slice(repl);
    let limit = limit_from_i32(cap_limit);

    let mut cache_left = c.re_leftmost.create_cache();
    let mut cache_all = c.re_all.create_cache();

    let mut out: Vec<u8> = Vec::with_capacity(hay_len.saturating_add(repl_s.len()));
    let mut last_end: usize = 0;
    let mut pos: usize = 0;

    let mut replaced: usize = 0;
    while replaced < limit && pos <= hay_len {
        let Some((s, e)) = find_leftmost_longest_at(&c, hay, pos, &mut cache_left, &mut cache_all)
        else {
            break;
        };
        out.extend_from_slice(&hay[last_end..s]);
        out.extend_from_slice(repl_s);
        replaced = replaced.saturating_add(1);
        last_end = e;
        if e > s {
            pos = e;
        } else {
            pos = s.saturating_add(1);
        }
    }
    out.extend_from_slice(&hay[last_end..hay_len]);

    if out.len() > u32::MAX as usize {
        return make_err(CODE_COMPILE_TOO_MANY_STATES, 0);
    }

    let out_len = out.len() as u32;
    let out_b = alloc_bytes(out_len);
    let dst = bytes_as_mut_slice(out_b);
    dst.copy_from_slice(&out);
    out_b
}
