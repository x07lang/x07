use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use x07_host_runner::{compile_program_with_options, run_artifact_file, RunnerConfig};
use x07_worlds::WorldId;
use x07c::compile;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn config() -> RunnerConfig {
    RunnerConfig {
        world: WorldId::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_rr_index: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 20,
        debug_borrow_checks: false,
    }
}

fn entry(imports: &[&str], solve: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": "x07.x07ast@0.3.0",
        "kind": "entry",
        "module_id": "main",
        "imports": imports,
        "decls": [],
        "solve": solve,
    }))
    .expect("encode x07AST entry JSON")
}

fn compile_exe(program: &[u8], module_roots: Vec<PathBuf>) -> PathBuf {
    let cfg = config();
    let compile_options = compile::CompileOptions {
        world: Default::default(),
        enable_fs: false,
        enable_rr: false,
        enable_kv: false,
        module_roots,
        emit_main: true,
        freestanding: false,
        allow_unsafe: None,
        allow_ffi: None,
    };
    let compile = compile_program_with_options(program, &cfg, None, &compile_options, &[])
        .expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    compile.compiled_exe.expect("compiled exe")
}

fn run_exe(exe: &Path, input: &[u8]) -> Vec<u8> {
    let cfg = config();
    let res = run_artifact_file(&cfg, exe, input).expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    res.solve_output
}

fn write_module(out_root: &Path, module_id: &str, src: &str) {
    let mut rel = PathBuf::new();
    for seg in module_id.split('.') {
        rel.push(seg);
    }
    rel.set_extension("x07.json");
    let path = out_root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create module dir");
    }
    std::fs::write(&path, src.as_bytes()).expect("write module");
}

fn import_rust_module(module_id: &str, rel_path: &str) -> String {
    let root = repo_root();
    let rel = PathBuf::from(rel_path);
    let abs = root.join(&rel);
    let src = std::fs::read_to_string(&abs).expect("read rust source");
    let m = x07import_core::rust::import_rust_file(module_id, &rel, &src).expect("import rust");
    x07import_core::x07_emit::emit_module(&m).expect("emit module")
}

#[test]
fn difftest_ascii_normalize_lines_and_tokenize() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.text.ascii";
    let src = import_rust_module(
        module_id,
        "labs/x07import/fixtures/import_sources/rust/std_text_ascii@0.1.1/ascii.rs",
    );
    write_module(&module_root, module_id, &src);

    let normalize_lines = format!("{module_id}.normalize_lines");
    let tokenize_words_lower = format!("{module_id}.tokenize_words_lower");

    let program_norm = entry(&[module_id], json!([normalize_lines, "input"]));
    let exe_norm = compile_exe(program_norm.as_slice(), vec![module_root.clone()]);
    let program_tok = entry(&[module_id], json!([tokenize_words_lower, "input"]));
    let exe_tok = compile_exe(program_tok.as_slice(), vec![module_root.clone()]);

    for input in [
        b"  a \r\n\tb\t\n\n c  \r\n".as_slice(),
        b" \t\r\n \n".as_slice(),
        b"one\nTWO\r\nthree".as_slice(),
    ] {
        rt::reset();
        let b = rt::bytes_from_slice(input);
        let v = rt::bytes_view(b);
        let expected_norm = rt::bytes_to_vec(ascii_ref::normalize_lines(v));
        let got_norm = run_exe(&exe_norm, input);
        assert_eq!(got_norm, expected_norm);

        rt::reset();
        let b = rt::bytes_from_slice(input);
        let v = rt::bytes_view(b);
        let expected_tok = rt::bytes_to_vec(ascii_ref::tokenize_words_lower(v));
        let got_tok = run_exe(&exe_tok, input);
        assert_eq!(got_tok, expected_tok);
    }
}

