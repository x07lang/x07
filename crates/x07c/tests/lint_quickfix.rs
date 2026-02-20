use serde_json::json;
use x07_contracts::X07AST_SCHEMA_VERSION;
use x07c::compile::{compile_program_to_c, CompileOptions};
use x07c::diagnostics::QuickfixKind;
use x07c::{json_patch, lint, x07ast};

fn parse_doc(src: &str) -> serde_json::Value {
    let mut v: serde_json::Value = serde_json::from_str(src).expect("valid JSON");
    v["schema_version"] = json!(X07AST_SCHEMA_VERSION);
    v
}

#[test]
fn lint_quickfix_wraps_for_varargs_body_in_begin() {
    let mut doc = parse_doc(
        r#"
        {
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

#[test]
fn lint_quickfix_copies_bytes_view_for_if_condition() {
    let mut doc = parse_doc(
        r#"
        {
          "kind":"entry",
          "module_id":"main",
          "imports":[],
          "decls":[],
          "solve":["begin",
            ["let","resp",["bytes.alloc",0]],
            ["if",
              ["=",["view.len",["bytes.view","resp"]],0],
              ["bytes.alloc",0],
              ["view.to_bytes",["bytes.view","resp"]]
            ]
          ]
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
        .find(|d| d.code == "X07-MOVE-0002")
        .expect("expected move diagnostic");
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

    let x07c::ast::Expr::List {
        items: inner_items, ..
    } = &items[2]
    else {
        panic!("expected nested begin at /solve/2");
    };
    assert_eq!(inner_items[0].as_ident(), Some("begin"));

    let x07c::ast::Expr::List {
        items: let_items, ..
    } = &inner_items[1]
    else {
        panic!("expected let binding in nested begin");
    };
    assert_eq!(let_items[0].as_ident(), Some("let"));
    assert_eq!(let_items[1].as_ident(), Some("_x07_tmp_copy"));
    assert_eq!(let_items.len(), 3, "let binding must have 3 elements total");

    let x07c::ast::Expr::List {
        items: init_items, ..
    } = &let_items[2]
    else {
        panic!("expected let init expression to be a list");
    };
    assert_eq!(init_items[0].as_ident(), Some("view.to_bytes"));
    let x07c::ast::Expr::List {
        items: init_view_items,
        ..
    } = &init_items[1]
    else {
        panic!("expected view.to_bytes arg to be a list");
    };
    assert_eq!(init_view_items[0].as_ident(), Some("bytes.view"));
    assert_eq!(init_view_items[1].as_ident(), Some("resp"));

    let x07c::ast::Expr::List {
        items: if_items, ..
    } = &inner_items[2]
    else {
        panic!("expected if in nested begin");
    };
    assert_eq!(if_items[0].as_ident(), Some("if"));

    fn find_bytes_view_owner(expr: &x07c::ast::Expr) -> Option<String> {
        match expr {
            x07c::ast::Expr::Int { .. } | x07c::ast::Expr::Ident { .. } => None,
            x07c::ast::Expr::List { items, .. } => {
                if items.len() == 2 && items[0].as_ident() == Some("bytes.view") {
                    return items[1].as_ident().map(|s| s.to_string());
                }
                for item in items {
                    if let Some(v) = find_bytes_view_owner(item) {
                        return Some(v);
                    }
                }
                None
            }
        }
    }

    let cond_owner = find_bytes_view_owner(&if_items[1]).expect("expected bytes.view in if cond");
    assert_eq!(cond_owner, "_x07_tmp_copy");
}

#[test]
fn lint_quickfix_copies_one_side_for_bytes_concat_and_compiles() {
    let mut doc = parse_doc(
        r#"
        {
          "kind":"entry",
          "module_id":"main",
          "imports":[],
          "decls":[],
          "solve":["begin",
            ["let","x",["bytes.lit","a"]],
            ["bytes.concat","x","x"]
          ]
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
        .find(|d| d.code == "X07-MOVE-0001")
        .expect("expected move diagnostic");
    let q = d.quickfix.as_ref().expect("expected quickfix");
    assert_eq!(q.kind, QuickfixKind::JsonPatch);
    json_patch::apply_patch(&mut doc, &q.patch).expect("apply quickfix patch");

    let patched_bytes = serde_json::to_vec(&doc).expect("serialize patched");
    let mut patched_file = x07ast::parse_x07ast_json(&patched_bytes).expect("reparse patched");
    x07ast::canonicalize_x07ast_file(&mut patched_file);
    let patched_report = lint::lint_file(&patched_file, lint::LintOptions::default());
    assert!(
        patched_report.ok,
        "expected lint ok after patch: {:?}",
        patched_report.diagnostics
    );

    compile_program_to_c(&patched_bytes, &CompileOptions::default())
        .expect("patched program must compile");
}
