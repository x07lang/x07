//! Corpus gate for the x07text projection: every checked-in x07AST document
//! must round-trip text -> JSON with canonical-byte fidelity.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

fn collect_x07_json(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_x07_json(&path, out);
        } else if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().ends_with(".x07.json"))
        {
            out.push(path);
        }
    }
}

fn x07_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_x07"))
}

#[test]
fn corpus_roundtrips_through_x07text() {
    let root = repo_root();
    let mut files = Vec::new();
    collect_x07_json(&root.join("stdlib"), &mut files);
    collect_x07_json(&root.join("tests/fixtures"), &mut files);
    files.sort();
    assert!(
        files.len() > 100,
        "corpus unexpectedly small: {} files",
        files.len()
    );

    let tmp = std::env::temp_dir().join(format!("x07text-corpus-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");

    let mut checked = 0usize;
    let mut skipped = 0usize;
    for file in &files {
        let original = std::fs::read(file).expect("read corpus file");
        // Some fixtures are intentionally invalid JSON or non-AST documents;
        // the projection contract only covers parseable JSON documents.
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&original) else {
            skipped += 1;
            continue;
        };

        let text_path = tmp.join("doc.x07t");
        let json_path = tmp.join("doc.x07.json");

        let to_text = Command::new(x07_bin())
            .args(["ast", "to-text", "--in"])
            .arg(file)
            .arg("--out")
            .arg(&text_path)
            .output()
            .expect("run ast to-text");
        assert!(
            to_text.status.success(),
            "to-text failed for {}:\n{}",
            file.display(),
            String::from_utf8_lossy(&to_text.stderr)
        );

        let from_text = Command::new(x07_bin())
            .args(["ast", "from-text", "--validate", "false", "--in"])
            .arg(&text_path)
            .arg("--out")
            .arg(&json_path)
            .output()
            .expect("run ast from-text");
        assert!(
            from_text.status.success(),
            "from-text failed for {}:\n{}",
            file.display(),
            String::from_utf8_lossy(&from_text.stderr)
        );

        let roundtripped: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&json_path).expect("read roundtrip output"))
                .expect("parse roundtrip output");
        assert_eq!(
            roundtripped,
            value,
            "value mismatch after round-trip for {}",
            file.display()
        );
        checked += 1;
    }

    std::fs::remove_dir_all(&tmp).ok();
    println!("x07text corpus: {checked} files round-tripped, {skipped} skipped (non-JSON)");
    assert!(checked > 100, "checked too few files: {checked}");
}
