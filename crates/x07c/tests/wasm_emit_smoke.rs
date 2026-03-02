use x07c::compile::CompileOptions;
use x07c::program::Program;
use x07c::wasm_emit::{WasmEmitOptions, WasmMemLimits};

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
    let mut mem_min: Option<u64> = None;
    let mut mem_max: Option<u64> = None;

    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        match payload.expect("parse") {
            wasmparser::Payload::ExportSection(s) => {
                for e in s {
                    exports.push(e.expect("export").name.to_string());
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
}