#[test]
fn difftest_ascii_views() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.text.ascii";
    let src = import_rust_module(
        module_id,
        "labs/x07import/fixtures/import_sources/rust/std_text_ascii@0.1.1/ascii.rs",
    );
    write_module(&module_root, module_id, &src);

    let first_line_view = format!("{module_id}.first_line_view");
    let last_line_view = format!("{module_id}.last_line_view");
    let kth_line_view = format!("{module_id}.kth_line_view");

    let program_first = entry(
        &[module_id],
        json!(["view.to_bytes", [first_line_view, "input"]]),
    );
    let exe_first = compile_exe(program_first.as_slice(), vec![module_root.clone()]);
    let program_last = entry(
        &[module_id],
        json!(["view.to_bytes", [last_line_view, "input"]]),
    );
    let exe_last = compile_exe(program_last.as_slice(), vec![module_root.clone()]);
    let program_k1 = entry(
        &[module_id],
        json!(["view.to_bytes", [kth_line_view, "input", 1]]),
    );
    let exe_k1 = compile_exe(program_k1.as_slice(), vec![module_root.clone()]);

    let input = b"first\nsecond\nthird\n";
    rt::reset();
    let b = rt::bytes_from_slice(input);
    let v = rt::bytes_view(b);
    let expected_first = rt::bytes_to_vec(rt::view_to_bytes(ascii_ref::first_line_view(v)));
    assert_eq!(run_exe(&exe_first, input), expected_first);

    rt::reset();
    let b = rt::bytes_from_slice(input);
    let v = rt::bytes_view(b);
    let expected_last = rt::bytes_to_vec(rt::view_to_bytes(ascii_ref::last_line_view(v)));
    assert_eq!(run_exe(&exe_last, input), expected_last);

    rt::reset();
    let b = rt::bytes_from_slice(input);
    let v = rt::bytes_view(b);
    let expected_k1 = rt::bytes_to_vec(rt::view_to_bytes(ascii_ref::kth_line_view(v, 1)));
    assert_eq!(run_exe(&exe_k1, input), expected_k1);
}

#[test]
fn difftest_ascii_split_helpers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.text.ascii";
    let src = import_rust_module(
        module_id,
        "labs/x07import/fixtures/import_sources/rust/std_text_ascii@0.1.1/ascii.rs",
    );
    write_module(&module_root, module_id, &src);

    let split_u8 = format!("{module_id}.split_u8");
    let split_lines_view = format!("{module_id}.split_lines_view");

    let program_split = entry(&[module_id], json!([split_u8, "input", 44]));
    let exe_split = compile_exe(program_split.as_slice(), vec![module_root.clone()]);
    let program_lines = entry(&[module_id], json!([split_lines_view, "input"]));
    let exe_lines = compile_exe(program_lines.as_slice(), vec![module_root.clone()]);

    for input in [
        b"".as_slice(),
        b"," as &[u8],
        b",,a,,b,".as_slice(),
        b"hello,world".as_slice(),
    ] {
        rt::reset();
        let b = rt::bytes_from_slice(input);
        let v = rt::bytes_view(b);
        let expected = rt::bytes_to_vec(ascii_ref::split_u8(v, 44));
        assert_eq!(run_exe(&exe_split, input), expected);
    }

    for input in [
        b"".as_slice(),
        b"one".as_slice(),
        b"one\n".as_slice(),
        b"one\r\ntwo\nthree\r\n".as_slice(),
    ] {
        rt::reset();
        let b = rt::bytes_from_slice(input);
        let v = rt::bytes_view(b);
        let expected = rt::bytes_to_vec(ascii_ref::split_lines_view(v));
        assert_eq!(run_exe(&exe_lines, input), expected);
    }
}

#[test]
fn difftest_ascii_to_upper_u8() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.text.ascii";
    let src = import_rust_module(
        module_id,
        "labs/x07import/fixtures/import_sources/rust/std_text_ascii@0.1.1/ascii.rs",
    );
    write_module(&module_root, module_id, &src);

    let to_upper_u8 = format!("{module_id}.to_upper_u8");
    let program = entry(
        &[module_id],
        json!(["bytes1", [to_upper_u8, ["bytes.get_u8", "input", 0]]]),
    );
    let exe = compile_exe(program.as_slice(), vec![module_root.clone()]);

    for input in [
        b"a".as_slice(),
        b"z".as_slice(),
        b"A".as_slice(),
        b"Z".as_slice(),
        b"0".as_slice(),
    ] {
        let c = input[0] as i32;
        let expected = vec![ascii_ref::to_upper_u8(c) as u8];
        let got = run_exe(&exe, input);
        assert_eq!(got, expected);
    }
}

