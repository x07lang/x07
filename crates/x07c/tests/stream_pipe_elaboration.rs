use serde_json::json;

use x07_worlds::WorldId;
use x07c::ast::expr_from_json;
use x07c::compile::CompileOptions;
use x07c::program::{AsyncFunctionDef, FunctionDef, Program};
use x07c::stream_pipe;
use x07c::types::Ty;

fn ident(name: &str) -> x07c::ast::Expr {
    x07c::ast::Expr::Ident {
        name: name.to_string(),
        ptr: String::new(),
    }
}

fn list(items: Vec<x07c::ast::Expr>) -> x07c::ast::Expr {
    x07c::ast::Expr::List {
        items,
        ptr: String::new(),
    }
}

fn pipe_expr(path_lit: &str) -> x07c::ast::Expr {
    expr_from_json(&json!([
        "std.stream.pipe_v1",
        [
            "std.stream.cfg_v1",
            ["chunk_max_bytes", 65536],
            ["bufread_cap_bytes", 65536],
            ["max_in_bytes", 1024],
            ["max_out_bytes", 1024],
            ["max_items", 100]
        ],
        [
            "std.stream.src.fs_open_read_v1",
            ["std.stream.expr_v1", ["bytes.lit", path_lit]]
        ],
        [
            "std.stream.chain_v1",
            [
                "std.stream.xf.split_lines_v1",
                ["std.stream.expr_v1", 10],
                ["std.stream.expr_v1", 128]
            ]
        ],
        ["std.stream.sink.collect_bytes_v1"]
    ]))
    .expect("expr_from_json")
}

fn pipe_expr_par_map() -> x07c::ast::Expr {
    expr_from_json(&json!([
        "std.stream.pipe_v1",
        [
            "std.stream.cfg_v1",
            ["chunk_max_bytes", 64],
            ["bufread_cap_bytes", 64],
            ["max_in_bytes", 1024],
            ["max_out_bytes", 1024],
            ["max_items", 100]
        ],
        [
            "std.stream.src.bytes_v1",
            ["std.stream.expr_v1", ["bytes.lit", "a\nb\nc\n"]]
        ],
        [
            "std.stream.chain_v1",
            [
                "std.stream.xf.split_lines_v1",
                ["std.stream.expr_v1", 10],
                ["std.stream.expr_v1", 128]
            ],
            [
                "std.stream.xf.par_map_stream_v1",
                ["max_inflight", 2],
                ["max_item_bytes", 64],
                ["mapper_defasync", ["std.stream.fn_v1", "main.mapper"]]
            ]
        ],
        ["std.stream.sink.collect_bytes_v1"]
    ]))
    .expect("expr_from_json")
}

fn contains_head(expr: &x07c::ast::Expr, head: &str) -> bool {
    match expr {
        x07c::ast::Expr::Int { .. } | x07c::ast::Expr::Ident { .. } => false,
        x07c::ast::Expr::List { items, .. } => {
            if items.first().and_then(x07c::ast::Expr::as_ident) == Some(head) {
                return true;
            }
            items.iter().any(|e| contains_head(e, head))
        }
    }
}

#[test]
fn pipe_elaboration_injects_helper_and_rewrites_call_site() {
    let mut program = Program {
        functions: Vec::new(),
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: pipe_expr("in.txt"),
    };

    stream_pipe::elaborate_stream_pipes(&mut program, &CompileOptions::default())
        .expect("elaboration");

    let helpers: Vec<_> = program
        .functions
        .iter()
        .filter(|f| f.name.starts_with("main.__std_stream_pipe_v1_"))
        .collect();
    assert_eq!(helpers.len(), 1);
    let helper = helpers[0];
    assert_eq!(helper.params.len(), 3);
    assert_eq!(helper.params[0].ty, Ty::BytesView);
    assert_eq!(helper.params[1].ty, Ty::I32);
    assert_eq!(helper.params[2].ty, Ty::I32);
    let x07c::ast::Expr::List {
        items: helper_items,
        ..
    } = &helper.body
    else {
        panic!("helper body is not a list");
    };
    assert_eq!(
        helper_items.first().and_then(x07c::ast::Expr::as_ident),
        Some("begin")
    );

    let solve = &program.solve;
    let x07c::ast::Expr::List { items, .. } = solve else {
        panic!("solve is not a list");
    };
    assert_eq!(
        items.first().and_then(x07c::ast::Expr::as_ident),
        Some("begin")
    );
    // begin, let arg0, let arg1, let arg2, call
    assert_eq!(items.len(), 5);
    let call = &items[4];
    let x07c::ast::Expr::List {
        items: call_items, ..
    } = call
    else {
        panic!("call is not a list");
    };
    assert_eq!(
        call_items.first().and_then(x07c::ast::Expr::as_ident),
        Some(helper.name.as_str())
    );
    assert_eq!(call_items.len(), 4);
}

