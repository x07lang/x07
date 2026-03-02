use x07c::compile::CompileOptions;
use x07c::program::Program;
use x07c::wasm_emit::{WasmEmitOptions, WasmMemLimits};

fn const_expr_i32(expr: &wasmparser::ConstExpr<'_>) -> Option<i32> {
    let mut r = expr.get_operators_reader();
    match (r.read().ok()?, r.read().ok()?) {
        (wasmparser::Operator::I32Const { value }, wasmparser::Operator::End) => Some(value),
        _ => None,
    }
}

fn empty_program() -> Program {
    Program {
        functions: Vec::new(),
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: x07c::ast::Expr::List {
            items: vec![
                x07c::ast::Expr::Ident {
                    name: "bytes.lit".to_string(),
                    ptr: String::new(),
                },
                x07c::ast::Expr::Ident {
                    name: "".to_string(),
                    ptr: String::new(),
                },
            ],
            ptr: String::new(),
        },
    }
}

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

fn program_with_contract_pure_cmp() -> Program {
    Program {
        functions: Vec::new(),
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("view.eq"),
                expr_ident("input"),
                expr_ident("input"),
            ]),
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
        ]),
    }
}

#[test]
fn wasm_emit_smoke_exports_and_memory_limits() {
    let program = empty_program();
    let options = CompileOptions {
        freestanding: true,
        ..Default::default()
    };

    let wasm = x07c::wasm_emit::emit_solve_pure_wasm_v1(
        &program,
        &options,
        &WasmEmitOptions {
            mem: WasmMemLimits {
                initial_memory_bytes: 2 * 65536,
                max_memory_bytes: 2 * 65536,
                no_growable_memory: true,
            },
            features: x07c::wasm_emit::features::supported_features_v1(),
        },
    )
    .expect("emit wasm");

    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("validate wasm");

    let mut exports: Vec<String> = Vec::new();
    let mut heap_base_global_idx: Option<u32> = None;
    let mut data_end_global_idx: Option<u32> = None;
    let mut global_i32_init: Vec<Option<i32>> = Vec::new();
    let mut mem_min: Option<u64> = None;
    let mut mem_max: Option<u64> = None;

    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        match payload.expect("parse") {
            wasmparser::Payload::ExportSection(s) => {
                for e in s {
                    let e = e.expect("export");
                    exports.push(e.name.to_string());
                    if e.kind == wasmparser::ExternalKind::Global {
                        match e.name {
                            "__heap_base" => heap_base_global_idx = Some(e.index),
                            "__data_end" => data_end_global_idx = Some(e.index),
                            _ => {}
                        }
                    }
                }
            }
            wasmparser::Payload::GlobalSection(s) => {
                for g in s {
                    let g = g.expect("global");
                    global_i32_init.push(const_expr_i32(&g.init_expr));
                }
            }
            wasmparser::Payload::MemorySection(s) => {
                for m in s {
                    let ty = m.expect("memory");
                    mem_min = Some(ty.initial);
                    mem_max = ty.maximum;
                }
            }
            _ => {}
        }
    }

    exports.sort();
    for want in ["memory", "x07_solve_v2", "__heap_base", "__data_end"] {
        assert!(
            exports.iter().any(|e| e == want),
            "missing export {want:?} (have: {exports:?})"
        );
    }

    assert_eq!(mem_min, Some(2));
    assert_eq!(mem_max, Some(2));

    let heap_base_global_idx = heap_base_global_idx.expect("missing global export: __heap_base");
    let data_end_global_idx = data_end_global_idx.expect("missing global export: __data_end");

    let heap_base = u32::try_from(
        global_i32_init[heap_base_global_idx as usize].expect("__heap_base must be i32.const"),
    )
    .expect("__heap_base must be non-negative");
    let data_end = u32::try_from(
        global_i32_init[data_end_global_idx as usize].expect("__data_end must be i32.const"),
    )
    .expect("__data_end must be non-negative");

    assert!(
        heap_base.is_multiple_of(16),
        "__heap_base must be 16-byte aligned (got {heap_base})"
    );
    assert!(
        heap_base >= 16,
        "__heap_base must reserve low memory (got {heap_base})"
    );
    assert!(
        heap_base >= data_end,
        "__heap_base must be >= __data_end (heap_base={heap_base} data_end={data_end})"
    );
}

#[test]
fn wasm_emit_validates_view_eq_and_bytes_cmp_range() {
    let program = program_with_contract_pure_cmp();
    let options = CompileOptions {
        freestanding: true,
        world: x07_worlds::WorldId::SolvePure,
        ..Default::default()
    };

    let wasm = x07c::wasm_emit::emit_solve_pure_wasm_v1(
        &program,
        &options,
        &WasmEmitOptions {
            mem: WasmMemLimits {
                initial_memory_bytes: 2 * 65536,
                max_memory_bytes: 2 * 65536,
                no_growable_memory: true,
            },
            features: x07c::wasm_emit::features::supported_features_v1(),
        },
    )
    .expect("emit wasm");

    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("validate wasm");
}