#[test]
fn difftest_utf8_validate_or_empty() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.text.utf8";
    let src = import_rust_module(
        module_id,
        "labs/x07import/fixtures/import_sources/rust/std_text_utf8@0.1.1/utf8.rs",
    );
    write_module(&module_root, module_id, &src);

    let validate_or_empty = format!("{module_id}.validate_or_empty");
    let program = entry(&[module_id], json!([validate_or_empty, "input"]));
    let exe = compile_exe(program.as_slice(), vec![module_root.clone()]);

    for input in [
        b"hello".as_slice(),
        "€".as_bytes(),
        "Привет".as_bytes(),
        b"\xC3(".as_slice(),
        b"\xE2\x82".as_slice(),
    ] {
        rt::reset();
        let b = rt::bytes_from_slice(input);
        let v = rt::bytes_view(b);
        let expected = rt::bytes_to_vec(utf8_ref::validate_or_empty(v));
        let got = run_exe(&exe, input);
        assert_eq!(got, expected);
    }
}

#[test]
fn difftest_utf8_count_codepoints_or_neg1() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.text.utf8";
    let src = import_rust_module(
        module_id,
        "labs/x07import/fixtures/import_sources/rust/std_text_utf8@0.1.1/utf8.rs",
    );
    write_module(&module_root, module_id, &src);

    let count_codepoints_or_neg1 = format!("{module_id}.count_codepoints_or_neg1");
    let program = entry(
        &[module_id],
        json!(["codec.write_u32_le", [count_codepoints_or_neg1, "input"]]),
    );
    let exe = compile_exe(program.as_slice(), vec![module_root.clone()]);

    for input in [
        b"hello".as_slice(),
        "€".as_bytes(),
        "Привет".as_bytes(),
        b"\xFF".as_slice(),
        b"\xE2\x82".as_slice(),
    ] {
        rt::reset();
        let b = rt::bytes_from_slice(input);
        let v = rt::bytes_view(b);
        let n = utf8_ref::count_codepoints_or_neg1(v);
        let expected = rt::bytes_to_vec(rt::codec_write_u32_le(n));
        let got = run_exe(&exe, input);
        assert_eq!(got, expected);
    }
}

#[test]
fn difftest_regex_lite_find_literal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.regex-lite";
    let src = import_rust_module(
        module_id,
        "labs/x07import/fixtures/import_sources/rust/std_regex_lite@0.1.1/regex_lite.rs",
    );
    write_module(&module_root, module_id, &src);

    let find_literal = format!("{module_id}.find_literal");
    let program = entry(
        &[module_id],
        json!([
            "codec.write_u32_le",
            [find_literal, "input", ["bytes.lit", "needle"]]
        ]),
    );
    let exe = compile_exe(program.as_slice(), vec![module_root.clone()]);

    let input = b"abc needle xyz";
    rt::reset();
    let hay = rt::bytes_from_slice(input);
    let needle = rt::bytes_from_slice(b"needle");
    let hay_v = rt::bytes_view(hay);
    let needle_v = rt::bytes_view(needle);
    let idx = regex_ref::find_literal(hay_v, needle_v);
    let expected = rt::bytes_to_vec(rt::codec_write_u32_le(idx));

    let got = run_exe(&exe, input);
    assert_eq!(got, expected);
}

#[test]
fn difftest_regex_lite_is_match_literal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.regex-lite";
    let src = import_rust_module(
        module_id,
        "labs/x07import/fixtures/import_sources/rust/std_regex_lite@0.1.1/regex_lite.rs",
    );
    write_module(&module_root, module_id, &src);

    let is_match_literal = format!("{module_id}.is_match_literal");
    let program = entry(
        &[module_id],
        json!([
            "codec.write_u32_le",
            [is_match_literal, "input", ["bytes.lit", "needle"]]
        ]),
    );
    let exe = compile_exe(program.as_slice(), vec![module_root.clone()]);

    let input = b"abc needle xyz";
    rt::reset();
    let hay = rt::bytes_from_slice(input);
    let needle = rt::bytes_from_slice(b"needle");
    let hay_v = rt::bytes_view(hay);
    let needle_v = rt::bytes_view(needle);
    let ok = regex_ref::is_match_literal(hay_v, needle_v);
    let expected = rt::bytes_to_vec(rt::codec_write_u32_le(if ok { 1 } else { 0 }));

    let got = run_exe(&exe, input);
    assert_eq!(got, expected);
}

