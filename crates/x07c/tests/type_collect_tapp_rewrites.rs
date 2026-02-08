use serde_json::json;

use x07c::typecheck::{typecheck_file_local, TypecheckOptions};
use x07c::x07ast::TypeRef;

mod typecheck_testutil;

#[test]
fn collect_tapp_rewrite_for_simple_generic_call() {
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

    assert_eq!(
        report.tapp_rewrites.len(),
        1,
        "expected exactly one tapp rewrite, got: {:?}",
        report.tapp_rewrites
    );
    let r = report.tapp_rewrites.first().expect("len == 1");
    assert_eq!(r.call_ptr, "/solve/1/2");
    assert_eq!(r.callee, "main.id");
    assert_eq!(
        r.inferred_type_args,
        vec![TypeRef::Named("i32".to_string())]
    );
}
