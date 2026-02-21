#![no_main]

use x07_worlds::WorldId;
use x07c::compile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let data = if data.len() > 64 * 1024 {
        &data[..64 * 1024]
    } else {
        data
    };

    let Ok(v) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };

    let program_bytes: Vec<u8> = match v.as_object() {
        Some(obj) if obj.contains_key("schema_version") && obj.contains_key("kind") => {
            data.to_vec()
        }
        _ => {
            if x07c::ast::expr_from_json(&v).is_err() {
                return;
            }
            serde_json::to_vec(&serde_json::json!({
                "schema_version": "x07.x07ast@0.3.0",
                "kind": "entry",
                "module_id": "main",
                "imports": [],
                "decls": [],
                "solve": v,
            }))
            .unwrap_or_default()
        }
    };
    if program_bytes.is_empty() {
        return;
    }

    let opts = compile::CompileOptions {
        world: WorldId::SolvePure,
        enable_fs: false,
        enable_rr: false,
        enable_kv: false,
        module_roots: Vec::new(),
        arch_root: None,
        emit_main: false,
        freestanding: false,
        optimize: true,
        contract_mode: compile::ContractMode::RuntimeTrap,
        allow_unsafe: None,
        allow_ffi: None,
    };

    let _ = compile::compile_program_to_c_with_stats(&program_bytes, &opts);
});