#[test]
fn difftest_regex_lite_count_matches_u32le() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.regex-lite";
    let src = import_rust_module(
        module_id,
        "labs/x07import/fixtures/import_sources/rust/std_regex_lite@0.1.1/regex_lite.rs",
    );
    write_module(&module_root, module_id, &src);

    let count_matches_u32le = format!("{module_id}.count_matches_u32le");
    let program = entry(&[module_id], json!([count_matches_u32le, "input"]));
    let exe = compile_exe(program.as_slice(), vec![module_root.clone()]);

    for input in [
        b"ab\0xxabyyab".as_slice(),
        b"a.\0a1a2a3".as_slice(),
        b"a*b\0aaababb".as_slice(),
    ] {
        rt::reset();
        let b = rt::bytes_from_slice(input);
        let v = rt::bytes_view(b);
        let expected = rt::bytes_to_vec(regex_ref::count_matches_u32le(v));
        let got = run_exe(&exe, input);
        assert_eq!(got, expected);
    }
}

#[test]
fn difftest_codec_and_fmt_wrappers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let codec_id = "x07import_test.codec";
    let codec_src = import_rust_module(
        codec_id,
        "labs/x07import/fixtures/import_sources/rust/std_codec@0.1.1/codec.rs",
    );
    write_module(&module_root, codec_id, &codec_src);

    let fmt_id = "x07import_test.fmt";
    let fmt_src = import_rust_module(
        fmt_id,
        "labs/x07import/fixtures/import_sources/rust/std_fmt@0.1.1/fmt.rs",
    );
    write_module(&module_root, fmt_id, &fmt_src);

    rt::reset();
    let expected_codec = rt::bytes_to_vec(codec_ref::write_u32_le(123456789));
    let write_u32_le = format!("{codec_id}.write_u32_le");
    let program_codec = entry(&[codec_id], json!([write_u32_le, 123456789]));
    let exe_codec = compile_exe(program_codec.as_slice(), vec![module_root.clone()]);
    assert_eq!(run_exe(&exe_codec, b""), expected_codec);

    rt::reset();
    let expected_fmt = rt::bytes_to_vec(fmt_ref::s32_to_dec(-5));
    let s32_to_dec = format!("{fmt_id}.s32_to_dec");
    let program_fmt = entry(&[fmt_id], json!([s32_to_dec, -5]));
    let exe_fmt = compile_exe(program_fmt.as_slice(), vec![module_root.clone()]);
    assert_eq!(run_exe(&exe_fmt, b""), expected_fmt);
}

#[test]
fn difftest_codec_read_u32_le() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let codec_id = "x07import_test.codec";
    let codec_src = import_rust_module(
        codec_id,
        "labs/x07import/fixtures/import_sources/rust/std_codec@0.1.1/codec.rs",
    );
    write_module(&module_root, codec_id, &codec_src);

    let read_u32_le = format!("{codec_id}.read_u32_le");
    let program = entry(
        &[codec_id],
        json!(["codec.write_u32_le", [read_u32_le, "input", 0]]),
    );
    let exe = compile_exe(program.as_slice(), vec![module_root.clone()]);

    let input = (123456789u32).to_le_bytes();
    rt::reset();
    let b = rt::bytes_from_slice(&input);
    let v = rt::bytes_view(b);
    let x = codec_ref::read_u32_le(v, 0);
    let expected = rt::bytes_to_vec(rt::codec_write_u32_le(x));
    assert_eq!(run_exe(&exe, &input), expected);
}

#[test]
fn difftest_fmt_u32_to_dec() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let fmt_id = "x07import_test.fmt";
    let fmt_src = import_rust_module(
        fmt_id,
        "labs/x07import/fixtures/import_sources/rust/std_fmt@0.1.1/fmt.rs",
    );
    write_module(&module_root, fmt_id, &fmt_src);

    let u32_to_dec = format!("{fmt_id}.u32_to_dec");
    let program = entry(&[fmt_id], json!([u32_to_dec, 42]));
    let exe = compile_exe(program.as_slice(), vec![module_root.clone()]);

    rt::reset();
    let expected = rt::bytes_to_vec(fmt_ref::u32_to_dec(42));
    assert_eq!(run_exe(&exe, b""), expected);
}

mod rt {
    use std::cell::RefCell;

    pub type Bytes = i32;
    pub type VecU8 = i32;
    pub type BytesView = i32;

