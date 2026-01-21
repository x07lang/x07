use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use x07_contracts::{NATIVE_BACKENDS_SCHEMA_VERSION, NATIVE_REQUIRES_SCHEMA_VERSION};
use x07_host_runner::plan_native_link_argv;
use x07c::native::{NativeBackendReq, NativeRequires};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    base.join(format!("{prefix}_{pid}_{n}"))
}

fn write_bytes(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    std::fs::write(path, bytes).expect("write file");
}

const MANIFEST_JSON: &str = r#"
{
  "schema_version": "x07.native-backends@0.1.0",
  "backends": [
    {
      "backend_id": "x07.ext.net",
      "abi_major": 1,
      "link": {
        "linux": { "kind": "static", "files": ["deps/x07/libx07_ext_net.a"], "args": ["-pthread"], "search_paths": [], "force_load": false, "whole_archive": false },
        "macos": { "kind": "static", "files": ["deps/x07/libx07_ext_net.a"], "args": [], "search_paths": [], "force_load": false, "whole_archive": false },
        "windows-msvc": { "kind": "static", "files": ["deps/x07/x07_ext_net.lib"], "args": ["Ws2_32.lib"], "search_paths": [], "force_load": false, "whole_archive": false },
        "windows-gnu": { "kind": "static", "files": ["deps/x07/libx07_ext_net.a"], "args": ["-lws2_32"], "search_paths": [], "force_load": false, "whole_archive": false }
      }
    },
    {
      "backend_id": "x07.ext.regex",
      "abi_major": 1,
      "link": {
        "linux": { "kind": "static", "files": ["deps/x07/libx07_ext_regex.a"], "args": [], "search_paths": [], "force_load": false, "whole_archive": false },
        "macos": { "kind": "static", "files": ["deps/x07/libx07_ext_regex.a"], "args": [], "search_paths": [], "force_load": false, "whole_archive": false },
        "windows-msvc": { "kind": "static", "files": ["deps/x07/x07_ext_regex.lib"], "args": [], "search_paths": [], "force_load": false, "whole_archive": false },
        "windows-gnu": { "kind": "static", "files": ["deps/x07/libx07_ext_regex.a"], "args": [], "search_paths": [], "force_load": false, "whole_archive": false }
      }
    },
    {
      "backend_id": "x07.ext.sqlite3",
      "abi_major": 1,
      "link": {
        "linux": { "kind": "static", "files": ["deps/x07/libx07_ext_sqlite3.a"], "args": [], "search_paths": [], "force_load": false, "whole_archive": false },
        "macos": { "kind": "static", "files": ["deps/x07/libx07_ext_sqlite3.a"], "args": [], "search_paths": [], "force_load": false, "whole_archive": false },
        "windows-msvc": { "kind": "static", "files": ["deps/x07/x07_ext_sqlite3.lib"], "args": [], "search_paths": [], "force_load": false, "whole_archive": false },
        "windows-gnu": { "kind": "static", "files": ["deps/x07/libx07_ext_sqlite3.a"], "args": [], "search_paths": [], "force_load": false, "whole_archive": false }
      }
    }
  ]
}
"#;

fn write_fixture_toolchain_root(root: &Path) {
    write_bytes(
        &root.join("deps/x07/native_backends.json"),
        MANIFEST_JSON.as_bytes(),
    );

    for rel in [
        "deps/x07/libx07_ext_net.a",
        "deps/x07/libx07_ext_regex.a",
        "deps/x07/libx07_ext_sqlite3.a",
        "deps/x07/x07_ext_net.lib",
        "deps/x07/x07_ext_regex.lib",
        "deps/x07/x07_ext_sqlite3.lib",
    ] {
        write_bytes(&root.join(rel), b"dummy");
    }
}

fn requires_doc() -> NativeRequires {
    NativeRequires {
        schema_version: NATIVE_REQUIRES_SCHEMA_VERSION.to_string(),
        world: None,
        requires: vec![
            NativeBackendReq {
                backend_id: "x07.ext.regex".to_string(),
                abi_major: 1,
                features: Vec::new(),
            },
            NativeBackendReq {
                backend_id: "x07.ext.sqlite3".to_string(),
                abi_major: 1,
                features: Vec::new(),
            },
            NativeBackendReq {
                backend_id: "x07.ext.net".to_string(),
                abi_major: 1,
                features: Vec::new(),
            },
        ],
    }
}

