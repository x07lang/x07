use x07c::compile::{CompileErrorKind, CompileOptions};
use x07c::program::Program;
use x07c::wasm_emit::features::{WasmFeatureSetV1, WasmFeatureV1};
use x07c::wasm_emit::{WasmEmitOptions, WasmMemLimits};

fn expr_ident(name: &str) -> x07c::ast::Expr {
    x07c::ast::Expr::Ident {
        name: name.to_string(),
        ptr: String::new(),
    }
}

fn expr_int(value: i32) -> x07c::ast::Expr {
    x07c::ast::Expr::Int {
        value,
        ptr: String::new(),
    }
}

fn expr_list(items: Vec<x07c::ast::Expr>) -> x07c::ast::Expr {
    x07c::ast::Expr::List {
        items,
        ptr: String::new(),
    }
}

fn program_with_solve(solve: x07c::ast::Expr) -> Program {
    Program {
        functions: Vec::new(),
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve,
    }
}

fn default_compile_options() -> CompileOptions {
    CompileOptions {
        freestanding: true,
        world: x07_worlds::WorldId::SolvePure,
        ..Default::default()
    }
}

fn default_mem_limits() -> WasmMemLimits {
    WasmMemLimits {
        initial_memory_bytes: 2 * 65536,
        max_memory_bytes: 2 * 65536,
        no_growable_memory: true,
    }
}

#[test]
fn wasm_feature_gate_bytes_builtins_blocks_cmp_range() {
    let program = program_with_solve(expr_list(vec![
        expr_ident("begin"),
        expr_list(vec![
            expr_ident("bytes.cmp_range"),
            expr_ident("input"),
            expr_int(0),
            expr_int(0),
            expr_ident("input"),
            expr_int(0),
            expr_int(0),
        ]),
        expr_list(vec![expr_ident("view.to_bytes"), expr_ident("input")]),
    ]));

    let options = default_compile_options();
    let wasm_opts = WasmEmitOptions {
        mem: default_mem_limits(),
        features: WasmFeatureSetV1::new(&[
            WasmFeatureV1::CoreFormsV1,
            WasmFeatureV1::ViewToBytesV1,
            WasmFeatureV1::ViewReadV1,
        ]),
    };

    let err = x07c::wasm_emit::emit_solve_pure_wasm_v1(&program, &options, &wasm_opts)
        .expect_err("expected feature gate error");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);

    let diag = err.diagnostic.expect("expected diagnostic");
    assert_eq!(diag.code, "X07C_WASM_BACKEND_UNSUPPORTED_BUILTIN");
    assert_eq!(
        diag.data.get("kind").and_then(|v| v.as_str()),
        Some("builtin")
    );
    assert_eq!(
        diag.data.get("name").and_then(|v| v.as_str()),
        Some("bytes.cmp_range")
    );
    assert_eq!(
        diag.data.get("requires_feature").and_then(|v| v.as_str()),
        Some("BytesBuiltinsV1")
    );
}

#[test]
fn wasm_feature_gate_ops_logic_blocks_and() {
    let program = program_with_solve(expr_list(vec![
        expr_ident("begin"),
        expr_list(vec![expr_ident("&&"), expr_int(1), expr_int(1)]),
        expr_list(vec![expr_ident("view.to_bytes"), expr_ident("input")]),
    ]));

    let options = default_compile_options();
    let wasm_opts = WasmEmitOptions {
        mem: default_mem_limits(),
        features: WasmFeatureSetV1::new(&[
            WasmFeatureV1::CoreFormsV1,
            WasmFeatureV1::ViewToBytesV1,
            WasmFeatureV1::ViewReadV1,
        ]),
    };

    let err = x07c::wasm_emit::emit_solve_pure_wasm_v1(&program, &options, &wasm_opts)
        .expect_err("expected feature gate error");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);

    let diag = err.diagnostic.expect("expected diagnostic");
    assert_eq!(diag.code, "X07C_WASM_BACKEND_UNSUPPORTED_OP");
    assert_eq!(diag.data.get("kind").and_then(|v| v.as_str()), Some("op"));
    assert_eq!(diag.data.get("name").and_then(|v| v.as_str()), Some("&&"));
    assert_eq!(
        diag.data.get("requires_feature").and_then(|v| v.as_str()),
        Some("OpsLogicV1")
    );
}