    #[derive(Clone, Debug)]
    struct View {
        b: Bytes,
        start: usize,
        len: usize,
    }

    #[derive(Default)]
    struct Rt {
        bytes: Vec<Vec<u8>>,
        vecs: Vec<Vec<u8>>,
        views: Vec<View>,
    }

    thread_local! {
        static RT: RefCell<Rt> = RefCell::new(Rt::default());
    }

    fn bytes_index(b: Bytes) -> usize {
        usize::try_from(b)
            .ok()
            .and_then(|v| v.checked_sub(1))
            .unwrap()
    }

    fn view_index(v: BytesView) -> usize {
        usize::try_from(-v)
            .ok()
            .and_then(|v| v.checked_sub(1))
            .unwrap()
    }

    fn resolve_span(rt: &Rt, v: BytesView) -> (Bytes, usize, usize) {
        assert!(v != 0);
        if v < 0 {
            let vw = &rt.views[view_index(v)];
            (vw.b, vw.start, vw.len)
        } else {
            let b = v;
            let len = rt.bytes[bytes_index(b)].len();
            (b, 0, len)
        }
    }

    pub fn reset() {
        RT.with(|rt| *rt.borrow_mut() = Rt::default());
    }

    pub fn bytes_from_slice(s: &[u8]) -> Bytes {
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.bytes.push(s.to_vec());
            rt.bytes.len() as i32
        })
    }

    pub fn bytes_to_vec(b: Bytes) -> Vec<u8> {
        RT.with(|rt| rt.borrow().bytes[(b as usize) - 1].clone())
    }

    pub fn bytes_alloc(n: i32) -> Bytes {
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let n = usize::try_from(n).unwrap_or(0);
            rt.bytes.push(vec![0u8; n]);
            rt.bytes.len() as i32
        })
    }

    pub fn bytes_len(b: BytesView) -> i32 {
        RT.with(|rt| {
            let rt = rt.borrow();
            let (_b, _start, len) = resolve_span(&rt, b);
            len as i32
        })
    }

    pub fn bytes_get_u8(b: BytesView, i: i32) -> i32 {
        RT.with(|rt| {
            let rt = rt.borrow();
            let (b, start, _len) = resolve_span(&rt, b);
            let idx = start + (i as usize);
            rt.bytes[bytes_index(b)][idx] as i32
        })
    }

    pub fn bytes_set_u8(b: Bytes, i: i32, v: i32) -> Bytes {
        assert!(b > 0);
        RT.with(|rt| rt.borrow_mut().bytes[bytes_index(b)][i as usize] = v as u8);
        b
    }

    pub fn bytes_view(b: Bytes) -> BytesView {
        assert!(b > 0);
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let len = rt.bytes[bytes_index(b)].len();
            rt.views.push(View { b, start: 0, len });
            -(rt.views.len() as i32)
        })
    }

    pub fn view_len(v: BytesView) -> i32 {
        RT.with(|rt| {
            let rt = rt.borrow();
            let (_b, _start, len) = resolve_span(&rt, v);
            len as i32
        })
    }

    pub fn view_get_u8(v: BytesView, i: i32) -> i32 {
        RT.with(|rt| {
            let rt = rt.borrow();
            let (b, start, _len) = resolve_span(&rt, v);
            let idx = start + (i as usize);
            rt.bytes[bytes_index(b)][idx] as i32
        })
    }

    pub fn view_slice(v: BytesView, start: i32, len: i32) -> BytesView {
        let start = usize::try_from(start).unwrap_or(0);
        let len = usize::try_from(len).unwrap_or(0);
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let (b, base_start, base_len) = resolve_span(&rt, v);
            let start = start.min(base_len);
            let len = len.min(base_len.saturating_sub(start));
            rt.views.push(View {
                b,
                start: base_start + start,
                len,
            });
            -(rt.views.len() as i32)
        })
    }

    pub fn view_to_bytes(v: BytesView) -> Bytes {
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let (b, start, len) = resolve_span(&rt, v);
            let buf = &rt.bytes[bytes_index(b)];
            let end = start.saturating_add(len).min(buf.len());
            let slice = buf[start..end].to_vec();
            rt.bytes.push(slice);
            rt.bytes.len() as i32
        })
    }

    pub fn vec_u8_with_capacity(cap: i32) -> VecU8 {
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let cap = usize::try_from(cap).unwrap_or(0);
            rt.vecs.push(Vec::with_capacity(cap));
            rt.vecs.len() as i32
        })
    }

    pub fn vec_u8_len(h: VecU8) -> i32 {
        RT.with(|rt| rt.borrow().vecs[(h as usize) - 1].len() as i32)
    }

    pub fn vec_u8_push(h: VecU8, x: i32) -> VecU8 {
        RT.with(|rt| rt.borrow_mut().vecs[(h as usize) - 1].push(x as u8));
        h
    }

    pub fn vec_u8_extend_bytes_range(h: VecU8, b: BytesView, start: i32, len: i32) -> VecU8 {
        let start = usize::try_from(start).unwrap_or(0);
        let len = usize::try_from(len).unwrap_or(0);
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let (bb, base_start, base_len) = resolve_span(&rt, b);
            let start = start.min(base_len);
            let end = start.saturating_add(len).min(base_len);
            let buf = &rt.bytes[bytes_index(bb)];
            let slice = buf[(base_start + start)..(base_start + end)].to_vec();
            rt.vecs[(h as usize) - 1].extend_from_slice(&slice);
        });
        h
    }

    pub fn vec_u8_into_bytes(h: VecU8) -> Bytes {
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let buf = rt.vecs[(h as usize) - 1].clone();
            rt.vecs[(h as usize) - 1].clear();
            rt.bytes.push(buf);
            rt.bytes.len() as i32
        })
    }

    pub fn lt_u(a: i32, b: i32) -> bool {
        (a as u32) < (b as u32)
    }

    pub fn ge_u(a: i32, b: i32) -> bool {
        (a as u32) >= (b as u32)
    }

    pub fn codec_read_u32_le(b: BytesView, off: i32) -> i32 {
        let off = usize::try_from(off).unwrap_or(0);
        RT.with(|rt| {
            let rt = rt.borrow();
            let (bb, start, len) = resolve_span(&rt, b);
            let buf = &rt.bytes[bytes_index(bb)];
            let off = off.min(len.saturating_sub(4));
            let base = start + off;
            let raw: [u8; 4] = buf[base..base + 4].try_into().unwrap();
            u32::from_le_bytes(raw) as i32
        })
    }

    pub fn codec_write_u32_le(x: i32) -> Bytes {
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let ux = x as u32;
            rt.bytes.push(ux.to_le_bytes().to_vec());
            rt.bytes.len() as i32
        })
    }

    pub fn fmt_u32_to_dec(x: i32) -> Bytes {
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let s = (x as u32).to_string();
            rt.bytes.push(s.into_bytes());
            rt.bytes.len() as i32
        })
    }

    pub fn fmt_s32_to_dec(x: i32) -> Bytes {
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            let s = x.to_string();
            rt.bytes.push(s.into_bytes());
            rt.bytes.len() as i32
        })
    }
}