#[test]
fn fixture_constants_match() {
    assert_eq!(NATIVE_BACKENDS_SCHEMA_VERSION, "x07.native-backends@0.1.0");
    assert_eq!(NATIVE_REQUIRES_SCHEMA_VERSION, "x07.native-requires@0.1.0");
}

#[test]
#[cfg(target_os = "linux")]
fn native_link_argv_linux_exact() {
    let dir = temp_dir("x07_native_link_linux");
    write_fixture_toolchain_root(&dir);

    let argv = plan_native_link_argv(&dir, &requires_doc()).expect("plan argv");

    let expected = vec![
        "-pthread".to_string(),
        "-Wl,--start-group".to_string(),
        dir.join("deps/x07/libx07_ext_net.a")
            .to_string_lossy()
            .to_string(),
        dir.join("deps/x07/libx07_ext_regex.a")
            .to_string_lossy()
            .to_string(),
        dir.join("deps/x07/libx07_ext_sqlite3.a")
            .to_string_lossy()
            .to_string(),
        "-Wl,--end-group".to_string(),
    ];

    assert_eq!(argv, expected);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[cfg(target_os = "macos")]
fn native_link_argv_macos_exact() {
    let dir = temp_dir("x07_native_link_macos");
    write_fixture_toolchain_root(&dir);

    let argv = plan_native_link_argv(&dir, &requires_doc()).expect("plan argv");

    let expected = vec![
        dir.join("deps/x07/libx07_ext_net.a")
            .to_string_lossy()
            .to_string(),
        dir.join("deps/x07/libx07_ext_regex.a")
            .to_string_lossy()
            .to_string(),
        dir.join("deps/x07/libx07_ext_sqlite3.a")
            .to_string_lossy()
            .to_string(),
    ];

    assert_eq!(argv, expected);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[cfg(windows)]
fn native_link_argv_windows_msvc_exact() {
    let prev_cc = std::env::var_os("X07_CC");
    std::env::set_var("X07_CC", "cl.exe");

    let dir = temp_dir("x07_native_link_windows");
    write_fixture_toolchain_root(&dir);

    let argv = plan_native_link_argv(&dir, &requires_doc()).expect("plan argv");

    let expected = vec![
        "Ws2_32.lib".to_string(),
        dir.join("deps")
            .join("x07")
            .join("x07_ext_net.lib")
            .to_string_lossy()
            .to_string(),
        dir.join("deps")
            .join("x07")
            .join("x07_ext_regex.lib")
            .to_string_lossy()
            .to_string(),
        dir.join("deps")
            .join("x07")
            .join("x07_ext_sqlite3.lib")
            .to_string_lossy()
            .to_string(),
    ];

    assert_eq!(argv, expected);

    let _ = std::fs::remove_dir_all(&dir);

    match prev_cc {
        Some(v) => std::env::set_var("X07_CC", v),
        None => std::env::remove_var("X07_CC"),
    }
}

#[test]
#[cfg(windows)]
fn native_link_argv_windows_gnu_exact() {
    let prev_cc = std::env::var_os("X07_CC");
    std::env::set_var("X07_CC", "gcc");

    let dir = temp_dir("x07_native_link_windows_gnu");
    write_fixture_toolchain_root(&dir);

    let argv = plan_native_link_argv(&dir, &requires_doc()).expect("plan argv");

    let expected = vec![
        "-lws2_32".to_string(),
        dir.join("deps/x07/libx07_ext_net.a")
            .to_string_lossy()
            .to_string(),
        dir.join("deps/x07/libx07_ext_regex.a")
            .to_string_lossy()
            .to_string(),
        dir.join("deps/x07/libx07_ext_sqlite3.a")
            .to_string_lossy()
            .to_string(),
    ];

    assert_eq!(argv, expected);

    let _ = std::fs::remove_dir_all(&dir);

    match prev_cc {
        Some(v) => std::env::set_var("X07_CC", v),
        None => std::env::remove_var("X07_CC"),
    }
}