#[test]
fn pipe_elaboration_injects_async_helper_and_rewrites_call_site_to_await() {
    let mut program = Program {
        functions: Vec::new(),
        async_functions: vec![AsyncFunctionDef {
            name: "main.mapper".to_string(),
            params: vec![
                x07c::program::FunctionParam {
                    name: "ctx".to_string(),
                    ty: Ty::BytesView,
                },
                x07c::program::FunctionParam {
                    name: "item".to_string(),
                    ty: Ty::Bytes,
                },
            ],
            ret_ty: Ty::Bytes,
            body: ident("item"),
        }],
        extern_functions: Vec::new(),
        solve: pipe_expr_par_map(),
    };

    stream_pipe::elaborate_stream_pipes(&mut program, &CompileOptions::default())
        .expect("elaboration");
    x07c::c_emit::check_c_program(&program, &CompileOptions::default()).expect("check_c_program");

    let helpers: Vec<_> = program
        .async_functions
        .iter()
        .filter(|f| f.name.starts_with("main.__std_stream_pipe_v1_"))
        .collect();
    assert_eq!(helpers.len(), 1);
    let helper = helpers[0];
    assert_eq!(helper.params.len(), 3);
    assert_eq!(helper.params[0].ty, Ty::Bytes);
    assert_eq!(helper.params[1].ty, Ty::I32);
    assert_eq!(helper.params[2].ty, Ty::I32);
    assert!(
        contains_head(&helper.body, "task.scope_v1"),
        "expected task.scope_v1 wrapper in async pipe helper"
    );

    let solve = &program.solve;
    let x07c::ast::Expr::List { items, .. } = solve else {
        panic!("solve is not a list");
    };
    assert_eq!(
        items.first().and_then(x07c::ast::Expr::as_ident),
        Some("begin")
    );
    // begin, let arg0, let arg1, let arg2, await(call)
    assert_eq!(items.len(), 5);
    let await_expr = &items[4];
    let x07c::ast::Expr::List {
        items: await_items, ..
    } = await_expr
    else {
        panic!("await is not a list");
    };
    assert_eq!(
        await_items.first().and_then(x07c::ast::Expr::as_ident),
        Some("await")
    );
    assert_eq!(await_items.len(), 2);
    let call = &await_items[1];
    let x07c::ast::Expr::List {
        items: call_items, ..
    } = call
    else {
        panic!("call is not a list");
    };
    assert_eq!(
        call_items.first().and_then(x07c::ast::Expr::as_ident),
        Some(helper.name.as_str())
    );
    assert_eq!(call_items.len(), 4);
}