#[allow(
    clippy::assign_op_pattern,
    clippy::if_same_then_else,
    clippy::needless_bool
)]
mod ascii_ref {
    use super::rt::*;
    include!("../../../labs/x07import/fixtures/import_sources/rust/std_text_ascii@0.1.1/ascii.rs");
}

#[allow(
    clippy::assign_op_pattern,
    clippy::if_same_then_else,
    clippy::needless_bool
)]
mod utf8_ref {
    use super::rt::*;
    include!("../../../labs/x07import/fixtures/import_sources/rust/std_text_utf8@0.1.1/utf8.rs");
}

#[allow(
    clippy::assign_op_pattern,
    clippy::if_same_then_else,
    clippy::needless_bool
)]
mod regex_ref {
    use super::rt::*;
    include!(
        "../../../labs/x07import/fixtures/import_sources/rust/std_regex_lite@0.1.1/regex_lite.rs"
    );
}

#[allow(
    clippy::assign_op_pattern,
    clippy::if_same_then_else,
    clippy::needless_bool
)]
mod codec_ref {
    use super::rt::*;
    include!("../../../labs/x07import/fixtures/import_sources/rust/std_codec@0.1.1/codec.rs");
}

#[allow(
    clippy::assign_op_pattern,
    clippy::if_same_then_else,
    clippy::needless_bool
)]
mod fmt_ref {
    use super::rt::*;
    include!("../../../labs/x07import/fixtures/import_sources/rust/std_fmt@0.1.1/fmt.rs");
}
