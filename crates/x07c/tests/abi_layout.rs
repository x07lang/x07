use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn abi_layout_c_static_asserts_compile() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let include_dir = repo_root.join("crates/x07c/include");

    let rel = "crates/x07c/tests/abi_layout.c";
    let c_path = repo_root.join(rel);
    let cc = std::env::var_os("X07_CC").unwrap_or_else(|| OsStr::new("cc").to_os_string());
    let null_out = "/dev/null";

    let out = Command::new(cc)
        .args([
            "-std=c11",
            "-Werror",
            "-I",
            include_dir.to_str().expect("include dir must be UTF-8"),
            "-c",
            c_path.to_str().expect("c path must be UTF-8"),
            "-o",
            null_out,
        ])
        .output()
        .expect("run cc");

    assert!(
        out.status.success(),
        "C ABI layout test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
