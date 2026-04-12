use serde_json::json;

use x07_contracts::X07AST_SCHEMA_VERSION_V0_8_0;
use x07c::typecheck::{typecheck_file_local, TypecheckOptions};

mod typecheck_testutil;

#[test]
fn typecheck_builtins_smoke_and_call_arg_coercions() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.smoke",
                "params": [],
                "result": "bytes",
                "body": [
                    "begin",
                    ["let", "b", ["bytes.lit", "abcd"]],
                    ["let", "o", ["option_i32.some", 1]],
                    ["let", "x", ["option_i32.unwrap_or", "o", 0]],
                    ["let", "rb", ["result_bytes.err", "x"]],
                    ["let", "_rb_code", ["result_bytes.err_code", "rb"]],
                    ["let", "_rb_payload", ["result_bytes.unwrap_or", "rb", ["bytes.alloc", 0]]],
                    ["let", "sl", ["bytes.slice", "b", 0, 2]],
                    ["let", "_u32", ["codec.read_u32_le", "b", 0]],
                    ["let", "buf", ["vec_u8.with_capacity", 0]],
                    ["set", "buf", ["vec_u8.extend_bytes_range", "buf", "b", 0, 2]],
                    ["set", "buf", ["vec_u8.extend_bytes", "buf", "b"]],
                    ["let", "v", ["vec_u8.as_view", "buf"]],
                    ["let", "eq", ["view.eq", "v", ["bytes.view", "sl"]]],
                    ["if", ["=", "eq", 1], ["vec_u8.into_bytes", "buf"], ["bytes.copy", "sl", "sl"]]
                ]
            }
        ],
        "solve": ["main.smoke"]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        report.diagnostics
    );

    let mut got = report
        .implicit_call_arg_coercions
        .iter()
        .map(|c| {
            (
                c.callee.as_str(),
                c.arg_index,
                c.got.as_str(),
                c.want.as_str(),
            )
        })
        .collect::<Vec<_>>();
    got.sort();

    let want = [
        ("bytes.slice", 0, "bytes", "bytes_view"),
        ("codec.read_u32_le", 0, "bytes", "bytes_view"),
        ("vec_u8.extend_bytes", 1, "bytes", "bytes_view"),
        ("vec_u8.extend_bytes_range", 1, "bytes", "bytes_view"),
    ];
    for it in want {
        assert!(
            got.contains(&it),
            "expected implicit coercion {:?}; got: {:?}",
            it,
            got
        );
    }
}

#[test]
fn typecheck_accepts_std_rr_with_scopes() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.rr_with_policy_v1_smoke",
                "params": [],
                "result": "result_bytes",
                "body": [
                    "std.rr.with_policy_v1",
                    ["bytes.lit", "smoke_rr_v1"],
                    ["bytes.lit", "smoke.rrbin"],
                    ["i32.lit", 2],
                    ["result_bytes.ok", ["bytes.alloc", 0]]
                ]
            },
            {
                "kind": "defn",
                "name": "main.rr_with_v1_smoke",
                "params": [],
                "result": "result_bytes",
                "body": [
                    "std.rr.with_v1",
                    ["bytes.view", ["bytes.lit", "cfg"]],
                    ["result_bytes.ok", ["bytes.alloc", 0]]
                ]
            }
        ],
        "solve": ["bytes.alloc", 0]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn typecheck_accepts_std_brand_symbol_args() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.cast_view",
                "params": [{"name":"doc","ty":"bytes_view"}],
                "result": "result_bytes_view",
                "result_brand": "types_pipes_lab.frame_payload_v1",
                "body": [
                    "std.brand.cast_view_v1",
                    "types_pipes_lab.frame_payload_v1",
                    "types_pipes_lab.schema.frame_payload_v1.validate_doc_v1",
                    "doc"
                ]
            },
            {
                "kind": "defn",
                "name": "main.view_full",
                "params": [],
                "result": "bytes_view",
                "body": ["std.brand.view_v1", ["bytes.lit", "abc"]]
            }
        ],
        "solve": ["bytes.alloc", 0]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn typecheck_accepts_std_stream_symbol_fields() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.smoke",
                "params": [],
                "result": "i32",
                "body": [
                    "begin",
                    ["std.stream.fn_v1", "main.mapper_v1"],
                    ["std.stream.xf.require_brand_v1",
                        ["brand", "types_pipes_lab.frame_payload_v1"],
                        ["validator", "brand_registry.validate_frame_payload_v1"],
                        ["max_item_bytes", 4096]
                    ],
                    0
                ]
            }
        ],
        "solve": ["bytes.alloc", 0]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());
    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        report.diagnostics
    );
}
