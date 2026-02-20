use serde_json::json;
use x07_contracts::X07AST_SCHEMA_VERSION;
use x07c::diagnostics::Report;
use x07c::{lint, x07ast};

fn lint_json(doc: &serde_json::Value) -> Report {
    let doc_bytes = serde_json::to_vec(doc).expect("serialize doc");
    let mut file = x07ast::parse_x07ast_json(&doc_bytes).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);
    lint::lint_file(&file, lint::LintOptions::default())
}

fn assert_mem_provenance(doc: &serde_json::Value, report: &Report, want_code: &str) {
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == want_code)
        .unwrap_or_else(|| {
            panic!(
                "expected diagnostic {want_code}, got: {:?}",
                report.diagnostics
            )
        });

    let mp = diag
        .data
        .get("mem_provenance")
        .unwrap_or_else(|| panic!("{want_code}: expected diagnostics.data.mem_provenance"));
    let mp_obj = mp
        .as_object()
        .unwrap_or_else(|| panic!("{want_code}: mem_provenance must be an object"));

    assert_eq!(
        mp_obj.get("schema_version").and_then(|v| v.as_str()),
        Some("x07.mem.provenance_graph@0.1.0"),
        "{want_code}: schema_version mismatch"
    );

    let focus_ptr = mp_obj
        .get("focus")
        .and_then(|v| v.get("ptr"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{want_code}: expected focus.ptr"));
    assert!(
        doc.pointer(focus_ptr).is_some(),
        "{want_code}: focus.ptr must resolve: {focus_ptr}"
    );

    let nodes = mp_obj
        .get("nodes")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("{want_code}: expected nodes[]"));
    let edges = mp_obj
        .get("edges")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("{want_code}: expected edges[]"));

    let mut node_ids: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for node in nodes {
        let node_obj = node
            .as_object()
            .unwrap_or_else(|| panic!("{want_code}: node must be object"));
        let id = node_obj
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{want_code}: node.id missing"));
        let ptr = node_obj
            .get("ptr")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{want_code}: node.ptr missing (id={id})"));
        assert!(
            doc.pointer(ptr).is_some(),
            "{want_code}: node.ptr must resolve: id={id} ptr={ptr}"
        );
        node_ids.insert(id);
    }

    for edge in edges {
        let edge_obj = edge
            .as_object()
            .unwrap_or_else(|| panic!("{want_code}: edge must be object"));
        let from = edge_obj
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{want_code}: edge.from missing"));
        let to = edge_obj
            .get("to")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{want_code}: edge.to missing"));
        assert!(
            node_ids.contains(from),
            "{want_code}: edge.from must refer to a node id: {from}"
        );
        assert!(
            node_ids.contains(to),
            "{want_code}: edge.to must refer to a node id: {to}"
        );
    }

    let violation_node = mp_obj
        .get("violation")
        .and_then(|v| v.get("node"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{want_code}: violation.node missing"));
    assert!(
        node_ids.contains(violation_node),
        "{want_code}: violation.node must refer to a node id: {violation_node}"
    );
}

#[test]
fn lint_emits_mem_provenance_for_borrow_from_temporary() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["view.to_bytes", ["bytes.view", ["bytes.lit", "hello"]]]
    });

    let r1 = lint_json(&doc);
    assert!(!r1.ok, "expected lint error");
    assert_mem_provenance(&doc, &r1, "X07-BORROW-0001");

    let r2 = lint_json(&doc);
    let mp1 = r1
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-BORROW-0001")
        .and_then(|d| d.data.get("mem_provenance"))
        .cloned()
        .expect("mem_provenance");
    let mp2 = r2
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-BORROW-0001")
        .and_then(|d| d.data.get("mem_provenance"))
        .cloned()
        .expect("mem_provenance");
    assert_eq!(mp1, mp2, "mem_provenance must be deterministic");
}

#[test]
fn lint_emits_mem_provenance_for_use_after_move_bytes_concat() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": [
            "begin",
            ["let", "b", ["bytes.lit", "hi"]],
            ["bytes.concat", "b", "b"]
        ]
    });

    let r1 = lint_json(&doc);
    assert!(!r1.ok, "expected lint error");
    assert_mem_provenance(&doc, &r1, "X07-MOVE-0001");

    let r2 = lint_json(&doc);
    let mp1 = r1
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-MOVE-0001")
        .and_then(|d| d.data.get("mem_provenance"))
        .cloned()
        .expect("mem_provenance");
    let mp2 = r2
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-MOVE-0001")
        .and_then(|d| d.data.get("mem_provenance"))
        .cloned()
        .expect("mem_provenance");
    assert_eq!(mp1, mp2, "mem_provenance must be deterministic");
}

#[test]
fn lint_emits_mem_provenance_for_if_bytes_view_borrow_conflict() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["begin",
          ["let", "resp", ["bytes.alloc", 0]],
          ["if",
            ["=", ["view.len", ["bytes.view", "resp"]], 0],
            ["bytes.alloc", 0],
            ["view.to_bytes", ["bytes.view", "resp"]]
          ]
        ]
    });

    let r1 = lint_json(&doc);
    assert!(!r1.ok, "expected lint error");
    assert_mem_provenance(&doc, &r1, "X07-MOVE-0002");

    let r2 = lint_json(&doc);
    let mp1 = r1
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-MOVE-0002")
        .and_then(|d| d.data.get("mem_provenance"))
        .cloned()
        .expect("mem_provenance");
    let mp2 = r2
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-MOVE-0002")
        .and_then(|d| d.data.get("mem_provenance"))
        .cloned()
        .expect("mem_provenance");
    assert_eq!(mp1, mp2, "mem_provenance must be deterministic");
}
