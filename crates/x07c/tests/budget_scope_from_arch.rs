use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;
use x07_contracts::{X07AST_SCHEMA_VERSION, X07_BUDGET_PROFILE_SCHEMA_VERSION};
use x07c::compile::{compile_program_to_c, CompileOptions};

fn create_temp_dir(prefix: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{pid}_{n}"))
}

#[test]
fn budget_scope_from_arch_v1_loads_profile_cfg() {
    let root = create_temp_dir("x07c_budget_scope_from_arch");
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove old temp dir");
    }
    std::fs::create_dir_all(&root).expect("create temp dir");

    let profile_id = "test.profile_v1";
    let profile_dir = root.join("arch").join("budgets").join("profiles");
    std::fs::create_dir_all(&profile_dir).expect("create profile dir");
    let profile_path = profile_dir.join(format!("{profile_id}.budget.json"));
    std::fs::write(
        &profile_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": X07_BUDGET_PROFILE_SCHEMA_VERSION,
            "id": profile_id,
            "v": 1,
            "cfg": {
                "mode": "trap_v1",
                "label": "test.profile_v1",
                "alloc_bytes": 123456789,
                "alloc_calls": 0,
                "realloc_calls": 0,
                "memcpy_bytes": 0,
                "sched_ticks": 0,
                "fuel": 0
            }
        }))
        .expect("encode profile json"),
    )
    .expect("write profile json");

    let program = serde_json::to_vec(&json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": [
            "budget.scope_from_arch_v1",
            ["bytes.lit", profile_id],
            ["bytes.alloc", 0]
        ],
    }))
    .expect("encode x07AST entry JSON");

    let options = CompileOptions {
        arch_root: Some(root),
        ..Default::default()
    };
    let c_src = compile_program_to_c(program.as_slice(), &options).expect("compile ok");
    assert!(
        c_src.contains("UINT64_C(123456789)"),
        "expected alloc_bytes limit in C output"
    );
}
