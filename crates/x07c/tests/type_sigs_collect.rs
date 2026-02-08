use serde_json::json;

use x07c::diagnostics::Location;
use x07c::typecheck::{typecheck_file_local, TypecheckOptions};

mod typecheck_testutil;

#[test]
fn sigs_collect_decl_ptr_accounts_for_export_and_extern_slots() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.4.0",
        "kind": "module",
        "module_id": "main",
        "imports": [],
        "decls": [
            {"kind": "export", "names": ["main.ext", "main.f"]},
            {
                "kind": "extern",
                "abi": "C",
                "name": "main.ext",
                "link_name": "main_ext",
                "params": [{"name": "x", "ty": "i32"}],
                "result": "i32"
            },
            {
                "kind": "defn",
                "name": "main.f",
                "params": [{"name": "x", "ty": "i32"}],
                "result": "i32",
                "body": "x"
            }
        ],
        "solve": ["begin", ["let", "y", ["main.f", 1, 2]], ["bytes.alloc", 0]]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-TYPE-CALL-0003")
        .expect("expected arity mismatch diagnostic");

    assert!(
        d.related
            .iter()
            .any(|l| matches!(l, Location::X07Ast { ptr } if ptr == "/decls/2")),
        "expected related location to point at main.f decl (/decls/2), got: {:?}",
        d.related
    );
}

#[test]
fn sigs_collect_tracks_generic_type_params_via_inference() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.4.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.id",
                "type_params": [{"name": "A"}],
                "params": [{"name": "x", "ty": ["t", "A"]}],
                "result": ["t", "A"],
                "body": "x"
            }
        ],
        "solve": ["begin", ["let", "x", ["main.id", 0]], ["bytes.alloc", 0]]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    let r = report
        .tapp_rewrites
        .iter()
        .find(|r| r.callee == "main.id")
        .expect("expected tapp rewrite for main.id");
    assert_eq!(
        r.inferred_type_args,
        vec![x07c::x07ast::TypeRef::Named("i32".to_string())]
    );
}
