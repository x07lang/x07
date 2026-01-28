use x07c::diagnostics::QuickfixKind;
use x07c::{json_patch, lint, x07ast};

fn parse_doc(src: &str) -> serde_json::Value {
    serde_json::from_str(src).expect("valid JSON")
}

#[test]
fn lint_quickfix_wraps_for_varargs_body_in_begin() {
    let mut doc = parse_doc(
        r#"
        {
          "schema_version":"x07.x07ast@0.2.0",
          "kind":"entry",
          "module_id":"main",
          "imports":[],
          "decls":[],
          "solve":["for","i",0,1,["let","x",0],["set","x",1],0]
        }
        "#,
    );

    let doc_bytes = serde_json::to_vec(&doc).expect("serialize doc");
    let mut file = x07ast::parse_x07ast_json(&doc_bytes).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);
    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(!report.ok, "expected lint errors");

    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-ARITY-FOR-0001")
        .expect("expected for arity diagnostic");
    let q = d.quickfix.as_ref().expect("expected quickfix");
    assert_eq!(q.kind, QuickfixKind::JsonPatch);

    json_patch::apply_patch(&mut doc, &q.patch).expect("apply quickfix patch");

    let patched_bytes = serde_json::to_vec(&doc).expect("serialize patched");
    let file = x07ast::parse_x07ast_json(&patched_bytes).expect("reparse patched");
    let solve = file.solve.expect("solve present");
    let x07c::ast::Expr::List { items, .. } = solve else {
        panic!("solve must be a list after patch");
    };
    assert_eq!(items[0].as_ident(), Some("for"));
    assert_eq!(items.len(), 5, "for list must have 5 elements total");
    let x07c::ast::Expr::List {
        items: body_items, ..
    } = &items[4]
    else {
        panic!("for body must be a list");
    };
    assert_eq!(body_items[0].as_ident(), Some("begin"));
}

#[test]
fn lint_quickfix_rewrites_let_with_body_into_begin() {
    let mut doc = parse_doc(
        r#"
        {
          "schema_version":"x07.x07ast@0.2.0",
          "kind":"entry",
          "module_id":"main",
          "imports":[],
          "decls":[],
          "solve":["let","x",0,["set","x",1],["bytes.alloc",0]]
        }
        "#,
    );

    let doc_bytes = serde_json::to_vec(&doc).expect("serialize doc");
    let mut file = x07ast::parse_x07ast_json(&doc_bytes).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);
    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(!report.ok, "expected lint errors");

    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-ARITY-LET-0001")
        .expect("expected let arity diagnostic");
    let q = d.quickfix.as_ref().expect("expected quickfix");
    assert_eq!(q.kind, QuickfixKind::JsonPatch);

    json_patch::apply_patch(&mut doc, &q.patch).expect("apply quickfix patch");

    let patched_bytes = serde_json::to_vec(&doc).expect("serialize patched");
    let file = x07ast::parse_x07ast_json(&patched_bytes).expect("reparse patched");
    let solve = file.solve.expect("solve present");
    let x07c::ast::Expr::List { items, .. } = solve else {
        panic!("solve must be a list after patch");
    };
    assert_eq!(items[0].as_ident(), Some("begin"));
}

#[test]
fn lint_quickfix_removes_forbidden_imports() {
    let mut doc = parse_doc(
        r#"
        {
          "schema_version":"x07.x07ast@0.2.0",
          "kind":"entry",
          "module_id":"main",
          "imports":["std.fs","std.bytes"],
          "decls":[],
          "solve":["bytes.alloc",0]
        }
        "#,
    );

    let doc_bytes = serde_json::to_vec(&doc).expect("serialize doc");
    let mut file = x07ast::parse_x07ast_json(&doc_bytes).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);
    let report = lint::lint_file(
        &file,
        lint::LintOptions {
            world: Default::default(),
            enable_fs: false,
            enable_rr: false,
            enable_kv: false,
            allow_unsafe: None,
            allow_ffi: None,
        },
    );
    assert!(!report.ok, "expected lint errors");
    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-WORLD-0001")
        .expect("expected world import diagnostic");
    let q = d.quickfix.as_ref().expect("expected quickfix");
    assert_eq!(q.kind, QuickfixKind::JsonPatch);
    json_patch::apply_patch(&mut doc, &q.patch).expect("apply quickfix patch");

    let patched_bytes = serde_json::to_vec(&doc).expect("serialize patched");
    let file = x07ast::parse_x07ast_json(&patched_bytes).expect("reparse patched");
    assert!(!file.imports.contains("std.fs"));
    assert!(file.imports.contains("std.bytes"));
}
