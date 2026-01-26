use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    }
}

fn entry(imports: &[&str], solve: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": "x07.x07ast@0.2.0",
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
    assert!(
        compile.ok,
        "compile_error={:?}\ncompile stdout:\n{}\ncompile stderr:\n{}",
        compile.compile_error,
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );
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

fn compile_c_ref(tmp: &tempfile::TempDir, src: &Path) -> PathBuf {
    let cc = std::env::var("X07_CC").unwrap_or_else(|_| "cc".to_string());
    let out = tmp.path().join("c_ref");

    let ref_src = std::fs::read_to_string(src).expect("read C ref src");
    std::fs::write(tmp.path().join("ref.c"), ref_src.as_bytes()).expect("write ref.c");

    let harness = r#"
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "ref.c"

static int read_u32_le(uint32_t* out) {
  uint8_t buf[4];
  if (fread(buf, 1, 4, stdin) != 4) return 0;
  *out = ((uint32_t)buf[0]) |
         ((uint32_t)buf[1] << 8) |
         ((uint32_t)buf[2] << 16) |
         ((uint32_t)buf[3] << 24);
  return 1;
}

static void write_u32_le(uint32_t x) {
  uint8_t buf[4];
  buf[0] = (uint8_t)(x & 0xffu);
  buf[1] = (uint8_t)((x >> 8) & 0xffu);
  buf[2] = (uint8_t)((x >> 16) & 0xffu);
  buf[3] = (uint8_t)((x >> 24) & 0xffu);
  (void)fwrite(buf, 1, 4, stdout);
}

int main(int argc, char** argv) {
  if (argc != 2) return 3;

  uint32_t raw = 0;
  if (!read_u32_le(&raw)) return 2;
  int32_t x = (int32_t)raw;

  int32_t y = 0;
  if (strcmp(argv[1], "add1") == 0) {
    y = add1(x);
  } else if (strcmp(argv[1], "abs_i32") == 0) {
    y = abs_i32(x);
  } else {
    return 4;
  }

  write_u32_le((uint32_t)y);
  return 0;
}
"#;

    let harness_path = tmp.path().join("harness.c");
    std::fs::write(&harness_path, harness.as_bytes()).expect("write harness.c");

    let status = Command::new(&cc)
        .arg("-std=c11")
        .arg("-O0")
        .arg("-o")
        .arg(&out)
        .arg(&harness_path)
        .status()
        .expect("cc invocation ok");
    assert!(status.success(), "cc failed: {cc}");

    out
}

fn run_c_ref(exe: &Path, func: &str, input: &[u8]) -> Vec<u8> {
    let mut child = Command::new(exe)
        .arg(func)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn c_ref");

    {
        let mut stdin = child.stdin.take().expect("stdin");
        use std::io::Write;
        stdin.write_all(input).expect("write input");
    }

    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success(), "c_ref failed: {out:?}");
    out.stdout
}

#[test]
fn c_frontend_smoke() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().to_path_buf();

    let module_id = "x07import_test.c.smoke";
    let src_abs =
        repo_root().join("tests/x07import/fixtures/import_sources/c/x07import_smoke@0.1.0/smoke.c");
    let m = x07import_core::c::import_c_file(module_id, &src_abs).expect("import C");
    let src = x07import_core::x07_emit::emit_module(&m).expect("emit module");
    write_module(&module_root, module_id, &src);

    let c_ref_exe = compile_c_ref(&tmp, &src_abs);

    let add1 = format!("{module_id}.add1");
    let abs_i32 = format!("{module_id}.abs_i32");

    let program_add1 = entry(
        &[module_id],
        json!([
            "codec.write_u32_le",
            [add1, ["codec.read_u32_le", "input", 0]]
        ]),
    );
    let exe_add1 = compile_exe(program_add1.as_slice(), vec![module_root.clone()]);

    let program_abs = entry(
        &[module_id],
        json!([
            "codec.write_u32_le",
            [abs_i32, ["codec.read_u32_le", "input", 0]]
        ]),
    );
    let exe_abs = compile_exe(program_abs.as_slice(), vec![module_root.clone()]);

    for input in [
        (41i32).to_le_bytes().to_vec(),
        (-5i32).to_le_bytes().to_vec(),
        (0i32).to_le_bytes().to_vec(),
    ] {
        let out_x07 = run_exe(&exe_add1, &input);
        let out_c = run_c_ref(&c_ref_exe, "add1", &input);
        assert_eq!(out_x07, out_c);

        let out_x07 = run_exe(&exe_abs, &input);
        let out_c = run_c_ref(&c_ref_exe, "abs_i32", &input);
        assert_eq!(out_x07, out_c);
    }
}
