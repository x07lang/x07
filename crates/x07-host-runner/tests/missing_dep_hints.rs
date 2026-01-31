use serde_json::json;
use x07_host_runner::{compile_program, RunnerConfig};
use x07_worlds::WorldId;

mod x07_program;

fn latest_ext_version(pkg_dir: &str) -> String {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("x07 repo root");
    let dir = root.join("packages").join("ext").join(pkg_dir);

    let mut best: Option<(u64, u64, u64, String)> = None;
    for entry in std::fs::read_dir(&dir).expect("read packages/ext") {
        let entry = entry.expect("read dir entry");
        if !entry.file_type().expect("file type").is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let parts: Vec<_> = name.split('.').collect();
        if parts.len() != 3 {
            continue;
        }
        let Ok(major) = parts[0].parse::<u64>() else {
            continue;
        };
        let Ok(minor) = parts[1].parse::<u64>() else {
            continue;
        };
        let Ok(patch) = parts[2].parse::<u64>() else {
            continue;
        };
        let key = (major, minor, patch, name.to_string());
        if best.as_ref().map(|b| (b.0, b.1, b.2)) < Some((major, minor, patch)) {
            best = Some(key);
        }
    }

    best.map(|t| t.3).expect("at least one semver dir")
}

fn config() -> RunnerConfig {
    RunnerConfig {
        world: WorldId::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_rr_index: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 20,
        debug_borrow_checks: false,
    }
}

#[test]
fn unknown_module_includes_pkg_add_hint_when_available() {
    let cfg = config();

    let program = x07_program::entry(&["std.text.ws"], json!(["view.to_bytes", "input"]));
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(!compile.ok);

    let msg = compile.compile_error.unwrap_or_default();
    let ext_text_ver = latest_ext_version("x07-ext-text");
    assert!(
        msg.contains(&format!("x07 pkg add ext-text@{ext_text_ver} --sync")),
        "compile_error={msg:?}"
    );
}