#[test]
fn wasm_feature_gate_fmt_builtins_blocks_s32_to_dec() {
    let program = program_with_solve(expr_list(vec![expr_ident("fmt.s32_to_dec"), expr_int(-1)]));

    let options = default_compile_options();
    let wasm_opts = WasmEmitOptions {
        mem: default_mem_limits(),
        features: WasmFeatureSetV1::new(&[WasmFeatureV1::CoreFormsV1]),
    };

    let err = x07c::wasm_emit::emit_solve_pure_wasm_v1(&program, &options, &wasm_opts)
        .expect_err("expected feature gate error");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);

    let diag = err.diagnostic.expect("expected diagnostic");
    assert_eq!(diag.code, "X07C_WASM_BACKEND_UNSUPPORTED_BUILTIN");
    assert_eq!(
        diag.data.get("kind").and_then(|v| v.as_str()),
        Some("builtin")
    );
    assert_eq!(
        diag.data.get("name").and_then(|v| v.as_str()),
        Some("fmt.s32_to_dec")
    );
    assert_eq!(
        diag.data.get("requires_feature").and_then(|v| v.as_str()),
        Some("FmtBuiltinsV1")
    );
}

#[test]
fn wasm_feature_gate_parse_builtins_blocks_u32_dec() {
    let program = program_with_solve(expr_list(vec![
        expr_ident("parse.u32_dec"),
        expr_ident("input"),
    ]));

    let options = default_compile_options();
    let wasm_opts = WasmEmitOptions {
        mem: default_mem_limits(),
        features: WasmFeatureSetV1::new(&[WasmFeatureV1::CoreFormsV1]),
    };

    let err = x07c::wasm_emit::emit_solve_pure_wasm_v1(&program, &options, &wasm_opts)
        .expect_err("expected feature gate error");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);

    let diag = err.diagnostic.expect("expected diagnostic");
    assert_eq!(diag.code, "X07C_WASM_BACKEND_UNSUPPORTED_BUILTIN");
    assert_eq!(
        diag.data.get("kind").and_then(|v| v.as_str()),
        Some("builtin")
    );
    assert_eq!(
        diag.data.get("name").and_then(|v| v.as_str()),
        Some("parse.u32_dec")
    );
    assert_eq!(
        diag.data.get("requires_feature").and_then(|v| v.as_str()),
        Some("ParseBuiltinsV1")
    );
}

#[test]
fn wasm_backend_accepts_contract_pure_surface_by_default() {
    let program = program_with_solve(expr_list(vec![
        expr_ident("begin"),
        expr_list(vec![expr_ident("/"), expr_int(6), expr_int(3)]),
        expr_list(vec![expr_ident("%"), expr_int(7), expr_int(3)]),
        expr_list(vec![
            expr_ident("view.to_bytes"),
            expr_list(vec![
                expr_ident("bytes.subview"),
                expr_list(vec![expr_ident("bytes.lit"), expr_ident("abcd")]),
                expr_list(vec![expr_ident("i32.lit"), expr_int(0)]),
                expr_list(vec![expr_ident("i32.lit"), expr_int(2)]),
            ]),
        ]),
    ]));

    let options = default_compile_options();
    let wasm_opts = WasmEmitOptions {
        mem: default_mem_limits(),
        features: x07c::wasm_emit::features::supported_features_v1(),
    };

    let wasm =
        x07c::wasm_emit::emit_solve_pure_wasm_v1(&program, &options, &wasm_opts).expect("emit");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("validate");
}