#[test]
fn pipe_elaboration_rejects_concurrency_pipes_inside_defn() {
    let mut program = Program {
        functions: vec![FunctionDef {
            name: "main.f".to_string(),
            params: Vec::new(),
            ret_ty: Ty::Bytes,
            body: pipe_expr_par_map(),
        }],
        async_functions: vec![AsyncFunctionDef {
            name: "main.mapper".to_string(),
            params: vec![
                x07c::program::FunctionParam {
                    name: "ctx".to_string(),
                    ty: Ty::BytesView,
                },
                x07c::program::FunctionParam {
                    name: "item".to_string(),
                    ty: Ty::Bytes,
                },
            ],
            ret_ty: Ty::Bytes,
            body: ident("item"),
        }],
        extern_functions: Vec::new(),
        solve: pipe_expr("in.txt"),
    };

    let err = stream_pipe::elaborate_stream_pipes(&mut program, &CompileOptions::default())
        .expect_err("must reject concurrency pipe in defn");
    assert_eq!(err.kind, x07c::compile::CompileErrorKind::Typing);
    assert!(
        err.message.contains(
            "std.stream.pipe_v1 with concurrency stages is only allowed in solve or defasync"
        ),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn pipe_elaboration_dedups_helper_by_hash_ignoring_expr_bodies() {
    let mut program = Program {
        functions: Vec::new(),
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: list(vec![ident("begin"), pipe_expr("a.txt"), pipe_expr("b.txt")]),
    };

    stream_pipe::elaborate_stream_pipes(&mut program, &CompileOptions::default())
        .expect("elaboration");

    let helper_count = program
        .functions
        .iter()
        .filter(|f| f.name.starts_with("main.__std_stream_pipe_v1_"))
        .count();
    assert_eq!(helper_count, 1);
}

#[test]
fn pipe_elaboration_streaming_fs_sink_emits_stream_builtins() {
    let mut program = Program {
        functions: Vec::new(),
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: expr_from_json(&json!([
            "std.stream.pipe_v1",
            [
                "std.stream.cfg_v1",
                ["chunk_max_bytes", 64],
                ["bufread_cap_bytes", 64],
                ["max_in_bytes", 1024],
                ["max_out_bytes", 1024],
                ["max_items", 100]
            ],
            [
                "std.stream.src.bytes_v1",
                ["std.stream.expr_v1", ["bytes.lit", "abc"]]
            ],
            ["std.stream.chain_v1"],
            [
                "std.stream.sink.world_fs_write_stream_v1",
                ["std.stream.expr_v1", ["bytes.lit", "out.bin"]],
                ["std.stream.expr_v1", ["bytes.lit", "caps"]]
            ]
        ]))
        .expect("expr_from_json"),
    };

    let options = CompileOptions {
        world: WorldId::RunOs,
        ..Default::default()
    };

    stream_pipe::elaborate_stream_pipes(&mut program, &options).expect("elaboration");

    let helper = program
        .functions
        .iter()
        .find(|f| f.name.starts_with("main.__std_stream_pipe_v1_"))
        .expect("helper injected");

    assert!(contains_head(&helper.body, "os.fs.stream_open_write_v1"));
    assert!(contains_head(&helper.body, "os.fs.stream_write_all_v1"));
    assert!(contains_head(&helper.body, "os.fs.stream_close_v1"));
    assert!(contains_head(&helper.body, "os.fs.stream_drop_v1"));
}

#[test]
fn pipe_elaboration_is_per_module() {
    let mut program = Program {
        functions: vec![FunctionDef {
            name: "foo.test".to_string(),
            params: Vec::new(),
            ret_ty: Ty::Bytes,
            body: pipe_expr("in.txt"),
        }],
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: pipe_expr("in.txt"),
    };

    stream_pipe::elaborate_stream_pipes(&mut program, &CompileOptions::default())
        .expect("elaboration");

    let main_helpers: Vec<_> = program
        .functions
        .iter()
        .filter(|f| f.name.starts_with("main.__std_stream_pipe_v1_"))
        .collect();
    let foo_helpers: Vec<_> = program
        .functions
        .iter()
        .filter(|f| f.name.starts_with("foo.__std_stream_pipe_v1_"))
        .collect();
    assert_eq!(main_helpers.len(), 1);
    assert_eq!(foo_helpers.len(), 1);

    let main_suffix = main_helpers[0]
        .name
        .trim_start_matches("main.__std_stream_pipe_v1_");
    let foo_suffix = foo_helpers[0]
        .name
        .trim_start_matches("foo.__std_stream_pipe_v1_");
    assert_eq!(main_suffix, foo_suffix);
}
